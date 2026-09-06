//! # fo3_viewer
//!
//! Fallout 3 メッシュ & セルシーンビューアー (winit + wgpu)。
//!
//! 使用法:
//!   - NIF 単体表示: `cargo run -p fo3_viewer -- <data_dir> <nif_relative_path>`
//!   - セル一括表示: `cargo run -p fo3_viewer -- cell <data_dir> <cell_edid>`
//!
//! 例:
//!   `cargo run -p fo3_viewer -- "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "meshes\weapons\1handpistol\10mmpistol.nif"`
//!   `cargo run -p fo3_viewer -- cell "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "Vault101a"`
//!   `cargo run -p fo3_viewer -- cell "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "MegatonCommonHouse"`

use std::collections::HashMap;
use std::env;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use fo3_bsa::BsaArchive;
use fo3_esm::EsmReader;
use fo3_gamebryo_core::NiTransform;
use fo3_nif::NifFile;
use fo3_render::{OrbitCamera, RenderContext, RenderScene};
use fo3_vfs::VfsManager;
use pollster::FutureExt;
use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

/// ビューアーの表示対象。
#[derive(Clone, Debug)]
enum ViewerTarget {
    Mesh(String),
    Cell(String),
}

struct ViewerState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    context: RenderContext,
    camera: OrbitCamera,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    depth_view: wgpu::TextureView,
    scene: RenderScene,
    // マウス入力状態
    left_mouse_down: bool,
    right_mouse_down: bool,
    last_mouse_pos: Option<(f64, f64)>,
}

impl ViewerState {
    async fn new(window: Arc<Window>, data_dir: &str, target: &ViewerTarget) -> Self {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).expect("Failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find suitable GPU adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Device"),
                    required_features: wgpu::Features::TEXTURE_COMPRESSION_BC,
                    required_limits: wgpu::Limits::default(),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .expect("Failed to create device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let context = RenderContext::new(&device, surface_format);
        let depth_view = RenderContext::create_depth_texture(&device, width, height);

        // VFS の初期化
        let mut vfs = VfsManager::new();
        let data_p = Path::new(data_dir);
        vfs.add_loose_root(data_p);

        let bsa_names = [
            "Fallout - Meshes.bsa",
            "Fallout - Textures.bsa",
            "Fallout - Misc.bsa",
        ];
        for bsa_name in &bsa_names {
            let bsa_file = data_p.join(bsa_name);
            if bsa_file.exists() {
                if let Ok(archive) = BsaArchive::open(&bsa_file) {
                    vfs.add_bsa(archive);
                }
            }
        }

        // 表示ターゲットに応じたシーンの構築
        let scene = match target {
            ViewerTarget::Mesh(nif_path) => {
                println!("VFS から NIF ファイルを取得中: {}", nif_path);
                let nif_bytes = vfs.read(nif_path).expect("Failed to read NIF from VFS");
                let mut cursor = Cursor::new(nif_bytes);
                let nif_file = NifFile::read(&mut cursor).expect("Failed to parse NIF");
                println!("NIF パース成功 (ブロック数: {})。GPU シーンを構築中...", nif_file.blocks.len());
                RenderScene::from_nif(&device, &queue, &context, &nif_file, &mut vfs)
            }
            ViewerTarget::Cell(cell_edid) => {
                println!("ESM からセル \"{}\" を検索中...", cell_edid);
                let esm_path = data_p.join("Fallout3.esm");
                let mut esm_reader = EsmReader::open(&esm_path).expect("Failed to open Fallout3.esm");
                println!("3D モデル保持レコード (STAT, SCOL, DOOR, ACTI, FURN, etc.) を一括走査中...");
                let model_map = esm_reader.read_all_models_map().expect("Failed to read models map");
                println!("モデルマップ登録件数: {} 件", model_map.len());

                let (cell, refrs, land) = esm_reader
                    .find_cell_by_edid(cell_edid)
                    .expect("Failed to find cell")
                    .unwrap_or_else(|| panic!("セル \"{}\" が見つかりませんでした", cell_edid));

                println!(
                    "セル取得成功: \"{}\" (表示名: {:?}, REFR総数: {}, 地形LAND: {})",
                    cell.edid,
                    cell.full_name,
                    refrs.len(),
                    if land.is_some() { "あり" } else { "なし" }
                );

                let mut nif_cache: HashMap<String, Arc<NifFile>> = HashMap::new();
                let mut placed_items: Vec<(Arc<NifFile>, NiTransform)> = Vec::new();
                let mut skipped_markers = 0;

                for refr in &refrs {
                    if let Some(obj_info) = model_map.get(&refr.base_object) {
                        if obj_info.model.is_empty() {
                            continue;
                        }

                        // エディタ専用マーカー（矢印等）や光線エフェクト（真っ白な板になる）を除外
                        if is_editor_marker_or_effect(&obj_info.edid, &obj_info.model) {
                            skipped_markers += 1;
                            continue;
                        }

                        let model_key = obj_info.model.to_ascii_lowercase();
                        let nif = if let Some(n) = nif_cache.get(&model_key) {
                            n.clone()
                        } else {
                            let mesh_path = if model_key.starts_with("meshes\\") || model_key.starts_with("meshes/") {
                                obj_info.model.clone()
                            } else {
                                format!("meshes\\{}", obj_info.model)
                            };
                            match vfs.read(&mesh_path) {
                                Ok(bytes) => {
                                    let mut cursor = Cursor::new(bytes);
                                    match NifFile::read(&mut cursor) {
                                        Ok(parsed) => {
                                            let arc = Arc::new(parsed);
                                            nif_cache.insert(model_key, arc.clone());
                                            arc
                                        }
                                        Err(_) => continue,
                                    }
                                }
                                Err(_) => continue,
                            }
                        };

                        let pos = glam::Vec3::new(refr.position[0], refr.position[1], refr.position[2]);
                        let rot = glam::Vec3::new(refr.rotation[0], refr.rotation[1], refr.rotation[2]);
                        let world_transform = NiTransform::from_euler_xyz(pos, rot, refr.scale);
                        placed_items.push((nif, world_transform));
                    }
                }

                println!(
                    "配置メッシュロード完了: {} 件 (マーカー/エフェクト除外: {} 件)。GPU シーン構築中...",
                    placed_items.len(),
                    skipped_markers
                );
                let placed_refs: Vec<(&NifFile, NiTransform)> =
                    placed_items.iter().map(|(n, t)| (n.as_ref(), *t)).collect();
                let land_info = land.as_ref().and_then(|l| {
                    cell.grid.map(|(gx, gy)| (l, gx, gy))
                });
                RenderScene::from_cell(&device, &queue, &context, &placed_refs, land_info, &mut vfs)
            }
        };

        println!("GPU シーン構築完了: {} メッシュノード描画準備完了", scene.meshes.len());

        let title = match target {
            ViewerTarget::Mesh(path) => format!("OpenFallout3 - Mesh: {}", path),
            ViewerTarget::Cell(edid) => format!("OpenFallout3 - Cell: {}", edid),
        };
        window.set_title(&title);

        // カメラの初期化（シーン全体のバウンディングに自動フォーカス）
        let aspect = width as f32 / height as f32;
        let mut camera = OrbitCamera::new(aspect);
        camera.focus(scene.bounds_center, scene.bounds_radius);
        println!(
            "カメラ自動フォーカス: 注視点 {:?}, 距離 {:.1}",
            camera.target, camera.distance
        );

        let camera_uniform = camera.build_uniform();
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &context.camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        ViewerState {
            window,
            surface,
            device,
            queue,
            config,
            size,
            context,
            camera,
            camera_buffer,
            camera_bind_group,
            depth_view,
            scene,
            left_mouse_down: false,
            right_mouse_down: false,
            last_mouse_pos: None,
        }
    }

    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_view = RenderContext::create_depth_texture(
                &self.device,
                new_size.width,
                new_size.height,
            );
            self.camera.aspect = new_size.width as f32 / new_size.height as f32;
        }
    }

    fn update(&mut self) {
        let uniform = self.camera.build_uniform();
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[uniform]),
        );
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.12,
                            b: 0.15,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.context.pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            self.scene.render(&mut render_pass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

struct App {
    data_dir: String,
    target: ViewerTarget,
    state: Option<ViewerState>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title("OpenFallout3 Viewer")
                .with_inner_size(PhysicalSize::new(1280, 720));
            let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
            let state = ViewerState::new(window.clone(), &self.data_dir, &self.target).block_on();
            self.state = Some(state);
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = match self.state.as_mut() {
            Some(s) => s,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                state.resize(physical_size);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                state.update();
                match state.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                    Err(e) => eprintln!("レンダリングエラー: {:?}", e),
                }
            }
            WindowEvent::MouseInput { button, state: btn_state, .. } => {
                let pressed = btn_state == ElementState::Pressed;
                match button {
                    MouseButton::Left => state.left_mouse_down = pressed,
                    MouseButton::Right => state.right_mouse_down = pressed,
                    _ => {}
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some((last_x, last_y)) = state.last_mouse_pos {
                    let dx = (position.x - last_x) as f32;
                    let dy = (position.y - last_y) as f32;

                    if state.left_mouse_down {
                        state.camera.rotate(dx, dy);
                        state.window.request_redraw();
                    } else if state.right_mouse_down {
                        state.camera.pan(dx, dy);
                        state.window.request_redraw();
                    }
                }
                state.last_mouse_pos = Some((position.x, position.y));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let zoom_amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.05,
                };
                state.camera.zoom(zoom_amount);
                state.window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    if let PhysicalKey::Code(key) = event.physical_key {
                        match key {
                            KeyCode::KeyR => {
                                state.camera.focus(state.scene.bounds_center, state.scene.bounds_radius);
                                state.window.request_redraw();
                            }
                            KeyCode::Escape => {
                                event_loop.exit();
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("使用法:");
        println!("  - メッシュ単体表示: cargo run -p fo3_viewer -- <DataDir> <RelativeNifPath>");
        println!("  - セル一括表示:     cargo run -p fo3_viewer -- cell <DataDir> <CellEDID>");
        println!("例:");
        println!("  cargo run -p fo3_viewer -- \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"meshes\\weapons\\1handpistol\\10mmpistol.nif\"");
        println!("  cargo run -p fo3_viewer -- cell \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"Vault101a\"");
        println!("  cargo run -p fo3_viewer -- cell \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"MegatonCommonHouse\"");
        return Ok(());
    }

    let (data_dir, target) = if args[1] == "cell" {
        if args.len() < 4 {
            eprintln!("エラー: セル表示モードには <DataDir> と <CellEDID> が必要です。");
            return Ok(());
        }
        (args[2].clone(), ViewerTarget::Cell(args[3].clone()))
    } else {
        (args[1].clone(), ViewerTarget::Mesh(args[2].clone()))
    };

    let event_loop = EventLoop::new()?;
    let mut app = App {
        data_dir,
        target,
        state: None,
    };

    event_loop.run_app(&mut app)?;

    Ok(())
}

/// エディタ用配置マーカー（矢印、ボックス）や光線エフェクトメッシュ（真っ白な板になる）かどうかを判定。
fn is_editor_marker_or_effect(edid: &str, model: &str) -> bool {
    let lower_model = model.to_ascii_lowercase();
    let lower_edid = edid.to_ascii_lowercase();

    // エディタ専用マーカー (矢印 MarkerXHeading、不可視ドアマーカー等)
    if lower_model.contains("marker") || lower_edid.contains("marker") {
        return true;
    }
    // 環境光線・グローエフェクト (不透明ジオメトリ描画では真っ白な板として現れてしまうもの)
    if lower_model.contains("lightbeam")
        || lower_model.contains("glow")
        || lower_model.contains("ray")
        || lower_model.starts_with("effects\\")
        || lower_model.starts_with("effects/")
    {
        return true;
    }

    false
}
