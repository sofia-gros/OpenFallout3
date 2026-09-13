//! ビューアーのメインアプリケーションループおよび GPU レンダリング状態。
//!
//! 参照元: Gamebryo 2.6 レンダリングパイプライン & winit イベントループ

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use fo3_esm::{CellLighting, EsmMasterContext, FormId};
use fo3_render::{
    GpuTexture, LightingUniform, NifCache, PlacedPointLight, RenderContext, RenderScene,
};
use fo3_vfs::VfsManager;
use pollster::FutureExt;
use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use std::time::Instant;
use crate::anim::AnimState;
use crate::controller::Controller;
use crate::hud::HudRenderer;
use crate::interact::{
    find_focused_by_raycast, find_focused_interactable, InteractableObject,
    F_ACTIVATE_PICK_LENGTH,
};
use crate::interactive_anim::{InteractiveAnimator, RefrBinding};
use crate::inventory::PlayerInventory;
use crate::loader::load_scene;
use crate::types::{print_controls_guide, update_window_title, CameraMode, ViewerTarget};
use crate::ui::ViewerMode;

pub struct ViewerState {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: PhysicalSize<u32>,
    pub context: RenderContext,
    pub camera_buffer: wgpu::Buffer,
    pub mode: ViewerMode,
    pub camera_bind_group: wgpu::BindGroup,
    pub lighting_buffer: wgpu::Buffer,
    pub cell_lighting: Option<CellLighting>,
    pub placed_lights: Vec<PlacedPointLight>,
    pub clear_color: wgpu::Color,
    pub depth_view: wgpu::TextureView,
    pub scene: RenderScene,
    pub show_collision: bool,
    pub enable_fog: bool,
    pub headlight: bool,
    pub controller: Controller,
    pub anim: AnimState,
    pub hud: HudRenderer,
    pub interactables: Vec<InteractableObject>,
    pub focused_interactable: Option<InteractableObject>,
    pub refr_bindings: HashMap<u32, RefrBinding>,
    pub animators: HashMap<u32, InteractiveAnimator>,
    pub inventory: PlayerInventory,
    pub data_dir: String,
    pub start_time: Instant,
    pub vfs: VfsManager,
    pub master_context: Arc<EsmMasterContext>,
    pub nif_cache: NifCache,
    pub texture_cache: HashMap<String, GpuTexture>,
    pub vm: fo3_script::ScriptVm,
    pub dispatcher: fo3_script::EventDispatcher,
    pub ui_renderer: fo3_render::UiRenderer,
    /// キーバインド・入力管理マネージャー (Fallout 3 実機標準 + F1-F12 デバッグ)
    pub input_manager: crate::input::InputManager,
    /// 全画面フェードエフェクト色 (暗転・ホワイトアウト用)
    pub screen_fade_color: [f32; 4],
    /// 全画面フェードエフェクトの不透明度 (0.0=透明, 1.0=完全不透明)
    pub screen_fade_alpha: f32,
    /// オーディオ再生及びダイアログ・字幕進行管理
    pub sound_engine: crate::audio::SoundEngine,
}

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

        // 永続 VFS の初期化 (全 BSA を一度だけオープン・インデックス常駐)
        let mut vfs = crate::init::initialize_vfs(data_dir);

        // マスター ESM 静的定義の初期化 (3Dモデル、アクター、防具、光源マップの一括常駐)
        let master_context = Arc::new(crate::init::initialize_master_context(data_dir));

        // NIF AST および GPU テクスチャの永続キャッシュ
        let mut nif_cache = NifCache::new();
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

        let (spawn_pos, initial_yaw) = if matches!(target, ViewerTarget::NewGame) {
            // 実機 CG00PlayerStartMarker (0x00039562): Pos=[-5275.8867, -7148.175, 7542.536], Rot=[0.0, 0.0, PI]
            let marker_pos = glam::Vec3::new(-5275.8867, -7148.175, 7542.536);
            println!("★ ニューゲーム開始: CG00PlayerStartMarker へ配置 {:?}", marker_pos);
            (marker_pos, std::f32::consts::PI)
        } else if let Some((door_pos, _)) = loaded.door_spawn_point {
            let to_center = loaded.scene.bounds_center - door_pos;
            let into_room = if to_center.x.hypot(to_center.y) > 1.0 {
                glam::Vec3::new(to_center.x, to_center.y, 0.0).normalize()
            } else {
                glam::Vec3::X
            };
            let pos = door_pos + into_room * 80.0 + glam::Vec3::new(0.0, 0.0, 65.0);
            println!("出入口ドアから室内方向への初期スポーン地点を設定: {:?}", pos);
            (pos, into_room.y.atan2(into_room.x))
        } else {
            let ray_origin =
                loaded.scene.bounds_center + glam::Vec3::new(0.0, 0.0, loaded.scene.bounds_radius * 0.5);
            let ray_dir = glam::Vec3::new(0.0, 0.0, -1.0);
            let pos = if let Some(hit) =
                loaded.physics_world.cast_ray(ray_origin, ray_dir, loaded.scene.bounds_radius * 2.0)
            {
                println!("レイキャストによる安全な床面検出に成功: Z = {:.1}", hit.point.z);
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
            show_collision: false,
            enable_fog,
            headlight: false,
            controller,
            anim,
            hud,
            interactables: loaded.interactables,
            focused_interactable: None,
            refr_bindings: loaded.refr_bindings,
            animators: HashMap::new(),
            inventory: PlayerInventory::new(),
            data_dir: data_dir.to_string(),
            start_time: Instant::now(),
            vfs,
            master_context,
            nif_cache,
            texture_cache,
            vm,
            dispatcher,
            ui_renderer,
            input_manager: crate::input::InputManager::new(),
            screen_fade_color: [0.0, 0.0, 0.0, 1.0],
            screen_fade_alpha: if matches!(target, ViewerTarget::NewGame) { 1.0 } else { 0.0 },
            sound_engine: crate::audio::SoundEngine::new(),
        };

        state.setup_scripts_for_cell();

        if matches!(target, ViewerTarget::NewGame) {
            println!("============================================================");
            println!("★ 実機ニューゲームシーケンス開始: CG00 (FormID: 0x0001F388)");
            println!("============================================================");

            // 1. 実機オープニングムービー (Fallout INTRO Vsk.bik) のフルスクリーン再生
            let intro_bik = Path::new(data_dir).join("Video").join("Fallout INTRO Vsk.bik");
            if intro_bik.exists() {
                crate::action::play_bink_video(&intro_bik.to_string_lossy());
            }

            // 2. CG00 クエスト Stage 0 開始
            let cg00_id = fo3_esm::types::FormId(0x0001F388);
            state.vm.set_stage(cg00_id, 0);
        }

        state
    }

    /// セル内の配置オブジェクトにアタッチされたスクリプトを抽出し、イベントディスパッチャーへ登録する。
    pub fn setup_scripts_for_cell(&mut self) {
        // 0. クエスト EditorID -> FormID マップを VM へ登録
        for (edid, form_id) in &self.master_context.quest_edid_map {
            self.vm.edid_map.insert(edid.clone(), *form_id);
        }

        // 0.1 セル内の配置参照 (REFR / ACHR) の EditorID およびスクリプトを登録
        let esm_path = Path::new(&self.data_dir).join("Fallout3.esm");
        if let Ok(mut reader) = fo3_esm::EsmReader::open(&esm_path) {
            if let Ok(Some(cells)) = reader.find_cell_and_neighbors("Vault101d", 0) {
                for (_, refrs, _) in cells {
                    for refr in refrs {
                        if !refr.edid.is_empty() {
                            self.vm.edid_map.insert(refr.edid.to_ascii_uppercase(), refr.form_id);
                        }
                        if let Some(scpt_id) = refr.script {
                            self.dispatcher.attach_script(refr.form_id, scpt_id);
                        }
                    }
                }
            }
        }

        // 1. master_context に存在する全スクリプトを dispatcher に登録
        for scpt in self.master_context.script_map.values() {
            self.dispatcher.register_script(scpt.clone());
        }

        // 2. セル内の配置オブジェクト (interactables) のスクリプトをアタッチ
        for obj in &self.interactables {
            let obj_id = FormId(obj.form_id);
            if let Some(base_info) = self.master_context.model_map.get(&obj_id) {
                if let Some(script_id) = base_info.script {
                    self.dispatcher.attach_script(obj_id, script_id);
                }
            }
        }
    }

    /// フォーカス中オブジェクトに対するインタラクトアクション (`E` キー) を実行。
    /// ドア (`XTEL`) の場合は遷移先セルを解決してロードしテレポートを行う。
    /// 参照元: Gamebryo 2.6 セル遷移 & `references/openmw/components/esm4/loadrefr.cpp:103-127`
    pub fn interact_or_teleport(&mut self) {
        crate::action::perform_interact(self);
    }


    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_view =
                RenderContext::create_depth_texture(&self.device, new_size.width, new_size.height);
            self.controller.camera.aspect = new_size.width as f32 / new_size.height as f32;
        }
    }

    pub fn update(&mut self) {
        self.input_manager.update_frame();
        let dt = self.controller.update();

        // 0. スクリプトからの動画再生要求の消化
        while !self.vm.play_bink_queue.is_empty() {
            let bink_file = self.vm.play_bink_queue.remove(0);
            crate::action::play_bink_video(&bink_file);
        }

        // 0.1 スクリプトからのテレポート移動要求 (MoveTo) の消化
        crate::action::process_teleport_requests(self);

        // 0.2 オーディオ・会話シーケンスの進行更新 (実機 DIAL/INFO/SOUN 連動)
        self.sound_engine.update(dt, &mut self.vm, &self.master_context, &mut self.vfs);

        // 開閉アニメーションの進行および物理剛体・GPUメッシュの追従更新
        // 参照元: Gamebryo 2.6 `bhkRigidBody` (MO_SYS_KEYFRAMED) 追従
        for (_form_id, animator) in self.animators.iter_mut() {
            if animator.is_animating() {
                animator.update(dt);
                let part_transforms = animator.compute_part_transforms();
                for (mesh_indices, rigid_bodies, world_mat, pos, rot) in part_transforms {
                    // 1. 可動パーツの物理剛体の位置・回転を同期
                    for rb in rigid_bodies {
                        self.controller.physics_world.set_rigid_body_transform(*rb, pos, rot);
                    }
                    // 2. 可動パーツの GPU 描画メッシュのワールド行列を同期
                    let model_mat = world_mat.to_cols_array_2d();
                    for &mesh_idx in mesh_indices {
                        if let Some(mesh) = self.scene.meshes.get(mesh_idx) {
                            self.queue.write_buffer(
                                &mesh.model_uniform_buffer,
                                0,
                                bytemuck::cast_slice(&[model_mat]),
                            );
                        }
                    }
                }
            }
        }

        let uniform = self.controller.camera.build_uniform();
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[uniform]));

        let mut current_lighting = self.cell_lighting.clone();
        if !self.enable_fog {
            if let Some(ref mut cl) = current_lighting {
                cl.fog_far = 0.0;
            }
        }

        let mut lights = self.placed_lights.clone();
        if self.headlight {
            lights.push(PlacedPointLight {
                position: self.controller.camera.eye_position(),
                radius: self.controller.camera.distance * 2.0 + 2000.0,
                color: [1.0, 0.98, 0.95],
                falloff: 1.0,
            });
        }

        let light_uniform = LightingUniform::from_cell_lighting_with_focus(
            current_lighting.as_ref(),
            &lights,
            self.controller.camera.eye_position(),
            Some(self.controller.camera.target),
        );
        self.queue.write_buffer(
            &self.lighting_buffer,
            0,
            bytemuck::cast_slice(&[light_uniform]),
        );

        // セル配置アクターの更新
        self.scene.update_actors(dt, &self.device, &self.queue);

        // プレイヤーアクター (三人称・一人称) の姿勢・位置・ロコモーション・GPUスキニング更新
        if let Some(ref mut player) = self.controller.player_actor {
            let feet_pos = self.controller.character_controller.feet_position();
            let cam_yaw = self.controller.player_camera.yaw;
            let is_moving = self.controller.key_forward || self.controller.key_backward || self.controller.key_left || self.controller.key_right;
            let fwd = glam::Vec3::new(cam_yaw.cos(), cam_yaw.sin(), 0.0).normalize();
            let rgt = glam::Vec3::new(cam_yaw.sin(), -cam_yaw.cos(), 0.0).normalize();
            let mut m_dir = glam::Vec3::ZERO;
            if self.controller.key_forward { m_dir += fwd; }
            if self.controller.key_backward { m_dir -= fwd; }
            if self.controller.key_right { m_dir += rgt; }
            if self.controller.key_left { m_dir -= rgt; }
            let move_opt = if is_moving { Some(m_dir.normalize()) } else { None };

            player.update(
                dt,
                feet_pos,
                cam_yaw,
                is_moving,
                move_opt,
                &self.device,
                &self.queue,
                &mut self.scene.actors,
                &mut self.scene.meshes,
            );
        }

        // 単体 Anim / Actor モード時のスキニング・アニメーション更新
        self.anim.update(dt, &self.device, &self.queue, &mut self.scene);

        // 全 NPC アクターのアニメーション・ボーン姿勢・メッシュ更新
        for actor in &mut self.scene.actors {
            actor.update(dt, &self.device, &self.queue, &mut self.scene.meshes);
        }

        // CG00 出産シーケンス (Chargen 拘束中) の赤ちゃん仰向け視点
        let cg00_id = fo3_esm::types::FormId(0x0001F388);
        let cg00_stage = self.vm.get_stage(cg00_id);
        if cg00_stage > 0 && cg00_stage < 15 && !self.vm.player_controls_enabled {
            let baby_eye = glam::Vec3::new(-5275.8867, -7148.175, 7542.536 + 22.0);
            self.controller.player_camera.current_eye = baby_eye;
            self.controller.player_camera.yaw = -2.15;
            self.controller.player_camera.pitch = 0.52;
            self.controller.camera.override_eye = Some(baby_eye);
            self.controller.camera.yaw = -2.15;
            self.controller.camera.pitch = 0.52;
        }

        // ゲーム内スクリプトイベントの毎フレームディスパッチ (GameMode ループ)
        self.vm.delta_time = dt;
        self.dispatcher.push_event(fo3_script::GameEvent::GameMode);
        let _ = self.dispatcher.process_queue(&mut self.vm);

        // 画面エフェクト (暗転・ホワイトアウト・徐々に視界が開ける演出)
        // 参照元: GECK `imod CG00BlackScreenISFX`, `imod CG00BirthISFX`
        if self.vm.active_imods.iter().any(|m| m.eq_ignore_ascii_case("CG00BirthISFX") || m.eq_ignore_ascii_case("CG00BirthBaseISFX")) {
            self.screen_fade_color = [1.0, 1.0, 1.0, 1.0];
            if self.screen_fade_alpha > 0.0 {
                self.screen_fade_alpha = (self.screen_fade_alpha - dt * 0.2).max(0.0);
            }
        } else if self.vm.active_imods.iter().any(|m| m.eq_ignore_ascii_case("CG00BlackScreenISFX")) {
            self.screen_fade_color = [0.0, 0.0, 0.0, 1.0];
            self.screen_fade_alpha = 1.0;
        } else if self.screen_fade_alpha > 0.0 {
            self.screen_fade_alpha = (self.screen_fade_alpha - dt * 1.0).max(0.0);
        }

        // クエスト通知の出力
        for notif in self.vm.quest_manager.notifications.drain(..) {
            println!("[HUD通知] {}", notif);
        }

        // プレイヤー正面の視線レイキャスト & オブジェクト検知
        // 参照元: Gamebryo 2.6 NiPick, FO3 実機インタラクト判定 (GMST fActivatePickLength = 180.0)
        let eye = self.controller.camera.eye_position();
        let forward = self.controller.camera.forward_vector();
        let prev_focus = self.focused_interactable.as_ref().map(|o| o.form_id);

        // 1. 画面中央（クロスヘア）からの物理レイキャストによる精密判定
        let ray_hit = self.controller.physics_world.cast_ray_interaction(eye, forward, F_ACTIVATE_PICK_LENGTH);
        let mut new_focus = find_focused_by_raycast(ray_hit, &self.interactables).cloned();

        // 2. コライダーを持たない一部アイテム/NPCに対する幾何フォールバック (手前に遮蔽壁がない場合のみ)
        if new_focus.is_none() && ray_hit.is_none() {
            new_focus = find_focused_interactable(eye, forward, &self.interactables, F_ACTIVATE_PICK_LENGTH).cloned();
        }

        if let Some(ref focused) = new_focus {
            if prev_focus != Some(focused.form_id) {
                println!("[インタラクト検知] {}", focused.prompt_text());
            }
        }
        self.focused_interactable = new_focus;
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
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
            self.scene.render_with_camera_pos(
                &mut render_pass,
                &self.context,
                Some(self.controller.camera.eye_position()),
            );

            if self.show_collision {
                self.scene.render_collision(&mut render_pass, &self.context);
            }

            // Fallout 3 実機 HUD / UI オーバーレイ描画
            let elapsed = self.start_time.elapsed().as_secs_f32();
            if matches!(self.mode, ViewerMode::Exploring) && self.screen_fade_alpha < 0.99 {
                self.hud.render_crosshair(
                    &mut render_pass,
                    &self.queue,
                    self.size.width as f32,
                    self.size.height as f32,
                    elapsed,
                );
            }

            // 全画面フェードエフェクト (暗転・ホワイトアウト・視界開放)
            if self.screen_fade_alpha > 0.001 {
                self.hud.render_rect(
                    &mut render_pass,
                    &self.queue,
                    0.0,
                    0.0,
                    self.size.width as f32,
                    self.size.height as f32,
                    [
                        self.screen_fade_color[0],
                        self.screen_fade_color[1],
                        self.screen_fade_color[2],
                        self.screen_fade_alpha,
                    ],
                    self.size.width as f32,
                    self.size.height as f32,
                );
            }
        }

        // Phase 9: 最前面 2D UI (会話テキスト・選択肢・ターミナル画面) の描画パス
        let mut ui_batch = fo3_render::TextBatch::default();
        self.mode.populate_batch(
            &mut ui_batch,
            self.ui_renderer.font(),
            self.size.width as f32,
            self.size.height as f32,
        );

        // 字幕 (Subtitle) の描画
        if let Some(ref sub) = self.sound_engine.active_subtitle {
            let subtitle_text = format!("{}: {}", sub.speaker, sub.text);
            let screen_w = self.size.width as f32;
            let screen_h = self.size.height as f32;
            ui_batch.add_text(
                self.ui_renderer.font(),
                &subtitle_text,
                screen_w * 0.1,
                screen_h * 0.85,
                1.2,
                [0.2, 1.0, 0.4, 1.0],
            );
        }

        // 実機メッセージメニュー (MESG / ShowMessage) の描画
        if let Some(msg_id) = self.vm.show_messages.first() {
            let mesg_opt = self.master_context.mesg_edid_map.get(&msg_id.to_ascii_uppercase())
                .and_then(|fid| self.master_context.mesg_map.get(fid))
                .or_else(|| {
                    let hex_str = msg_id.trim_start_matches("0x").trim_start_matches("0X");
                    if let Ok(val) = u32::from_str_radix(hex_str, 16) {
                        self.master_context.mesg_map.get(&fo3_esm::FormId(val))
                    } else {
                        None
                    }
                });

            if let Some(mesg) = mesg_opt {
                let screen_w = self.size.width as f32;
                let screen_h = self.size.height as f32;
                let cx = screen_w * 0.35;
                let mut cy = screen_h * 0.42;

                if !mesg.text.is_empty() {
                    ui_batch.add_text(self.ui_renderer.font(), &mesg.text, cx, cy, 1.25, [1.0, 0.9, 0.2, 1.0]);
                    cy += 35.0;
                }

                for (idx, btn_text) in mesg.buttons.iter().enumerate() {
                    let label = format!("[{}] {}", idx + 1, btn_text);
                    ui_batch.add_text(self.ui_renderer.font(), &label, cx, cy, 1.1, [0.2, 1.0, 0.4, 1.0]);
                    cy += 28.0;
                }
            }
        }
        if !ui_batch.indices.is_empty() {
            self.ui_renderer.update_resolution(
                &self.queue,
                self.size.width as f32,
                self.size.height as f32,
            );
            self.ui_renderer.upload_batch(&self.device, &ui_batch);

            let mut ui_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("2D UI Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.ui_renderer.render(&mut ui_pass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

pub struct App {
    pub data_dir: String,
    pub target: ViewerTarget,
    pub state: Option<ViewerState>,
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
            WindowEvent::MouseInput {
                button,
                state: btn_state,
                ..
            } => {
                let pressed = btn_state == ElementState::Pressed;
                if pressed && button == MouseButton::Left && state.mode.is_ui_active() {
                    let _ = state.mode.handle_key_with_vm(winit::keyboard::KeyCode::Space, &mut state.vm);
                }
                match button {
                    MouseButton::Left => state.controller.left_mouse_down = pressed,
                    MouseButton::Right => state.controller.right_mouse_down = pressed,
                    _ => {}
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if state.mode.is_ui_active() {
                    state.controller.last_mouse_pos = Some((position.x, position.y));
                    return;
                }
                if let Some((last_x, last_y)) = state.controller.last_mouse_pos {
                    let dx = (position.x - last_x) as f32;
                    let dy = (position.y - last_y) as f32;

                    match state.controller.camera_mode {
                        CameraMode::FreeOrbit => {
                            if state.controller.left_mouse_down {
                                state.controller.camera.rotate(dx, dy);
                                state.window.request_redraw();
                            } else if state.controller.right_mouse_down {
                                state.controller.camera.pan(dx, dy);
                                state.window.request_redraw();
                            }
                        }
                        CameraMode::Standard => {
                            if state.vm.player_controls.looking && (state.controller.left_mouse_down || state.controller.right_mouse_down) {
                                state.controller.player_camera.rotate(dx * 0.003, dy * 0.003);
                                state.window.request_redraw();
                            }
                        }
                    }
                }
                state.controller.last_mouse_pos = Some((position.x, position.y));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let zoom_amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.05,
                };
                if state.controller.camera_mode == CameraMode::Standard {
                    if state.vm.player_controls.pov {
                        state.controller.player_camera.zoom(zoom_amount);
                        if let Some(ref mut player) = state.controller.player_actor {
                            player.set_view_mode(state.controller.player_camera.mode, &mut state.scene.meshes);
                        }
                    }
                } else {
                    state.controller.camera.zoom(zoom_amount);
                }
                state.window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                if let PhysicalKey::Code(key) = event.physical_key {
                    // 実機メッセージダイアログ (MESG / ShowMessage) のボタン選択
                    if pressed {
                        if let Some(msg_id) = state.vm.show_messages.first().cloned() {
                            let mesg_opt = state.master_context.mesg_edid_map.get(&msg_id.to_ascii_uppercase())
                                .and_then(|fid| state.master_context.mesg_map.get(fid));
                            if let Some(mesg) = mesg_opt {
                                let btn_idx = match key {
                                    KeyCode::Digit1 | KeyCode::Numpad1 => Some(0),
                                    KeyCode::Digit2 | KeyCode::Numpad2 => Some(1),
                                    KeyCode::Digit3 | KeyCode::Numpad3 => Some(2),
                                    KeyCode::Digit4 | KeyCode::Numpad4 => Some(3),
                                    _ => None,
                                };
                                if let Some(idx) = btn_idx {
                                    if idx < mesg.buttons.len() {
                                        state.vm.show_messages.remove(0);
                                        state.vm.set_button_pressed(idx as i32);
                                        println!("[MessageMenu] ボタン選択: {} -> GetButtonPressed", idx);
                                        state.window.request_redraw();
                                        return;
                                    }
                                }
                            }
                        }
                    }

                    if pressed && state.mode.is_ui_active() {
                        if state.mode.handle_key_with_vm(key, &mut state.vm) {
                            state.window.request_redraw();
                            return;
                        }
                    }

                    // キーマネージャーの押下状態更新
                    state.input_manager.handle_key_event(key, pressed);

                    // キャラクタ移動・姿勢入力の同期 (実機 DisablePlayerControls 反映)
                    let can_move = state.vm.player_controls.movement;
                    state.controller.key_forward = can_move && state.input_manager.is_action_down(crate::input::GameAction::Forward);
                    state.controller.key_backward = can_move && state.input_manager.is_action_down(crate::input::GameAction::Backward);
                    state.controller.key_left = can_move && state.input_manager.is_action_down(crate::input::GameAction::StrafeLeft);
                    state.controller.key_right = can_move && state.input_manager.is_action_down(crate::input::GameAction::StrafeRight);
                    state.controller.key_jump = can_move && state.input_manager.is_action_down(crate::input::GameAction::Jump);
                    state.controller.key_sneak = can_move && state.input_manager.is_action_down(crate::input::GameAction::Sneak);
                    state.controller.key_run = !state.input_manager.is_action_down(crate::input::GameAction::Run);

                    if pressed {
                        if let Some(action) = state.input_manager.get_action(key) {
                            match action {
                                crate::input::InputCommand::Game(crate::input::GameAction::Activate) => {
                                    if state.vm.player_controls.movement && !state.vm.in_chargen {
                                        state.interact_or_teleport();
                                    }
                                }
                                crate::input::InputCommand::Game(crate::input::GameAction::TogglePOV) => {
                                    state.controller.player_camera.toggle_view_mode();
                                    if let Some(ref mut player) = state.controller.player_actor {
                                        player.set_view_mode(state.controller.player_camera.mode, &mut state.scene.meshes);
                                    }
                                    println!("[視点切替] 現在の視点モード: {:?}", state.controller.player_camera.mode);
                                }
                                crate::input::InputCommand::Game(crate::input::GameAction::PipBoy) => {
                                    if state.mode.is_ui_active() {
                                        state.mode = ViewerMode::Exploring;
                                    } else {
                                        println!("[Pip-Boy] メニュー (Tab)");
                                    }
                                }
                                crate::input::InputCommand::Debug(crate::input::DebugAction::Help) => {
                                    println!("{}", state.input_manager.get_guide_text());
                                }
                                crate::input::InputCommand::Debug(crate::input::DebugAction::ToggleCollision) => {
                                    state.show_collision = !state.show_collision;
                                    println!("Havok コリジョン表示 [F2]: {}", if state.show_collision { "ON" } else { "OFF" });
                                }
                                crate::input::InputCommand::Debug(crate::input::DebugAction::ToggleFog) => {
                                    state.enable_fog = !state.enable_fog;
                                    println!("セル環境フォグ [F3]: {}", if state.enable_fog { "ON" } else { "OFF" });
                                }
                                crate::input::InputCommand::Debug(crate::input::DebugAction::ToggleHeadlight) => {
                                    state.headlight = !state.headlight;
                                    println!("ビューア補助ヘッドライト [F4]: {}", if state.headlight { "ON" } else { "OFF" });
                                }
                                crate::input::InputCommand::Debug(crate::input::DebugAction::ResetCamera) => {
                                    state.controller.camera.focus(state.scene.bounds_center, state.scene.bounds_radius);
                                    state.controller.character_controller.position = state.scene.bounds_center + glam::Vec3::new(0.0, 0.0, 64.0);
                                    state.controller.vertical_velocity = 0.0;
                                    println!("カメラ・スポーン位置再フォーカス [F7]");
                                }
                                crate::input::InputCommand::Debug(crate::input::DebugAction::ToggleFreeOrbit) => {
                                    state.controller.camera_mode = match state.controller.camera_mode {
                                        CameraMode::Standard => {
                                            println!("\n[カメラモード] F12: フリーオービットカメラ (全体俯瞰・回転周回) に切り替えました。");
                                            println!("  左ドラッグ: 回転, 右ドラッグ: 平行移動, ホイール: ズーム, F12: 実機カメラへ復帰");
                                            state.controller.camera.target = state.controller.character_controller.position;
                                            CameraMode::FreeOrbit
                                        }
                                        CameraMode::FreeOrbit => {
                                            println!("\n[カメラモード] F12: Fallout 3 実機標準プレイヤーカメラに復帰しました。");
                                            println!("  WASD: 移動, Space: ジャンプ, Ctrl: しゃがみ, E: 調べる, F/V: 視点切替");
                                            state.controller.character_controller.position = state.controller.initial_spawn_point;
                                            state.controller.vertical_velocity = 0.0;
                                            CameraMode::Standard
                                        }
                                    };
                                }
                                _ => {}
                            }
                        } else if key == KeyCode::Escape {
                            event_loop.exit();
                        }
                    }
                }
                state.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(ref state) = self.state {
            state.window.request_redraw();
        }
    }
}
