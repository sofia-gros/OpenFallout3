//! # Fallout 3 オブジェクト・アイテム系レコード定義モジュール
//!
//! DOOR, ACTI, CONT, FURN, WEAP, AMMO, MISC, KEYM, ALCH, BOOK, NOTE, GMST 等の主要レコード。
//!
//! 参照元:
//! - `references/openmw/components/esm4/loaddoor.hpp`, `loaddoor.cpp`
//! - `references/openmw/components/esm4/loadacti.hpp`, `loadacti.cpp`
//! - `references/openmw/components/esm4/loadcont.hpp`, `loadcont.cpp`
//! - `references/openmw/components/esm4/loadfurn.hpp`, `loadfurn.cpp`
//! - `references/openmw/components/esm4/loadweap.hpp`, `loadweap.cpp`
//! - `references/openmw/components/esm4/loadammo.hpp`, `loadammo.cpp`
//! - `references/openmw/components/esm4/loadmisc.hpp`, `loadmisc.cpp`
//! - `references/openmw/components/esm4/loadkeym.hpp`, `loadkeym.cpp`
//! - `references/openmw/components/esm4/loadalch.hpp`, `loadalch.cpp`
//! - `references/openmw/components/esm4/loadbook.hpp`, `loadbook.cpp`
//! - `references/openmw/components/esm4/loadnote.hpp`, `loadnote.cpp`
//! - `references/openmw/components/esm4/loadgmst.hpp`, `loadgmst.cpp`

use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::FormId;
use std::io;

/// ドア定義レコード (`DOOR`)。
///
/// 参照元: `references/openmw/components/esm4/loaddoor.hpp:42-60`
#[derive(Clone, Debug, PartialEq)]
pub struct DoorRecord {
    pub form_id: FormId,
    pub edid: String,
    pub full_name: Option<String>,
    pub model: String,
    pub open_sound: Option<FormId>,
    pub close_sound: Option<FormId>,
    pub loop_sound: Option<FormId>,
    pub flags: u8,
    pub unknown_subrecords: Vec<Subrecord>,
}

impl DoorRecord {
    pub fn parse(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut edid = String::new();
        let mut full_name = None;
        let mut model = String::new();
        let mut open_sound = None;
        let mut close_sound = None;
        let mut loop_sound = None;
        let mut flags = 0;
        let mut unknown_subrecords = Vec::new();

        for sub in subrecords {
            match &sub.type_id.0 {
                b"EDID" => edid = sub.as_string(),
                b"FULL" => full_name = Some(sub.as_string()),
                b"MODL" => model = sub.as_string(),
                b"SNAM" => {
                    if sub.data.len() >= 4 {
                        open_sound = Some(FormId(u32::from_le_bytes(sub.data[..4].try_into().unwrap())));
                    }
                }
                b"ANAM" => {
                    if sub.data.len() >= 4 {
                        close_sound = Some(FormId(u32::from_le_bytes(sub.data[..4].try_into().unwrap())));
                    }
                }
                b"BNAM" => {
                    if sub.data.len() >= 4 {
                        loop_sound = Some(FormId(u32::from_le_bytes(sub.data[..4].try_into().unwrap())));
                    }
                }
                b"FNAM" => {
                    if !sub.data.is_empty() {
                        flags = sub.data[0];
                    }
                }
                _ => unknown_subrecords.push(sub.clone()),
            }
        }

        Ok(Self {
            form_id: header.form_id,
            edid,
            full_name,
            model,
            open_sound,
            close_sound,
            loop_sound,
            flags,
            unknown_subrecords,
        })
    }
}

/// アクティベーター定義レコード (`ACTI`)。
///
/// 参照元: `references/openmw/components/esm4/loadacti.hpp:42-60`
#[derive(Clone, Debug, PartialEq)]
pub struct ActiRecord {
    pub form_id: FormId,
    pub edid: String,
    pub full_name: Option<String>,
    pub model: String,
    pub sound: Option<FormId>,
    pub loop_sound: Option<FormId>,
    pub activate_sound: Option<FormId>,
    pub script: Option<FormId>,
    pub unknown_subrecords: Vec<Subrecord>,
}

impl ActiRecord {
    pub fn parse(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut edid = String::new();
        let mut full_name = None;
        let mut model = String::new();
        let mut sound = None;
        let mut loop_sound = None;
        let mut activate_sound = None;
        let mut script = None;
        let mut unknown_subrecords = Vec::new();

        for sub in subrecords {
            match &sub.type_id.0 {
                b"EDID" => edid = sub.as_string(),
                b"FULL" => full_name = Some(sub.as_string()),
                b"MODL" => model = sub.as_string(),
                b"SNAM" => {
                    if sub.data.len() >= 4 {
                        sound = Some(FormId(u32::from_le_bytes(sub.data[..4].try_into().unwrap())));
                    }
                }
                b"VNAM" => {
                    if sub.data.len() >= 4 {
                        activate_sound = Some(FormId(u32::from_le_bytes(sub.data[..4].try_into().unwrap())));
                    }
                }
                b"RNAM" => {
                    if sub.data.len() >= 4 {
                        loop_sound = Some(FormId(u32::from_le_bytes(sub.data[..4].try_into().unwrap())));
                    }
                }
                b"SCRI" => {
                    if sub.data.len() >= 4 {
                        script = Some(FormId(u32::from_le_bytes(sub.data[..4].try_into().unwrap())));
                    }
                }
                _ => unknown_subrecords.push(sub.clone()),
            }
        }

        Ok(Self {
            form_id: header.form_id,
            edid,
            full_name,
            model,
            sound,
            loop_sound,
            activate_sound,
            script,
            unknown_subrecords,
        })
    }
}

/// コンテナ定義レコード (`CONT`)。
///
/// 参照元: `references/openmw/components/esm4/loadcont.hpp:42-65`
#[derive(Clone, Debug, PartialEq)]
pub struct ContRecord {
    pub form_id: FormId,
    pub edid: String,
    pub full_name: Option<String>,
    pub model: String,
    pub capacity: f32,
    pub flags: u8,
    pub open_sound: Option<FormId>,
    pub close_sound: Option<FormId>,
    pub script: Option<FormId>,
    pub items: Vec<(FormId, i32)>,
    pub unknown_subrecords: Vec<Subrecord>,
}

impl ContRecord {
    pub fn parse(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut edid = String::new();
        let mut full_name = None;
        let mut model = String::new();
        let mut capacity = 0.0;
        let mut flags = 0;
        let mut open_sound = None;
        let mut close_sound = None;
        let mut script = None;
        let mut items = Vec::new();
        let mut unknown_subrecords = Vec::new();

        for sub in subrecords {
            match &sub.type_id.0 {
                b"EDID" => edid = sub.as_string(),
                b"FULL" => full_name = Some(sub.as_string()),
                b"MODL" => model = sub.as_string(),
                b"DATA" => {
                    if !sub.data.is_empty() {
                        flags = sub.data[0];
                    }
                    if sub.data.len() >= 8 {
                        capacity = f32::from_le_bytes(sub.data[4..8].try_into().unwrap());
                    }
                }
                b"SNAM" => {
                    if sub.data.len() >= 4 {
                        open_sound = Some(FormId(u32::from_le_bytes(sub.data[..4].try_into().unwrap())));
                    }
                }
                b"QNAM" => {
                    if sub.data.len() >= 4 {
                        close_sound = Some(FormId(u32::from_le_bytes(sub.data[..4].try_into().unwrap())));
                    }
                }
                b"SCRI" => {
                    if sub.data.len() >= 4 {
                        script = Some(FormId(u32::from_le_bytes(sub.data[..4].try_into().unwrap())));
                    }
                }
                b"CNTO" => {
                    if sub.data.len() >= 8 {
                        let item_id = FormId(u32::from_le_bytes(sub.data[0..4].try_into().unwrap()));
                        let count = i32::from_le_bytes(sub.data[4..8].try_into().unwrap());
                        items.push((item_id, count));
                    }
                }
                _ => unknown_subrecords.push(sub.clone()),
            }
        }

        Ok(Self {
            form_id: header.form_id,
            edid,
            full_name,
            model,
            capacity,
            flags,
            open_sound,
            close_sound,
            script,
            items,
            unknown_subrecords,
        })
    }
}

/// 家具定義レコード (`FURN`)。
///
/// 参照元: `references/openmw/components/esm4/loadfurn.hpp:42-60`
#[derive(Clone, Debug, PartialEq)]
pub struct FurnRecord {
    pub form_id: FormId,
    pub edid: String,
    pub full_name: Option<String>,
    pub model: String,
    pub unknown_subrecords: Vec<Subrecord>,
}

impl FurnRecord {
    pub fn parse(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut edid = String::new();
        let mut full_name = None;
        let mut model = String::new();
        let mut unknown_subrecords = Vec::new();

        for sub in subrecords {
            match &sub.type_id.0 {
                b"EDID" => edid = sub.as_string(),
                b"FULL" => full_name = Some(sub.as_string()),
                b"MODL" => model = sub.as_string(),
                _ => unknown_subrecords.push(sub.clone()),
            }
        }

        Ok(Self {
            form_id: header.form_id,
            edid,
            full_name,
            model,
            unknown_subrecords,
        })
    }
}

/// その他アイテム定義レコード (`MISC` / `KEYM`)。
///
/// 参照元: `references/openmw/components/esm4/loadmisc.hpp`, `loadkeym.hpp`
#[derive(Clone, Debug, PartialEq)]
pub struct MiscRecord {
    pub form_id: FormId,
    pub edid: String,
    pub full_name: Option<String>,
    pub model: String,
    pub value: u32,
    pub weight: f32,
    pub unknown_subrecords: Vec<Subrecord>,
}

impl MiscRecord {
    pub fn parse(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut edid = String::new();
        let mut full_name = None;
        let mut model = String::new();
        let mut value = 0;
        let mut weight = 0.0;
        let mut unknown_subrecords = Vec::new();

        for sub in subrecords {
            match &sub.type_id.0 {
                b"EDID" => edid = sub.as_string(),
                b"FULL" => full_name = Some(sub.as_string()),
                b"MODL" => model = sub.as_string(),
                b"DATA" => {
                    if sub.data.len() >= 4 {
                        value = u32::from_le_bytes(sub.data[0..4].try_into().unwrap());
                    }
                    if sub.data.len() >= 8 {
                        weight = f32::from_le_bytes(sub.data[4..8].try_into().unwrap());
                    }
                }
                _ => unknown_subrecords.push(sub.clone()),
            }
        }

        Ok(Self {
            form_id: header.form_id,
            edid,
            full_name,
            model,
            value,
            weight,
            unknown_subrecords,
        })
    }
}

/// ゲーム設定値定義レコード (`GMST`)。
///
/// 参照元: `references/openmw/components/esm4/loadgmst.hpp:35-50`
#[derive(Clone, Debug, PartialEq)]
pub enum GmstValue {
    Bool(bool),
    Int(i32),
    Float(f32),
    String(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct GmstRecord {
    pub form_id: FormId,
    pub edid: String,
    pub value: GmstValue,
    pub unknown_subrecords: Vec<Subrecord>,
}

impl GmstRecord {
    pub fn parse(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut edid = String::new();
        let mut value = GmstValue::Int(0);
        let mut unknown_subrecords = Vec::new();

        for sub in subrecords {
            match &sub.type_id.0 {
                b"EDID" => edid = sub.as_string(),
                b"DATA" => {
                    // EDID の先頭文字で型が決定する (Gamebryo / Bethesda 規約)
                    // b: bool, i: int, u: uint, f: float, s: string
                    if let Some(first_char) = edid.chars().next() {
                        match first_char {
                            'b' => {
                                let v = if !sub.data.is_empty() { sub.data[0] != 0 } else { false };
                                value = GmstValue::Bool(v);
                            }
                            'i' | 'u' => {
                                if sub.data.len() >= 4 {
                                    value = GmstValue::Int(i32::from_le_bytes(sub.data[..4].try_into().unwrap()));
                                }
                            }
                            'f' => {
                                if sub.data.len() >= 4 {
                                    value = GmstValue::Float(f32::from_le_bytes(sub.data[..4].try_into().unwrap()));
                                }
                            }
                            's' => {
                                value = GmstValue::String(sub.as_string());
                            }
                            _ => {
                                if sub.data.len() >= 4 {
                                    value = GmstValue::Int(i32::from_le_bytes(sub.data[..4].try_into().unwrap()));
                                }
                            }
                        }
                    }
                }
                _ => unknown_subrecords.push(sub.clone()),
            }
        }

        Ok(Self {
            form_id: header.form_id,
            edid,
            value,
            unknown_subrecords,
        })
    }
}
