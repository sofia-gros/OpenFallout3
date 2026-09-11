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

use std::collections::HashMap;
use fo3_gamebryo_core::NiTransform;
use fo3_nif::{NifBlock, NifFile};
use fo3_vfs::VfsManager;
use glam::{Mat4, Vec3};
use crate::skinning::apply_skinning_cpu_with_bones;

use wgpu::util::DeviceExt;

use crate::collision::{extract_collision_lines, GpuCollisionMesh};
use crate::mesh::GpuMesh;
use crate::pipeline::{ModelUniform, RenderContext};
use crate::texture::GpuTexture;

/// 単一のメッシュ描画単位。
pub struct RenderMesh {
    pub name: String,
    pub mesh: GpuMesh,
    pub model_bind_group: wgpu::BindGroup,
    pub texture_bind_group: wgpu::BindGroup,
    /// 半透明合成（Alpha Blending）を行うか
    pub is_transparent: bool,
    /// カメラ距離によるソートを行うか
    pub alpha_sort: bool,
    /// メッシュのワールド空間中心座標 (ソート用)
    pub world_center: Vec3,
}

/// アニメーション対応スキンメッシュの更新用情報。
///
/// スキニング計算に必要な NIF データへの参照（ブロックインデックス）を保持し、
/// 毎フレームの姿勢更新時に `RenderScene::update_animated_skins` から参照される。
///
/// 参照元: Gamebryo 2.6 `NiSkinInstance::Update`（毎フレームの変形計算）
pub struct AnimatedSkinMesh {
    /// `RenderScene::meshes` 中の対象 `RenderMesh` インデックス
    pub mesh_index: usize,
    /// NiTriShapeData ブロックインデックス（NIF 内）
    pub geo_data_block: i32,
    /// NiSkinInstance ブロックインデックス（NIF 内）
    pub skin_instance_block: i32,
}

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
                &default_texture,
                &default_normal_texture,
                &default_glow_texture,
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
                        &default_texture,
                        &default_normal_texture,
                        &default_glow_texture,
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
}

fn traverse_block(
    block_index: i32,
    parent_world: &NiTransform,
    parent_alpha: Option<&fo3_nif::NiAlphaProperty>,
    parent_material: Option<&fo3_nif::NiMaterialProperty>,
    nif: &NifFile,
    vfs: &mut VfsManager,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    context: &RenderContext,
    out_meshes: &mut Vec<RenderMesh>,
    texture_cache: &mut HashMap<String, GpuTexture>,
    bone_world_map: &mut HashMap<i32, Mat4>,
    default_texture: &GpuTexture,
    default_normal_texture: &GpuTexture,
    default_glow_texture: &GpuTexture,
) {
    if block_index < 0 || block_index as usize >= nif.blocks.len() {
        return;
    }

    let block = &nif.blocks[block_index as usize];
    match block {
        NifBlock::NiNode(node) => {
            if is_node_hidden(&node.av, nif) {
                return;
            }
            let local_transform = to_core_transform(&node.av);
            let world_transform = parent_world.compose(&local_transform);
            // ボーン階層構築: 各 NiNode のワールド変換行列を block_index → Mat4 で蓄積
            // 参照元: Gamebryo 2.6 NiAVObject::UpdateDownwardPass
            bone_world_map.insert(block_index, world_transform.to_mat4());
            let current_alpha = find_alpha_property(&node.av.properties, nif).or(parent_alpha);
            let current_material = find_material_property(&node.av.properties, nif).or(parent_material);
            for &child in &node.children {
                traverse_block(
                    child,
                    &world_transform,
                    current_alpha,
                    current_material,
                    nif,
                    vfs,
                    device,
                    queue,
                    context,
                    out_meshes,
                    texture_cache,
                    bone_world_map,
                    default_texture,
                    default_normal_texture,
                    default_glow_texture,
                );
            }
        }
        NifBlock::BSFadeNode(fade) => {
            if is_node_hidden(&fade.node.av, nif) {
                return;
            }
            let local_transform = to_core_transform(&fade.node.av);
            let world_transform = parent_world.compose(&local_transform);
            // BSFadeNode もボーン階層に含まれる場合があるため蓄積
            bone_world_map.insert(block_index, world_transform.to_mat4());
            let current_alpha = find_alpha_property(&fade.node.av.properties, nif).or(parent_alpha);
            let current_material = find_material_property(&fade.node.av.properties, nif).or(parent_material);
            for &child in &fade.node.children {
                traverse_block(
                    child,
                    &world_transform,
                    current_alpha,
                    current_material,
                    nif,
                    vfs,
                    device,
                    queue,
                    context,
                    out_meshes,
                    texture_cache,
                    bone_world_map,
                    default_texture,
                    default_normal_texture,
                    default_glow_texture,
                );
            }
        }
        NifBlock::NiTriShape(shape) => {
            if is_node_hidden(&shape.geom.av, nif) {
                return;
            }
            let local_transform = to_core_transform(&shape.geom.av);
            let world_transform = parent_world.compose(&local_transform);
            let name = nif.get_string(shape.geom.av.net.name_index).unwrap_or("").to_string();

            if shape.geom.data >= 0 && (shape.geom.data as usize) < nif.blocks.len() {
                if let NifBlock::NiTriShapeData(ref data) = nif.blocks[shape.geom.data as usize] {
                    // NiSkinInstance / BSDismemberSkinInstance が存在する場合は CPU スキニングを適用する
                    // 参照元: knowledge/actor_and_skin_mesh.md, Gamebryo 2.6 NiSkinInstance::Update
                    let gpu_mesh = if shape.geom.skin_instance >= 0 {
                        let inst_idx = shape.geom.skin_instance as usize;
                        if inst_idx < nif.blocks.len() {
                            let skin_inst_ref = match &nif.blocks[inst_idx] {
                                NifBlock::NiSkinInstance(ref inst) => Some(inst),
                                NifBlock::BSDismemberSkinInstance(ref bdsi) => Some(&bdsi.skin_instance),
                                _ => None,
                            };
                            if let Some(inst) = skin_inst_ref {
                                // NiSkinInstance.bones から各ボーンのワールド行列を解決
                                let bone_transforms = resolve_bone_world_transforms(inst, bone_world_map);
                                let bone_refs = if bone_transforms.is_empty() {
                                    None
                                } else {
                                    Some(bone_transforms.as_slice())
                                };
                                if let Some((pos, nrm)) = apply_skinning_cpu_with_bones(data, inst, nif, bone_refs) {
                                    GpuMesh::from_tri_shape_skinned(device, data, &pos, &nrm)
                                } else {
                                    GpuMesh::from_tri_shape(device, data)
                                }
                            } else {
                                GpuMesh::from_tri_shape(device, data)
                            }
                        } else {
                            GpuMesh::from_tri_shape(device, data)
                        }
                    } else {
                        GpuMesh::from_tri_shape(device, data)
                    };


                    if let Some(gpu_mesh) = gpu_mesh {
                        let render_mesh = create_render_mesh(
                            device,
                            context,
                            &name,
                            gpu_mesh,
                            &world_transform,
                            &shape.geom.av.properties,
                            parent_alpha,
                            parent_material,
                            nif,
                            vfs,
                            queue,
                            texture_cache,
                            default_texture,
                            default_normal_texture,
                            default_glow_texture,
                        );
                        out_meshes.push(render_mesh);
                    }
                }
            }
        }

        NifBlock::NiTriStrips(strips) => {
            if is_node_hidden(&strips.geom.av, nif) {
                return;
            }
            let local_transform = to_core_transform(&strips.geom.av);
            let world_transform = parent_world.compose(&local_transform);
            let name = nif.get_string(strips.geom.av.net.name_index).unwrap_or("").to_string();

            if strips.geom.data >= 0 && (strips.geom.data as usize) < nif.blocks.len() {
                if let NifBlock::NiTriStripsData(ref data) = nif.blocks[strips.geom.data as usize] {
                    if let Some(gpu_mesh) = GpuMesh::from_tri_strips(device, data) {
                        let render_mesh = create_render_mesh(
                            device,
                            context,
                            &name,
                            gpu_mesh,
                            &world_transform,
                            &strips.geom.av.properties,
                            parent_alpha,
                            parent_material,
                            nif,
                            vfs,
                            queue,
                            texture_cache,
                            default_texture,
                            default_normal_texture,
                            default_glow_texture,
                        );
                        out_meshes.push(render_mesh);
                    }
                }
            }
        }
        _ => {}
    }
}

/// スケルトン階層の全 NiNode / BSFadeNode ワールド変換行列を `bone_world_map` に蓄積する。
///
/// `traverse_block` に先立って実行するプレパス。単一パスの深さ優先トラバースでは、
/// 装備 NIF (`meshes\armor\...\outfit*.nif`) のように NiTriShape ブロックがボーン
/// NiNode ブロックより先の位置・子ノード配列順で現れる場合、NiTriShape のスキニング
/// 処理時にボーンが未登録のまま `Mat4::IDENTITY` へフォールバックしてしまう。
/// このプレパスにより全ボーンのワールド変換が先に解決され、アニメーション適用時に
/// 常に正しい現在行列を参照できる。
///
/// 参照元: Gamebryo 2.6 `NiAVObject::UpdateDownwardPass`, `NiSkinInstance::Update`

/// スケルトン階層の全 NiNode / BSFadeNode ワールド変換行列を `bone_world_map` に蓄積する。
///
/// `traverse_block` に先立って実行するプレパス。単一パスの深さ優先トラバースでは、
/// 装備 NIF (`meshes\armor\...\outfit*.nif`) のように NiTriShape ブロックがボーン
/// NiNode ブロックより先の位置・子ノード配列順で現れる場合、NiTriShape のスキニング
/// 処理時にボーンが未登録のまま `Mat4::IDENTITY` へフォールバックしてしまう。
/// このプレパスにより全ボーンのワールド変換が先に解決され、アニメーション適用時に
/// 常に正しい現在行列を参照できる。
///
/// 参照元: Gamebryo 2.6 `NiAVObject::UpdateDownwardPass`, `NiSkinInstance::Update`
pub fn collect_bone_world_transforms(
    block_index: i32,
    parent_world: &NiTransform,
    nif: &NifFile,
    bone_world_map: &mut HashMap<i32, Mat4>,
) {
    if block_index < 0 || block_index as usize >= nif.blocks.len() {
        return;
    }

    let block = &nif.blocks[block_index as usize];
    match block {
        NifBlock::NiNode(node) => {
            if is_node_hidden(&node.av, nif) {
                return;
            }
            let local_transform = to_core_transform(&node.av);
            let world_transform = parent_world.compose(&local_transform);
            bone_world_map.insert(block_index, world_transform.to_mat4());
            for &child in &node.children {
                collect_bone_world_transforms(child, &world_transform, nif, bone_world_map);
            }
        }
        NifBlock::BSFadeNode(fade) => {
            if is_node_hidden(&fade.node.av, nif) {
                return;
            }
            let local_transform = to_core_transform(&fade.node.av);
            let world_transform = parent_world.compose(&local_transform);
            bone_world_map.insert(block_index, world_transform.to_mat4());
            for &child in &fade.node.children {
                collect_bone_world_transforms(child, &world_transform, nif, bone_world_map);
            }
        }
        _ => {}
    }
}

/// スケルトン NIF の Forward Kinematics (FK) を再計算し、
/// ブロックインデックスマップ (`bone_world_map`) およびボーン名マップ (`bone_name_world_map`) の両方を更新する。
///
/// 参照元:
/// - Gamebryo 2.6 `NiControllerSequence::Update` (ボーンローカル変換の適用)
/// - Gamebryo 2.6 `NiAVObject::UpdateDownwardPass` (FK ワールド合成)
/// - `knowledge/actor_and_skin_mesh.md` (セクション 4.4: スケルトン分離とボーン名マッピング)
pub fn recompute_bone_world_maps_with_pose(
    nif: &NifFile,
    pose: &crate::animation::SkeletonPose,
    bone_world_map: &mut HashMap<i32, Mat4>,
    bone_name_world_map: &mut HashMap<String, Mat4>,
) {
    bone_world_map.clear();
    bone_name_world_map.clear();
    recompute_fk_with_names(0, &NiTransform::default(), nif, pose, bone_world_map, bone_name_world_map);
}

/// NIF 内のボーン階層をアニメーション姿勢 (`pose`) で順運動学 (FK) 再計算し、
/// 各ボーンノードのワールド変換行列を `bone_world_map` に格納する。
///
/// 参照元:
/// - Gamebryo 2.6 `NiControllerSequence::Update` (ボーンローカル変換の適用)
/// - Gamebryo 2.6 `NiAVObject::UpdateDownwardPass` (FK ワールド合成)
/// - `knowledge/animation_kf_format.md` (セクション 5)
pub fn recompute_bone_world_map_with_pose(
    nif: &NifFile,
    pose: &crate::animation::SkeletonPose,
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
    pose: &crate::animation::SkeletonPose,
    bone_world_map: &mut HashMap<i32, Mat4>,
    bone_name_world_map: &mut HashMap<String, Mat4>,
) {
    if block_index < 0 || block_index as usize >= nif.blocks.len() {
        return;
    }

    let block = &nif.blocks[block_index as usize];
    match block {
        NifBlock::NiNode(node) => {
            if is_node_hidden(&node.av, nif) {
                return;
            }
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
            if is_node_hidden(&fade.node.av, nif) {
                return;
            }
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
fn lookup_override(
    base: NiTransform,
    av: &fo3_nif::NiAVObject,
    nif: &NifFile,
    pose: &crate::animation::SkeletonPose,
) -> NiTransform {
    if let Some(name) = nif.get_string(av.net.name_index) {
        if let Some(t) = pose.overrides.get(name) {
            return *t;
        }
    }
    base
}

/// `NiSkinInstance.bones` の各ブロックインデックスからパーツ NIF 側のボーンノード名を取得し、
/// スケルトン側で計算されたボーン名→ワールド行列マップ (`bone_name_world_map`) から行列配列を解決する。
///
/// スケルトン側に同名ボーンが存在しない場合は、フォールバックとして `fallback_bone_world_map` を参照し、
/// それもなければ `Mat4::IDENTITY` を返す。
///
/// 参照元:
/// - Gamebryo 2.6 `NiSkinInstance::Update`
/// - `knowledge/actor_and_skin_mesh.md` (セクション 4.4)
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

/// `NiSkinInstance.bones` の各ブロックインデックスからワールド変換行列配列を解決する。
///
/// `bone_world_map` (traverse_block 中に蓄積された block_index → Mat4) を参照し、
/// `NiSkinInstance.bones[i]` が指す各 NiNode の現在のワールド行列を返す。
/// マップに存在しないボーンは `Mat4::IDENTITY` で埋める。
///
/// 参照元: `knowledge/actor_and_skin_mesh.md`, Gamebryo 2.6 `NiSkinInstance::Update`
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

/// NIF 内のスキンメッシュを走査し、アニメーション更新に必要な情報を収集する。
///
/// `RenderScene::anim_skin_meshes` の初期化に使用される。スキン情報（`NiSkinInstance`
/// または `BSDismemberSkinInstance`）を持つ `NiTriShape` ブロックを検出し、
/// `RenderScene::meshes` 内の対応インデックスとともに `AnimatedSkinMesh` を構築する。
///
/// 参照元: Gamebryo 2.6 `NiSkinInstance::Update`
fn collect_anim_skin_meshes(nif: &NifFile, meshes: &[RenderMesh]) -> Vec<AnimatedSkinMesh> {
    let mut result = Vec::new();
    for (block_idx, block) in nif.blocks.iter().enumerate() {
        let NifBlock::NiTriShape(shape) = block else { continue };
        let skin_inst_block = shape.geom.skin_instance;
        if skin_inst_block < 0 { continue; }
        let geo_data_block = shape.geom.data;
        if geo_data_block < 0 { continue; }
        // NIF ブロック名でメッシュを検索
        let mesh_name = nif.get_string(shape.geom.av.net.name_index).unwrap_or("");
        let block_hint = block_idx.to_string();
        // meshes 内で対応する RenderMesh を探す（名前一致 or ブロックインデックス含む）
        if let Some(mesh_index) = meshes.iter().position(|m| {
            m.name == mesh_name || m.name.contains(&block_hint)
        }) {
            result.push(AnimatedSkinMesh {
                mesh_index,
                geo_data_block,
                skin_instance_block: skin_inst_block,
            });
        }
    }
    result
}

impl RenderScene {
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

    /// スケルトン側のボーン名ワールド行列を用いて、パーツメッシュ NIF の全スキンメッシュの頂点バッファを更新する。
    ///
    /// キャラクタの衣装やボディパーツ（`upperbody.nif` 等）は、スケルトン NIF（`skeleton.nif`）で計算された
    /// 各ボーンノードのワールド変換をボーン名で引き当てて変形する。
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
        for anim in &self.anim_skin_meshes {
            let mesh_index = anim.mesh_index;
            if mesh_index >= self.meshes.len() { continue; }

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
}

/// ノードが非表示（App Culled / エディタマーカー）であるかを判定する。
///
/// 判定基準:
/// 1. `NiAVObject::flags` のビット 0 (`flags & 0x0001 != 0`): `Hidden` / `App Culled`
///    参照元: `references/openmw/components/nif/node.hpp:77`, `references/nifskope/src/gl/glnode.cpp:456`
/// 2. ノード名が `"EditorMarker"` や `"Marker"` で始まる (Creation Kit マーカー)
///    参照元: `references/nifskope/src/gl/glmesh.cpp:760`
fn is_node_hidden(av: &fo3_nif::NiAVObject, nif: &NifFile) -> bool {
    // 1. App Culled (Hidden) フラグ
    if av.flags & 0x0001 != 0 {
        return true;
    }

    // 2. エディタマーカー名判定
    if let Some(name) = nif.get_string(av.net.name_index) {
        let lower = name.to_ascii_lowercase();
        if lower.starts_with("editormarker") || lower.starts_with("marker") {
            return true;
        }
    }

    false
}

fn to_core_transform(av: &fo3_nif::NiAVObject) -> NiTransform {
    let rot = glam::Mat3::from_cols_array_2d(&av.rotation.m).transpose();
    NiTransform {
        rotation: rot,
        translation: glam::Vec3::new(av.translation.x, av.translation.y, av.translation.z),
        scale: av.scale,
    }
}

fn create_render_mesh(
    device: &wgpu::Device,
    context: &RenderContext,
    name: &str,
    mesh: GpuMesh,
    world_transform: &NiTransform,
    properties: &[i32],
    parent_alpha: Option<&fo3_nif::NiAlphaProperty>,
    parent_material: Option<&fo3_nif::NiMaterialProperty>,
    nif: &NifFile,
    vfs: &mut VfsManager,
    queue: &wgpu::Queue,
    texture_cache: &mut HashMap<String, GpuTexture>,
    default_texture: &GpuTexture,
    default_normal_texture: &GpuTexture,
    default_glow_texture: &GpuTexture,
) -> RenderMesh {
    // アルファプロパティの解決 (自身のプロパティ優先、無ければ親から継承)
    // 参照元: Gamebryo 2.6 NiAVObject::AttachProperty, Property Cascading
    let effective_alpha = find_alpha_property(properties, nif).or(parent_alpha);
    let is_transparent = effective_alpha.map_or(false, |a| a.is_blend_enabled());
    let alpha_sort = effective_alpha.map_or(false, |a| !a.is_no_sorter());

    // マテリアルプロパティの解決 (スペキュラ、エミッシブ、光沢度: 自身のプロパティ優先、無ければ親から継承)
    // 参照元: Gamebryo 2.6 NiMaterialProperty, Property Cascading, references/nifxml/nif.xml:L4363
    let material_prop = find_material_property(properties, nif).or(parent_material);

    // テクスチャ探索 (Slot 0: Diffuse, Slot 1: Normal Map, Slot 2: Glow Map)
    // 参照元: references/openmw/components/nifosg/nifloader.cpp:L2401-2426, references/nifxml/nif.xml:L6307
    let (diffuse_path, normal_path, glow_path) = find_texture_paths(properties, nif);
    ensure_texture_cached(&diffuse_path, vfs, device, queue, texture_cache);
    ensure_texture_cached(&normal_path, vfs, device, queue, texture_cache);
    ensure_texture_cached(&glow_path, vfs, device, queue, texture_cache);

    let has_glow_map = glow_path.is_some() && glow_path.as_ref().map_or(false, |p| texture_cache.contains_key(p));

    // Model Uniform バッファ作成
    let world_mat = world_transform.to_mat4();
    let model_uniform = ModelUniform::new(world_mat, effective_alpha, material_prop, has_glow_map);
    let model_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("Model Uniform Buffer: {}", name)),
        contents: bytemuck::bytes_of(&model_uniform),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let model_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(&format!("Model Bind Group: {}", name)),
        layout: &context.model_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: model_uniform_buffer.as_entire_binding(),
        }],
    });

    let diffuse_tex = diffuse_path
        .as_ref()
        .and_then(|p| texture_cache.get(p))
        .unwrap_or(default_texture);
    let normal_tex = normal_path
        .as_ref()
        .and_then(|p| texture_cache.get(p))
        .unwrap_or(default_normal_texture);
    let glow_tex = glow_path
        .as_ref()
        .and_then(|p| texture_cache.get(p))
        .unwrap_or(default_glow_texture);

    let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(&format!("Texture Bind Group: {}", name)),
        layout: &context.texture_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&diffuse_tex.view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&diffuse_tex.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&normal_tex.view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&glow_tex.view),
            },
        ],
    });

    RenderMesh {
        name: name.to_string(),
        mesh,
        model_bind_group,
        texture_bind_group,
        is_transparent,
        alpha_sort,
        world_center: world_transform.translation,
    }
}

/// テクスチャをキャッシュまたは VFS からロードしてキャッシュに格納する。
fn ensure_texture_cached(
    path_opt: &Option<String>,
    vfs: &mut VfsManager,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture_cache: &mut HashMap<String, GpuTexture>,
) {
    if let Some(ref path) = path_opt {
        if !texture_cache.contains_key(path) {
            match vfs.read(path) {
                Ok(bytes) => {
                    match GpuTexture::from_dds_bytes(device, queue, &bytes, Some(path)) {
                        Ok(tex) => {
                            texture_cache.insert(path.clone(), tex);
                        }
                        Err(e) => {
                            eprintln!("警告: テクスチャ '{}' のパース失敗: {}", path, e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("警告: VFS からテクスチャ '{}' を読み込めません: {}", path, e);
                }
            }
        }
    }
}

/// マテリアルプロパティからディフューズ (スロット 0)、法線マップ (スロット 1)、およびグローマップ (スロット 2) を取得。
/// 参照元: references/openmw/components/nifosg/nifloader.cpp:L2401-2426, references/nifxml/nif.xml:L6307
fn find_texture_paths(properties: &[i32], nif: &NifFile) -> (Option<String>, Option<String>, Option<String>) {
    for &prop_idx in properties {
        if prop_idx >= 0 && (prop_idx as usize) < nif.blocks.len() {
            if let NifBlock::BSShaderPPLightingProperty(ref shader_prop) = nif.blocks[prop_idx as usize] {
                if shader_prop.texture_set >= 0 && (shader_prop.texture_set as usize) < nif.blocks.len() {
                    if let NifBlock::BSShaderTextureSet(ref tex_set) = nif.blocks[shader_prop.texture_set as usize] {
                        let diff = if !tex_set.textures.is_empty() && !tex_set.textures[0].is_empty() {
                            Some(normalize_texture_path(&tex_set.textures[0]))
                        } else {
                            None
                        };
                        let norm = if tex_set.textures.len() > 1 && !tex_set.textures[1].is_empty() {
                            Some(normalize_texture_path(&tex_set.textures[1]))
                        } else {
                            None
                        };
                        let glow = if tex_set.textures.len() > 2 && !tex_set.textures[2].is_empty() {
                            Some(normalize_texture_path(&tex_set.textures[2]))
                        } else {
                            None
                        };
                        return (diff, norm, glow);
                    }
                }
            }
        }
    }
    (None, None, None)
}

/// プロパティリストから NiMaterialProperty を検索する。
/// 参照元: references/nifxml/nif.xml:L4363, Gamebryo 2.6 NiMaterialProperty
fn find_material_property<'a>(properties: &[i32], nif: &'a NifFile) -> Option<&'a fo3_nif::NiMaterialProperty> {
    for &prop_idx in properties {
        if prop_idx >= 0 && (prop_idx as usize) < nif.blocks.len() {
            if let NifBlock::NiMaterialProperty(ref mat) = nif.blocks[prop_idx as usize] {
                return Some(mat);
            }
        }
    }
    None
}

/// プロパティリストから NiAlphaProperty を検索する。
/// 参照元: references/nifskope/src/gl/glnode.cpp:295, references/nifxml/nif.xml:L3972
fn find_alpha_property<'a>(properties: &[i32], nif: &'a NifFile) -> Option<&'a fo3_nif::NiAlphaProperty> {
    for &prop_idx in properties {
        if prop_idx >= 0 && (prop_idx as usize) < nif.blocks.len() {
            if let NifBlock::NiAlphaProperty(ref alpha) = nif.blocks[prop_idx as usize] {
                return Some(alpha);
            }
        }
    }
    None
}

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

/// テクスチャパスを正規化（区切り文字をバックスラッシュに統一し、先頭に 'textures\' を補完）する。
/// BSA 内では 'textures/landscape/...' のように格納されているため。
fn normalize_texture_path(path: &str) -> String {
    let p = path.replace('/', "\\");
    if p.to_ascii_lowercase().starts_with("textures\\") {
        p
    } else {
        format!("textures\\{}", p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `resolve_bone_world_transforms` が `NiSkinInstance.bones` のブロックインデックスから
    /// 各ボーンのワールド変換行列を正しく解決できることを検証する。
    #[test]
    fn test_resolve_bone_world_transforms() {
        // block_index 0: ルートボーン "Bip01" → 並進 (1, 2, 3)
        // block_index 1: 子ボーン "Bip01 Pelvis" → 並進 (4, 5, 6)
        let mut bone_world_map = HashMap::new();
        bone_world_map.insert(
            0i32,
            Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0)),
        );
        bone_world_map.insert(
            1i32,
            Mat4::from_translation(Vec3::new(4.0, 5.0, 6.0)),
        );

        let skin_instance = fo3_nif::NiSkinInstance {
            data: -1,
            skin_partition: -1,
            skeleton_root: 0,
            bones: vec![0, 1],
        };

        let transforms = resolve_bone_world_transforms(&skin_instance, &bone_world_map);
        assert_eq!(transforms.len(), 2);
        // ボーン 0: 並進 (1, 2, 3) が保持されている
        assert_eq!(
            transforms[0].transform_point3(Vec3::ZERO),
            Vec3::new(1.0, 2.0, 3.0)
        );
        // ボーン 1: 並進 (4, 5, 6) が保持されている
        assert_eq!(
            transforms[1].transform_point3(Vec3::ZERO),
            Vec3::new(4.0, 5.0, 6.0)
        );
    }

    /// マップに存在しないブロックインデックスは `Mat4::IDENTITY` で補完されることを検証する。
    #[test]
    fn test_resolve_bone_missing_identity() {
        let mut bone_world_map = HashMap::new();
        bone_world_map.insert(10i32, Mat4::from_scale(Vec3::splat(2.0)));

        let skin_instance = fo3_nif::NiSkinInstance {
            data: -1,
            skin_partition: -1,
            skeleton_root: 10,
            bones: vec![10, 999], // 999 はマップに存在しない
        };

        let transforms = resolve_bone_world_transforms(&skin_instance, &bone_world_map);
        assert_eq!(transforms.len(), 2);
        assert_eq!(transforms[1], Mat4::IDENTITY);
    }

    /// プレパス `collect_bone_world_transforms` が、NiTriShape (メッシュ) ブロックが
    /// ボーン NiNode より先の子ノード配列順で現れる NIF でも全ボーンのワールド変換を
    /// 解決できることを検証する。
    ///
    /// 装備 NIF (`meshes\armor\...\outfit*.nif`) では NiTriShape が NiNode (ボーン) より
    /// ブロック先頭側に配置されるため、メッシュビルドより先に全ボーンを登録しておく必要がある。
    #[test]
    fn test_collect_bone_world_transforms_mesh_first_order() {
        use fo3_nif::blocks::{NiAVObject, NiNode, NiObjectNET};
        use fo3_nif::header::{BSStreamHeader, ExportString, NifHeader};
        use fo3_nif::NifFile;
        use fo3_nif::{Matrix33, Vector3};

        let make_av = |translation: Vector3| NiAVObject {
            net: NiObjectNET {
                name_index: 0,
                extra_data_list: vec![],
                controller: -1,
            },
            flags: 0,
            translation,
            rotation: Matrix33 {
                m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            },
            scale: 1.0,
            properties: vec![],
            collision_object: -1,
        };

        // ルート[0] の子ノード配列: 並び順は [メッシュ1, ボーン2]
        let root = NiNode {
            av: make_av(Vector3 { x: 0.0, y: 0.0, z: 0.0 }),
            children: vec![1, 2],
            effects: vec![],
        };
        // ボーン[2] "Bip01" はローカル並進 (10, 20, 30)
        let bone = NiNode {
            av: make_av(Vector3 { x: 10.0, y: 20.0, z: 30.0 }),
            children: vec![],
            effects: vec![],
        };

        let nif = NifFile {
            header: NifHeader {
                header_string: "Gamebryo File Format, Version 20.2.0.7\n".to_string(),
                version: 0x14020007,
                endian_type: 1,
                user_version: 11,
                num_blocks: 0,
                bs_header: BSStreamHeader {
                    bs_version: 34,
                    author: ExportString { value: String::new() },
                    process_script: None,
                    export_script: ExportString { value: String::new() },
                },
                block_types: vec![],
                block_type_indices: vec![],
                block_sizes: vec![],
                strings: vec![],
            },
            blocks: vec![
                NifBlock::NiNode(root),
                // スキンメッシュを模した未対応ブロック (メッシュが先)
                NifBlock::Unknown {
                    type_name: "NiTriShape".to_string(),
                    data: vec![],
                },
                NifBlock::NiNode(bone),
            ],
        };

        let mut bone_world_map = HashMap::new();
        collect_bone_world_transforms(0, &NiTransform::default(), &nif, &mut bone_world_map);

        // ルート[0] とボーン[2] の両方がワールド行列として登録されている
        assert!(bone_world_map.contains_key(&0));
        assert!(bone_world_map.contains_key(&2));
        // ボーン[2] のワールド並進が (10, 20, 30) として保持されている
        assert_eq!(
            bone_world_map[&2].transform_point3(Vec3::ZERO),
            Vec3::new(10.0, 20.0, 30.0)
        );
    }

    /// プレパスが BSFadeNode ルート (skeleton.nif の "Scene Root") でも
    /// 正しくボーン階層を登録できることを検証する。
    #[test]
    fn test_collect_bone_world_transforms_bsfade_root() {
        use fo3_nif::blocks::{BSFadeNode, NiAVObject, NiNode, NiObjectNET};
        use fo3_nif::header::{BSStreamHeader, ExportString, NifHeader};
        use fo3_nif::NifFile;
        use fo3_nif::{Matrix33, Vector3};

        let make_av = |translation: Vector3| NiAVObject {
            net: NiObjectNET {
                name_index: 0,
                extra_data_list: vec![],
                controller: -1,
            },
            flags: 0,
            translation,
            rotation: Matrix33 {
                m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            },
            scale: 1.0,
            properties: vec![],
            collision_object: -1,
        };

        // [0] BSFadeNode "Scene Root" (skeleton.nif のルート) → 子 [1]
        let fade_root = BSFadeNode {
            node: NiNode {
                av: make_av(Vector3 { x: 0.0, y: 0.0, z: 0.0 }),
                children: vec![1],
                effects: vec![],
            },
        };
        // [1] ボーン "Bip01" (ローカル並進 (1, 2, 3))
        let pelvis = NiNode {
            av: make_av(Vector3 { x: 1.0, y: 2.0, z: 3.0 }),
            children: vec![],
            effects: vec![],
        };

        let nif = NifFile {
            header: NifHeader {
                header_string: "Gamebryo File Format, Version 20.2.0.7\n".to_string(),
                version: 0x14020007,
                endian_type: 1,
                user_version: 11,
                num_blocks: 0,
                bs_header: BSStreamHeader {
                    bs_version: 34,
                    author: ExportString { value: String::new() },
                    process_script: None,
                    export_script: ExportString { value: String::new() },
                },
                block_types: vec![],
                block_type_indices: vec![],
                block_sizes: vec![],
                strings: vec![],
            },
            blocks: vec![NifBlock::BSFadeNode(fade_root), NifBlock::NiNode(pelvis)],
        };

        let mut bone_world_map = HashMap::new();
        collect_bone_world_transforms(0, &NiTransform::default(), &nif, &mut bone_world_map);

        assert!(bone_world_map.contains_key(&0));
        assert_eq!(
            bone_world_map[&1].transform_point3(Vec3::ZERO),
            Vec3::new(1.0, 2.0, 3.0)
        );
    }

    /// `recompute_bone_world_map_with_pose` が、姿勢のオーバーライドをボーンのローカル変換に
    /// 反映し、Forward Kinematics でワールド行列を再計算できることを検証する。
    #[test]
    fn test_recompute_bone_world_map_with_pose() {
        use crate::animation::SkeletonPose;
        use fo3_nif::blocks::{NiAVObject, NiNode, NiObjectNET};
        use fo3_nif::header::{BSStreamHeader, ExportString, NifHeader};
        use fo3_nif::NifFile;
        use fo3_nif::{Matrix33, Vector3};

        let make_av = |name_idx: u32, translation: Vector3| NiAVObject {
            net: NiObjectNET {
                name_index: name_idx,
                extra_data_list: vec![],
                controller: -1,
            },
            flags: 0,
            translation,
            rotation: Matrix33 {
                m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            },
            scale: 1.0,
            properties: vec![],
            collision_object: -1,
        };

        // [0] "Bip01": ローカル並進 (1, 2, 3) → 子 [1]
        let root = NiNode {
            av: make_av(1, Vector3 { x: 1.0, y: 2.0, z: 3.0 }),
            children: vec![1],
            effects: vec![],
        };
        // [1] "Bip01 Pelvis": ローカル並進 (4, 5, 6)
        let pelvis = NiNode {
            av: make_av(2, Vector3 { x: 4.0, y: 5.0, z: 6.0 }),
            children: vec![],
            effects: vec![],
        };

        // 文字列プール: 0 = "", 1 = "Bip01", 2 = "Bip01 Pelvis"
        let nif = NifFile {
            header: NifHeader {
                header_string: "Gamebryo File Format, Version 20.2.0.7\n".to_string(),
                version: 0x14020007,
                endian_type: 1,
                user_version: 11,
                num_blocks: 0,
                bs_header: BSStreamHeader {
                    bs_version: 34,
                    author: ExportString { value: String::new() },
                    process_script: None,
                    export_script: ExportString { value: String::new() },
                },
                block_types: vec![],
                block_type_indices: vec![],
                block_sizes: vec![],
                strings: vec![
                    String::new(),
                    "Bip01".to_string(),
                    "Bip01 Pelvis".to_string(),
                ],
            },
            blocks: vec![NifBlock::NiNode(root), NifBlock::NiNode(pelvis)],
        };

        // バインドポーズ FK: 骨盤のワールド並進 = (1+4, 2+5, 3+6) = (5, 7, 9)
        let mut map = HashMap::new();
        collect_bone_world_transforms(0, &NiTransform::default(), &nif, &mut map);
        assert_eq!(
            map[&1].transform_point3(Vec3::ZERO),
            Vec3::new(5.0, 7.0, 9.0)
        );

        // 姿勢: "Bip01 Pelvis" のローカル並進を (10, 20, 30) に上書き
        let mut pose = SkeletonPose::default();
        pose.overrides.insert(
            "Bip01 Pelvis".to_string(),
            NiTransform {
                rotation: glam::Mat3::IDENTITY,
                translation: Vec3::new(10.0, 20.0, 30.0),
                scale: 1.0,
            },
        );

        recompute_bone_world_map_with_pose(&nif, &pose, &mut map);
        // 骨盤のワールド並進 = (1+10, 2+20, 3+30) = (11, 22, 33)
        assert_eq!(
            map[&1].transform_point3(Vec3::ZERO),
            Vec3::new(11.0, 22.0, 33.0)
        );
        // ルート自体はポーズ対象外なのでバインドポーズのまま
        assert_eq!(
            map[&0].transform_point3(Vec3::ZERO),
            Vec3::new(1.0, 2.0, 3.0)
        );
    }

    /// `recompute_bone_world_maps_with_pose` と `resolve_bone_world_transforms_by_name` が、
    /// スケルトン側のノード名とパーツメッシュ側のボーン名を正しくマッチングしてワールド変換を解決することを検証する。
    ///
    /// 参照元: `knowledge/actor_and_skin_mesh.md` (セクション 4.4)
    #[test]
    fn test_recompute_bone_world_maps_with_pose_and_name_resolution() {
        use crate::animation::SkeletonPose;
        use fo3_nif::blocks::{NiAVObject, NiNode, NiObjectNET};
        use fo3_nif::header::{BSStreamHeader, ExportString, NifHeader};
        use fo3_nif::NiSkinInstance;
        use fo3_nif::NifFile;
        use fo3_nif::{Matrix33, Vector3};

        let make_av = |name_idx: u32, translation: Vector3| NiAVObject {
            net: NiObjectNET {
                name_index: name_idx,
                extra_data_list: vec![],
                controller: -1,
            },
            flags: 0,
            translation,
            rotation: Matrix33 {
                m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            },
            scale: 1.0,
            properties: vec![],
            collision_object: -1,
        };

        let root = NiNode {
            av: make_av(1, Vector3 { x: 0.0, y: 0.0, z: 0.0 }),
            children: vec![1],
            effects: vec![],
        };
        let pelvis = NiNode {
            av: make_av(2, Vector3 { x: 10.0, y: 0.0, z: 0.0 }),
            children: vec![],
            effects: vec![],
        };

        // スケルトン NIF: [0] = "Bip01", [1] = "Bip01 Pelvis"
        let skel_nif = NifFile {
            header: NifHeader {
                header_string: "Gamebryo File Format, Version 20.2.0.7\n".to_string(),
                version: 0x14020007,
                endian_type: 1,
                user_version: 11,
                num_blocks: 0,
                bs_header: BSStreamHeader {
                    bs_version: 34,
                    author: ExportString { value: String::new() },
                    process_script: None,
                    export_script: ExportString { value: String::new() },
                },
                block_types: vec![],
                block_type_indices: vec![],
                block_sizes: vec![],
                strings: vec![
                    String::new(),
                    "Bip01".to_string(),
                    "Bip01 Pelvis".to_string(),
                ],
            },
            blocks: vec![NifBlock::NiNode(root), NifBlock::NiNode(pelvis)],
        };

        // パーツメッシュ NIF: [0] = ダミーボーン "Bip01 Pelvis" (親子なし)
        let mesh_pelvis_bone = NiNode {
            av: make_av(1, Vector3 { x: 0.0, y: 0.0, z: 0.0 }),
            children: vec![],
            effects: vec![],
        };
        let mesh_nif = NifFile {
            header: NifHeader {
                header_string: "Gamebryo File Format, Version 20.2.0.7\n".to_string(),
                version: 0x14020007,
                endian_type: 1,
                user_version: 11,
                num_blocks: 0,
                bs_header: BSStreamHeader {
                    bs_version: 34,
                    author: ExportString { value: String::new() },
                    process_script: None,
                    export_script: ExportString { value: String::new() },
                },
                block_types: vec![],
                block_type_indices: vec![],
                block_sizes: vec![],
                strings: vec![
                    String::new(),
                    "Bip01 Pelvis".to_string(),
                ],
            },
            blocks: vec![NifBlock::NiNode(mesh_pelvis_bone)],
        };

        let mut pose = SkeletonPose::default();
        pose.overrides.insert(
            "Bip01 Pelvis".to_string(),
            NiTransform {
                rotation: glam::Mat3::IDENTITY,
                translation: Vec3::new(100.0, 200.0, 300.0),
                scale: 1.0,
            },
        );

        let mut bone_world_map = HashMap::new();
        let mut bone_name_world_map = HashMap::new();
        recompute_bone_world_maps_with_pose(
            &skel_nif,
            &pose,
            &mut bone_world_map,
            &mut bone_name_world_map,
        );

        // スケルトンのボーン名マップに "Bip01 Pelvis" のワールド座標 (100, 200, 300) が格納されていること
        assert!(bone_name_world_map.contains_key("Bip01 Pelvis"));
        assert_eq!(
            bone_name_world_map["Bip01 Pelvis"].transform_point3(Vec3::ZERO),
            Vec3::new(100.0, 200.0, 300.0)
        );

        // パーツメッシュの NiSkinInstance がボーン [0] ("Bip01 Pelvis") を参照しているとき
        let skin_instance = NiSkinInstance {
            data: 0,
            skin_partition: 0,
            skeleton_root: 0,
            bones: vec![0],
        };

        // 名前引きで解決すると、スケルトン側の (100, 200, 300) が得られること
        let resolved = resolve_bone_world_transforms_by_name(
            &skin_instance,
            &mesh_nif,
            &bone_name_world_map,
            None,
        );
        assert_eq!(resolved.len(), 1);
        assert_eq!(
            resolved[0].transform_point3(Vec3::ZERO),
            Vec3::new(100.0, 200.0, 300.0)
        );
    }
}

