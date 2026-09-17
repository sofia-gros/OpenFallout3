//! # メッセージレコード (`MESG`)
//!
//! ゲーム内メッセージボックス、確認ダイアログ、および性別選択などのボタン定義を格納する。
//!
//! 参照元: `references/nifxml/nif.xml`, GECK `Message` 仕様, Fallout 3 `Fallout3.esm:MESG`

use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, REC_MESG};
use std::io;

/// メッセージレコード (`MESG`)。
#[derive(Clone, Debug, PartialEq)]
pub struct MesgRecord {
    /// FormID
    pub form_id: FormId,
    /// エディタ ID (EDID)
    pub editor_id: String,
    /// メッセージ本文 (DESC / FULL)
    pub text: String,
    /// メッセージフラグ (DNAM: ビット0=MessageBox表示, etc.)
    pub flags: u32,
    /// ボタンラベル一覧 (ITXT の出現順)
    pub buttons: Vec<String>,
    /// アイコン画像パス (ICON, 存在する場合)
    pub icon: Option<String>,
}

impl MesgRecord {
    /// サブレコード配列から MESG レコードをパースする。
    pub fn parse(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        if header.type_id != REC_MESG {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected MESG record, found {:?}", header.type_id),
            ));
        }

        let mut editor_id = String::new();
        let mut text = String::new();
        let mut flags = 0;
        let mut buttons = Vec::new();
        let mut icon = None;

        for sub in subrecords {
            match &sub.type_id.0 {
                b"EDID" => {
                    editor_id = sub.as_string();
                }
                b"DESC" => {
                    text = sub.as_string();
                }
                b"FULL" => {
                    if text.is_empty() {
                        text = sub.as_string();
                    }
                }
                b"DNAM" => {
                    if sub.data.len() >= 4 {
                        flags = u32::from_le_bytes(sub.data[..4].try_into().unwrap());
                    }
                }
                b"ITXT" => {
                    buttons.push(sub.as_string());
                }
                b"ICON" => {
                    icon = Some(sub.as_string());
                }
                _ => {}
            }
        }

        Ok(Self {
            form_id: header.form_id,
            editor_id,
            text,
            flags,
            buttons,
            icon,
        })
    }

    /// メッセージボックス (モーダルダイアログ) として表示すべきか判定する。
    pub fn is_message_box(&self) -> bool {
        (self.flags & 0x01) != 0 || !self.buttons.is_empty()
    }
}
