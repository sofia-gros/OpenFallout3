//! # GPU 頂点データ構造
//!
//! NIF ジオメトリから変換された GPU 描画用頂点バッファレイアウト。
//! 参照元: Gamebryo 2.6 `NiGeometryData`, `references/nifxml/nif.xml`

use bytemuck::{Pod, Zeroable};

/// GPU 描画用の標準頂点フォーマット。
///
/// 座標、法線、テクスチャ座標 (UV)、頂点カラーを保持。
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
            ],
        }
    }
}
