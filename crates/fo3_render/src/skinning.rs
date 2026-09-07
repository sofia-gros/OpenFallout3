//! # CPU スキニング実装
//!
//! Gamebryo 2.6 のスキニング変形計算を CPU 上で実行する。
//!
//! ## スキニング計算式
//!
//! 参照元: `knowledge/actor_and_skin_mesh.md`, Gamebryo 2.6 `NiSkinInstance::Update`
//!
//! ```text
//! v' = Σ(i=0..3) w[i] * M_bone[i] * B_bone[i] * v
//! ```
//!
//! - `w[i]`: `SkinPartition.vertex_weights[vtx][i]` — ボーン影響度
//! - `B_bone[i]`: `NiSkinData.bone_list[bone_idx].skin_transform` を行列化 — 逆バインドポーズ
//! - `M_bone[i]`: ボーンノードのワールド変換行列（現フレームでは恒等行列 = バインドポーズ固定）
//!
//! ## 初期実装スコープ
//!
//! - ボーン階層の動的変換は未実装（恒等行列として扱う）
//! - NiSkinData の `skin_transform` (ルートオフセット) と各ボーンの `bone_transform` を適用
//! - 結果として T-Pose (バインドポーズ) での正しい頂点位置を得る

use glam::{Mat4, Vec3, Vec4};
use fo3_nif::{NiSkinInstance, NifBlock, NifFile};
use fo3_nif::NiTriShapeData;
use fo3_nif::types::{Matrix33, Vector3};

/// バインドポーズ行列（逆スキン変換）を行列として組み立てる。
///
/// Gamebryo の NiTransform は (translation, rotation, scale) の組み合わせで、
/// 変換行列は M = T * R * S として計算される。
/// 参照元: Gamebryo 2.6 `NiSkinData::BoneData::skin_transform`
fn build_bone_matrix(translation: Vector3, rotation: Matrix33, scale: f32) -> Mat4 {
    // nif.xml の Matrix33 は行優先 (row-major) で格納されている
    let r = &rotation.m;
    let rot_mat = Mat4::from_cols_array(&[
        r[0][0], r[1][0], r[2][0], 0.0,
        r[0][1], r[1][1], r[2][1], 0.0,
        r[0][2], r[1][2], r[2][2], 0.0,
        0.0,     0.0,     0.0,     1.0,
    ]);
    let trans_mat = Mat4::from_translation(Vec3::new(translation.x, translation.y, translation.z));
    let scale_mat = Mat4::from_scale(Vec3::splat(scale));
    trans_mat * rot_mat * scale_mat
}

/// CPU スキニングを適用し、変換後の頂点位置・法線を返す（バインドポーズ固定）。
///
/// 参照元:
/// - `knowledge/actor_and_skin_mesh.md`
/// - `references/nifxml/nif.xml:L5067` (`NiSkinData`)
/// - `references/nifxml/nif.xml:L2143` (`SkinPartition`)
pub fn apply_skinning_cpu(
    geo_data: &NiTriShapeData,
    skin_instance: &NiSkinInstance,
    nif: &NifFile,
) -> Option<(Vec<[f32; 3]>, Vec<[f32; 3]>)> {
    apply_skinning_cpu_with_bones(geo_data, skin_instance, nif, None)
}

/// CPU スキニングを適用し、変換後の頂点位置・法線を返す（動的ボーン行列対応）。
///
/// `bone_world_transforms` は `NiSkinInstance.bones` 配列のインデックス順に対応する
/// 各ボーンの現在時刻におけるワールド変換行列（$M_{\text{bone}}[i]$）。
/// `None` が渡された場合はバインドポーズ（恒等行列）として処理される。
pub fn apply_skinning_cpu_with_bones(
    geo_data: &NiTriShapeData,
    skin_instance: &NiSkinInstance,
    nif: &NifFile,
    bone_world_transforms: Option<&[Mat4]>,
) -> Option<(Vec<[f32; 3]>, Vec<[f32; 3]>)> {
    // NiSkinData を解決
    let skin_data_idx = skin_instance.data;
    if skin_data_idx < 0 || skin_data_idx as usize >= nif.blocks.len() {
        return None;
    }
    let skin_data = match &nif.blocks[skin_data_idx as usize] {
        NifBlock::NiSkinData(d) => d,
        _ => return None,
    };

    // NiSkinPartition を解決
    let part_idx = skin_instance.skin_partition;
    if part_idx < 0 || part_idx as usize >= nif.blocks.len() {
        return None;
    }
    let skin_partition = match &nif.blocks[part_idx as usize] {
        NifBlock::NiSkinPartition(p) => p,
        _ => return None,
    };

    let n_verts = geo_data.common.vertices.len();
    let mut out_positions: Vec<Vec4> = vec![Vec4::ZERO; n_verts];
    let mut out_normals:   Vec<Vec4> = vec![Vec4::ZERO; n_verts];
    let mut written: Vec<bool>       = vec![false; n_verts];

    // ルートスキン変換行列（NiSkinData の skin_transform）
    // 参照元: nif.xml:L5069 "Skin Transform"
    let root_mat = build_bone_matrix(
        skin_data.skin_transform_translation,
        skin_data.skin_transform_rotation,
        skin_data.skin_transform_scale,
    );
    let root_mat_inv = root_mat.inverse();

    for partition in &skin_partition.partitions {
        if partition.vertex_map.is_empty()
            || partition.vertex_weights.is_empty()
            || partition.bone_indices.is_empty()
        {
            continue;
        }

        // 各パーティション内ボーンの変換行列を事前計算
        let mut bone_matrices: Vec<Mat4> = Vec::with_capacity(partition.bones.len());
        for &bone_idx_in_instance in &partition.bones {
            let bone_idx = bone_idx_in_instance as usize;
            if bone_idx < skin_data.bone_list.len() {
                let bd = &skin_data.bone_list[bone_idx];
                // B_bone: 逆バインドポーズ行列 (skin_to_bone 変換)
                let b_bone = build_bone_matrix(
                    bd.skin_transform_translation,
                    bd.skin_transform_rotation,
                    bd.skin_transform_scale,
                );
                // M_bone: アニメーションによるボーンのワールド変換行列
                let m_bone = if let Some(transforms) = bone_world_transforms {
                    if bone_idx < transforms.len() {
                        transforms[bone_idx]
                    } else {
                        Mat4::IDENTITY
                    }
                } else {
                    Mat4::IDENTITY
                };

                // 合成変換行列: root_mat_inv * M_bone * B_bone
                bone_matrices.push(root_mat_inv * m_bone * b_bone);
            } else {
                bone_matrices.push(Mat4::IDENTITY);
            }
        }

        let n_part_verts = partition.num_vertices as usize;
        for vi in 0..n_part_verts {
            if vi >= partition.vertex_map.len() { break; }
            let geo_idx = partition.vertex_map[vi] as usize;
            if geo_idx >= n_verts { continue; }

            let src_pos = if geo_idx < geo_data.common.vertices.len() {
                let v = &geo_data.common.vertices[geo_idx];
                Vec4::new(v.x, v.y, v.z, 1.0)
            } else { continue; };

            let src_nrm = if geo_idx < geo_data.common.normals.len() {
                let n = &geo_data.common.normals[geo_idx];
                Vec4::new(n.x, n.y, n.z, 0.0)
            } else {
                Vec4::new(0.0, 0.0, 1.0, 0.0)
            };

            let weights = if vi < partition.vertex_weights.len() {
                partition.vertex_weights[vi]
            } else {
                [1.0, 0.0, 0.0, 0.0]
            };
            let bone_idxs = if vi < partition.bone_indices.len() {
                partition.bone_indices[vi]
            } else {
                [0, 0, 0, 0]
            };

            let mut blended_pos = Vec4::ZERO;
            let mut blended_nrm = Vec4::ZERO;
            let n_inf = (partition.num_weights_per_vertex as usize).min(4);
            for k in 0..n_inf {
                let w = weights[k];
                if w < 1e-6 { continue; }
                let bi = bone_idxs[k] as usize;
                if bi >= bone_matrices.len() { continue; }
                let mat = bone_matrices[bi];
                blended_pos += w * (mat * src_pos);
                blended_nrm += w * (mat * src_nrm);
            }

            out_positions[geo_idx] = blended_pos;
            out_normals[geo_idx]   = blended_nrm;
            written[geo_idx] = true;
        }
    }

    let result_positions: Vec<[f32; 3]> = (0..n_verts).map(|i| {
        if written[i] {
            [out_positions[i].x, out_positions[i].y, out_positions[i].z]
        } else if i < geo_data.common.vertices.len() {
            let v = &geo_data.common.vertices[i];
            [v.x, v.y, v.z]
        } else {
            [0.0, 0.0, 0.0]
        }
    }).collect();

    let result_normals: Vec<[f32; 3]> = (0..n_verts).map(|i| {
        if written[i] {
            let n = Vec3::new(out_normals[i].x, out_normals[i].y, out_normals[i].z);
            let len = n.length();
            if len > 1e-6 { [n.x / len, n.y / len, n.z / len] } else { [0.0, 0.0, 1.0] }
        } else if i < geo_data.common.normals.len() {
            let n = &geo_data.common.normals[i];
            [n.x, n.y, n.z]
        } else {
            [0.0, 0.0, 1.0]
        }
    }).collect();

    Some((result_positions, result_normals))
}
