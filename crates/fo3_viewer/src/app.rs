//! ビューアーのメインアプリケーションループおよび GPU レンダリング状態。
//!
//! 参照元: Gamebryo 2.6 レンダリングパイプライン & winit イベントループ

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use fo3_esm::{CellLighting, EsmMasterContext};
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
    find_focused_by_raycast, find_focused_interactable, InteractableKind, InteractableObject,
    F_ACTIVATE_PICK_LENGTH,
};
use crate::interactive_anim::{InteractiveAnimator, RefrBinding};
use crate::inventory::PlayerInventory;
use crate::loader::load_scene;
use crate::types::{print_controls_guide, update_window_title, CameraMode, ViewerTarget};
use crate::ui::{DialogChoice, DialogState, TerminalState, ViewerMode};

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
        let mut vfs = initialize_vfs(data_dir);

        // マスター ESM 静的定義の初期化 (3Dモデル、アクター、防具、光源マップの一括常駐)
        let master_context = Arc::new(initialize_master_context(data_dir));

        // NIF AST および GPU テクスチャの永続キャッシュ
        let mut nif_cache = NifCache::new();
        let mut texture_cache = HashMap::new();

        // シーン・物理・ライティングのロード
        let loaded = load_scene(
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

        let spawn_pos = if let Some((door_pos, _)) = loaded.door_spawn_point {
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
            pos
        } else {
            let ray_origin =
                loaded.scene.bounds_center + glam::Vec3::new(0.0, 0.0, loaded.scene.bounds_radius * 0.5);
            let ray_dir = glam::Vec3::new(0.0, 0.0, -1.0);
            if let Some(hit) =
                loaded.physics_world.cast_ray(ray_origin, ray_dir, loaded.scene.bounds_radius * 2.0)
            {
                println!(
                    "レイキャストによる安全な床面検出に成功: Z = {:.1}",
                    hit.point.z
                );
                hit.point + glam::Vec3::new(0.0, 0.0, 65.0)
            } else {
                loaded.scene.bounds_center + glam::Vec3::new(0.0, 0.0, 64.0)
            }
        };

        let mut controller = Controller::new(
            aspect,
            loaded.scene.bounds_center,
            loaded.scene.bounds_radius,
            spawn_pos,
            loaded.physics_world,
        );

        if let Some((door_pos, _)) = loaded.door_spawn_point {
            let to_center = loaded.scene.bounds_center - door_pos;
            let into_room = if to_center.x.hypot(to_center.y) > 1.0 {
                glam::Vec3::new(to_center.x, to_center.y, 0.0).normalize()
            } else {
                glam::Vec3::X
            };
            controller.camera.yaw = into_room.y.atan2(into_room.x);
            controller.camera.pitch = 0.0;
        }

        let enable_fog = if let Some(ref cl) = loaded.cell_lighting {
            if cl.fog_far > 0.0 && controller.camera.distance > cl.fog_far {
                println!(
                    "注記: カメラ距離 ({:.1}) がセルフォグ Far ({:.1}) を超えているため、初期状態でフォグを無効化しています (F キーでフォグ表示切替)。",
                    controller.camera.distance, cl.fog_far
                );
                false
            } else {
                true
            }
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

        print_controls_guide();

        Self {
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
        }
    }

    /// フォーカス中オブジェクトに対するインタラクトアクション (`E` キー) を実行。
    /// ドア (`XTEL`) の場合は遷移先セルを解決してロードしテレポートを行う。
    /// 参照元: Gamebryo 2.6 セル遷移 & `references/openmw/components/esm4/loadrefr.cpp:103-127`
    pub fn interact_or_teleport(&mut self) {
        let focused = match &self.focused_interactable {
            Some(f) => f.clone(),
            None => {
                println!("[インタラクト] 正面に操作可能なオブジェクトがありません。");
                return;
            }
        };

        println!("[インタラクト実行] {}", focused.prompt_text());

        match &focused.kind {
            InteractableKind::Door { teleport, lock, is_open } => {
                if let Some(l) = lock {
                    println!("  - このドアは施錠されています (難易度: {})。鍵が必要です。", l.lock_level);
                    return;
                }
                if let Some(ref tp) = teleport {
                    println!(
                        "  - テレポートドア起動: 遷移先ドア FormID 0x{:08X}, 出現座標: {:?}, 出現回転: {:?}",
                        tp.dest_door.0, tp.dest_pos, tp.dest_rot
                    );

                    // Fallout3.esm から dest_door の親セルおよび所属ワールドを逆引き検索
                    let esm_path = std::path::Path::new(&self.data_dir).join("Fallout3.esm");
                    let dest_cell_info = if let Ok(mut reader) = fo3_esm::EsmReader::open(&esm_path) {
                        match reader.find_cell_containing_refr(tp.dest_door) {
                            Ok(Some((cell, _, _, parent_world))) => Some((cell, parent_world)),
                            _ => None,
                        }
                    } else {
                        None
                    };

                    let next_target = if let Some((ref c, ref parent_world)) = dest_cell_info {
                        println!("  - テレポート先セルを解決: \"{}\" (FormID: 0x{:08X})", c.edid, c.form_id.0);
                        if c.is_interior() || parent_world.is_none() {
                            ViewerTarget::Cell(c.edid.clone())
                        } else {
                            let world = parent_world.as_ref().unwrap();
                            println!("  - 所属ワールドスペースを解決: \"{}\" (FormID: 0x{:08X}, 親: {:?})", world.edid, world.form_id.0, world.parent_world);
                            ViewerTarget::World(world.edid.clone(), c.grid)
                        }
                    } else {
                        println!("  - 相手側ドアからセル特定不能のため、出現座標グリッドからロードを試行します");
                        let gx = (tp.dest_pos[0] / 4096.0).floor() as i32;
                        let gy = (tp.dest_pos[1] / 4096.0).floor() as i32;
                        ViewerTarget::World("Wasteland".to_string(), Some((gx, gy)))
                    };

                    println!("  - 次のセルをロード中: {:?}...", next_target);
                    let loaded = load_scene(
                        &self.device,
                        &self.queue,
                        &self.context,
                        &self.data_dir,
                        &next_target,
                        &mut self.vfs,
                        &self.master_context,
                        &mut self.nif_cache,
                        &mut self.texture_cache,
                    );

                    // シーンおよびワールド状態の置換
                    self.scene = loaded.scene;
                    self.cell_lighting = loaded.cell_lighting;
                    self.placed_lights = loaded.placed_lights;
                    self.clear_color = loaded.clear_color;
                    self.interactables = loaded.interactables;
                    self.refr_bindings = loaded.refr_bindings;
                    self.animators.clear();
                    self.focused_interactable = None;
                    self.anim = AnimState::new(&next_target, &mut self.vfs);

                    // プレイヤーの出現位置および姿勢角の設定
                    // テレポートマーカー座標 (tp.dest_pos) は床面（足元）位置。
                    // カプセルコライダー中心は足元から +65.0 単位上の位置に配置する。
                    let marker_pos = glam::Vec3::new(tp.dest_pos[0], tp.dest_pos[1], tp.dest_pos[2]);
                    // 出現位置の真上 (+100.0) から真下へレイキャストを行い、実コリジョン床面へ精密スナップ
                    let ray_origin = marker_pos + glam::Vec3::new(0.0, 0.0, 100.0);
                    let ray_dir = glam::Vec3::new(0.0, 0.0, -1.0);
                    let spawn_pos = if let Some(hit) = loaded.physics_world.cast_ray(ray_origin, ray_dir, 200.0) {
                        println!("  - テレポート先床面コリジョン検出: Z = {:.1} -> スポーン中心 Z = {:.1}", hit.point.z, hit.point.z + 65.0);
                        glam::Vec3::new(marker_pos.x, marker_pos.y, hit.point.z + 65.0)
                    } else {
                        println!("  - テレポート先床面レイキャスト未ヒット: デフォルトオフセット (+65.0) で配置");
                        marker_pos + glam::Vec3::new(0.0, 0.0, 65.0)
                    };

                    self.controller.physics_world = loaded.physics_world;
                    self.controller.initial_spawn_point = spawn_pos;
                    self.controller.character_controller.position = spawn_pos;
                    self.controller.character_controller.is_grounded = true;
                    self.controller.vertical_velocity = 0.0;
                    self.controller.camera.yaw = tp.dest_rot[2];
                    self.controller.camera.pitch = tp.dest_rot[0].clamp(-1.2, 1.2);
                    self.controller.camera.distance = 150.0;
                    self.controller.camera.target = spawn_pos;

                    update_window_title(&self.window, &next_target);
                    println!("  - セル間テレポート完了: プレイヤー座標 {:?}", spawn_pos);
                } else {
                    let currently_open = *is_open;
                    let target_open = !currently_open;
                    println!(
                        "  - 通常ドア開閉アニメーション起動: {} (現在: {})",
                        if target_open { "開く" } else { "閉じる" },
                        if currently_open { "開" } else { "閉" }
                    );
                    let form_id = focused.form_id;
                    if let Some(binding) = self.refr_bindings.get(&form_id) {
                        let anim = self.animators.entry(form_id).or_insert_with(|| {
                            InteractiveAnimator::from_binding(binding, currently_open)
                        });
                        anim.toggle();
                    }

                    // インタラクティブ対象の状態を更新
                    if let Some(obj) = self.interactables.iter_mut().find(|o| o.form_id == form_id) {
                        if let InteractableKind::Door { ref mut is_open, .. } = obj.kind {
                            *is_open = target_open;
                        }
                    }
                    if let Some(ref mut obj) = self.focused_interactable {
                        if obj.form_id == form_id {
                            if let InteractableKind::Door { ref mut is_open, .. } = obj.kind {
                                *is_open = target_open;
                            }
                        }
                    }
                }
            }
            InteractableKind::Container { form_id: _, lock, is_open } => {
                if let Some(l) = lock {
                    println!("  - このコンテナは施錠されています (難易度: {})。鍵が必要です。", l.lock_level);
                    return;
                }
                let currently_open = *is_open;
                let target_open = !currently_open;
                println!(
                    "  - コンテナ開閉アニメーション起動: \"{}\" -> {}",
                    focused.name,
                    if target_open { "開く" } else { "閉じる" }
                );
                let form_id = focused.form_id;
                if let Some(binding) = self.refr_bindings.get(&form_id) {
                    let anim = self.animators.entry(form_id).or_insert_with(|| {
                        InteractiveAnimator::from_binding(binding, currently_open)
                    });
                    anim.toggle();
                }

                // インタラクティブ対象の状態を更新
                if let Some(obj) = self.interactables.iter_mut().find(|o| o.form_id == form_id) {
                    if let InteractableKind::Container { ref mut is_open, .. } = obj.kind {
                        *is_open = target_open;
                    }
                }
                if let Some(ref mut obj) = self.focused_interactable {
                    if obj.form_id == form_id {
                        if let InteractableKind::Container { ref mut is_open, .. } = obj.kind {
                            *is_open = target_open;
                        }
                    }
                }
            }
            InteractableKind::Item { form_id: base_form_id } => {
                // アイテムをプレイヤーインベントリへ追加
                self.inventory.add_item(fo3_esm::FormId(*base_form_id), 1, &focused.name);
                let total = self.inventory.get_count(fo3_esm::FormId(*base_form_id));
                println!(
                    "  - アイテム取得: \"{}\" (FormID: 0x{:08X}) をインベントリに追加 (所持数: {})",
                    focused.name, base_form_id, total
                );

                // 物理ワールドから剛体を削除
                if let Some(binding) = self.refr_bindings.get(&focused.form_id) {
                    for rb in &binding.static_rigid_bodies {
                        self.controller.physics_world.remove_rigid_body(*rb);
                    }
                    for part in &binding.moving_parts {
                        for rb in &part.rigid_bodies {
                            self.controller.physics_world.remove_rigid_body(*rb);
                        }
                    }
                    // GPU メッシュをスケール 0 にして不可視化
                    let mut all_meshes = binding.static_mesh_indices.clone();
                    for part in &binding.moving_parts {
                        all_meshes.extend_from_slice(&part.mesh_indices);
                    }
                    for mesh_idx in all_meshes {
                        if let Some(mesh) = self.scene.meshes.get(mesh_idx) {
                            self.queue.write_buffer(
                                &mesh.model_uniform_buffer,
                                0,
                                bytemuck::cast_slice(&[[[0.0f32; 4]; 4]]),
                            );
                        }
                    }
                }

                // インタラクト候補から削除
                self.interactables.retain(|obj| obj.form_id != focused.form_id);
                self.focused_interactable = None;
            }
            InteractableKind::Actor { form_id: actor_form_id, is_dead } => {
                if *is_dead {
                    println!("  - アクター \"{}\" の所持品を調べます。", focused.name);
                    return;
                }
                println!("  - アクター \"{}\" (FormID: 0x{:08X}) との会話を開始します...", focused.name, actor_form_id);
                let esm_path = std::path::Path::new(&self.data_dir).join("Fallout3.esm");
                if let Ok(mut reader) = fo3_esm::EsmReader::open(&esm_path) {
                    match reader.find_npc_dialogue(fo3_esm::FormId(*actor_form_id)) {
                        Ok((greeting_opt, topics)) => {
                            let greeting = greeting_opt
                                .as_ref()
                                .map(|g| g.response_text.as_str())
                                .unwrap_or("何か用か？");
                            let choices: Vec<DialogChoice> = topics
                                .into_iter()
                                .map(|(dial, infos)| {
                                    let prompt = dial.prompt.unwrap_or(dial.edid);
                                    let response = infos
                                        .first()
                                        .map(|i| i.response_text.clone())
                                        .unwrap_or_else(|| "...".to_string());
                                    let is_goodbye = infos
                                        .first()
                                        .map(|i| i.is_goodbye())
                                        .unwrap_or(false);
                                    DialogChoice { prompt, response, is_goodbye }
                                })
                                .collect();
                            let dialog_state = DialogState::new(&focused.name, greeting, choices);
                            println!("  - 会話UIモードへ遷移: 挨拶「{}」 (選択肢: {} 件)", greeting, dialog_state.choices.len());
                            self.mode = ViewerMode::Dialog(dialog_state);
                        }
                        _ => {
                            println!("  - アクター \"{}\" には利用可能な会話データがありません。", focused.name);
                        }
                    }
                }
            }
            InteractableKind::Terminal { form_id: term_form_id, lock } => {
                if let Some(l) = lock {
                    println!("  - このターミナルは施錠されています (難易度: {})。ハッキングが必要です。", l.lock_level);
                    return;
                }
                println!("  - ターミナル \"{}\" (FormID: 0x{:08X}) を起動します...", focused.name, term_form_id);
                let esm_path = std::path::Path::new(&self.data_dir).join("Fallout3.esm");
                if let Ok(mut reader) = fo3_esm::EsmReader::open(&esm_path) {
                    match reader.find_terminal(fo3_esm::FormId(*term_form_id)) {
                        Ok(Some(term_rec)) => {
                            let term_state = TerminalState::from_record(&term_rec);
                            println!("  - ターミナルUIモードへ遷移: \"{}\" (項目: {} 件)", term_state.title, term_state.menu_items.len());
                            self.mode = ViewerMode::Terminal(term_state);
                        }
                        _ => {
                            println!("  - ターミナルデータが見つかりませんでした。");
                        }
                    }
                }
            }
            InteractableKind::Activator { .. } => {
                println!("  - アクティベーター \"{}\" を作動させました。", focused.name);
            }
        }
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
        let dt = self.controller.update();

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

        // 単体 Anim / Actor モード時のスキニング・アニメーション更新
        self.anim.update(dt, &self.device, &self.queue, &mut self.scene);

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
            match &self.mode {
                ViewerMode::Exploring => {
                    self.hud.render_crosshair(
                        &mut render_pass,
                        &self.queue,
                        self.size.width as f32,
                        self.size.height as f32,
                        elapsed,
                    );
                }
                ViewerMode::Dialog(dialog_state) => {
                    self.hud.render_dialog_overlay(
                        &mut render_pass,
                        &self.queue,
                        self.size.width as f32,
                        self.size.height as f32,
                        dialog_state,
                    );
                }
                ViewerMode::Terminal(term_state) => {
                    self.hud.render_terminal_overlay(
                        &mut render_pass,
                        &self.queue,
                        self.size.width as f32,
                        self.size.height as f32,
                        term_state,
                    );
                }
            }
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
                        CameraMode::Orbit => {
                            if state.controller.left_mouse_down {
                                state.controller.camera.rotate(dx, dy);
                                state.window.request_redraw();
                            } else if state.controller.right_mouse_down {
                                state.controller.camera.pan(dx, dy);
                                state.window.request_redraw();
                            }
                        }
                        CameraMode::Walkthrough => {
                            if state.controller.left_mouse_down || state.controller.right_mouse_down {
                                state.controller.camera.rotate(dx, dy);
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
                state.controller.camera.zoom(zoom_amount);
                state.window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                if let PhysicalKey::Code(key) = event.physical_key {
                    // 会話・ターミナル等の UI 表示中は専用の入力処理を実行
                    if pressed && state.mode.is_ui_active() {
                        if state.mode.handle_key(key) {
                            state.window.request_redraw();
                            return;
                        }
                    }

                    match key {
                        KeyCode::KeyW => state.controller.key_forward = pressed,
                        KeyCode::KeyS => state.controller.key_backward = pressed,
                        KeyCode::KeyA => state.controller.key_left = pressed,
                        KeyCode::KeyD => state.controller.key_right = pressed,
                        KeyCode::Space => state.controller.key_jump = pressed,
                        _ => {}
                    }

                    if pressed {
                        match key {
                            KeyCode::Tab | KeyCode::KeyM => {
                                state.controller.camera_mode = match state.controller.camera_mode {
                                    CameraMode::Orbit => {
                                        println!("\n[カメラモード] FPS ウォークスルー歩行モード (物理演算 & KCC 有効) に切り替えました。");
                                        println!("  WASD: 移動, Space: ジャンプ, マウスドラッグ: 視線変更, Tab/M: オービット復帰");
                                        state.controller.character_controller.position =
                                            state.controller.initial_spawn_point;
                                        state.controller.vertical_velocity = 0.0;
                                        CameraMode::Walkthrough
                                    }
                                    CameraMode::Walkthrough => {
                                        println!("\n[カメラモード] オービットカメラ (全体周回) に切り替えました。");
                                        state.controller.camera.target = state.controller.character_controller.position;
                                        CameraMode::Orbit
                                    }
                                };
                                state.window.request_redraw();
                            }
                            KeyCode::KeyR => {
                                state
                                    .controller
                                    .camera
                                    .focus(state.scene.bounds_center, state.scene.bounds_radius);
                                state.controller.character_controller.position =
                                    state.scene.bounds_center + glam::Vec3::new(0.0, 0.0, 64.0);
                                state.controller.vertical_velocity = 0.0;
                                state.window.request_redraw();
                            }
                            KeyCode::KeyE => {
                                state.interact_or_teleport();
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

/// ゲーム起動時に仮想ファイルシステム (VFS) を初期化し、全 BSA アーカイブのインデックスを常駐させる。
///
/// 参照元: Gamebryo 2.6 アーカイブマネージャ, `references/openmw/components/resource/resourcesystem.hpp`
fn initialize_vfs(data_dir: &str) -> VfsManager {
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
            if let Ok(archive) = fo3_bsa::BsaArchive::open(&bsa_file) {
                vfs.add_bsa(archive);
            }
        }
    }
    vfs
}

/// マスター ESM (`Fallout3.esm`) から全静的定義 (3Dモデル、アクター、防具、光源) を一括ロードして常駐させる。
///
/// 参照元: Bethesda ESM/BSA アーキテクチャ, `knowledge/gamebryo_resource_management_and_caching.md`
fn initialize_master_context(data_dir: &str) -> EsmMasterContext {
    let esm_path = Path::new(data_dir).join("Fallout3.esm");
    if esm_path.exists() {
        println!("マスター ESM \"{:?}\" から静的定義を一括ロード中...", esm_path);
        match EsmMasterContext::open_and_load(&esm_path) {
            Ok(ctx) => {
                println!(
                    "マスター定義ロード完了: 3Dモデル {} 件, アクター {} 件, 防具 {} 件, 光源 {} 件",
                    ctx.model_map.len(),
                    ctx.npc_map.len(),
                    ctx.armor_map.len(),
                    ctx.light_map.len()
                );
                ctx
            }
            Err(e) => {
                eprintln!("警告: マスター定義のロードに失敗しました: {:?}", e);
                EsmMasterContext::default()
            }
        }
    } else {
        EsmMasterContext::default()
    }
}
