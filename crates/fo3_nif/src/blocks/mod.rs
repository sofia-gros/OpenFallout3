//! # NIF ブロック定義モジュール

pub mod bhk;
pub mod geometry;
pub mod node;
pub mod shader;

pub use bhk::{
    BhkBlendCollisionObject, BhkBoxShape, BhkCapsuleShape, BhkCollisionObject,
    BhkConvexVerticesShape, BhkListShape, BhkMoppBvTreeShape,
    BhkPackedNiTriStripsShape, BhkRigidBody, BhkSphereShape,
    BhkWorldObjectCommon, HkPackedNiTriStripsData, HkSubPartData, HkTriangleData,
};
pub use geometry::{NiGeometry, NiGeometryDataCommon, NiTriShape, NiTriShapeData, NiTriStrips, NiTriStripsData};
pub use node::{BSFadeNode, NiAVObject, NiNode, NiObjectNET};
pub use shader::{BSShaderPPLightingProperty, BSShaderTextureSet, NiAlphaProperty, NiMaterialProperty};

/// パースされた NIF ブロックの列挙型。
#[derive(Clone, Debug, PartialEq)]
pub enum NifBlock {
    NiNode(NiNode),
    BSFadeNode(BSFadeNode),
    NiTriShape(NiTriShape),
    NiTriShapeData(NiTriShapeData),
    NiTriStrips(NiTriStrips),
    NiTriStripsData(NiTriStripsData),
    BSShaderPPLightingProperty(BSShaderPPLightingProperty),
    BSShaderTextureSet(BSShaderTextureSet),
    NiMaterialProperty(NiMaterialProperty),
    NiAlphaProperty(NiAlphaProperty),
    // Havok コリジョンブロック
    BhkCollisionObject(BhkCollisionObject),
    BhkBlendCollisionObject(BhkBlendCollisionObject),
    BhkRigidBody(BhkRigidBody),
    BhkRigidBodyT(BhkRigidBody),
    BhkMoppBvTreeShape(BhkMoppBvTreeShape),
    BhkPackedNiTriStripsShape(BhkPackedNiTriStripsShape),
    HkPackedNiTriStripsData(HkPackedNiTriStripsData),
    BhkBoxShape(BhkBoxShape),
    BhkSphereShape(BhkSphereShape),
    BhkCapsuleShape(BhkCapsuleShape),
    BhkConvexVerticesShape(BhkConvexVerticesShape),
    BhkListShape(BhkListShape),
    /// 現段階でパース未対応のブロックは生バイト列として安全に保持
    Unknown {
        type_name: String,
        data: Vec<u8>,
    },
}
