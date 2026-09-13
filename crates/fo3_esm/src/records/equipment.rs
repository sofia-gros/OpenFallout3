use std::io;

use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::FormId;

/// 武器 (Weapon) レコード。
/// 参照元: `references/openmw/components/esm4/loadweap.hpp` (ESM4::Weapon)
#[derive(Clone, Debug, Default)]
pub struct WeapRecord {
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
    pub ammo_id: Option<FormId>,
    pub weapon_type: u32,
    pub speed: f32,
    pub reach: f32,
    pub weap_flags: u32,
    pub value: u32,
    pub health: u32,
    pub weight: f32,
    pub damage: u16,
    pub clip_size: u8,
    /// 全サブレコードをロスレス保持
    pub unknown_subrecords: Vec<Subrecord>,
}

impl WeapRecord {
    /// レコードヘッダーとサブレコード列から WeapRecord をパースする。
    /// 参照元: `references/openmw/components/esm4/loadweap.cpp`
    pub fn parse(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut rec = WeapRecord {
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
                b"NAM0" | b"AMMO" => {
                    if sub.data.len() >= 4 {
                        let id = u32::from_le_bytes(sub.data[0..4].try_into().unwrap());
                        rec.ammo_id = Some(FormId(id));
                    }
                }
                b"DATA" => {
                    // Fallout 3 Weapon Data: 29 bytes (type, speed, reach, flags, value, health, weight, damage, clipSize)
                    if sub.data.len() >= 28 {
                        rec.weapon_type = u32::from_le_bytes(sub.data[0..4].try_into().unwrap());
                        rec.speed = f32::from_le_bytes(sub.data[4..8].try_into().unwrap());
                        rec.reach = f32::from_le_bytes(sub.data[8..12].try_into().unwrap());
                        rec.weap_flags = u32::from_le_bytes(sub.data[12..16].try_into().unwrap());
                        rec.value = u32::from_le_bytes(sub.data[16..20].try_into().unwrap());
                        rec.health = u32::from_le_bytes(sub.data[20..24].try_into().unwrap());
                        rec.weight = f32::from_le_bytes(sub.data[24..28].try_into().unwrap());
                        if sub.data.len() >= 30 {
                            rec.damage = u16::from_le_bytes(sub.data[28..30].try_into().unwrap());
                        }
                        if sub.data.len() >= 31 {
                            rec.clip_size = sub.data[30];
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(rec)
    }
}

/// 弾薬 (Ammunition) レコード。
/// 参照元: `references/openmw/components/esm4/loadammo.hpp` (ESM4::Ammunition)
#[derive(Clone, Debug, Default)]
pub struct AmmoRecord {
    pub form_id: FormId,
    pub flags: u32,
    pub editor_id: Option<String>,
    pub full_name: Option<String>,
    pub model: Option<String>,
    pub icon: Option<String>,
    pub speed: f32,
    pub ammo_flags: u32,
    pub value: u32,
    pub weight: f32,
    pub damage: u16,
    /// 全サブレコードをロスレス保持
    pub unknown_subrecords: Vec<Subrecord>,
}

impl AmmoRecord {
    /// レコードヘッダーとサブレコード列から AmmoRecord をパースする。
    /// 参照元: `references/openmw/components/esm4/loadammo.cpp`
    pub fn parse(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut rec = AmmoRecord {
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
                b"DATA" => {
                    // Fallout 3: speed, flags, value, weight, damage
                    if sub.data.len() >= 16 {
                        rec.speed = f32::from_le_bytes(sub.data[0..4].try_into().unwrap());
                        rec.ammo_flags = u32::from_le_bytes(sub.data[4..8].try_into().unwrap());
                        rec.value = u32::from_le_bytes(sub.data[8..12].try_into().unwrap());
                        rec.weight = f32::from_le_bytes(sub.data[12..16].try_into().unwrap());
                    }
                    if sub.data.len() >= 18 {
                        rec.damage = u16::from_le_bytes(sub.data[16..18].try_into().unwrap());
                    }
                }
                _ => {}
            }
        }

        Ok(rec)
    }
}
