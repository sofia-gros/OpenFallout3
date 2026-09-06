//! # NIF 物理非依存コリジョン抽出モジュール
//!
//! NIF ファイル内の Havok コリジョンブロック群 (`bhkCollisionObject`, `bhkRigidBody`,
//! `bhkMoppBvTreeShape`, `bhkPackedNiTriStripsShape`, `bhkBoxShape`, `bhkSphereShape`,
//! `bhkCapsuleShape`, `bhkConvexVerticesShape`, `bhkConvexTransformShape`, `bhkConvexListShape`)
//! から、描画システムや特定の物理エンジン（Havok / Rapier）に依存しない純粋な
//! コリジョン幾何形状および力学パラメータを抽出・構築します。
//!
//! 参照元:
//! - `references/nifskope/src/gl/gltools.cpp:L327-366` (`hkScale660`, `bhkBodyTrans`)
//! - `references/nifskope/src/gl/glnode.cpp:L742-897` (`drawHvkShape`)
//! - `references/nifskope/src/data/niftypes.h:L1018` (`Matrix4::operator*`)
//! - `references/nifxml/nif.xml:L2765-3420, L444, L719`

use crate::blocks::{Fallout3HavokMaterial, Fallout3Layer};
use crate::{NifBlock, NifFile};

/// Havok 物理単位 (メートル) から Gamebryo ゲーム単位 (GU) への変換スケール係数。
///
/// 参照元: `references/nifskope/src/gl/gltools.cpp:L327` (`hkScale660 = 1.0 / 1.42875 * 10.0 ≈ 6.999125`)
pub const HAVOK_SCALE: f32 = 1.0 / 0.142875;

/// 物理エンジン非依存コリジョン形状。
#[derive(Clone, Debug, PartialEq)]
pub enum CollisionShape {
    /// 三角形メッシュ (`bhkPackedNiTriStripsShape` / `hkPackedNiTriStripsData`)
    TriMesh {
        /// 頂点座標リスト (Gamebryo ゲーム単位)
        vertices: Vec<[f32; 3]>,
        /// 三角形インデックスリスト (頂点番号の3つ組)
        indices: Vec<[u32; 3]>,
        /// 材質
        material: Fallout3HavokMaterial,
    },
    /// 直方体 (`bhkBoxShape`)
    Box {
        /// 半分の幅・高さ・奥行き (Half Extents, Gamebryo ゲーム単位)
        half_extents: [f32; 3],
        /// ローカル中心座標 (Gamebryo ゲーム単位)
        center: [f32; 3],
        /// 材質
        material: Fallout3HavokMaterial,
    },
    /// 球体 (`bhkSphereShape`)
    Sphere {
        /// 中心座標 (Gamebryo ゲーム単位)
        center: [f32; 3],
        /// 半径 (Gamebryo ゲーム単位)
        radius: f32,
        /// 材質
        material: Fallout3HavokMaterial,
    },
    /// カプセル (`bhkCapsuleShape`)
    Capsule {
        /// 第1の端点 (Gamebryo ゲーム単位)
        p1: [f32; 3],
        /// 第2の端点 (Gamebryo ゲーム単位)
        p2: [f32; 3],
        /// カプセル半径 (Gamebryo ゲーム単位)
        radius: f32,
        /// 材質
        material: Fallout3HavokMaterial,
    },
    /// 凸包ポリゴン (`bhkConvexVerticesShape`)
    ConvexHull {
        /// 凸頂点リスト (Gamebryo ゲーム単位)
        vertices: Vec<[f32; 3]>,
        /// 材質
        material: Fallout3HavokMaterial,
    },
    /// 複合形状 (`bhkListShape`, `bhkConvexListShape`, 局所変換適用済み形状など)
    Compound(Vec<CollisionShape>),
}

impl CollisionShape {
    /// この形状に含まれる総ポリゴン（三角形）数概算を取得する。
    pub fn triangle_count(&self) -> usize {
        match self {
            Self::TriMesh { indices, .. } => indices.len(),
            Self::Box { .. } => 12,
            Self::Sphere { .. } => 0,
            Self::Capsule { .. } => 0,
            Self::ConvexHull { vertices, .. } => vertices.len().saturating_sub(2),
            Self::Compound(children) => children.iter().map(|c| c.triangle_count()).sum(),
        }
    }
}

/// 剛体物理パラメータおよび形状データ。
#[derive(Clone, Debug, PartialEq)]
pub struct RigidBodyData {
    /// コリジョン形状
    pub shape: CollisionShape,
    /// 質量 (kg)。0.0 は不動の静的オブジェクト (Static)
    pub mass: f32,
    /// 摩擦係数 (デフォルト 0.5)
    pub friction: f32,
    /// 反発係数 (デフォルト 0.4)
    pub restitution: f32,
    /// 線形減衰率
    pub linear_damping: f32,
    /// 角減衰率
    pub angular_damping: f32,
    /// コリジョン接触レイヤー
    pub layer: Fallout3Layer,
    /// 平行移動オフセット (Gamebryo ゲーム単位)
    pub translation: [f32; 3],
    /// 回転クォータニオン (X, Y, Z, W)
    pub rotation: [f32; 4],
}

/// 単一の NIF ファイルから抽出された全剛体・コリジョンデータ。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NifCollisionData {
    /// 剛体リスト
    pub bodies: Vec<RigidBodyData>,
}

impl NifCollisionData {
    /// コリジョンが一切存在しないか判定する。
    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }
}

/// NIF ファイルから物理コリジョンデータを抽出する。
///
/// 参照元: `references/nifskope/src/gl/glnode.cpp:L742-897`
pub fn extract_collision_data(nif: &NifFile) -> NifCollisionData {
    let mut bodies = Vec::new();

    for block in &nif.blocks {
        match block {
            NifBlock::BhkCollisionObject(co) | NifBlock::BhkSPCollisionObject(co) => {
                if co.body >= 0 && (co.body as usize) < nif.blocks.len() {
                    if let Some(body_data) = extract_rigid_body(co.body as usize, nif) {
                        bodies.push(body_data);
                    }
                }
            }
            NifBlock::BhkBlendCollisionObject(bco) => {
                if bco.col.body >= 0 && (bco.col.body as usize) < nif.blocks.len() {
                    if let Some(body_data) = extract_rigid_body(bco.col.body as usize, nif) {
                        bodies.push(body_data);
                    }
                }
            }
            _ => {}
        }
    }

    NifCollisionData { bodies }
}

/// 剛体ブロックまたはファントムから物理剛体データを抽出する。
fn extract_rigid_body(body_idx: usize, nif: &NifFile) -> Option<RigidBodyData> {
    let block = &nif.blocks[body_idx];

    match block {
        NifBlock::BhkRigidBody(rb) => {
            let shape_idx = rb.world_obj.shape;
            if shape_idx < 0 || (shape_idx as usize) >= nif.blocks.len() {
                return None;
            }
            let shape = extract_shape(shape_idx as usize, nif)?;
            Some(RigidBodyData {
                translation: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
                shape,
                layer: Fallout3Layer::from(rb.world_obj.havok_filter_layer),
                mass: rb.mass,
                linear_damping: rb.linear_damping,
                angular_damping: rb.angular_damping,
                friction: rb.friction,
                restitution: rb.restitution,
            })
        }
        NifBlock::BhkRigidBodyT(rbt) => {
            let shape_idx = rbt.world_obj.shape;
            if shape_idx < 0 || (shape_idx as usize) >= nif.blocks.len() {
                return None;
            }
            let shape = extract_shape(shape_idx as usize, nif)?;
            let t = [
                rbt.translation[0] * HAVOK_SCALE,
                rbt.translation[1] * HAVOK_SCALE,
                rbt.translation[2] * HAVOK_SCALE,
            ];
            Some(RigidBodyData {
                translation: t,
                rotation: rbt.rotation,
                shape,
                layer: Fallout3Layer::from(rbt.world_obj.havok_filter_layer),
                mass: rbt.mass,
                linear_damping: rbt.linear_damping,
                angular_damping: rbt.angular_damping,
                friction: rbt.friction,
                restitution: rbt.restitution,
            })
        }
        NifBlock::BhkSimpleShapePhantom(p) => {
            let shape_idx = p.common.shape;
            if shape_idx < 0 || (shape_idx as usize) >= nif.blocks.len() {
                return None;
            }
            let inner_shape = extract_shape(shape_idx as usize, nif)?;
            let shape = apply_transform_to_shape(inner_shape, &p.transform);
            Some(RigidBodyData {
                translation: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
                shape,
                layer: Fallout3Layer::from(p.common.havok_filter_layer),
                mass: 0.0,
                linear_damping: 0.0,
                angular_damping: 0.0,
                friction: 0.0,
                restitution: 0.0,
            })
        }
        _ => None,
    }
}

/// 形状ブロックからコリジョン形状を再帰的に抽出する。
fn extract_shape(shape_idx: usize, nif: &NifFile) -> Option<CollisionShape> {
    let block = &nif.blocks[shape_idx];

    match block {
        NifBlock::BhkMoppBvTreeShape(mopp) => {
            if mopp.shape >= 0 && (mopp.shape as usize) < nif.blocks.len() {
                extract_shape(mopp.shape as usize, nif)
            } else {
                None
            }
        }
        NifBlock::BhkPackedNiTriStripsShape(packed) => {
            if packed.data >= 0 && (packed.data as usize) < nif.blocks.len() {
                if let NifBlock::HkPackedNiTriStripsData(ref data) = nif.blocks[packed.data as usize] {
                    // hkPackedNiTriStripsData の頂点は Havok 物理単位 (メートル) で格納されているため、
                    // HAVOK_SCALE (1.0 / 0.142875 ≈ 6.999125) を乗算して Gamebryo ゲーム単位に変換する。
                    // 参照元: references/nifskope/src/gl/gltools.cpp:L327 (hkScale660), references/nifskope/src/gl/glnode.cpp:L883
                    let vertices: Vec<[f32; 3]> = data
                        .vertices
                        .iter()
                        .map(|v| [v[0] * HAVOK_SCALE, v[1] * HAVOK_SCALE, v[2] * HAVOK_SCALE])
                        .collect();
                    let indices: Vec<[u32; 3]> = data
                        .triangles
                        .iter()
                        .map(|t| [t.triangle[0] as u32, t.triangle[1] as u32, t.triangle[2] as u32])
                        .collect();

                    let material = data
                        .sub_shapes
                        .first()
                        .map(|s| Fallout3HavokMaterial::from(s.material))
                        .unwrap_or(Fallout3HavokMaterial::Stone);

                    Some(CollisionShape::TriMesh {
                        vertices,
                        indices,
                        material,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        }
        NifBlock::BhkBoxShape(box_shape) => {
            // 直方体: ハーフエクステントに HAVOK_SCALE を乗算
            let half_extents = [
                box_shape.dimensions.x * HAVOK_SCALE,
                box_shape.dimensions.y * HAVOK_SCALE,
                box_shape.dimensions.z * HAVOK_SCALE,
            ];
            Some(CollisionShape::Box {
                half_extents,
                center: [0.0, 0.0, 0.0],
                material: Fallout3HavokMaterial::from(box_shape.material),
            })
        }
        NifBlock::BhkSphereShape(sphere) => {
            Some(CollisionShape::Sphere {
                center: [0.0, 0.0, 0.0],
                radius: sphere.radius * HAVOK_SCALE,
                material: Fallout3HavokMaterial::from(sphere.material),
            })
        }
        NifBlock::BhkCapsuleShape(capsule) => {
            let p1 = [
                capsule.first_point.x * HAVOK_SCALE,
                capsule.first_point.y * HAVOK_SCALE,
                capsule.first_point.z * HAVOK_SCALE,
            ];
            let p2 = [
                capsule.second_point.x * HAVOK_SCALE,
                capsule.second_point.y * HAVOK_SCALE,
                capsule.second_point.z * HAVOK_SCALE,
            ];
            let radius = capsule.radius1 * HAVOK_SCALE;

            Some(CollisionShape::Capsule {
                p1,
                p2,
                radius,
                material: Fallout3HavokMaterial::from(capsule.material),
            })
        }
        NifBlock::BhkConvexVerticesShape(convex) => {
            let vertices: Vec<[f32; 3]> = convex
                .vertices
                .iter()
                .map(|v| [v[0] * HAVOK_SCALE, v[1] * HAVOK_SCALE, v[2] * HAVOK_SCALE])
                .collect();

            Some(CollisionShape::ConvexHull {
                vertices,
                material: Fallout3HavokMaterial::from(convex.material),
            })
        }
        NifBlock::BhkConvexTransformShape(ct) => {
            if ct.shape >= 0 && (ct.shape as usize) < nif.blocks.len() {
                let inner = extract_shape(ct.shape as usize, nif)?;
                // 4x4 行列を内包形状に適用
                Some(apply_transform_to_shape(inner, &ct.transform))
            } else {
                None
            }
        }
        NifBlock::BhkTransformShape(ts) => {
            if ts.shape >= 0 && (ts.shape as usize) < nif.blocks.len() {
                let inner = extract_shape(ts.shape as usize, nif)?;
                Some(apply_transform_to_shape(inner, &ts.transform))
            } else {
                None
            }
        }
        NifBlock::BhkNiTriStripsShape(ss) => {
            let mut all_verts = Vec::new();
            let mut all_tris = Vec::new();
            for &data_idx in &ss.strips_data {
                if data_idx >= 0 && (data_idx as usize) < nif.blocks.len() {
                    if let NifBlock::NiTriStripsData(ref strips_data) = nif.blocks[data_idx as usize] {
                        let base_v = all_verts.len() as u32;
                        for v in &strips_data.common.vertices {
                            all_verts.push([v.x * HAVOK_SCALE, v.y * HAVOK_SCALE, v.z * HAVOK_SCALE]);
                        }
                        for strip in &strips_data.strips {
                            for j in 0..strip.len().saturating_sub(2) {
                                let i0 = strip[j];
                                let i1 = strip[j + 1];
                                let i2 = strip[j + 2];
                                if i0 != i1 && i1 != i2 && i0 != i2 {
                                    if j % 2 == 0 {
                                        all_tris.push([base_v + i0 as u32, base_v + i1 as u32, base_v + i2 as u32]);
                                    } else {
                                        all_tris.push([base_v + i1 as u32, base_v + i0 as u32, base_v + i2 as u32]);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if all_verts.is_empty() {
                None
            } else {
                Some(CollisionShape::TriMesh {
                    vertices: all_verts,
                    indices: all_tris,
                    material: ss.material,
                })
            }
        }
        NifBlock::BhkListShape(list) => {
            let mut children = Vec::new();
            for &sub_idx in &list.sub_shapes {
                if sub_idx >= 0 && (sub_idx as usize) < nif.blocks.len() {
                    if let Some(child) = extract_shape(sub_idx as usize, nif) {
                        children.push(child);
                    }
                }
            }
            Some(CollisionShape::Compound(children))
        }
        NifBlock::BhkConvexListShape(list) => {
            let mut children = Vec::new();
            for &sub_idx in &list.sub_shapes {
                if sub_idx >= 0 && (sub_idx as usize) < nif.blocks.len() {
                    if let Some(child) = extract_shape(sub_idx as usize, nif) {
                        children.push(child);
                    }
                }
            }
            Some(CollisionShape::Compound(children))
        }
        _ => None,
    }
}

/// 4x4 行列による 3次元座標点の変換。
///
/// 参照元: `references/nifskope/src/data/niftypes.h:L1018`
fn transform_point(m: &[f32; 16], p: [f32; 3]) -> [f32; 3] {
    [
        m[0] * p[0] + m[1] * p[1] + m[2] * p[2] + m[3] * HAVOK_SCALE,
        m[4] * p[0] + m[5] * p[1] + m[6] * p[2] + m[7] * HAVOK_SCALE,
        m[8] * p[0] + m[9] * p[1] + m[10] * p[2] + m[11] * HAVOK_SCALE,
    ]
}

/// 局所変換行列をコリジョン形状に適用する。
fn apply_transform_to_shape(shape: CollisionShape, m: &[f32; 16]) -> CollisionShape {
    match shape {
        CollisionShape::TriMesh {
            mut vertices,
            indices,
            material,
        } => {
            for v in &mut vertices {
                *v = transform_point(m, *v);
            }
            CollisionShape::TriMesh {
                vertices,
                indices,
                material,
            }
        }
        CollisionShape::ConvexHull {
            mut vertices,
            material,
        } => {
            for v in &mut vertices {
                *v = transform_point(m, *v);
            }
            CollisionShape::ConvexHull { vertices, material }
        }
        CollisionShape::Box {
            half_extents,
            center,
            material,
        } => {
            let new_center = transform_point(m, center);
            CollisionShape::Box {
                half_extents,
                center: new_center,
                material,
            }
        }
        CollisionShape::Sphere {
            center,
            radius,
            material,
        } => {
            let new_center = transform_point(m, center);
            CollisionShape::Sphere {
                center: new_center,
                radius,
                material,
            }
        }
        CollisionShape::Capsule {
            p1,
            p2,
            radius,
            material,
        } => {
            let new_p1 = transform_point(m, p1);
            let new_p2 = transform_point(m, p2);
            CollisionShape::Capsule {
                p1: new_p1,
                p2: new_p2,
                radius,
                material,
            }
        }
        CollisionShape::Compound(children) => {
            let transformed = children
                .into_iter()
                .map(|c| apply_transform_to_shape(c, m))
                .collect();
            CollisionShape::Compound(transformed)
        }
    }
}

