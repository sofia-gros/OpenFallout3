//! # Havok コリジョンワイヤーフレーム生成モジュール
//!
//! NIF ファイル内の Havok コリジョンブロック (`bhkCollisionObject`, `bhkRigidBody`,
//! `bhkPackedNiTriStripsShape`, `bhkBoxShape`, `bhkSphereShape`, `bhkCapsuleShape`,
//! `bhkConvexVerticesShape`) を走査し、GPU 描画用のラインリスト (`LineList`) を構築する。
//!
//! 参照元:
//! - `references/nifskope/src/gl/glnode.cpp:L742-897` (`drawHvkShape`)
//! - `references/nifskope/src/gl/gltools.cpp:L327-345` (`hkScale660`, `drawBox`, `drawSphere`)
//! - `references/nifxml/nif.xml:L2765-3420`

use bytemuck::{Pod, Zeroable};
use fo3_gamebryo_core::NiTransform;
use fo3_nif::{NifBlock, NifFile};
use glam::Vec3;
use std::f32::consts::PI;
use wgpu::util::DeviceExt;

/// Havok 単位 (メートル) から Gamebryo ゲーム単位 (GU) への変換スケール係数。
/// 参照元: `references/nifskope/src/gl/gltools.cpp:L327` (`hkScale660 = 1.0 / 1.42875 * 10.0 ≈ 6.999125`)
pub const HAVOK_SCALE: f32 = 1.0 / 0.142875;

/// コリジョンライン描画用頂点。
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct CollisionVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl CollisionVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<CollisionVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// GPU 上に確保されたコリジョンワイヤーフレームメッシュ。
pub struct GpuCollisionMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub num_vertices: u32,
    pub model_bind_group: wgpu::BindGroup,
}

/// 単一の NIF ファイルからコリジョンライン頂点群を抽出・構築する。
pub fn extract_collision_lines(nif: &NifFile) -> Vec<CollisionVertex> {
    let mut vertices = Vec::new();

    for block in &nif.blocks {
        match block {
            NifBlock::BhkCollisionObject(co) => {
                if co.body >= 0 && (co.body as usize) < nif.blocks.len() {
                    extract_body_lines(co.body as usize, nif, &mut vertices);
                }
            }
            NifBlock::BhkBlendCollisionObject(bco) => {
                if bco.col.body >= 0 && (bco.col.body as usize) < nif.blocks.len() {
                    extract_body_lines(bco.col.body as usize, nif, &mut vertices);
                }
            }
            _ => {}
        }
    }

    vertices
}

fn extract_body_lines(body_idx: usize, nif: &NifFile, out: &mut Vec<CollisionVertex>) {
    let block = &nif.blocks[body_idx];
    let (shape_idx, body_trans) = match block {
        NifBlock::BhkRigidBody(rb) => (rb.world_obj.shape, Vec3::ZERO),
        NifBlock::BhkRigidBodyT(rbt) => {
            let t = Vec3::new(rbt.translation[0], rbt.translation[1], rbt.translation[2]) * HAVOK_SCALE;
            (rbt.world_obj.shape, t)
        }
        _ => return,
    };

    if shape_idx >= 0 && (shape_idx as usize) < nif.blocks.len() {
        extract_shape_lines(shape_idx as usize, body_trans, nif, out);
    }
}

fn extract_shape_lines(shape_idx: usize, offset: Vec3, nif: &NifFile, out: &mut Vec<CollisionVertex>) {
    let block = &nif.blocks[shape_idx];
    match block {
        NifBlock::BhkMoppBvTreeShape(mopp) => {
            if mopp.shape >= 0 && (mopp.shape as usize) < nif.blocks.len() {
                extract_shape_lines(mopp.shape as usize, offset, nif, out);
            }
        }
        NifBlock::BhkPackedNiTriStripsShape(packed) => {
            if packed.data >= 0 && (packed.data as usize) < nif.blocks.len() {
                if let NifBlock::HkPackedNiTriStripsData(ref data) = nif.blocks[packed.data as usize] {
                    // bhkPackedNiTriStripsShape の頂点は既にゲーム単位
                    let col = [0.0, 1.0, 0.4, 1.0]; // 明るいグリーン
                    for tri in &data.triangles {
                        let i0 = tri.triangle[0] as usize;
                        let i1 = tri.triangle[1] as usize;
                        let i2 = tri.triangle[2] as usize;
                        if i0 < data.vertices.len() && i1 < data.vertices.len() && i2 < data.vertices.len() {
                            let v0 = Vec3::new(data.vertices[i0][0], data.vertices[i0][1], data.vertices[i0][2]) + offset;
                            let v1 = Vec3::new(data.vertices[i1][0], data.vertices[i1][1], data.vertices[i1][2]) + offset;
                            let v2 = Vec3::new(data.vertices[i2][0], data.vertices[i2][1], data.vertices[i2][2]) + offset;

                            // 3 本のエッジを LineList に追加
                            add_line(out, v0, v1, col);
                            add_line(out, v1, v2, col);
                            add_line(out, v2, v0, col);
                        }
                    }
                }
            }
        }
        NifBlock::BhkBoxShape(box_shape) => {
            // 直方体: ハーフエクステントに HAVOK_SCALE を乗算
            let ext = Vec3::new(box_shape.dimensions.x, box_shape.dimensions.y, box_shape.dimensions.z) * HAVOK_SCALE;
            let col = [0.0, 0.8, 1.0, 1.0]; // シアン
            add_box_lines(out, offset - ext, offset + ext, col);
        }
        NifBlock::BhkSphereShape(sphere) => {
            // 球体
            let r = sphere.radius * HAVOK_SCALE;
            let col = [1.0, 0.9, 0.0, 1.0]; // イエロー
            add_sphere_lines(out, offset, r, col);
        }
        NifBlock::BhkCapsuleShape(capsule) => {
            // カプセル
            let p1 = Vec3::new(capsule.first_point.x, capsule.first_point.y, capsule.first_point.z) * HAVOK_SCALE + offset;
            let p2 = Vec3::new(capsule.second_point.x, capsule.second_point.y, capsule.second_point.z) * HAVOK_SCALE + offset;
            let r = capsule.radius1 * HAVOK_SCALE;
            let col = [1.0, 0.3, 0.8, 1.0]; // マゼンタ
            add_capsule_lines(out, p1, p2, r, col);
        }
        NifBlock::BhkConvexVerticesShape(convex) => {
            // 凸包頂点: 頂点群に HAVOK_SCALE を乗算
            let col = [1.0, 0.5, 0.0, 1.0]; // オレンジ
            let verts: Vec<Vec3> = convex.vertices.iter()
                .map(|v| Vec3::new(v[0], v[1], v[2]) * HAVOK_SCALE + offset)
                .collect();

            // 頂点間の接続 (各頂点を近傍頂点と結ぶ)
            for i in 0..verts.len() {
                for j in (i + 1)..verts.len() {
                    let d = verts[i].distance(verts[j]);
                    // 一定距離以下の頂点ペアをエッジとして描画 (簡易凸包ワイヤーフレーム)
                    if d < (convex.radius * HAVOK_SCALE * 4.0).max(10.0) {
                        add_line(out, verts[i], verts[j], col);
                    }
                }
            }
        }
        NifBlock::BhkListShape(list) => {
            for &child_shape in &list.sub_shapes {
                if child_shape >= 0 && (child_shape as usize) < nif.blocks.len() {
                    extract_shape_lines(child_shape as usize, offset, nif, out);
                }
            }
        }
        _ => {}
    }
}

#[inline]
fn add_line(out: &mut Vec<CollisionVertex>, p0: Vec3, p1: Vec3, color: [f32; 4]) {
    out.push(CollisionVertex { position: [p0.x, p0.y, p0.z], color });
    out.push(CollisionVertex { position: [p1.x, p1.y, p1.z], color });
}

/// 直方体の 12 本のエッジを追加。
/// 参照元: `references/nifskope/src/gl/gltools.cpp:L480` (`drawBox`)
fn add_box_lines(out: &mut Vec<CollisionVertex>, min: Vec3, max: Vec3, color: [f32; 4]) {
    let p000 = Vec3::new(min.x, min.y, min.z);
    let p001 = Vec3::new(min.x, min.y, max.z);
    let p010 = Vec3::new(min.x, max.y, min.z);
    let p011 = Vec3::new(min.x, max.y, max.z);
    let p100 = Vec3::new(max.x, min.y, min.z);
    let p101 = Vec3::new(max.x, min.y, max.z);
    let p110 = Vec3::new(max.x, max.y, min.z);
    let p111 = Vec3::new(max.x, max.y, max.z);

    // 下面 4 辺
    add_line(out, p000, p100, color);
    add_line(out, p100, p110, color);
    add_line(out, p110, p010, color);
    add_line(out, p010, p000, color);

    // 上面 4 辺
    add_line(out, p001, p101, color);
    add_line(out, p101, p111, color);
    add_line(out, p111, p011, color);
    add_line(out, p011, p001, color);

    // 縦 4 辺
    add_line(out, p000, p001, color);
    add_line(out, p100, p101, color);
    add_line(out, p110, p111, color);
    add_line(out, p010, p011, color);
}

/// 球体の 3 つの大円リングを追加。
fn add_sphere_lines(out: &mut Vec<CollisionVertex>, center: Vec3, radius: f32, color: [f32; 4]) {
    let segments = 24;
    for i in 0..segments {
        let a0 = (i as f32) / (segments as f32) * PI * 2.0;
        let a1 = ((i + 1) as f32) / (segments as f32) * PI * 2.0;

        // XY 平面
        let p0_xy = center + Vec3::new(a0.cos() * radius, a0.sin() * radius, 0.0);
        let p1_xy = center + Vec3::new(a1.cos() * radius, a1.sin() * radius, 0.0);
        add_line(out, p0_xy, p1_xy, color);

        // XZ 平面
        let p0_xz = center + Vec3::new(a0.cos() * radius, 0.0, a0.sin() * radius);
        let p1_xz = center + Vec3::new(a1.cos() * radius, 0.0, a1.sin() * radius);
        add_line(out, p0_xz, p1_xz, color);

        // YZ 平面
        let p0_yz = center + Vec3::new(0.0, a0.cos() * radius, a0.sin() * radius);
        let p1_yz = center + Vec3::new(0.0, a1.cos() * radius, a1.sin() * radius);
        add_line(out, p0_yz, p1_yz, color);
    }
}

/// カプセルの端点・接続線を追加。
fn add_capsule_lines(out: &mut Vec<CollisionVertex>, p1: Vec3, p2: Vec3, radius: f32, color: [f32; 4]) {
    add_sphere_lines(out, p1, radius, color);
    add_sphere_lines(out, p2, radius, color);

    // 接続軸に直交する基底ベクトル
    let axis = (p2 - p1).normalize_or_zero();
    let up = if axis.z.abs() < 0.9 { Vec3::Z } else { Vec3::X };
    let right = axis.cross(up).normalize();
    let forward = axis.cross(right).normalize();

    // 4 本の円筒接続ライン
    add_line(out, p1 + right * radius, p2 + right * radius, color);
    add_line(out, p1 - right * radius, p2 - right * radius, color);
    add_line(out, p1 + forward * radius, p2 + forward * radius, color);
    add_line(out, p1 - forward * radius, p2 - forward * radius, color);
}

impl GpuCollisionMesh {
    /// 頂点配列とワールド変換から GpuCollisionMesh を生成する。
    pub fn new(
        device: &wgpu::Device,
        model_bind_group_layout: &wgpu::BindGroupLayout,
        vertices: &[CollisionVertex],
        world_transform: &NiTransform,
    ) -> Option<Self> {
        if vertices.is_empty() {
            return None;
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Collision Line Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let world_mat = world_transform.to_mat4();
        let model_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Collision Model Uniform Buffer"),
            contents: bytemuck::cast_slice(&world_mat.to_cols_array()),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let model_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Collision Model Bind Group"),
            layout: model_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: model_buffer.as_entire_binding(),
            }],
        });

        Some(Self {
            vertex_buffer,
            num_vertices: vertices.len() as u32,
            model_bind_group,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_box_lines() {
        let mut lines = Vec::new();
        let min = Vec3::new(-10.0, -10.0, -10.0);
        let max = Vec3::new(10.0, 10.0, 10.0);
        let col = [1.0, 0.0, 0.0, 1.0];
        add_box_lines(&mut lines, min, max, col);
        // 直方体は 12 本のエッジ = 24 頂点
        assert_eq!(lines.len(), 24);
        assert_eq!(lines[0].color, col);
    }

    #[test]
    fn test_add_sphere_lines() {
        let mut lines = Vec::new();
        let center = Vec3::ZERO;
        let r = 5.0;
        let col = [0.0, 1.0, 0.0, 1.0];
        add_sphere_lines(&mut lines, center, r, col);
        // 3 平面 (XY, XZ, YZ) に各 24 セグメント = 24 * 2 * 3 = 144 頂点
        assert_eq!(lines.len(), 144);
    }
}
