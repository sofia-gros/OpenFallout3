//! # WRLD (World Space) レコード
//!
//! Fallout 3 (Gamebryo 2.6) における屋外環境・ワールドスペースを定義。
//! 参照元:
//! - `references/openmw/components/esm4/loadwrld.hpp`
//! - `references/openmw/components/esm4/loadwrld.cpp`
//! - `knowledge/worldspace_cells.md`

use crate::types::{FormId, SUB_CNAM, SUB_DATA, SUB_EDID, SUB_FULL, SUB_NAM2, SUB_WNAM};
use crate::subrecord::Subrecord;

/// ワールドスペースレコード。
#[derive(Clone, Debug, PartialEq)]
pub struct WorldRecord {
    pub form_id: FormId,
    pub edid: String,
    pub full_name: Option<String>,
    pub world_flags: u8,
    pub parent_world: Option<FormId>,
    pub climate: Option<FormId>,
    pub water: Option<FormId>,
}

impl WorldRecord {
    /// サブレコード配列から WorldRecord を構築する。
    pub fn from_subrecords(form_id: FormId, subrecords: &[Subrecord]) -> Self {
        let mut edid = String::new();
        let mut full_name = None;
        let mut world_flags = 0u8;
        let mut parent_world = None;
        let mut climate = None;
        let mut water = None;

        for sub in subrecords {
            match sub.type_id {
                SUB_EDID => {
                    edid = sub.as_string();
                }
                SUB_FULL => {
                    full_name = Some(sub.as_string());
                }
                SUB_DATA => {
                    if !sub.data.is_empty() {
                        world_flags = sub.data[0];
                    }
                }
                SUB_WNAM => {
                    if sub.data.len() >= 4 {
                        parent_world = Some(FormId(u32::from_le_bytes(sub.data[0..4].try_into().unwrap())));
                    }
                }
                SUB_CNAM => {
                    if sub.data.len() >= 4 {
                        climate = Some(FormId(u32::from_le_bytes(sub.data[0..4].try_into().unwrap())));
                    }
                }
                SUB_NAM2 => {
                    if sub.data.len() >= 4 {
                        water = Some(FormId(u32::from_le_bytes(sub.data[0..4].try_into().unwrap())));
                    }
                }
                _ => {}
            }
        }

        Self {
            form_id,
            edid,
            full_name,
            world_flags,
            parent_world,
            climate,
            water,
        }
    }
}
