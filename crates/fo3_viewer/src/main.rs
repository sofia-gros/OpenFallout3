//! # fo3_viewer
//!
//! Fallout 3 メッシュ & セルシーンビューアー (winit + wgpu)。
//!
//! 使用法:
//!   - NIF 単体表示: `cargo run -p fo3_viewer -- <data_dir> <nif_relative_path>`
//!   - セル一括表示: `cargo run -p fo3_viewer -- cell <data_dir> <cell_edid>`
//!   - ワールド表示: `cargo run -p fo3_viewer -- world <data_dir> <world_edid> [grid_x] [grid_y]`
//!
//! 例:
//!   `cargo run -p fo3_viewer -- "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "meshes\weapons\1handpistol\10mmpistol.nif"`
//!   `cargo run -p fo3_viewer -- cell "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "Vault101a"`
//!   `cargo run -p fo3_viewer -- cell "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "Springvale"`
//!   `cargo run -p fo3_viewer -- world "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "MegatonWorld"`
//!   `cargo run -p fo3_viewer -- world "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "DCWorld01"`
//!   `cargo run -p fo3_viewer -- world "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "Wasteland" -1 2`

use std::collections::HashMap;
use std::env;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use fo3_bsa::BsaArchive;
use fo3_esm::{CellLighting, CellRecord, EsmReader, LandRecord, RefrRecord};
use fo3_gamebryo_core::NiTransform;
use fo3_nif::NifFile;
use fo3_render::{LightingUniform, OrbitCamera, PlacedPointLight, RenderContext, RenderScene};
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
    World(String, Option<(i32, i32)>),
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
    lighting_buffer: wgpu::Buffer,
    cell_lighting: Option<CellLighting>,
    placed_lights: Vec<PlacedPointLight>,
    clear_color: wgpu::Color,
    depth_view: wgpu::TextureView,
    scene: RenderScene,
    show_collision: bool,
    enable_fog: bool,
    headlight: bool,
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
        let (scene, cell_lighting, placed_lights, clear_color) = match target {
            ViewerTarget::Mesh(nif_path) => {
                println!("VFS から NIF ファイルを取得中: {}", nif_path);
                let nif_bytes = vfs.read(nif_path).expect("Failed to read NIF from VFS");
                let mut cursor = Cursor::new(nif_bytes);
                let nif_file = NifFile::read(&mut cursor).expect("Failed to parse NIF");
                println!("NIF パース成功 (ブロック数: {})。GPU シーンを構築中...", nif_file.blocks.len());
                let scene = RenderScene::from_nif(&device, &queue, &context, &nif_file, &mut vfs);
                (
                    scene,
                    None,
                    Vec::new(),
                    wgpu::Color {
                        r: 0.1,
                        g: 0.12,
                        b: 0.15,
                        a: 1.0,
                    },
                )
            }
            ViewerTarget::Cell(..) | ViewerTarget::World(..) => {
                let esm_path = data_p.join("Fallout3.esm");
                let mut esm_reader = EsmReader::open(&esm_path).expect("Failed to open Fallout3.esm");

                // セル群の検索
                let cells: Vec<(CellRecord, Vec<RefrRecord>, Option<LandRecord>)> = match target {
                    ViewerTarget::Cell(cell_edid) => {
                        println!("ESM からセル \"{}\" および近傍セルを検索中...", cell_edid);
                        esm_reader
                            .find_cell_and_neighbors(cell_edid, 1)
                            .expect("Failed to find cell and neighbors")
                            .unwrap_or_else(|| panic!("セル \"{}\" が見つかりませんでした", cell_edid))
                    }
                    ViewerTarget::World(world_edid, grid_opt) => {
                        println!("ESM からワールド \"{}\" を検索中...", world_edid);
                        let (world_rec, group_start, group_end) = esm_reader
                            .find_world_by_edid(world_edid)
                            .expect("Failed to search world")
                            .unwrap_or_else(|| panic!("ワールドスペース \"{}\" が見つかりませんでした", world_edid));
                        println!(
                            "ワールドスペース発見: \"{}\" (FormID: 0x{:08X}, 表示名: {:?})",
                            world_rec.edid, world_rec.form_id.0, world_rec.full_name
                        );
                        println!("ワールド所属セル群を走査中 (中心グリッド: {:?}, 半径: 1)...", grid_opt);
                        let (resolved_center, cells) = esm_reader
                            .read_cells_in_world_region(group_start, group_end, *grid_opt, 1)
                            .expect("Failed to read world cells");
                        println!("走査結果: 解決中心グリッド: {:?}, 取得セル数: {}", resolved_center, cells.len());
                        if cells.is_empty() {
                            panic!("ワールド \"{}\" 内に指定グリッドのセルが見つかりませんでした", world_edid);
                        }
                        cells
                    }
                    _ => unreachable!(),
                };

                println!("取得セル総数: {} 件", cells.len());
                for (c, r, l) in &cells {
                    println!(
                        "  - セル \"{}\" (Grid: {:?}, REFR: {}, LAND: {})",
                        c.edid,
                        c.grid,
                        r.len(),
                        if l.is_some() { "あり" } else { "なし" }
                    );
                }

                println!("3D モデル保持レコード (STAT, SCOL, DOOR, ACTI, FURN, etc.) を一括走査中...");
                let model_map = esm_reader.read_all_models_map().expect("Failed to read models map");
                println!("モデルマップ登録件数: {} 件", model_map.len());
                let light_map = esm_reader.read_light_map().unwrap_or_default();
                println!("光源レコード (LIGHT) 登録件数: {} 件", light_map.len());

                let mut nif_cache: HashMap<String, Arc<NifFile>> = HashMap::new();
                let mut all_cell_items: Vec<Vec<(Arc<NifFile>, NiTransform)>> = Vec::new();
                let mut placed_lights: Vec<PlacedPointLight> = Vec::new();
                let mut skipped_markers = 0;
                let mut primary_lighting: Option<CellLighting> = None;

                for (cell, refrs, _) in &cells {
                    if primary_lighting.is_none() && cell.lighting.is_some() {
                        primary_lighting = cell.lighting.clone();
                    }

                    let mut cell_items: Vec<(Arc<NifFile>, NiTransform)> = Vec::new();
                    for refr in refrs {
                        // 配置点光源の収集 (LIGHT レコード)
                        if let Some(light_rec) = light_map.get(&refr.base_object) {
                            let pos = glam::Vec3::new(refr.position[0], refr.position[1], refr.position[2]);
                            let r = light_rec.colour[0] as f32 / 255.0;
                            let g = light_rec.colour[1] as f32 / 255.0;
                            let b = light_rec.colour[2] as f32 / 255.0;
                            let radius = if light_rec.radius > 0 { light_rec.radius as f32 } else { 500.0 };
                            let falloff = if light_rec.falloff > 0.01 { light_rec.falloff } else { 1.0 };
                            placed_lights.push(PlacedPointLight {
                                position: pos,
                                radius,
                                color: [r, g, b],
                                falloff,
                            });
                        }

                        // 配置メッシュの収集
                        if let Some(obj_info) = model_map.get(&refr.base_object) {
                            if obj_info.model.is_empty() {
                                continue;
                            }

                            // エディタ専用マーカーや光線エフェクトを除外
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
                            cell_items.push((nif, world_transform));
                        }
                    }
                    all_cell_items.push(cell_items);
                }

                let total_placed: usize = all_cell_items.iter().map(|it| it.len()).sum();
                println!(
                    "全セル配置メッシュロード完了: {} 件 (マーカー/エフェクト除外: {} 件), 点光源: {} 灯。GPU シーン構築中...",
                    total_placed,
                    skipped_markers,
                    placed_lights.len()
                );

                let cell_inputs: Vec<(Vec<(&NifFile, NiTransform)>, Option<(&LandRecord, i32, i32)>)> =
                    cells.iter().zip(all_cell_items.iter()).map(|((cell, _, land), items)| {
                        let placed_refs: Vec<(&NifFile, NiTransform)> = items.iter().map(|(n, t)| (n.as_ref(), *t)).collect();
                        let land_info = land.as_ref().and_then(|l| cell.grid.map(|(gx, gy)| (l, gx, gy)));
                        (placed_refs, land_info)
                    }).collect();

                let cell_refs: Vec<(&[(&NifFile, NiTransform)], Option<(&LandRecord, i32, i32)>)> =
                    cell_inputs.iter().map(|(refs, land_info)| (refs.as_slice(), *land_info)).collect();

                let landscape_texture_map = esm_reader.read_landscape_texture_map().ok();
                if let Some(ref tex_map) = landscape_texture_map {
                    println!("地形テクスチャセット解決: {} 件", tex_map.len());
                }

                let scene = RenderScene::from_cells(
                    &device,
                    &queue,
                    &context,
                    &cell_refs,
                    landscape_texture_map.as_ref(),
                    &mut vfs,
                );

                let clear_color = if let Some(ref cl) = primary_lighting {
                    if cl.fog_far > 0.0 {
                        wgpu::Color {
                            r: (cl.fog_color[0] as f64) / 255.0 * 0.25,
                            g: (cl.fog_color[1] as f64) / 255.0 * 0.25,
                            b: (cl.fog_color[2] as f64) / 255.0 * 0.25,
                            a: 1.0,
                        }
                    } else {
                        wgpu::Color {
                            r: (cl.ambient[0] as f64) / 255.0 * 0.4,
                            g: (cl.ambient[1] as f64) / 255.0 * 0.4,
                            b: (cl.ambient[2] as f64) / 255.0 * 0.4,
                            a: 1.0,
                        }
                    }
                } else {
                    wgpu::Color {
                        r: 0.12,
                        g: 0.14,
                        b: 0.18,
                        a: 1.0,
                    }
                };

                (scene, primary_lighting, placed_lights, clear_color)
            }
        };

        println!("GPU シーン構築完了: {} メッシュノード描画準備完了", scene.meshes.len());

        let title = match target {
            ViewerTarget::Mesh(path) => format!("OpenFallout3 - Mesh: {}", path),
            ViewerTarget::Cell(edid) => format!("OpenFallout3 - Cell: {}", edid),
            ViewerTarget::World(edid, _) => format!("OpenFallout3 - World: {}", edid),
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

        // カメラ距離がフォグ Far 距離を超えている場合（全体俯瞰時）、メッシュがフォグに完全に埋没するのを防ぐため初期状態でフォグを OFF に設定
        let enable_fog = if let Some(ref cl) = cell_lighting {
            if cl.fog_far > 0.0 && camera.distance > cl.fog_far {
                println!(
                    "注記: カメラ距離 ({:.1}) がセルフォグ Far ({:.1}) を超えているため、初期状態でフォグを無効化しています (F キーでフォグ表示切替)。",
                    camera.distance, cl.fog_far
                );
                false
            } else {
                true
            }
        } else {
            false
        };

        let camera_uniform = camera.build_uniform();
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let mut initial_cell_lighting = cell_lighting.clone();
        if !enable_fog {
            if let Some(ref mut cl) = initial_cell_lighting {
                cl.fog_far = 0.0;
            }
        }
        let initial_lighting = LightingUniform::from_cell_lighting(
            initial_cell_lighting.as_ref(),
            &placed_lights,
            camera.eye_position(),
        );
        let lighting_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Lighting Buffer"),
            contents: bytemuck::cast_slice(&[initial_lighting]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera & Lighting Bind Group"),
            layout: &context.camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: lighting_buffer.as_entire_binding(),
                },
            ],
        });

        println!("\n=== 操作ガイド ===");
        println!("  左ドラッグ:       カメラ回転 (Orbit)");
        println!("  右ドラッグ:       カメラ平行移動 (Pan)");
        println!("  ホイール:         ズームイン / アウト");
        println!("  R キー:           カメラ自動再フォーカス (Reset)");
        println!("  C キー:           Havok コリジョンワイヤーフレーム重畳表示切替 (Collision ON/OFF)");
        println!("  F キー:           セル環境フォグ表示切替 (Fog ON/OFF)");
        println!("  L キー:           ビューア補助ヘッドライト切替 (Light ON/OFF)");
        println!("  Esc キー:         終了\n");

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
            lighting_buffer,
            cell_lighting,
            placed_lights,
            clear_color,
            depth_view,
            scene,
            show_collision: false,
            enable_fog,
            headlight: false,
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

        let mut current_lighting = self.cell_lighting.clone();
        if !self.enable_fog {
            if let Some(ref mut cl) = current_lighting {
                cl.fog_far = 0.0;
            }
        }

        let mut lights = self.placed_lights.clone();
        if self.headlight {
            // カメラ位置に配置するビューア補助ヘッドライト
            lights.push(PlacedPointLight {
                position: self.camera.eye_position(),
                radius: self.camera.distance * 2.0 + 2000.0,
                color: [1.0, 0.98, 0.95],
                falloff: 1.0,
            });
        }

        let light_uniform = LightingUniform::from_cell_lighting(
            current_lighting.as_ref(),
            &lights,
            self.camera.eye_position(),
        );
        self.queue.write_buffer(
            &self.lighting_buffer,
            0,
            bytemuck::cast_slice(&[light_uniform]),
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
                        load: wgpu::LoadOp::Clear(self.clear_color),
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

            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            self.scene.render(&mut render_pass, &self.context);

            if self.show_collision {
                self.scene.render_collision(&mut render_pass, &self.context);
            }
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
                            KeyCode::KeyC => {
                                state.show_collision = !state.show_collision;
                                println!(
                                    "Havok コリジョンワイヤーフレーム表示: {}",
                                    if state.show_collision { "ON" } else { "OFF" }
                                );
                                state.window.request_redraw();
                            }
                            KeyCode::KeyF => {
                                state.enable_fog = !state.enable_fog;
                                println!(
                                    "セル環境フォグ表示: {}",
                                    if state.enable_fog { "ON" } else { "OFF" }
                                );
                                state.window.request_redraw();
                            }
                            KeyCode::KeyL => {
                                state.headlight = !state.headlight;
                                println!(
                                    "ビューア補助ヘッドライト: {}",
                                    if state.headlight { "ON" } else { "OFF" }
                                );
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
        println!("  - ワールド表示:     cargo run -p fo3_viewer -- world <DataDir> <WorldEDID> [GridX] [GridY]");
        println!("例:");
        println!("  cargo run -p fo3_viewer -- \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"meshes\\weapons\\1handpistol\\10mmpistol.nif\"");
        println!("  cargo run -p fo3_viewer -- cell \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"Vault101a\"");
        println!("  cargo run -p fo3_viewer -- cell \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"Springvale\"");
        println!("  cargo run -p fo3_viewer -- world \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"MegatonWorld\"");
        println!("  cargo run -p fo3_viewer -- world \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"DCWorld01\"");
        println!("  cargo run -p fo3_viewer -- world \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"Wasteland\" -1 2");
        return Ok(());
    }

    let (data_dir, target) = if args[1] == "cell" {
        if args.len() < 4 {
            eprintln!("エラー: セル表示モードには <DataDir> と <CellEDID> が必要です。");
            return Ok(());
        }
        (args[2].clone(), ViewerTarget::Cell(args[3].clone()))
    } else if args[1] == "world" {
        if args.len() < 4 {
            eprintln!("エラー: ワールド表示モードには <DataDir> と <WorldEDID> [GridX] [GridY] が必要です。");
            return Ok(());
        }
        let grid = if args.len() >= 6 {
            let gx = args[4].parse::<i32>().unwrap_or(0);
            let gy = args[5].parse::<i32>().unwrap_or(0);
            Some((gx, gy))
        } else {
            None
        };
        (args[2].clone(), ViewerTarget::World(args[3].clone(), grid))
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
