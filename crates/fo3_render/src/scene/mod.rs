//! # シーングラフ描画ノード構築
//!
//! NIF ファイル内の階層ノードを再帰的にトラバースし、
//! Gamebryo 2.6 準拠のワールドトランスフォーム合成を行い、
//! GPU 描画コマンドリスト (`RenderScene`) を構築する。
//! 参照元: Gamebryo 2.6 `NiAVObject::UpdateDownwardPass`
//!
//! ## ボーン階層構築
//!
//! スキンメッシュのスキニング変形に必要なボーンのワールド変換行列は、
//! トラバース中に `bone_world_map` (block_index → Mat4) に蓄積される。
//! `NiSkinInstance.bones` の各ブロックインデックスからこのマップを参照し、
//! `apply_skinning_cpu_with_bones` に渡すことで、バインドポーズからの
//! 変形を正しく計算する。
//!
//! 参照元: `knowledge/actor_and_skin_mesh.md`, Gamebryo 2.6 `NiSkinInstance::Update`

pub mod actor;
pub mod bones;
pub mod bounds;
pub mod cell;
pub mod mesh;
pub mod traversal;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use glam::{Mat4, Vec3};

use fo3_gamebryo_core::NiTransform;
pub use fo3_nif::{NifBlock, NifFile};
use fo3_vfs::VfsManager;

use crate::collision::{extract_collision_lines, GpuCollisionMesh};
use crate::pipeline::RenderContext;
use crate::texture::GpuTexture;

pub use actor::*;
pub use bones::*;
pub use bounds::*;
pub use mesh::*;
pub use traversal::*;

/// NIF から構築された完全な描画シーン。
pub struct RenderScene {
    pub meshes: Vec<RenderMesh>,
    /// Havok コリジョンワイヤーフレームメッシュ群
    pub collision_meshes: Vec<GpuCollisionMesh>,
    /// シーン全体のバウンディング中心
    pub bounds_center: Vec3,
    /// シーン全体のバウンディング半径
    pub bounds_radius: f32,
    /// アニメーション対応スキンメッシュの更新情報リスト
    pub anim_skin_meshes: Vec<AnimatedSkinMesh>,
    /// アニメーション対応剛体アタッチメントメッシュの更新情報リスト (目・歯・舌など)
    pub anim_rigid_meshes: Vec<AnimatedRigidMesh>,
    /// セルまたはワールド内に配置された独立アクター群
    pub actors: Vec<RenderActorInstance>,
    /// 配置オブジェクト (REFR) ごとの生成メッシュインデックス範囲リスト
    pub refr_mesh_ranges: Vec<std::ops::Range<usize>>,
}

impl RenderScene {
    /// NIF ファイルと VFS から RenderScene を構築。
    pub fn from_nif(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        context: &RenderContext,
        nif: &NifFile,
        vfs: &mut VfsManager,
    ) -> Self {
        let mut meshes = Vec::new();
        let mut texture_cache: HashMap<String, GpuTexture> = HashMap::new();
        let mut bone_world_map: HashMap<i32, Mat4> = HashMap::new();
        let default_texture = GpuTexture::create_default_white(device, queue);
        let default_normal_texture = GpuTexture::create_default_normal(device, queue);
        let default_glow_texture = GpuTexture::create_default_black(device, queue);

        // ルートブロック（通常 0 番）からトラバース開始
        let root_transform = NiTransform::default();

        // プレパス: スケルトン階層全体のワールド変換を先に蓄積する。
        // NiTriShape がボーン NiNode より先のブロック順 / 子ノード配列順で
        // 現れる NIF ファイルでも、スキニング時に全ボーン行列が解決済みであることを保証する。
        // 参照元: knowledge/actor_and_skin_mesh.md, Gamebryo 2.6 NiAVObject::UpdateDownwardPass
        if !nif.blocks.is_empty() {
            collect_bone_world_transforms(0, &root_transform, nif, &mut bone_world_map);
            traverse_block(
                0,
                &root_transform,
                None,
                None,
                nif,
                vfs,
                device,
                queue,
                context,
                &mut meshes,
                &mut texture_cache,
                &mut bone_world_map,
                None,
                &default_texture,
                &default_normal_texture,
                &default_glow_texture,
                None,
            );
        }

        // コリジョンワイヤーフレームの抽出
        let mut collision_meshes = Vec::new();
        let col_lines = extract_collision_lines(nif);
        if let Some(gpu_col) = GpuCollisionMesh::new(device, &context.model_bind_group_layout, &col_lines, &root_transform) {
            collision_meshes.push(gpu_col);
        }

        // バウンディング計算 (簡易 AABB から包含球を概算)
        let (bounds_center, bounds_radius) = calculate_scene_bounds(nif);

        // アニメーション対応スキンメッシュ情報を収集
        let anim_skin_meshes = collect_anim_skin_meshes(nif, &meshes);

        RenderScene {
            meshes,
            collision_meshes,
            bounds_center,
            bounds_radius,
            anim_skin_meshes,
            anim_rigid_meshes: Vec::new(),
            actors: Vec::new(),
            refr_mesh_ranges: Vec::new(),
        }
    }

    /// 複数パーツ NIF とスケルトン NIF からキャラクタ（人型アクター）用 RenderScene を構築する。
    ///
    /// Fallout 3 の人型キャラクタは、スケルトン (`skeleton.nif`) に
    /// 複数のパーツ NIF（頭部 `headhuman.nif`、胴体 `upperbody.nif` / 衣装、右手 `righthand.nif`、左手 `lefthand.nif` 等）を
    /// 結合（アセンブリ）して構成される。
    /// 各パーツのスキンメッシュは、スケルトン側の同一ボーン名ノードのワールド変換によって変形される。
    ///
    /// 参照元:
    /// - Gamebryo 2.6 キャラクタパーツ合成 (`NiActorManager` / シーングラフ結合)
    /// - `knowledge/actor_and_skin_mesh.md` (セクション 4.4, 4.5)
    pub fn from_actor_parts(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        context: &RenderContext,
        skeleton_nif: &NifFile,
        parts: &[&NifFile],
        vfs: &mut VfsManager,
    ) -> Self {
        let mut meshes = Vec::new();
        let mut texture_cache: HashMap<String, GpuTexture> = HashMap::new();
        let default_texture = GpuTexture::create_default_white(device, queue);
        let default_normal_texture = GpuTexture::create_default_normal(device, queue);
        let default_glow_texture = GpuTexture::create_default_black(device, queue);

        // 1. スケルトンの初期姿勢（T-Pose）におけるボーン名ワールド変換マップを構築
        let mut skel_bone_world_map = HashMap::new();
        let mut skel_bone_name_world_map = HashMap::new();
        let initial_pose = crate::animation::SkeletonPose::default();
        crate::recompute_bone_world_maps_with_pose(
            skeleton_nif,
            &initial_pose,
            &mut skel_bone_world_map,
            &mut skel_bone_name_world_map,
        );

        let mut anim_skin_meshes = Vec::new();
        let mut anim_rigid_meshes = Vec::new();

        // 2. 各パーツ NIF のメッシュを走査・生成
        for (part_idx, part_nif) in parts.iter().enumerate() {
            if part_nif.blocks.is_empty() {
                continue;
            }
            let mut part_bone_world_map = HashMap::new();

            // パーツが剛体アタッチメントボーン (例: "Bip01 Head") を指定しているか判定
            let attach_bone_opt = find_attach_bone_name(part_nif);
            let root_transform = if let Some(ref bone_name) = attach_bone_opt {
                if let Some(skel_world) = skel_bone_name_world_map.get(bone_name) {
                    NiTransform::from_mat4(*skel_world)
                } else {
                    NiTransform::default()
                }
            } else {
                NiTransform::default()
            };

            // パーツ自身のローカルボーンマップも収集（フォールバック用）
            collect_bone_world_transforms(0, &root_transform, part_nif, &mut part_bone_world_map);

            let mut collector = ActorPartAnimCollector::new(
                part_idx,
                attach_bone_opt.as_deref(),
                false,
                false,
                None,
                None,
                None,
                None,
                None,
                part_nif,
                &mut anim_skin_meshes,
                &mut anim_rigid_meshes,
            );

            // スケルトンのボーン名マップを優先して初期スキニング
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
                &mut meshes,
                &mut texture_cache,
                &mut part_bone_world_map,
                Some(&skel_bone_name_world_map),
                &default_texture,
                &default_normal_texture,
                &default_glow_texture,
                Some(&mut collector),
            );
        }

        // コリジョンワイヤーフレームの抽出（スケルトンから）
        let mut collision_meshes = Vec::new();
        let col_lines = extract_collision_lines(skeleton_nif);
        let root_transform = NiTransform::default();
        if let Some(gpu_col) = GpuCollisionMesh::new(device, &context.model_bind_group_layout, &col_lines, &root_transform) {
            collision_meshes.push(gpu_col);
        }

        // バウンディング計算（全パーツの AABB 包含）
        let (bounds_center, bounds_radius) = calculate_scene_bounds_from_parts(parts);

        RenderScene {
            meshes,
            collision_meshes,
            bounds_center,
            bounds_radius,
            anim_skin_meshes,
            anim_rigid_meshes,
            actors: Vec::new(),
            refr_mesh_ranges: Vec::new(),
        }
    }

    /// シーン内のすべてのメッシュを描画する（不透明パス → 半透明パス）。
    /// 参照元: Gamebryo 2.6 レンダリング順序（不透明オブジェクトを先に深度書き込みありで描画し、その後半透明オブジェクトを合成）
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, context: &'a RenderContext) {
        self.render_with_camera_pos(render_pass, context, None);
    }

    /// カメラ位置を考慮してシーンを描画（半透明メッシュをカメラから遠い順にソートして合成）。
    /// 参照元: Gamebryo 2.6 `NiAccumulator::RecordObject` (奥から手前へのソート描画)
    pub fn render_with_camera_pos<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        context: &'a RenderContext,
        camera_pos: Option<Vec3>,
    ) {
        // 1. 不透明メッシュ群の描画 (深度書き込み有効)
        let mut current_skinned: Option<bool> = None;
        for mesh_node in self.meshes.iter().filter(|m| !m.is_transparent) {
            let is_skinned = mesh_node.bone_palette.is_some();
            if current_skinned != Some(is_skinned) {
                render_pass.set_pipeline(if is_skinned {
                    &context.skinned_pipeline
                } else {
                    &context.pipeline
                });
                current_skinned = Some(is_skinned);
            }

            render_pass.set_bind_group(1, &mesh_node.model_bind_group, &[]);
            render_pass.set_bind_group(2, &mesh_node.texture_bind_group, &[]);
            if let Some(ref bp) = mesh_node.bone_palette {
                render_pass.set_bind_group(3, &bp.bind_group, &[]);
            }
            render_pass.set_vertex_buffer(0, mesh_node.mesh.vertex_buffer.slice(..));
            render_pass.set_index_buffer(mesh_node.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..mesh_node.mesh.num_elements, 0, 0..1);
        }

        // 2. 半透明メッシュ群の描画 (深度書き込み無効、アルファブレンド)
        let mut transparent_meshes: Vec<&RenderMesh> = self.meshes.iter().filter(|m| m.is_transparent).collect();

        // ソートが要求されている（!is_no_sorter()）かつカメラ座標が与えられている場合、カメラから遠い順（降順）にソート
        // 参照元: references/nifskope/src/gl/glproperty.cpp:L230, Gamebryo 2.6 NiAlphaProperty
        if let Some(cam) = camera_pos {
            transparent_meshes.sort_by(|a, b| {
                if a.alpha_sort && b.alpha_sort {
                    let dist_a = a.world_center.distance_squared(cam);
                    let dist_b = b.world_center.distance_squared(cam);
                    dist_b.partial_cmp(&dist_a).unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    std::cmp::Ordering::Equal
                }
            });
        }

        let mut current_trans_skinned: Option<bool> = None;
        for mesh_node in transparent_meshes {
            let is_skinned = mesh_node.bone_palette.is_some();
            if current_trans_skinned != Some(is_skinned) {
                render_pass.set_pipeline(if is_skinned {
                    &context.transparent_skinned_pipeline
                } else {
                    &context.transparent_pipeline
                });
                current_trans_skinned = Some(is_skinned);
            }

            render_pass.set_bind_group(1, &mesh_node.model_bind_group, &[]);
            render_pass.set_bind_group(2, &mesh_node.texture_bind_group, &[]);
            if let Some(ref bp) = mesh_node.bone_palette {
                render_pass.set_bind_group(3, &bp.bind_group, &[]);
            }
            render_pass.set_vertex_buffer(0, mesh_node.mesh.vertex_buffer.slice(..));
            render_pass.set_index_buffer(mesh_node.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..mesh_node.mesh.num_elements, 0, 0..1);
        }
    }

    /// シーン内のすべての Havok コリジョンワイヤーフレームを描画する。
    pub fn render_collision<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, context: &'a RenderContext) {
        render_pass.set_pipeline(&context.collision_pipeline);
        for col_mesh in &self.collision_meshes {
            render_pass.set_bind_group(1, &col_mesh.model_bind_group, &[]);
            render_pass.set_vertex_buffer(0, col_mesh.vertex_buffer.slice(..));
            render_pass.draw(0..col_mesh.num_vertices, 0..1);
        }
    }
}

