//! # GPU メッシュデータ構造
//!
//! NIF のジオメトリブロック（NiTriShapeData, NiTriStripsData）から
//! GPU 用の頂点バッファ・インデックスバッファへの変換および保持を担当。
//! 参照元: `references/openmw/components/nifosg/nifloader.cpp:L1600-1623`

use fo3_nif::{NiGeometryDataCommon, NiTriShapeData, NiTriStripsData};
use wgpu::util::DeviceExt;
use crate::vertex::Vertex;

/// トライアングルストリップ列を三角形リスト (TriangleList) のインデックス配列に展開する。
///
/// 参照元: `references/openmw/components/nifosg/nifloader.cpp:L1613-1620`
/// 縮退三角形 (degenerate triangles: a==b または b==c または a==c) はスキップする。
pub fn strips_to_triangles(strips: &[Vec<u16>]) -> Vec<u16> {
    let mut indices = Vec::new();
    for strip in strips {
        if strip.len() < 3 {
            continue;
        }
        for i in 0..strip.len() - 2 {
            let a = strip[i];
            let b = if i % 2 == 0 { strip[i + 1] } else { strip[i + 2] };
            let c = if i % 2 == 0 { strip[i + 2] } else { strip[i + 1] };
            // 縮退三角形を除外
            if a != b && b != c && a != c {
                indices.push(a);
                indices.push(b);
                indices.push(c);
            }
        }
    }
    indices
}

/// NIF の共通ジオメトリデータから GPU 頂点配列を構築する。
pub fn build_vertices(common: &NiGeometryDataCommon) -> Vec<Vertex> {
    let n = common.num_vertices as usize;
    let mut vertices = Vec::with_capacity(n);

    let has_normals = !common.normals.is_empty();
    let has_uv = !common.uv_sets.is_empty() && !common.uv_sets[0].is_empty();
    let has_colors = !common.vertex_colors.is_empty();

    for i in 0..n {
        let pos = if i < common.vertices.len() {
            [common.vertices[i].x, common.vertices[i].y, common.vertices[i].z]
        } else {
            [0.0, 0.0, 0.0]
        };

        let normal = if has_normals && i < common.normals.len() {
            [common.normals[i].x, common.normals[i].y, common.normals[i].z]
        } else {
            [0.0, 0.0, 1.0]
        };

        let uv = if has_uv && i < common.uv_sets[0].len() {
            [common.uv_sets[0][i].u, common.uv_sets[0][i].v]
        } else {
            [0.0, 0.0]
        };

        let color = if has_colors && i < common.vertex_colors.len() {
            [
                common.vertex_colors[i].r,
                common.vertex_colors[i].g,
                common.vertex_colors[i].b,
                common.vertex_colors[i].a,
            ]
        } else {
            [1.0, 1.0, 1.0, 1.0]
        };

        vertices.push(Vertex {
            position: pos,
            normal,
            uv,
            color,
        });
    }

    vertices
}

/// GPU 上に確保されたメッシュリソース。
pub struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
}

impl GpuMesh {
    /// NiTriShapeData から GpuMesh を生成する。
    pub fn from_tri_shape(device: &wgpu::Device, data: &NiTriShapeData) -> Option<Self> {
        let vertices = build_vertices(&data.common);
        if vertices.is_empty() {
            return None;
        }

        let mut indices = Vec::with_capacity(data.triangles.len() * 3);
        for tri in &data.triangles {
            indices.push(tri.v1);
            indices.push(tri.v2);
            indices.push(tri.v3);
        }

        if indices.is_empty() {
            return None;
        }

        Self::create(device, &vertices, &indices)
    }

    /// NiTriStripsData から GpuMesh を生成する。
    pub fn from_tri_strips(device: &wgpu::Device, data: &NiTriStripsData) -> Option<Self> {
        let vertices = build_vertices(&data.common);
        if vertices.is_empty() {
            return None;
        }

        let indices = strips_to_triangles(&data.strips);
        if indices.is_empty() {
            return None;
        }

        Self::create(device, &vertices, &indices)
    }

    fn create(device: &wgpu::Device, vertices: &[Vertex], indices: &[u16]) -> Option<Self> {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mesh Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mesh Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Some(GpuMesh {
            vertex_buffer,
            index_buffer,
            num_elements: indices.len() as u32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strips_to_triangles() {
        // 4頂点のストリップ [0, 1, 2, 3] -> 2つの三角形 (0, 1, 2) と (1, 3, 2)
        let strips = vec![vec![0, 1, 2, 3]];
        let triangles = strips_to_triangles(&strips);
        assert_eq!(triangles, vec![0, 1, 2, 1, 3, 2]);
    }

    #[test]
    fn test_degenerate_triangles_filtered() {
        // 縮退を含むストリップ [0, 1, 2, 2, 3, 4]
        let strips = vec![vec![0, 1, 2, 2, 3, 4]];
        let triangles = strips_to_triangles(&strips);
        // (0,1,2), (1,2,2: 縮退除外), (2,2,3: 縮退除外), (2,4,3)
        assert_eq!(triangles, vec![0, 1, 2, 2, 4, 3]);
    }
}
