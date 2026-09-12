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

/// トピックへの応答セリフレコード (`INFO`)。
#[derive(Clone, Debug, PartialEq)]
pub struct InfoRecord {
    /// レコード FormID
    pub form_id: FormId,
    /// 親 DIAL (Topic) FormID (`PNAM` または所属グループ)
    pub topic_id: Option<FormId>,
    /// NPC 応答セリフ本文 (`NAM1`)
    pub response_text: String,
    /// フラグ (`DATA`: Goodbye 等)
    pub flags: u16,
    /// 話者 NPC の FormID 条件 (CTDA 等から抽出)
    pub speaker_npc: Option<FormId>,
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
                    // CTDA 条件: 比較対象パラメータ (FormID)
                    // 参照元: `references/openmw/components/esm4/loadinfo.cpp:75`
                    if sub.data.len() >= 12 {
                        let param1 = u32::from_le_bytes(sub.data[4..8].try_into().unwrap());
                        if param1 != 0 && speaker_npc.is_none() {
                            speaker_npc = Some(FormId(param1));
                        }
                    }
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
        })
    }

    /// 会話終了 (Goodbye) フラグが立っているかを返す。
    pub fn is_goodbye(&self) -> bool {
        (self.flags & info_flags::GOODBYE) != 0
    }
}
