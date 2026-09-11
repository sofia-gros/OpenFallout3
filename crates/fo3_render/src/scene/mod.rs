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
pub mod mesh;
pub mod traversal;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use fo3_gamebryo_core::NiTransform;
use fo3_nif::{NifBlock, NifFile};
use fo3_vfs::VfsManager;

use crate::collision::{extract_collision_lines, GpuCollisionMesh};
use crate::mesh::GpuMesh;
use crate::pipeline::{ModelUniform, RenderContext};
use crate::skinning::apply_skinning_cpu_with_bones;
use crate::texture::GpuTexture;

pub use actor::*;
pub use bones::*;
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
        }
    }

    /// 複数の配置済み NIF インスタンスとワールド変換から RenderScene を構築。
    pub fn from_placed_nifs(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        context: &RenderContext,
        placed_nifs: &[(&NifFile, NiTransform)],
        vfs: &mut VfsManager,
    ) -> Self {
        Self::from_cell(device, queue, context, placed_nifs, None, None, vfs)
    }

    /// 単一 ESM セルデータから 3D シーンを構築。
    pub fn from_cell(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        context: &RenderContext,
        placed_nifs: &[(&NifFile, NiTransform)],
        land_info: Option<(&fo3_esm::LandRecord, i32, i32)>,
        landscape_texture_map: Option<&HashMap<fo3_esm::FormId, (String, String)>>,
        vfs: &mut VfsManager,
    ) -> Self {
        Self::from_cells(
            device,
            queue,
            context,
            &[(placed_nifs, land_info)],
            landscape_texture_map,
            vfs,
        )
    }

    /// 複数の ESM セルデータから 3D シーンを構築（屋外 3x3 セルやワールドスペース表示に対応）。
    ///
    /// 参照元:
    /// - `references/openmw/components/esm4/loadland.hpp` (クアドラント 0..3, ATXT, VTXT)
    /// - `knowledge/worldspace_cells.md` (ワールドスペースと外部セルグリッド)
    pub fn from_cells(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        context: &RenderContext,
        cells_data: &[(&[(&NifFile, NiTransform)], Option<(&fo3_esm::LandRecord, i32, i32)>)],
        landscape_texture_map: Option<&HashMap<fo3_esm::FormId, (String, String)>>,
        vfs: &mut VfsManager,
    ) -> Self {
        let mut meshes = Vec::new();
        let mut collision_meshes = Vec::new();
        let mut texture_cache: HashMap<String, GpuTexture> = HashMap::new();
        let mut bone_world_map: HashMap<i32, Mat4> = HashMap::new();
        let default_texture = GpuTexture::create_default_white(device, queue);
        let default_normal_texture = GpuTexture::create_default_normal(device, queue);
        let default_glow_texture = GpuTexture::create_default_black(device, queue);

        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        let mut found = false;

        for (placed_nifs, land_info) in cells_data {
            // 1. 地形 (LAND) メッシュの生成と登録 (4クアドラント下地 + 追加レイヤーブレンド)
            if let Some((land, grid_x, grid_y)) = *land_info {
                let origin_x = grid_x as f32 * fo3_esm::LAND_REAL_SIZE;
                let origin_y = grid_y as f32 * fo3_esm::LAND_REAL_SIZE;

                // ① 下地ベーステクスチャ (BTXT) メッシュ (4 クアドラント)
                for q in 0..4 {
                    if let Some(gpu_mesh) = GpuMesh::from_land_quadrant(device, land, grid_x, grid_y, q) {
                        let form_id = land.base_textures[q];
                        let (diff_name, norm_name) = if form_id != fo3_esm::FormId(0) {
                            if let Some(tex_map) = landscape_texture_map {
                                if let Some((diff, norm)) = tex_map.get(&form_id) {
                                    (
                                        normalize_texture_path(diff),
                                        if norm.is_empty() {
                                            None
                                        } else {
                                            Some(normalize_texture_path(norm))
                                        },
                                    )
                                } else {
                                    ("textures\\landscape\\dirtwasteland01.dds".to_string(), None)
                                }
                            } else {
                                ("textures\\landscape\\dirtwasteland01.dds".to_string(), None)
                            }
                        } else {
                            ("textures\\landscape\\dirtwasteland01.dds".to_string(), None)
                        };

                        let diff_opt = Some(diff_name.clone());
                        ensure_texture_cached(&diff_opt, vfs, device, queue, &mut texture_cache);
                        ensure_texture_cached(&norm_name, vfs, device, queue, &mut texture_cache);

                        let diffuse_texture = texture_cache.get(&diff_name).unwrap_or(&default_texture);
                        let normal_texture = norm_name
                            .as_ref()
                            .and_then(|p| texture_cache.get(p))
                            .unwrap_or(&default_normal_texture);

                        let model_uniform = ModelUniform::new(glam::Mat4::IDENTITY, None, None, false);
                        let model_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some(&format!("Model Uniform Buffer: Landscape Q{}", q)),
                            contents: bytemuck::bytes_of(&model_uniform),
                            usage: wgpu::BufferUsages::UNIFORM,
                        });

                        let model_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some(&format!("Model Bind Group: Landscape Q{}", q)),
                            layout: &context.model_bind_group_layout,
                            entries: &[wgpu::BindGroupEntry {
                                binding: 0,
                                resource: model_uniform_buffer.as_entire_binding(),
                            }],
                        });

                        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some(&format!("Texture Bind Group: Landscape Q{}", q)),
                            layout: &context.texture_bind_group_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                                    },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 2,
                                    resource: wgpu::BindingResource::TextureView(&normal_texture.view),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 3,
                                    resource: wgpu::BindingResource::TextureView(&default_glow_texture.view),
                                },
                            ],
                        });

                        let q_offset_x = if q % 2 == 1 {
                            fo3_esm::LAND_REAL_SIZE * 0.75
                        } else {
                            fo3_esm::LAND_REAL_SIZE * 0.25
                        };
                        let q_offset_y = if q >= 2 {
                            fo3_esm::LAND_REAL_SIZE * 0.75
                        } else {
                            fo3_esm::LAND_REAL_SIZE * 0.25
                        };

                        meshes.push(RenderMesh {
                            name: format!("Landscape_Q{}_Cell_{}_{}", q, grid_x, grid_y),
                            mesh: gpu_mesh,
                            model_bind_group,
                            model_uniform_buffer,
                            texture_bind_group,
                            is_transparent: false,
                            alpha_sort: false,
                            world_center: Vec3::new(origin_x + q_offset_x, origin_y + q_offset_y, 0.0),
                        });
                    }
                }

                // ② 追加レイヤー (ATXT/VTXT: 道路・瓦礫・草) の半透明重畳描画
                for (l_idx, layer) in land.layers.iter().enumerate() {
                    if let Some(gpu_mesh) = GpuMesh::from_land_quadrant_layer(device, land, grid_x, grid_y, layer) {
                        let (diff_name, norm_name) = if layer.form_id != fo3_esm::FormId(0) {
                            if let Some(tex_map) = landscape_texture_map {
                                if let Some((diff, norm)) = tex_map.get(&layer.form_id) {
                                    (
                                        normalize_texture_path(diff),
                                        if norm.is_empty() {
                                            None
                                        } else {
                                            Some(normalize_texture_path(norm))
                                        },
                                    )
                                } else {
                                    ("textures\\landscape\\dirtwasteland01.dds".to_string(), None)
                                }
                            } else {
                                ("textures\\landscape\\dirtwasteland01.dds".to_string(), None)
                            }
                        } else {
                            ("textures\\landscape\\dirtwasteland01.dds".to_string(), None)
                        };

                        let diff_opt = Some(diff_name.clone());
                        ensure_texture_cached(&diff_opt, vfs, device, queue, &mut texture_cache);
                        ensure_texture_cached(&norm_name, vfs, device, queue, &mut texture_cache);

                        let diffuse_texture = texture_cache.get(&diff_name).unwrap_or(&default_texture);
                        let normal_texture = norm_name
                            .as_ref()
                            .and_then(|p| texture_cache.get(p))
                            .unwrap_or(&default_normal_texture);

                        let model_uniform = ModelUniform::new(glam::Mat4::IDENTITY, None, None, false);
                        let model_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some(&format!("Model Uniform Buffer: Landscape Layer Q{}_{}", layer.quadrant, l_idx)),
                            contents: bytemuck::bytes_of(&model_uniform),
                            usage: wgpu::BufferUsages::UNIFORM,
                        });

                        let model_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some(&format!("Model Bind Group: Landscape Layer Q{}_{}", layer.quadrant, l_idx)),
                            layout: &context.model_bind_group_layout,
                            entries: &[wgpu::BindGroupEntry {
                                binding: 0,
                                resource: model_uniform_buffer.as_entire_binding(),
                            }],
                        });

                        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some(&format!("Texture Bind Group: Landscape Layer Q{}_{}", layer.quadrant, l_idx)),
                            layout: &context.texture_bind_group_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 2,
                                    resource: wgpu::BindingResource::TextureView(&normal_texture.view),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 3,
                                    resource: wgpu::BindingResource::TextureView(&default_glow_texture.view),
                                },
                            ],
                        });

                        meshes.push(RenderMesh {
                            name: format!("Landscape_Q{}_Layer{}_Cell_{}_{}", layer.quadrant, l_idx, grid_x, grid_y),
                            mesh: gpu_mesh,
                            model_bind_group,
                            model_uniform_buffer,
                            texture_bind_group,
                            is_transparent: true,
                            alpha_sort: false,
                            world_center: Vec3::new(
                                origin_x + fo3_esm::LAND_REAL_SIZE * 0.5,
                                origin_y + fo3_esm::LAND_REAL_SIZE * 0.5,
                                0.0,
                            ),
                        });
                    }
                }

                // 地形のバウンディング反映
                let heights = land.compute_heights();
                for &h in &heights {
                    min.z = min.z.min(h);
                    max.z = max.z.max(h);
                }
                min.x = min.x.min(origin_x);
                max.x = max.x.max(origin_x + fo3_esm::LAND_REAL_SIZE);
                min.y = min.y.min(origin_y);
                max.y = max.y.max(origin_y + fo3_esm::LAND_REAL_SIZE);
                found = true;
            }

            // 2. 配置された 3D オブジェクト (REFR) の走査と登録
            for (nif, world_transform) in *placed_nifs {
                if !nif.blocks.is_empty() {
                    // NIF ファイルごとにボーン世界変換マップをクリア（ブロックインデックスは NIF ごとに独立）
                    bone_world_map.clear();
                    // プレパス: スキルトン階層全体のワールド変換を先に蓄積する
                    // (装備 NIF のように NiTriShape がボーン NiNode より前のブロック順で
                    //  配置されていても、スキニング時に全ボーン行列が解決済みとする)
                    // 参照元: knowledge/actor_and_skin_mesh.md, Gamebryo 2.6 NiAVObject::UpdateDownwardPass
                    collect_bone_world_transforms(0, world_transform, nif, &mut bone_world_map);
                    traverse_block(
                        0,
                        world_transform,
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

                    // コリジョンワイヤーフレームの抽出
                    let col_lines = extract_collision_lines(nif);
                    if let Some(gpu_col) = GpuCollisionMesh::new(device, &context.model_bind_group_layout, &col_lines, world_transform) {
                        collision_meshes.push(gpu_col);
                    }

                    let mat = world_transform.to_mat4();
                    for block in &nif.blocks {
                        match block {
                            NifBlock::NiTriShapeData(d) => {
                                for v in &d.common.vertices {
                                    let p = mat.transform_point3(Vec3::new(v.x, v.y, v.z));
                                    min = min.min(p);
                                    max = max.max(p);
                                    found = true;
                                }
                            }
                            NifBlock::NiTriStripsData(d) => {
                                for v in &d.common.vertices {
                                    let p = mat.transform_point3(Vec3::new(v.x, v.y, v.z));
                                    min = min.min(p);
                                    max = max.max(p);
                                    found = true;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        let (bounds_center, bounds_radius) = if found {
            let center = (min + max) * 0.5;
            let radius = (max - min).length() * 0.5;
            (center, radius.max(100.0))
        } else {
            (Vec3::ZERO, 100.0)
        };

        RenderScene {
            meshes,
            collision_meshes,
            bounds_center,
            bounds_radius,
            anim_skin_meshes: Vec::new(),
            anim_rigid_meshes: Vec::new(),
            actors: Vec::new(),
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
        // 1. 不透明メッシュ群の描画 (通常パイプライン: 深度書き込み有効)
        render_pass.set_pipeline(&context.pipeline);
        for mesh_node in self.meshes.iter().filter(|m| !m.is_transparent) {
            render_pass.set_bind_group(1, &mesh_node.model_bind_group, &[]);
            render_pass.set_bind_group(2, &mesh_node.texture_bind_group, &[]);
            render_pass.set_vertex_buffer(0, mesh_node.mesh.vertex_buffer.slice(..));
            render_pass.set_index_buffer(mesh_node.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..mesh_node.mesh.num_elements, 0, 0..1);
        }

        // 2. 半透明メッシュ群の描画 (半透明パイプライン: 深度書き込み無効、アルファブレンド)
        render_pass.set_pipeline(&context.transparent_pipeline);
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

        for mesh_node in transparent_meshes {
            render_pass.set_bind_group(1, &mesh_node.model_bind_group, &[]);
            render_pass.set_bind_group(2, &mesh_node.texture_bind_group, &[]);
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

    /// アニメーション更新後のボーン行列で、全スキンメッシュの頂点バッファを更新する。
    ///
    /// `AnimationPlayer::update()` → `recompute_bone_world_map_with_pose()` 後に呼び出すことで、
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
        parts: &[&NifFile],
        bone_name_world_map: &HashMap<String, Mat4>,
    ) {
        for anim in &self.anim_skin_meshes {
            let mesh_index = anim.mesh_index;
            if mesh_index >= self.meshes.len() { continue; }
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

            // スケルトンのボーン名から現在のボーン行列を解決してスキニング計算
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
        mesh_nif: &NifFile,
        bone_name_world_map: &HashMap<String, Mat4>,
    ) {
        self.update_animated_skins_multi_parts(device, &[mesh_nif], bone_name_world_map);
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
                    let s_name = part_nif.get_string(s.geom.av.net.name_index).unwrap_or("").to_ascii_lowercase();
                    s_name.contains("hair") || s_name.contains("nohat") || s_name == "hat"
                }
                _ => false,
            });

            let part_tint = if is_hair { hair_tint } else { None };

            let mut collector = ActorPartAnimCollector::new(
                part_idx,
                attach_bone_opt.as_deref(),
                is_hair,
                has_hat,
                part_tint,
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

        let anim_player = anim_clip.map(|clip| {
            crate::animation::AnimationPlayer::new((*clip).clone())
        });

        // 剛体アタッチメントパーツ (目・歯・舌・帽子・髪・武器) の初期姿勢 Uniform を同期
        for rigid in &actor_anim_rigids {
            if rigid.mesh_index < self.meshes.len() {
                if let Some(bone_world) = skel_bone_name_world_map.get(&rigid.bone_name) {
                    let current_world = actor_world_mat * (*bone_world * rigid.local_transform);
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
            anim_pose: initial_pose,
            anim_skin_meshes: actor_anim_skins,
            anim_rigid_meshes: actor_anim_rigids,
        });

        actor_idx
    }
}

/// 単一 NIF のアニメーション対応スキンメッシュ情報を収集する。
fn collect_anim_skin_meshes(nif: &NifFile, meshes: &[RenderMesh]) -> Vec<AnimatedSkinMesh> {
    let mesh_names: Vec<&str> = meshes.iter().map(|m| m.name.as_str()).collect();
    collect_anim_skin_meshes_for_names(0, 0, nif, &mesh_names)
}

/// 単一 NIF のバウンディング中心と球半径を計算する。
fn calculate_scene_bounds(nif: &NifFile) -> (Vec3, f32) {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut found = false;

    for block in &nif.blocks {
        match block {
            NifBlock::NiTriShapeData(d) => {
                for v in &d.common.vertices {
                    let p = Vec3::new(v.x, v.y, v.z);
                    min = min.min(p);
                    max = max.max(p);
                    found = true;
                }
            }
            NifBlock::NiTriStripsData(d) => {
                for v in &d.common.vertices {
                    let p = Vec3::new(v.x, v.y, v.z);
                    min = min.min(p);
                    max = max.max(p);
                    found = true;
                }
            }
            _ => {}
        }
    }

    if !found {
        return (Vec3::ZERO, 50.0);
    }

    let center = (min + max) * 0.5;
    let radius = (max - min).length() * 0.5;
    (center, radius.max(10.0))
}

/// 複数パーツ NIF のメッシュ頂点群を包含するバウンディング中心と半径を計算する。
fn calculate_scene_bounds_from_parts(parts: &[&NifFile]) -> (Vec3, f32) {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut found = false;

    for nif in parts {
        for block in &nif.blocks {
            match block {
                NifBlock::NiTriShapeData(d) => {
                    for v in &d.common.vertices {
                        let p = Vec3::new(v.x, v.y, v.z);
                        min = min.min(p);
                        max = max.max(p);
                        found = true;
                    }
                }
                NifBlock::NiTriStripsData(d) => {
                    for v in &d.common.vertices {
                        let p = Vec3::new(v.x, v.y, v.z);
                        min = min.min(p);
                        max = max.max(p);
                        found = true;
                    }
                }
                _ => {}
            }
        }
    }

    if !found {
        return (Vec3::ZERO, 50.0);
    }

    let center = (min + max) * 0.5;
    let radius = (max - min).length() * 0.5;
    (center, radius.max(10.0))
}
