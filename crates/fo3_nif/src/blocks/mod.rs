//! # NIF ブロック定義モジュール

pub mod bhk;
pub mod extra;
pub mod geometry;
pub mod node;
pub mod shader;
pub mod skin;

pub use bhk::{
    BhkBlendCollisionObject, BhkBoxShape, BhkCapsuleShape, BhkCollisionObject,
    BhkConvexListShape, BhkConvexTransformShape, BhkConvexVerticesShape,
    BhkListShape, BhkMoppBvTreeShape, BhkNiTriStripsShape, BhkPackedNiTriStripsShape,
    BhkRigidBody, BhkSimpleShapePhantom, BhkSphereShape, BhkTransformShape,
    BhkWorldObjectCommon, Fallout3HavokMaterial, Fallout3Layer,
    HkPackedNiTriStripsData, HkSubPartData, HkTriangleData,
};
pub use extra::{BSBound, BSXFlags, NiFloatExtraData, NiIntegerExtraData, NiStringExtraData};
pub use geometry::{NiGeometry, NiGeometryDataCommon, NiTriShape, NiTriShapeData, NiTriStrips, NiTriStripsData};
pub use node::{BSFadeNode, NiAVObject, NiNode, NiObjectNET};
pub use shader::{
    BSShaderNoLightingProperty, BSShaderPPLightingProperty, BSShaderTextureSet,
    NiAlphaProperty, NiMaterialProperty, NiStencilProperty,
};
pub use skin::{BoneData, BoneVertData, BSDismemberSkinInstance, BodyPartList, NiSkinData, NiSkinInstance, NiSkinPartition, SkinPartition};



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
    BSShaderNoLightingProperty(BSShaderNoLightingProperty),
    BSShaderTextureSet(BSShaderTextureSet),
    NiMaterialProperty(NiMaterialProperty),
    NiAlphaProperty(NiAlphaProperty),
    NiStencilProperty(NiStencilProperty),
    // ExtraData ブロック
    BSXFlags(BSXFlags),
    NiStringExtraData(NiStringExtraData),
    NiIntegerExtraData(NiIntegerExtraData),
    NiFloatExtraData(NiFloatExtraData),
    BSBound(BSBound),
    // スキン・ボーンブロック
    NiSkinData(NiSkinData),
    NiSkinInstance(NiSkinInstance),
    NiSkinPartition(NiSkinPartition),
    BSDismemberSkinInstance(BSDismemberSkinInstance),

    // Havok コリジョンブロック
    BhkCollisionObject(BhkCollisionObject),
    BhkSPCollisionObject(BhkCollisionObject),
    BhkBlendCollisionObject(BhkBlendCollisionObject),
    BhkRigidBody(BhkRigidBody),
    BhkRigidBodyT(BhkRigidBody),
    BhkMoppBvTreeShape(BhkMoppBvTreeShape),
    BhkPackedNiTriStripsShape(BhkPackedNiTriStripsShape),
    BhkNiTriStripsShape(BhkNiTriStripsShape),
    HkPackedNiTriStripsData(HkPackedNiTriStripsData),
    BhkBoxShape(BhkBoxShape),
    BhkSphereShape(BhkSphereShape),
    BhkCapsuleShape(BhkCapsuleShape),
    BhkConvexVerticesShape(BhkConvexVerticesShape),
    BhkConvexTransformShape(BhkConvexTransformShape),
    BhkTransformShape(BhkTransformShape),
    BhkConvexListShape(BhkConvexListShape),
    BhkListShape(BhkListShape),
    BhkSimpleShapePhantom(BhkSimpleShapePhantom),
    /// 現段階でパース未対応のブロックは生バイト列として安全に保持
    Unknown {
        type_name: String,
        data: Vec<u8>,
    },
}
