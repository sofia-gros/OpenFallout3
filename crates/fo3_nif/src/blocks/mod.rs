//! # NIF ブロック定義モジュール

pub mod geometry;
pub mod node;
pub mod shader;

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
    /// 現段階でパース未対応のブロック（Havok 等）は生バイト列として安全に保持
    Unknown {
        type_name: String,
        data: Vec<u8>,
    },
}
