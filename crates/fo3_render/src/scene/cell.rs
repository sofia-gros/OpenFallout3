//! セルおよび配置済みオブジェクト群からの 3D シーン構築。
//! 参照元:
//! - `references/openmw/components/esm4/loadland.hpp` (クアドラント 0..3, ATXT, VTXT)
//! - `knowledge/worldspace_cells.md` (ワールドスペースと外部セルグリッド)

use std::collections::HashMap;
use fo3_gamebryo_core::NiTransform;
use fo3_nif::{NifBlock, NifFile};
use fo3_vfs::VfsManager;
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::collision::{extract_collision_lines, GpuCollisionMesh};
use crate::mesh::GpuMesh;
use crate::pipeline::{ModelUniform, RenderContext};
use crate::scene::bones::collect_bone_world_transforms;
use crate::scene::mesh::{ensure_texture_cached, normalize_texture_path, RenderMesh};
use crate::scene::traversal::traverse_block;
use crate::texture::GpuTexture;
use super::RenderScene;

impl RenderScene {
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
        let mut texture_cache = HashMap::new();
        Self::from_cells(
            device,
            queue,
            context,
            &[(placed_nifs, land_info)],
            landscape_texture_map,
            vfs,
            &mut texture_cache,
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
        texture_cache: &mut HashMap<String, GpuTexture>,
    ) -> Self {
        let mut meshes = Vec::new();
        let mut refr_mesh_ranges = Vec::new();
        let mut collision_meshes = Vec::new();
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

                        ensure_texture_cached(&Some(diff_name.clone()), vfs, device, queue, texture_cache);
                        ensure_texture_cached(&norm_name, vfs, device, queue, texture_cache);

                        let diffuse_tex = texture_cache.get(&diff_name).unwrap_or(&default_texture);
                        let normal_tex = norm_name
                            .as_ref()
                            .and_then(|p| texture_cache.get(p))
                            .unwrap_or(&default_normal_texture);

                        let model_uniform = ModelUniform::new(glam::Mat4::IDENTITY, None, None, false);
                        let model_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Landscape Model Uniform Buffer"),
                            contents: bytemuck::bytes_of(&model_uniform),
                            usage: wgpu::BufferUsages::UNIFORM,
                        });

                        let model_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("Landscape Model Bind Group"),
                            layout: &context.model_bind_group_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: model_uniform_buffer.as_entire_binding(),
                                },
                            ],
                        });

                        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("Landscape Texture Bind Group"),
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
                                    resource: wgpu::BindingResource::TextureView(&default_glow_texture.view),
                                },
                            ],
                        });

                        let world_bound = gpu_mesh.bound;
                        meshes.push(RenderMesh {
                            name: format!("Landscape_Q{}_Cell_{}_{}", q, grid_x, grid_y),
                            mesh: gpu_mesh,
                            model_bind_group,
                            model_uniform_buffer,
                            texture_bind_group,
                            is_transparent: false,
                            alpha_sort: false,
                            world_center: world_bound.center,
                            world_bound,
                            bone_palette: None,
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

                        ensure_texture_cached(&Some(diff_name.clone()), vfs, device, queue, texture_cache);
                        ensure_texture_cached(&norm_name, vfs, device, queue, texture_cache);

                        let diffuse_tex = texture_cache.get(&diff_name).unwrap_or(&default_texture);
                        let normal_tex = norm_name
                            .as_ref()
                            .and_then(|p| texture_cache.get(p))
                            .unwrap_or(&default_normal_texture);

                        let model_uniform = ModelUniform::new(glam::Mat4::IDENTITY, None, None, false);
                        let model_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Landscape Layer Model Uniform Buffer"),
                            contents: bytemuck::bytes_of(&model_uniform),
                            usage: wgpu::BufferUsages::UNIFORM,
                        });

                        let model_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("Landscape Layer Model Bind Group"),
                            layout: &context.model_bind_group_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: model_uniform_buffer.as_entire_binding(),
                                },
                            ],
                        });

                        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("Landscape Layer Texture Bind Group"),
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
                                    resource: wgpu::BindingResource::TextureView(&default_glow_texture.view),
                                },
                            ],
                        });

                        let world_bound = gpu_mesh.bound;
                        meshes.push(RenderMesh {
                            name: format!("Landscape_Q{}_Layer{}_Cell_{}_{}", layer.quadrant, l_idx, grid_x, grid_y),
                            mesh: gpu_mesh,
                            model_bind_group,
                            model_uniform_buffer,
                            texture_bind_group,
                            is_transparent: true,
                            alpha_sort: false,
                            world_center: world_bound.center,
                            world_bound,
                            bone_palette: None,
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
                let start_idx = meshes.len();
                if !nif.blocks.is_empty() {
                    bone_world_map.clear();
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
                        texture_cache,
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
                let end_idx = meshes.len();
                refr_mesh_ranges.push(start_idx..end_idx);
            }
        }

        let (bounds_center, bounds_radius) = if found {
            let center = (min + max) * 0.5;
            let radius = (max - min).length() * 0.5;
            (center, radius.max(10.0))
        } else {
            (Vec3::ZERO, 50.0)
        };

        RenderScene {
            meshes,
            anim_skin_meshes: Vec::new(),
            anim_rigid_meshes: Vec::new(),
            collision_meshes,
            actors: Vec::new(),
            bounds_center,
            bounds_radius,
            refr_mesh_ranges,
        }
    }
}
