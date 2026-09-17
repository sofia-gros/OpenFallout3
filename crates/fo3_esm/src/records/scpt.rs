//! # スクリプトレコード (`SCPT`) パースモジュール
//!
//! 参照元:
//! - `references/openmw/components/esm4/loadscpt.hpp`, `loadscpt.cpp`
//! - `references/openmw/components/esm4/script.hpp:346-378`
//! - `references/bevyout/src/vsa/scripts/record.rs:210-250`

use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, REC_SCPT};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{self, Cursor};

/// スクリプト種別 (`SCHR.type`)。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ScriptType {
    /// オブジェクトスクリプト (REFR / CONT / DOOR / NPC 等にアタッチ)
    Object,
    /// クエストスクリプト (QUST にアタッチ)
    Quest,
    /// マジックエフェクトスクリプト (SPEL / ENCH 等)
    Effect,
    /// 未知の種別
    Unknown(u16),
}

impl From<u16> for ScriptType {
    fn from(val: u16) -> Self {
        match val {
            0 => Self::Object,
            1 => Self::Quest,
            0x100 => Self::Effect,
            other => Self::Unknown(other),
        }
    }
}

/// スクリプトヘッダー (`SCHR`, 固定 20 バイト)。
///
/// 参照元: `references/openmw/components/esm4/script.hpp:346-355`
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptHeader {
    /// 参照オブジェクト (`SCRO`/`SCRV`) 数
    pub ref_count: u32,
    /// コンパイル済みバイトコード (`SCDA`) サイズ
    pub compiled_size: u32,
    /// ローカル変数 (`SLSD`) 数
    pub variable_count: u32,
    /// スクリプト種別 (Object / Quest / Effect)
    pub script_type: ScriptType,
    /// フラグ (0x01: Enabled)
    pub flags: u16,
}

/// ローカル変数定義 (`SLSD` + `SCVR`)。
///
/// 参照元: `references/openmw/components/esm4/script.hpp:357-368`
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptLocalVar {
    /// 変数インデックス (1-indexed)
    pub index: u32,
    /// 変数型 (0: short/long/int, 1: float)
    pub var_type: u32,
    /// 変数名 (`SCVR`)
    pub name: String,
}

/// GECK スクリプトのイベント種別 (`Begin [EventType]`)。
/// 参照元: GECK Wiki "Begin", `references/openmw/apps/openmw/mwworld/refdata.cpp`
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ScriptEventType {
    /// プレイヤーまたは NPC がオブジェクトを操作 (Eキー)
    OnActivate,
    /// アイテムがインベントリに追加された時
    OnAdd,
    /// アイテムがドロップされた時
    OnDrop,
    /// アイテムが装備された時
    OnEquip,
    /// アイテムの装備が解除された時
    OnUnequip,
    /// トリガーボリューム境界に滞在中の毎フレーム
    OnTrigger,
    /// トリガーボリューム境界に侵入した時
    OnTriggerEnter,
    /// トリガーボリューム境界から退出した時
    OnTriggerLeave,
    /// 通常ゲームプレイ中、毎フレーム実行
    GameMode,
    /// UI メニューモード中実行
    MenuMode,
    /// アクター死亡時
    OnDeath,
    /// 被弾時
    OnHit,
    /// セルまたはオブジェクトのロード時
    OnLoad,
    /// その他のカスタムイベント名
    Custom(String),
}

impl ScriptEventType {
    /// イベント名文字列から ScriptEventType を判定する (大文字小文字不問)。
    pub fn from_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "onactivate" => Self::OnActivate,
            "onadd" => Self::OnAdd,
            "ondrop" => Self::OnDrop,
            "onequip" => Self::OnEquip,
            "onunequip" => Self::OnUnequip,
            "ontrigger" => Self::OnTrigger,
            "ontriggerenter" => Self::OnTriggerEnter,
            "ontriggerleave" => Self::OnTriggerLeave,
            "gamemode" => Self::GameMode,
            "menumode" => Self::MenuMode,
            "ondeath" => Self::OnDeath,
            "onhit" => Self::OnHit,
            "onload" => Self::OnLoad,
            _ => Self::Custom(name.to_string()),
        }
    }
}

/// スクリプト内の 1 つのイベントブロック (`Begin ... End`)。
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptBlock {
    /// イベント種別
    pub event_type: ScriptEventType,
    /// ブロック引数 (例: `player` などのターゲット名やFormID)
    pub args: Vec<String>,
    /// ブロック内のソース行一覧 (コメント除外・トリム済み)
    pub lines: Vec<String>,
}

/// スクリプトレコード (`SCPT`)。
#[derive(Clone, Debug, PartialEq)]
pub struct ScptRecord {
    /// レコード FormID
    pub form_id: FormId,
    /// エディタ ID (`EDID`)
    pub edid: String,
    /// スクリプトヘッダー (`SCHR`)
    pub header: ScriptHeader,
    /// コンパイル済みスクリプトバイトコード (`SCDA`)
    pub bytecode: Vec<u8>,
    /// スクリプトソーステキスト (`SCTX`)
    pub source_text: Option<String>,
    /// ローカル変数一覧 (`SLSD` + `SCVR`)
    pub local_vars: Vec<ScriptLocalVar>,
    /// 参照 FormID 一覧 (`SCRO` / `SCRV`)
    pub ref_objects: Vec<FormId>,
}

impl ScptRecord {
    /// ソーステキスト (`SCTX`) から `Begin ... End` イベントブロック群を構文解析する。
    pub fn parse_blocks(&self) -> Vec<ScriptBlock> {
        let Some(ref text) = self.source_text else {
            return Vec::new();
        };

        let mut blocks = Vec::new();
        let mut current_block: Option<(ScriptEventType, Vec<String>, Vec<String>)> = None;

        for raw_line in text.lines() {
            // コメント (; 以降) の除去と前後の空白トリム
            let line = raw_line.split(';').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }

            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.is_empty() {
                continue;
            }

            let first_lower = tokens[0].to_ascii_lowercase();

            if first_lower == "begin" {
                // 既存の未終了ブロックがあれば回収
                if let Some((event_type, args, lines)) = current_block.take() {
                    blocks.push(ScriptBlock {
                        event_type,
                        args,
                        lines,
                    });
                }

                if tokens.len() > 1 {
                    let event_type = ScriptEventType::from_name(tokens[1]);
                    let args = tokens[2..].iter().map(|s| s.to_string()).collect();
                    current_block = Some((event_type, args, Vec::new()));
                }
            } else if first_lower == "end" {
                if let Some((event_type, args, lines)) = current_block.take() {
                    blocks.push(ScriptBlock {
                        event_type,
                        args,
                        lines,
                    });
                }
            } else if let Some((_, _, ref mut lines)) = current_block {
                lines.push(line.to_string());
            }
        }

        // ファイル末尾で End が省略されている場合の回収
        if let Some((event_type, args, lines)) = current_block {
            blocks.push(ScriptBlock {
                event_type,
                args,
                lines,
            });
        }

        blocks
    }

    /// サブレコード群から SCPT レコードをパースする。
    pub fn parse(record_header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        if record_header.type_id != REC_SCPT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected SCPT record, found {:?}", record_header.type_id),
            ));
        }

        let mut edid = String::new();
        let mut header = None;
        let mut bytecode = Vec::new();
        let mut source_text = None;
        let mut local_vars = Vec::new();
        let mut ref_objects = Vec::new();

        let mut current_slsd: Option<(u32, u32)> = None;

        for sub in subrecords {
            match &sub.type_id.0 {
                b"EDID" => {
                    edid = sub.as_string();
                }
                b"SCHR" => {
                    if sub.data.len() >= 20 {
                        let mut cur = Cursor::new(&sub.data);
                        let _unused = cur.read_u32::<LittleEndian>()?;
                        let ref_count = cur.read_u32::<LittleEndian>()?;
                        let compiled_size = cur.read_u32::<LittleEndian>()?;
                        let variable_count = cur.read_u32::<LittleEndian>()?;
                        let script_type_raw = cur.read_u16::<LittleEndian>()?;
                        let flags = cur.read_u16::<LittleEndian>()?;

                        header = Some(ScriptHeader {
                            ref_count,
                            compiled_size,
                            variable_count,
                            script_type: ScriptType::from(script_type_raw),
                            flags,
                        });
                    }
                }
                b"SCDA" => {
                    bytecode = sub.data.clone();
                }
                b"SCTX" => {
                    source_text = Some(sub.as_string());
                }
                b"SLSD" => {
                    if sub.data.len() >= 20 {
                        let mut cur = Cursor::new(&sub.data);
                        let index = cur.read_u32::<LittleEndian>()?;
                        let _unk1 = cur.read_u32::<LittleEndian>()?;
                        let _unk2 = cur.read_u32::<LittleEndian>()?;
                        let _unk3 = cur.read_u32::<LittleEndian>()?;
                        let var_type = cur.read_u32::<LittleEndian>()?;
                        current_slsd = Some((index, var_type));
                    }
                }
                b"SCVR" => {
                    let var_name = sub.as_string();
                    if let Some((idx, vtype)) = current_slsd.take() {
                        local_vars.push(ScriptLocalVar {
                            index: idx,
                            var_type: vtype,
                            name: var_name,
                        });
                    }
                }
                b"SCRO" | b"SCRV" => {
                    if sub.data.len() >= 4 {
                        let mut cur = Cursor::new(&sub.data);
                        let form_id = FormId(cur.read_u32::<LittleEndian>()?);
                        ref_objects.push(form_id);
                    }
                }
                _ => {}
            }
        }

        let header = header.ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Missing SCHR subrecord in SCPT")
        })?;

        Ok(Self {
            form_id: record_header.form_id,
            edid,
            header,
            bytecode,
            source_text,
            local_vars,
            ref_objects,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_scpt_record() {
        let record_header = RecordHeader {
            type_id: REC_SCPT,
            data_size: 100,
            flags: 0,
            form_id: FormId(0x00012345),
            vc_info: 0,
            form_version: 15,
            vc_info2: 0,
        };

        // SCHR: 20 bytes (unused=0, ref_count=1, compiled_size=8, var_count=1, type=0 (Object), flags=1)
        let mut schr_bytes = Vec::new();
        schr_bytes.extend_from_slice(&0u32.to_le_bytes());
        schr_bytes.extend_from_slice(&1u32.to_le_bytes());
        schr_bytes.extend_from_slice(&8u32.to_le_bytes());
        schr_bytes.extend_from_slice(&1u32.to_le_bytes());
        schr_bytes.extend_from_slice(&0u16.to_le_bytes());
        schr_bytes.extend_from_slice(&1u16.to_le_bytes());

        // SLSD: 24 bytes
        let mut slsd_bytes = vec![0u8; 24];
        slsd_bytes[0..4].copy_from_slice(&1u32.to_le_bytes()); // index 1
        slsd_bytes[16..20].copy_from_slice(&0u32.to_le_bytes()); // type 0

        let subrecords = vec![
            Subrecord {
                type_id: crate::types::FourCC(*b"EDID"),
                data: b"TestDoorScript\0".to_vec(),
            },
            Subrecord {
                type_id: crate::types::FourCC(*b"SCHR"),
                data: schr_bytes,
            },
            Subrecord {
                type_id: crate::types::FourCC(*b"SCDA"),
                data: vec![0x10, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00],
            },
            Subrecord {
                type_id: crate::types::FourCC(*b"SCTX"),
                data: b"scn TestDoorScript\nBegin OnActivate\nEnd\0".to_vec(),
            },
            Subrecord {
                type_id: crate::types::FourCC(*b"SLSD"),
                data: slsd_bytes,
            },
            Subrecord {
                type_id: crate::types::FourCC(*b"SCVR"),
                data: b"bOpen\0".to_vec(),
            },
            Subrecord {
                type_id: crate::types::FourCC(*b"SCRO"),
                data: 0x000abcdeu32.to_le_bytes().to_vec(),
            },
        ];

        let scpt = ScptRecord::parse(&record_header, &subrecords).expect("Failed to parse SCPT");
        assert_eq!(scpt.edid, "TestDoorScript");
        assert_eq!(scpt.header.script_type, ScriptType::Object);
        assert_eq!(scpt.header.compiled_size, 8);
        assert_eq!(scpt.bytecode.len(), 8);
        assert_eq!(scpt.local_vars.len(), 1);
        assert_eq!(scpt.local_vars[0].name, "bOpen");
        assert_eq!(scpt.ref_objects, vec![FormId(0x000abcde)]);

        // イベントブロックの解析テスト
        let blocks = scpt.parse_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].event_type, ScriptEventType::OnActivate);
        assert!(blocks[0].args.is_empty());
    }

    #[test]
    fn test_parse_multi_block_script() {
        let script_text = r#"
scn MoriartyDoorScript

short bLocked
float timer

; ドアの操作イベント
Begin OnActivate player
    if ( bLocked == 1 )
        ShowMessage DoorLockedMsg
        Return
    endif
    Activate
End

; 毎フレームの更新
Begin GameMode
    if ( timer > 0 )
        set timer to timer - GetSecondsPassed
    endif
End
"#;

        let record = ScptRecord {
            form_id: FormId(0x1),
            edid: "MoriartyDoorScript".to_string(),
            header: ScriptHeader {
                ref_count: 0,
                compiled_size: 0,
                variable_count: 2,
                script_type: ScriptType::Object,
                flags: 1,
            },
            bytecode: Vec::new(),
            source_text: Some(script_text.to_string()),
            local_vars: Vec::new(),
            ref_objects: Vec::new(),
        };

        let blocks = record.parse_blocks();
        assert_eq!(blocks.len(), 2);

        assert_eq!(blocks[0].event_type, ScriptEventType::OnActivate);
        assert_eq!(blocks[0].args, vec!["player"]);
        assert_eq!(blocks[0].lines.len(), 5);
        assert_eq!(blocks[0].lines[0], "if ( bLocked == 1 )");
        assert_eq!(blocks[0].lines[4], "Activate");

        assert_eq!(blocks[1].event_type, ScriptEventType::GameMode);
        assert!(blocks[1].args.is_empty());
        assert_eq!(blocks[1].lines.len(), 3);
        assert_eq!(blocks[1].lines[0], "if ( timer > 0 )");
    }
}
