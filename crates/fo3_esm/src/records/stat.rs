//! # STAT レコード (静的オブジェクト定義)
//!
//! 建物、家具、小道具、岩などワールド上に配置される静的メッシュの定義。
//! 参照元: `references/openmw/components/esm4/loadstat.hpp`

use std::io;
use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, ObjectBounds, SUB_EDID, SUB_MODB, SUB_MODL, SUB_OBND};

/// 静的配置可能オブジェクト定義 (STAT)。
#[derive(Clone, Debug, PartialEq)]
pub struct StatRecord {
    pub form_id: FormId,
    /// エディタ用識別名 (EDID)
    pub edid: String,
    /// オブジェクト境界ボックス (OBND)
    pub bounds: Option<ObjectBounds>,
    /// NIF モデルファイル相対パス (MODL)
    pub model: String,
    /// モデルバウンディング半径 (MODB)
    pub model_bound: Option<f32>,
}

impl StatRecord {
    pub fn from_record(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut edid = String::new();
        let mut bounds = None;
        let mut model = String::new();
        let mut model_bound = None;

        for sub in subrecords {
            match sub.type_id {
                SUB_EDID => {
                    edid = sub.as_string();
                }
                SUB_OBND => {
                    if let Ok(b) = sub.as_bounds() {
                        bounds = Some(b);
                    }
                }
                SUB_MODL => {
                    model = sub.as_string();
                }
                SUB_MODB => {
                    if let Ok(b) = sub.as_f32() {
                        model_bound = Some(b);
                    }
                }
                _ => {}
            }
        }

        Ok(StatRecord {
            form_id: header.form_id,
            edid,
            bounds,
            model,
            model_bound,
        })
    }
}
