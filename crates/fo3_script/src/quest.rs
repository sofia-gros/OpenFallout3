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
    pub current_stages: HashMap<FormId, u16>,
    /// 到達済みのステージ履歴 (GetStageDone 用)
    stage_history: HashMap<FormId, HashSet<u16>>,
    /// 各クエストの目標表示状態 ((QuestFormId, ObjectiveIndex) -> Displayed)
    objectives_displayed: HashMap<(FormId, u32), bool>,
    /// クエスト固有のスクリプト変数 (QuestFormId -> VarName -> Value)
    quest_variables: HashMap<FormId, HashMap<String, f64>>,
    /// 実機 ESM からロードされたクエスト定義レコード群 (FormId -> QuestRecord)
    pub quests: HashMap<FormId, QuestRecord>,
    /// EditorID (大文字) -> Quest FormID 逆引きマップ
    pub edid_map: HashMap<String, FormId>,
    /// HUD / UI 表示用通知キュー
    pub notifications: Vec<String>,
}

impl QuestManager {
    /// 新しい QuestManager を生成する。
    pub fn new() -> Self {
        Self::default()
    }

    /// マスター ESM からロードされた全クエスト定義を一括登録する。
    pub fn register_all_quests(&mut self, quests: HashMap<FormId, QuestRecord>, edid_map: HashMap<String, FormId>) {
        self.quests = quests;
        self.edid_map = edid_map;
    }

    /// 単一のクエスト定義レコードを登録する。
    pub fn register_quest(&mut self, record: QuestRecord) {
        if !record.editor_id.is_empty() {
            self.edid_map.insert(record.editor_id.to_ascii_uppercase(), record.form_id);
        }
        self.quests.insert(record.form_id, record);
    }

    /// クエスト FormID または EditorID から FormId を解決する。
    pub fn resolve_quest_id(&self, identifier: &str) -> Option<FormId> {
        let trimmed = identifier.trim();
        if let Some(stripped) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
            if let Ok(val) = u32::from_str_radix(stripped, 16) {
                return Some(FormId(val));
            }
        }
        if let Ok(val) = trimmed.parse::<u32>() {
            return Some(FormId(val));
        }
        self.edid_map.get(&trimmed.to_ascii_uppercase()).copied()
    }

    /// クエスト定義レコードを取得する。
    pub fn get_quest(&self, quest: FormId) -> Option<&QuestRecord> {
        self.quests.get(&quest)
    }

    /// 指定クエストの目標テキスト (NNAM) を取得する。
    pub fn get_objective_text(&self, quest: FormId, objective: u32) -> Option<&str> {
        self.quests.get(&quest).and_then(|q| {
            q.objectives
                .iter()
                .find(|o| o.index == objective)
                .map(|o| o.text.as_str())
        })
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
    /// 実機レコードにステージ Result Script (SCTX) が定義されている場合はそれを抽出して返す。
    /// ステージ 0 以外は同ステージに既に到達済みの場合は重複実行をスキップする。
    /// 参照元: GECK `SetStage <QuestID> <Stage>`, `GetStageDone <QuestID> <Stage>`
    pub fn set_stage(
        &mut self,
        quest: FormId,
        stage: u16,
        record: Option<&QuestRecord>,
    ) -> Option<String> {
        // ステージ 0 以外は重複実行をガード (実機の GetStageDone チェックに相当)
        // 参照元: Fallout 3 実機スクリプト — setstage は履歴済みステージを再実行しない
        if stage > 0 {
            let already_done = self.stage_history
                .get(&quest)
                .map(|h| h.contains(&stage))
                .unwrap_or(false);
            if already_done {
                let quest_name = self.quests.get(&quest)
                    .map(|r| if !r.name.is_empty() { r.name.as_str() } else { r.editor_id.as_str() })
                    .unwrap_or("Unknown Quest");
                println!(
                    "[QuestManager] クエスト \"{}\" (0x{:08X}) ステージ {} は既に実行済み — スキップ",
                    quest_name, quest.0, stage
                );
                return None;
            }
        }

        let prev_stage = self.get_stage(quest);
        self.current_stages.insert(quest, stage);
        self.stage_history
            .entry(quest)
            .or_default()
            .insert(stage);

        // 引数 record が None の場合、内部保持している実機 QuestRecord から自動解決
        let effective_record = record.or_else(|| self.quests.get(&quest));

        let quest_name = effective_record
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

        // クエスト新規開始時の通知
        if prev_stage == 0 && stage > 0 {
            let notif = format!("[Quest Started] {}", quest_name);
            self.notifications.push(notif);
        }

        // ステージに紐づく Result Script を抽出
        if let Some(r) = effective_record {
            for st in &r.stages {
                if st.index == stage {
                    if let Some(ref script) = st.script_source {
                        println!(
                            "  - ステージ {} Result Script 自動実行: \"{}\"",
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
        let obj_text = self.get_objective_text(quest, objective).unwrap_or("");
        println!(
            "[QuestManager] クエスト 0x{:08X} 目標 {} (\"{}\") 表示設定: {}",
            quest.0, objective, obj_text, displayed
        );

        if displayed && !obj_text.is_empty() {
            let notif = format!("[Objective Added] {}", obj_text);
            self.notifications.push(notif);
        }
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

        // クエスト事前登録による record=None 時の自動解決テスト
        qm.register_quest(record);
        let script20 = qm.set_stage(q_id, 20, None);
        assert_eq!(script20, None);
        assert_eq!(qm.get_stage(q_id), 20);
        assert!(qm.get_stage_done(q_id, 10));
        assert!(qm.get_stage_done(q_id, 20));

        // 通知キューの検証 ([Quest Started] Following in His Footsteps)
        assert_eq!(qm.notifications.len(), 1);
        assert_eq!(qm.notifications[0], "[Quest Started] Following in His Footsteps");

        // 目標テスト
        qm.set_objective_displayed(q_id, 10, true);
        assert!(qm.is_objective_displayed(q_id, 10));
        assert!(!qm.is_objective_displayed(q_id, 20));
    }
}
