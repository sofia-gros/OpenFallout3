//! セル群（Interior / Exterior / Grids）の探索・静的メッシュ配置・光源収集モジュール。
//!
//! 参照元: Gamebryo 2.6 セルグラフ構築, `references/openmw/components/esm4/loadcell.hpp`

use std::collections::HashMap;
use std::sync::Arc;

use fo3_esm::{CellLighting, EsmMasterContext, LandRecord};
use fo3_gamebryo_core::NiTransform;
use fo3_nif::NifFile;
use fo3_physics::RapierPhysicsWorld;
use fo3_render::{GpuTexture, NifCache, PlacedPointLight, RenderContext, RenderScene};
use fo3_vfs::VfsManager;

use crate::interact::{InteractableKind, InteractableObject};
use crate::types::{is_editor_marker_or_effect, ViewerTarget};
use super::actor::{build_scene_actors, try_resolve_actor, CellNpcSpawn};
use super::finder::find_cells_for_target;
use super::interactable::try_resolve_interactable;
use super::physics::build_cell_physics;
use super::LoadedSceneResult;

/// セルおよび近傍セル・ワールドスペースの配置オブジェクトを一括ロードする。
pub fn load_cell_targets(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    context: &RenderContext,
    data_dir: &str,
    target: &ViewerTarget,
    vfs: &mut VfsManager,
    master_context: &EsmMasterContext,
    nif_cache: &mut NifCache,
    texture_cache: &mut HashMap<String, GpuTexture>,
) -> LoadedSceneResult {
    let (mut esm_reader, cells) = find_cells_for_target(data_dir, target);

    let model_map = &master_context.model_map;
    let light_map = &master_context.light_map;

    let mut all_cell_items: Vec<
        Vec<(
            u32,
            u32,
            Arc<NifFile>,
            NiTransform,
            fo3_esm::types::FourCC,
            String,
        )>,
    > = Vec::new();
    let mut placed_lights: Vec<PlacedPointLight> = Vec::new();
    let mut interactables: Vec<InteractableObject> = Vec::new();
    let mut cell_npcs: Vec<CellNpcSpawn> = Vec::new();
    let mut markers: HashMap<String, (glam::Vec3, glam::Vec3)> = HashMap::new();
    let mut primary_lighting: Option<CellLighting> = None;
    let mut door_spawn_point: Option<(glam::Vec3, f32)> = None;

    for (cell, refrs, _) in &cells {
        if primary_lighting.is_none() && cell.lighting.is_some() {
            primary_lighting = cell.lighting.clone();
        }

        // 出入口ドア (XTEL) またはプレイヤー出現ポイントの検索
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

        let mut cell_items: Vec<(
            u32,
            u32,
            Arc<NifFile>,
            NiTransform,
            fo3_esm::types::FourCC,
            String,
        )> = Vec::new();

        for refr in refrs {
            let pos = glam::Vec3::new(refr.position[0], refr.position[1], refr.position[2]);
            let rot = glam::Vec3::new(refr.rotation[0], refr.rotation[1], refr.rotation[2]);
            let world_transform = NiTransform::from_euler_xyz(pos, rot, refr.scale);

            // マーカー (EDID 付き参照) の座標・回転をデータ駆動マップへ登録
            if !refr.edid.is_empty() {
                markers.insert(refr.edid.to_ascii_lowercase(), (pos, rot));
            }

            // アクター (ACHR / NPC_) の解決
            if let Some(npc) = try_resolve_actor(refr, master_context, world_transform) {
                interactables.push(InteractableObject {
                    form_id: npc.form_id,
                    base_id: npc.base_form_id,
                    edid: refr.edid.clone(),
                    name: npc.name.clone(),
                    position: npc.transform.translation,
                    height: 125.0,
                    radius: 45.0,
                    kind: InteractableKind::Actor {
                        form_id: npc.form_id,
                        base_form_id: npc.base_form_id,
                        is_dead: false,
                    },
                });
                cell_npcs.push(npc);
                continue;
            }

            // 配置点光源の収集 (LIGHT レコード)
            if let Some(light_rec) = light_map.get(&refr.base_object) {
                let pos =
                    glam::Vec3::new(refr.position[0], refr.position[1], refr.position[2]);
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

            // 3D モデルパスの決定 (通常オブジェクト)
            let mesh_file_path = if let Some(obj_info) = model_map.get(&refr.base_object) {
                if obj_info.model.is_empty() || is_editor_marker_or_effect(&obj_info.edid, &obj_info.model) {
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
                        eprintln!(
                            "警告: メッシュ \"{}\" のロード失敗（スキップ）: {}",
                            model_path, e
                        );
                        continue;
                    }
                };

                let record_type = model_map
                    .get(&refr.base_object)
                    .map(|o| o.record_type)
                    .unwrap_or(fo3_esm::types::FourCC(*b"    "));
                let model_path = model_map
                    .get(&refr.base_object)
                    .map(|o| o.model.clone())
                    .unwrap_or_default();
                cell_items.push((
                    refr.form_id.0,
                    refr.base_object.0,
                    nif,
                    world_transform,
                    record_type,
                    model_path,
                ));

                // インタラクト可能オブジェクト (アクティベーターやドアなど)
                if let Some(obj_info) = model_map.get(&refr.base_object) {
                    if let Some(interactable) = try_resolve_interactable(refr, obj_info, pos) {
                        interactables.push(interactable);
                    }
                }
            }
        }
        all_cell_items.push(cell_items);
    }

    let mut animated_statics: Vec<(
        u32,
        u32,
        Arc<NifFile>,
        NiTransform,
        Option<Arc<fo3_render::animation::AnimationClip>>,
        String,
    )> = Vec::new();

    let cell_inputs: Vec<(
        u32,
        Vec<(&NifFile, NiTransform)>,
        Option<(&LandRecord, i32, i32)>,
    )> = cells
        .iter()
        .zip(all_cell_items.iter())
        .map(|((cell, _, land), items)| {
            let mut placed_refs = Vec::new();
            for (form_id, base_id, n, t, record_type, model_path) in items {
                if let Some(clip) =
                    fo3_render::animation::AnimationClip::from_transform_controllers(n.as_ref())
                {
                    animated_statics.push((
                        *form_id,
                        *base_id,
                        n.clone(),
                        *t,
                        Some(Arc::new(clip)),
                        model_path.clone(),
                    ));
                } else {
                    placed_refs.push((n.as_ref(), *t));
                }
            }
            let land_info = land
                .as_ref()
                .and_then(|l| cell.grid.map(|g| (l, g.0, g.1)));
            (cell.form_id.0, placed_refs, land_info)
        })
        .collect();

    let cell_refs: Vec<(
        u32,
        &[(&NifFile, NiTransform)],
        Option<(&LandRecord, i32, i32)>,
    )> = cell_inputs
        .iter()
        .map(|(cell_id, refs, land_info)| (*cell_id, refs.as_slice(), *land_info))
        .collect();

    let landscape_texture_map = esm_reader.read_landscape_texture_map().ok();
    if let Some(ref tex_map) = landscape_texture_map {
        println!("地形テクスチャセット解決: {} 件", tex_map.len());
    }

    println!("[DEBUG:LOADER] RenderScene::from_cells 開始 (cell_refs count: {})", cell_refs.len());
    let mut scene = RenderScene::from_cells(
        device,
        queue,
        context,
        &cell_refs,
        landscape_texture_map.as_ref(),
        vfs,
        texture_cache,
    );
    println!("[DEBUG:LOADER] RenderScene::from_cells 完了 (meshes count: {})", scene.meshes.len());

    println!("[DEBUG:LOADER] animated_statics 適用 (count: {})", animated_statics.len());
    for (form_id, base_id, nif, transform, clip, model_path) in animated_statics {
        scene.add_animated_static(
            device,
            queue,
            context,
            vfs,
            form_id,
            base_id,
            &model_path,
            &transform,
            nif,
            clip,
            texture_cache,
        );
    }

    if !cell_npcs.is_empty() {
        println!("[DEBUG:LOADER] build_scene_actors 開始 (cell_npcs count: {})", cell_npcs.len());
        build_scene_actors(
            device,
            queue,
            context,
            &mut scene,
            &cell_npcs,
            vfs,
            master_context,
            nif_cache,
        );
        println!("[DEBUG:LOADER] build_scene_actors 完了");
    }

    let mut physics_world = RapierPhysicsWorld::new();
    let cell_form_ids: Vec<u32> = cells.iter().map(|c| c.0.form_id.0).collect();
    let land_inputs: Vec<(u32, Option<(&LandRecord, i32, i32)>)> = cell_inputs
        .iter()
        .map(|(cell_id, _, land_info)| (*cell_id, *land_info))
        .collect();
    let (refr_bindings, total_colliders) = build_cell_physics(
        &mut physics_world,
        &land_inputs,
        &scene,
        &cell_form_ids,
        &all_cell_items,
    );
    println!("物理ワールド構築完了: 登録剛体数 {}", total_colliders);

    let clear_color = if let Some(ref cl) = primary_lighting {
        if cl.fog_color != [0, 0, 0, 0] {
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

    println!(
        "GPU シーン構築完了: {} メッシュノード描画準備完了",
        scene.meshes.len()
    );

    LoadedSceneResult {
        scene,
        cell_lighting: primary_lighting,
        placed_lights,
        clear_color,
        physics_world,
        door_spawn_point,
        interactables,
        refr_bindings,
        markers,
    }
}
