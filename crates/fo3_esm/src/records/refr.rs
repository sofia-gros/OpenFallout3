//! # REFR レコード (オブジェクト配置参照)
//!
//! セル空間内に配置された静的・動的オブジェクトの実体参照レコード。
//! 参照元: `references/openmw/components/esm4/loadrefr.hpp`, `loadrefr.cpp`

use std::io::{self, Cursor};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{
    FormId, FourCC, SUB_DATA, SUB_EDID, SUB_NAME, SUB_TNAM, SUB_XCNT, SUB_XESP,
    SUB_XLOC, SUB_XMRK, SUB_XOWN, SUB_XRNK, SUB_XSCL, SUB_XTEL,
};

/// ドアのテレポート先遷移情報 (`XTEL` サブレコード)。
///
/// 参照元: `references/openmw/components/esm4/loadrefr.hpp:60-66`, `loadrefr.cpp:103-127`
#[derive(Clone, Debug, PartialEq)]
pub struct TeleportDoor {
    /// 遷移先となる相手側ドアの配置参照 FormID
    pub dest_door: FormId,
    /// 出現位置 [X, Y, Z]
    pub dest_pos: [f32; 3],
    /// 出現姿勢回転 [RotX, RotY, RotZ] (ラジアン単位)
    pub dest_rot: [f32; 3],
    /// ドアフラグ (FO3 では通常 0、TES5 では 0x01: No Alarm 等)
    pub flags: u32,
}

/// ドアやコンテナの施錠設定 (`XLOC` サブレコード)。
///
/// 参照元: `references/openmw/components/esm4/loadrefr.cpp:248-270`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LockData {
    /// ロック難易度レベル
    /// (0: Very Easy, 25: Easy, 50: Average, 75: Hard, 100: Very Hard, 255: 要キー/施錠解除不可)
    pub lock_level: u8,
    /// 解錠に必要な鍵アイテムの FormID (存在する場合)
    pub key: Option<FormId>,
    /// 施錠フラグ
    pub flags: u32,
}

/// 親オブジェクトとの連動有効化設定 (`XESP` サブレコード)。
///
/// 参照元: `references/openmw/components/esm4/reference.hpp:39-49`, `loadrefr.cpp:95-101`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnableParent {
    /// 連動親となる配置参照の FormID
    pub parent: FormId,
    /// 連動フラグ (0x01: Inversed = 逆連動, 0x02: PopIn)
    pub flags: u32,
}

/// 配置参照レコード (`REFR`, `ACHR`, `ACRE`)。
#[derive(Clone, Debug, PartialEq)]
pub struct RefrRecord {
    /// レコード種別 (`REC_REFR`, `REC_ACHR`, `REC_ACRE`)
    pub record_type: FourCC,
    /// この配置インスタンスの一意 FormID
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
    /// ドアのテレポート先情報 (XTEL, 存在する場合)
    pub teleport: Option<TeleportDoor>,
    /// 施錠情報 (XLOC, 存在する場合)
    pub lock: Option<LockData>,
    /// 連動親設定 (XESP, 存在する場合)
    pub enable_parent: Option<EnableParent>,
    /// マップマーカーフラグ (XMRK)
    pub is_map_marker: bool,
    /// マップマーカー種別 (TNAM)
    pub map_marker_type: Option<u16>,
    /// 所有者 NPC / Faction (XOWN)
    pub owner: Option<FormId>,
    /// 所有ファクションランク (XRNK)
    pub faction_rank: Option<i32>,
    /// 配置スタック個数 (XCNT, アイテム配置時。省略時は 1)
    pub count: i32,
}

impl RefrRecord {
    /// サブレコード列から RefrRecord を生成する。
    pub fn from_record(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut edid = String::new();
        let mut base_object = FormId(0);
        let mut position = [0.0f32; 3];
        let mut rotation = [0.0f32; 3];
        let mut scale = 1.0f32;
        let mut teleport = None;
        let mut lock = None;
        let mut enable_parent = None;
        let mut is_map_marker = false;
        let mut map_marker_type = None;
        let mut owner = None;
        let mut faction_rank = None;
        let mut count = 1;

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
                SUB_XTEL => {
                    // 参照元: references/openmw/components/esm4/loadrefr.cpp:103-127
                    if sub.data.len() >= 28 {
                        let mut cursor = Cursor::new(&sub.data);
                        if let Ok(dest_id) = cursor.read_u32::<LittleEndian>() {
                            let px = cursor.read_f32::<LittleEndian>().unwrap_or(0.0);
                            let py = cursor.read_f32::<LittleEndian>().unwrap_or(0.0);
                            let pz = cursor.read_f32::<LittleEndian>().unwrap_or(0.0);
                            let rx = cursor.read_f32::<LittleEndian>().unwrap_or(0.0);
                            let ry = cursor.read_f32::<LittleEndian>().unwrap_or(0.0);
                            let rz = cursor.read_f32::<LittleEndian>().unwrap_or(0.0);
                            let flags = if sub.data.len() >= 32 {
                                cursor.read_u32::<LittleEndian>().unwrap_or(0)
                            } else {
                                0
                            };
                            teleport = Some(TeleportDoor {
                                dest_door: FormId(dest_id),
                                dest_pos: [px, py, pz],
                                dest_rot: [rx, ry, rz],
                                flags,
                            });
                        }
                    }
                }
                SUB_XLOC => {
                    // 参照元: references/openmw/components/esm4/loadrefr.cpp:248-270
                    if sub.data.len() >= 8 {
                        let lock_level = sub.data[0];
                        let key_id = u32::from_le_bytes([sub.data[4], sub.data[5], sub.data[6], sub.data[7]]);
                        let key = if key_id != 0 { Some(FormId(key_id)) } else { None };
                        let flags = if sub.data.len() >= 12 {
                            u32::from_le_bytes([sub.data[8], sub.data[9], sub.data[10], sub.data[11]])
                        } else {
                            0
                        };
                        lock = Some(LockData {
                            lock_level,
                            key,
                            flags,
                        });
                    }
                }
                SUB_XESP => {
                    // 参照元: references/openmw/components/esm4/reference.hpp:39-49
                    if sub.data.len() >= 8 {
                        let parent_id = u32::from_le_bytes([sub.data[0], sub.data[1], sub.data[2], sub.data[3]]);
                        let flags = u32::from_le_bytes([sub.data[4], sub.data[5], sub.data[6], sub.data[7]]);
                        enable_parent = Some(EnableParent {
                            parent: FormId(parent_id),
                            flags,
                        });
                    }
                }
                SUB_XMRK => {
                    is_map_marker = true;
                }
                SUB_TNAM => {
                    if let Ok(t) = sub.as_u16() {
                        map_marker_type = Some(t);
                    }
                }
                SUB_XOWN => {
                    if let Ok(id) = sub.as_form_id() {
                        owner = Some(id);
                    }
                }
                SUB_XRNK => {
                    if let Ok(r) = sub.as_i32() {
                        faction_rank = Some(r);
                    }
                }
                SUB_XCNT => {
                    if let Ok(c) = sub.as_i32() {
                        count = c;
                    }
                }
                _ => {}
            }
        }

        Ok(RefrRecord {
            record_type: header.type_id,
            form_id: header.form_id,
            edid,
            base_object,
            position,
            rotation,
            scale,
            teleport,
            lock,
            enable_parent,
            is_map_marker,
            map_marker_type,
            owner,
            faction_rank,
            count,
        })
    }
}
