//! # シーングラフ描画ノード構築
//!
//! NIF ファイル内の階層ノードを再帰的にトラバースし、
//! Gamebryo 2.6 準拠のワールドトランスフォーム合成を行い、
//! GPU 描画コマンドリスト (`RenderScene`) を構築する。
//! 参照元: Gamebryo 2.6 `NiAVObject::UpdateDownwardPass`

use std::collections::HashMap;
use fo3_gamebryo_core::NiTransform;
use fo3_nif::{NifBlock, NifFile};
use fo3_vfs::VfsManager;
use glam::Vec3;
use crate::skinning::apply_skinning_cpu;

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

/// NIF から構築された完全な描画シーン。
pub struct RenderScene {
    pub meshes: Vec<RenderMesh>,
    /// Havok コリジョンワイヤーフレームメッシュ群
    pub collision_meshes: Vec<GpuCollisionMesh>,
    /// シーン全体のバウンディング中心
    pub bounds_center: Vec3,
    /// シーン全体のバウンディング半径
    pub bounds_radius: f32,
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
        let default_texture = GpuTexture::create_default_white(device, queue);
        let default_normal_texture = GpuTexture::create_default_normal(device, queue);
        let default_glow_texture = GpuTexture::create_default_black(device, queue);

        // ルートブロック（通常 0 番）からトラバース開始
        let root_transform = NiTransform::default();
        if !nif.blocks.is_empty() {
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

        RenderScene {
            meshes,
            collision_meshes,
            bounds_center,
            bounds_radius,
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
                    // NiSkinInstance が存在する場合は CPU スキニングを適用する
                    // 参照元: knowledge/actor_and_skin_mesh.md, Gamebryo 2.6 NiSkinInstance::Update
                    let gpu_mesh = if shape.geom.skin_instance >= 0 {
                        if let Some(inst_idx) = Some(shape.geom.skin_instance as usize) {
                            if inst_idx < nif.blocks.len() {
                                if let NifBlock::NiSkinInstance(ref inst) = nif.blocks[inst_idx] {
                                    if let Some((pos, nrm)) = apply_skinning_cpu(data, inst, nif) {
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
    let rot = glam::Mat3::from_cols_array_2d(&av.rotation.m);
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

