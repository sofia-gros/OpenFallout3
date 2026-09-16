//! PACK (AI Package) レコードのデータ構造およびパース。
//!
//! 参照元: `references/openmw/components/esm4/loadpack.hpp`,
//! `references/bevyout/src/vsa/openmw_esm4/actor_support.rs:L534-L600` (Fallout 3 PACK 仕様),
//! `references/bevyout/src/vsa/openmw_esm4/actor_support.rs:L503-511, L659-745` (Idle Collection),
//! `knowledge/phase11_ai_package_and_quest_progression.md`

use std::io;
use byteorder::{ByteOrder, LittleEndian};
use crate::subrecord::Subrecord;
use crate::types::{
    FormId, SUB_CNAM, SUB_CTDA, SUB_EDID, SUB_IDLA, SUB_IDLC, SUB_IDLF, SUB_IDLT, SUB_PKDT,
    SUB_PLDT, SUB_PSDT, SUB_PTDT,
};
use crate::records::TargetCondition;

/// パッケージの動作種別 (AI Package Type)。
/// 参照元: `references/openmw/components/esm4/loadpack.hpp:PKDT`, `actor_support.rs:L570`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PackType {
    Find = 0,
    Follow = 1,
    Escort = 2,
    Eat = 3,
    Sleep = 4,
    Wander = 5,
    Travel = 6,
    Accompany = 7,
    UseItemAt = 8,
    Ambush = 9,
    FleeNotCombat = 10,
    CastMagic = 11,
    Sandbox = 12,
    UseWeapon = 13,
    Unknown(u8),
}

impl From<u8> for PackType {
    fn from(val: u8) -> Self {
        match val {
            0 => PackType::Find,
            1 => PackType::Follow,
            2 => PackType::Escort,
            3 => PackType::Eat,
            4 => PackType::Sleep,
            5 => PackType::Wander,
            6 => PackType::Travel,
            7 => PackType::Accompany,
            8 => PackType::UseItemAt,
            9 => PackType::Ambush,
            10 => PackType::FleeNotCombat,
            11 => PackType::CastMagic,
            12 => PackType::Sandbox,
            13 => PackType::UseWeapon,
            other => PackType::Unknown(other),
        }
    }
}

/// パッケージの移動・滞在先ロケーションデータ (PLDT)。
/// 参照元: `references/openmw/components/esm4/loadpack.hpp:PLDT` (12バイト)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackLocation {
    /// 0: Near reference, 1: In cell, 2: Current loc, 3: Editor loc, 4: Object ID, 5: Object type, 0xFF: なし
    pub location_type: i32,
    /// 対象 FormId (location_type が 5 の場合以外)
    pub form_id: Option<FormId>,
    /// 許容半径 (World units)
    pub radius: i32,
}

/// パッケージの行動ターゲットデータ (PTDT)。
/// 参照元: `references/openmw/components/esm4/loadpack.hpp:PTDT` (12または16バイト)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackTarget {
    /// 0: Specific reference, 1: Object ID, 2: Object type, 0xFF: なし
    pub target_type: i32,
    /// 対象 FormId (target_type が 2 の場合以外)
    pub form_id: Option<FormId>,
    /// 目標距離
    pub distance: i32,
}

/// パッケージのスケジュールデータ (PSDT)。
/// 参照元: `references/openmw/components/esm4/loadpack.hpp:PSDT` (8バイト)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackSchedule {
    pub month: u8,
    pub day_of_week: u8,
    pub date: u8,
    pub time: u8,
    pub duration: u32,
}

/// パッケージに紐づく Idle アニメーションコレクション (IDLF/IDLC/IDLT/IDLA)。
/// NPC がこのパッケージ適用中に再生する Idle アニメーションの IDLE レコード FormID 列表。
/// 参照元: `references/bevyout/src/vsa/openmw_esm4/actor_support.rs:L503-511` (PackageIdleCollection),
///         `references/bevyout/src/vsa/openmw_esm4/actor_support.rs:L659-745` (decode_package_idle_collection)
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PackIdleCollection {
    /// アニメーションコレクションフラグ (IDLF, 先頭 1 バイト)
    pub flags: u8,
    /// アニメーション切替タイマー秒数 (IDLT, 4 バイト f32)
    pub timer_seconds: f32,
    /// 参照する IDLE レコード FormID リスト (IDLA, 4 バイト×N)
    pub animation_form_ids: Vec<FormId>,
}

/// PACK (AI Package) レコード。
/// NPC の自律動作・目的地移動・会話・追従パターンを規定する。
///
/// CTDA 条件式 (`conditions`) は FO3 では固定 20 バイト (`fnIndex`/`param1`/`param2` を含む)。
/// 参照元: `references/openmw/components/esm4/loadinfo.cpp:81-105` (CTDA サイズ分岐), `loadpack.cpp`
#[derive(Debug, Clone)]
pub struct PackRecord {
    pub form_id: FormId,
    pub record_flags: u32,
    pub editor_id: Option<String>,
    pub pack_type: PackType,
    pub flags: u32,
    pub behavior_flags: u16,
    pub location: Option<PackLocation>,
    pub target: Option<PackTarget>,
    pub schedule: Option<PackSchedule>,
    pub idle_collection: Option<PackIdleCollection>,
    pub quest_form_id: Option<FormId>,
    pub combat_style_form_id: Option<FormId>,
    pub topic_id: Option<FormId>,
    /// パッケージに付随する条件 (CTDA)（複数可）。すべて満たした時のみ有効
    pub conditions: Vec<TargetCondition>,
}

impl PackRecord {
    /// サブレコード群から PACK レコードを解析・構築する。
    /// 参照元: `actor_support.rs:parse_package`
    pub fn parse(
        form_id: FormId,
        record_flags: u32,
        subrecords: &[Subrecord],
    ) -> io::Result<Self> {
        let mut editor_id = None;
        let mut pack_type = PackType::Wander;
        let mut flags = 0u32;
        let mut behavior_flags = 0u16;
        let mut location = None;
        let mut target = None;
        let mut schedule = None;
        let mut idle_flags = 0u8;
        let mut idle_timer = 0.0f32;
        let mut idle_form_ids: Vec<FormId> = Vec::new();
        let mut quest_form_id = None;
        let mut combat_style_form_id = None;
        let mut topic_id = None;
        let mut conditions: Vec<TargetCondition> = Vec::new();

        for sub in subrecords {
            match sub.type_id {
                SUB_EDID => {
                    editor_id = Some(sub.as_string());
                }
                SUB_PKDT => {
                    let data = &sub.data;
                    if data.len() >= 4 {
                        flags = LittleEndian::read_u32(&data[0..4]);
                    }
                    if data.len() >= 5 {
                        pack_type = PackType::from(data[4]);
                    }
                    if data.len() >= 8 {
                        behavior_flags = LittleEndian::read_u16(&data[6..8]);
                    }
                }
                SUB_PLDT => {
                    let data = &sub.data;
                    if data.len() >= 12 {
                        let loc_type = LittleEndian::read_i32(&data[0..4]);
                        let raw_id = LittleEndian::read_u32(&data[4..8]);
                        let radius = LittleEndian::read_i32(&data[8..12]);
                        let fid = if loc_type != 5 && raw_id != 0 {
                            Some(FormId(raw_id))
                        } else {
                            None
                        };
                        location = Some(PackLocation {
                            location_type: loc_type,
                            form_id: fid,
                            radius,
                        });
                    }
                }
                SUB_PTDT => {
                    let data = &sub.data;
                    if data.len() >= 12 {
                        let tgt_type = LittleEndian::read_i32(&data[0..4]);
                        let raw_id = LittleEndian::read_u32(&data[4..8]);
                        let distance = LittleEndian::read_i32(&data[8..12]);
                        let fid = if tgt_type != 2 && raw_id != 0 {
                            Some(FormId(raw_id))
                        } else {
                            None
                        };
                        target = Some(PackTarget {
                            target_type: tgt_type,
                            form_id: fid,
                            distance,
                        });
                    }
                }
                SUB_PSDT => {
                    let data = &sub.data;
                    if data.len() >= 8 {
                        schedule = Some(PackSchedule {
                            month: data[0],
                            day_of_week: data[1],
                            date: data[2],
                            time: data[3],
                            duration: LittleEndian::read_u32(&data[4..8]),
                        });
                    }
                }
                SUB_CNAM => {
                    if sub.data.len() >= 4 {
                        combat_style_form_id = Some(FormId(LittleEndian::read_u32(&sub.data[0..4])));
                    }
                }
                // Idle Collection 関連 (IDLF/IDLC/IDLT/IDLA)
                // 参照: `actor_support.rs:decode_package_idle_collection` (L659-745)
                SUB_IDLF => {
                    if let Some(&b0) = sub.data.first() {
                        idle_flags = b0;
                    }
                }
                SUB_IDLC => {
                    // 数値情報 (1 または 4 バイト) – 今回は使わない
                }
                SUB_IDLT => {
                    if sub.data.len() >= 4 {
                        idle_timer = LittleEndian::read_f32(&sub.data[0..4]);
                    }
                }
                SUB_IDLA => {
                    // 4 バイトずつ切り出して IDLE FormID 抽出
                    let complete_len = sub.data.len() / 4 * 4;
                    for chunk in sub.data[..complete_len].chunks_exact(4) {
                        let raw = LittleEndian::read_u32(chunk);
                        if raw != 0 {
                            idle_form_ids.push(FormId(raw));
                        }
                    }
                }
                SUB_CTDA => {
                    if let Some(cond) = TargetCondition::parse(&sub.data) {
                        conditions.push(cond);
                    }
                }
                _ => {
                    // QSTI 等その他
                    if sub.type_id.0 == *b"QSTI" && sub.data.len() >= 4 {
                        quest_form_id = Some(FormId(LittleEndian::read_u32(&sub.data[0..4])));
                    }
                    if sub.type_id.0 == *b"TNAM" && sub.data.len() >= 4 {
                        topic_id = Some(FormId(LittleEndian::read_u32(&sub.data[0..4])));
                    }
                }
            }
        }

        let idle_collection = if idle_form_ids.is_empty() {
            None
        } else {
            Some(PackIdleCollection {
                flags: idle_flags,
                timer_seconds: idle_timer,
                animation_form_ids: idle_form_ids,
            })
        };

        Ok(Self {
            form_id,
            record_flags,
            editor_id,
            pack_type,
            flags,
            behavior_flags,
            location,
            target,
            schedule,
            idle_collection,
            quest_form_id,
            combat_style_form_id,
            topic_id,
            conditions,
        })
    }
}
