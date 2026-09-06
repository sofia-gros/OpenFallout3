//! # TXST (Texture Set) レコードパーサー
//!
//! テクスチャセット定義レコード。
//! ディフューズテクスチャ (TX00)、法線マップ (TX01) 等の画像ファイルパスを保持。
//! 参照元: `references/openmw/components/esm4/loadtxst.hpp`, `loadtxst.cpp`

use std::io;
use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, SUB_EDID, SUB_TX00, SUB_TX01};

/// テクスチャセットレコード (`TXST`)。
#[derive(Clone, Debug, PartialEq)]
pub struct TextureSetRecord {
    /// 一意の FormID
    pub form_id: FormId,
    /// エディタ ID (`EDID`)
    pub edid: String,
    /// ディフューズテクスチャ画像パス (`TX00`)
    pub diffuse: String,
    /// 法線マップ画像パス (`TX01`)
    pub normal_map: String,
}

impl TextureSetRecord {
    /// レコードヘッダーとサブレコード群から TextureSetRecord を構築する。
    pub fn read(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut edid = String::new();
        let mut diffuse = String::new();
        let mut normal_map = String::new();

        for sub in subrecords {
            match sub.type_id {
                SUB_EDID => {
                    edid = sub.as_string();
                }
                SUB_TX00 => {
                    diffuse = sub.as_string();
                }
                SUB_TX01 => {
                    normal_map = sub.as_string();
                }
                _ => {}
            }
        }

        Ok(Self {
            form_id: header.form_id,
            edid,
            diffuse,
            normal_map,
        })
    }
}
