//! # ARMO レコード (防具・衣装定義)
//!
//! NPC やプレイヤーが装備する防具・服・ボディパーツの定義。
//! 参照元: `references/openmw/components/esm4/loadarmo.hpp`, `loadarmo.cpp`

use std::io;
use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, FourCC, ObjectBounds, SUB_EDID, SUB_FULL, SUB_MODL, SUB_OBND};

pub const SUB_MOD2: FourCC = FourCC(*b"MOD2");
pub const SUB_MOD3: FourCC = FourCC(*b"MOD3");
pub const SUB_MOD4: FourCC = FourCC(*b"MOD4");

/// 防具・衣装定義レコード (ARMO)。
#[derive(Clone, Debug, PartialEq)]
pub struct ArmorRecord {
    pub form_id: FormId,
    /// エディタ用識別名 (EDID)
    pub edid: String,
    /// 表示名 (FULL)
    pub full_name: Option<String>,
    /// 境界ボックス (OBND)
    pub bounds: Option<ObjectBounds>,
    /// 男性ボディ着用モデルパス (MODL)
    pub male_model: String,
    /// 男性ワールドドロップモデルパス (MOD2)
    pub male_world_model: String,
    /// 女性ボディ着用モデルパス (MOD3)
    pub female_model: String,
    /// 女性ワールドドロップモデルパス (MOD4)
    pub female_world_model: String,
}

impl ArmorRecord {
    pub fn from_record(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut edid = String::new();
        let mut full_name = None;
        let mut bounds = None;
        let mut male_model = String::new();
        let mut male_world_model = String::new();
        let mut female_model = String::new();
        let mut female_world_model = String::new();

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
                SUB_MODL => {
                    male_model = sub.as_string();
                }
                SUB_MOD2 => {
                    male_world_model = sub.as_string();
                }
                SUB_MOD3 => {
                    female_model = sub.as_string();
                }
                SUB_MOD4 => {
                    female_world_model = sub.as_string();
                }
                _ => {}
            }
        }

        Ok(ArmorRecord {
            form_id: header.form_id,
            edid,
            full_name,
            bounds,
            male_model,
            male_world_model,
            female_model,
            female_world_model,
        })
    }
}
