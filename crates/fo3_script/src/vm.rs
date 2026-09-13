//! # スクリプト仮想マシン (VM) カーネル
//!
//! Fallout 3 実機アセットのスクリプト (`SCPT`) および会話・ターミナルの Result Script を決定論的に実行する。
//! 参照元: `references/bevyout/docs/plans/M7_SCRIPTING_ARCHITECTURE_ROADMAP.md`

use std::collections::{HashMap, HashSet};
use fo3_esm::{EsmMasterContext, FormId, ScptRecord};
use crate::opcodes::functions::*;
use crate::quest::QuestManager;

/// スクリプト実行時エラー。
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptError {
    UnknownCommand(String),
    ParseError(String),
    InvalidArguments(String),
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScriptError::UnknownCommand(cmd) => write!(f, "Unknown command or variable: {}", cmd),
            ScriptError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            ScriptError::InvalidArguments(msg) => write!(f, "Invalid arguments for function: {}", msg),
        }
    }
}

impl std::error::Error for ScriptError {}

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
    /// 実機 ESM スクリプトレコード群 (FormId -> ScptRecord)
    pub scripts: HashMap<FormId, ScptRecord>,
    /// Bink ムービー再生リクエストキュー (再生ファイル名)
    pub play_bink_queue: Vec<String>,
    /// 表示メッセージリクエストキュー
    pub show_messages: Vec<String>,
    /// プレイヤー操作の有効/無効フラグ
    pub player_controls_enabled: bool,
    /// 画面エフェクトスタック (ImageSpaceModifier)
    pub active_imods: Vec<String>,
    /// サウンド再生キュー
    pub sound_queue: Vec<String>,
    /// キャラクター作成イベントキュー (GetPlayerName, ShowRaceMenu)
    pub chargen_events: Vec<String>,
    /// テレポート移動リクエストキュー: (Subject FormID (None は player), Target Marker EDID)
    pub teleport_requests: Vec<(Option<FormId>, String)>,
    /// フレームデルタタイム秒 (GetSecondsPassed 評価用)
    pub delta_time: f32,
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
            scripts: HashMap::new(),
            play_bink_queue: Vec::new(),
            show_messages: Vec::new(),
            player_controls_enabled: true,
            active_imods: Vec::new(),
            sound_queue: Vec::new(),
            chargen_events: Vec::new(),
            teleport_requests: Vec::new(),
            delta_time: 0.016,
        }
    }
}

impl ScriptVm {
    /// 新規 ScriptVm を生成。
    pub fn new() -> Self {
        Self::default()
    }

    /// マスター ESM コンテキストから全定義を一括初期化する。
    pub fn initialize_from_master(&mut self, master: &EsmMasterContext) {
        self.quest_manager.register_all_quests(master.quest_map.clone(), master.quest_edid_map.clone());
        self.scripts = master.script_map.clone();
        for (edid, &form_id) in &master.quest_edid_map {
            self.edid_map.insert(edid.clone(), form_id);
        }
        println!(
            "[ScriptVm] マスター定義同期完了: クエスト {} 件, スクリプト {} 件",
            self.quest_manager.quests.len(),
            self.scripts.len()
        );
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
        } else if let Some(form_id) = self.quest_manager.resolve_quest_id(clean) {
            Ok(form_id)
        } else {
            Err(ScriptError::ParseError(format!("Unknown FormID/EditorID: {}", clean)))
        }
    }

    /// クエストのステージを設定し、ステージに紐づく Result Script (SCTX) を即座に自動実行する。
    pub fn set_stage(&mut self, quest_id: FormId, stage: u32) {
        self.quest_stages.insert(quest_id, stage);
        if let Some(script) = self.quest_manager.set_stage(quest_id, stage as u16, None) {
            let lines: Vec<String> = script.lines().map(|s| s.to_string()).collect();
            if let Err(e) = self.execute_block(&lines, Some(quest_id)) {
                eprintln!("警告: クエスト 0x{:08X} ステージ {} スクリプト実行エラー: {:?}", quest_id.0, stage, e);
            }
        }
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
                // set [var] to [expr...]
                if parts.len() >= 4 && parts[2].eq_ignore_ascii_case("to") {
                    let var_name = parts[1];
                    let expr = parts[3..].join(" ");
                    let val = self.eval_expr(&expr);
                    let lower_var = var_name.to_ascii_lowercase();
                    if self.globals.contains_key(var_name) {
                        self.globals.insert(var_name.to_string(), val);
                    } else {
                        self.locals.insert(lower_var.clone(), val);
                    }
                    if let Some((_, sub)) = lower_var.split_once('.') {
                        self.locals.insert(sub.to_string(), val);
                    } else {
                        self.locals.insert(format!("cg00.{}", lower_var), val);
                    }
                }
            }
            "disableplayercontrols" => {
                self.player_controls_enabled = false;
                println!("[Script] DisablePlayerControls: プレイヤー操作無効化");
            }
            "enableplayercontrols" => {
                self.player_controls_enabled = true;
                println!("[Script] EnablePlayerControls: プレイヤー操作有効化");
            }
            "imod" => {
                if parts.len() >= 2 {
                    let imod_name = parts[1].to_string();
                    println!("[Script] imod (ImageSpaceModifier 適用): {}", imod_name);
                    self.active_imods.push(imod_name);
                }
            }
            "rimod" => {
                if parts.len() >= 2 {
                    let imod_name = parts[1];
                    println!("[Script] rimod (ImageSpaceModifier 解除): {}", imod_name);
                    self.active_imods.retain(|m| !m.eq_ignore_ascii_case(imod_name));
                }
            }
            "playsound" => {
                if parts.len() >= 2 {
                    let sound_id = parts[1].to_string();
                    println!("[Script] PlaySound: {}", sound_id);
                    self.sound_queue.push(sound_id);
                }
            }
            "triggerscreenblood" => {
                if parts.len() >= 2 {
                    println!("[Script] TriggerScreenBlood: {}", parts[1]);
                }
            }
            "getplayername" => {
                println!("[Script] GetPlayerName: プレイヤー名入力ダイアログ要求");
                self.chargen_events.push("GetPlayerName".to_string());
            }
            "showracemenu" => {
                println!("[Script] ShowRaceMenu: プレイヤー容姿・性別カスタマイズ要求");
                self.chargen_events.push("ShowRaceMenu".to_string());
            }
            "addscriptpackage" | "player.addscriptpackage" => {
                if parts.len() >= 2 {
                    println!("[Script] AddScriptPackage: {}", parts[1]);
                }
            }
            "removescriptpackage" | "player.removescriptpackage" => {
                println!("[Script] RemoveScriptPackage");
            }
            "setinchargen" => {
                if parts.len() >= 2 {
                    println!("[Script] SetInCharGen: {}", parts[1]);
                }
            }
            "setpcyoung" | "agerace" | "player.agerace" => {
                println!("[Script] Player Age/Young 変更要求: {:?}", parts);
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
            "setobjectivecompleted" => {
                // SetObjectiveCompleted <QuestID> <ObjectiveIndex> <Flag>
                if parts.len() >= 4 {
                    let q_id = self.resolve_form_id(parts[1])?;
                    let obj_idx = parts[2].parse::<u32>().map_err(|e| ScriptError::ParseError(e.to_string()))?;
                    let obj_text = self.quest_manager.get_objective_text(q_id, obj_idx).unwrap_or("");
                    println!("[QuestManager] クエスト 0x{:08X} 目標 {} (\"{}\") 完了", q_id.0, obj_idx, obj_text);
                    if !obj_text.is_empty() {
                        self.quest_manager.notifications.push(format!("[Objective Completed] {}", obj_text));
                    }
                }
            }
            "showmessage" => {
                if parts.len() >= 2 {
                    let msg_id = parts[1];
                    println!("[Script] ShowMessage: {}", msg_id);
                    self.show_messages.push(msg_id.to_string());
                }
            }
            "playbink" => {
                if parts.len() >= 2 {
                    let bink_file = parts[1..].join(" ").trim_matches('"').to_string();
                    println!("[Script] PlayBink ムービー再生要求: \"{}\"", bink_file);
                    self.play_bink_queue.push(bink_file);
                }
            }
            "moveto" => {
                if parts.len() >= 2 {
                    let target_marker = parts[1].to_string();
                    println!("[Script] MoveTo 要求: subject={:?}, target={}", self_id, target_marker);
                    self.teleport_requests.push((self_id, target_marker));
                }
            }
            "evaluatepackage" | "evp" => {
                println!("[Script] EvaluatePackage (AI パッケージ再評価要求)");
            }
            "say" => {
                if parts.len() >= 2 {
                    println!("[Script] Say (台詞発言要求): topic={:?}", parts[1]);
                }
            }
            "startquest" => {
                if parts.len() >= 2 {
                    let q_id = self.resolve_form_id(parts[1])?;
                    println!("[Script] StartQuest: 0x{:08X}", q_id.0);
                    self.set_stage(q_id, 10);
                }
            }
            "completequest" | "stopquest" => {
                if parts.len() >= 2 {
                    let q_id = self.resolve_form_id(parts[1])?;
                    println!("[Script] CompleteQuest: 0x{:08X}", q_id.0);
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
                // ドット呼び出し対応 (例: player.moveto, TargetRef.Unlock, TargetRef.SetStage)
                if let Some((target_str, sub_cmd)) = cmd.split_once('.') {
                    let target_id = if target_str.eq_ignore_ascii_case("player") {
                        Some(FormId(0x00000014))
                    } else if let Ok(fid) = self.resolve_form_id(target_str) {
                        Some(fid)
                    } else {
                        parse_form_id(target_str).ok()
                    };
                    let mut sub_parts = vec![sub_cmd];
                    sub_parts.extend_from_slice(&parts[1..]);
                    let sub_line = sub_parts.join(" ");
                    return self.execute_statement(&sub_line, target_id);
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

            let left_val = self.eval_expr(left_str);
            let right_val = self.eval_expr(right_str);

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
            self.eval_expr(clean).abs() > 1e-4
        }
    }

    /// 算術式 (加算・減算) または単一トークンを評価して float 値を算出する。
    pub fn eval_expr(&self, expr: &str) -> f32 {
        let clean = expr.trim();
        if let Some((left, right)) = clean.split_once(" - ") {
            return self.eval_expr(left) - self.eval_expr(right);
        }
        if let Some((left, right)) = clean.split_once(" + ") {
            return self.eval_expr(left) + self.eval_expr(right);
        }
        self.resolve_value(clean)
    }

    /// 変数名、関数式、または数値リテラルから float 値を解決する。
    pub fn resolve_value(&self, token: &str) -> f32 {
        let trimmed = token.trim();
        let lower = trimmed.to_ascii_lowercase();

        // 1. GECK 組み込み関数の解決
        if lower == "getsecondspassed" {
            return self.delta_time;
        }
        if lower == "getbuttonpressed" {
            return 0.0; // 性別ダイアログ: 0 = male
        }
        if lower.starts_with("getstage ") {
            let q_str = lower["getstage ".len()..].trim();
            if let Ok(qid) = self.resolve_form_id(q_str) {
                return self.get_stage(qid) as f32;
            }
        }

        // 2. 数値リテラル (小数の先頭ドット .01 に対応)
        let normalized = if trimmed.starts_with('.') {
            format!("0{}", trimmed)
        } else if trimmed.starts_with("-.") {
            format!("-0{}", &trimmed[1..])
        } else {
            trimmed.to_string()
        };
        if let Ok(v) = normalized.parse::<f32>() {
            return v;
        }

        // 3. ローカル変数
        if let Some(&v) = self.locals.get(&lower) {
            return v;
        }
        if let Some((_, sub)) = lower.split_once('.') {
            if let Some(&v) = self.locals.get(sub) {
                return v;
            }
        } else {
            let prefixed = format!("cg00.{}", lower);
            if let Some(&v) = self.locals.get(&prefixed) {
                return v;
            }
        }

        // 4. グローバル変数
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

        // ステージスクリプト自動連鎖実行テスト
        let q_mq01 = FormId(0x00014E87);
        let mut test_quest = fo3_esm::QuestRecord {
            form_id: q_mq01,
            editor_id: "MQ01".to_string(),
            name: "Following in His Footsteps".to_string(),
            ..Default::default()
        };
        test_quest.stages.push(fo3_esm::QuestStage {
            index: 20,
            flags: 0,
            script_source: Some("player.additem 0x0000000F 25\nSetObjectiveDisplayed MQ01 20 1".to_string()),
        });
        test_quest.objectives.push(fo3_esm::QuestObjective {
            index: 20,
            text: "Speak to Colin Moriarty".to_string(),
        });
        vm.quest_manager.register_quest(test_quest);
        vm.edid_map.insert("MQ01".to_string(), q_mq01);

        // SetStage MQ01 20 を実行 -> ステージスクリプトが自動実行されてアイテムと目標が更新されること！
        vm.execute_statement("SetStage MQ01 20", None).unwrap();
        assert_eq!(vm.get_stage(q_mq01), 20);
        assert_eq!(vm.get_item_count(item_id), 175); // 150 + 25
        assert!(vm.quest_manager.is_objective_displayed(q_mq01, 20));
        assert!(vm.quest_manager.notifications.iter().any(|n| n.contains("Speak to Colin Moriarty")));
    }
}
