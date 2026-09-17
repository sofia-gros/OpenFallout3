//! QUST (Quest) レコードのデータ構造およびパース。
//!
//! 参照元: Gamebryo 2.6 / GECK スクリプトシステム & `references/openmw/components/esm4/loadqust.hpp`
//! 参照元: `knowledge/phase10_script_and_event_system.md:QUST レコードのバイナリ構造仕様`

use crate::subrecord::Subrecord;
use crate::types::{
    FormId, SUB_DATA, SUB_EDID, SUB_FULL, SUB_INDX, SUB_NNAM, SUB_QOBJ, SUB_QSDT, SUB_SCRI,
    SUB_SCTX,
};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{self, Cursor};

/// クエストステージ情報。
#[derive(Debug, Clone, Default)]
pub struct QuestStage {
    /// ステージ番号 (例: 10, 20, 100)
    pub index: u16,
    /// ステージフラグ (0x01: Complete / Done)
    pub flags: u8,
    /// ステージ遷移時に実行されるスクリプトソース (SCTX)
    pub script_source: Option<String>,
}

/// クエスト目標 (Objective) 情報。
#[derive(Debug, Clone, Default)]
pub struct QuestObjective {
    /// 目標インデックス (例: 10, 20)
    pub index: u32,
    /// 目標表示テキスト (NNAM)
    pub text: String,
}

/// QUST (Quest) レコード。
#[derive(Debug, Clone, Default)]
pub struct QuestRecord {
    pub form_id: FormId,
    pub editor_id: String,
    pub name: String,
    pub flags: u8,
    pub priority: u8,
    pub quest_delay: f32,
    pub script_form_id: Option<FormId>,
    pub stages: Vec<QuestStage>,
    pub objectives: Vec<QuestObjective>,
}

impl QuestRecord {
    /// サブレコード一覧から QUST レコードを解析する。
    ///
    /// 参照元: `references/openmw/components/esm4/loadqust.cpp`
    pub fn parse(form_id: FormId, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut record = Self {
            form_id,
            ..Default::default()
        };

        let mut current_stage: Option<QuestStage> = None;
        let mut current_objective: Option<QuestObjective> = None;

        for sub in subrecords {
            match sub.type_id {
                SUB_EDID => {
                    record.editor_id = sub.as_string().trim_end_matches('\0').to_string();
                }
                SUB_FULL => {
                    record.name = sub.as_string().trim_end_matches('\0').to_string();
                }
                SUB_DATA => {
                    if sub.data.len() >= 8 {
                        let mut cur = Cursor::new(&sub.data);
                        record.flags = cur.read_u8()?;
                        record.priority = cur.read_u8()?;
                        let _pad = cur.read_u16::<LittleEndian>()?;
                        record.quest_delay = cur.read_f32::<LittleEndian>()?;
                    } else if !sub.data.is_empty() {
                        record.flags = sub.data[0];
                    }
                }
                SUB_SCRI => {
                    if sub.data.len() >= 4 {
                        let mut cur = Cursor::new(&sub.data);
                        record.script_form_id = Some(FormId(cur.read_u32::<LittleEndian>()?));
                    }
                }
                SUB_INDX => {
                    // 直前のステージを確定して保存
                    if let Some(st) = current_stage.take() {
                        record.stages.push(st);
                    }
                    if sub.data.len() >= 2 {
                        let mut cur = Cursor::new(&sub.data);
                        let idx = cur.read_u16::<LittleEndian>()?;
                        current_stage = Some(QuestStage {
                            index: idx,
                            flags: 0,
                            script_source: None,
                        });
                    }
                }
                SUB_QSDT => {
                    if let Some(ref mut st) = current_stage {
                        if !sub.data.is_empty() {
                            st.flags = sub.data[0];
                        }
                    }
                }
                SUB_SCTX => {
                    let source = sub.as_string().trim_end_matches('\0').to_string();
                    if let Some(ref mut st) = current_stage {
                        st.script_source = Some(source);
                    }
                }
                SUB_QOBJ => {
                    // 直前の目標を確定して保存
                    if let Some(obj) = current_objective.take() {
                        record.objectives.push(obj);
                    }
                    if sub.data.len() >= 4 {
                        let mut cur = Cursor::new(&sub.data);
                        let idx = cur.read_u32::<LittleEndian>()?;
                        current_objective = Some(QuestObjective {
                            index: idx,
                            text: String::new(),
                        });
                    }
                }
                SUB_NNAM => {
                    let text = sub.as_string().trim_end_matches('\0').to_string();
                    if let Some(ref mut obj) = current_objective {
                        obj.text = text;
                    }
                }
                _ => {}
            }
        }

        if let Some(st) = current_stage {
            record.stages.push(st);
        }
        if let Some(obj) = current_objective {
            record.objectives.push(obj);
        }

        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_quest_record() {
        let mut subrecords = Vec::new();
        // EDID
        subrecords.push(Subrecord {
            type_id: SUB_EDID,
            data: b"MQ01\0".to_vec(),
        });
        // FULL
        subrecords.push(Subrecord {
            type_id: SUB_FULL,
            data: b"Following in His Footsteps\0".to_vec(),
        });
        // DATA: flags=1, priority=50, pad=0, delay=5.0
        let mut data_bytes = vec![1u8, 50u8, 0, 0];
        data_bytes.extend_from_slice(&5.0f32.to_le_bytes());
        subrecords.push(Subrecord {
            type_id: SUB_DATA,
            data: data_bytes,
        });
        // Stage 10
        subrecords.push(Subrecord {
            type_id: SUB_INDX,
            data: 10u16.to_le_bytes().to_vec(),
        });
        subrecords.push(Subrecord {
            type_id: SUB_QSDT,
            data: vec![0u8],
        });
        // Stage 100 (complete)
        subrecords.push(Subrecord {
            type_id: SUB_INDX,
            data: 100u16.to_le_bytes().to_vec(),
        });
        subrecords.push(Subrecord {
            type_id: SUB_QSDT,
            data: vec![1u8],
        });
        subrecords.push(Subrecord {
            type_id: SUB_SCTX,
            data: b"SetStage MQ02 10\0".to_vec(),
        });
        // Objective 10
        subrecords.push(Subrecord {
            type_id: SUB_QOBJ,
            data: 10u32.to_le_bytes().to_vec(),
        });
        subrecords.push(Subrecord {
            type_id: SUB_NNAM,
            data: b"Speak to Colin Moriarty\0".to_vec(),
        });

        let quest = QuestRecord::parse(FormId(0x00014E87), &subrecords).expect("parse quest");
        assert_eq!(quest.editor_id, "MQ01");
        assert_eq!(quest.name, "Following in His Footsteps");
        assert_eq!(quest.flags, 1);
        assert_eq!(quest.priority, 50);
        assert_eq!(quest.quest_delay, 5.0);
        assert_eq!(quest.stages.len(), 2);
        assert_eq!(quest.stages[0].index, 10);
        assert_eq!(quest.stages[0].flags, 0);
        assert_eq!(quest.stages[1].index, 100);
        assert_eq!(quest.stages[1].flags, 1);
        assert_eq!(
            quest.stages[1].script_source.as_deref(),
            Some("SetStage MQ02 10")
        );
        assert_eq!(quest.objectives.len(), 1);
        assert_eq!(quest.objectives[0].index, 10);
        assert_eq!(quest.objectives[0].text, "Speak to Colin Moriarty");
    }
}
