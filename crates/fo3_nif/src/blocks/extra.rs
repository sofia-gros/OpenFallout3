//! # NIF 追加データ (ExtraData) ブロック定義
//!
//! 参照元:
//! - `references/nifxml/nif.xml:L3220` (`NiExtraData`)
//! - `references/nifxml/nif.xml:L4264` (`NiFloatExtraData`)
//! - `references/nifxml/nif.xml:L4293` (`NiIntegerExtraData`)
//! - `references/nifxml/nif.xml:L4298` (`BSXFlags`)
//! - `references/nifxml/nif.xml:L5163` (`NiStringExtraData`)
//! - `references/nifxml/nif.xml:L3932` (`BSBound`)

use std::io::{self, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::types::Vector3;

/// 文字列追加データブロック。
///
/// 参照元: `references/nifxml/nif.xml:L5163` (`NiStringExtraData`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiStringExtraData {
    /// エクストラデータ名（文字列プールインデックス）
    pub name_index: u32,
    /// 格納されている文字列（文字列プールインデックス）
    pub string_data_index: u32,
}

impl NiStringExtraData {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let name_index = reader.read_u32::<LittleEndian>()?;
        let string_data_index = reader.read_u32::<LittleEndian>()?;
        Ok(NiStringExtraData {
            name_index,
            string_data_index,
        })
    }
}

/// 整数追加データブロック。
///
/// 参照元: `references/nifxml/nif.xml:L4293` (`NiIntegerExtraData`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiIntegerExtraData {
    /// エクストラデータ名（文字列プールインデックス）
    pub name_index: u32,
    /// 整数値
    pub integer_data: u32,
}

impl NiIntegerExtraData {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let name_index = reader.read_u32::<LittleEndian>()?;
        let integer_data = reader.read_u32::<LittleEndian>()?;
        Ok(NiIntegerExtraData {
            name_index,
            integer_data,
        })
    }
}

/// Bethesda 固有の物理・アニメーション・機能制御フラグブロック。
///
/// 参照元: `references/nifxml/nif.xml:L4298` (`BSXFlags`)
#[derive(Clone, Debug, PartialEq)]
pub struct BSXFlags {
    /// エクストラデータ名（文字列プールインデックス）
    pub name_index: u32,
    /// ビットフラグ
    pub flags: u32,
}

impl BSXFlags {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let name_index = reader.read_u32::<LittleEndian>()?;
        let flags = reader.read_u32::<LittleEndian>()?;
        Ok(BSXFlags { name_index, flags })
    }

    /// Havok 物理シミュレーションが有効か判定 (Bit 0)
    #[inline]
    pub fn has_havok(&self) -> bool {
        (self.flags & 0x0001) != 0
    }

    /// コリジョン判定が有効か判定 (Bit 1)
    #[inline]
    pub fn has_collision(&self) -> bool {
        (self.flags & 0x0002) != 0
    }

    /// スケルトン NIF か判定 (Bit 2)
    #[inline]
    pub fn is_skeleton(&self) -> bool {
        (self.flags & 0x0004) != 0
    }

    /// アニメーションが有効か判定 (Bit 3)
    #[inline]
    pub fn has_animation(&self) -> bool {
        (self.flags & 0x0008) != 0
    }

    /// エディタマーカーが存在するか判定 (Bit 5)
    #[inline]
    pub fn has_editor_markers(&self) -> bool {
        (self.flags & 0x0020) != 0
    }

    /// 動的 (Dynamic) オブジェクトか判定 (Bit 6)
    #[inline]
    pub fn is_dynamic(&self) -> bool {
        (self.flags & 0x0040) != 0
    }
}

/// 浮動小数点数追加データブロック。
///
/// 参照元: `references/nifxml/nif.xml:L4264` (`NiFloatExtraData`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiFloatExtraData {
    /// エクストラデータ名（文字列プールインデックス）
    pub name_index: u32,
    /// 浮動小数点数値
    pub float_data: f32,
}

impl NiFloatExtraData {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let name_index = reader.read_u32::<LittleEndian>()?;
        let float_data = reader.read_f32::<LittleEndian>()?;
        Ok(NiFloatExtraData {
            name_index,
            float_data,
        })
    }
}

/// Bethesda 固有のバウンディングボックスブロック。
///
/// 参照元: `references/nifxml/nif.xml:L3932` (`BSBound`)
#[derive(Clone, Debug, PartialEq)]
pub struct BSBound {
    /// エクストラデータ名（文字列プールインデックス）
    pub name_index: u32,
    /// バウンディングボックス中心座標
    pub center: Vector3,
    /// バウンディングボックスのハーフエクステント（中心からの半分の幅・高さ・奥行き）
    pub dimensions: Vector3,
}

impl BSBound {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let name_index = reader.read_u32::<LittleEndian>()?;
        let center = Vector3::read(reader)?;
        let dimensions = Vector3::read(reader)?;
        Ok(BSBound {
            name_index,
            center,
            dimensions,
        })
    }
}
