//! # ESM / ESP ヘッダー定義
//!
//! Fallout 3 (24 バイトヘッダー) に完全準拠した
//! レコードヘッダー、グループヘッダー、サブレコードヘッダーの定義。
//! 参照元: `references/openmw/components/esm4/reader.hpp:L62-97`

use std::io::{self, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::types::{FormId, FourCC};

/// レコードヘッダー (24 バイト)。
///
/// 参照元: `references/openmw/components/esm4/reader.hpp:L74-85`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordHeader {
    /// レコード種別シグネチャ (例: `TES4`, `STAT`, `CELL`, `WRLD`, etc.)
    pub type_id: FourCC,
    /// データ本体サイズ（24 バイトのヘッダーを含まない）
    pub data_size: u32,
    /// レコードフラグ
    pub flags: u32,
    /// 32-bit オブジェクト ID
    pub form_id: FormId,
    /// バージョン管理情報 1
    pub vc_info: u32,
    /// フォームバージョン (Fallout 3 では通常 15 = 0x000F)
    pub form_version: u16,
    /// バージョン管理情報 2
    pub vc_info2: u16,
}

impl RecordHeader {
    pub const SIZE: usize = 24;

    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut type_bytes = [0u8; 4];
        reader.read_exact(&mut type_bytes)?;
        let type_id = FourCC(type_bytes);

        let data_size = reader.read_u32::<LittleEndian>()?;
        let flags = reader.read_u32::<LittleEndian>()?;
        let form_id = FormId(reader.read_u32::<LittleEndian>()?);
        let vc_info = reader.read_u32::<LittleEndian>()?;
        let form_version = reader.read_u16::<LittleEndian>()?;
        let vc_info2 = reader.read_u16::<LittleEndian>()?;

        Ok(RecordHeader {
            type_id,
            data_size,
            flags,
            form_id,
            vc_info,
            form_version,
            vc_info2,
        })
    }

    /// データが zlib 圧縮されているか判定。
    pub fn is_compressed(&self) -> bool {
        (self.flags & 0x00040000) != 0
    }

    /// マスターファイル (ESM) レコードか判定。
    pub fn is_esm(&self) -> bool {
        (self.flags & 0x00000001) != 0
    }
}

/// グループヘッダー (24 バイト)。
///
/// 参照元: `references/openmw/components/esm4/reader.hpp:L62-72`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupHeader {
    /// 常に `GRUP`
    pub type_id: FourCC,
    /// ヘッダーの 24 バイトを含むグループ全体のサイズ
    pub group_size: u32,
    /// ラベル (Top グループの場合はレコードシグネチャ)
    pub label: [u8; 4],
    /// グループ種別 (0: Top, 1: WorldChildren, 2: InteriorCellBlock, etc.)
    pub group_type: i32,
    /// タイムスタンプ
    pub stamp: u16,
    pub unknown1: u16,
    pub version: u16,
    pub unknown2: u16,
}

impl GroupHeader {
    pub const SIZE: usize = 24;

    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut type_bytes = [0u8; 4];
        reader.read_exact(&mut type_bytes)?;
        let type_id = FourCC(type_bytes);

        let group_size = reader.read_u32::<LittleEndian>()?;
        let mut label = [0u8; 4];
        reader.read_exact(&mut label)?;
        let group_type = reader.read_i32::<LittleEndian>()?;
        let stamp = reader.read_u16::<LittleEndian>()?;
        let unknown1 = reader.read_u16::<LittleEndian>()?;
        let version = reader.read_u16::<LittleEndian>()?;
        let unknown2 = reader.read_u16::<LittleEndian>()?;

        Ok(GroupHeader {
            type_id,
            group_size,
            label,
            group_type,
            stamp,
            unknown1,
            version,
            unknown2,
        })
    }

    /// Top レベルグループ (group_type == 0) の場合、対象レコード型 (FourCC) を返す。
    pub fn target_record_type(&self) -> Option<FourCC> {
        if self.group_type == 0 {
            Some(FourCC(self.label))
        } else {
            None
        }
    }
}

/// サブレコードヘッダー (6 バイト)。
///
/// 参照元: `references/openmw/components/esm4/reader.hpp:L93-97`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubrecordHeader {
    pub type_id: FourCC,
    pub data_size: u16,
}

impl SubrecordHeader {
    pub const SIZE: usize = 6;

    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut type_bytes = [0u8; 4];
        reader.read_exact(&mut type_bytes)?;
        let type_id = FourCC(type_bytes);
        let data_size = reader.read_u16::<LittleEndian>()?;

        Ok(SubrecordHeader { type_id, data_size })
    }
}
