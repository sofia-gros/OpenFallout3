//! ビューアーの初期化処理 (`ViewerState::new`)。
//!
//! 参照元: Gamebryo 2.6 レンダリングパイプライン & winit イベントループ

use fo3_render::{LightingUniform, RenderContext};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::anim::AnimState;
use crate::controller::Controller;
use crate::hud::HudRenderer;
use crate::inventory::PlayerInventory;
use crate::loader::load_scene;
use crate::types::{print_controls_guide, update_window_title, ViewerTarget};
use crate::ui::ViewerMode;
use super::ViewerState;

impl ViewerState {
    pub async fn new(window: Arc<Window>, data_dir: &str, target: &ViewerTarget) -> Self {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .expect("Failed to create surface");

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

        // VSync (PresentMode::Fifo) を優先選択し、フレームレート暴走とGPU/CPU過負荷によるPCクラッシュを防止
        let present_mode = surface_caps
            .present_modes
            .iter()
            .copied()
            .find(|&m| m == wgpu::PresentMode::Fifo)
            .unwrap_or(surface_caps.present_modes[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let context = RenderContext::new(&device, surface_format);
        let depth_view = RenderContext::create_depth_texture(&device, width, height);

        // 永続 VFS の初期化 (全 BSA を一度だけオープン・インデックス常駐)
        let mut vfs = crate::init::initialize_vfs(data_dir);

        // マスター ESM 静的定義の初期化 (3Dモデル、アクター、防具、光源マップの一括常駐)
        let master_context = Arc::new(crate::init::initialize_master_context(data_dir));

        // NIF AST および GPU テクスチャの永続キャッシュ
        let mut nif_cache = fo3_render::NifCache::new();
        let mut texture_cache = HashMap::new();

        // シーン・物理・ライティングのロード
        let mut loaded = load_scene(
            &device,
            &queue,
            &context,
            data_dir,
            target,
            &mut vfs,
            &master_context,
            &mut nif_cache,
            &mut texture_cache,
        );

        // アニメーション再生状態の初期化
        let anim = AnimState::new(target, &mut vfs);

        update_window_title(&window, target);

        // カメラ・物理コントローラーの初期化
        let aspect = width as f32 / height as f32;

        let (spawn_pos, initial_yaw) = if matches!(target, ViewerTarget::NewGame { .. }) {
            // 実機 CG00PlayerStartMarker (0x00039562): Pos=[-5275.8867, -7148.175, 7542.536], Rot=[0.0, 0.0, PI]
            let marker_pos = glam::Vec3::new(-5275.8867, -7148.175, 7542.536);
            println!(
                "★ ニューゲーム開始: CG00PlayerStartMarker へ配置 {:?}",
                marker_pos
            );
            (marker_pos, std::f32::consts::PI)
        } else if let Some((door_pos, _)) = loaded.door_spawn_point {
            let to_center = loaded.scene.bounds_center - door_pos;
            let into_room = if to_center.x.hypot(to_center.y) > 1.0 {
                glam::Vec3::new(to_center.x, to_center.y, 0.0).normalize()
            } else {
                glam::Vec3::X
            };
            let pos = door_pos + into_room * 80.0 + glam::Vec3::new(0.0, 0.0, 65.0);
            println!(
                "出入口ドアから室内方向への初期スポーン地点を設定: {:?}",
                pos
            );
            (pos, into_room.y.atan2(into_room.x))
        } else {
            let ray_origin = loaded.scene.bounds_center
                + glam::Vec3::new(0.0, 0.0, loaded.scene.bounds_radius * 0.5);
            let ray_dir = glam::Vec3::new(0.0, 0.0, -1.0);
            let pos = if let Some(hit) =
                loaded
                    .physics_world
                    .cast_ray(ray_origin, ray_dir, loaded.scene.bounds_radius * 2.0)
            {
                println!(
                    "レイキャストによる安全な床面検出に成功: Z = {:.1}",
                    hit.point.z
                );
                hit.point + glam::Vec3::new(0.0, 0.0, 65.0)
            } else {
                loaded.scene.bounds_center + glam::Vec3::new(0.0, 0.0, 64.0)
            };
            (pos, 0.0)
        };

        let mut controller = Controller::new(
            aspect,
            loaded.scene.bounds_center,
            loaded.scene.bounds_radius,
            spawn_pos,
            loaded.physics_world,
        );

        controller.camera.yaw = initial_yaw;
        controller.camera.pitch = 0.0;
        controller.player_camera.yaw = initial_yaw;
        controller.player_camera.pitch = 0.0;

        // プレイヤーアクター (三人称全身モデル + 一人称腕モデル) の生成・配置
        let player_actor = crate::player::build_player_actor(
            &device,
            &queue,
            &context,
            &mut vfs,
            &mut nif_cache,
            &mut texture_cache,
            &mut loaded.scene,
            spawn_pos,
            controller.player_camera.yaw,
        );
        controller.player_actor = player_actor;

        let enable_fog = if let Some(ref cl) = loaded.cell_lighting {
            !(cl.fog_far > 0.0 && controller.camera.distance > cl.fog_far)
        } else {
            false
        };

        let camera_uniform = controller.camera.build_uniform();
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let mut initial_cell_lighting = loaded.cell_lighting.clone();
        if !enable_fog {
            if let Some(ref mut cl) = initial_cell_lighting {
                cl.fog_far = 0.0;
            }
        }
        let initial_lighting = LightingUniform::from_cell_lighting_with_focus(
            initial_cell_lighting.as_ref(),
            &loaded.placed_lights,
            controller.camera.eye_position(),
            Some(controller.camera.target),
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

        let hud = HudRenderer::new(&device, &queue, surface_format, &mut vfs);
        let ui_renderer = fo3_render::UiRenderer::new(&device, &queue, surface_format);
        let mut vm = fo3_script::ScriptVm::new();
        vm.initialize_from_master(&master_context);
        let mut dispatcher = fo3_script::EventDispatcher::new();
        dispatcher.register_all_scripts(&master_context.script_map);

        print_controls_guide();

        let ui_rtt = fo3_render::GpuTexture::create_render_target(
            &device,
            1024,
            1024,
            surface_format,
            Some("UI RTT"),
        );

        let mut state = Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            context,
            camera_buffer,
            mode: ViewerMode::Exploring,
            camera_bind_group,
            lighting_buffer,
            cell_lighting: loaded.cell_lighting,
            placed_lights: loaded.placed_lights,
            clear_color: loaded.clear_color,
            depth_view,
            scene: loaded.scene,
            streamer: None,
            show_collision: false,
            enable_fog,
            headlight: false,
            controller,
            anim,
            hud,
            interactables: loaded.interactables,
            focused_interactable: None,
            markers: loaded.markers,
            refr_bindings: loaded.refr_bindings,
            animators: HashMap::new(),
            inventory: PlayerInventory::new(),
            data_dir: data_dir.to_string(),
            start_time: Instant::now(),
            vfs,
            ui_rtt,
            active_rtt_bindings: Vec::new(),
            master_context,
            nif_cache,
            texture_cache,
            vm,
            ai: crate::ai::AiManager::new(),
            nav_graph: fo3_navigation::NavGraph::new(),
            dispatcher,
            ui_renderer,
            input_manager: crate::input::InputManager::new(),
            screen_fade_color: [0.0, 0.0, 0.0, 1.0],
            screen_fade_alpha: if matches!(target, ViewerTarget::NewGame { .. }) {
                1.0
            } else {
                0.0
            },
            sound_engine: crate::audio::SoundEngine::new(),
            chargen_menu: crate::chargen_menu::ChargenMenu::new(),
            bink_player: None,
            bink_video_bind_group: None,
            frame_count: 0,
        };

        if let ViewerTarget::World(world_edid, _) = target {
            state.streamer = Some(crate::streamer::WorldStreamer::new(world_edid.clone(), 1));
        }

        // NavGraphの構築
        for record in state.master_context.navm_map.values() {
            state.nav_graph.add_navmesh(record);
        }

        // セル EDID の解決: ターゲット種別に応じてスクリプト登録対象のセルを決定
        let cell_edid = match target {
            ViewerTarget::NewGame { .. } => "Vault101Infirmary",
            ViewerTarget::Cell(edid) => edid.as_str(),
            _ => "",
        };
        state.setup_scripts_for_cell(cell_edid);

        let interactables = state.interactables.clone();
        for obj in interactables {
            if let crate::interact::InteractableKind::Actor {
                form_id,
                base_form_id,
                ..
            } = obj.kind
            {
                state.ai.register_actor(
                    fo3_esm::types::FormId(form_id),
                    fo3_esm::types::FormId(base_form_id),
                    &state.master_context,
                );
            }
        }

        if let ViewerTarget::NewGame {
            intro_movie,
            start_quest,
            start_stage,
        } = &target
        {
            println!("============================================================");
            println!(
                "⚙ ニューゲーム初期化: Quest (FormID: 0x{:08X}), Stage: {}",
                start_quest, start_stage
            );
            println!("============================================================");

            if let Some(intro) = intro_movie {
                let intro_bik = Path::new(data_dir).join(intro);
                if intro_bik.exists() {
                    state
                        .vm
                        .play_bink_queue
                        .push(intro_bik.to_string_lossy().to_string());
                }
            }

            let quest_id = fo3_esm::types::FormId(*start_quest);
            state.vm.set_stage(quest_id, *start_stage);
        }

        state
    }
}
