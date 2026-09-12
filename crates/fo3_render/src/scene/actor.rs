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
use crate::sequence::{SequenceManager, SequenceTrack};
use crate::pipeline::RenderContext;
use crate::scene::bones::{
    collect_bone_world_transforms, collect_shape_transforms, find_attach_bone_name,
    recompute_bone_world_maps_with_pose, resolve_bone_world_transforms,
    resolve_bone_world_transforms_by_name,
};
use crate::scene::mesh::{to_core_transform, RenderMesh};
use crate::scene::traversal::{is_dismember_hidden, is_node_hidden, traverse_block, ActorPartAnimCollector};
use crate::skinning::apply_skinning_cpu_with_bones;
use crate::texture::GpuTexture;
use fo3_vfs::VfsManager;

use super::RenderScene;

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
    /// 単一アニメーション再生用プレイヤー (レガシー/簡易用)
    pub anim_player: Option<AnimationPlayer>,
    /// アニメーションキーフレーム NIF ファイル (KF)
    pub kf_nif: Option<Arc<NifFile>>,
    /// 複数シーケンス合成マネージャー (Gamebryo 2.6 NiControllerManager: ボディ+フェイシャル+リップシンク)
    pub sequence_manager: Option<SequenceManager>,
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
    /// - Gamebryo 2.6 `NiControllerManager::Update` → `NiControllerSequence::Update` → `NiSkinInstance::Update`
    /// - `knowledge/actor_and_skin_mesh.md` (セクション 4.4, 4.7)
    /// - `knowledge/animation_sequence_blending.md`
    pub fn update(
        &mut self,
        dt: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        meshes: &mut [RenderMesh],
    ) {
        // 複数シーケンスマネージャーが設定されている場合はマルチトラック優先度合成（ボディ＋フェイシャル＋リップシンク）
        if let Some(ref mut seq_mgr) = self.sequence_manager {
            self.anim_pose = seq_mgr.update(dt);
        } else if let (Some(ref mut player), Some(ref kf_nif)) = (&mut self.anim_player, &self.kf_nif) {
            player.update(kf_nif, dt, &mut self.anim_pose);
        }

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
                if let Some(ref bp) = meshes[mesh_index].bone_palette {
                    bp.update(queue, &skel_bone_name_world_map);
                } else {
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

    /// フェイシャルまたはリップシンクのシーケンストラックを追加する。
    ///
    /// 既存の単一 `anim_player` がある場合は自動的に基底トラック (Priority: 0) として統合し、
    /// マルチトラック合成マネージャーに切り替える。
    ///
    /// 参照元: Gamebryo 2.6 `NiControllerManager::AddSequence`
    pub fn add_sequence_track(&mut self, track: SequenceTrack) {
        if self.sequence_manager.is_none() {
            let mut mgr = SequenceManager::new();
            if let (Some(player), Some(ref kf)) = (self.anim_player.take(), &self.kf_nif) {
                mgr.add_track(SequenceTrack::new("Base", player, Arc::clone(kf), 1.0, 0));
            }
            self.sequence_manager = Some(mgr);
        }
        if let Some(ref mut mgr) = self.sequence_manager {
            mgr.add_track(track);
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

/// 単一 NIF のアニメーション対応スキンメッシュ情報を収集する。
pub fn collect_anim_skin_meshes(nif: &NifFile, meshes: &[RenderMesh]) -> Vec<AnimatedSkinMesh> {
    let mesh_names: Vec<&str> = meshes.iter().map(|m| m.name.as_str()).collect();
    collect_anim_skin_meshes_for_names(0, 0, nif, &mesh_names)
}

impl RenderScene {
    /// 最新の姿勢が反映されたスキン変形結果をGPUバッファに書き込む。
    ///
    /// 参照元:
    /// - Gamebryo 2.6 `NiSkinInstance::Update`（毎フレームの変形計算）
    /// - `knowledge/actor_and_skin_mesh.md`（スキニング計算式）
    pub fn update_animated_skins(
        &mut self,
        device: &wgpu::Device,
        nif: &NifFile,
        bone_world_map: &HashMap<i32, Mat4>,
    ) {
        for anim in &self.anim_skin_meshes {
            let mesh_index = anim.mesh_index;
            if mesh_index >= self.meshes.len() { continue; }

            // NiTriShapeData を解決
            let geo_data_block = anim.geo_data_block as usize;
            if geo_data_block >= nif.blocks.len() { continue; }
            let NifBlock::NiTriShapeData(geo_data) = &nif.blocks[geo_data_block] else { continue };

            // NiSkinInstance を解決
            let skin_inst_block = anim.skin_instance_block as usize;
            if skin_inst_block >= nif.blocks.len() { continue; }
            let skin_inst = match &nif.blocks[skin_inst_block] {
                NifBlock::NiSkinInstance(inst) => inst,
                NifBlock::BSDismemberSkinInstance(bdsi) => &bdsi.skin_instance,
                _ => continue,
            };

            // 現在のボーン行列を解決してスキニング計算
            let bone_transforms = resolve_bone_world_transforms(skin_inst, bone_world_map);
            let bone_refs = if bone_transforms.is_empty() { None } else { Some(bone_transforms.as_slice()) };
            if let Some((positions, normals)) = apply_skinning_cpu_with_bones(geo_data, skin_inst, nif, bone_refs) {
                self.meshes[mesh_index].mesh.update_skinned_vertices(device, geo_data, &positions, &normals);
            }
        }
    }

    /// マルチパーツ構成のアクターに対し、スケルトンのボーン名ワールド行列を用いて全スキンメッシュの頂点バッファを更新する。
    ///
    /// キャラクタ（NPC）を構成する各パーツ（頭部、胴体、防具、手など）のスキンメッシュは、
    /// スケルトン NIF（`skeleton.nif`）で計算された各ボーンノードのワールド変換をボーン名で引き当てて変形する。
    ///
    /// 参照元:
    /// - Gamebryo 2.6 `NiSkinInstance::Update`（毎フレームの変形計算）
    /// - `knowledge/actor_and_skin_mesh.md` (セクション 4.4: スケルトン分離とボーン名マッピング)
    pub fn update_animated_skins_multi_parts(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        parts: &[&NifFile],
        bone_name_world_map: &HashMap<String, Mat4>,
    ) {
        for anim in &self.anim_skin_meshes {
            let mesh_index = anim.mesh_index;
            if mesh_index >= self.meshes.len() { continue; }

            // GPU スキニング (Phase 6-D): ボーンパレットが存在すればパレット Uniform を更新して完了
            if let Some(ref bp) = self.meshes[mesh_index].bone_palette {
                bp.update(queue, bone_name_world_map);
                continue;
            }

            if anim.part_index >= parts.len() { continue; }
            let mesh_nif = parts[anim.part_index];

            // NiTriShapeData を解決
            let geo_data_block = anim.geo_data_block as usize;
            if geo_data_block >= mesh_nif.blocks.len() { continue; }
            let NifBlock::NiTriShapeData(geo_data) = &mesh_nif.blocks[geo_data_block] else { continue };

            // NiSkinInstance を解決
            let skin_inst_block = anim.skin_instance_block as usize;
            if skin_inst_block >= mesh_nif.blocks.len() { continue; }
            let skin_inst = match &mesh_nif.blocks[skin_inst_block] {
                NifBlock::NiSkinInstance(inst) => inst,
                NifBlock::BSDismemberSkinInstance(bdsi) => &bdsi.skin_instance,
                _ => continue,
            };

            // スケルトンのボーン名から現在のボーン行列を解決してスキニング計算 (CPU フォールバック)
            let bone_transforms = resolve_bone_world_transforms_by_name(skin_inst, mesh_nif, bone_name_world_map, None);
            let bone_refs = if bone_transforms.is_empty() { None } else { Some(bone_transforms.as_slice()) };
            if let Some((positions, normals)) = apply_skinning_cpu_with_bones(geo_data, skin_inst, mesh_nif, bone_refs) {
                self.meshes[mesh_index].mesh.update_skinned_vertices(device, geo_data, &positions, &normals);
            }
        }
    }

    /// スケルトン側のボーン名ワールド行列を用いて、単一パーツメッシュ NIF の全スキンメッシュの頂点バッファを更新する。
    ///
    /// 参照元:
    /// - Gamebryo 2.6 `NiSkinInstance::Update`
    /// - `knowledge/actor_and_skin_mesh.md` (セクション 4.4: スケルトン分離とボーン名マッピング)
    pub fn update_animated_skins_with_skeleton(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh_nif: &NifFile,
        bone_name_world_map: &HashMap<String, Mat4>,
    ) {
        self.update_animated_skins_multi_parts(device, queue, &[mesh_nif], bone_name_world_map);
    }

    /// アニメーション更新後のボーン行列で、全剛体アタッチメントメッシュ (目、歯、舌など) のモデル変換行列バッファを更新する。
    ///
    /// 参照元: Gamebryo 2.6 `NiNode::UpdateDownwardPass`, `knowledge/actor_and_skin_mesh.md` (セクション 4.7)
    pub fn update_animated_rigid_meshes(
        &mut self,
        queue: &wgpu::Queue,
        bone_name_world_map: &HashMap<String, Mat4>,
    ) {
        for rigid in &self.anim_rigid_meshes {
            if rigid.mesh_index >= self.meshes.len() {
                continue;
            }
            if let Some(bone_world) = bone_name_world_map.get(&rigid.bone_name) {
                let current_world = *bone_world * rigid.local_transform;
                queue.write_buffer(
                    &self.meshes[rigid.mesh_index].model_uniform_buffer,
                    0,
                    bytemuck::cast_slice(&[current_world.to_cols_array_2d()]),
                );
            }
        }
    }

    /// セルまたはワールド内の全アクターのアニメーション・スキニング・剛体アタッチメントを更新する。
    ///
    /// 参照元: Gamebryo 2.6 `NiControllerSequence::Update` → `NiSkinInstance::Update`
    pub fn update_actors(
        &mut self,
        dt: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        for actor in &mut self.actors {
            actor.update(dt, device, queue, &mut self.meshes);
        }
    }

    /// セルまたはワールド描画シーンに独立したアクター（NPC）を追加インスタンス化する。
    ///
    /// 参照元: Gamebryo 2.6 `NiNode::AttachChild`, Fallout 3 `ACHR` 配置アクター仕様
    pub fn add_actor(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        context: &RenderContext,
        vfs: &mut VfsManager,
        form_id: u32,
        name: &str,
        world_transform: &NiTransform,
        skeleton_nif: Arc<NifFile>,
        parts: Vec<Arc<NifFile>>,
        kf_nif: Option<Arc<NifFile>>,
        anim_clip: Option<Arc<crate::animation::AnimationClip>>,
        texture_cache: &mut HashMap<String, GpuTexture>,
        hair_color: Option<[u8; 3]>,
        has_hat: bool,
        head_texture_override: Option<&GpuTexture>,
        head_geometry_morph: Option<&crate::facegen::GeometryMorph>,
        head_fg_sym: Option<&[f32]>,
        head_fg_asym: Option<&[f32]>,
    ) -> usize {
        let default_texture = GpuTexture::create_default_white(device, queue);
        let default_normal_texture = GpuTexture::create_default_normal(device, queue);
        let default_glow_texture = GpuTexture::create_default_black(device, queue);

        let hair_tint = hair_color.map(|c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0]);

        // 1. スケルトンの初期姿勢（T-Pose）におけるボーン名ワールド変換マップを構築
        let mut skel_bone_world_map = HashMap::new();
        let mut skel_bone_name_world_map = HashMap::new();
        let initial_pose = crate::animation::SkeletonPose::default();
        recompute_bone_world_maps_with_pose(
            &skeleton_nif,
            &initial_pose,
            &mut skel_bone_world_map,
            &mut skel_bone_name_world_map,
        );

        let actor_world_mat = world_transform.to_mat4();
        let mut actor_anim_skins = Vec::new();
        let mut actor_anim_rigids = Vec::new();

        // 2. 各パーツ NIF のメッシュを走査・生成
        for (part_idx, part_nif) in parts.iter().enumerate() {
            if part_nif.blocks.is_empty() {
                continue;
            }
            let mut part_bone_world_map = HashMap::new();

            // パーツが剛体アタッチメントボーン (例: "Bip01 Head", "Weapon") を指定しているか判定
            let is_weapon_part = part_nif.blocks.iter().any(|b| match b {
                NifBlock::NiNode(n) => {
                    let name = part_nif.get_string(n.av.net.name_index).unwrap_or("").to_ascii_lowercase();
                    name.contains("weap") || name.contains("pistol") || name.contains("rifle") || name.contains("gun")
                }
                NifBlock::BSFadeNode(n) => {
                    let name = part_nif.get_string(n.node.av.net.name_index).unwrap_or("").to_ascii_lowercase();
                    name.contains("weap") || name.contains("pistol") || name.contains("rifle") || name.contains("gun")
                }
                _ => false,
            });
            let has_skin = part_nif.blocks.iter().any(|b| matches!(b, NifBlock::NiSkinInstance(_) | NifBlock::BSDismemberSkinInstance(_)));
            let mut attach_bone_opt = find_attach_bone_name(part_nif);
            if attach_bone_opt.is_none() && is_weapon_part && !has_skin {
                attach_bone_opt = Some("Weapon".to_string());
            }

            let root_transform = if let Some(ref bone_name) = attach_bone_opt {
                if let Some(skel_world) = skel_bone_name_world_map.get(bone_name) {
                    NiTransform::from_mat4(actor_world_mat * (*skel_world))
                } else {
                    world_transform.clone()
                }
            } else {
                world_transform.clone()
            };

            collect_bone_world_transforms(0, &root_transform, part_nif, &mut part_bone_world_map);

            // 髪の毛パーツの判定: 剛体アタッチメントが "Bip01 Head" であり、かつ内部に "Hair" や "NoHat" / "Hat" シェイプを持つか
            let is_hair = part_nif.blocks.iter().any(|b| match b {
                NifBlock::NiTriShape(s) => {
                    let name = part_nif.get_string(s.geom.av.net.name_index).unwrap_or("").to_ascii_lowercase();
                    name.contains("hair") || name.contains("nohat") || name.contains("hat")
                }
                _ => false,
            });
            let tint_to_apply = if is_hair { hair_tint } else { None };

            let mut collector = ActorPartAnimCollector::new(
                part_idx,
                attach_bone_opt.as_deref(),
                is_hair,
                has_hat,
                tint_to_apply,
                head_texture_override,
                head_geometry_morph,
                head_fg_sym,
                head_fg_asym,
                part_nif,
                &mut actor_anim_skins,
                &mut actor_anim_rigids,
            );

            traverse_block(
                0,
                &root_transform,
                None,
                None,
                part_nif,
                vfs,
                device,
                queue,
                context,
                &mut self.meshes,
                texture_cache,
                &mut part_bone_world_map,
                Some(&skel_bone_name_world_map),
                &default_texture,
                &default_normal_texture,
                &default_glow_texture,
                Some(&mut collector),
            );
        }

        // 3. アニメーションプレイヤーの初期化
        let anim_player = anim_clip.as_ref().map(|c| AnimationPlayer::new((**c).clone()));

        // 4. 初期剛体パーツのワールド位置を行列バッファに即座に反映
        for rigid in &actor_anim_rigids {
            if rigid.mesh_index < self.meshes.len() {
                if let Some(bone_world) = skel_bone_name_world_map.get(&rigid.bone_name) {
                    let current_world = actor_world_mat * (*bone_world) * rigid.local_transform;
                    queue.write_buffer(
                        &self.meshes[rigid.mesh_index].model_uniform_buffer,
                        0,
                        bytemuck::cast_slice(&[current_world.to_cols_array_2d()]),
                    );
                }
            }
        }

        let actor_idx = self.actors.len();
        self.actors.push(RenderActorInstance {
            form_id,
            name: name.to_string(),
            world_transform: world_transform.clone(),
            skeleton_nif,
            parts,
            anim_player,
            kf_nif,
            sequence_manager: None,
            anim_pose: initial_pose,
            anim_skin_meshes: actor_anim_skins,
            anim_rigid_meshes: actor_anim_rigids,
        });

        actor_idx
    }
}
