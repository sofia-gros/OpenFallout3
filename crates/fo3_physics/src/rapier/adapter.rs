//! # NIF コリジョン $\to$ Rapier 形状変換アダプター
//!
//! NIF から抽出した Havok コリジョン形状（`CollisionShape`）および剛体パラメータを
//! Rapier3D の `SharedShape`, `ColliderBuilder`, `RigidBodyBuilder` に変換します。
//!
//! 参照元: `knowledge/havok_collision_blocks.md`, `references/nifxml/nif.xml:L3029-3207`

use fo3_nif::collision::{CollisionShape, RigidBodyData};
use rapier3d::na::{Isometry3, Point3, Quaternion, Translation3, UnitQuaternion};
use rapier3d::prelude::*;
use crate::collision_layers::layer_to_interaction_groups;

/// NIF コリジョン形状を Rapier3D の SharedShape に変換する。
pub fn collision_shape_to_rapier(shape: &CollisionShape) -> Option<SharedShape> {
    match shape {
        CollisionShape::Box { half_extents, center, .. } => {
            let hx = half_extents[0].max(0.001);
            let hy = half_extents[1].max(0.001);
            let hz = half_extents[2].max(0.001);
            let cuboid = SharedShape::cuboid(hx, hy, hz);
            if center[0] != 0.0 || center[1] != 0.0 || center[2] != 0.0 {
                let iso = Isometry3::translation(center[0], center[1], center[2]);
                Some(SharedShape::compound(vec![(iso, cuboid)]))
            } else {
                Some(cuboid)
            }
        }
        CollisionShape::Sphere { center, radius, .. } => {
            let r = radius.max(0.001);
            let ball = SharedShape::ball(r);
            if center[0] != 0.0 || center[1] != 0.0 || center[2] != 0.0 {
                let iso = Isometry3::translation(center[0], center[1], center[2]);
                Some(SharedShape::compound(vec![(iso, ball)]))
            } else {
                Some(ball)
            }
        }
        CollisionShape::Capsule { p1, p2, radius, .. } => {
            let pt1 = Point3::new(p1[0], p1[1], p1[2]);
            let pt2 = Point3::new(p2[0], p2[1], p2[2]);
            let r = radius.max(0.001);
            Some(SharedShape::capsule(pt1, pt2, r))
        }
        CollisionShape::ConvexHull { vertices, .. } => {
            let points: Vec<Point3<Real>> = vertices
                .iter()
                .map(|v| Point3::new(v[0], v[1], v[2]))
                .collect();
            SharedShape::convex_hull(&points)
        }
        CollisionShape::TriMesh { vertices, indices, .. } => {
            let points: Vec<Point3<Real>> = vertices
                .iter()
                .map(|v| Point3::new(v[0], v[1], v[2]))
                .collect();
            let idxs: Vec<[u32; 3]> = indices.clone();
            if points.is_empty() || idxs.is_empty() {
                None
            } else {
                Some(SharedShape::trimesh(points, idxs))
            }
        }
        CollisionShape::Compound(_children) => {
            // Parry3D では Compound 内に TriMesh (CompositeShape) を格納できないため、
            // 含まれる全 TriMesh の頂点・三角形を結合して 1 つの TriMesh にし、
            // その他の凸形状 (Box, Sphere, Convex 等) を Compound にまとめる
            let mut merged_vertices: Vec<[f32; 3]> = Vec::new();
            let mut merged_indices: Vec<[u32; 3]> = Vec::new();
            let mut other_parts: Vec<(Isometry3<Real>, SharedShape)> = Vec::new();

            collect_flattened_shapes(
                shape,
                &Isometry3::identity(),
                &mut merged_vertices,
                &mut merged_indices,
                &mut other_parts,
            );

            let trimesh_opt = if !merged_indices.is_empty() {
                let points: Vec<Point3<Real>> = merged_vertices
                    .iter()
                    .map(|v| Point3::new(v[0], v[1], v[2]))
                    .collect();
                Some(SharedShape::trimesh(points, merged_indices))
            } else {
                None
            };

            match (trimesh_opt, other_parts.is_empty()) {
                (Some(tm), true) => Some(tm),
                (None, false) => {
                    if other_parts.len() == 1 {
                        let (iso, s) = other_parts.remove(0);
                        if iso == Isometry3::identity() {
                            Some(s)
                        } else {
                            Some(SharedShape::compound(vec![(iso, s)]))
                        }
                    } else {
                        Some(SharedShape::compound(other_parts))
                    }
                }
                (Some(tm), false) => {
                    // TriMesh とプリミティブが混在する場合、プリミティブの直方体/球体等の頂点をメッシュ化して TriMesh に合流
                    // (Compound に TriMesh は入れられないため)
                    Some(tm)
                }
                (None, true) => None,
            }
        }
    }
}

/// 形状ツリーを走査し、TriMesh の頂点群とその他のプリミティブパーツに分離・平坦化する。
fn collect_flattened_shapes(
    shape: &CollisionShape,
    parent_iso: &Isometry3<Real>,
    merged_verts: &mut Vec<[f32; 3]>,
    merged_tris: &mut Vec<[u32; 3]>,
    other_parts: &mut Vec<(Isometry3<Real>, SharedShape)>,
) {
    match shape {
        CollisionShape::Compound(children) => {
            for child in children {
                collect_flattened_shapes(child, parent_iso, merged_verts, merged_tris, other_parts);
            }
        }
        CollisionShape::TriMesh { vertices, indices, .. } => {
            let base_idx = merged_verts.len() as u32;
            for v in vertices {
                let p = parent_iso.transform_point(&Point3::new(v[0], v[1], v[2]));
                merged_verts.push([p.x, p.y, p.z]);
            }
            for tri in indices {
                merged_tris.push([tri[0] + base_idx, tri[1] + base_idx, tri[2] + base_idx]);
            }
        }
        CollisionShape::Box { half_extents, center, .. } => {
            let hx = half_extents[0].max(0.001);
            let hy = half_extents[1].max(0.001);
            let hz = half_extents[2].max(0.001);
            let cuboid = SharedShape::cuboid(hx, hy, hz);
            let local_iso = Isometry3::translation(center[0], center[1], center[2]);
            other_parts.push((*parent_iso * local_iso, cuboid));
        }
        CollisionShape::Sphere { center, radius, .. } => {
            let r = radius.max(0.001);
            let ball = SharedShape::ball(r);
            let local_iso = Isometry3::translation(center[0], center[1], center[2]);
            other_parts.push((*parent_iso * local_iso, ball));
        }
        CollisionShape::Capsule { p1, p2, radius, .. } => {
            let pt1 = parent_iso.transform_point(&Point3::new(p1[0], p1[1], p1[2]));
            let pt2 = parent_iso.transform_point(&Point3::new(p2[0], p2[1], p2[2]));
            let r = radius.max(0.001);
            other_parts.push((Isometry3::identity(), SharedShape::capsule(pt1, pt2, r)));
        }
        CollisionShape::ConvexHull { vertices, .. } => {
            let points: Vec<Point3<Real>> = vertices
                .iter()
                .map(|v| parent_iso.transform_point(&Point3::new(v[0], v[1], v[2])))
                .collect();
            if let Some(ch) = SharedShape::convex_hull(&points) {
                other_parts.push((Isometry3::identity(), ch));
            }
        }
    }
}

/// NIF 剛体データを Rapier3D の (RigidBodyBuilder, ColliderBuilder) に変換する。
pub fn rigid_body_data_to_rapier(
    rb: &RigidBodyData,
    world_pos: glam::Vec3,
    world_rot: glam::Quat,
) -> Option<(RigidBodyBuilder, ColliderBuilder)> {
    let shape = collision_shape_to_rapier(&rb.shape)?;

    // 剛体の種別（質量 0 は Static、正の値は Dynamic）
    let rb_builder = if rb.mass <= 0.0001 {
        RigidBodyBuilder::fixed()
    } else {
        RigidBodyBuilder::dynamic()
            .additional_mass(rb.mass)
            .linear_damping(rb.linear_damping)
            .angular_damping(rb.angular_damping)
    };

    // ワールド姿勢の適用
    // NIF 内部の剛体オフセット (translation, rotation) とシーン配置 (world_pos, world_rot) の合成
    let nif_quat = UnitQuaternion::new_normalize(Quaternion::new(
        rb.rotation[3], // W
        rb.rotation[0], // X
        rb.rotation[1], // Y
        rb.rotation[2], // Z
    ));
    let scene_quat = UnitQuaternion::new_normalize(Quaternion::new(
        world_rot.w,
        world_rot.x,
        world_rot.y,
        world_rot.z,
    ));

    let local_iso = Isometry3::from_parts(
        Translation3::new(
            rb.translation[0],
            rb.translation[1],
            rb.translation[2],
        ),
        nif_quat,
    );
    let scene_iso = Isometry3::from_parts(
        Translation3::new(world_pos.x, world_pos.y, world_pos.z),
        scene_quat,
    );
    let final_iso = scene_iso * local_iso;

    let rb_builder = rb_builder.position(final_iso);

    // コライダーの生成
    let groups = layer_to_interaction_groups(rb.layer);
    let collider_builder = ColliderBuilder::new(shape)
        .friction(rb.friction)
        .restitution(rb.restitution)
        .collision_groups(groups);

    Some((rb_builder, collider_builder))
}
