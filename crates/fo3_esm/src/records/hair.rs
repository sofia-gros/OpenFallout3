//! # HAIR レコード (髪型定義)
//!
//! 髪型や髭のメッシュ定義。
//! 参照元: `references/openmw/components/esm4/loadhair.hpp`, `loadhair.cpp`

use std::io;
use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, SUB_DATA, SUB_EDID, SUB_FULL, SUB_MODL};

/// 髪型定義レコード (HAIR)。
#[derive(Clone, Debug, PartialEq)]
pub struct HairRecord {
    pub form_id: FormId,
    /// エディタ用識別名 (EDID)
    pub edid: String,
    /// 表示名 (FULL)
    pub full_name: Option<String>,
    /// 髪メッシュファイルパス (MODL)
    pub model: String,
    /// フラグ (0x01: Playable でない, 0x02: 男性非対応, 0x04: 女性非対応)
    pub flags: u8,
}

impl HairRecord {
    pub fn from_record(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut edid = String::new();
        let mut full_name = None;
        let mut model = String::new();
        let mut flags = 0u8;

        for sub in subrecords {
            match sub.type_id {
                SUB_EDID => {
                    edid = sub.as_string();
                }
                SUB_FULL => {
                    full_name = Some(sub.as_string());
                }
                SUB_MODL => {
                    model = sub.as_string();
                }
                SUB_DATA => {
                    if !sub.data.is_empty() {
                        flags = sub.data[0];
                    }
                }
                _ => {}
            }
        }

        Ok(HairRecord {
            form_id: header.form_id,
            edid,
            full_name,
            model,
            flags,
        })
    }
}
