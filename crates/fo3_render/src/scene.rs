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
use wgpu::util::DeviceExt;

use crate::collision::{extract_collision_lines, GpuCollisionMesh};
use crate::mesh::GpuMesh;
use crate::pipeline::RenderContext;
use crate::texture::GpuTexture;

/// 単一のメッシュ描画単位。
pub struct RenderMesh {
    pub name: String,
    pub mesh: GpuMesh,
    pub model_bind_group: wgpu::BindGroup,
    pub texture_bind_group: wgpu::BindGroup,
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

        // ルートブロック（通常 0 番）からトラバース開始
        let root_transform = NiTransform::default();
        if !nif.blocks.is_empty() {
            traverse_block(
                0,
                &root_transform,
                nif,
                vfs,
                device,
                queue,
                context,
                &mut meshes,
                &mut texture_cache,
                &default_texture,
                &default_normal_texture,
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
        Self::from_cell(device, queue, context, placed_nifs, None, vfs)
    }

    /// セルの配置オブジェクト群および地形 (LAND) から RenderScene を構築する。
    ///
    /// 参照元:
    /// - `references/openmw/components/esmterrain/` (地形チャンク生成)
    /// - `references/openmw/components/esm4/loadland.hpp` (LAND レコード仕様)
    pub fn from_cell(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        context: &RenderContext,
        placed_nifs: &[(&NifFile, NiTransform)],
        land_info: Option<(&fo3_esm::LandRecord, i32, i32)>,
        vfs: &mut VfsManager,
    ) -> Self {
        let mut meshes = Vec::new();
        let mut texture_cache: HashMap<String, GpuTexture> = HashMap::new();
        let default_texture = GpuTexture::create_default_white(device, queue);
        let default_normal_texture = GpuTexture::create_default_normal(device, queue);

        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        let mut found = false;

        // 1. 地形 (LAND) メッシュの生成と登録
        if let Some((land, grid_x, grid_y)) = land_info {
            if let Some(gpu_mesh) = GpuMesh::from_land(device, land, grid_x, grid_y) {
                // デフォルトの荒野テクスチャをロード
                let diffuse_path = "textures/landscape/dirtwasteland01.dds";
                if !texture_cache.contains_key(diffuse_path) {
                    if let Ok(bytes) = vfs.read(diffuse_path) {
                        if let Ok(tex) = GpuTexture::from_dds_bytes(device, queue, &bytes, Some(diffuse_path)) {
                            texture_cache.insert(diffuse_path.to_string(), tex);
                        }
                    }
                }
                let texture = texture_cache.get(diffuse_path).unwrap_or(&default_texture);

                let model_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Model Uniform Buffer: Landscape"),
                    contents: bytemuck::cast_slice(&glam::Mat4::IDENTITY.to_cols_array()),
                    usage: wgpu::BufferUsages::UNIFORM,
                });

                let model_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Model Bind Group: Landscape"),
                    layout: &context.model_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: model_uniform_buffer.as_entire_binding(),
                    }],
                });

                let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Texture Bind Group: Landscape"),
                    layout: &context.texture_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&texture.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&texture.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&default_normal_texture.view),
                        },
                    ],
                });

                meshes.push(RenderMesh {
                    name: "Landscape".to_string(),
                    mesh: gpu_mesh,
                    model_bind_group,
                    texture_bind_group,
                });

                // 地形のバウンディング反映
                let origin_x = grid_x as f32 * fo3_esm::LAND_REAL_SIZE;
                let origin_y = grid_y as f32 * fo3_esm::LAND_REAL_SIZE;
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
        }

        // 2. 配置された 3D オブジェクト (REFR) の走査と登録
        let mut collision_meshes = Vec::new();
        for (nif, world_transform) in placed_nifs {
            if !nif.blocks.is_empty() {
                traverse_block(
                    0,
                    world_transform,
                    nif,
                    vfs,
                    device,
                    queue,
                    context,
                    &mut meshes,
                    &mut texture_cache,
                    &default_texture,
                    &default_normal_texture,
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
                                let local_p = glam::Vec4::new(v.x, v.y, v.z, 1.0);
                                let wp = mat * local_p;
                                let p = Vec3::new(wp.x, wp.y, wp.z);
                                min = min.min(p);
                                max = max.max(p);
                                found = true;
                            }
                        }
                        NifBlock::NiTriStripsData(d) => {
                            for v in &d.common.vertices {
                                let local_p = glam::Vec4::new(v.x, v.y, v.z, 1.0);
                                let wp = mat * local_p;
                                let p = Vec3::new(wp.x, wp.y, wp.z);
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

        let (bounds_center, bounds_radius) = if found {
            let center = (min + max) * 0.5;
            let radius = (max - min).length() * 0.5;
            (center, radius.max(10.0))
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

    /// シーン内のすべてのメッシュを描画する。
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        for mesh_node in &self.meshes {
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
    nif: &NifFile,
    vfs: &mut VfsManager,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    context: &RenderContext,
    out_meshes: &mut Vec<RenderMesh>,
    texture_cache: &mut HashMap<String, GpuTexture>,
    default_texture: &GpuTexture,
    default_normal_texture: &GpuTexture,
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
            for &child in &node.children {
                traverse_block(
                    child,
                    &world_transform,
                    nif,
                    vfs,
                    device,
                    queue,
                    context,
                    out_meshes,
                    texture_cache,
                    default_texture,
                    default_normal_texture,
                );
            }
        }
        NifBlock::BSFadeNode(fade) => {
            if is_node_hidden(&fade.node.av, nif) {
                return;
            }
            let local_transform = to_core_transform(&fade.node.av);
            let world_transform = parent_world.compose(&local_transform);
            for &child in &fade.node.children {
                traverse_block(
                    child,
                    &world_transform,
                    nif,
                    vfs,
                    device,
                    queue,
                    context,
                    out_meshes,
                    texture_cache,
                    default_texture,
                    default_normal_texture,
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
                    if let Some(gpu_mesh) = GpuMesh::from_tri_shape(device, data) {
                        let render_mesh = create_render_mesh(
                            device,
                            context,
                            &name,
                            gpu_mesh,
                            &world_transform,
                            &shape.geom.av.properties,
                            nif,
                            vfs,
                            queue,
                            texture_cache,
                            default_texture,
                            default_normal_texture,
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
                            nif,
                            vfs,
                            queue,
                            texture_cache,
                            default_texture,
                            default_normal_texture,
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
    nif: &NifFile,
    vfs: &mut VfsManager,
    queue: &wgpu::Queue,
    texture_cache: &mut HashMap<String, GpuTexture>,
    default_texture: &GpuTexture,
    default_normal_texture: &GpuTexture,
) -> RenderMesh {
    // Model Uniform バッファ作成
    let world_mat = world_transform.to_mat4();
    let model_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("Model Uniform Buffer: {}", name)),
        contents: bytemuck::cast_slice(&world_mat.to_cols_array()),
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

    // テクスチャ探索 (Slot 0: Diffuse, Slot 1: Normal Map)
    // 参照元: references/openmw/components/nifosg/nifloader.cpp:L2401-2426
    let (diffuse_path, normal_path) = find_texture_paths(properties, nif);
    ensure_texture_cached(&diffuse_path, vfs, device, queue, texture_cache);
    ensure_texture_cached(&normal_path, vfs, device, queue, texture_cache);

    let diffuse_tex = diffuse_path
        .as_ref()
        .and_then(|p| texture_cache.get(p))
        .unwrap_or(default_texture);
    let normal_tex = normal_path
        .as_ref()
        .and_then(|p| texture_cache.get(p))
        .unwrap_or(default_normal_texture);

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
        ],
    });

    RenderMesh {
        name: name.to_string(),
        mesh,
        model_bind_group,
        texture_bind_group,
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

/// マテリアルプロパティからディフューズ (スロット 0) と法線マップ (スロット 1) を取得。
/// 参照元: references/openmw/components/nifosg/nifloader.cpp:L2401-2426
fn find_texture_paths(properties: &[i32], nif: &NifFile) -> (Option<String>, Option<String>) {
    for &prop_idx in properties {
        if prop_idx >= 0 && (prop_idx as usize) < nif.blocks.len() {
            if let NifBlock::BSShaderPPLightingProperty(ref shader_prop) = nif.blocks[prop_idx as usize] {
                if shader_prop.texture_set >= 0 && (shader_prop.texture_set as usize) < nif.blocks.len() {
                    if let NifBlock::BSShaderTextureSet(ref tex_set) = nif.blocks[shader_prop.texture_set as usize] {
                        let diff = if !tex_set.textures.is_empty() && !tex_set.textures[0].is_empty() {
                            Some(tex_set.textures[0].clone())
                        } else {
                            None
                        };
                        let norm = if tex_set.textures.len() > 1 && !tex_set.textures[1].is_empty() {
                            Some(tex_set.textures[1].clone())
                        } else {
                            None
                        };
                        return (diff, norm);
                    }
                }
            }
        }
    }
    (None, None)
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
