//! # TES4 レコード (ESM ファイルヘッダー)
//!
//! ESM / ESP ファイルの先頭に必ず存在するメタデータレコード。
//! 参照元: `references/openmw/components/esm4/loadtes4.hpp`

use std::io::{self, Cursor};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::subrecord::Subrecord;
use crate::types::{SUB_CNAM, SUB_HEDR, SUB_MAST, SUB_SNAM};

/// TES4 ファイルヘッダー情報。
#[derive(Clone, Debug, PartialEq)]
pub struct Tes4Header {
    /// ファイルフォーマットバージョン (Fallout 3 では通常 0.94)
    pub version: f32,
    /// レコード総数
    pub num_records: i32,
    /// 次に割り当てられるオブジェクトID
    pub next_object_id: u32,
    /// 作者名 (CNAM)
    pub author: String,
    /// ファイル説明 (SNAM)
    pub description: String,
    /// 依存マスターファイル一覧 (MAST)
    pub masters: Vec<String>,
}

impl Tes4Header {
    pub fn from_subrecords(subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut version = 0.0f32;
        let mut num_records = 0i32;
        let mut next_object_id = 0u32;
        let mut author = String::new();
        let mut description = String::new();
        let mut masters = Vec::new();

        for sub in subrecords {
            match sub.type_id {
                SUB_HEDR => {
                    if sub.data.len() >= 12 {
                        let mut cursor = Cursor::new(&sub.data);
                        version = cursor.read_f32::<LittleEndian>()?;
                        num_records = cursor.read_i32::<LittleEndian>()?;
                        next_object_id = cursor.read_u32::<LittleEndian>()?;
                    }
                }
                SUB_CNAM => {
                    author = sub.as_string();
                }
                SUB_SNAM => {
                    description = sub.as_string();
                }
                SUB_MAST => {
                    masters.push(sub.as_string());
                }
                _ => {}
            }
        }

        Ok(Tes4Header {
            version,
            num_records,
            next_object_id,
            author,
            description,
            masters,
        })
    }
}
