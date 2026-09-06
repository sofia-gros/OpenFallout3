//! # REFR レコード (オブジェクト配置参照)
//!
//! セル空間内に配置された静的・動的オブジェクトの実体参照レコード。
//! 参照元: `references/openmw/components/esm4/loadrefr.hpp`, `loadrefr.cpp`

use std::io;
use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, SUB_DATA, SUB_EDID, SUB_NAME, SUB_XSCL};

/// 配置参照レコード (`REFR`)。
#[derive(Clone, Debug, PartialEq)]
pub struct RefrRecord {
    pub form_id: FormId,
    /// エディタ用識別名 (EDID, 存在する場合)
    pub edid: String,
    /// 配置対象となるベースオブジェクトの FormID (NAME)
    pub base_object: FormId,
    /// 配置位置 [X, Y, Z] (DATA)
    pub position: [f32; 3],
    /// オイラー角姿勢回転 [RotX, RotY, RotZ] (DATA, ラジアン単位)
    /// 参照元: `references/nifskope/src/gl/glcontroller.cpp:489` (合成順: Rz * Ry * Rx)
    pub rotation: [f32; 3],
    /// 配置スケール倍率 (XSCL, 省略時は 1.0)
    pub scale: f32,
}

impl RefrRecord {
    /// サブレコード列から RefrRecord を生成する。
    pub fn from_record(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut edid = String::new();
        let mut base_object = FormId(0);
        let mut position = [0.0f32; 3];
        let mut rotation = [0.0f32; 3];
        let mut scale = 1.0f32;

        for sub in subrecords {
            match sub.type_id {
                SUB_EDID => {
                    edid = sub.as_string();
                }
                SUB_NAME => {
                    if let Ok(id) = sub.as_form_id() {
                        base_object = id;
                    }
                }
                SUB_DATA => {
                    if let Ok((pos, rot)) = sub.as_pos_rot() {
                        position = pos;
                        rotation = rot;
                    }
                }
                SUB_XSCL => {
                    if let Ok(s) = sub.as_f32() {
                        scale = s;
                    }
                }
                _ => {}
            }
        }

        Ok(RefrRecord {
            form_id: header.form_id,
            edid,
            base_object,
            position,
            rotation,
            scale,
        })
    }
}
