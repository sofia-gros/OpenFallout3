//! # TERM (Terminal) レコードモジュール
//!
//! Fallout 3 のコンピュータ・ターミナル定義レコード。
//! タイトル名、ウェルカムメッセージ、ハッキング難易度、および階層メニュー項目群を保持する。
//!
//! 参照元:
//! - `references/openmw/components/esm4/loadterm.hpp`
//! - `references/openmw/components/esm4/loadterm.cpp:34-117`
//! - `references/nifxml/nif.xml`
//! - UESP: `Fallout 3: Mod_File_Format/TERM`

use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, REC_TERM};
use std::io;

/// ターミナルのメニュー項目。
#[derive(Clone, Debug, PartialEq)]
pub struct TermMenuItem {
    /// 項目表示テキスト (`ITXT`)
    pub item_text: String,
    /// 遷移先 FormID (`INAM`: サブターミナルまたは Note)
    pub target_form_id: Option<FormId>,
    /// 選択時に表示される実行結果テキスト (`RNAM`)
    pub result_text: Option<String>,
}

/// ターミナル (`TERM`) レコード。
#[derive(Clone, Debug, PartialEq)]
pub struct TermRecord {
    /// レコード FormID
    pub form_id: FormId,
    /// エディタ ID (`EDID`)
    pub edid: String,
    /// ターミナル表示名 (`FULL`)
    pub full_name: Option<String>,
    /// ウェルカムテキスト / 本文 (`DESC`)
    pub description: Option<String>,
    /// パスワードノート FormID (`PNAM`)
    pub password_note: Option<FormId>,
    /// ハッキング難易度 (`DNAM`: 0=Very Easy, 1=Easy, 2=Average, 3=Hard, 4=Very Hard)
    pub difficulty: u8,
    /// メニュー項目一覧 (`ITXT`, `INAM`, `RNAM`)
    pub menu_items: Vec<TermMenuItem>,
}

impl TermRecord {
    /// サブレコード群から TERM レコードをパースする。
    ///
    /// 参照元: `references/openmw/components/esm4/loadterm.cpp:39-116`
    pub fn parse(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        if header.type_id != REC_TERM {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected TERM record, found {:?}", header.type_id),
            ));
        }

        let mut edid = String::new();
        let mut full_name = None;
        let mut description = None;
        let mut password_note = None;
        let mut difficulty = 0;
        let mut menu_items = Vec::new();

        let mut current_item_text = None;
        let mut current_target_id = None;
        let mut current_result_text = None;

        for sub in subrecords {
            match &sub.type_id.0 {
                b"EDID" => {
                    edid = sub.as_string();
                }
                b"FULL" => {
                    let s = sub.as_string();
                    if !s.is_empty() {
                        full_name = Some(s);
                    }
                }
                b"DESC" => {
                    let s = sub.as_string();
                    if !s.is_empty() {
                        description = Some(s);
                    }
                }
                b"PNAM" => {
                    if sub.data.len() >= 4 {
                        let id = u32::from_le_bytes(sub.data[..4].try_into().unwrap());
                        password_note = Some(FormId(id));
                    }
                }
                b"DNAM" => {
                    if !sub.data.is_empty() {
                        difficulty = sub.data[0];
                    }
                }
                b"ITXT" => {
                    // 新しいメニュー項目の開始: 前の項目があれば確定
                    if let Some(text) = current_item_text.take() {
                        menu_items.push(TermMenuItem {
                            item_text: text,
                            target_form_id: current_target_id.take(),
                            result_text: current_result_text.take(),
                        });
                    }
                    current_item_text = Some(sub.as_string());
                }
                b"INAM" => {
                    if sub.data.len() >= 4 {
                        let id = u32::from_le_bytes(sub.data[..4].try_into().unwrap());
                        current_target_id = Some(FormId(id));
                    }
                }
                b"RNAM" => {
                    let s = sub.as_string();
                    if !s.is_empty() {
                        current_result_text = Some(s);
                    }
                }
                _ => {}
            }
        }

        // 最後のメニュー項目を確定
        if let Some(text) = current_item_text {
            menu_items.push(TermMenuItem {
                item_text: text,
                target_form_id: current_target_id,
                result_text: current_result_text,
            });
        }

        Ok(Self {
            form_id: header.form_id,
            edid,
            full_name,
            description,
            password_note,
            difficulty,
            menu_items,
        })
    }
}
