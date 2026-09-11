//! # アクターインスタンスおよびアニメーション更新
//!
//! キャラクタ（人型アクター）のアセンブリ、スケルトンボーン姿勢更新、
//! スキン変形（CPU スキニング）、剛体パーツ（目・歯・舌）追従更新を統括する。
//! 参照元: Gamebryo 2.6 `NiActorManager`, `NiSkinInstance::Update`, `NiNode::AttachChild`

use std::collections::HashMap;
use std::sync::Arc;
use glam::Mat4;
use wgpu;

use fo3_gamebryo_core::NiTransform;
use fo3_nif::{NifBlock, NifFile};

use crate::animation::{AnimationPlayer, SkeletonPose};
use crate::scene::bones::{
    collect_shape_transforms, recompute_bone_world_maps_with_pose,
    resolve_bone_world_transforms_by_name,
};
use crate::scene::mesh::{to_core_transform, RenderMesh};
use crate::scene::traversal::{is_dismember_hidden, is_node_hidden};
use crate::skinning::apply_skinning_cpu_with_bones;

/// アニメーション対応スキンメッシュの更新用情報。
///
/// スキニング計算に必要な NIF データへの参照（ブロックインデックス）を保持し、
/// 毎フレームの姿勢更新時に参照される。
///
/// 参照元: Gamebryo 2.6 `NiSkinInstance::Update`（毎フレームの変形計算）
#[derive(Clone, Debug)]
pub struct AnimatedSkinMesh {
    /// `RenderScene::meshes` 中の対象 `RenderMesh` インデックス
    pub mesh_index: usize,
    /// パーツ NIF のインデックス（単一 NIF 構築時は 0）
    pub part_index: usize,
    /// NiTriShapeData ブロックインデックス（該当 NIF 内）
    pub geo_data_block: i32,
    /// NiSkinInstance ブロックインデックス（該当 NIF 内）
    pub skin_instance_block: i32,
}

/// アニメーションによるボーン追従を行う剛体アタッチメントメッシュ情報。
///
/// スキンメッシュを持たず、スケルトンの特定ボーンノード (例: "Bip01 Head") にアタッチされる
/// 剛体パーツ (目、歯、舌など) のモデル変換行列を毎フレーム追従更新するために使用する。
///
/// 参照元: Gamebryo 2.6 `NiNode::AttachChild`, `knowledge/actor_and_skin_mesh.md` (セクション 4.7)
#[derive(Clone, Debug)]
pub struct AnimatedRigidMesh {
    /// `RenderScene::meshes` 中の対象 `RenderMesh` インデックス
    pub mesh_index: usize,
    /// アタッチ先のスケルトンボーン名 (例: "Bip01 Head")
    pub bone_name: String,
    /// パーツルート基準のローカル変換行列
    pub local_transform: Mat4,
}

/// セルまたはワールド内に配置された独立アクターインスタンス。
///
/// 各アクターは自身のワールド配置変換、スケルトン、パーツ群、アニメーションプレイヤー、
/// スキンメッシュおよび剛体パーツ（目・歯・舌）情報を保持し、独立してアニメーション更新される。
///
/// 参照元: Gamebryo 2.6 `NiNode` シーングラフ階層, Fallout 3 `ACHR` 配置アクター仕様
#[derive(Clone, Debug)]
pub struct RenderActorInstance {
    /// アクターの FormID または一意の識別番号
    pub form_id: u32,
    /// アクター名 (エディタ ID または表示名)
    pub name: String,
    /// ワールド空間変換（セル内の配置位置・回転・スケール）
    pub world_transform: NiTransform,
    /// スケルトン NIF ファイル
    pub skeleton_nif: Arc<NifFile>,
    /// アクターを構成する全パーツ NIF ファイル群
    pub parts: Vec<Arc<NifFile>>,
    /// アニメーション再生用プレイヤー
    pub anim_player: Option<AnimationPlayer>,
    /// アニメーションキーフレーム NIF ファイル (KF)
    pub kf_nif: Option<Arc<NifFile>>,
    /// 現在のボーンアニメーション姿勢
    pub anim_pose: SkeletonPose,
    /// このアクターに属するスキンメッシュ更新情報リスト
    pub anim_skin_meshes: Vec<AnimatedSkinMesh>,
    /// このアクターに属する剛体アタッチメントメッシュ更新情報リスト (目・歯・舌など)
    pub anim_rigid_meshes: Vec<AnimatedRigidMesh>,
}

impl RenderActorInstance {
    /// アニメーション時間を進め、ボーン FK、スキンメッシュ頂点、および剛体パーツ Uniform を更新する。
    ///
    /// 参照元:
    /// - Gamebryo 2.6 `NiControllerSequence::Update` → `NiSkinInstance::Update`
    /// - `knowledge/actor_and_skin_mesh.md` (セクション 4.4, 4.7)
    pub fn update(
        &mut self,
        dt: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        meshes: &mut [RenderMesh],
    ) {
        if let (Some(ref mut player), Some(ref kf_nif)) = (&mut self.anim_player, &self.kf_nif) {
            player.update(kf_nif, dt, &mut self.anim_pose);

            let mut skel_bone_world_map = HashMap::new();
            let mut skel_bone_name_world_map = HashMap::new();
            recompute_bone_world_maps_with_pose(
                &self.skeleton_nif,
                &self.anim_pose,
                &mut skel_bone_world_map,
                &mut skel_bone_name_world_map,
            );

            let actor_world_mat = self.world_transform.to_mat4();

            // 1. スキンメッシュ更新 (CPU スキニング頂点更新)
            let part_refs: Vec<&NifFile> = self.parts.iter().map(|p| p.as_ref()).collect();
            for anim in &self.anim_skin_meshes {
                let mesh_index = anim.mesh_index;
                if mesh_index >= meshes.len() {
                    continue;
                }
                if anim.part_index >= part_refs.len() {
                    continue;
                }
                let mesh_nif = part_refs[anim.part_index];

                let geo_data_block = anim.geo_data_block as usize;
                if geo_data_block >= mesh_nif.blocks.len() {
                    continue;
                }
                let NifBlock::NiTriShapeData(geo_data) = &mesh_nif.blocks[geo_data_block] else {
                    continue;
                };

                let skin_inst_block = anim.skin_instance_block as usize;
                if skin_inst_block >= mesh_nif.blocks.len() {
                    continue;
                }
                let skin_inst = match &mesh_nif.blocks[skin_inst_block] {
                    NifBlock::NiSkinInstance(inst) => inst,
                    NifBlock::BSDismemberSkinInstance(bdsi) => &bdsi.skin_instance,
                    _ => continue,
                };

                let bone_transforms = resolve_bone_world_transforms_by_name(
                    skin_inst,
                    mesh_nif,
                    &skel_bone_name_world_map,
                    None,
                );
                let bone_refs = if bone_transforms.is_empty() {
                    None
                } else {
                    Some(bone_transforms.as_slice())
                };
                if let Some((positions, normals)) =
                    apply_skinning_cpu_with_bones(geo_data, skin_inst, mesh_nif, bone_refs)
                {
                    meshes[mesh_index]
                        .mesh
                        .update_skinned_vertices(device, geo_data, &positions, &normals);
                }
            }

            // 2. 剛体アタッチメントパーツ (目・歯・舌) のモデル Uniform 更新
            for rigid in &self.anim_rigid_meshes {
                if rigid.mesh_index >= meshes.len() {
                    continue;
                }
                if let Some(bone_world) = skel_bone_name_world_map.get(&rigid.bone_name) {
                    let current_world = actor_world_mat * (*bone_world * rigid.local_transform);
                    queue.write_buffer(
                        &meshes[rigid.mesh_index].model_uniform_buffer,
                        0,
                        bytemuck::cast_slice(&[current_world.to_cols_array_2d()]),
                    );
                }
            }
        }
    }
}

/// メッシュ名リストから、該当 NIF に対応するスキンメッシュ情報を収集する。
pub fn collect_anim_skin_meshes_for_names(
    part_index: usize,
    mesh_offset: usize,
    nif: &NifFile,
    mesh_names: &[&str],
) -> Vec<AnimatedSkinMesh> {
    let mut result = Vec::new();
    let relevant_names = if mesh_offset < mesh_names.len() {
        &mesh_names[mesh_offset..]
    } else {
        &[]
    };

    for (block_idx, block) in nif.blocks.iter().enumerate() {
        let NifBlock::NiTriShape(shape) = block else { continue };
        let skin_inst_block = shape.geom.skin_instance;
        if skin_inst_block < 0 { continue; }
        if is_dismember_hidden(skin_inst_block, nif) { continue; }
        let geo_data_block = shape.geom.data;
        if geo_data_block < 0 { continue; }
        // NIF ブロック名でメッシュを検索
        let mesh_name = nif.get_string(shape.geom.av.net.name_index).unwrap_or("");
        let block_hint = block_idx.to_string();
        // relevant_names 内で対応する RenderMesh 名を探す（名前一致 or ブロックインデックス含む）
        if let Some(pos) = relevant_names.iter().position(|name| {
            *name == mesh_name || name.contains(&block_hint)
        }) {
            result.push(AnimatedSkinMesh {
                mesh_index: mesh_offset + pos,
                part_index,
                geo_data_block,
                skin_instance_block: skin_inst_block,
            });
        }
    }
    result
}

/// 各メッシュの NIF 内ローカルトランスフォームを保持した `AnimatedRigidMesh` を収集する。
///
/// `Bip01 Head` などの剛体アタッチメントボーンに対して、NIF 内部のノード階層変換を正しく合成し、
/// 3ds Max Biped ボーン座標系（長手方向=X, 前面=Y, 左右=-Z）から正立頭部空間（上=Z, 前=Y, 右=X）への
/// アライメント補正を適用して頭部追従姿勢を構築する。
///
/// 参照元: Gamebryo 2.6 `NiAVObject::m_kLocal`, `NiNode::AttachChild`, `knowledge/actor_and_skin_mesh.md` (セクション 4.7, 4.9)
pub fn collect_anim_rigid_meshes_for_part(
    mesh_offset: usize,
    bone_name: &str,
    nif: &NifFile,
    mesh_names: &[&str],
) -> Vec<AnimatedRigidMesh> {
    let mut result = Vec::new();
    let relevant_names = if mesh_offset < mesh_names.len() {
        &mesh_names[mesh_offset..]
    } else {
        &[]
    };

    let mut shape_transforms = HashMap::new();
    collect_shape_transforms(0, &NiTransform::default(), nif, &mut shape_transforms);

    // Bip01 Head ボーン座標系 (長手方向=X, 前面=Y, 左右=-Z) から
    // 頭部正立空間 (上=Z, 前=Y, 右=X) への直交基底変換行列
    // head_mat * head_align = IDENTITY となる基底。
    // 参照元: eyelefthuman.nif Block 0, knowledge/actor_and_skin_mesh.md (セクション 4.9)
    let head_align = Mat4::from_cols_array_2d(&[
        [0.0, 0.0, -1.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    for (block_idx, block) in nif.blocks.iter().enumerate() {
        let (name_index, av_obj) = match block {
            NifBlock::NiTriShape(shape) => (shape.geom.av.net.name_index, &shape.geom.av),
            NifBlock::NiTriStrips(strips) => (strips.geom.av.net.name_index, &strips.geom.av),
            _ => continue,
        };

        if is_node_hidden(av_obj, nif) {
            continue;
        }

        let mesh_name = nif.get_string(name_index).unwrap_or("");
        let block_hint = block_idx.to_string();
        if let Some(pos) = relevant_names.iter().position(|name| {
            *name == mesh_name || name.contains(&block_hint)
        }) {
            let mut local_transform = shape_transforms
                .get(&block_idx)
                .copied()
                .unwrap_or_else(|| to_core_transform(av_obj).to_mat4());

            if bone_name == "Bip01 Head" {
                // NIF ルートノード等に既に head_align (Z-up 軸整合) が含まれているか判定
                // 第0列 (X軸) の Z 成分が -1.0 に近ければ既に整合済み
                let x_col = local_transform.col(0);
                let already_aligned = x_col.z < -0.8;
                if !already_aligned {
                    local_transform = head_align * local_transform;
                }
            }

            result.push(AnimatedRigidMesh {
                mesh_index: mesh_offset + pos,
                bone_name: bone_name.to_string(),
                local_transform,
            });
        }
    }

    // もしブロック名でマッチしなかった場合のフォールバック: mesh_offset 以降の全メッシュを登録
    if result.is_empty() && !relevant_names.is_empty() {
        for pos in 0..relevant_names.len() {
            let fallback_transform = if bone_name == "Bip01 Head" {
                head_align
            } else {
                Mat4::IDENTITY
            };
            result.push(AnimatedRigidMesh {
                mesh_index: mesh_offset + pos,
                bone_name: bone_name.to_string(),
                local_transform: fallback_transform,
            });
        }
    }

    result
}
