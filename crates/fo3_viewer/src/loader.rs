//! シーン・アクター・コリジョン・ライティングのロードおよび GPU 初期化モジュール。
//!
//! 参照元: Gamebryo 2.6 セルグラフ構築, `knowledge/actor_and_skin_mesh.md`

use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use fo3_esm::{CellLighting, CellRecord, EsmReader, LandRecord, RefrRecord};
use fo3_gamebryo_core::NiTransform;
use fo3_nif::collision::extract_collision_data;
use fo3_nif::NifFile;
use fo3_esm::EsmMasterContext;
use fo3_physics::RapierPhysicsWorld;
use fo3_render::{GpuTexture, NifCache, PlacedPointLight, RenderContext, RenderScene};
use fo3_vfs::VfsManager;

use crate::interactive_anim::{MovingPartBinding, RefrBinding};
use crate::interact::{InteractableKind, InteractableObject};
use crate::types::{
    get_actor_part_paths, is_editor_marker_or_effect, is_ghoul_race, resolve_candidate_items,
    ViewerTarget,
};

/// ロード結果をまとめた構造体。
pub struct LoadedSceneResult {
    pub scene: RenderScene,
    pub cell_lighting: Option<CellLighting>,
    pub placed_lights: Vec<PlacedPointLight>,
    pub clear_color: wgpu::Color,
    pub physics_world: RapierPhysicsWorld,
    pub door_spawn_point: Option<(glam::Vec3, f32)>,
    pub interactables: Vec<InteractableObject>,
    pub refr_bindings: HashMap<u32, RefrBinding>,
}

/// 指定ターゲットのシーン、光源、物理ワールドを一括ロード・構築する。
///
/// 参照元: Gamebryo 2.6 `NiStream`, `NiSourceTexture`, `knowledge/gamebryo_resource_management_and_caching.md`
pub fn load_scene(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    context: &RenderContext,
    data_dir: &str,
    target: &ViewerTarget,
    mut vfs: &mut VfsManager,
    master_context: &EsmMasterContext,
    nif_cache: &mut NifCache,
    texture_cache: &mut HashMap<String, GpuTexture>,
) -> LoadedSceneResult {

    let (
        scene,
        cell_lighting,
        placed_lights,
        clear_color,
        physics_world,
        door_spawn_point,
        interactables,
        refr_bindings,
    ) = match target {
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
                        RenderScene::from_nif(&device, &queue, &context, &nif_file, &mut *vfs);

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
                        Vec::new(),
                        HashMap::new(),
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
                    let part_paths = get_actor_part_paths(is_female, fo3_esm::FormId(0), &body_path, None, None, None, None, false);

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
                        &device, &queue, &context, &skel_nif, &part_refs, &mut *vfs,
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
                        Vec::new(),
                        HashMap::new(),
                    )
                }
                ViewerTarget::Cell(..) | ViewerTarget::World(..) => {
                    let esm_path = Path::new(data_dir).join("Fallout3.esm");
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

                    let model_map = &master_context.model_map;
                    let npc_map = &master_context.npc_map;
                    let armor_map = &master_context.armor_map;
                    let outfit_map = &master_context.outfit_map;
                    let hair_map = &master_context.hair_map;
                    let lvli_map = &master_context.lvli_map;
                    let light_map = &master_context.light_map;

                    struct CellNpcSpawn {
                        form_id: u32,
                        name: String,
                        transform: NiTransform,
                        is_female: bool,
                        race: fo3_esm::FormId,
                        outfit_model: Option<String>,
                        head_gear_model: Option<String>,
                        hand_gear_model: Option<String>,
                        weapon_model: Option<String>,
                        hair_model: Option<String>,
                        hair_color: Option<[u8; 3]>,
                        has_hat: bool,
                        hide_hair: bool,
                        facegen_texture_symmetric: Option<Vec<f32>>,
                        facegen_geometry_symmetric: Option<Vec<f32>>,
                        facegen_geometry_asymmetric: Option<Vec<f32>>,
                    }
                    let mut cell_npcs: Vec<CellNpcSpawn> = Vec::new();
                    let mut all_cell_items: Vec<Vec<(u32, Arc<NifFile>, NiTransform)>> = Vec::new();
                    let mut placed_lights: Vec<PlacedPointLight> = Vec::new();
                    let mut interactables: Vec<InteractableObject> = Vec::new();
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

                        let mut cell_items: Vec<(u32, Arc<NifFile>, NiTransform)> = Vec::new();
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

                                // 1. 全防具・武器の装備スロット解決 (DOFT -> WNAM -> CNTO 所持品 -> LVLI 再帰展開)
                                // 参照元: references/openmw/components/esm4/loadnpc.cpp:60, loadlvli.cpp:57, loadarmo.hpp:L65-85
                                let mut raw_items = Vec::new();
                                if let Some(doft_id) = npc.default_outfit {
                                    if let Some(otft) = outfit_map.get(&doft_id) {
                                        raw_items.extend(otft.inventory.iter().copied());
                                    } else {
                                        raw_items.push(doft_id);
                                    }
                                }
                                if let Some(wnam) = npc.default_armor {
                                    raw_items.push(wnam);
                                }
                                for inv in &npc.inventory {
                                    raw_items.push(inv.item);
                                }
                                let candidate_items = resolve_candidate_items(&raw_items, &lvli_map);

                                let mut outfit_model = None;
                                let mut head_gear_model = None;
                                let mut hand_gear_model = None;
                                let mut weapon_model = None;
                                let mut has_hat = false;
                                let mut hide_hair = false;

                                for item_id in candidate_items {
                                    if let Some(armo) = armor_map.get(&item_id) {
                                        let model_path = armo.model_for_gender(npc.is_female);
                                        if armo.is_head() && head_gear_model.is_none() {
                                            if let Some(m) = model_path {
                                                head_gear_model = Some(m.to_string());
                                                if armo.shows_hat() {
                                                    has_hat = true;
                                                }
                                                if armo.hides_hair() {
                                                    hide_hair = true;
                                                }
                                            }
                                        } else if armo.is_upper_body() && outfit_model.is_none() {
                                            if let Some(m) = model_path {
                                                outfit_model = Some(m.to_string());
                                            }
                                        } else if armo.is_hands() && hand_gear_model.is_none() {
                                            if let Some(m) = model_path {
                                                hand_gear_model = Some(m.to_string());
                                            }
                                        }
                                    } else if weapon_model.is_none() {
                                        if let Some(base_info) = model_map.get(&item_id) {
                                            let m_lower = base_info.model.to_ascii_lowercase();
                                            if !base_info.model.is_empty()
                                                && (base_info.record_type == fo3_esm::types::REC_WEAP
                                                    || m_lower.contains("weapons")
                                                    || m_lower.contains("weapon"))
                                            {
                                                weapon_model = Some(base_info.model.clone());
                                            }
                                        }
                                    }
                                }

                                // 胴体衣装が未解決の場合のフォールバック
                                let outfit_model = outfit_model.or_else(|| {
                                    if npc.is_female {
                                        Some("Armor\\WastelandClothing01\\OutfitF.NIF".to_string())
                                    } else {
                                        Some("Armor\\WastelandClothing01\\OutfitM.NIF".to_string())
                                    }
                                });

                                // 2. 髪型モデルの解決 (HNAM -> HAIR -> MODL -> デフォルト髪型フォールバック)
                                let hair_model = if hide_hair {
                                    None
                                } else {
                                    npc.hair
                                        .and_then(|hair_id| hair_map.get(&hair_id))
                                        .and_then(|hair| {
                                            if !hair.model.is_empty() {
                                                Some(hair.model.clone())
                                            } else {
                                                None
                                            }
                                        })
                                        .or_else(|| {
                                            // HNAM 未指定時の性別別デフォルト髪型フォールバック
                                            if npc.is_female {
                                                Some("Characters\\Hair\\HairBun.NIF".to_string())
                                            } else {
                                                Some("Characters\\Hair\\HairMessy03.NIF".to_string())
                                            }
                                        })
                                };

                                // 髪色フォールバック: HCLR が未定義の NPC は自然なダークブラウン系 [65, 45, 30] を適用
                                let hair_color = npc.hair_color.or(Some([65, 45, 30]));

                                let name = npc.full_name.clone().unwrap_or_else(|| npc.edid.clone());
                                cell_npcs.push(CellNpcSpawn {
                                    form_id: refr.form_id.0,
                                    name,
                                    transform: world_transform,
                                    is_female: npc.is_female,
                                    race: npc.race,
                                    outfit_model,
                                    head_gear_model,
                                    hand_gear_model,
                                    weapon_model,
                                    hair_model,
                                    hair_color,
                                    has_hat,
                                    hide_hair,
                                    facegen_texture_symmetric: npc.facegen_texture_symmetric.clone(),
                                    facegen_geometry_symmetric: npc.facegen_geometry_symmetric.clone(),
                                    facegen_geometry_asymmetric: npc.facegen_geometry_asymmetric.clone(),
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
                                let nif = match nif_cache.get_or_load(&model_path, vfs) {
                                    Ok(n) => n,
                                    Err(e) => {
                                        eprintln!("警告: メッシュ \"{}\" のロード失敗（スキップ）: {}", model_path, e);
                                        continue;
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
                                cell_items.push((refr.form_id.0, nif, world_transform));

                                // インタラクト対象の判定 (ドア、コンテナ、アイテム、アクティベーター)
                                // 参照元: references/openmw/components/esm4/loadrefr.hpp, Gamebryo 2.6 NiPick
                                if let Some(obj_info) = model_map.get(&refr.base_object) {
                                    let kind_opt = if refr.teleport.is_some()
                                        || obj_info.record_type == fo3_esm::types::REC_DOOR
                                    {
                                        Some((
                                            InteractableKind::Door {
                                                teleport: refr.teleport.clone(),
                                                lock: refr.lock,
                                                is_open: false,
                                            },
                                            140.0,
                                            50.0,
                                        ))
                                    } else if obj_info.record_type == fo3_esm::types::REC_CONT {
                                        Some((
                                            InteractableKind::Container {
                                                form_id: refr.base_object.0,
                                                lock: refr.lock,
                                                is_open: false,
                                            },
                                            60.0,
                                            45.0,
                                        ))
                                    } else if obj_info.record_type == fo3_esm::types::REC_ALCH
                                        || obj_info.record_type == fo3_esm::types::REC_WEAP
                                        || obj_info.record_type == fo3_esm::types::REC_ARMO
                                        || obj_info.record_type == fo3_esm::types::REC_BOOK
                                        || obj_info.record_type == fo3_esm::types::REC_MISC
                                    {
                                        Some((
                                            InteractableKind::Item {
                                                form_id: refr.base_object.0,
                                            },
                                            20.0,
                                            35.0,
                                        ))
                                    } else if obj_info.record_type == fo3_esm::types::REC_TERM {
                                        Some((
                                            InteractableKind::Terminal {
                                                form_id: refr.base_object.0,
                                                lock: refr.lock,
                                            },
                                            90.0,
                                            45.0,
                                        ))
                                    } else if obj_info.record_type == fo3_esm::types::REC_ACTI {
                                        Some((
                                            InteractableKind::Activator {
                                                form_id: refr.base_object.0,
                                            },
                                            80.0,
                                            45.0,
                                        ))
                                    } else {
                                        None
                                    };

                                    if let Some((kind, height, radius)) = kind_opt {
                                        let name = if !refr.edid.is_empty() {
                                            refr.edid.clone()
                                        } else {
                                            obj_info.edid.clone()
                                        };
                                        interactables.push(InteractableObject {
                                            form_id: refr.form_id.0,
                                            edid: refr.edid.clone(),
                                            name,
                                            position: pos,
                                            height,
                                            radius,
                                            kind,
                                        });
                                    }
                                }
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
                                items.iter().map(|(_, n, t)| (n.as_ref(), *t)).collect();
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
                        device,
                        queue,
                        context,
                        &cell_refs,
                        landscape_texture_map.as_ref(),
                        vfs,
                        texture_cache,
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

                        // FaceGen EGM (幾何形状モーフ) のキャッシュ
                        let egm_male_path = "meshes\\characters\\head\\headmale.egm";
                        let egm_female_path = "meshes\\characters\\head\\headfemale.egm";
                        let egm_male = if let Ok(bytes) = vfs.read(egm_male_path) {
                            fo3_render::parse_geometry_morph(&bytes).ok()
                        } else {
                            None
                        };
                        let egm_female = if let Ok(bytes) = vfs.read(egm_female_path) {
                            fo3_render::parse_geometry_morph(&bytes).ok()
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

                            let part_paths = get_actor_part_paths(
                                npc.is_female,
                                npc.race,
                                &body_path,
                                npc.head_gear_model.as_deref(),
                                npc.hand_gear_model.as_deref(),
                                npc.weapon_model.as_deref(),
                                npc.hair_model.as_deref(),
                                npc.hide_hair,
                            );
                            let mut parts = Vec::new();
                            for path in &part_paths {
                                let nif = nif_cache.get_or_load(path, vfs).ok();
                                if let Some(p) = nif {
                                    parts.push(p);
                                }
                            }

                            // FaceGen EGT テクスチャ合成 (化粧・肌色差分)
                            // 参照元: `references/bevyout/src/vsa/prepare/facegen.rs`, `knowledge/actor_and_skin_mesh.md`
                            let head_diffuse_override = if let Some(ref fgts) = npc.facegen_texture_symmetric {
                                let egt_path = if npc.is_female {
                                    "meshes\\characters\\head\\headfemale.egt"
                                } else {
                                    "meshes\\characters\\head\\headmale.egt"
                                };
                                let default_head_dds = if is_ghoul_race(npc.race) {
                                    "textures\\characters\\head\\headghoul.dds"
                                } else if npc.is_female {
                                    "textures\\characters\\head\\headhumanfemale.dds"
                                } else {
                                    "textures\\characters\\head\\headhuman.dds"
                                };
                                let dds_path = if vfs.exists(default_head_dds) {
                                    default_head_dds
                                } else {
                                    "textures\\characters\\head\\headhuman.dds"
                                };

                                let synthesized_tex = if let (Ok(egt_bytes), Ok(dds_bytes)) = (vfs.read(egt_path), vfs.read(dds_path)) {
                                    if let (Ok(morph), Ok((w, h, base_rgba))) = (
                                        fo3_render::parse_texture_morph(&egt_bytes),
                                        fo3_render::decode_dds_to_rgba8(&dds_bytes),
                                    ) {
                                        let final_rgba = fo3_render::synthesize_head_diffuse(
                                            base_rgba,
                                            w,
                                            h,
                                            &morph,
                                            fgts,
                                        );
                                        fo3_render::GpuTexture::from_rgba8(
                                            &device,
                                            &queue,
                                            w,
                                            h,
                                            &final_rgba,
                                            Some(&format!("FaceGen_{:08X}", npc.form_id)),
                                        ).ok()
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };

                                synthesized_tex
                            } else {
                                None
                            };

                            let override_ref = head_diffuse_override.as_ref();

                            let egm_ref = if npc.is_female {
                                egm_female.as_ref().or(egm_male.as_ref())
                            } else {
                                egm_male.as_ref().or(egm_female.as_ref())
                            };
                            let fg_sym_ref = npc.facegen_geometry_symmetric.as_deref();
                            let fg_asym_ref = npc.facegen_geometry_asymmetric.as_deref();

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
                                npc.hair_color,
                                npc.has_hat,
                                override_ref,
                                egm_ref,
                                fg_sym_ref,
                                fg_asym_ref,
                            );
                            interactables.push(InteractableObject {
                                form_id: npc.form_id,
                                edid: String::new(),
                                name: npc.name.clone(),
                                position: npc.transform.translation,
                                height: 125.0,
                                radius: 45.0,
                                kind: InteractableKind::Actor {
                                    form_id: npc.form_id,
                                    is_dead: false,
                                },
                            });
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

                    // 2. 配置オブジェクト (REFR) コライダーの登録および RefrBinding 構築
                    let mut refr_bindings = HashMap::new();
                    let mut flat_idx = 0;
                    for (form_id, nif, world_transform) in
                        all_cell_items.iter().flat_map(|items| items.iter())
                    {
                        let col_data = extract_collision_data(nif);
                        let mut rigid_bodies = Vec::new();
                        if !col_data.bodies.is_empty() {
                            total_colliders += col_data.bodies.len();
                            let quat = glam::Quat::from_mat3(&world_transform.rotation);
                            let handles = physics_world.add_nif_collision_with_user_data(
                                &col_data,
                                world_transform.translation,
                                quat,
                                *form_id as u128,
                            );
                            rigid_bodies = handles.into_iter().map(|(rb, _)| rb).collect();
                        }
                        let mesh_range = scene.refr_mesh_ranges.get(flat_idx).cloned().unwrap_or(0..0);
                        flat_idx += 1;
                        let all_meshes: Vec<usize> = (mesh_range.start..mesh_range.end).collect();

                        let open_clip = fo3_render::animation::AnimationClip::from_nif_sequence(nif, "Open");
                        let close_clip = fo3_render::animation::AnimationClip::from_nif_sequence(nif, "Close");

                        let mut static_mesh_indices = Vec::new();
                        let mut static_rigid_bodies = Vec::new();
                        let mut moving_parts = Vec::new();

                        if let Some(ref clip) = open_clip {
                            // NIF にシーケンスがある場合 (Vault ドア、スライディングドア、Shack ドア等)
                            for (channel_name, _) in &clip.channels {
                                moving_parts.push(MovingPartBinding {
                                    mesh_indices: all_meshes.clone(),
                                    rigid_bodies: rigid_bodies.clone(),
                                    node_name: channel_name.clone(),
                                    local_bind_matrix: glam::Mat4::IDENTITY,
                                    pivot_offset: glam::Vec3::ZERO,
                                    rotation_axis: glam::Vec3::Z,
                                    max_angle: -std::f32::consts::PI * 0.5,
                                });
                            }
                        } else if all_meshes.len() > 1 {
                            // シーケンスがなく複数メッシュが存在する場合 (例: 金属箱 MetalBox01)
                            // 最初のメッシュを固定本体、2番目以降を可動蓋とする
                            static_mesh_indices.push(all_meshes[0]);
                            if !rigid_bodies.is_empty() {
                                static_rigid_bodies.push(rigid_bodies[0]);
                            }
                            let moving_meshes = all_meshes[1..].to_vec();
                            let moving_rb = if rigid_bodies.len() > 1 {
                                rigid_bodies[1..].to_vec()
                            } else {
                                Vec::new()
                            };
                            moving_parts.push(MovingPartBinding {
                                mesh_indices: moving_meshes,
                                rigid_bodies: moving_rb,
                                node_name: "Lid".to_string(),
                                local_bind_matrix: glam::Mat4::IDENTITY,
                                pivot_offset: glam::Vec3::new(0.0, -15.0, 10.0),
                                rotation_axis: glam::Vec3::X,
                                max_angle: std::f32::consts::PI * 0.45,
                            });
                        } else {
                            // 単一メッシュのドア等: 全体をヒンジ回転パーツとする
                            moving_parts.push(MovingPartBinding {
                                mesh_indices: all_meshes,
                                rigid_bodies,
                                node_name: "Door".to_string(),
                                local_bind_matrix: glam::Mat4::IDENTITY,
                                pivot_offset: glam::Vec3::ZERO,
                                rotation_axis: glam::Vec3::Z,
                                max_angle: -std::f32::consts::PI * 0.5,
                            });
                        }

                        refr_bindings.insert(
                            *form_id,
                            RefrBinding {
                                form_id: *form_id,
                                static_mesh_indices,
                                static_rigid_bodies,
                                moving_parts,
                                base_translation: world_transform.translation,
                                base_rotation: glam::Quat::from_mat3(&world_transform.rotation),
                                open_clip,
                                close_clip,
                                nif: Some(nif.clone()),
                            },
                        );
                    }
                    println!("物理ワールド構築完了: 登録剛体数 {}", total_colliders);

                    (
                        scene,
                        primary_lighting,
                        placed_lights,
                        clear_color,
                        physics_world,
                        door_spawn_point,
                        interactables,
                        refr_bindings,
                    )
                }
            };

    println!("GPU シーン構築完了: {} メッシュノード描画準備完了", scene.meshes.len());
    LoadedSceneResult {
        scene,
        cell_lighting,
        placed_lights,
        clear_color,
        physics_world,
        door_spawn_point,
        interactables,
        refr_bindings,
    }
}
