//! # NIF シェーダー・マテリアル・プロパティブロック
//!
//! 参照元:
//! - `references/nifxml/nif.xml:L6307` (`BSShaderTextureSet`)
//! - `references/nifxml/nif.xml:L6242` (`BSShaderPPLightingProperty`)
//! - `references/nifxml/nif.xml:L4363` (`NiMaterialProperty`)
//! - `references/nifxml/nif.xml:L3972` (`NiAlphaProperty`)

use std::io::{self, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::blocks::node::NiObjectNET;
use crate::types::{read_sized_string, Color3};

/// Bethesda 固有のテクスチャセットブロック。
/// ディフューズ、ノーマルマップ、グローマップ等のファイルパスリストを保持。
///
/// 参照元: `references/nifxml/nif.xml:L6307` (`BSShaderTextureSet`)
#[derive(Clone, Debug, PartialEq)]
pub struct BSShaderTextureSet {
    /// テクスチャパス配列 (通常 6〜8 件)
    /// 0: Diffuse, 1: Normal/Gloss, 2: Glow/Rim, 3: Height/Parallax, 4: Environment, 5: EnvMask
    pub textures: Vec<String>,
}

impl BSShaderTextureSet {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let num_textures = reader.read_u32::<LittleEndian>()? as usize;
        let mut textures = Vec::with_capacity(num_textures);
        for _ in 0..num_textures {
            textures.push(read_sized_string(reader)?);
        }
        Ok(BSShaderTextureSet { textures })
    }
}

/// Bethesda 固有のピクセルパーピクセルライティング用シェーダープロパティ。
///
/// 参照元:
/// - `references/nifxml/nif.xml:L6242` (`BSShaderPPLightingProperty`)
/// - `references/nifxml/nif.xml:L5062` (`NiShadeProperty`)
/// - `references/openmw/components/nif/property.cpp:L130-220` (`BSShaderProperty`, `BSShaderPPLightingProperty`)
#[derive(Clone, Debug, PartialEq)]
pub struct BSShaderPPLightingProperty {
    pub net: NiObjectNET,
    /// NiShadeProperty::flags (スムースシェーディング等フラグ)
    pub shade_flags: u16,
    pub shader_type: u32,
    pub shader_flags: u32,
    pub shader_flags2: u32,
    pub env_map_scale: f32,
    pub texture_clamp_mode: u32,
    /// BSShaderTextureSet への参照 (-1 は None)
    pub texture_set: i32,
    pub refraction_strength: f32,
    pub refraction_fire_period: i32,
    pub parallax_max_passes: f32,
    pub parallax_scale: f32,
}

impl BSShaderPPLightingProperty {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let net = NiObjectNET::read(reader)?;

        let shade_flags = reader.read_u16::<LittleEndian>()?;
        let shader_type = reader.read_u32::<LittleEndian>()?;
        let shader_flags = reader.read_u32::<LittleEndian>()?;
        let shader_flags2 = reader.read_u32::<LittleEndian>()?;
        let env_map_scale = reader.read_f32::<LittleEndian>()?;
        let texture_clamp_mode = reader.read_u32::<LittleEndian>()?;
        let texture_set = reader.read_i32::<LittleEndian>()?;
        let refraction_strength = reader.read_f32::<LittleEndian>()?;
        let refraction_fire_period = reader.read_i32::<LittleEndian>()?;
        let parallax_max_passes = reader.read_f32::<LittleEndian>()?;
        let parallax_scale = reader.read_f32::<LittleEndian>()?;

        Ok(BSShaderPPLightingProperty {
            net,
            shade_flags,
            shader_type,
            shader_flags,
            shader_flags2,
            env_map_scale,
            texture_clamp_mode,
            texture_set,
            refraction_strength,
            refraction_fire_period,
            parallax_max_passes,
            parallax_scale,
        })
    }
}

/// 標準マテリアルプロパティ（スペキュラ、エミッシブ、透明度）。
///
/// 参照元: `references/nifxml/nif.xml:L4363` (`NiMaterialProperty`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiMaterialProperty {
    pub net: NiObjectNET,
    pub specular_color: Color3,
    pub emissive_color: Color3,
    pub glossiness: f32,
    pub alpha: f32,
    pub emissive_mult: f32,
}

impl NiMaterialProperty {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let net = NiObjectNET::read(reader)?;

        let specular_color = Color3::read(reader)?;
        let emissive_color = Color3::read(reader)?;
        let glossiness = reader.read_f32::<LittleEndian>()?;
        let alpha = reader.read_f32::<LittleEndian>()?;
        let emissive_mult = reader.read_f32::<LittleEndian>()?;

        Ok(NiMaterialProperty {
            net,
            specular_color,
            emissive_color,
            glossiness,
            alpha,
            emissive_mult,
        })
    }
}

/// アルファテストおよびアルファブレンド制御プロパティ。
///
/// 参照元: `references/nifxml/nif.xml:L3972` (`NiAlphaProperty`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiAlphaProperty {
    pub net: NiObjectNET,
    pub flags: u16,
    pub threshold: u8,
}

impl NiAlphaProperty {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let net = NiObjectNET::read(reader)?;
        let flags = reader.read_u16::<LittleEndian>()?;
        let threshold = reader.read_u8()?;

        Ok(NiAlphaProperty {
            net,
            flags,
            threshold,
        })
    }
}
