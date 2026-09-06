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
    let has_tangents = !common.tangents.is_empty();
    let has_bitangents = !common.bitangents.is_empty();

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

        let tangent = if has_tangents && i < common.tangents.len() {
            [common.tangents[i].x, common.tangents[i].y, common.tangents[i].z]
        } else {
            [0.0, 0.0, 0.0]
        };

        let bitangent = if has_bitangents && i < common.bitangents.len() {
            [common.bitangents[i].x, common.bitangents[i].y, common.bitangents[i].z]
        } else {
            [0.0, 0.0, 0.0]
        };

        vertices.push(Vertex {
            position: pos,
            normal,
            uv,
            color,
            tangent,
            bitangent,
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

    /// 地形 (LAND) レコードとセルグリッド座標 (grid_x, grid_y) から GpuMesh を生成する。
    ///
    /// 参照元:
    /// - `references/openmw/components/esm4/loadland.hpp` (LAND_VERTS_PER_SIDE = 33, sRealSize = 4096)
    /// - `references/openmw/components/esm/esmterrain.cpp:52-71` (標高復元計算式)
    pub fn from_land(
        device: &wgpu::Device,
        land: &fo3_esm::LandRecord,
        grid_x: i32,
        grid_y: i32,
    ) -> Option<Self> {
        use fo3_esm::{LAND_NUM_VERTS, LAND_REAL_SIZE, LAND_VERTS_PER_SIDE};

        let heights = land.compute_heights();
        let step = LAND_REAL_SIZE / (LAND_VERTS_PER_SIDE - 1) as f32; // 128.0
        let origin_x = grid_x as f32 * LAND_REAL_SIZE;
        let origin_y = grid_y as f32 * LAND_REAL_SIZE;

        let mut vertices = Vec::with_capacity(LAND_NUM_VERTS);
        for y in 0..LAND_VERTS_PER_SIDE {
            for x in 0..LAND_VERTS_PER_SIDE {
                let idx = y * LAND_VERTS_PER_SIDE + x;
                let wx = origin_x + x as f32 * step;
                let wy = origin_y + y as f32 * step;
                let wz = heights[idx];

                let normal = if let Some(ref norms) = land.normals {
                    if idx < norms.len() {
                        let n = norms[idx];
                        [n[0] as f32 / 127.0, n[1] as f32 / 127.0, n[2] as f32 / 127.0]
                    } else {
                        [0.0, 0.0, 1.0]
                    }
                } else {
                    [0.0, 0.0, 1.0]
                };

                let color = if let Some(ref cols) = land.vertex_colors {
                    if idx < cols.len() {
                        let c = cols[idx];
                        [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0]
                    } else {
                        [1.0, 1.0, 1.0, 1.0]
                    }
                } else {
                    [1.0, 1.0, 1.0, 1.0]
                };

                // UV は 1 グリッドあたり複数回タイリング（1マスあたり1回）
                let uv = [x as f32, y as f32];

                vertices.push(Vertex {
                    position: [wx, wy, wz],
                    normal,
                    uv,
                    color,
                    tangent: [1.0, 0.0, 0.0],
                    bitangent: [0.0, 1.0, 0.0],
                });
            }
        }

        // 32 x 32 クアッド -> 2048 三角形 (6144 インデックス)
        let num_quads = LAND_VERTS_PER_SIDE - 1; // 32
        let mut indices = Vec::with_capacity(num_quads * num_quads * 6);
        for y in 0..num_quads {
            for x in 0..num_quads {
                let i0 = (y * LAND_VERTS_PER_SIDE + x) as u16;
                let i1 = (y * LAND_VERTS_PER_SIDE + x + 1) as u16;
                let i2 = ((y + 1) * LAND_VERTS_PER_SIDE + x) as u16;
                let i3 = ((y + 1) * LAND_VERTS_PER_SIDE + x + 1) as u16;

                // 三角形 1: i0 -> i2 -> i1
                indices.push(i0);
                indices.push(i2);
                indices.push(i1);

                // 三角形 2: i1 -> i2 -> i3
                indices.push(i1);
                indices.push(i2);
                indices.push(i3);
            }
        }

        Self::create(device, &vertices, &indices)
    }

    /// 地形 (LAND) レコードの特定のクアドラント (0..3) から GpuMesh を生成する。
    ///
    /// 参照元:
    /// - `references/openmw/components/esm4/loadland.hpp:79` (0 = bottom left, 1 = bottom right, 2 = top left, 3 = top right)
    /// - `references/openmw/components/esm4/loadland.hpp:53-65` (sVertsPerSide = 33, sRealSize = 4096)
    /// - `references/openmw/components/esm/esmterrain.cpp:52-71` (標高復元計算式)
    pub fn from_land_quadrant(
        device: &wgpu::Device,
        land: &fo3_esm::LandRecord,
        grid_x: i32,
        grid_y: i32,
        quadrant: usize,
    ) -> Option<Self> {
        use fo3_esm::{LAND_REAL_SIZE, LAND_VERTS_PER_SIDE};

        if quadrant >= 4 {
            return None;
        }

        let heights = land.compute_heights();
        let step = LAND_REAL_SIZE / (LAND_VERTS_PER_SIDE - 1) as f32; // 128.0
        let origin_x = grid_x as f32 * LAND_REAL_SIZE;
        let origin_y = grid_y as f32 * LAND_REAL_SIZE;

        // クアドラントごとの開始頂点オフセット (各クアドラントは 16x16 クワッド = 17x17 頂点)
        let (x_start, y_start) = match quadrant {
            0 => (0, 0),   // 左下 (Bottom-Left)
            1 => (16, 0),  // 右下 (Bottom-Right)
            2 => (0, 16),  // 左上 (Top-Left)
            3 => (16, 16), // 右上 (Top-Right)
            _ => unreachable!(),
        };

        const QUAD_VERTS_PER_SIDE: usize = 17;
        const QUAD_QUADS_PER_SIDE: usize = 16;

        let mut vertices = Vec::with_capacity(QUAD_VERTS_PER_SIDE * QUAD_VERTS_PER_SIDE);
        for ly in 0..QUAD_VERTS_PER_SIDE {
            for lx in 0..QUAD_VERTS_PER_SIDE {
                let gx = x_start + lx;
                let gy = y_start + ly;
                let idx = gy * LAND_VERTS_PER_SIDE + gx;

                let wx = origin_x + gx as f32 * step;
                let wy = origin_y + gy as f32 * step;
                let wz = heights[idx];

                let normal = if let Some(ref norms) = land.normals {
                    if idx < norms.len() {
                        let n = norms[idx];
                        [n[0] as f32 / 127.0, n[1] as f32 / 127.0, n[2] as f32 / 127.0]
                    } else {
                        [0.0, 0.0, 1.0]
                    }
                } else {
                    [0.0, 0.0, 1.0]
                };

                let color = if let Some(ref cols) = land.vertex_colors {
                    if idx < cols.len() {
                        let c = cols[idx];
                        [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0]
                    } else {
                        [1.0, 1.0, 1.0, 1.0]
                    }
                } else {
                    [1.0, 1.0, 1.0, 1.0]
                };

                // UV は 1 グリッドあたり 1 回タイリング
                let uv = [gx as f32, gy as f32];

                vertices.push(Vertex {
                    position: [wx, wy, wz],
                    normal,
                    uv,
                    color,
                    tangent: [1.0, 0.0, 0.0],
                    bitangent: [0.0, 1.0, 0.0],
                });
            }
        }

        // 16 x 16 クアッド -> 512 三角形 (1536 インデックス)
        let mut indices = Vec::with_capacity(QUAD_QUADS_PER_SIDE * QUAD_QUADS_PER_SIDE * 6);
        for ly in 0..QUAD_QUADS_PER_SIDE {
            for lx in 0..QUAD_QUADS_PER_SIDE {
                let i0 = (ly * QUAD_VERTS_PER_SIDE + lx) as u16;
                let i1 = (ly * QUAD_VERTS_PER_SIDE + lx + 1) as u16;
                let i2 = ((ly + 1) * QUAD_VERTS_PER_SIDE + lx) as u16;
                let i3 = ((ly + 1) * QUAD_VERTS_PER_SIDE + lx + 1) as u16;

                // 三角形 1: i0 -> i2 -> i1
                indices.push(i0);
                indices.push(i2);
                indices.push(i1);

                // 三角形 2: i1 -> i2 -> i3
                indices.push(i1);
                indices.push(i2);
                indices.push(i3);
            }
        }

        Self::create(device, &vertices, &indices)
    }

    /// 地形 (LAND) の追加テクスチャレイヤー (ATXT/VTXT) 用 GpuMesh を生成する。
    /// 頂点カラーのアルファチャンネル (color[3]) に透過率 (0.0〜1.0) を格納。
    ///
    /// 参照元:
    /// - `references/openmw/components/esm4/loadland.hpp:84-98` (ATXT, VTXT)
    /// - `knowledge/worldspace_cells.md:セクション5`
    pub fn from_land_quadrant_layer(
        device: &wgpu::Device,
        land: &fo3_esm::LandRecord,
        grid_x: i32,
        grid_y: i32,
        layer: &fo3_esm::LandTextureLayer,
    ) -> Option<Self> {
        use fo3_esm::{LAND_REAL_SIZE, LAND_VERTS_PER_SIDE};

        if layer.quadrant >= 4 {
            return None;
        }

        const QUAD_VERTS_PER_SIDE: usize = 17;
        const QUAD_QUADS_PER_SIDE: usize = 16;
        const NUM_QUAD_VERTS: usize = QUAD_VERTS_PER_SIDE * QUAD_VERTS_PER_SIDE; // 289

        // 17x17 頂点の透過率テーブルを構築
        let mut opacity_grid = [0.0f32; NUM_QUAD_VERTS];
        let mut has_visible = false;
        for &(pos, op) in &layer.opacities {
            if (pos as usize) < NUM_QUAD_VERTS {
                opacity_grid[pos as usize] = op.clamp(0.0, 1.0);
                if op > 0.001 {
                    has_visible = true;
                }
            }
        }

        // 不透明度がすべてゼロならメッシュ生成不要
        if !has_visible {
            return None;
        }

        let heights = land.compute_heights();
        let step = LAND_REAL_SIZE / (LAND_VERTS_PER_SIDE - 1) as f32;
        let origin_x = grid_x as f32 * LAND_REAL_SIZE;
        let origin_y = grid_y as f32 * LAND_REAL_SIZE;

        let (x_start, y_start) = match layer.quadrant {
            0 => (0, 0),
            1 => (16, 0),
            2 => (0, 16),
            3 => (16, 16),
            _ => unreachable!(),
        };

        let mut vertices = Vec::with_capacity(NUM_QUAD_VERTS);
        for ly in 0..QUAD_VERTS_PER_SIDE {
            for lx in 0..QUAD_VERTS_PER_SIDE {
                let local_idx = ly * QUAD_VERTS_PER_SIDE + lx;
                let gx = x_start + lx;
                let gy = y_start + ly;
                let idx = gy * LAND_VERTS_PER_SIDE + gx;

                let wx = origin_x + gx as f32 * step;
                let wy = origin_y + gy as f32 * step;
                let wz = heights[idx];

                let normal = if let Some(ref norms) = land.normals {
                    if idx < norms.len() {
                        let n = norms[idx];
                        [n[0] as f32 / 127.0, n[1] as f32 / 127.0, n[2] as f32 / 127.0]
                    } else {
                        [0.0, 0.0, 1.0]
                    }
                } else {
                    [0.0, 0.0, 1.0]
                };

                let alpha = opacity_grid[local_idx];
                let color = [1.0, 1.0, 1.0, alpha];
                let uv = [gx as f32, gy as f32];

                vertices.push(Vertex {
                    position: [wx, wy, wz],
                    normal,
                    uv,
                    color,
                    tangent: [1.0, 0.0, 0.0],
                    bitangent: [0.0, 1.0, 0.0],
                });
            }
        }

        let mut indices = Vec::with_capacity(QUAD_QUADS_PER_SIDE * QUAD_QUADS_PER_SIDE * 6);
        for ly in 0..QUAD_QUADS_PER_SIDE {
            for lx in 0..QUAD_QUADS_PER_SIDE {
                let i0 = (ly * QUAD_VERTS_PER_SIDE + lx) as u16;
                let i1 = (ly * QUAD_VERTS_PER_SIDE + lx + 1) as u16;
                let i2 = ((ly + 1) * QUAD_VERTS_PER_SIDE + lx) as u16;
                let i3 = ((ly + 1) * QUAD_VERTS_PER_SIDE + lx + 1) as u16;

                indices.push(i0);
                indices.push(i2);
                indices.push(i1);

                indices.push(i1);
                indices.push(i2);
                indices.push(i3);
            }
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
