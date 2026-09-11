//! # OTFT レコード (衣装・アウトフィット定義)
//!
//! NPC が着用する衣装セットの定義。インベントリ (INAM) に含まれる防具等の FormID リストを持つ。
//! 参照元: `references/openmw/components/esm4/loadotft.hpp`, `loadotft.cpp`

use std::io;
use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, SUB_EDID, SUB_INAM};

/// 衣装定義レコード (OTFT)。
#[derive(Clone, Debug, PartialEq)]
pub struct OtftRecord {
    pub form_id: FormId,
    /// エディタ用識別名 (EDID)
    pub edid: String,
    /// 衣装に含まれる防具等の FormID リスト (INAM)
    pub inventory: Vec<FormId>,
}

impl OtftRecord {
    pub fn from_record(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut edid = String::new();
        let mut inventory = Vec::new();

        for sub in subrecords {
            match sub.type_id {
                SUB_EDID => {
                    edid = sub.as_string();
                }
                SUB_INAM => {
                    // INAM は 4 バイトごとの FormID リスト
                    let count = sub.data.len() / 4;
                    for i in 0..count {
                        let offset = i * 4;
                        let fid = u32::from_le_bytes([
                            sub.data[offset],
                            sub.data[offset + 1],
                            sub.data[offset + 2],
                            sub.data[offset + 3],
                        ]);
                        inventory.push(FormId(fid));
                    }
                }
                _ => {}
            }
        }

        Ok(OtftRecord {
            form_id: header.form_id,
            edid,
            inventory,
        })
    }
}
