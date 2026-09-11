//! # LVLI レコード (レベルドアイテムリスト)
//!
//! プレイヤーのレベルや NPC の設定に応じて出現するアイテム（防具、武器、弾薬等）の抽選リストを定義する。
//! Fallout 3 の NPC インベントリ (CNTO) やコンテナは、具体的な武器・防具を直接保持する代わりに
//! LVLI を指定することが多く、実体装備の解決には LVLI の再帰的展開が必要となる。
//!
//! 参照元:
//! - `references/openmw/components/esm4/loadlvli.hpp:44` (`LevelledItem`)
//! - `references/openmw/components/esm4/loadlvli.cpp:57` (`ESM::fourCC("LVLO")`)
//! - `references/openmw/components/esm4/inventory.hpp:38` (`LVLO` struct)

use std::io;
use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, SUB_EDID, SUB_LVLD, SUB_LVLF, SUB_LVLO};

/// レベルドアイテムリスト内の個別エントリ (LVLO)。
///
/// 参照元: `references/openmw/components/esm4/inventory.hpp:38`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LvloEntry {
    /// 出現に必要な最小レベル
    pub level: i16,
    /// 対象アイテム (ARMO, WEAP, AMMO, あるいは入れ子の LVLI) の FormID
    pub item: FormId,
    /// アイテム個数
    pub count: i16,
}

/// レベルドアイテムリストレコード (LVLI)。
///
/// 参照元: `references/openmw/components/esm4/loadlvli.hpp:44`
#[derive(Clone, Debug, PartialEq)]
pub struct LvliRecord {
    pub form_id: FormId,
    /// エディタ用識別名 (EDID)
    pub edid: String,
    /// 何も出現しない確率 (LVLD)
    pub chance_none: i8,
    /// フラグ (LVLF)
    pub flags: u8,
    /// 抽選・配給対象アイテムリスト (LVLO)
    pub entries: Vec<LvloEntry>,
}

impl LvliRecord {
    pub fn from_record(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut edid = String::new();
        let mut chance_none = 0;
        let mut flags = 0;
        let mut entries = Vec::new();

        for sub in subrecords {
            match sub.type_id {
                SUB_EDID => {
                    edid = sub.as_string();
                }
                SUB_LVLD => {
                    if !sub.data.is_empty() {
                        chance_none = sub.data[0] as i8;
                    }
                }
                SUB_LVLF => {
                    if !sub.data.is_empty() {
                        flags = sub.data[0];
                    }
                }
                SUB_LVLO => {
                    // Fallout 3 / FONV: 12 バイト [level: i16, pad: u16, item: u32, count: i16, pad2: u16]
                    // Oblivion: 8 バイト [level: i16, item: u32, count: i16]
                    // 参照元: references/openmw/components/esm4/loadlvli.cpp:60-76
                    if sub.data.len() >= 12 {
                        let level = i16::from_le_bytes([sub.data[0], sub.data[1]]);
                        let item = u32::from_le_bytes([sub.data[4], sub.data[5], sub.data[6], sub.data[7]]);
                        let count = i16::from_le_bytes([sub.data[8], sub.data[9]]);
                        entries.push(LvloEntry {
                            level,
                            item: FormId(item),
                            count,
                        });
                    } else if sub.data.len() >= 8 {
                        let level = i16::from_le_bytes([sub.data[0], sub.data[1]]);
                        let item = u32::from_le_bytes([sub.data[2], sub.data[3], sub.data[4], sub.data[5]]);
                        let count = i16::from_le_bytes([sub.data[6], sub.data[7]]);
                        entries.push(LvloEntry {
                            level,
                            item: FormId(item),
                            count,
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(LvliRecord {
            form_id: header.form_id,
            edid,
            chance_none,
            flags,
            entries,
        })
    }
}
