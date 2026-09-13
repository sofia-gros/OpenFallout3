//! # スクリプト仮想マシン (VM) カーネル
//!
//! Fallout 3 実機アセットのスクリプト (`SCPT`) および会話・ターミナルの Result Script を決定論的に実行する。
//! 参照元: `references/bevyout/docs/plans/M7_SCRIPTING_ARCHITECTURE_ROADMAP.md`

use std::collections::{HashMap, HashSet};
use fo3_esm::FormId;
use crate::opcodes::functions::*;
use crate::quest::QuestManager;

/// スクリプト実行時エラー。
#[derive(Debug, thiserror::Error)]
pub enum ScriptError {
    #[error("Unknown command or variable: {0}")]
    UnknownCommand(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Invalid arguments for function: {0}")]
    InvalidArguments(String),
}

/// スクリプト実行コンテキストおよび永続化ステートマシン。
pub struct ScriptVm {
    /// グローバル変数 (`GLOB`) マップ (EDID -> f32)
    pub globals: HashMap<String, f32>,
    /// クエストステージマップ (QuestFormID -> Stage) [互換用]
    pub quest_stages: HashMap<FormId, u32>,
    /// クエスト進行状態マネージャー (GetStageDone, 目標管理など)
    pub quest_manager: QuestManager,
    /// EditorID (大文字) -> FormID 逆引きマップ
    pub edid_map: HashMap<String, FormId>,
    /// 現在実行中のローカル変数テーブル (変数名 -> f32)
    pub locals: HashMap<String, f32>,
    /// プレイヤー所持品マップ (ItemFormID -> Count)
    pub inventory: HashMap<FormId, u32>,
    /// 有効化されたオブジェクト FormID 群
    pub enabled_objects: HashSet<FormId>,
    /// 無効化されたオブジェクト FormID 群
    pub disabled_objects: HashSet<FormId>,
    /// 解錠されたオブジェクト FormID 群
    pub unlocked_objects: HashSet<FormId>,
}

impl Default for ScriptVm {
    fn default() -> Self {
        let mut globals = HashMap::new();
        globals.insert("GameHour".to_string(), 8.0);
        globals.insert("GameDaysPassed".to_string(), 1.0);

        Self {
            globals,
            quest_stages: HashMap::new(),
            quest_manager: QuestManager::new(),
            edid_map: HashMap::new(),
            locals: HashMap::new(),
            inventory: HashMap::new(),
            enabled_objects: HashSet::new(),
            disabled_objects: HashSet::new(),
            unlocked_objects: HashSet::new(),
        }
    }
}

impl ScriptVm {
    /// 新規 ScriptVm を生成。
    pub fn new() -> Self {
        Self::default()
    }

    /// 文字列 (0x16進数, 10進数, または EditorID) から FormID を解決。
    pub fn resolve_form_id(&self, s: &str) -> Result<FormId, ScriptError> {
        let clean = s.trim();
        if let Some(hex) = clean.strip_prefix("0x").or_else(|| clean.strip_prefix("0X")) {
            u32::from_str_radix(hex, 16)
                .map(FormId)
                .map_err(|e| ScriptError::ParseError(e.to_string()))
        } else if let Ok(val) = clean.parse::<u32>() {
            Ok(FormId(val))
        } else if let Some(&form_id) = self.edid_map.get(&clean.to_ascii_uppercase()) {
            Ok(form_id)
        } else {
            Err(ScriptError::ParseError(format!("Unknown FormID/EditorID: {}", clean)))
        }
    }

    /// クエストのステージを設定。
    pub fn set_stage(&mut self, quest_id: FormId, stage: u32) {
        self.quest_stages.insert(quest_id, stage);
        self.quest_manager.set_stage(quest_id, stage as u16, None);
    }

    /// クエストのステージを取得。
    pub fn get_stage(&self, quest_id: FormId) -> u32 {
        self.quest_manager.get_stage(quest_id) as u32
    }

    /// アイテムを追加。
    pub fn add_item(&mut self, item_id: FormId, count: u32) {
        *self.inventory.entry(item_id).or_insert(0) += count;
    }

    /// アイテムを除去。
    pub fn remove_item(&mut self, item_id: FormId, count: u32) -> u32 {
        if let Some(c) = self.inventory.get_mut(&item_id) {
            let removed = (*c).min(count);
            *c -= removed;
            removed
        } else {
            0
        }
    }

    /// アイテム所持数を取得。
    pub fn get_item_count(&self, item_id: FormId) -> u32 {
        self.inventory.get(&item_id).copied().unwrap_or(0)
    }

    /// オブジェクトを解錠。
    pub fn unlock(&mut self, form_id: FormId) {
        self.unlocked_objects.insert(form_id);
    }

    /// オブジェクトが解錠されているか判定。
    pub fn is_unlocked(&self, form_id: FormId) -> bool {
        self.unlocked_objects.contains(&form_id)
    }

    /// 単一行のテキストスクリプトステートメントを実行。
    ///
    /// 例:
    /// - `SetStage 0x00012345 10`
    /// - `set bOpen to 1`
    /// - `player.additem 0x000abcde 1`
    /// - `Unlock`
    pub fn execute_statement(&mut self, line: &str, self_id: Option<FormId>) -> Result<(), ScriptError> {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') {
            return Ok(());
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        let cmd = parts[0].to_lowercase();

        match cmd.as_str() {
            "setstage" => {
                if parts.len() >= 3 {
                    let q_id = self.resolve_form_id(parts[1])?;
                    let stage = parts[2].parse::<u32>().map_err(|e| ScriptError::ParseError(e.to_string()))?;
                    self.set_stage(q_id, stage);
                }
            }
            "setobjectivedisplayed" => {
                // SetObjectiveDisplayed <QuestID> <ObjectiveIndex> <Flag>
                if parts.len() >= 4 {
                    let q_id = self.resolve_form_id(parts[1])?;
                    let obj_idx = parts[2].parse::<u32>().map_err(|e| ScriptError::ParseError(e.to_string()))?;
                    let flag = parts[3].parse::<u32>().unwrap_or(0) != 0;
                    self.quest_manager.set_objective_displayed(q_id, obj_idx, flag);
                }
            }
            "set" => {
                // set [var] to [value]
                if parts.len() >= 4 && parts[2].eq_ignore_ascii_case("to") {
                    let var_name = parts[1];
                    let val = parts[3].parse::<f32>().map_err(|e| ScriptError::ParseError(e.to_string()))?;
                    if self.globals.contains_key(var_name) {
                        self.globals.insert(var_name.to_string(), val);
                    } else {
                        self.locals.insert(var_name.to_string(), val);
                    }
                }
            }
            "additem" | "player.additem" => {
                if parts.len() >= 3 {
                    let item_id = parse_form_id(parts[1])?;
                    let count = parts[2].parse::<u32>().unwrap_or(1);
                    self.add_item(item_id, count);
                }
            }
            "removeitem" | "player.removeitem" => {
                if parts.len() >= 3 {
                    let item_id = parse_form_id(parts[1])?;
                    let count = parts[2].parse::<u32>().unwrap_or(1);
                    self.remove_item(item_id, count);
                }
            }
            "unlock" => {
                if let Some(id) = self_id {
                    self.unlock(id);
                } else if parts.len() >= 2 {
                    let id = parse_form_id(parts[1])?;
                    self.unlock(id);
                }
            }
            "enable" => {
                if let Some(id) = self_id {
                    self.enabled_objects.insert(id);
                    self.disabled_objects.remove(&id);
                }
            }
            "disable" => {
                if let Some(id) = self_id {
                    self.disabled_objects.insert(id);
                    self.enabled_objects.remove(&id);
                }
            }
            "return" => {
                // Return は execute_block で中断フラグとして検出される
            }
            _ => {
                // ドット呼び出し対応 (例: TargetRef.Unlock, TargetRef.SetStage)
                if let Some((target_str, sub_cmd)) = cmd.split_once('.') {
                    if let Ok(target_id) = parse_form_id(target_str) {
                        let mut sub_parts = vec![sub_cmd];
                        sub_parts.extend_from_slice(&parts[1..]);
                        let sub_line = sub_parts.join(" ");
                        return self.execute_statement(&sub_line, Some(target_id));
                    }
                }
            }
        }

        Ok(())
    }

    /// GECK スクリプトの単純な二項比較条件式 (例: `bLocked == 1`, `timer > 0`) を評価する。
    pub fn eval_condition_str(&self, expr: &str) -> bool {
        // 前後のカッコや空白を除去
        let clean = expr.trim().trim_matches(|c| c == '(' || c == ')').trim();
        if clean.is_empty() {
            return true;
        }

        // 演算子の検出 (順序重要: ==, !=, <=, >= を優先)
        let op_opt = if let Some(pos) = clean.find("==") {
            Some((pos, 2, "=="))
        } else if let Some(pos) = clean.find("!=") {
            Some((pos, 2, "!="))
        } else if let Some(pos) = clean.find("<=") {
            Some((pos, 2, "<="))
        } else if let Some(pos) = clean.find(">=") {
            Some((pos, 2, ">="))
        } else if let Some(pos) = clean.find('<') {
            Some((pos, 1, "<"))
        } else if let Some(pos) = clean.find('>') {
            Some((pos, 1, ">"))
        } else {
            None
        };

        if let Some((pos, len, op)) = op_opt {
            let left_str = clean[..pos].trim();
            let right_str = clean[pos + len..].trim();

            let left_val = self.resolve_value(left_str);
            let right_val = self.resolve_value(right_str);

            match op {
                "==" => (left_val - right_val).abs() < 1e-4,
                "!=" => (left_val - right_val).abs() >= 1e-4,
                "<" => left_val < right_val,
                ">" => left_val > right_val,
                "<=" => left_val <= right_val + 1e-4,
                ">=" => left_val >= right_val - 1e-4,
                _ => true,
            }
        } else {
            // 演算子なし (単一変数または数値): 0 でなければ true
            self.resolve_value(clean).abs() > 1e-4
        }
    }

    /// 変数名、関数式、または数値リテラルから float 値を解決する。
    fn resolve_value(&self, token: &str) -> f32 {
        let trimmed = token.trim();
        // 数値リテラル
        if let Ok(v) = trimmed.parse::<f32>() {
            return v;
        }
        // ローカル変数 (大文字小文字不問)
        let lower = trimmed.to_ascii_lowercase();
        if let Some(&v) = self.locals.get(&lower) {
            return v;
        }
        // グローバル変数
        if let Some(&v) = self.globals.get(trimmed) {
            return v;
        }
        0.0
    }

    /// 複数行のスクリプトブロック (If / ElseIf / Else / EndIf / Return / Activate 対応) を実行する。
    /// 戻り値: スクリプト内で明示的に `Activate` 命令が実行されたかどうか
    pub fn execute_block(&mut self, lines: &[String], self_id: Option<FormId>) -> Result<bool, ScriptError> {
        let mut activated = false;
        // if スタック: (現在の分岐が実行中か, すでにこの if 系列のいずれかの分岐が実行されたか)
        let mut if_stack: Vec<(bool, bool)> = Vec::new();

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with(';') {
                continue;
            }

            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            let first = parts[0].to_ascii_lowercase();

            // 制御フロー構文の処理
            if first == "if" {
                let cond_str = if parts.len() > 1 {
                    trimmed[parts[0].len()..].trim()
                } else {
                    ""
                };

                let parent_active = if_stack.last().map(|&(active, _)| active).unwrap_or(true);
                if parent_active {
                    let cond = self.eval_condition_str(cond_str);
                    if_stack.push((cond, cond));
                } else {
                    if_stack.push((false, true)); // 親が無効なら自身も無効
                }
                continue;
            } else if first == "elseif" {
                let cond_str = if parts.len() > 1 {
                    trimmed[parts[0].len()..].trim()
                } else {
                    ""
                };

                let len = if_stack.len();
                let parent_active = if len > 1 { if_stack[len - 2].0 } else { true };
                if let Some((ref mut active, ref mut matched)) = if_stack.last_mut() {
                    if parent_active && !*matched {
                        let cond = self.eval_condition_str(cond_str);
                        *active = cond;
                        if cond {
                            *matched = true;
                        }
                    } else {
                        *active = false;
                    }
                }
                continue;
            } else if first == "else" {
                let len = if_stack.len();
                let parent_active = if len > 1 { if_stack[len - 2].0 } else { true };
                if let Some((ref mut active, ref mut matched)) = if_stack.last_mut() {
                    if parent_active && !*matched {
                        *active = true;
                        *matched = true;
                    } else {
                        *active = false;
                    }
                }
                continue;
            } else if first == "endif" {
                if_stack.pop();
                continue;
            }

            // 現在のブロックが有効（実行対象）か判定
            let is_active = if_stack.last().map(|&(active, _)| active).unwrap_or(true);
            if !is_active {
                continue;
            }

            // Return 命令: ブロックの残りの実行を直ちに中断
            if first == "return" {
                break;
            }

            // Activate 命令の検出
            if first == "activate" {
                activated = true;
                continue;
            }

            // 通常ステートメント実行
            self.execute_statement(trimmed, self_id)?;
        }

        Ok(activated)
    }

    /// 会話やターミナルの Result Script (テキスト) を実行。
    pub fn execute_result_script(&mut self, script: &str, self_id: Option<FormId>) -> Result<(), ScriptError> {
        let lines: Vec<String> = script.lines().map(|s| s.to_string()).collect();
        self.execute_block(&lines, self_id)?;
        Ok(())
    }

    /// コンパイル済みバイトコード (`SCDA`) をデコードして実行。
    ///
    /// 参照元: `references/bevyout/src/vsa/scripts/record.rs`
    pub fn execute_bytecode(&mut self, bytecode: &[u8], self_id: Option<FormId>) -> Result<(), ScriptError> {
        let mut pc = 0;
        while pc + 2 <= bytecode.len() {
            let op = u16::from_le_bytes(bytecode[pc..pc + 2].try_into().unwrap());
            pc += 2;

            match op {
                FN_SET_STAGE => {
                    if pc + 8 <= bytecode.len() {
                        let q_id = FormId(u32::from_le_bytes(bytecode[pc..pc + 4].try_into().unwrap()));
                        let stage = u32::from_le_bytes(bytecode[pc + 4..pc + 8].try_into().unwrap());
                        self.set_stage(q_id, stage);
                        pc += 8;
                    }
                }
                FN_ADD_ITEM => {
                    if pc + 8 <= bytecode.len() {
                        let item_id = FormId(u32::from_le_bytes(bytecode[pc..pc + 4].try_into().unwrap()));
                        let count = u32::from_le_bytes(bytecode[pc + 4..pc + 8].try_into().unwrap());
                        self.add_item(item_id, count);
                        pc += 8;
                    }
                }
                FN_UNLOCK => {
                    if let Some(id) = self_id {
                        self.unlock(id);
                    }
                }
                FN_ENABLE => {
                    if let Some(id) = self_id {
                        self.enabled_objects.insert(id);
                        self.disabled_objects.remove(&id);
                    }
                }
                FN_DISABLE => {
                    if let Some(id) = self_id {
                        self.disabled_objects.insert(id);
                        self.enabled_objects.remove(&id);
                    }
                }
                _ => {
                    // 他の命令はスキップ
                    break;
                }
            }
        }
        Ok(())
    }
}

/// 文字列から FormID をパース (0x16進数または10進数)。
fn parse_form_id(s: &str) -> Result<FormId, ScriptError> {
    let clean = s.trim();
    if let Some(hex) = clean.strip_prefix("0x").or_else(|| clean.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16)
            .map(FormId)
            .map_err(|e| ScriptError::ParseError(e.to_string()))
    } else {
        clean.parse::<u32>()
            .map(FormId)
            .map_err(|e| ScriptError::ParseError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_vm_statement_execution() {
        let mut vm = ScriptVm::new();
        let quest_id = FormId(0x00014E89);
        let item_id = FormId(0x0000000F); // Caps

        // SetStage テスト
        vm.execute_statement("SetStage 0x00014E89 10", None).unwrap();
        assert_eq!(vm.get_stage(quest_id), 10);

        // AddItem テスト
        vm.execute_statement("player.additem 0x0000000F 100", None).unwrap();
        assert_eq!(vm.get_item_count(item_id), 100);

        // Unlock テスト
        let door_id = FormId(0x00035DA9);
        vm.execute_statement("Unlock", Some(door_id)).unwrap();
        assert!(vm.is_unlocked(door_id));

        // Result Script テスト
        let script = "SetStage 0x00014E89 20\nplayer.additem 0x0000000F 50";
        vm.execute_result_script(script, None).unwrap();
        assert_eq!(vm.get_stage(quest_id), 20);
        assert_eq!(vm.get_item_count(item_id), 150);

        // EditorID 逆引き & SetObjectiveDisplayed テスト
        vm.edid_map.insert("MQ01".to_string(), quest_id);
        vm.execute_statement("SetStage MQ01 30", None).unwrap();
        assert_eq!(vm.get_stage(quest_id), 30);
        assert!(vm.quest_manager.get_stage_done(quest_id, 30));

        vm.execute_statement("SetObjectiveDisplayed MQ01 10 1", None).unwrap();
        assert!(vm.quest_manager.is_objective_displayed(quest_id, 10));
    }
}
