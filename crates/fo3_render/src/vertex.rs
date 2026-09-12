//! # GPU 頂点データ構造
//!
//! NIF ジオメトリから変換された GPU 描画用頂点バッファレイアウト。
//! 参照元: Gamebryo 2.6 `NiGeometryData`, `references/nifxml/nif.xml`

use bytemuck::{Pod, Zeroable};

/// GPU 描画用の標準頂点フォーマット。
///
/// 座標、法線、テクスチャ座標 (UV)、頂点カラー、タンジェント、ビットタンジェントを保持。
/// 参照元: Gamebryo 2.6 `NiGeometryData`, `references/nifxml/nif.xml:L1234`
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    /// 3D 空間位置 (X, Y, Z)
    pub position: [f32; 3],
    /// 法線ベクトル (NX, NY, NZ)
    pub normal: [f32; 3],
    /// テクスチャ座標 (U, V)
    pub uv: [f32; 2],
    /// 頂点カラー (R, G, B, A)
    pub color: [f32; 4],
    /// 接線ベクトル (TX, TY, TZ)
    pub tangent: [f32; 3],
    /// 従法線ベクトル (BX, BY, BZ)
    pub bitangent: [f32; 3],
}

impl Vertex {
    /// wgpu 用の頂点バッファレイアウトディスクリプタを返す。
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // location 0: position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // location 1: normal
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // location 2: uv
                wgpu::VertexAttribute {
                    offset: (mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // location 3: color
                wgpu::VertexAttribute {
                    offset: (mem::size_of::<[f32; 3]>() * 2 + mem::size_of::<[f32; 2]>()) as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // location 4: tangent
                wgpu::VertexAttribute {
                    offset: (mem::size_of::<[f32; 3]>() * 2 + mem::size_of::<[f32; 2]>() + mem::size_of::<[f32; 4]>()) as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // location 5: bitangent
                wgpu::VertexAttribute {
                    offset: (mem::size_of::<[f32; 3]>() * 3 + mem::size_of::<[f32; 2]>() + mem::size_of::<[f32; 4]>()) as wgpu::BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

/// GPU スキニング用頂点フォーマット (Phase 6-D)。
///
/// 標準属性に加え、頂点ごとのボーンインデックス (4本) とボーン重み (4本) を保持。
/// 参照元: Gamebryo 2.6 `NiSkinPartition`, `knowledge/actor_and_skin_mesh.md`
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SkinnedVertex {
    /// 3D 空間位置 (X, Y, Z) - 初期バインドポーズ
    pub position: [f32; 3],
    /// 法線ベクトル (NX, NY, NZ) - 初期バインドポーズ
    pub normal: [f32; 3],
    /// テクスチャ座標 (U, V)
    pub uv: [f32; 2],
    /// 頂点カラー (R, G, B, A)
    pub color: [f32; 4],
    /// 接線ベクトル (TX, TY, TZ)
    pub tangent: [f32; 3],
    /// 従法線ベクトル (BX, BY, BZ)
    pub bitangent: [f32; 3],
    /// 参照ボーンインデックス (最大 4 ボーン)
    pub bone_indices: [u32; 4],
    /// ボーン重み (合計 1.0)
    pub bone_weights: [f32; 4],
}

impl SkinnedVertex {
    /// wgpu 用のスキン頂点バッファレイアウトディスクリプタを返す。
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<SkinnedVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // location 0: position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // location 1: normal
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // location 2: uv
                wgpu::VertexAttribute {
                    offset: (mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // location 3: color
                wgpu::VertexAttribute {
                    offset: (mem::size_of::<[f32; 3]>() * 2 + mem::size_of::<[f32; 2]>()) as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // location 4: tangent
                wgpu::VertexAttribute {
                    offset: (mem::size_of::<[f32; 3]>() * 2 + mem::size_of::<[f32; 2]>() + mem::size_of::<[f32; 4]>()) as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // location 5: bitangent
                wgpu::VertexAttribute {
                    offset: (mem::size_of::<[f32; 3]>() * 3 + mem::size_of::<[f32; 2]>() + mem::size_of::<[f32; 4]>()) as wgpu::BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // location 6: bone_indices
                wgpu::VertexAttribute {
                    offset: (mem::size_of::<[f32; 3]>() * 4 + mem::size_of::<[f32; 2]>() + mem::size_of::<[f32; 4]>()) as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Uint32x4,
                },
                // location 7: bone_weights
                wgpu::VertexAttribute {
                    offset: (mem::size_of::<[f32; 3]>() * 4 + mem::size_of::<[f32; 2]>() + mem::size_of::<[f32; 4]>() + mem::size_of::<[u32; 4]>()) as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// 1 パーティションあたりの最大ボーン数。
/// 参照元: Gamebryo 2.6 ハードウェアスキニング定数 (通常最大 60〜80 ボーン)
pub const MAX_BONES_PER_PALETTE: usize = 80;

/// GPU ボーンパレット Uniform バッファ構造体。
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct BonePaletteUniform {
    pub matrices: [[[f32; 4]; 4]; MAX_BONES_PER_PALETTE],
}

impl Default for BonePaletteUniform {
    fn default() -> Self {
        Self {
            matrices: [glam::Mat4::IDENTITY.to_cols_array_2d(); MAX_BONES_PER_PALETTE],
        }
    }
}

