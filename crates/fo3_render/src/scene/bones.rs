//! # ボーン階層探索および順運動学 (FK) 計算
//!
//! スケルトンボーンのワールド変換計算、アニメーション姿勢オーバーライド、
//! スキンインスタンスボーン引き当て、剛体アタッチメントボーン解決を担当する。
//! 参照元: Gamebryo 2.6 `NiAVObject::UpdateDownwardPass`, `NiSkinInstance::Update`, `NiNode::AttachChild`

use std::collections::HashMap;
use glam::Mat4;

use fo3_gamebryo_core::NiTransform;
use fo3_nif::{NifBlock, NifFile};

use crate::animation::SkeletonPose;
use crate::scene::mesh::to_core_transform;

/// スケルトン階層の全 NiNode / BSFadeNode ワールド変換行列を `bone_world_map` に蓄積する。
///
/// `traverse_block` に先立って実行するプレパス。単一パスの深さ優先トラバースでは、
/// 装備 NIF (`meshes\armor\...\outfit*.nif`) のように NiTriShape ブロックがボーン
/// NiNode ブロックより先の位置・子ノード配列順で現れる場合、NiTriShape のスキニング
/// 処理時にボーンが未登録のまま `Mat4::IDENTITY` へフォールバックしてしまう。
/// このプレパスにより全ボーンのワールド変換が先に解決され、アニメーション適用時に
/// 常に正しい現在行列を参照できる。
///
/// 参照元: `knowledge/actor_and_skin_mesh.md`, Gamebryo 2.6 `NiAVObject::UpdateDownwardPass`
pub fn collect_bone_world_transforms(
    block_index: i32,
    parent_world: &NiTransform,
    nif: &NifFile,
    bone_world_map: &mut HashMap<i32, Mat4>,
) {
    if block_index < 0 || block_index as usize >= nif.blocks.len() {
        return;
    }
    match &nif.blocks[block_index as usize] {
        NifBlock::NiNode(node) => {
            let local = to_core_transform(&node.av);
            let world = parent_world.compose(&local);
            bone_world_map.insert(block_index, world.to_mat4());
            for &child in &node.children {
                collect_bone_world_transforms(child, &world, nif, bone_world_map);
            }
        }
        NifBlock::BSFadeNode(fade) => {
            let local = to_core_transform(&fade.node.av);
            let world = parent_world.compose(&local);
            bone_world_map.insert(block_index, world.to_mat4());
            for &child in &fade.node.children {
                collect_bone_world_transforms(child, &world, nif, bone_world_map);
            }
        }
        _ => {}
    }
}

/// NIF 内のボーン階層をアニメーション姿勢 (`pose`) で順運動学 (FK) 再計算し、
/// 各ボーンノードのワールド変換行列を `bone_world_map` (block_index キー) および
/// `bone_name_world_map` (ボーン名キー) に格納する。
///
/// 参照元:
/// - Gamebryo 2.6 `NiControllerSequence::Update` (ボーンローカル変換の適用)
/// - Gamebryo 2.6 `NiAVObject::UpdateDownwardPass` (FK ワールド合成)
/// - `knowledge/actor_and_skin_mesh.md` (セクション 4.4: スケルトン分離とボーン名マッピング)
pub fn recompute_bone_world_maps_with_pose(
    nif: &NifFile,
    pose: &SkeletonPose,
    bone_world_map: &mut HashMap<i32, Mat4>,
    bone_name_world_map: &mut HashMap<String, Mat4>,
) {
    bone_world_map.clear();
    bone_name_world_map.clear();
    recompute_fk_with_names(0, &NiTransform::default(), nif, pose, bone_world_map, bone_name_world_map);
}

/// NIF 内のボーン階層をアニメーション姿勢 (`pose`) で順運動学 (FK) 再計算し、
/// 各ボーンノードのワールド変換行列を `bone_world_map` に格納する。
pub fn recompute_bone_world_map_with_pose(
    nif: &NifFile,
    pose: &SkeletonPose,
    bone_world_map: &mut HashMap<i32, Mat4>,
) {
    bone_world_map.clear();
    let mut dummy_name_map = HashMap::new();
    recompute_fk_with_names(0, &NiTransform::default(), nif, pose, bone_world_map, &mut dummy_name_map);
}

/// Forward Kinematics 再帰: `pose.overrides` に一致するノードはローカル変換を差し替えて
/// ワールド合成し、`bone_world_map` および `bone_name_world_map` に登録する。
fn recompute_fk_with_names(
    block_index: i32,
    parent_world: &NiTransform,
    nif: &NifFile,
    pose: &SkeletonPose,
    bone_world_map: &mut HashMap<i32, Mat4>,
    bone_name_world_map: &mut HashMap<String, Mat4>,
) {
    if block_index < 0 || block_index as usize >= nif.blocks.len() {
        return;
    }

    let block = &nif.blocks[block_index as usize];
    match block {
        NifBlock::NiNode(node) => {
            let base = to_core_transform(&node.av);
            let local = lookup_override(base, &node.av, nif, pose);
            let world = parent_world.compose(&local);
            let mat = world.to_mat4();
            bone_world_map.insert(block_index, mat);
            if let Some(name) = nif.get_string(node.av.net.name_index) {
                bone_name_world_map.insert(name.to_string(), mat);
            }
            for &child in &node.children {
                recompute_fk_with_names(child, &world, nif, pose, bone_world_map, bone_name_world_map);
            }
        }
        NifBlock::BSFadeNode(fade) => {
            let base = to_core_transform(&fade.node.av);
            let local = lookup_override(base, &fade.node.av, nif, pose);
            let world = parent_world.compose(&local);
            let mat = world.to_mat4();
            bone_world_map.insert(block_index, mat);
            if let Some(name) = nif.get_string(fade.node.av.net.name_index) {
                bone_name_world_map.insert(name.to_string(), mat);
            }
            for &child in &fade.node.children {
                recompute_fk_with_names(child, &world, nif, pose, bone_world_map, bone_name_world_map);
            }
        }
        _ => {}
    }
}

/// ノード名がポーズのオーバーライドに存在する場合はそのローカル変換を返す。
///
/// Gamebryo 2.6 アニメーション蓄積 (Root Motion / Accumulation) 仕様:
/// `Bip01 NonAccum` が移動トラックを持つ場合、親 `Bip01` のバインドポーズ高さ (Z ≈ 67.77) と
/// 二重加算されるのを防ぐため、`Bip01` のローカル平行移動量はゼロリセットする。
/// 参照元: `knowledge/actor_and_skin_mesh.md` (セクション 4.11)
fn lookup_override(
    mut base: NiTransform,
    av: &fo3_nif::NiAVObject,
    nif: &NifFile,
    pose: &SkeletonPose,
) -> NiTransform {
    if let Some(name) = nif.get_string(av.net.name_index) {
        if name == "Bip01" && pose.overrides.contains_key("Bip01 NonAccum") {
            base.translation = glam::Vec3::ZERO;
        }
        if let Some(t) = pose.overrides.get(name) {
            return t.apply_to(base);
        }
    }
    base
}

/// `NiSkinInstance.bones` の各ブロックインデックスからパーツ NIF 側のボーンノード名を取得し、
/// スケルトン側で計算されたボーン名→ワールド行列マップ (`bone_name_world_map`) から行列配列を解決する。
///
/// 参照元: Gamebryo 2.6 `NiSkinInstance::Update`, `knowledge/actor_and_skin_mesh.md` (セクション 4.4)
pub fn resolve_bone_world_transforms_by_name(
    skin_instance: &fo3_nif::NiSkinInstance,
    mesh_nif: &NifFile,
    bone_name_world_map: &HashMap<String, Mat4>,
    fallback_bone_world_map: Option<&HashMap<i32, Mat4>>,
) -> Vec<Mat4> {
    skin_instance
        .bones
        .iter()
        .map(|&bone_block_idx| {
            if bone_block_idx >= 0 && (bone_block_idx as usize) < mesh_nif.blocks.len() {
                let name = match &mesh_nif.blocks[bone_block_idx as usize] {
                    NifBlock::NiNode(node) => mesh_nif.get_string(node.av.net.name_index),
                    NifBlock::BSFadeNode(fade) => mesh_nif.get_string(fade.node.av.net.name_index),
                    _ => None,
                };
                if let Some(name) = name {
                    if let Some(&mat) = bone_name_world_map.get(name) {
                        return mat;
                    }
                }
            }
            if let Some(fallback) = fallback_bone_world_map {
                fallback.get(&bone_block_idx).copied().unwrap_or(Mat4::IDENTITY)
            } else {
                Mat4::IDENTITY
            }
        })
        .collect()
}

/// `NiSkinInstance.bones` の各ブロックインデックスに対応するボーンのワールド変換行列を
/// `bone_world_map` (traverse_block 中に蓄積された block_index → Mat4) を参照して解決する。
pub fn resolve_bone_world_transforms(
    skin_instance: &fo3_nif::NiSkinInstance,
    bone_world_map: &HashMap<i32, Mat4>,
) -> Vec<Mat4> {
    skin_instance
        .bones
        .iter()
        .map(|&bone_block_idx| {
            bone_world_map
                .get(&bone_block_idx)
                .copied()
                .unwrap_or(Mat4::IDENTITY)
        })
        .collect()
}

/// スキンを持たない剛体パーツ NIF (目、歯、舌、髪など) について、
/// NIF 内のルートノードから各ジオメトリシェイプまでの累積トランスフォームを再帰的に収集する。
/// 参照元: Gamebryo 2.6 `NiAVObject::UpdateDownwardPass`, `NiNode::AttachChild`
pub fn collect_shape_transforms(
    block_index: i32,
    parent_world: &NiTransform,
    nif: &NifFile,
    shape_transforms: &mut HashMap<usize, Mat4>,
) {
    if block_index < 0 || block_index as usize >= nif.blocks.len() {
        return;
    }
    let block = &nif.blocks[block_index as usize];
    match block {
        NifBlock::NiNode(node) => {
            let local = to_core_transform(&node.av);
            let world = parent_world.compose(&local);
            for &child in &node.children {
                collect_shape_transforms(child, &world, nif, shape_transforms);
            }
        }
        NifBlock::BSFadeNode(fade) => {
            let local = to_core_transform(&fade.node.av);
            let world = parent_world.compose(&local);
            for &child in &fade.node.children {
                collect_shape_transforms(child, &world, nif, shape_transforms);
            }
        }
        NifBlock::NiTriShape(shape) => {
            let local = to_core_transform(&shape.geom.av);
            let world = parent_world.compose(&local);
            shape_transforms.insert(block_index as usize, world.to_mat4());
        }
        NifBlock::NiTriStrips(strips) => {
            let local = to_core_transform(&strips.geom.av);
            let world = parent_world.compose(&local);
            shape_transforms.insert(block_index as usize, world.to_mat4());
        }
        _ => {}
    }
}

/// 剛体パーツ（目・歯・舌・髪）のローカル変換行列を算出し、頭部ボーン軸整合を適用する。
/// 参照元: eyelefthuman.nif Block 0, `knowledge/actor_and_skin_mesh.md` (セクション 4.9)
pub fn compute_rigid_part_local_transform(
    block_index: usize,
    bone_name: &str,
    shape_transforms: &HashMap<usize, Mat4>,
    av_obj: &fo3_nif::NiAVObject,
) -> Mat4 {
    let mut local_transform = shape_transforms
        .get(&block_index)
        .copied()
        .unwrap_or_else(|| to_core_transform(av_obj).to_mat4());

    if bone_name == "Bip01 Head" {
        let head_align = Mat4::from_cols_array_2d(&[
            [0.0, 0.0, -1.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        let x_col = local_transform.col(0);
        let already_aligned = x_col.z < -0.8;
        if !already_aligned {
            local_transform = head_align * local_transform;
        }
    }
    local_transform
}

/// NIF のルートブロックからアタッチメント対象ボーン名を検出する。
///
/// Fallout 3 の HeadParts (目、歯、舌、帽子など) や Weapon は、
/// ルートノードの `extra_data_list` に対象ボーン名 (例: "Bip01 Head", "Weapon") を保持した `NiStringExtraData` を持つ。
/// コリジョンメタデータ (ColGroupInfo, Havok パラメータ等) や特殊タグ (#) は除外する。
/// 参照元: `references/nifxml/nif.xml:L1340` (`NiStringExtraData`), `references/nifskope/src/spells/mesh.cpp`
pub fn find_attach_bone_name(nif: &NifFile) -> Option<String> {
    if nif.blocks.is_empty() {
        return None;
    }
    let extra_data_list = match &nif.blocks[0] {
        NifBlock::NiNode(node) => &node.av.net.extra_data_list,
        NifBlock::BSFadeNode(fade) => &fade.node.av.net.extra_data_list,
        _ => return None,
    };
    for &extra_idx in extra_data_list {
        if extra_idx < 0 || extra_idx as usize >= nif.blocks.len() {
            continue;
        }
        if let NifBlock::NiStringExtraData(ref extra) = nif.blocks[extra_idx as usize] {
            if let Some(s) = nif.get_string(extra.string_data_index) {
                let s_trim = s.trim();
                if !s_trim.is_empty()
                    && !s_trim.contains('\n')
                    && !s_trim.contains('\r')
                    && !s_trim.contains('=')
                    && !s_trim.contains('#')
                {
                    return Some(s_trim.to_string());
                }
            }
        }
    }
    None
}
