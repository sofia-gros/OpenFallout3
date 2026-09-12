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
pub const REC_SCOL: FourCC = FourCC(*b"SCOL");
pub const REC_DOOR: FourCC = FourCC(*b"DOOR");
pub const REC_ACTI: FourCC = FourCC(*b"ACTI");
pub const REC_FURN: FourCC = FourCC(*b"FURN");
pub const REC_CONT: FourCC = FourCC(*b"CONT");
pub const REC_MSTT: FourCC = FourCC(*b"MSTT");
pub const REC_TERM: FourCC = FourCC(*b"TERM");
pub const REC_LIGH: FourCC = FourCC(*b"LIGH");
pub const REC_MISC: FourCC = FourCC(*b"MISC");
pub const REC_BOOK: FourCC = FourCC(*b"BOOK");
pub const REC_ALCH: FourCC = FourCC(*b"ALCH");
pub const REC_KEYM: FourCC = FourCC(*b"KEYM");
pub const REC_WEAP: FourCC = FourCC(*b"WEAP");
pub const REC_AMMO: FourCC = FourCC(*b"AMMO");
pub const REC_ARMO: FourCC = FourCC(*b"ARMO");
pub const REC_NPC_: FourCC = FourCC(*b"NPC_");
pub const REC_CREA: FourCC = FourCC(*b"CREA");
pub const REC_CELL: FourCC = FourCC(*b"CELL");
pub const REC_WRLD: FourCC = FourCC(*b"WRLD");
pub const REC_REFR: FourCC = FourCC(*b"REFR");
pub const REC_ACHR: FourCC = FourCC(*b"ACHR");
pub const REC_ACRE: FourCC = FourCC(*b"ACRE");
pub const REC_LAND: FourCC = FourCC(*b"LAND");
pub const REC_NAVM: FourCC = FourCC(*b"NAVM");
pub const REC_LGTM: FourCC = FourCC(*b"LGTM");
pub const REC_LTEX: FourCC = FourCC(*b"LTEX");
pub const REC_TXST: FourCC = FourCC(*b"TXST");
pub const REC_OTFT: FourCC = FourCC(*b"OTFT");
pub const REC_HAIR: FourCC = FourCC(*b"HAIR");
pub const REC_LVLI: FourCC = FourCC(*b"LVLI");
pub const REC_DIAL: FourCC = FourCC(*b"DIAL");
pub const REC_INFO: FourCC = FourCC(*b"INFO");

pub const SUB_EDID: FourCC = FourCC(*b"EDID");
pub const SUB_LVLO: FourCC = FourCC(*b"LVLO");
pub const SUB_LVLD: FourCC = FourCC(*b"LVLD");
pub const SUB_LVLF: FourCC = FourCC(*b"LVLF");
pub const SUB_HEDR: FourCC = FourCC(*b"HEDR");
pub const SUB_CNAM: FourCC = FourCC(*b"CNAM");
pub const SUB_SNAM: FourCC = FourCC(*b"SNAM");
pub const SUB_INAM: FourCC = FourCC(*b"INAM");
pub const SUB_DOFT: FourCC = FourCC(*b"DOFT");
pub const SUB_CNTO: FourCC = FourCC(*b"CNTO");
pub const SUB_MAST: FourCC = FourCC(*b"MAST");
pub const SUB_DATA: FourCC = FourCC(*b"DATA");
pub const SUB_HNAM: FourCC = FourCC(*b"HNAM");
pub const SUB_ICON: FourCC = FourCC(*b"ICON");
pub const SUB_GNAM: FourCC = FourCC(*b"GNAM");
pub const SUB_TNAM: FourCC = FourCC(*b"TNAM");
pub const SUB_TX00: FourCC = FourCC(*b"TX00");
pub const SUB_TX01: FourCC = FourCC(*b"TX01");
pub const SUB_OBND: FourCC = FourCC(*b"OBND");
pub const SUB_MODL: FourCC = FourCC(*b"MODL");
pub const SUB_MODB: FourCC = FourCC(*b"MODB");
pub const SUB_MODT: FourCC = FourCC(*b"MODT");
pub const SUB_FULL: FourCC = FourCC(*b"FULL");
pub const SUB_NAME: FourCC = FourCC(*b"NAME");
pub const SUB_XSCL: FourCC = FourCC(*b"XSCL");
pub const SUB_XCLC: FourCC = FourCC(*b"XCLC");
pub const SUB_XCLL: FourCC = FourCC(*b"XCLL");
pub const SUB_LTMP: FourCC = FourCC(*b"LTMP");
pub const SUB_LNAM: FourCC = FourCC(*b"LNAM");
pub const SUB_XXXX: FourCC = FourCC(*b"XXXX");
pub const SUB_VHGT: FourCC = FourCC(*b"VHGT");
pub const SUB_VNML: FourCC = FourCC(*b"VNML");
pub const SUB_VCLR: FourCC = FourCC(*b"VCLR");
pub const SUB_BTXT: FourCC = FourCC(*b"BTXT");
pub const SUB_ATXT: FourCC = FourCC(*b"ATXT");
pub const SUB_VTXT: FourCC = FourCC(*b"VTXT");
pub const SUB_WNAM: FourCC = FourCC(*b"WNAM");
pub const SUB_ENAM: FourCC = FourCC(*b"ENAM");
pub const SUB_HCLR: FourCC = FourCC(*b"HCLR");
pub const SUB_NAM2: FourCC = FourCC(*b"NAM2");
pub const SUB_XTEL: FourCC = FourCC(*b"XTEL");
pub const SUB_XLOC: FourCC = FourCC(*b"XLOC");
pub const SUB_XESP: FourCC = FourCC(*b"XESP");
pub const SUB_XMRK: FourCC = FourCC(*b"XMRK");
pub const SUB_XOWN: FourCC = FourCC(*b"XOWN");
pub const SUB_XRNK: FourCC = FourCC(*b"XRNK");
pub const SUB_XCNT: FourCC = FourCC(*b"XCNT");
pub const SUB_XEMI: FourCC = FourCC(*b"XEMI");
pub const SUB_XLIG: FourCC = FourCC(*b"XLIG");
pub const SUB_XRDS: FourCC = FourCC(*b"XRDS");
pub const SUB_FGGS: FourCC = FourCC(*b"FGGS");
pub const SUB_FGGA: FourCC = FourCC(*b"FGGA");
pub const SUB_FGTS: FourCC = FourCC(*b"FGTS");

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
