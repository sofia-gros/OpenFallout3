//! # fo3_viewer
//!
//! Fallout 3 メッシュ & セルシーンビューアー (winit + wgpu)。
//!
//! 使用法:
//!   - NIF 単体表示: `cargo run -p fo3_viewer -- <data_dir> <nif_relative_path>`
//!   - セル一括表示: `cargo run -p fo3_viewer -- cell <data_dir> <cell_edid>`
//!   - ワールド表示: `cargo run -p fo3_viewer -- world <data_dir> <world_edid> [grid_x] [grid_y]`
//!   - アニメーション: `cargo run -p fo3_viewer -- anim <data_dir> <nif_relative_path> <kf_relative_path>`
//!   - アクター全身合成: `cargo run -p fo3_viewer -- actor <data_dir> [naked | outfit_relative_path] <kf_relative_path>`
//!
//! 例:
//!   `cargo run -p fo3_viewer -- "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "meshes\weapons\1handpistol\10mmpistol.nif"`
//!   `cargo run -p fo3_viewer -- cell "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "Vault101a"`
//!   `cargo run -p fo3_viewer -- cell "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "Springvale"`
//!   `cargo run -p fo3_viewer -- world "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "MegatonWorld"`
//!   `cargo run -p fo3_viewer -- world "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "DCWorld01"`
//!   `cargo run -p fo3_viewer -- world "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "Wasteland" -1 2`
//!   `cargo run -p fo3_viewer -- anim "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "meshes\characters\_male\upperbody.nif" "meshes\characters\_male\idleanims\ttnpchappysubtlelistena.kf"`
//!   `cargo run -p fo3_viewer -- actor "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" naked "meshes\characters\_male\idleanims\ttnpchappysubtlelistena.kf"`
//!   `cargo run -p fo3_viewer -- actor "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "meshes\armor\wastelandclothing01\outfitm.nif" "meshes\characters\_male\idleanims\ttnpchappysubtlelistena.kf"`

use std::collections::HashMap;
use std::env;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use fo3_bsa::BsaArchive;
use fo3_esm::{CellLighting, CellRecord, EsmReader, LandRecord, RefrRecord};
use fo3_gamebryo_core::NiTransform;
use fo3_nif::collision::extract_collision_data;
use fo3_nif::NifFile;
use fo3_physics::{RapierCharacterController, RapierPhysicsWorld};
use fo3_render::{
    AnimationPlayer, LightingUniform, OrbitCamera, PlacedPointLight, RenderContext,
    RenderScene, SkeletonPose,
};
use fo3_vfs::VfsManager;
use glam::Mat4;
use pollster::FutureExt;
use std::time::Instant;
use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

/// カメラの動作モード。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CameraMode {
    /// オービットカメラ（ターゲット注視・全体周回）
    Orbit,
    /// FPS ウォークスルー歩行モード（物理エンジン + キャラクタコントローラー）
    Walkthrough,
}

/// ビューアーの表示対象。
#[derive(Clone, Debug)]
enum ViewerTarget {
    Mesh(String),
    Cell(String),
    World(String, Option<(i32, i32)>),
    /// 単一スキンメッシュ + KF アニメーション再生モード
    Anim {
        nif_path: String,
        kf_path: String,
    },
    /// 人型アクター（全身パーツ合成: 頭部、胴体/衣装、右手、左手）+ KF アニメーション再生モード
    Actor {
        outfit_or_naked: String,
        kf_path: String,
    },
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
    // カメラおよび物理
    camera_mode: CameraMode,
    physics_world: RapierPhysicsWorld,
    character_controller: RapierCharacterController,
    initial_spawn_point: glam::Vec3,
    vertical_velocity: f32,
    last_frame_time: Instant,
    // キー入力状態
    key_forward: bool,
    key_backward: bool,
    key_left: bool,
    key_right: bool,
    key_jump: bool,
    // マウス入力状態
    left_mouse_down: bool,
    right_mouse_down: bool,
    last_mouse_pos: Option<(f64, f64)>,
    // アニメーション再生状態 (Anim / Actor モード時のみ Some)
    anim_player: Option<AnimationPlayer>,
    /// KF ファイルの NifFile（アニメーションデータ）
    anim_kf_nif: Option<NifFile>,
    /// パーツメッシュ NIF 群（スキニング変形対象）
    anim_parts: Vec<NifFile>,
    /// スケルトン NIF（真のボーン階層ツリー走査用）
    anim_skeleton_nif: Option<NifFile>,
    /// 現在の骨格姿勢（KF → SkeletonPose のオーバーライド）
    anim_pose: SkeletonPose,
    /// ボーンワールド行列マップ（block_index → Mat4）
    anim_bone_world_map: HashMap<i32, Mat4>,
    /// ボーン名ワールド行列マップ（bone_name → Mat4）
    anim_bone_name_world_map: HashMap<String, Mat4>,
}

impl ViewerState {
    async fn new(window: Arc<Window>, data_dir: &str, target: &ViewerTarget) -> Self {
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
        let (scene, cell_lighting, placed_lights, clear_color, physics_world, door_spawn_point) =
            match target {
                ViewerTarget::Mesh(nif_path) | ViewerTarget::Anim { nif_path, .. } => {
                    println!("VFS から NIF ファイルを取得中: {}", nif_path);
                    let nif_bytes = vfs.read(nif_path).expect("Failed to read NIF from VFS");
                    let mut cursor = Cursor::new(nif_bytes);
                    let nif_file = NifFile::read(&mut cursor).expect("Failed to parse NIF");
                    println!(
                        "NIF パース成功 (ブロック数: {})。GPU シーンを構築中...",
                        nif_file.blocks.len()
                    );
                    let scene =
                        RenderScene::from_nif(&device, &queue, &context, &nif_file, &mut vfs);

                    let mut physics_world = RapierPhysicsWorld::new();
                    let col_data = extract_collision_data(&nif_file);
                    if !col_data.bodies.is_empty() {
                        physics_world.add_nif_collision(
                            &col_data,
                            glam::Vec3::ZERO,
                            glam::Quat::IDENTITY,
                        );
                        println!(
                            "物理ワールド登録: 単体メッシュ コリジョン剛体数 {}",
                            col_data.bodies.len()
                        );
                    }

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
                        physics_world,
                        None,
                    )
                }
                ViewerTarget::Actor {
                    outfit_or_naked,
                    kf_path,
                } => {
                    println!(
                        "人型アクター全身パーツ合成モード (outfit: {}, anim: {})",
                        outfit_or_naked, kf_path
                    );
                    let body_path = if outfit_or_naked.eq_ignore_ascii_case("naked") {
                        "meshes\\characters\\_male\\upperbody.nif".to_string()
                    } else {
                        outfit_or_naked.clone()
                    };

                    let is_female = outfit_or_naked.to_ascii_lowercase().contains("female")
                        || outfit_or_naked.to_ascii_lowercase().contains("outfitf");
                    let part_paths = get_actor_part_paths(is_female, &body_path);

                    let mut parts = Vec::new();
                    for path in &part_paths {
                        println!("VFS からアクターパーツ NIF を取得中: {}", path);
                        match vfs.read(path) {
                            Ok(bytes) => {
                                let mut cursor = Cursor::new(bytes);
                                match NifFile::read(&mut cursor) {
                                    Ok(nif) => {
                                        println!(
                                            "  パーツロード成功: {} (ブロック数: {})",
                                            path,
                                            nif.blocks.len()
                                        );
                                        parts.push(nif);
                                    }
                                    Err(e) => eprintln!("  パーツパースエラー {}: {}", path, e),
                                }
                            }
                            Err(e) => eprintln!("  パーツ読み込みエラー {}: {}", path, e),
                        }
                    }

                    // スケルトン NIF の読み込み
                    let skel_path = "meshes\\characters\\_male\\skeleton.nif";
                    println!("スケルトン NIF をロード中: {}", skel_path);
                    let skel_bytes = vfs
                        .read(skel_path)
                        .expect("スケルトン NIF の読み込みに失敗しました");
                    let mut skel_cursor = Cursor::new(skel_bytes);
                    let skel_nif =
                        NifFile::read(&mut skel_cursor).expect("スケルトン NIF のパースに失敗しました");

                    let part_refs: Vec<&NifFile> = parts.iter().collect();
                    let scene = RenderScene::from_actor_parts(
                        &device, &queue, &context, &skel_nif, &part_refs, &mut vfs,
                    );

                    let mut physics_world = RapierPhysicsWorld::new();
                    let col_data = extract_collision_data(&skel_nif);
                    if !col_data.bodies.is_empty() {
                        physics_world.add_nif_collision(
                            &col_data,
                            glam::Vec3::ZERO,
                            glam::Quat::IDENTITY,
                        );
                    }

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
                        physics_world,
                        None,
                    )
                }
                ViewerTarget::Cell(..) | ViewerTarget::World(..) => {
                    let esm_path = data_p.join("Fallout3.esm");
                    let mut esm_reader =
                        EsmReader::open(&esm_path).expect("Failed to open Fallout3.esm");

                    // セル群の検索
                    let cells: Vec<(CellRecord, Vec<RefrRecord>, Option<LandRecord>)> = match target
                    {
                        ViewerTarget::Cell(cell_edid) => {
                            println!("ESM からセル \"{}\" および近傍セルを検索中...", cell_edid);
                            esm_reader
                                .find_cell_and_neighbors(cell_edid, 1)
                                .expect("Failed to find cell and neighbors")
                                .unwrap_or_else(|| {
                                    panic!("セル \"{}\" が見つかりませんでした", cell_edid)
                                })
                        }
                        ViewerTarget::World(world_edid, grid_opt) => {
                            println!("ESM からワールド \"{}\" を検索中...", world_edid);
                            let (world_rec, group_start, group_end) = esm_reader
                                .find_world_by_edid(world_edid)
                                .expect("Failed to search world")
                                .unwrap_or_else(|| {
                                    panic!(
                                        "ワールドスペース \"{}\" が見つかりませんでした",
                                        world_edid
                                    )
                                });
                            println!(
                                "ワールドスペース発見: \"{}\" (FormID: 0x{:08X}, 表示名: {:?})",
                                world_rec.edid, world_rec.form_id.0, world_rec.full_name
                            );
                            println!(
                                "ワールド所属セル群を走査中 (中心グリッド: {:?}, 半径: 1)...",
                                grid_opt
                            );
                            let (resolved_center, cells) = esm_reader
                                .read_cells_in_world_region(group_start, group_end, *grid_opt, 1)
                                .expect("Failed to read world cells");
                            println!(
                                "走査結果: 解決中心グリッド: {:?}, 取得セル数: {}",
                                resolved_center,
                                cells.len()
                            );
                            if cells.is_empty() {
                                panic!(
                                    "ワールド \"{}\" 内に指定グリッドのセルが見つかりませんでした",
                                    world_edid
                                );
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
                    let model_map = esm_reader
                        .read_all_models_map()
                        .expect("Failed to read models map");
                    println!("モデルマップ登録件数: {} 件", model_map.len());
                    let (npc_map, armor_map) =
                        esm_reader.read_npc_and_armor_map().unwrap_or_default();
                    println!(
                        "アクター定義: {} 件, 防具定義: {} 件",
                        npc_map.len(),
                        armor_map.len()
                    );
                    let light_map = esm_reader.read_light_map().unwrap_or_default();
                    println!("光源レコード (LIGHT) 登録件数: {} 件", light_map.len());

                    struct CellNpcSpawn {
                        form_id: u32,
                        name: String,
                        transform: NiTransform,
                        is_female: bool,
                        outfit_model: Option<String>,
                    }
                    let mut cell_npcs: Vec<CellNpcSpawn> = Vec::new();

                    let mut nif_cache: HashMap<String, Arc<NifFile>> = HashMap::new();
                    let mut all_cell_items: Vec<Vec<(Arc<NifFile>, NiTransform)>> = Vec::new();
                    let mut placed_lights: Vec<PlacedPointLight> = Vec::new();
                    let mut skipped_markers = 0;
                    let mut primary_lighting: Option<CellLighting> = None;
                    let mut door_spawn_point: Option<(glam::Vec3, f32)> = None;

                    for (cell, refrs, _) in &cells {
                        if primary_lighting.is_none() && cell.lighting.is_some() {
                            primary_lighting = cell.lighting.clone();
                        }

                        // 出入口ドア (XTEL) またはプレイヤー出現ポイントの検索
                        // 外部洞窟出口 (CaveDoor / ExitDoor) ではなく、Vault 内部居住区への連絡ドアを優先する
                        if door_spawn_point.is_none() {
                            let mut fallback_door = None;
                            for refr in refrs {
                                if refr.teleport.is_some() {
                                    let is_exit = refr.edid.to_ascii_lowercase().contains("cave")
                                        || refr.edid.to_ascii_lowercase().contains("exit");
                                    let door_pos = glam::Vec3::new(
                                        refr.position[0],
                                        refr.position[1],
                                        refr.position[2],
                                    );
                                    let yaw = refr.rotation[2];
                                    if is_exit {
                                        if fallback_door.is_none() {
                                            fallback_door = Some((door_pos, yaw));
                                        }
                                    } else {
                                        door_spawn_point = Some((door_pos, yaw));
                                        break;
                                    }
                                }
                            }
                            if door_spawn_point.is_none() {
                                door_spawn_point = fallback_door;
                            }
                        }

                        let mut cell_items: Vec<(Arc<NifFile>, NiTransform)> = Vec::new();
                        for refr in refrs {
                            // 配置点光源の収集 (LIGHT レコード)
                            if let Some(light_rec) = light_map.get(&refr.base_object) {
                                let pos = glam::Vec3::new(
                                    refr.position[0],
                                    refr.position[1],
                                    refr.position[2],
                                );
                                let r = light_rec.colour[0] as f32 / 255.0;
                                let g = light_rec.colour[1] as f32 / 255.0;
                                let b = light_rec.colour[2] as f32 / 255.0;
                                let radius = if light_rec.radius > 0 {
                                    light_rec.radius as f32
                                } else {
                                    500.0
                                };
                                let falloff = if light_rec.falloff > 0.01 {
                                    light_rec.falloff
                                } else {
                                    1.0
                                };
                                placed_lights.push(PlacedPointLight {
                                    position: pos,
                                    radius,
                                    color: [r, g, b],
                                    falloff,
                                });
                            }

                            // アクター (NPC_ / ACHR) の検出と収集
                            if let Some(npc) = npc_map.get(&refr.base_object) {
                                let pos = glam::Vec3::new(
                                    refr.position[0],
                                    refr.position[1],
                                    refr.position[2],
                                );
                                let rot = glam::Vec3::new(
                                    refr.rotation[0],
                                    refr.rotation[1],
                                    refr.rotation[2],
                                );
                                let world_transform =
                                    NiTransform::from_euler_xyz(pos, rot, refr.scale);

                                let outfit_model = npc.default_armor
                                    .and_then(|armo_id| armor_map.get(&armo_id))
                                    .and_then(|armo| {
                                        let m = if npc.is_female && !armo.female_model.is_empty() {
                                            &armo.female_model
                                        } else {
                                            &armo.male_model
                                        };
                                        if !m.is_empty() {
                                            Some(m.clone())
                                        } else {
                                            None
                                        }
                                    });

                                let name = npc.full_name.clone().unwrap_or_else(|| npc.edid.clone());
                                cell_npcs.push(CellNpcSpawn {
                                    form_id: refr.form_id.0,
                                    name,
                                    transform: world_transform,
                                    is_female: npc.is_female,
                                    outfit_model,
                                });
                                continue;
                            }

                            // 3D モデルパスの決定 (通常オブジェクト)
                            let mesh_file_path = if let Some(obj_info) =
                                model_map.get(&refr.base_object)
                            {
                                if obj_info.model.is_empty() {
                                    None
                                } else if is_editor_marker_or_effect(
                                    &obj_info.edid,
                                    &obj_info.model,
                                ) {
                                    skipped_markers += 1;
                                    None
                                } else {
                                    Some(obj_info.model.clone())
                                }
                            } else {
                                None
                            };

                            if let Some(model_path) = mesh_file_path {
                                let model_key = model_path.to_ascii_lowercase();
                                let nif = if let Some(n) = nif_cache.get(&model_key) {
                                    n.clone()
                                } else {
                                    let path = if model_key.starts_with("meshes\\")
                                        || model_key.starts_with("meshes/")
                                    {
                                        model_path.clone()
                                    } else {
                                        format!("meshes\\{}", model_path)
                                    };
                                    match vfs.read(&path) {
                                        Ok(bytes) => {
                                            let mut cursor = Cursor::new(bytes);
                                            match NifFile::read(&mut cursor) {
                                                Ok(parsed) => {
                                                    let arc = Arc::new(parsed);
                                                    nif_cache.insert(model_key, arc.clone());
                                                    arc
                                                }
                                                Err(e) => {
                                                    eprintln!("警告: メッシュ \"{}\" のパースに失敗しました（スキップします）: {}", path, e);
                                                    continue;
                                                }
                                            }
                                        }
                                        Err(_) => continue,
                                    }
                                };

                                let pos = glam::Vec3::new(
                                    refr.position[0],
                                    refr.position[1],
                                    refr.position[2],
                                );
                                let rot = glam::Vec3::new(
                                    refr.rotation[0],
                                    refr.rotation[1],
                                    refr.rotation[2],
                                );
                                let world_transform =
                                    NiTransform::from_euler_xyz(pos, rot, refr.scale);
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

                    let cell_inputs: Vec<(
                        Vec<(&NifFile, NiTransform)>,
                        Option<(&LandRecord, i32, i32)>,
                    )> = cells
                        .iter()
                        .zip(all_cell_items.iter())
                        .map(|((cell, _, land), items)| {
                            let placed_refs: Vec<(&NifFile, NiTransform)> =
                                items.iter().map(|(n, t)| (n.as_ref(), *t)).collect();
                            let land_info = land
                                .as_ref()
                                .and_then(|l| cell.grid.map(|(gx, gy)| (l, gx, gy)));
                            (placed_refs, land_info)
                        })
                        .collect();

                    let cell_refs: Vec<(
                        &[(&NifFile, NiTransform)],
                        Option<(&LandRecord, i32, i32)>,
                    )> = cell_inputs
                        .iter()
                        .map(|(refs, land_info)| (refs.as_slice(), *land_info))
                        .collect();

                    let landscape_texture_map = esm_reader.read_landscape_texture_map().ok();
                    if let Some(ref tex_map) = landscape_texture_map {
                        println!("地形テクスチャセット解決: {} 件", tex_map.len());
                    }

                    let mut scene = RenderScene::from_cells(
                        &device,
                        &queue,
                        &context,
                        &cell_refs,
                        landscape_texture_map.as_ref(),
                        &mut vfs,
                    );

                    if !cell_npcs.is_empty() {
                        println!(
                            "セル内配置アクター (ACHR / NPC_) を生成中: {} 体...",
                            cell_npcs.len()
                        );
                        let mut actor_texture_cache = HashMap::new();

                        // スケルトン NIF のキャッシュ
                        let skel_male_path = "meshes\\characters\\_male\\skeleton.nif";
                        let skel_female_path = "meshes\\characters\\_female\\skeleton.nif";
                        let skel_male = if let Ok(bytes) = vfs.read(skel_male_path) {
                            let mut cursor = Cursor::new(bytes);
                            NifFile::read(&mut cursor).ok().map(Arc::new)
                        } else {
                            None
                        };
                        let skel_female = if let Ok(bytes) = vfs.read(skel_female_path) {
                            let mut cursor = Cursor::new(bytes);
                            NifFile::read(&mut cursor).ok().map(Arc::new)
                        } else {
                            None
                        };

                        // アイドルアニメーション KF のキャッシュ
                        let idle_kf_path =
                            "meshes\\characters\\_male\\idleanims\\ttnpchappysubtlelistena.kf";
                        let (kf_nif, anim_clip) = if let Ok(bytes) = vfs.read(idle_kf_path) {
                            let mut cursor = Cursor::new(bytes);
                            if let Ok(kf) = NifFile::read(&mut cursor) {
                                let clip = fo3_render::AnimationClip::from_kf(&kf).map(Arc::new);
                                (Some(Arc::new(kf)), clip)
                            } else {
                                (None, None)
                            }
                        } else {
                            (None, None)
                        };

                        for npc in &cell_npcs {
                            let skeleton = if npc.is_female {
                                skel_female.clone().or_else(|| skel_male.clone())
                            } else {
                                skel_male.clone().or_else(|| skel_female.clone())
                            };

                            let Some(skel) = skeleton else {
                                eprintln!(
                                    "警告: スケルトン NIF が読み込めないためアクター \"{}\" をスキップします",
                                    npc.name
                                );
                                continue;
                            };

                            let body_path = if let Some(ref model) = npc.outfit_model {
                                model.clone()
                            } else if npc.is_female {
                                "meshes\\characters\\_female\\upperbody.nif".to_string()
                            } else {
                                "meshes\\characters\\_male\\upperbody.nif".to_string()
                            };

                            let part_paths = get_actor_part_paths(npc.is_female, &body_path);
                            let mut parts = Vec::new();
                            for path in &part_paths {
                                let nif = if let Some(cached) = nif_cache.get(path) {
                                    Some(cached.clone())
                                } else {
                                    let full_path = if path.starts_with("meshes\\")
                                        || path.starts_with("meshes/")
                                    {
                                        path.clone()
                                    } else {
                                        format!("meshes\\{}", path)
                                    };
                                    if let Ok(bytes) = vfs.read(&full_path) {
                                        let mut cursor = Cursor::new(bytes);
                                        if let Ok(parsed) = NifFile::read(&mut cursor) {
                                            let arc = Arc::new(parsed);
                                            nif_cache.insert(path.clone(), arc.clone());
                                            Some(arc)
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                };
                                if let Some(p) = nif {
                                    parts.push(p);
                                }
                            }

                            scene.add_actor(
                                &device,
                                &queue,
                                &context,
                                &mut vfs,
                                npc.form_id,
                                &npc.name,
                                &npc.transform,
                                skel,
                                parts,
                                kf_nif.clone(),
                                anim_clip.clone(),
                                &mut actor_texture_cache,
                            );
                            println!(
                                "  - アクター \"{}\" (FormID: 0x{:08X}, 性別: {}) を配置 (パーツ数: {})",
                                npc.name,
                                npc.form_id,
                                if npc.is_female { "女" } else { "男" },
                                part_paths.len()
                            );
                        }
                    }

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

                    println!("セル内の Havok コリジョン情報を物理ワールドに登録中...");
                    let mut physics_world = RapierPhysicsWorld::new();
                    let mut total_colliders = 0;

                    // 1. 地形 (LAND) コライダーの登録
                    for (_, land_info) in &cell_inputs {
                        if let Some((land, gx, gy)) = land_info {
                            let heights = land.compute_heights();
                            if physics_world
                                .add_land_collision(&heights, *gx, *gy)
                                .is_some()
                            {
                                total_colliders += 1;
                            }
                        }
                    }

                    // 2. 配置オブジェクト (REFR) コライダーの登録
                    for (nif, world_transform) in
                        cell_inputs.iter().flat_map(|(items, _)| items.iter())
                    {
                        let col_data = extract_collision_data(nif);
                        if !col_data.bodies.is_empty() {
                            total_colliders += col_data.bodies.len();
                            let quat = glam::Quat::from_mat3(&world_transform.rotation);
                            physics_world.add_nif_collision(
                                &col_data,
                                world_transform.translation,
                                quat,
                            );
                        }
                    }
                    println!("物理ワールド構築完了: 登録剛体数 {}", total_colliders);

                    (
                        scene,
                        primary_lighting,
                        placed_lights,
                        clear_color,
                        physics_world,
                        door_spawn_point,
                    )
                }
            };

        println!(
            "GPU シーン構築完了: {} メッシュノード描画準備完了",
            scene.meshes.len()
        );

        let title = match target {
            ViewerTarget::Mesh(path) => format!("OpenFallout3 - Mesh: {}", path),
            ViewerTarget::Cell(edid) => format!("OpenFallout3 - Cell: {}", edid),
            ViewerTarget::World(edid, _) => format!("OpenFallout3 - World: {}", edid),
            ViewerTarget::Anim { nif_path, kf_path } => {
                format!("OpenFallout3 - Anim: {} + {}", nif_path, kf_path)
            }
            ViewerTarget::Actor {
                outfit_or_naked,
                kf_path,
            } => {
                format!("OpenFallout3 - Actor: {} + {}", outfit_or_naked, kf_path)
            }
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
        println!("  Tab / M キー:     カメラモード切替 [オービット周回 ⇔ FPS歩行モード]");
        println!("  -- オービットモード (Orbit) --");
        println!("    左ドラッグ:     カメラ回転 (Yaw / Pitch)");
        println!("    右ドラッグ:     カメラ平行移動 (Pan)");
        println!("    ホイール:       ズームイン / アウト");
        println!("    R キー:         カメラ自動再フォーカス (Reset)");
        println!("  -- FPS歩行モード (Walkthrough / Physics) --");
        println!("    WASD キー:      前後・左右移動 (コリジョン・階段昇降対応)");
        println!("    Space キー:     ジャンプ / 上昇");
        println!("    マウス移動:     視線方向回転 (Look)");
        println!("  -- 共通 --");
        println!("    C キー:         Havok コリジョンワイヤーフレーム表示切替 (Collision ON/OFF)");
        println!("    F キー:         セル環境フォグ表示切替 (Fog ON/OFF)");
        println!("    L キー:         ビューア補助ヘッドライト切替 (Light ON/OFF)");
        println!("    Esc キー:       終了\n");

        // キャラクタコントローラーの初期配置
        // 1. セル内の出入口ドア (XTEL) があれば、外から室内（セル中心方向）へ入ってきた位置にオフセットしてスポーン
        // 2. なければセル中心から下向きにレイキャストを飛ばして床コリジョンを検出
        let spawn_pos = if let Some((door_pos, _)) = door_spawn_point {
            // ドアから室内（セル中心）へ向かう水平方向ベクトル
            let to_center = scene.bounds_center - door_pos;
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
            // カメラの向きも室内方向に向ける
            camera.yaw = into_room.y.atan2(into_room.x);
            camera.pitch = 0.0;
            pos
        } else {
            let ray_origin =
                scene.bounds_center + glam::Vec3::new(0.0, 0.0, scene.bounds_radius * 0.5);
            let ray_dir = glam::Vec3::new(0.0, 0.0, -1.0);
            if let Some(hit) =
                physics_world.cast_ray(ray_origin, ray_dir, scene.bounds_radius * 2.0)
            {
                println!(
                    "レイキャストによる安全な床面検出に成功: Z = {:.1}",
                    hit.point.z
                );
                hit.point + glam::Vec3::new(0.0, 0.0, 65.0)
            } else {
                scene.bounds_center + glam::Vec3::new(0.0, 0.0, 64.0)
            }
        };

        let character_controller = RapierCharacterController::new(spawn_pos);

        // アニメーションモードまたはアクターモード時: KF + スケルトン + パーツ群をロードして AnimationPlayer を構築
        let (
            anim_player,
            anim_kf_nif,
            anim_parts,
            anim_skeleton_nif,
            anim_pose,
            anim_bone_world_map,
            anim_bone_name_world_map,
        ) = {
            if let ViewerTarget::Actor { outfit_or_naked, kf_path } = target {
                use fo3_render::{recompute_bone_world_maps_with_pose, AnimationClip};

                // 1. KF ファイル読み込み
                let kf_bytes = vfs
                    .read(kf_path)
                    .expect("KF ファイルの読み込みに失敗しました");
                let mut kf_cursor = Cursor::new(kf_bytes);
                let kf_nif =
                    NifFile::read(&mut kf_cursor).expect("KF ファイルのパースに失敗しました");
                println!("KF パース成功 (ブロック数: {})", kf_nif.blocks.len());

                // 2. スケルトン NIF 読み込み
                let skel_path = "meshes\\characters\\_male\\skeleton.nif";
                let skel_bytes = vfs
                    .read(skel_path)
                    .expect("スケルトン NIF の読み込みに失敗しました");
                let mut skel_cursor = Cursor::new(skel_bytes);
                let skel_nif =
                    NifFile::read(&mut skel_cursor).expect("スケルトン NIF のパースに失敗しました");

                // 3. 全パーツ NIF 群の読み込み
                let body_path = if outfit_or_naked.eq_ignore_ascii_case("naked") {
                    "meshes\\characters\\_male\\upperbody.nif".to_string()
                } else {
                    outfit_or_naked.clone()
                };
                let is_female = outfit_or_naked.to_ascii_lowercase().contains("female")
                    || outfit_or_naked.to_ascii_lowercase().contains("outfitf");
                let part_paths = get_actor_part_paths(is_female, &body_path);
                let mut anim_parts = Vec::new();
                for path in &part_paths {
                    if let Ok(bytes) = vfs.read(path) {
                        let mut cursor = Cursor::new(bytes);
                        if let Ok(nif) = NifFile::read(&mut cursor) {
                            anim_parts.push(nif);
                        }
                    }
                }

                // 4. 初期ポーズでボーンワールド行列およびボーン名マップを計算
                let mut bone_world_map = HashMap::new();
                let mut bone_name_world_map = HashMap::new();
                let initial_pose = SkeletonPose::default();
                recompute_bone_world_maps_with_pose(
                    &skel_nif,
                    &initial_pose,
                    &mut bone_world_map,
                    &mut bone_name_world_map,
                );

                // AnimationClip を構築して AnimationPlayer を作成
                let player = AnimationClip::from_kf(&kf_nif).map(|clip| {
                    println!(
                        "アクターアニメーションクリップ \"{}\" ロード完了: {:.2}s〜{:.2}s, チャンネル数: {}",
                        clip.name, clip.start_time, clip.stop_time, clip.channels.len()
                    );
                    AnimationPlayer::new(clip)
                });

                (
                    player,
                    Some(kf_nif),
                    anim_parts,
                    Some(skel_nif),
                    initial_pose,
                    bone_world_map,
                    bone_name_world_map,
                )
            } else if let ViewerTarget::Anim { nif_path, kf_path } = target {
                use fo3_render::{recompute_bone_world_maps_with_pose, AnimationClip};

                // 1. KF ファイル読み込み
                let kf_bytes = vfs
                    .read(kf_path)
                    .expect("KF ファイルの読み込みに失敗しました");
                let mut kf_cursor = Cursor::new(kf_bytes);
                let kf_nif =
                    NifFile::read(&mut kf_cursor).expect("KF ファイルのパースに失敗しました");
                println!("KF パース成功 (ブロック数: {})", kf_nif.blocks.len());

                // 2. メッシュ NIF 読み込み
                let mesh_bytes = vfs
                    .read(nif_path)
                    .expect("メッシュ NIF の読み込みに失敗しました");
                let mut mesh_cursor = Cursor::new(mesh_bytes);
                let mesh_nif =
                    NifFile::read(&mut mesh_cursor).expect("メッシュ NIF のパースに失敗しました");

                // 3. スケルトン NIF（真のボーン階層ツリー）の自動検出と読み込み
                // キャラクタパーツ（upperbody.nif, outfit*.nif など）はボーン階層がフラットなため、
                // 完全な親子ツリーを持つ skeleton.nif をロードしてアニメーションを適用する。
                // 参照元: Gamebryo 2.6 アクター構造、knowledge/actor_and_skin_mesh.md (セクション 4.4)
                let lower_path = nif_path.to_ascii_lowercase();
                let candidate_skel_path = if lower_path.contains("characters\\_female") || lower_path.contains("characters/_female") {
                    "meshes\\characters\\_female\\skeleton.nif"
                } else {
                    "meshes\\characters\\_male\\skeleton.nif"
                };

                let skel_nif = match vfs.read(candidate_skel_path) {
                    Ok(skel_bytes) => {
                        let mut skel_cursor = Cursor::new(skel_bytes);
                        match NifFile::read(&mut skel_cursor) {
                            Ok(parsed) => {
                                println!("キャラクタスケルトン \"{}\" を自動検出・ロード完了 (ブロック数: {})", candidate_skel_path, parsed.blocks.len());
                                parsed
                            }
                            Err(e) => {
                                eprintln!("警告: スケルトン \"{}\" のパースに失敗しました: {}。指定メッシュをスケルトンとして代用します。", candidate_skel_path, e);
                                mesh_nif.clone()
                            }
                        }
                    }
                    Err(_) => {
                        println!("指定メッシュ自身をスケルトン階層として使用します: {}", nif_path);
                        mesh_nif.clone()
                    }
                };

                // 初期ポーズでボーンワールド行列およびボーン名マップを計算
                let mut bone_world_map = HashMap::new();
                let mut bone_name_world_map = HashMap::new();
                let initial_pose = SkeletonPose::default();
                recompute_bone_world_maps_with_pose(
                    &skel_nif,
                    &initial_pose,
                    &mut bone_world_map,
                    &mut bone_name_world_map,
                );

                // AnimationClip を構築して AnimationPlayer を作成
                let player = AnimationClip::from_kf(&kf_nif).map(|clip| {
                    println!(
                        "アニメーションクリップ \"{}\" ロード完了: {:.2}s〜{:.2}s, チャンネル数: {}",
                        clip.name, clip.start_time, clip.stop_time, clip.channels.len()
                    );
                    AnimationPlayer::new(clip)
                });

                (
                    player,
                    Some(kf_nif),
                    vec![mesh_nif],
                    Some(skel_nif),
                    initial_pose,
                    bone_world_map,
                    bone_name_world_map,
                )
            } else {
                (
                    None,
                    None,
                    Vec::new(),
                    None,
                    SkeletonPose::default(),
                    HashMap::new(),
                    HashMap::new(),
                )
            }
        };

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
            camera_mode: CameraMode::Orbit,
            physics_world,
            character_controller,
            initial_spawn_point: spawn_pos,
            vertical_velocity: 0.0,
            last_frame_time: Instant::now(),
            key_forward: false,
            key_backward: false,
            key_left: false,
            key_right: false,
            key_jump: false,
            left_mouse_down: false,
            right_mouse_down: false,
            last_mouse_pos: None,
            anim_player,
            anim_kf_nif,
            anim_parts,
            anim_skeleton_nif,
            anim_pose,
            anim_bone_world_map,
            anim_bone_name_world_map,
        }
    }

    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_view =
                RenderContext::create_depth_texture(&self.device, new_size.width, new_size.height);
            self.camera.aspect = new_size.width as f32 / new_size.height as f32;
        }
    }

    fn update(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame_time).as_secs_f32().clamp(0.001, 0.1);
        self.last_frame_time = now;

        // FPS ウォークスルー歩行モード時の物理シミュレーション
        if self.camera_mode == CameraMode::Walkthrough {
            // 水平面上の移動方向（カメラのヨー角から計算: Z-up 右手系）
            let forward =
                glam::Vec3::new(self.camera.yaw.cos(), self.camera.yaw.sin(), 0.0).normalize();
            let right =
                glam::Vec3::new(self.camera.yaw.sin(), -self.camera.yaw.cos(), 0.0).normalize();

            let mut move_dir = glam::Vec3::ZERO;
            if self.key_forward {
                move_dir += forward;
            }
            if self.key_backward {
                move_dir -= forward;
            }
            if self.key_right {
                move_dir += right;
            }
            if self.key_left {
                move_dir -= right;
            }

            let move_speed = 300.0; // ゲーム単位/秒 (約 4.3 m/s)
            let horiz_velocity = if move_dir.length_squared() > 0.001 {
                move_dir.normalize() * move_speed
            } else {
                glam::Vec3::ZERO
            };

            // 重力とジャンプ
            let gravity = -980.0; // 重力加速度 (約 -14 m/s^2)
            if self.character_controller.is_grounded {
                if self.key_jump {
                    self.vertical_velocity = 350.0; // ジャンプ初速
                } else {
                    self.vertical_velocity = -10.0; // 地面スナップ維持のための微小押し下げ
                }
            } else {
                self.vertical_velocity += gravity * dt;
                self.vertical_velocity = self.vertical_velocity.clamp(-1200.0, 500.0);
            }

            let desired_translation =
                (horiz_velocity + glam::Vec3::new(0.0, 0.0, self.vertical_velocity)) * dt;

            // 物理エンジンによる移動計算 (階段自動昇降・衝突スライド・接地判定)
            self.character_controller.step_move(
                dt,
                desired_translation,
                &self.physics_world.rigid_body_set,
                &self.physics_world.collider_set,
                &self.physics_world.query_pipeline,
            );

            // カメラの目の高さをキャラクタ位置 + 55 単位（プレイヤーアイレベル 約 119）に設定
            let eye_level = self.character_controller.position + glam::Vec3::new(0.0, 0.0, 55.0);
            self.camera.override_eye = Some(eye_level);
        } else {
            self.camera.override_eye = None;
        }

        let uniform = self.camera.build_uniform();
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

        // セルまたはワールド内に配置された全アクターのアニメーション・スキニング更新
        self.scene.update_actors(dt, &self.device, &self.queue);

        // アニメーション更新ループ (Anim / Actor モード時)
        // 参照元: Gamebryo 2.6 `NiControllerSequence::Update` → `NiSkinInstance::Update`
        // 参照元: `knowledge/actor_and_skin_mesh.md` (セクション 4.4: スケルトン分離とボーン名マッピング)
        if let (Some(player), Some(kf_nif), Some(skel_nif)) = (
            self.anim_player.as_mut(),
            self.anim_kf_nif.as_ref(),
            self.anim_skeleton_nif.as_ref(),
        ) {
            // 1. アニメーション時刻を進めてボーン姿勢を更新
            player.update(kf_nif, dt, &mut self.anim_pose);

            // 2. スケルトン NIF の FK を再計算（アニメーション姿勢適用後のボーンワールド行列 & ボーン名マップ）
            fo3_render::recompute_bone_world_maps_with_pose(
                skel_nif,
                &self.anim_pose,
                &mut self.anim_bone_world_map,
                &mut self.anim_bone_name_world_map,
            );

            // 3. 全パーツスキンメッシュの頂点バッファをスケルトンのボーン名ワールド行列で更新
            let part_refs: Vec<&NifFile> = self.anim_parts.iter().collect();
            self.scene
                .update_animated_skins_multi_parts(&self.device, &part_refs, &self.anim_bone_name_world_map);

            // 4. 全剛体アタッチメントパーツ (目・歯・舌など) のモデル行列をボーン追従更新
            self.scene
                .update_animated_rigid_meshes(&self.queue, &self.anim_bone_name_world_map);
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
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
                Some(self.camera.eye_position()),
            );

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
            WindowEvent::MouseInput {
                button,
                state: btn_state,
                ..
            } => {
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

                    match state.camera_mode {
                        CameraMode::Orbit => {
                            if state.left_mouse_down {
                                state.camera.rotate(dx, dy);
                                state.window.request_redraw();
                            } else if state.right_mouse_down {
                                state.camera.pan(dx, dy);
                                state.window.request_redraw();
                            }
                        }
                        CameraMode::Walkthrough => {
                            // FPS 視点回転 (左ドラッグまたは右ドラッグで視線回転)
                            if state.left_mouse_down || state.right_mouse_down {
                                state.camera.rotate(dx, dy);
                                state.window.request_redraw();
                            }
                        }
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
                let pressed = event.state == ElementState::Pressed;
                if let PhysicalKey::Code(key) = event.physical_key {
                    // WASD / Space の移動キー追跡
                    match key {
                        KeyCode::KeyW => state.key_forward = pressed,
                        KeyCode::KeyS => state.key_backward = pressed,
                        KeyCode::KeyA => state.key_left = pressed,
                        KeyCode::KeyD => state.key_right = pressed,
                        KeyCode::Space => state.key_jump = pressed,
                        _ => {}
                    }

                    if pressed {
                        match key {
                            KeyCode::Tab | KeyCode::KeyM => {
                                state.camera_mode = match state.camera_mode {
                                    CameraMode::Orbit => {
                                        println!("\n[カメラモード] FPS ウォークスルー歩行モード (物理演算 & KCC 有効) に切り替えました。");
                                        println!("  WASD: 移動, Space: ジャンプ, マウスドラッグ: 視線変更, Tab/M: オービット復帰");
                                        // ロード時に決定した出入口ドア等の安全な初期スポーン地点に配置
                                        state.character_controller.position =
                                            state.initial_spawn_point;
                                        state.vertical_velocity = 0.0;
                                        CameraMode::Walkthrough
                                    }
                                    CameraMode::Walkthrough => {
                                        println!("\n[カメラモード] オービットカメラ (全体周回) に切り替えました。");
                                        // キャラクタの現在位置にカメラの注視点を合わせる
                                        state.camera.target = state.character_controller.position;
                                        CameraMode::Orbit
                                    }
                                };
                                state.window.request_redraw();
                            }
                            KeyCode::KeyR => {
                                state
                                    .camera
                                    .focus(state.scene.bounds_center, state.scene.bounds_radius);
                                state.character_controller.position =
                                    state.scene.bounds_center + glam::Vec3::new(0.0, 0.0, 64.0);
                                state.vertical_velocity = 0.0;
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("使用法:");
        println!("  - メッシュ単体表示: cargo run -p fo3_viewer -- <DataDir> <RelativeNifPath>");
        println!("  - セル一括表示:     cargo run -p fo3_viewer -- cell <DataDir> <CellEDID>");
        println!("  - ワールド表示:     cargo run -p fo3_viewer -- world <DataDir> <WorldEDID> [GridX] [GridY]");
        println!("  - アニメーション:   cargo run -p fo3_viewer -- anim <DataDir> <RelativeNifPath> <RelativeKfPath>");
        println!("例:");
        println!("  cargo run -p fo3_viewer -- \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"meshes\\weapons\\1handpistol\\10mmpistol.nif\"");
        println!("  cargo run -p fo3_viewer -- cell \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"Vault101a\"");
        println!("  cargo run -p fo3_viewer -- cell \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"Springvale\"");
        println!("  cargo run -p fo3_viewer -- world \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"MegatonWorld\"");
        println!("  cargo run -p fo3_viewer -- world \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"DCWorld01\"");
        println!("  cargo run -p fo3_viewer -- world \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"Wasteland\" -1 2");
        println!("  cargo run -p fo3_viewer -- anim \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"meshes\\characters\\_male\\upperbody.nif\" \"meshes\\characters\\_male\\idleanims\\ttnpchappysubtlelistena.kf\"");
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
    } else if args[1] == "anim" {
        // anim <DataDir> <NifPath> <KfPath>
        if args.len() < 5 {
            eprintln!("エラー: アニメーションモードには <DataDir> <NifPath> <KfPath> が必要です。");
            eprintln!("例: cargo run -p fo3_viewer -- anim \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"meshes\\characters\\_male\\upperbody.nif\" \"meshes\\characters\\_male\\idleanims\\ttnpchappysubtlelistena.kf\"");
            return Ok(());
        }
        (
            args[2].clone(),
            ViewerTarget::Anim {
                nif_path: args[3].clone(),
                kf_path: args[4].clone(),
            },
        )
    } else if args[1] == "actor" {
        // actor <DataDir> [naked | outfit_path] <KfPath>
        if args.len() < 5 {
            eprintln!("エラー: アクターモードには <DataDir> [naked | outfit_path] <KfPath> が必要です。");
            eprintln!("例 (素体): cargo run -p fo3_viewer -- actor \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" naked \"meshes\\characters\\_male\\idleanims\\ttnpchappysubtlelistena.kf\"");
            eprintln!("例 (防具): cargo run -p fo3_viewer -- actor \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"meshes\\armor\\wastelandclothing01\\outfitm.nif\" \"meshes\\characters\\_male\\idleanims\\ttnpchappysubtlelistena.kf\"");
            return Ok(());
        }
        (
            args[2].clone(),
            ViewerTarget::Actor {
                outfit_or_naked: args[3].clone(),
                kf_path: args[4].clone(),
            },
        )
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

/// 人型アクターのパーツ NIF 相対パス一覧を取得する。
///
/// 頭部、目 (左右)、歯 (上下)、舌、胴体/衣装、手 (左右) を過不足なく構成する。
/// 参照元: Gamebryo 2.6 キャラクタパーツ合成, `knowledge/actor_and_skin_mesh.md` (セクション 4.6, 4.7)
fn get_actor_part_paths(is_female: bool, body_path: &str) -> Vec<String> {
    let (right_hand, left_hand) = if is_female {
        ("meshes\\characters\\_female\\righthand.nif", "meshes\\characters\\_female\\lefthand.nif")
    } else {
        ("meshes\\characters\\_male\\righthand.nif", "meshes\\characters\\_male\\lefthand.nif")
    };
    vec![
        "meshes\\characters\\head\\headhuman.nif".to_string(),
        "meshes\\characters\\head\\eyelefthuman.nif".to_string(),
        "meshes\\characters\\head\\eyerighthuman.nif".to_string(),
        "meshes\\characters\\head\\teethupperhuman.nif".to_string(),
        "meshes\\characters\\head\\teethlowerhuman.nif".to_string(),
        "meshes\\characters\\head\\tonguehuman.nif".to_string(),
        body_path.to_string(),
        right_hand.to_string(),
        left_hand.to_string(),
    ]
}
