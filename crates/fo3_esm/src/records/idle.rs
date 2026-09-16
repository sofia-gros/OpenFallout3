//! IDLE (Idle Animation) レコードのデータ構造およびパース。
//!
//! PACK (AI Package) の `IDLA` が参照するアニメーションレコード。
//! `MODL` に KF アニメーションファイルのパスが格納される。
//!
//! 参照元: `references/bevyout/src/vsa/openmw_esm4/idle.rs:L5-22, L120-193`
//!         `knowledge/phase11_ai_package_and_quest_progression.md` (セクション 4.3)

use std::io;
use byteorder::{ByteOrder, LittleEndian};
use crate::subrecord::Subrecord;
use crate::types::{FormId, SUB_ANAM, SUB_CNAM, SUB_DATA, SUB_EDID, SUB_MODL};

/// IDLE (Idle Animation) レコード。
/// アニメーション群 (立っている・座る・歩き回る等) のうち 1 個のアニメーション定義。
///
/// 参照元: `references/bevyout/src/vsa/openmw_esm4/idle.rs:L5-22`
#[derive(Debug, Clone, Default)]
pub struct IdleRecord {
    pub form_id: FormId,
    pub record_flags: u32,
    /// エディタ識別名 (EDID)
    pub editor_id: Option<String>,
    /// アニメーション KF ファイルパス (MODL, C-String)
    /// 例: `Characters\_Male\IdleAnims\Swatting.KF`
    pub model_path: Option<String>,
    /// 親となるアニメーション FormID (ANAM offset 0)
    pub parent_form_id: Option<FormId>,
    /// 直前の兄弟アニメーション FormID (ANAM offset 4)
    pub previous_sibling_form_id: Option<FormId>,
    /// グリップセクション (DATA 先頭バイト、0x47/0x87/0x54 等)
    pub group_section_raw: u8,
    /// ループ最小回数
    pub loop_min: u8,
    /// ループ最大回数
    pub loop_max: u8,
    /// 再生間隔ディレイ秒数 (i16)
    pub replay_delay_seconds: i16,
    /// フラグ (DATA 末尾バイト)
    pub flags: u8,
    /// 適用条件 (CTDA, opaque bytes 保持 — 評価器は未実装)
    pub conditions: Vec<Vec<u8>>,
}

impl IdleRecord {
    /// サブレコード群から IDLE レコードを解析・構築する。
    /// 参照元: `references/bevyout/src/vsa/openmw_esm4/idle.rs:parse_idle`
    pub fn parse(form_id: FormId, record_flags: u32, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut editor_id = None;
        let mut model_path = None;
        let mut parent_form_id = None;
        let mut previous_sibling_form_id = None;
        let mut group_section_raw = 0u8;
        let mut loop_min = 0u8;
        let mut loop_max = 0u8;
        let mut replay_delay_seconds = 0i16;
        let mut flags = 0u8;
        let mut conditions = Vec::new();

        for sub in subrecords {
            match sub.type_id {
                SUB_EDID => {
                    editor_id = Some(sub.as_string());
                }
                SUB_MODL => {
                    model_path = Some(sub.as_string());
                }
                SUB_ANAM => {
                    // 8 バイト: [parent_form_id u32, previous_sibling_form_id u32]
                    if sub.data.len() >= 4 {
                        let raw_parent = LittleEndian::read_u32(&sub.data[0..4]);
                        if raw_parent != 0 {
                            parent_form_id = Some(FormId(raw_parent));
                        }
                    }
                    if sub.data.len() >= 8 {
                        let raw_sibling = LittleEndian::read_u32(&sub.data[4..8]);
                        if raw_sibling != 0 {
                            previous_sibling_form_id = Some(FormId(raw_sibling));
                        }
                    }
                }
                SUB_DATA => {
                    // 6 または 8 バイト: [group, loop_min, loop_max, replay(i16), flags]
                    // FO3 は 8 バイトレイアウトがドキュメント上だが、6 バイト (パディング省略) も許容する
                    let data = &sub.data;
                    if data.len() >= 3 {
                        group_section_raw = data[0];
                        loop_min = data[1];
                        loop_max = data[2];
                    }
                    if data.len() >= 5 {
                        replay_delay_seconds = LittleEndian::read_i16(&data[3..5]);
                    }
                    if data.len() >= 6 {
                        flags = data[5];
                    }
                }
                SUB_CNAM => {
                    // 戦闘スタイル等の参照 (IDLE では未使用のことが多い)
                }
                _ => {
                    // CTDA (適用条件) は opaque に保持
                    if sub.type_id.0 == *b"CTDA" {
                        conditions.push(sub.data.clone());
                    }
                }
            }
        }

        Ok(Self {
            form_id,
            record_flags,
            editor_id,
            model_path,
            parent_form_id,
            previous_sibling_form_id,
            group_section_raw,
            loop_min,
            loop_max,
            replay_delay_seconds,
            flags,
            conditions,
        })
    }
}