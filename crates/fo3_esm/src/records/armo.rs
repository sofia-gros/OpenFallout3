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
pub const SUB_BMDT: FourCC = FourCC(*b"BMDT");
pub const SUB_BODT: FourCC = FourCC(*b"BODT");

/// 防具スロットフラグ定数 (Fallout 3 `BMDT` / `BODT` ArmorFlags)。
/// 参照元: `references/openmw/components/esm4/loadarmo.hpp:L65-85`, `loadarmo.cpp:L121-148`
pub mod armor_flags {
    pub const FO3_HEAD: u32 = 0x00000001;
    pub const FO3_HAIR: u32 = 0x00000002;
    pub const FO3_UPPER_BODY: u32 = 0x00000004;
    pub const FO3_LEFT_HAND: u32 = 0x00000008;
    pub const FO3_RIGHT_HAND: u32 = 0x00000010;
    pub const FO3_WEAPON: u32 = 0x00000020;
    pub const FO3_PIPBOY: u32 = 0x00000040;
    pub const FO3_BACKPACK: u32 = 0x00000080;
    pub const FO3_NECKLACE: u32 = 0x00000100;
    pub const FO3_HEADBAND: u32 = 0x00000200;
    pub const FO3_HAT: u32 = 0x00000400;
    pub const FO3_EYE_GLASSES: u32 = 0x00000800;
    pub const FO3_MASK: u32 = 0x00004000;
}

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
    /// 装備スロットフラグ (BMDT / BODT)
    pub armor_flags: u32,
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
        let mut armor_flags = 0u32;

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
                SUB_BMDT | SUB_BODT => {
                    if sub.data.len() >= 4 {
                        armor_flags = u32::from_le_bytes([
                            sub.data[0],
                            sub.data[1],
                            sub.data[2],
                            sub.data[3],
                        ]);
                    }
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
            armor_flags,
        })
    }

    /// 性別に応じた着用モデルパスを取得する。
    pub fn model_for_gender(&self, is_female: bool) -> Option<&str> {
        if is_female {
            if !self.female_model.is_empty() {
                Some(&self.female_model)
            } else if !self.male_model.is_empty() {
                Some(&self.male_model)
            } else {
                None
            }
        } else {
            if !self.male_model.is_empty() {
                Some(&self.male_model)
            } else {
                None
            }
        }
    }

    /// 頭部装備（帽子・ヘルメット・眼鏡・マスク等）であるか判定する。
    pub fn is_head(&self) -> bool {
        use armor_flags::*;
        (self.armor_flags & (FO3_HEAD | FO3_HAIR | FO3_HAT | FO3_HEADBAND | FO3_EYE_GLASSES | FO3_MASK)) != 0
    }

    /// 胴体衣装（服・アーマー）であるか判定する。
    pub fn is_upper_body(&self) -> bool {
        (self.armor_flags & armor_flags::FO3_UPPER_BODY) != 0
    }

    /// 手袋装備であるか判定する。
    pub fn is_hands(&self) -> bool {
        use armor_flags::*;
        (self.armor_flags & (FO3_LEFT_HAND | FO3_RIGHT_HAND)) != 0
    }

    /// 頭髪を完全に隠すヘルメット等（髪 NIF 自体を除外すべき防具）であるか判定する。
    pub fn hides_hair(&self) -> bool {
        (self.armor_flags & armor_flags::FO3_HAIR) != 0
    }

    /// 帽子であり、頭髪の "Hat" メッシュを表示すべき防具であるか判定する。
    pub fn shows_hat(&self) -> bool {
        (self.armor_flags & armor_flags::FO3_HAT) != 0
    }
}
