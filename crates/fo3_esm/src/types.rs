//! # ESM 基本型定義
//!
//! 4文字シグネチャ (FourCC)、FormID、境界ボックス等の基本型。
//! 参照元: `references/openmw/components/esm4/reader.hpp`

use std::fmt;

/// 4文字の ASCII シグネチャ（レコード型やサブレコード型を表す）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FourCC(pub [u8; 4]);

impl FourCC {
    pub const fn new(b: &[u8; 4]) -> Self {
        FourCC(*b)
    }

    /// ASCII 文字列として取得（無効な UTF-8 の場合はフォールバック）。
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("????")
    }
}

impl fmt::Debug for FourCC {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FourCC(\"{}\")", self.as_str())
    }
}

impl fmt::Display for FourCC {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<[u8; 4]> for FourCC {
    fn from(b: [u8; 4]) -> Self {
        FourCC(b)
    }
}

impl From<&[u8; 4]> for FourCC {
    fn from(b: &[u8; 4]) -> Self {
        FourCC(*b)
    }
}

// 主要な FourCC 定数
pub const REC_TES4: FourCC = FourCC(*b"TES4");
pub const REC_GRUP: FourCC = FourCC(*b"GRUP");
pub const REC_GMST: FourCC = FourCC(*b"GMST");
pub const REC_STAT: FourCC = FourCC(*b"STAT");
pub const REC_CELL: FourCC = FourCC(*b"CELL");
pub const REC_WRLD: FourCC = FourCC(*b"WRLD");
pub const REC_REFR: FourCC = FourCC(*b"REFR");
pub const REC_ACHR: FourCC = FourCC(*b"ACHR");
pub const REC_LAND: FourCC = FourCC(*b"LAND");
pub const REC_NAVM: FourCC = FourCC(*b"NAVM");

pub const SUB_EDID: FourCC = FourCC(*b"EDID");
pub const SUB_HEDR: FourCC = FourCC(*b"HEDR");
pub const SUB_CNAM: FourCC = FourCC(*b"CNAM");
pub const SUB_SNAM: FourCC = FourCC(*b"SNAM");
pub const SUB_MAST: FourCC = FourCC(*b"MAST");
pub const SUB_DATA: FourCC = FourCC(*b"DATA");
pub const SUB_OBND: FourCC = FourCC(*b"OBND");
pub const SUB_MODL: FourCC = FourCC(*b"MODL");
pub const SUB_MODB: FourCC = FourCC(*b"MODB");
pub const SUB_MODT: FourCC = FourCC(*b"MODT");
pub const SUB_FULL: FourCC = FourCC(*b"FULL");
pub const SUB_NAME: FourCC = FourCC(*b"NAME");
pub const SUB_XSCL: FourCC = FourCC(*b"XSCL");
pub const SUB_XCLC: FourCC = FourCC(*b"XCLC");
pub const SUB_XCLL: FourCC = FourCC(*b"XCLL");
pub const SUB_XXXX: FourCC = FourCC(*b"XXXX");

/// 32-bit オブジェクト一意識別子 (FormID)。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FormId(pub u32);

impl fmt::Debug for FormId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FormId(0x{:08X})", self.0)
    }
}

impl fmt::Display for FormId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:08X}", self.0)
    }
}

impl From<u32> for FormId {
    fn from(val: u32) -> Self {
        FormId(val)
    }
}

/// オブジェクト境界ボックス (`OBND` サブレコード)。
/// 各軸の最小・最大整数座標を保持。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ObjectBounds {
    pub min: [i16; 3],
    pub max: [i16; 3],
}
