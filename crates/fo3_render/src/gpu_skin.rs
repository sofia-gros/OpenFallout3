//! # GPU ハードウェアスキニング (Phase 6-D)
//!
//! Gamebryo 2.6 `NiSkinPartition` および `NiSkinData` に準拠した GPU ボーンパレット管理。
//! 参照元: `knowledge/actor_and_skin_mesh.md`, Gamebryo 2.6 `NiSkinInstance::Update`

use std::collections::HashMap;
use fo3_gamebryo_core::NiBound;
use fo3_nif::blocks::geometry::NiTriShapeData;
use fo3_nif::blocks::skin::{NiSkinData, NiSkinInstance, SkinPartition};
use fo3_nif::NifFile;
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::mesh::GpuMesh;
use crate::pipeline::RenderContext;
use crate::vertex::{BonePaletteUniform, SkinnedVertex, MAX_BONES_PER_PALETTE};

/// GPU 上で管理される単一スキンパーティションのボーンパレットリソース。
/// 参照元: Gamebryo 2.6 ハードウェアスキニング (NiSkinPartition)
pub struct GpuBonePalette {
    /// ボーンパレット Uniform バッファ
    pub buffer: wgpu::Buffer,
    /// Group 3 ボーンパレット BindGroup
    pub bind_group: wgpu::BindGroup,
    /// パーティションのローカルボーンが参照する NiSkinInstance.bones のインデックス配列
    pub partition_bones: Vec<u16>,
    /// 各パーティションボーンに対応する逆バインドポーズ行列 (B_bone)
    pub inv_bind_matrices: Vec<Mat4>,
    /// スキンルート変換行列 (S_root)
    pub skin_transform: Mat4,
    /// 各ボーンノードの名前一覧 (スケルトンボーンマップ検索用)
    pub bone_names: Vec<String>,
    /// 各ボーンノードの NIF ブロック番号一覧
    pub bone_block_indices: Vec<i32>,
}

impl GpuBonePalette {
    /// NIF スキンインスタンスおよびスキンデータからボーンパレットを生成する。
    pub fn new(
        device: &wgpu::Device,
        context: &RenderContext,
        partition: &SkinPartition,
        skin_instance: &NiSkinInstance,
        skin_data: &NiSkinData,
        nif: &NifFile,
    ) -> Self {
        let uniform = BonePaletteUniform::default();
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Bone Palette Uniform Buffer"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bone Palette Bind Group"),
            layout: &context.bone_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        // ルートスキン変換行列 (S_root)
        // 参照元: knowledge/actor_and_skin_mesh.md (セクション 3)
        let skin_transform = crate::skinning::build_bone_matrix(
            skin_data.skin_transform_translation,
            skin_data.skin_transform_rotation,
            skin_data.skin_transform_scale,
        );

        let mut inv_bind_matrices = Vec::with_capacity(partition.bones.len());
        let mut bone_names = Vec::with_capacity(partition.bones.len());
        let mut bone_block_indices = Vec::with_capacity(partition.bones.len());

        for &local_bone_idx in &partition.bones {
            let bone_idx = local_bone_idx as usize;
            if bone_idx < skin_data.bone_list.len() {
                let bd = &skin_data.bone_list[bone_idx];
                let inv_b = crate::skinning::build_bone_matrix(
                    bd.skin_transform_translation,
                    bd.skin_transform_rotation,
                    bd.skin_transform_scale,
                );
                inv_bind_matrices.push(inv_b);
            } else {
                inv_bind_matrices.push(Mat4::IDENTITY);
            }

            // ボーンノード名およびブロック番号の解決
            if bone_idx < skin_instance.bones.len() {
                let node_block_idx = skin_instance.bones[bone_idx];
                bone_block_indices.push(node_block_idx);
                if node_block_idx >= 0 && (node_block_idx as usize) < nif.blocks.len() {
                    let name_opt = match &nif.blocks[node_block_idx as usize] {
                        fo3_nif::NifBlock::NiNode(ref node) => nif.get_string(node.av.net.name_index),
                        fo3_nif::NifBlock::BSFadeNode(ref fade) => nif.get_string(fade.node.av.net.name_index),
                        _ => None,
                    };
                    if let Some(name) = name_opt {
                        bone_names.push(name.to_string());
                        continue;
                    }
                }
            } else {
                bone_block_indices.push(-1);
            }
            bone_names.push(String::new());
        }

        Self {
            buffer,
            bind_group,
            partition_bones: partition.bones.clone(),
            inv_bind_matrices,
            skin_transform,
            bone_names,
            bone_block_indices,
        }
    }

    /// スケルトンボーン名マップに基づき、GPU ボーンパレット Uniform バッファを更新する。
    /// 参照元: Gamebryo 2.6 `NiSkinInstance::Update` (P_k = M_bone * B_bone * S_root)
    pub fn update(&self, queue: &wgpu::Queue, bone_world_map: &HashMap<String, Mat4>) {
        let mut uniform = BonePaletteUniform::default();

        for (k, name) in self.bone_names.iter().enumerate() {
            if k >= MAX_BONES_PER_PALETTE {
                break;
            }

            let m_bone = if !name.is_empty() {
                bone_world_map.get(name).copied().unwrap_or(Mat4::IDENTITY)
            } else {
                Mat4::IDENTITY
            };

            let b_bone = self.inv_bind_matrices.get(k).copied().unwrap_or(Mat4::IDENTITY);

            // 合成行列: P_k = M_bone * B_bone * S_root
            let p_k = m_bone * b_bone * self.skin_transform;
            uniform.matrices[k] = p_k.to_cols_array_2d();
        }

        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&uniform));
    }

    /// NIF ローカルブロック番号マップに基づき、GPU ボーンパレット Uniform バッファを更新する。
    pub fn update_with_blocks(&self, queue: &wgpu::Queue, bone_block_map: &HashMap<i32, Mat4>) {
        let mut uniform = BonePaletteUniform::default();

        for (k, &block_idx) in self.bone_block_indices.iter().enumerate() {
            if k >= MAX_BONES_PER_PALETTE {
                break;
            }

            let m_bone = if block_idx >= 0 {
                bone_block_map.get(&block_idx).copied().unwrap_or(Mat4::IDENTITY)
            } else {
                Mat4::IDENTITY
            };

            let b_bone = self.inv_bind_matrices.get(k).copied().unwrap_or(Mat4::IDENTITY);

            let p_k = m_bone * b_bone * self.skin_transform;
            uniform.matrices[k] = p_k.to_cols_array_2d();
        }

        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&uniform));
    }
}

/// NiSkinPartition の 1 パーティションから GPU スキンメッシュを構築する。
/// 参照元: Gamebryo 2.6 `NiSkinPartition`, `knowledge/actor_and_skin_mesh.md`
pub fn create_gpu_skin_mesh_from_partition(
    device: &wgpu::Device,
    geo_data: &NiTriShapeData,
    partition: &SkinPartition,
) -> Option<GpuMesh> {
    let common = &geo_data.common;
    let num_verts = partition.num_vertices as usize;
    if num_verts == 0 || partition.vertex_map.len() < num_verts {
        return None;
    }

    let mut skinned_vertices = Vec::with_capacity(num_verts);

    for i in 0..num_verts {
        let orig_idx = partition.vertex_map[i] as usize;
        if orig_idx >= common.vertices.len() {
            continue;
        }

        let pos = [
            common.vertices[orig_idx].x,
            common.vertices[orig_idx].y,
            common.vertices[orig_idx].z,
        ];

        let normal = if orig_idx < common.normals.len() {
            [
                common.normals[orig_idx].x,
                common.normals[orig_idx].y,
                common.normals[orig_idx].z,
            ]
        } else {
            [0.0, 0.0, 1.0]
        };

        let uv = if !common.uv_sets.is_empty() && orig_idx < common.uv_sets[0].len() {
            [common.uv_sets[0][orig_idx].u, common.uv_sets[0][orig_idx].v]
        } else {
            [0.0, 0.0]
        };

        let color = if orig_idx < common.vertex_colors.len() {
            [
                common.vertex_colors[orig_idx].r,
                common.vertex_colors[orig_idx].g,
                common.vertex_colors[orig_idx].b,
                common.vertex_colors[orig_idx].a,
            ]
        } else {
            [1.0, 1.0, 1.0, 1.0]
        };

        let tangent = if orig_idx < common.tangents.len() {
            [
                common.tangents[orig_idx].x,
                common.tangents[orig_idx].y,
                common.tangents[orig_idx].z,
            ]
        } else {
            [1.0, 0.0, 0.0]
        };

        let bitangent = if orig_idx < common.bitangents.len() {
            [
                common.bitangents[orig_idx].x,
                common.bitangents[orig_idx].y,
                common.bitangents[orig_idx].z,
            ]
        } else {
            [0.0, 1.0, 0.0]
        };

        let bone_indices = if i < partition.bone_indices.len() {
            [
                partition.bone_indices[i][0] as u32,
                partition.bone_indices[i][1] as u32,
                partition.bone_indices[i][2] as u32,
                partition.bone_indices[i][3] as u32,
            ]
        } else {
            [0, 0, 0, 0]
        };

        let bone_weights = if i < partition.vertex_weights.len() {
            partition.vertex_weights[i]
        } else {
            [1.0, 0.0, 0.0, 0.0]
        };

        skinned_vertices.push(SkinnedVertex {
            position: pos,
            normal,
            uv,
            color,
            tangent,
            bitangent,
            bone_indices,
            bone_weights,
        });
    }

    if skinned_vertices.is_empty() {
        return None;
    }

    // インデックスバッファの構築 (SkinPartition::triangles)
    let mut indices: Vec<u16> = Vec::with_capacity(partition.triangles.len() * 3);
    for tri in &partition.triangles {
        indices.push(tri[0]);
        indices.push(tri[1]);
        indices.push(tri[2]);
    }

    if indices.is_empty() {
        return None;
    }

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Skinned Vertex Buffer"),
        contents: bytemuck::cast_slice(&skinned_vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Skinned Index Buffer"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    let bs = &common.bounding_sphere;
    let bound = NiBound {
        center: Vec3::new(bs.center.x, bs.center.y, bs.center.z),
        radius: bs.radius,
    };

    Some(GpuMesh {
        vertex_buffer,
        index_buffer,
        num_elements: indices.len() as u32,
        bound,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BonePaletteUniform のメモリサイズとアライメントのテスト。
    /// WGSL の uniform バッファ要件 (16バイトアライメント、合計 5,120 バイト) を検証。
    #[test]
    fn test_bone_palette_uniform_size_and_alignment() {
        assert_eq!(
            std::mem::size_of::<BonePaletteUniform>(),
            80 * 64,
            "BonePaletteUniform must be exactly 5,120 bytes for 80 Mat4 matrices"
        );
        assert_eq!(
            std::mem::align_of::<BonePaletteUniform>(),
            4,
            "Alignment of f32 4x4 matrix array is at least 4 bytes"
        );
    }

    /// デフォルトのボーンパレット Uniform が単位行列で初期化されることを検証。
    #[test]
    fn test_bone_palette_uniform_default() {
        let uniform = BonePaletteUniform::default();
        let identity = glam::Mat4::IDENTITY.to_cols_array_2d();
        for i in 0..MAX_BONES_PER_PALETTE {
            assert_eq!(uniform.matrices[i], identity);
        }
    }

    /// Gamebryo 2.6 のスキニング合成行列 P_k = M_bone * B_bone * S_root の計算整合性を検証。
    #[test]
    fn test_bone_skin_composite_matrix_calculation() {
        let skin_transform = Mat4::from_translation(glam::Vec3::new(10.0, 0.0, 0.0));
        let bone_world = Mat4::from_translation(glam::Vec3::new(0.0, 20.0, 0.0));
        let inv_bind = Mat4::from_translation(glam::Vec3::new(-10.0, -20.0, 0.0));

        let p_k = bone_world * inv_bind * skin_transform;
        // 点 (0, 0, 0) を変換した場合:
        // (0,0,0) + (10, 0, 0) = (10, 0, 0)
        // + (-10, -20, 0) = (0, -20, 0)
        // + (0, 20, 0) = (0, 0, 0)
        let transformed = p_k.transform_point3(glam::Vec3::ZERO);
        assert!((transformed - glam::Vec3::ZERO).length() < 1e-4);
    }
}

