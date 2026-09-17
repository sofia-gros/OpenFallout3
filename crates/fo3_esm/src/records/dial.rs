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
///
/// 参照元: `references/openmw/components/esm4/loaddial.hpp:42-65`, `dialogue.hpp:32-44`
#[derive(Clone, Debug, PartialEq)]
pub struct DialRecord {
    /// レコード FormID
    pub form_id: FormId,
    /// エディタ ID (`EDID`, 例: "GREETING", "MegatonMoriartyDad")
    pub edid: String,
    /// トピック表示名 / プレイヤー選択肢テキスト (`FULL`)
    pub prompt: Option<String>,
    /// ダイアログ種別 (`DATA[0]`: 0=Topic, 1=Conversation, 2=Combat, 4=Detection, 6=Misc)
    pub dial_type: u8,
    /// ダイアログフラグ (`DATA[1]`: 0x01=Rumours, 0x02=TopLevel)
    pub dial_flags: u8,
    /// 優先度 (`PNAM`)
    pub priority: f32,
    /// 関連クエスト FormID 群 (`QSTI` / `QSTR`)
    pub quests: Vec<FormId>,
    /// 所属する INFO レコードの FormID 群（パース後に階層付け）
    pub info_ids: Vec<FormId>,
    /// スクリプト FormID (`SCRI`)
    pub script_id: Option<FormId>,
    /// 未知または拡張サブレコード群 (ロスレス保持)
    pub unknown_subrecords: Vec<Subrecord>,
}

impl DialRecord {
    /// プレイヤー選択可能な会話トピックかどうか (DTYP_Topic = 0)
    pub fn is_topic(&self) -> bool {
        self.dial_type == 0
    }

    /// 会話開始時にトップレベル選択肢として表示されるか (0x02: Top-Level)
    pub fn is_top_level(&self) -> bool {
        (self.dial_flags & 0x02) != 0
    }
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

/// 感情タイプ (`TRDT` 内の emotion type)。
/// 参照元: `references/openmw/components/esm4/loadinfo.hpp`
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum EmotionType {
    #[default]
    Neutral = 0,
    Anger = 1,
    Disgust = 2,
    Fear = 3,
    Sad = 4,
    Happy = 5,
    Surprise = 6,
}

/// セリフ応答メタデータ (`TRDT` サブレコード)。
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ResponseData {
    /// 感情タイプ
    pub emotion_type: u32,
    /// 感情強度 (0 - 100)
    pub emotion_value: u32,
    /// レスポンス番号
    pub response_number: u8,
    /// アイドルアニメーション FormID
    pub idle_anim: Option<FormId>,
}

/// トピックへの応答セリフレコード (`INFO`)。
///
/// 参照元: `references/openmw/components/esm4/loadinfo.hpp:56-85`, `loadinfo.cpp:51-185`
#[derive(Clone, Debug, PartialEq)]
pub struct InfoRecord {
    /// レコード FormID
    pub form_id: FormId,
    /// 親 DIAL (Topic) FormID (`PNAM` または所属グループ)
    pub topic_id: Option<FormId>,
    /// NPC 応答セリフ本文 (`NAM1`)
    pub response_text: String,
    /// アクター向け演出メモ (`NAM2`)
    pub actor_notes: Option<String>,
    /// フラグ (`DATA`: 0x01=Goodbye, 0x02=Random, 0x04=Say Once, 0x80=SpeechChallenge 等)
    pub flags: u16,
    /// 話者 NPC の FormID (`ANAM` または `CTDA: GetIsID`)
    pub speaker_npc: Option<FormId>,
    /// 評価条件式リスト (`CTDA`)
    pub conditions: Vec<TargetCondition>,
    /// レスポンスメタデータ (`TRDT`)
    pub response_data: Option<ResponseData>,
    /// スピーチチャレンジ難易度/データ (`DNAM`)
    pub speech_challenge: Option<u32>,
    /// プロンプト置換テキスト (`RNAM`)
    pub prompt_override: Option<String>,
    /// 分岐先選択肢トピック FormID 群 (`TCLT` / `NAME`)
    pub choices: Vec<FormId>,
    /// 選択時 Result Script ソース文字列 (`SCTX`)
    pub result_script_source: Option<String>,
    /// 選択時 Result Script バイトコード (`SCDA`)
    pub result_script_bytecode: Option<Vec<u8>>,
    /// 未知または拡張サブレコード群 (ロスレス保持)
    pub unknown_subrecords: Vec<Subrecord>,
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
        let mut dial_type = 0u8;
        let mut dial_flags = 0u8;
        let mut priority = 0.0f32;
        let mut script_id = None;
        let mut unknown_subrecords = Vec::new();

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
                b"DATA" => {
                    if !sub.data.is_empty() {
                        dial_type = sub.data[0];
                    }
                    if sub.data.len() >= 2 {
                        dial_flags = sub.data[1];
                    }
                }
                b"PNAM" => {
                    if sub.data.len() >= 4 {
                        priority = f32::from_le_bytes(sub.data[..4].try_into().unwrap());
                    }
                }
                b"QSTI" | b"QSTR" => {
                    if sub.data.len() >= 4 {
                        let id = u32::from_le_bytes(sub.data[..4].try_into().unwrap());
                        quests.push(FormId(id));
                    }
                }
                b"SCRI" => {
                    if sub.data.len() >= 4 {
                        let id = u32::from_le_bytes(sub.data[..4].try_into().unwrap());
                        script_id = Some(FormId(id));
                    }
                }
                _ => {
                    unknown_subrecords.push(sub.clone());
                }
            }
        }

        Ok(Self {
            form_id: header.form_id,
            edid,
            prompt,
            dial_type,
            dial_flags,
            priority,
            quests,
            info_ids: Vec::new(),
            script_id,
            unknown_subrecords,
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
        let mut actor_notes = None;
        let mut flags = 0;
        let mut speaker_npc = None;
        let mut conditions = Vec::new();
        let mut response_data = None;
        let mut speech_challenge = None;
        let mut prompt_override = None;
        let mut choices = Vec::new();
        let mut result_script_source = None;
        let mut result_script_bytecode = None;
        let mut unknown_subrecords = Vec::new();

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
                b"NAM2" => {
                    let s = sub.as_string();
                    if !s.is_empty() {
                        actor_notes = Some(s);
                    }
                }
                b"ANAM" => {
                    // Fallout 3: 話者 NPC FormID
                    if sub.data.len() >= 4 {
                        let id = u32::from_le_bytes(sub.data[..4].try_into().unwrap());
                        if id != 0 {
                            speaker_npc = Some(FormId(id));
                        }
                    }
                }
                b"DNAM" => {
                    // Fallout 3: スピーチチャレンジ難易度
                    if sub.data.len() >= 4 {
                        speech_challenge =
                            Some(u32::from_le_bytes(sub.data[..4].try_into().unwrap()));
                    }
                }
                b"RNAM" => {
                    let s = sub.as_string();
                    if !s.is_empty() {
                        prompt_override = Some(s);
                    }
                }
                b"PNAM" => {
                    if sub.data.len() >= 4 {
                        let id = u32::from_le_bytes(sub.data[..4].try_into().unwrap());
                        topic_id = Some(FormId(id));
                    }
                }
                b"TCLT" | b"NAME" => {
                    if sub.data.len() >= 4 {
                        let id = u32::from_le_bytes(sub.data[..4].try_into().unwrap());
                        choices.push(FormId(id));
                    }
                }
                b"TRDT" => {
                    if sub.data.len() >= 8 {
                        let emotion_type = u32::from_le_bytes(sub.data[0..4].try_into().unwrap());
                        let emotion_value = u32::from_le_bytes(sub.data[4..8].try_into().unwrap());
                        let response_number = if sub.data.len() >= 9 { sub.data[8] } else { 0 };
                        let idle_anim = if sub.data.len() >= 16 {
                            let id = u32::from_le_bytes(sub.data[12..16].try_into().unwrap());
                            if id != 0 {
                                Some(FormId(id))
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        response_data = Some(ResponseData {
                            emotion_type,
                            emotion_value,
                            response_number,
                            idle_anim,
                        });
                    }
                }
                b"CTDA" => {
                    if let Some(cond) = TargetCondition::parse(&sub.data) {
                        // 参照元: `references/openmw/components/esm4/script.hpp:114` (FUN_GetIsID = 72 = 0x0048)
                        if cond.function_index == 0x0048
                            && speaker_npc.is_none()
                            && cond.param1 != 0
                        {
                            speaker_npc = Some(FormId(cond.param1));
                        }
                        conditions.push(cond);
                    }
                }
                b"SCTX" => {
                    result_script_source = Some(sub.as_string());
                }
                b"SCDA" => {
                    result_script_bytecode = Some(sub.data.clone());
                }
                _ => {
                    unknown_subrecords.push(sub.clone());
                }
            }
        }

        Ok(Self {
            form_id: header.form_id,
            topic_id,
            response_text,
            actor_notes,
            flags,
            speaker_npc,
            conditions,
            response_data,
            speech_challenge,
            prompt_override,
            choices,
            result_script_source,
            result_script_bytecode,
            unknown_subrecords,
        })
    }

    /// 会話終了 (Goodbye) フラグが立っているかを返す。
    pub fn is_goodbye(&self) -> bool {
        (self.flags & info_flags::GOODBYE) != 0
    }
}
