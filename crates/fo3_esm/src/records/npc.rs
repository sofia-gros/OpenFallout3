//! # NPC_ レコード (ノンプレイヤーキャラクター定義)
//!
//! 人間やグールなど NPC のベースアクター定義。
//! 参照元: `references/openmw/components/esm4/loadnpc.hpp`, `loadnpc.cpp`

use std::io;
use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, FourCC, ObjectBounds, SUB_EDID, SUB_FULL, SUB_OBND};

pub const SUB_ACBS: FourCC = FourCC(*b"ACBS");
pub const SUB_RNAM: FourCC = FourCC(*b"RNAM");
pub const SUB_WNAM: FourCC = FourCC(*b"WNAM");

/// NPC 定義レコード (NPC_)。
#[derive(Clone, Debug, PartialEq)]
pub struct NpcRecord {
    pub form_id: FormId,
    /// エディタ用識別名 (EDID)
    pub edid: String,
    /// 表示名 (FULL)
    pub full_name: Option<String>,
    /// 境界ボックス (OBND)
    pub bounds: Option<ObjectBounds>,
    /// 女性フラグ (ACBS flags の bit 0)
    pub is_female: bool,
    /// 種族 FormID (RNAM)
    pub race: FormId,
    /// デフォルト装備防具 FormID (WNAM)
    pub default_armor: Option<FormId>,
}

impl NpcRecord {
    pub fn from_record(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut edid = String::new();
        let mut full_name = None;
        let mut bounds = None;
        let mut is_female = false;
        let mut race = FormId(0);
        let mut default_armor = None;

        for sub in subrecords {
            match sub.type_id {
                SUB_EDID => {
                    edid = sub.as_string();
                }
                SUB_FULL => {
                    full_name = Some(sub.as_string());
                }
                SUB_OBND => {
                    if let Ok(b) = sub.as_bounds() {
                        bounds = Some(b);
                    }
                }
                SUB_ACBS => {
                    // Fallout 3: 基本設定フラグ (u32) が先頭 4 バイト
                    if sub.data.len() >= 4 {
                        let flags = u32::from_le_bytes([
                            sub.data[0],
                            sub.data[1],
                            sub.data[2],
                            sub.data[3],
                        ]);
                        // bit 0: Female
                        is_female = (flags & 0x0001) != 0;
                    }
                }
                SUB_RNAM => {
                    if let Ok(id) = sub.as_form_id() {
                        race = id;
                    }
                }
                SUB_WNAM => {
                    if let Ok(id) = sub.as_form_id() {
                        default_armor = Some(id);
                    }
                }
                _ => {}
            }
        }

        Ok(NpcRecord {
            form_id: header.form_id,
            edid,
            full_name,
            bounds,
            is_female,
            race,
            default_armor,
        })
    }
}
