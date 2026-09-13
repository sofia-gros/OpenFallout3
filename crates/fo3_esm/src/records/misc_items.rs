use std::io;

use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::FormId;

/// 薬品・回復アイテム (Alchemy / Ingestible) レコード。
/// 参照元: `references/openmw/components/esm4/loadalch.hpp` (ESM4::Potion)
#[derive(Clone, Debug, Default)]
pub struct AlchRecord {
    pub form_id: FormId,
    pub flags: u32,
    pub editor_id: Option<String>,
    pub full_name: Option<String>,
    pub model: Option<String>,
    pub icon: Option<String>,
    pub mini_icon: Option<String>,
    pub script_id: Option<FormId>,
    pub pickup_sound: Option<FormId>,
    pub drop_sound: Option<FormId>,
    pub weight: f32,
    pub value: i32,
    pub alch_flags: u32,
    /// 全サブレコードをロスレス保持
    pub unknown_subrecords: Vec<Subrecord>,
}

impl AlchRecord {
    /// レコードヘッダーとサブレコード列から AlchRecord をパースする。
    /// 参照元: `references/openmw/components/esm4/loadalch.cpp`
    pub fn parse(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut rec = AlchRecord {
            form_id: header.form_id,
            flags: header.flags,
            unknown_subrecords: subrecords.to_vec(),
            ..Default::default()
        };

        for sub in subrecords {
            match &sub.type_id.0 {
                b"EDID" => {
                    rec.editor_id = Some(sub.as_string());
                }
                b"FULL" => {
                    rec.full_name = Some(sub.as_string());
                }
                b"MODL" => {
                    rec.model = Some(sub.as_string());
                }
                b"ICON" => {
                    rec.icon = Some(sub.as_string());
                }
                b"MICO" => {
                    rec.mini_icon = Some(sub.as_string());
                }
                b"SCRI" => {
                    if sub.data.len() >= 4 {
                        let id = u32::from_le_bytes(sub.data[0..4].try_into().unwrap());
                        rec.script_id = Some(FormId(id));
                    }
                }
                b"DATA" => {
                    if sub.data.len() >= 4 {
                        rec.weight = f32::from_le_bytes(sub.data[0..4].try_into().unwrap());
                    }
                }
                b"ENIT" => {
                    if sub.data.len() >= 4 {
                        rec.value = i32::from_le_bytes(sub.data[0..4].try_into().unwrap());
                    }
                    if sub.data.len() >= 8 {
                        rec.alch_flags = u32::from_le_bytes(sub.data[4..8].try_into().unwrap());
                    }
                }
                b"YNAM" => {
                    if sub.data.len() >= 4 {
                        let id = u32::from_le_bytes(sub.data[0..4].try_into().unwrap());
                        rec.pickup_sound = Some(FormId(id));
                    }
                }
                b"ZNAM" => {
                    if sub.data.len() >= 4 {
                        let id = u32::from_le_bytes(sub.data[0..4].try_into().unwrap());
                        rec.drop_sound = Some(FormId(id));
                    }
                }
                _ => {}
            }
        }

        Ok(rec)
    }
}

/// 書籍 (Book) レコード。
/// 参照元: `references/openmw/components/esm4/loadbook.hpp` (ESM4::Book)
#[derive(Clone, Debug, Default)]
pub struct BookRecord {
    pub form_id: FormId,
    pub flags: u32,
    pub editor_id: Option<String>,
    pub full_name: Option<String>,
    pub model: Option<String>,
    pub icon: Option<String>,
    pub script_id: Option<FormId>,
    pub text: Option<String>,
    pub book_flags: u8,
    pub book_skill: i8,
    pub value: u32,
    pub weight: f32,
    /// 全サブレコードをロスレス保持
    pub unknown_subrecords: Vec<Subrecord>,
}

impl BookRecord {
    /// レコードヘッダーとサブレコード列から BookRecord をパースする。
    /// 参照元: `references/openmw/components/esm4/loadbook.cpp`
    pub fn parse(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut rec = BookRecord {
            form_id: header.form_id,
            flags: header.flags,
            unknown_subrecords: subrecords.to_vec(),
            ..Default::default()
        };

        for sub in subrecords {
            match &sub.type_id.0 {
                b"EDID" => {
                    rec.editor_id = Some(sub.as_string());
                }
                b"FULL" => {
                    rec.full_name = Some(sub.as_string());
                }
                b"MODL" => {
                    rec.model = Some(sub.as_string());
                }
                b"ICON" => {
                    rec.icon = Some(sub.as_string());
                }
                b"DESC" => {
                    rec.text = Some(sub.as_string());
                }
                b"SCRI" => {
                    if sub.data.len() >= 4 {
                        let id = u32::from_le_bytes(sub.data[0..4].try_into().unwrap());
                        rec.script_id = Some(FormId(id));
                    }
                }
                b"DATA" => {
                    // Fallout 3: flags(1), bookSkill(1), value(4), weight(4) = 10 bytes
                    if sub.data.len() >= 10 {
                        rec.book_flags = sub.data[0];
                        rec.book_skill = sub.data[1] as i8;
                        rec.value = u32::from_le_bytes(sub.data[2..6].try_into().unwrap());
                        rec.weight = f32::from_le_bytes(sub.data[6..10].try_into().unwrap());
                    } else if sub.data.len() >= 8 {
                        rec.value = u32::from_le_bytes(sub.data[0..4].try_into().unwrap());
                        rec.weight = f32::from_le_bytes(sub.data[4..8].try_into().unwrap());
                    }
                }
                _ => {}
            }
        }

        Ok(rec)
    }
}

/// 鍵 (Key) レコード。
/// 参照元: `references/openmw/components/esm4/loadkeym.hpp` (ESM4::Key)
#[derive(Clone, Debug, Default)]
pub struct KeymRecord {
    pub form_id: FormId,
    pub flags: u32,
    pub editor_id: Option<String>,
    pub full_name: Option<String>,
    pub model: Option<String>,
    pub icon: Option<String>,
    pub script_id: Option<FormId>,
    pub value: u32,
    pub weight: f32,
    /// 全サブレコードをロスレス保持
    pub unknown_subrecords: Vec<Subrecord>,
}

impl KeymRecord {
    /// レコードヘッダーとサブレコード列から KeymRecord をパースする。
    /// 参照元: `references/openmw/components/esm4/loadkeym.cpp`
    pub fn parse(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut rec = KeymRecord {
            form_id: header.form_id,
            flags: header.flags,
            unknown_subrecords: subrecords.to_vec(),
            ..Default::default()
        };

        for sub in subrecords {
            match &sub.type_id.0 {
                b"EDID" => {
                    rec.editor_id = Some(sub.as_string());
                }
                b"FULL" => {
                    rec.full_name = Some(sub.as_string());
                }
                b"MODL" => {
                    rec.model = Some(sub.as_string());
                }
                b"ICON" => {
                    rec.icon = Some(sub.as_string());
                }
                b"SCRI" => {
                    if sub.data.len() >= 4 {
                        let id = u32::from_le_bytes(sub.data[0..4].try_into().unwrap());
                        rec.script_id = Some(FormId(id));
                    }
                }
                b"DATA" => {
                    if sub.data.len() >= 8 {
                        rec.value = u32::from_le_bytes(sub.data[0..4].try_into().unwrap());
                        rec.weight = f32::from_le_bytes(sub.data[4..8].try_into().unwrap());
                    }
                }
                _ => {}
            }
        }

        Ok(rec)
    }
}
