//! # メッシュ描画ノードおよびマテリアル・テクスチャバインド
//!
//! Gamebryo 2.6 のジオメトリプロパティ（マテリアル、アルファ、テクスチャ）を
//! wgpu のパイプラインバインドグループへ変換する。
//! 参照元: `references/nifxml/nif.xml`, `knowledge/nif_blocks_geometry.md`

use std::collections::HashMap;
use glam::{Mat3, Vec3};
use wgpu::util::DeviceExt;

use fo3_gamebryo_core::NiTransform;
use fo3_nif::{NifBlock, NifFile};
use fo3_vfs::VfsManager;

use crate::mesh::GpuMesh;
use crate::pipeline::{ModelUniform, RenderContext};
use crate::texture::GpuTexture;

/// 単一のメッシュ描画単位。
pub struct RenderMesh {
    pub name: String,
    pub mesh: GpuMesh,
    pub model_bind_group: wgpu::BindGroup,
    /// モデル・マテリアル用 Uniform バッファ (動的ワールド変換更新用)
    pub model_uniform_buffer: wgpu::Buffer,
    pub texture_bind_group: wgpu::BindGroup,
    /// 半透明合成（Alpha Blending）を行うか
    pub is_transparent: bool,
    /// カメラ距離によるソートを行うか
    pub alpha_sort: bool,
    /// メッシュのワールド空間中心座標 (ソート用)
    pub world_center: Vec3,
}

/// NIF の `NiAVObject` からコアトランスフォームへの変換。
pub fn to_core_transform(av: &fo3_nif::NiAVObject) -> NiTransform {
    let rot = Mat3::from_cols_array_2d(&av.rotation.m).transpose();
    NiTransform {
        rotation: rot,
        translation: Vec3::new(av.translation.x, av.translation.y, av.translation.z),
        scale: av.scale,
    }
}

/// ジオメトリブロックとプロパティ群から `RenderMesh` を構築する。
pub fn create_render_mesh(
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
    tint_color: Option<[f32; 4]>,
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

    // Model Uniform バッファ作成 (動的書き換え対応のため COPY_DST を付与)
    let world_mat = world_transform.to_mat4();
    let tint = tint_color.unwrap_or([1.0, 1.0, 1.0, 1.0]);
    let model_uniform = ModelUniform::new_with_tint(world_mat, effective_alpha, material_prop, has_glow_map, tint);
    let model_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("Model Uniform Buffer: {}", name)),
        contents: bytemuck::bytes_of(&model_uniform),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
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
        model_uniform_buffer,
        texture_bind_group,
        is_transparent,
        alpha_sort,
        world_center: world_transform.translation,
    }
}

/// テクスチャをキャッシュまたは VFS からロードしてキャッシュに格納する。
pub fn ensure_texture_cached(
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
/// 参照元: `references/openmw/components/nifosg/nifloader.cpp:L2401-2426`, `references/nifxml/nif.xml:L6307`
pub fn find_texture_paths(properties: &[i32], nif: &NifFile) -> (Option<String>, Option<String>, Option<String>) {
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
/// 参照元: `references/nifxml/nif.xml:L4363`, Gamebryo 2.6 `NiMaterialProperty`
pub fn find_material_property<'a>(properties: &[i32], nif: &'a NifFile) -> Option<&'a fo3_nif::NiMaterialProperty> {
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
/// 参照元: `references/nifskope/src/gl/glnode.cpp:295`, `references/nifxml/nif.xml:L3972`
pub fn find_alpha_property<'a>(properties: &[i32], nif: &'a NifFile) -> Option<&'a fo3_nif::NiAlphaProperty> {
    for &prop_idx in properties {
        if prop_idx >= 0 && (prop_idx as usize) < nif.blocks.len() {
            if let NifBlock::NiAlphaProperty(ref alpha) = nif.blocks[prop_idx as usize] {
                return Some(alpha);
            }
        }
    }
    None
}

/// テクスチャパスを正規化（区切り文字をバックスラッシュに統一し、先頭に 'textures\' を補完）する。
/// BSA 内では 'textures/landscape/...' のように格納されているため。
pub fn normalize_texture_path(path: &str) -> String {
    let p = path.replace('/', "\\");
    if p.to_ascii_lowercase().starts_with("textures\\") {
        p
    } else {
        format!("textures\\{}", p)
    }
}
