//! クエスト状態管理およびステージ進行エンジン。
//!
//! 参照元: Gamebryo 2.6 / GECK クエストシステム (`SetStage`, `GetStage`, `GetStageDone`, `SetObjectiveDisplayed`)
//! 参照元: `knowledge/phase10_script_and_event_system.md:Phase 10-C/D`

use std::collections::{HashMap, HashSet};
use fo3_esm::{FormId, QuestRecord};

/// クエスト進行状態マネージャー。
#[derive(Debug, Clone, Default)]
pub struct QuestManager {
    /// 各クエストの現在アクティブなステージ番号 (未開始は 0)
    current_stages: HashMap<FormId, u16>,
    /// 到達済みのステージ履歴 (GetStageDone 用)
    stage_history: HashMap<FormId, HashSet<u16>>,
    /// 各クエストの目標表示状態 ((QuestFormId, ObjectiveIndex) -> Displayed)
    objectives_displayed: HashMap<(FormId, u32), bool>,
    /// クエスト固有のスクリプト変数 (QuestFormId -> VarName -> Value)
    quest_variables: HashMap<FormId, HashMap<String, f64>>,
}

impl QuestManager {
    /// 新しい QuestManager を生成する。
    pub fn new() -> Self {
        Self::default()
    }

    /// クエストの現在ステージ番号を取得する。未開始の場合は 0 を返す。
    /// 参照元: GECK `GetStage <QuestID>`
    pub fn get_stage(&self, quest: FormId) -> u16 {
        self.current_stages.get(&quest).copied().unwrap_or(0)
    }

    /// クエストの指定ステージに到達済みかどうかを判定する。
    /// 参照元: GECK `GetStageDone <QuestID> <Stage>`
    pub fn get_stage_done(&self, quest: FormId, stage: u16) -> bool {
        self.stage_history
            .get(&quest)
            .map(|history| history.contains(&stage))
            .unwrap_or(false)
    }

    /// 全クエストの到達ステージ履歴 (FormId -> Set<u32>) を取得する。
    pub fn get_stage_history_u32(&self) -> HashMap<FormId, HashSet<u32>> {
        self.stage_history
            .iter()
            .map(|(k, v)| (*k, v.iter().map(|&s| s as u32).collect()))
            .collect()
    }

    /// クエストのステージを更新し、履歴へ登録する。
    /// ステージに紐づく Result Script ソースコードが存在する場合はそれを返す。
    /// 参照元: GECK `SetStage <QuestID> <Stage>`
    pub fn set_stage(
        &mut self,
        quest: FormId,
        stage: u16,
        record: Option<&QuestRecord>,
    ) -> Option<String> {
        let prev_stage = self.get_stage(quest);
        self.current_stages.insert(quest, stage);
        self.stage_history
            .entry(quest)
            .or_default()
            .insert(stage);

        let quest_name = record
            .map(|r| {
                if !r.name.is_empty() {
                    r.name.as_str()
                } else {
                    r.editor_id.as_str()
                }
            })
            .unwrap_or("Unknown Quest");

        println!(
            "[QuestManager] クエスト \"{}\" (0x{:08X}) ステージ更新: {} -> {}",
            quest_name, quest.0, prev_stage, stage
        );

        // ステージに紐づく Result Script を抽出
        if let Some(r) = record {
            for st in &r.stages {
                if st.index == stage {
                    if let Some(ref script) = st.script_source {
                        println!(
                            "  - ステージ {} Result Script 実行準備: \"{}\"",
                            stage,
                            script.trim()
                        );
                        return Some(script.clone());
                    }
                }
            }
        }

        None
    }

    /// クエスト目標の表示状態を設定する。
    /// 参照元: GECK `SetObjectiveDisplayed <QuestID> <ObjectiveIndex> <Flag>`
    pub fn set_objective_displayed(&mut self, quest: FormId, objective: u32, displayed: bool) {
        self.objectives_displayed.insert((quest, objective), displayed);
        println!(
            "[QuestManager] クエスト 0x{:08X} 目標 {} 表示設定: {}",
            quest.0, objective, displayed
        );
    }

    /// クエスト目標が表示中かどうかを取得する。
    pub fn is_objective_displayed(&self, quest: FormId, objective: u32) -> bool {
        self.objectives_displayed
            .get(&(quest, objective))
            .copied()
            .unwrap_or(false)
    }

    /// クエスト変数を設定する。
    pub fn set_quest_variable(&mut self, quest: FormId, name: &str, value: f64) {
        self.quest_variables
            .entry(quest)
            .or_default()
            .insert(name.to_ascii_uppercase(), value);
    }

    /// クエスト変数を取得する。
    pub fn get_quest_variable(&self, quest: FormId, name: &str) -> Option<f64> {
        self.quest_variables
            .get(&quest)
            .and_then(|vars| vars.get(&name.to_ascii_uppercase()).copied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fo3_esm::QuestStage;

    #[test]
    fn test_quest_manager_stages_and_objectives() {
        let mut qm = QuestManager::new();
        let q_id = FormId(0x00014E87); // MQ01

        let mut record = QuestRecord {
            form_id: q_id,
            editor_id: "MQ01".to_string(),
            name: "Following in His Footsteps".to_string(),
            ..Default::default()
        };
        record.stages.push(QuestStage {
            index: 10,
            flags: 0,
            script_source: Some("SetObjectiveDisplayed MQ01 10 1".to_string()),
        });
        record.stages.push(QuestStage {
            index: 20,
            flags: 0,
            script_source: None,
        });

        assert_eq!(qm.get_stage(q_id), 0);
        assert!(!qm.get_stage_done(q_id, 10));

        let script = qm.set_stage(q_id, 10, Some(&record));
        assert_eq!(script.as_deref(), Some("SetObjectiveDisplayed MQ01 10 1"));
        assert_eq!(qm.get_stage(q_id), 10);
        assert!(qm.get_stage_done(q_id, 10));

        qm.set_stage(q_id, 20, Some(&record));
        assert_eq!(qm.get_stage(q_id), 20);
        assert!(qm.get_stage_done(q_id, 10));
        assert!(qm.get_stage_done(q_id, 20));

        // 目標テスト
        qm.set_objective_displayed(q_id, 10, true);
        assert!(qm.is_objective_displayed(q_id, 10));
        assert!(!qm.is_objective_displayed(q_id, 20));
    }
}
