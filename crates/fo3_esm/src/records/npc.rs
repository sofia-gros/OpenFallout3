//! # NPC_ レコード (ノンプレイヤーキャラクター定義)
//!
//! 人間やグールなど NPC のベースアクター定義。
//! 参照元: `references/openmw/components/esm4/loadnpc.hpp`, `loadnpc.cpp`

use std::io;
use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, FourCC, ObjectBounds, SUB_DOFT, SUB_EDID, SUB_FULL, SUB_HNAM, SUB_OBND};

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
    /// デフォルト衣装 FormID (DOFT)
    pub default_outfit: Option<FormId>,
    /// 髪型 FormID (HNAM)
    pub hair: Option<FormId>,
}

impl NpcRecord {
    pub fn from_record(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut edid = String::new();
        let mut full_name = None;
        let mut bounds = None;
        let mut is_female = false;
        let mut race = FormId(0);
        let mut default_armor = None;
        let mut default_outfit = None;
        let mut hair = None;

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
                SUB_DOFT => {
                    if let Ok(id) = sub.as_form_id() {
                        default_outfit = Some(id);
                    }
                }
                SUB_HNAM => {
                    if let Ok(id) = sub.as_form_id() {
                        hair = Some(id);
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
            default_outfit,
            hair,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::{HairRecord, OtftRecord};
    use crate::types::{SUB_DATA, SUB_MODL};

    #[test]
    fn test_npc_record_doft_hnam_parsing() {
        let header = RecordHeader {
            type_id: crate::types::REC_NPC_,
            data_size: 0,
            flags: 0,
            form_id: FormId(0x00012345),
            vc_info: 0,
            form_version: 0,
            vc_info2: 0,
        };

        let subrecords = vec![
            Subrecord {
                type_id: SUB_EDID,
                data: b"TestNPC\0".to_vec(),
            },
            Subrecord {
                type_id: SUB_ACBS,
                data: 1u32.to_le_bytes().to_vec(), // is_female = true
            },
            Subrecord {
                type_id: SUB_DOFT,
                data: 0x00022222u32.to_le_bytes().to_vec(),
            },
            Subrecord {
                type_id: SUB_HNAM,
                data: 0x00033333u32.to_le_bytes().to_vec(),
            },
        ];

        let npc = NpcRecord::from_record(&header, &subrecords).unwrap();
        assert_eq!(npc.edid, "TestNPC");
        assert!(npc.is_female);
        assert_eq!(npc.default_outfit, Some(FormId(0x00022222)));
        assert_eq!(npc.hair, Some(FormId(0x00033333)));
    }

    #[test]
    fn test_otft_and_hair_record_parsing() {
        let otft_hdr = RecordHeader {
            type_id: crate::types::REC_OTFT,
            data_size: 0,
            flags: 0,
            form_id: FormId(0x00022222),
            vc_info: 0,
            form_version: 0,
            vc_info2: 0,
        };
        let mut inam_data = Vec::new();
        inam_data.extend_from_slice(&0x00044444u32.to_le_bytes());
        inam_data.extend_from_slice(&0x00055555u32.to_le_bytes());

        let otft_subs = vec![
            Subrecord {
                type_id: SUB_EDID,
                data: b"TestOutfit\0".to_vec(),
            },
            Subrecord {
                type_id: crate::types::SUB_INAM,
                data: inam_data,
            },
        ];

        let otft = OtftRecord::from_record(&otft_hdr, &otft_subs).unwrap();
        assert_eq!(otft.edid, "TestOutfit");
        assert_eq!(otft.inventory, vec![FormId(0x00044444), FormId(0x00055555)]);

        let hair_hdr = RecordHeader {
            type_id: crate::types::REC_HAIR,
            data_size: 0,
            flags: 0,
            form_id: FormId(0x00033333),
            vc_info: 0,
            form_version: 0,
            vc_info2: 0,
        };
        let hair_subs = vec![
            Subrecord {
                type_id: SUB_EDID,
                data: b"TestHair\0".to_vec(),
            },
            Subrecord {
                type_id: SUB_MODL,
                data: b"characters\\hair\\hair01.nif\0".to_vec(),
            },
            Subrecord {
                type_id: SUB_DATA,
                data: vec![0x02],
            },
        ];

        let hair = HairRecord::from_record(&hair_hdr, &hair_subs).unwrap();
        assert_eq!(hair.edid, "TestHair");
        assert_eq!(hair.model, "characters\\hair\\hair01.nif");
        assert_eq!(hair.flags, 0x02);
    }
}

