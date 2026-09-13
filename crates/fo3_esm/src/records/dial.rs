//! # DIAL & INFO (Dialogue & Topic Response) レコードモジュール
//!
//! Fallout 3 の NPC 会話ダイアログトピックおよびセリフ応答レコード。
//!
//! 参照元:
//! - `references/openmw/components/esm4/loaddial.hpp`, `loaddial.cpp`
//! - `references/openmw/components/esm4/loadinfo.hpp`, `loadinfo.cpp`
//! - UESP: `Fallout 3: Mod_File_Format/DIAL`, `Fallout 3: Mod_File_Format/INFO`

use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, REC_DIAL, REC_INFO};
use std::io;

/// INFO レコードのフラグ定義。
/// 参照元: `references/openmw/components/esm4/loadinfo.hpp:42-54`
pub mod info_flags {
    /// 会話終了フラグ (Goodbye)
    pub const GOODBYE: u16 = 0x0001;
    /// ランダム選択
    pub const RANDOM: u16 = 0x0002;
    /// 一度だけ発言 (SayOnce)
    pub const SAY_ONCE: u16 = 0x0004;
}

/// 会話ダイアログトピックレコード (`DIAL`)。
#[derive(Clone, Debug, PartialEq)]
pub struct DialRecord {
    /// レコード FormID
    pub form_id: FormId,
    /// エディタ ID (`EDID`, 例: "GREETING", "MegatonMoriartyDad")
    pub edid: String,
    /// トピック表示名 / プレイヤー選択肢テキスト (`FULL`)
    pub prompt: Option<String>,
    /// 関連クエスト FormID 群 (`QSTI` / `QSTR`)
    pub quests: Vec<FormId>,
    /// 所属する INFO レコードの FormID 群（パース後に階層付け）
    pub info_ids: Vec<FormId>,
}

/// 会話・スクリプト条件式 (`CTDA`, 固定 20/24/28 バイト)。
///
/// 参照元: `references/openmw/components/esm4/loadinfo.cpp:81-105`
#[derive(Clone, Debug, PartialEq)]
pub struct TargetCondition {
    /// 比較演算子 (0: ==, 1: !=, 2: >, 3: >=, 4: <, 5: <=, bit 7: OR 結合)
    pub operator: u8,
    /// 比較対象値 (f32)
    pub comparison_value: f32,
    /// 評価関数インデックス (例: 0x0048 = GetStage, 0x0046 = GetIsID)
    pub function_index: u16,
    /// 第1パラメータ (FormID または整数)
    pub param1: u32,
    /// 第2パラメータ
    pub param2: u32,
    /// 実行対象 (0: Subject, 1: Target, 2: Reference)
    pub run_on: u32,
    /// 対象リファレンス FormID
    pub reference: FormId,
}

impl TargetCondition {
    /// バイト列から CTDA 条件式をパースする。
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 20 {
            return None;
        }
        let operator = data[0];
        let comparison_value = f32::from_le_bytes(data[4..8].try_into().ok()?);
        let function_index = u16::from_le_bytes(data[8..10].try_into().ok()?);
        let param1 = u32::from_le_bytes(data[12..16].try_into().ok()?);
        let param2 = if data.len() >= 20 {
            u32::from_le_bytes(data[16..20].try_into().ok()?)
        } else {
            0
        };
        let run_on = if data.len() >= 24 {
            u32::from_le_bytes(data[20..24].try_into().ok()?)
        } else {
            0
        };
        let reference = if data.len() >= 28 {
            FormId(u32::from_le_bytes(data[24..28].try_into().ok()?))
        } else {
            FormId(0)
        };

        Some(Self {
            operator,
            comparison_value,
            function_index,
            param1,
            param2,
            run_on,
            reference,
        })
    }
}

/// トピックへの応答セリフレコード (`INFO`)。
#[derive(Clone, Debug, PartialEq)]
pub struct InfoRecord {
    /// レコード FormID
    pub form_id: FormId,
    /// 親 DIAL (Topic) FormID (`PNAM` または所属グループ)
    pub topic_id: Option<FormId>,
    /// NPC 応答セリフ本文 (`NAM1`)
    pub response_text: String,
    /// フラグ (`DATA`: 0x01=Goodbye, 0x02=Random, 0x04=Say Once 等)
    pub flags: u16,
    /// 話者 NPC の FormID 条件 (CTDA 等から抽出)
    pub speaker_npc: Option<FormId>,
    /// 評価条件式リスト (`CTDA`)
    pub conditions: Vec<TargetCondition>,
    /// 選択時 Result Script ソース文字列 (`SCTX`)
    pub result_script_source: Option<String>,
    /// 選択時 Result Script バイトコード (`SCDA`)
    pub result_script_bytecode: Option<Vec<u8>>,
}

impl DialRecord {
    /// サブレコード群から DIAL レコードをパースする。
    pub fn parse(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        if header.type_id != REC_DIAL {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected DIAL record, found {:?}", header.type_id),
            ));
        }

        let mut edid = String::new();
        let mut prompt = None;
        let mut quests = Vec::new();

        for sub in subrecords {
            match &sub.type_id.0 {
                b"EDID" => {
                    edid = sub.as_string();
                }
                b"FULL" => {
                    let s = sub.as_string();
                    if !s.is_empty() {
                        prompt = Some(s);
                    }
                }
                b"QSTI" | b"QSTR" => {
                    if sub.data.len() >= 4 {
                        let id = u32::from_le_bytes(sub.data[..4].try_into().unwrap());
                        quests.push(FormId(id));
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            form_id: header.form_id,
            edid,
            prompt,
            quests,
            info_ids: Vec::new(),
        })
    }
}

impl InfoRecord {
    /// サブレコード群から INFO レコードをパースする。
    pub fn parse(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        if header.type_id != REC_INFO {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected INFO record, found {:?}", header.type_id),
            ));
        }

        let mut topic_id = None;
        let mut response_text = String::new();
        let mut flags = 0;
        let mut speaker_npc = None;
        let mut conditions = Vec::new();
        let mut result_script_source = None;
        let mut result_script_bytecode = None;

        for sub in subrecords {
            match &sub.type_id.0 {
                b"DATA" => {
                    if sub.data.len() >= 2 {
                        flags = u16::from_le_bytes(sub.data[..2].try_into().unwrap());
                    }
                }
                b"NAM1" => {
                    response_text = sub.as_string();
                }
                b"PNAM" => {
                    if sub.data.len() >= 4 {
                        let id = u32::from_le_bytes(sub.data[..4].try_into().unwrap());
                        topic_id = Some(FormId(id));
                    }
                }
                b"CTDA" => {
                    if let Some(cond) = TargetCondition::parse(&sub.data) {
                        if cond.function_index == 0x0046 && speaker_npc.is_none() && cond.param1 != 0 {
                            // 0x0046: GetIsID (話者判定)
                            speaker_npc = Some(FormId(cond.param1));
                        }
                        conditions.push(cond);
                    } else if sub.data.len() >= 12 {
                        let param1 = u32::from_le_bytes(sub.data[4..8].try_into().unwrap());
                        if param1 != 0 && speaker_npc.is_none() {
                            speaker_npc = Some(FormId(param1));
                        }
                    }
                }
                b"SCTX" => {
                    result_script_source = Some(sub.as_string());
                }
                b"SCDA" => {
                    result_script_bytecode = Some(sub.data.clone());
                }
                _ => {}
            }
        }

        Ok(Self {
            form_id: header.form_id,
            topic_id,
            response_text,
            flags,
            speaker_npc,
            conditions,
            result_script_source,
            result_script_bytecode,
        })
    }

    /// 会話終了 (Goodbye) フラグが立っているかを返す。
    pub fn is_goodbye(&self) -> bool {
        (self.flags & info_flags::GOODBYE) != 0
    }
}
