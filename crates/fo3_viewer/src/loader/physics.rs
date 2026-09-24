//! セル内オブジェクト・地形の物理コライダー登録およびアニメーションパーツバインディング。
//!
//! 参照元: Gamebryo 2.6 `bhkRigidBody` (MO_SYS_KEYFRAMED) & Rapier 物理統合

use std::collections::HashMap;
use std::sync::Arc;
use fo3_esm::LandRecord;
use fo3_gamebryo_core::NiTransform;
use fo3_nif::collision::extract_collision_data;
use fo3_nif::NifFile;
use fo3_physics::RapierPhysicsWorld;
use fo3_render::RenderScene;

use crate::interactive_anim::{MovingPartBinding, RefrBinding};

/// 地形 (LAND) コライダーおよび配置オブジェクト (REFR) の物理剛体を登録し、
/// 可動パーツ（ドア・蓋など）の `RefrBinding` を構築する。
pub fn build_cell_physics(
    physics_world: &mut RapierPhysicsWorld,
    land_inputs: &[(u32, Option<(&LandRecord, i32, i32)>)],
    scene: &RenderScene,
    cells_form_ids: &[u32],
    all_cell_items: &[Vec<(u32, u32, Arc<NifFile>, NiTransform, fo3_esm::types::FourCC, String)>],
) -> (HashMap<u32, RefrBinding>, usize) {
    let mut total_colliders = 0;

    // 1. 地形 (LAND) コライダーの登録
    for (cell_id, land_info) in land_inputs {
        if let Some((land, gx, gy)) = land_info {
            let heights = land.compute_heights();
            if let Some((_, col_handle)) =
                physics_world.add_land_collision(&heights, *gx, *gy)
            {
                total_colliders += 1;
                physics_world
                    .cell_colliders
                    .entry(*cell_id)
                    .or_default()
                    .push(col_handle);
            }
        }
    }

    // 2. 配置オブジェクト (REFR) コライダーの登録および RefrBinding 構築
    let mut refr_bindings = HashMap::new();
    let mut flat_idx = 0;
    for (cell_id, cell_items) in cells_form_ids.iter().zip(all_cell_items.iter()) {
        for (form_id, _base_id, nif, world_transform, _record_type, _model_path) in cell_items {
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
                let col_handles: Vec<_> = handles.iter().map(|(_, c)| *c).collect();
                physics_world
                    .cell_colliders
                    .entry(*cell_id)
                    .or_default()
                    .extend(col_handles);
                rigid_bodies = handles.into_iter().map(|(rb, _)| rb).collect();
            }
            let mesh_range = scene
                .refr_mesh_ranges
                .get(flat_idx)
                .cloned()
                .unwrap_or(0..0);
            flat_idx += 1;
            let all_meshes: Vec<usize> = (mesh_range.start..mesh_range.end).collect();

            let open_clip =
                fo3_render::animation::AnimationClip::from_nif_sequence(nif, "Open");
            let close_clip =
                fo3_render::animation::AnimationClip::from_nif_sequence(nif, "Close");

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
    }

    (refr_bindings, total_colliders)
}
