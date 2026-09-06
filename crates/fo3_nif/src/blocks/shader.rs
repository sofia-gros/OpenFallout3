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

    /// アルファブレンド（半透明合成）が有効か判定する。
    /// 参照元: `references/nifskope/build/nif.xml:L1520` (bit 0)
    #[inline]
    pub fn is_blend_enabled(&self) -> bool {
        (self.flags & 0x0001) != 0
    }

    /// 送信元ブレンドモード (`AlphaFunction`) を取得する。
    /// 参照元: `references/nifskope/build/nif.xml:L1521` (bit 1..4)
    #[inline]
    pub fn src_blend_mode(&self) -> u8 {
        ((self.flags >> 1) & 0x0F) as u8
    }

    /// 送信先ブレンドモード (`AlphaFunction`) を取得する。
    /// 参照元: `references/nifskope/build/nif.xml:L1522` (bit 5..8)
    #[inline]
    pub fn dst_blend_mode(&self) -> u8 {
        ((self.flags >> 5) & 0x0F) as u8
    }

    /// アルファテスト（カットアウト）が有効か判定する。
    /// 参照元: `references/nifskope/build/nif.xml:L1523` (bit 9)
    #[inline]
    pub fn is_test_enabled(&self) -> bool {
        (self.flags & 0x0200) != 0
    }

    /// アルファテスト比較関数 (`TestFunction`) を取得する。
    /// 参照元: `references/nifskope/build/nif.xml:L1524` (bit 10..12)
    /// 0: ALWAYS, 1: LESS, 2: EQUAL, 3: LESS_EQUAL, 4: GREATER, 5: NOT_EQUAL, 6: GREATER_EQUAL, 7: NEVER
    #[inline]
    pub fn test_func(&self) -> u8 {
        ((self.flags >> 10) & 0x07) as u8
    }

    /// カメラ距離によるソートが無効（No Sorter）であるか判定する。
    /// 参照元: `references/nifskope/build/nif.xml:L1525` (bit 13)
    #[inline]
    pub fn is_no_sorter(&self) -> bool {
        (self.flags & 0x2000) != 0
    }

    /// 0.0〜1.0 に正規化されたアルファテスト閾値を取得する。
    /// 参照元: `references/nifskope/src/gl/glproperty.cpp:L229` (`threshold / 255.0`)
    #[inline]
    pub fn threshold_normalized(&self) -> f32 {
        self.threshold as f32 / 255.0
    }
}

/// Bethesda 固有の非ライティングシェーダープロパティ。
/// 主に発光オブジェクト、スカイボックス、UI、特殊エフェクト用。
///
/// 参照元:
/// - `references/nifxml/nif.xml:L6233` (`BSShaderNoLightingProperty`)
#[derive(Clone, Debug, PartialEq)]
pub struct BSShaderNoLightingProperty {
    pub net: NiObjectNET,
    /// スムースシェーディングフラグ
    pub shade_flags: u16,
    pub shader_type: u32,
    pub shader_flags: u32,
    pub shader_flags2: u32,
    pub env_map_scale: f32,
    pub texture_clamp_mode: u32,
    /// 発光テクスチャ（Glow Map）ファイルパス
    pub file_name: String,
    pub falloff_start_angle: f32,
    pub falloff_stop_angle: f32,
    pub falloff_start_opacity: f32,
    pub falloff_stop_opacity: f32,
}

impl BSShaderNoLightingProperty {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let net = NiObjectNET::read(reader)?;
        let shade_flags = reader.read_u16::<LittleEndian>()?;
        let shader_type = reader.read_u32::<LittleEndian>()?;
        let shader_flags = reader.read_u32::<LittleEndian>()?;
        let shader_flags2 = reader.read_u32::<LittleEndian>()?;
        let env_map_scale = reader.read_f32::<LittleEndian>()?;
        let texture_clamp_mode = reader.read_u32::<LittleEndian>()?;
        let file_name = read_sized_string(reader)?;
        let falloff_start_angle = reader.read_f32::<LittleEndian>()?;
        let falloff_stop_angle = reader.read_f32::<LittleEndian>()?;
        let falloff_start_opacity = reader.read_f32::<LittleEndian>()?;
        let falloff_stop_opacity = reader.read_f32::<LittleEndian>()?;

        Ok(BSShaderNoLightingProperty {
            net,
            shade_flags,
            shader_type,
            shader_flags,
            shader_flags2,
            env_map_scale,
            texture_clamp_mode,
            file_name,
            falloff_start_angle,
            falloff_stop_angle,
            falloff_start_opacity,
            falloff_stop_opacity,
        })
    }
}

/// ステンシルおよび両面描画制御プロパティ。
///
/// 参照元:
/// - `references/nifxml/nif.xml:L5147` (`NiStencilProperty`)
/// - `references/nifxml/nif.xml:L1572` (`StencilFlags`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiStencilProperty {
    pub net: NiObjectNET,
    /// ステンシルフラグ (`StencilFlags`: Bit 0=Enable, Bit 10-11=Draw Mode)
    pub flags: u16,
    pub stencil_ref: u32,
    pub stencil_mask: u32,
}

impl NiStencilProperty {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let net = NiObjectNET::read(reader)?;
        let flags = reader.read_u16::<LittleEndian>()?;
        let stencil_ref = reader.read_u32::<LittleEndian>()?;
        let stencil_mask = reader.read_u32::<LittleEndian>()?;

        Ok(NiStencilProperty {
            net,
            flags,
            stencil_ref,
            stencil_mask,
        })
    }

    /// ステンシルテストが有効か判定 (Bit 0)
    #[inline]
    pub fn is_stencil_enabled(&self) -> bool {
        (self.flags & 0x0001) != 0
    }

    /// 描画モード (Draw Mode: 0=CCW, 1=CW, 2/3=Both/両面)
    /// 参照元: `references/nifxml/nif.xml:L1578` (`StencilDrawMode`)
    #[inline]
    pub fn draw_mode(&self) -> u16 {
        (self.flags >> 10) & 0x0003
    }

    /// 両面描画（カリング無効）が指定されているか判定
    #[inline]
    pub fn is_double_sided(&self) -> bool {
        let mode = self.draw_mode();
        mode == 2 || mode == 3 || mode == 0 // DRAW_BOTH または DRAW_DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alpha_property_flags() {
        // 標準的なカットアウト設定: Alpha Test 有効 (bit 9), TEST_GREATER (func=4, bit 10..12 => 4 << 10 = 0x1000)
        // flags = 0x1200 (4608), threshold = 128 (0.5019)
        let alpha = NiAlphaProperty {
            net: NiObjectNET {
                name_index: u32::MAX,
                extra_data_list: Vec::new(),
                controller: -1,
            },
            flags: 0x1200,
            threshold: 128,
        };

        assert!(!alpha.is_blend_enabled());
        assert!(alpha.is_test_enabled());
        assert_eq!(alpha.test_func(), 4); // TEST_GREATER
        assert!(!alpha.is_no_sorter());
        assert!((alpha.threshold_normalized() - 0.50196).abs() < 0.001);
    }
}

