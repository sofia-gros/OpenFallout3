//! # スクリプト仮想マシン (VM) カーネル
//!
//! Fallout 3 実機アセットのスクリプト (`SCPT`) および会話・ターミナルの Result Script を決定論的に実行する。
//! 参照元: `references/bevyout/docs/plans/M7_SCRIPTING_ARCHITECTURE_ROADMAP.md`

use crate::opcodes::functions::*;
use crate::quest::QuestManager;
use fo3_esm::{EsmMasterContext, FormId, ScptRecord};
use std::collections::{HashMap, HashSet};

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
            ScriptError::InvalidArguments(msg) => {
                write!(f, "Invalid arguments for function: {}", msg)
            }
        }
    }
}

impl std::error::Error for ScriptError {}

/// プレイヤー操作の有効/無効フラグ (Gamebryo / GECK DisablePlayerControls 仕様)
/// 参照元: GECK Wiki `DisablePlayerControls`, `EnablePlayerControls`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerControlFlags {
    /// 移動操作 (WASD / スティック)
    pub movement: bool,
    /// 視点回転操作 (マウス移動 / 右スティック)
    pub looking: bool,
    /// Pip-Boy 開閉
    pub pipboy: bool,
    /// 攻撃・武器構え・戦闘
    pub fight: bool,
    /// 1人称/3人称 視点切り替え (マウスホイール等)
    pub pov: bool,
    /// カメラ切り替え
    pub cam_switch: bool,
}

impl Default for PlayerControlFlags {
    fn default() -> Self {
        Self {
            movement: true,
            looking: true,
            pipboy: true,
            fight: true,
            pov: true,
            cam_switch: true,
        }
    }
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
    /// 実機 ESM スクリプトレコード群 (FormId -> ScptRecord)
    pub scripts: HashMap<FormId, ScptRecord>,
    /// Bink ムービー再生リクエストキュー (再生ファイル名)
    pub play_bink_queue: Vec<String>,
    /// 表示メッセージリクエストキュー
    pub show_messages: Vec<String>,
    /// プレイヤー操作の個別フラグ (GECK DisablePlayerControls 仕様)
    pub player_controls: PlayerControlFlags,
    /// プレイヤー操作の有効/無効フラグ (互換用)
    pub player_controls_enabled: bool,
    /// キャラクター作成中フラグ (SetInCharGen)
    pub in_chargen: bool,
    /// 最後に押されたメッセージボタン番号 (GetButtonPressed で取得・消費)
    pub last_button_pressed: std::cell::Cell<Option<i32>>,
    /// 画面エフェクトスタック (ImageSpaceModifier)
    pub active_imods: Vec<String>,
    /// サウンド再生キュー
    pub sound_queue: Vec<String>,
    /// キャラクター作成イベントキュー (GetPlayerName, ShowRaceMenu)
    pub chargen_events: Vec<String>,
    /// テレポート移動リクエストキュー: (Subject FormID (None は player), Target Marker EDID)
    pub teleport_requests: Vec<(Option<FormId>, String)>,
    pub chargen_menu_active: bool,
    pub playgroup_queue: Vec<(FormId, String)>,
    /// 台詞発言リクエストキュー: (Speaker FormID, Topic EDID)
    pub say_queue: Vec<(Option<FormId>, String)>,
    /// スクリプトパッケージ追加リクエストキュー: (Subject FormID, Package EDID)
    pub script_package_requests: Vec<(Option<FormId>, String)>,
    /// AIパッケージ再評価リクエストキュー: (Subject FormID)
    pub evaluate_package_requests: Vec<Option<FormId>>,
    /// プレイヤーの性別 (true: Female, false: Male)
    pub player_is_female: bool,
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
            player_controls: PlayerControlFlags::default(),
            player_controls_enabled: true,
            in_chargen: false,
            last_button_pressed: std::cell::Cell::new(None),
            active_imods: Vec::new(),
            sound_queue: Vec::new(),
            chargen_events: Vec::new(),
            teleport_requests: Vec::new(),
            chargen_menu_active: false,
            playgroup_queue: Vec::new(),
            say_queue: Vec::new(),
            script_package_requests: Vec::new(),
            evaluate_package_requests: Vec::new(),
            player_is_female: false,
            delta_time: 0.016,
        }
    }
}

impl ScriptVm {
    /// メッセージダイアログ等のボタン押下結果を設定する (GetButtonPressed で取得・消費される)。
    pub fn set_button_pressed(&self, button: i32) {
        self.last_button_pressed.set(Some(button));
    }
    /// 新規 ScriptVm を生成。
    pub fn new() -> Self {
        Self::default()
    }

    /// マスター ESM コンテキストから全定義を一括初期化する。
    pub fn initialize_from_master(&mut self, master: &EsmMasterContext) {
        self.quest_manager
            .register_all_quests(master.quest_map.clone(), master.quest_edid_map.clone());
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
        if clean.eq_ignore_ascii_case("player") || clean.eq_ignore_ascii_case("playerref") {
            return Ok(FormId(0x14));
        }
        if let Some(hex) = clean
            .strip_prefix("0x")
            .or_else(|| clean.strip_prefix("0X"))
        {
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
            Err(ScriptError::ParseError(format!(
                "Unknown FormID/EditorID: {}",
                clean
            )))
        }
    }

    /// クエストのステージを設定し、ステージに紐づく Result Script (SCTX) を実行する。
    pub fn set_stage(&mut self, quest_id: FormId, stage: u32) {
        self.quest_stages.insert(quest_id, stage);
        if let Some(script) = self.quest_manager.set_stage(quest_id, stage as u16, None) {
            let lines: Vec<String> = script.lines().map(|s| s.to_string()).collect();
            if let Err(e) = self.execute_block(&lines, Some(quest_id)) {
                eprintln!("[Script] SetStage {} ResultScript 実行エラー: {:?}", stage, e);
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
    /// - `CG00DadREF.evp` (dot 記法: prefix が subject EDID)
    /// - `Unlock`
    pub fn execute_statement(
        &mut self,
        line: &str,
        self_id: Option<FormId>,
    ) -> Result<(), ScriptError> {
        let no_comment = if let Some(pos) = line.find(';') {
            &line[..pos]
        } else {
            line
        };
        let trimmed = no_comment.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        // dot 記法 subject 解決: `CG00DadREF.evp` や `player.additem` などの
        // `<ObjectRef>.<Command> [args...]` 形式を検出し、prefix を subject FormID に解決する。
        // 参照元: GECK スクリプトリファレンス — "Dot Notation (Object Reference)"
        let (effective_self_id, cmd, cmd_parts) = if let Some(dot_pos) = parts[0].find('.') {
            let prefix = &parts[0][..dot_pos];
            let cmd_after_dot = parts[0][dot_pos + 1..].to_lowercase();
            // prefix を EditorID または特殊キーワードとして解決
            let resolved = if prefix.eq_ignore_ascii_case("player") {
                Some(FormId(0x00000014)) // プレイヤーの固定 FormID
            } else if let Some(&fid) = self.edid_map.get(&prefix.to_ascii_uppercase()) {
                Some(fid)
            } else {
                // グローバル変数名などの場合は prefix スキップ (例: CG00.timer は set コマンドで処理)
                self_id
            };
            // コマンド引数を再構築: dot 後のコマンドと残りの parts
            let mut new_parts = vec![parts[0][dot_pos + 1..].as_ref()];
            new_parts.extend_from_slice(&parts[1..]);
            (resolved, cmd_after_dot, new_parts)
        } else {
            (self_id, parts[0].to_lowercase(), parts.to_vec())
        };

        let cmd = cmd.as_str();
        let parts = cmd_parts;
        match cmd {
            "setstage" => {
                if parts.len() >= 3 {
                    let q_id = self.resolve_form_id(parts[1])?;
                    let stage = parts[2]
                        .parse::<u32>()
                        .map_err(|e| ScriptError::ParseError(e.to_string()))?;
                    self.set_stage(q_id, stage);
                }
            }
            "setobjectivedisplayed" => {
                // SetObjectiveDisplayed <QuestID> <ObjectiveIndex> <Flag>
                if parts.len() >= 4 {
                    let q_id = self.resolve_form_id(parts[1])?;
                    let obj_idx = parts[2]
                        .parse::<u32>()
                        .map_err(|e| ScriptError::ParseError(e.to_string()))?;
                    let flag = parts[3].parse::<u32>().unwrap_or(0) != 0;
                    self.quest_manager
                        .set_objective_displayed(q_id, obj_idx, flag);
                }
            }
            "set" => {
                // set [var] to [expr...]
                if parts.len() >= 4 && parts[2].eq_ignore_ascii_case("to") {
                    let var_name = parts[1];
                    let expr = parts[3..].join(" ");
                    let val = self.eval_expr(&expr, self_id);
                    let lower_var = var_name.to_ascii_lowercase();
                    // グローバル変数                    // 変数解決 (case-insensitive 検索)
                    let global_key = self
                        .globals
                        .keys()
                        .find(|k| k.eq_ignore_ascii_case(var_name))
                        .cloned();
                    if let Some(k) = global_key {
                        self.globals.insert(k, val);
                    } else if let Some((prefix, sub)) = lower_var.split_once('.') {
                        // 2. Cross reference set (e.g. CG00.timer)
                        let mut target_q_id = None;
                        if let Some(q_id) = self
                            .edid_map
                            .get(prefix)
                            .or_else(|| self.edid_map.get(&prefix.to_ascii_uppercase()))
                        {
                            if self.quest_manager.quests.contains_key(q_id) {
                                target_q_id = Some(*q_id);
                            }
                        }
                        if let Some(q_id) = target_q_id {
                            self.quest_manager.set_quest_variable(q_id, sub, val as f64);
                        } else {
                            // 参照元のクエストが見つからない場合 (CG00MomREF など)、
                            // locals.clear() で消えないように globals に永続化する
                            let key = format!("{}.{}", prefix, sub);
                            self.globals.insert(key.clone(), val);
                            self.locals.insert(key, val);
                            self.locals.insert(sub.to_string(), val);
                        }
                    } else {
                        // 3. Local variable set
                        if let Some(q_id) = self_id {
                            if self.quest_manager.quests.contains_key(&q_id) {
                                self.quest_manager
                                    .set_quest_variable(q_id, &lower_var, val as f64);
                            }
                        }
                        self.locals.insert(lower_var, val);
                    }
                }
            }
            "disableplayercontrols" => {
                // 参照元: GECK Wiki `DisablePlayerControls [bMovement] [bLooking] [bPipboy] [bFight] [bPOV] [bCamSwitch]`
                if parts.len() == 1 {
                    self.player_controls.movement = false;
                    self.player_controls.looking = false;
                    self.player_controls.pipboy = false;
                    self.player_controls.fight = false;
                    self.player_controls.pov = false;
                    self.player_controls.cam_switch = false;
                } else {
                    if let Some(p) = parts.get(1) {
                        if *p == "1" {
                            self.player_controls.movement = false;
                        }
                    }
                    if let Some(p) = parts.get(2) {
                        if *p == "1" {
                            self.player_controls.looking = false;
                        }
                    }
                    if let Some(p) = parts.get(3) {
                        if *p == "1" {
                            self.player_controls.pipboy = false;
                        }
                    }
                    if let Some(p) = parts.get(4) {
                        if *p == "1" {
                            self.player_controls.fight = false;
                        }
                    }
                    if let Some(p) = parts.get(5) {
                        if *p == "1" {
                            self.player_controls.pov = false;
                        }
                    }
                    if let Some(p) = parts.get(6) {
                        if *p == "1" {
                            self.player_controls.cam_switch = false;
                        }
                    }
                }
                self.player_controls_enabled =
                    self.player_controls.movement && self.player_controls.looking;
                println!(
                    "[Script] DisablePlayerControls: プレイヤー操作無効化 (flags: {:?})",
                    self.player_controls
                );
            }
            "enableplayercontrols" => {
                // 参照元: GECK Wiki `EnablePlayerControls [bMovement] [bLooking] [bPipboy] [bFight] [bPOV] [bCamSwitch]`
                if parts.len() == 1 {
                    self.player_controls = PlayerControlFlags::default();
                } else {
                    if let Some(p) = parts.get(1) {
                        if *p == "1" {
                            self.player_controls.movement = true;
                        }
                    }
                    if let Some(p) = parts.get(2) {
                        if *p == "1" {
                            self.player_controls.looking = true;
                        }
                    }
                    if let Some(p) = parts.get(3) {
                        if *p == "1" {
                            self.player_controls.pipboy = true;
                        }
                    }
                    if let Some(p) = parts.get(4) {
                        if *p == "1" {
                            self.player_controls.fight = true;
                        }
                    }
                    if let Some(p) = parts.get(5) {
                        if *p == "1" {
                            self.player_controls.pov = true;
                        }
                    }
                    if let Some(p) = parts.get(6) {
                        if *p == "1" {
                            self.player_controls.cam_switch = true;
                        }
                    }
                }
                self.player_controls_enabled =
                    self.player_controls.movement && self.player_controls.looking;
                println!(
                    "[Script] EnablePlayerControls: プレイヤー操作有効化 (flags: {:?})",
                    self.player_controls
                );
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
                    self.active_imods
                        .retain(|m| !m.eq_ignore_ascii_case(imod_name));
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
                    let pkg_name = parts[1].to_string();
                    let target = self_id.unwrap_or(FormId(0x00000014));
                    println!(
                        "[Script] AddScriptPackage: {} (target={:?})",
                        pkg_name, target
                    );
                    self.script_package_requests.push((Some(target), pkg_name));
                }
            }
            "removescriptpackage" | "player.removescriptpackage" => {
                println!("[Script] RemoveScriptPackage");
            }
            "setinchargen" => {
                if parts.len() >= 2 {
                    let flag = parts[1] == "1";
                    self.in_chargen = flag;
                    println!("[Script] SetInCharGen: {} (in_chargen={})", parts[1], flag);
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
                    let obj_idx = parts[2]
                        .parse::<u32>()
                        .map_err(|e| ScriptError::ParseError(e.to_string()))?;
                    let obj_text = self
                        .quest_manager
                        .get_objective_text(q_id, obj_idx)
                        .unwrap_or("");
                    println!(
                        "[QuestManager] クエスト 0x{:08X} 目標 {} (\"{}\") 完了",
                        q_id.0, obj_idx, obj_text
                    );
                    if !obj_text.is_empty() {
                        self.quest_manager
                            .notifications
                            .push(format!("[Objective Completed] {}", obj_text));
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
                let bink_file = if let Some(first_quote) = trimmed.find('"') {
                    if let Some(second_quote) = trimmed[first_quote + 1..].find('"') {
                        trimmed[first_quote + 1..first_quote + 1 + second_quote].to_string()
                    } else {
                        trimmed[first_quote + 1..].trim().to_string()
                    }
                } else if parts.len() >= 2 {
                    parts[1].trim_matches('"').to_string()
                } else {
                    String::new()
                };
                if !bink_file.is_empty() {
                    println!("[Script] PlayBink ムービー再生要求: \"{}\"", bink_file);
                    self.play_bink_queue.push(bink_file);
                }
            }
            "moveto" => {
                if parts.len() >= 2 {
                    let target_marker = parts[1].to_string();
                    println!(
                        "[Script] MoveTo 要求: subject={:?}, target={}",
                        effective_self_id, target_marker
                    );
                    self.teleport_requests
                        .push((effective_self_id, target_marker));
                }
            }
            "evaluatepackage" | "evp" => {
                println!(
                    "[Script] EvaluatePackage (AI パッケージ再評価要求): subject={:?}",
                    effective_self_id
                );
                self.evaluate_package_requests.push(effective_self_id);
            }
            "say" => {
                if parts.len() >= 2 {
                    let topic = parts[1].to_string();
                    println!(
                        "[Script] Say (台詞発言要求): topic={:?}, speaker={:?}",
                        topic, effective_self_id
                    );
                    self.say_queue.push((effective_self_id, topic));
                }
            }
            "sayto" => {
                // SayTo <Target> <Topic> [Force]
                // 参照元: GECK Wiki `SayTo` — Target に発話させる (Speaker は self_id)
                if parts.len() >= 3 {
                    let topic = parts[2].to_string();
                    // parts[1] = Target (話しかける相手の FormID/EditorID)
                    // Speaker は自分自身 (self_id) が Target に向かって発話
                    let _target_id = self.resolve_form_id(parts[1]).ok();
                    println!(
                        "[Script] SayTo (台詞発言要求): topic={:?}, speaker={:?}, target_str={:?}",
                        topic, effective_self_id, parts[1]
                    );
                    self.say_queue.push((effective_self_id, topic));
                } else if parts.len() == 2 {
                    let topic = parts[1].to_string();
                    println!(
                        "[Script] SayTo (台詞発言要求): topic={:?}, speaker={:?}",
                        topic, effective_self_id
                    );
                    self.say_queue.push((effective_self_id, topic));
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
    pub fn eval_condition_str(&self, expr: &str, self_id: Option<FormId>) -> bool {
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

            let left_val = self.eval_expr(left_str, self_id);
            let right_val = self.eval_expr(right_str, self_id);

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
            // 単一値 (ブール評価): 0 以外なら true
            self.eval_expr(clean, self_id).abs() > 1e-4
        }
    }

    /// 算術式 (加算・減算) または単一トークンを評価して float 値を算出する。
    pub fn eval_expr(&self, expr: &str, self_id: Option<FormId>) -> f32 {
        let clean = expr.trim();
        if let Some((left, right)) = clean.split_once(" - ") {
            return self.eval_expr(left, self_id) - self.eval_expr(right, self_id);
        }
        if let Some((left, right)) = clean.split_once(" + ") {
            return self.eval_expr(left, self_id) + self.eval_expr(right, self_id);
        }
        self.resolve_value(clean, self_id)
    }

    /// 変数名、関数式、または数値リテラルから float 値を解決する。
    pub fn resolve_value(&self, token: &str, self_id: Option<FormId>) -> f32 {
        let trimmed = token.trim();
        let lower = trimmed.to_ascii_lowercase();

        // 1. GECK 組み込み関数の解決
        if lower == "getsecondspassed" {
            return self.delta_time;
        }
        if lower == "getbuttonpressed" {
            // 参照元: GECK Wiki `GetButtonPressed`
            // ボタンが押されていればインデックス (0, 1, ...) を返し、直後に -1 にリセットされる。未押下は -1。
            return self
                .last_button_pressed
                .take()
                .map(|b| b as f32)
                .unwrap_or(-1.0);
        }
        if lower == "getinchargen" {
            // 参照元: GECK Wiki `GetInCharGen`
            return if self.in_chargen { 1.0 } else { 0.0 };
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

        // 3. ローカル変数 / クエスト変数
        if let Some((prefix, sub)) = lower.split_once('.') {
            // "QuestID.var" の形式
            let mut target_q_id = None;
            if let Some(q_id) = self
                .edid_map
                .get(prefix)
                .or_else(|| self.edid_map.get(&prefix.to_ascii_uppercase()))
            {
                if self.quest_manager.quests.contains_key(q_id) {
                    target_q_id = Some(*q_id);
                }
            }
            if let Some(q_id) = target_q_id {
                if let Some(v) = self.quest_manager.get_quest_variable(q_id, sub) {
                    return v as f32;
                }
            }
            if let Some(&v) = self.locals.get(sub) {
                return v;
            }
        } else {
            // プレフィックスなし変数 (timer等) -> locals を優先確認 (同一ブロック内での更新を反映)
            if let Some(&v) = self.locals.get(&lower) {
                return v;
            }
            if let Some(q_id) = self_id {
                if self.quest_manager.quests.contains_key(&q_id) {
                    if let Some(v) = self.quest_manager.get_quest_variable(q_id, &lower) {
                        return v as f32;
                    }
                }
            }
        }

        // 4. グローバル変数 (case-insensitive 検索)
        // 参照元: GECK Wiki — スクリプト変数は大文字小文字を区別しない
        if let Some((_, &v)) = self
            .globals
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(trimmed))
        {
            return v;
        }

        0.0
    }

    /// 複数行のスクリプトブロック (If / ElseIf / Else / EndIf / Return / Activate 対応) を実行する。
    /// 戻り値: スクリプト内で明示的に `Activate` 命令が実行されたかどうか
    pub fn execute_block(
        &mut self,
        lines: &[String],
        self_id: Option<FormId>,
    ) -> Result<bool, ScriptError> {
        let joined = lines.join("\n");
        let mut parser = crate::parser::Parser::new(&joined);
        let stmts = match parser.parse_statements() {
            Ok(s) => s,
            Err(e) => return Err(ScriptError::ParseError(e)),
        };

        let mut activated = false;
        for stmt in stmts {
            let res = self.execute_ast_statement(&stmt, self_id)?;
            if res.activated {
                activated = true;
            }
            if res.returned {
                break;
            }
        }
        Ok(activated)
    }

    pub fn execute_result_script(
        &mut self,
        script: &str,
        self_id: Option<FormId>,
    ) -> Result<(), ScriptError> {
        let lines: Vec<String> = script.lines().map(|s| s.to_string()).collect();
        self.execute_block(&lines, self_id)?;
        Ok(())
    }

    /// コンパイル済みバイトコード (`SCDA`) をデコードして実行。
    ///
    /// 参照元: `references/bevyout/src/vsa/scripts/record.rs`
    pub fn execute_bytecode(
        &mut self,
        bytecode: &[u8],
        self_id: Option<FormId>,
    ) -> Result<(), ScriptError> {
        let mut pc = 0;
        while pc + 2 <= bytecode.len() {
            let op = u16::from_le_bytes(bytecode[pc..pc + 2].try_into().unwrap());
            pc += 2;

            match op {
                FN_SET_STAGE => {
                    if pc + 8 <= bytecode.len() {
                        let q_id =
                            FormId(u32::from_le_bytes(bytecode[pc..pc + 4].try_into().unwrap()));
                        let stage =
                            u32::from_le_bytes(bytecode[pc + 4..pc + 8].try_into().unwrap());
                        self.set_stage(q_id, stage);
                        pc += 8;
                    }
                }
                FN_ADD_ITEM => {
                    if pc + 8 <= bytecode.len() {
                        let item_id =
                            FormId(u32::from_le_bytes(bytecode[pc..pc + 4].try_into().unwrap()));
                        let count =
                            u32::from_le_bytes(bytecode[pc + 4..pc + 8].try_into().unwrap());
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
    if let Some(hex) = clean
        .strip_prefix("0x")
        .or_else(|| clean.strip_prefix("0X"))
    {
        u32::from_str_radix(hex, 16)
            .map(FormId)
            .map_err(|e| ScriptError::ParseError(e.to_string()))
    } else {
        clean
            .parse::<u32>()
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
        vm.execute_statement("SetStage 0x00014E89 10", None)
            .unwrap();
        assert_eq!(vm.get_stage(quest_id), 10);

        // AddItem テスト
        vm.execute_statement("player.additem 0x0000000F 100", None)
            .unwrap();
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

        vm.execute_statement("SetObjectiveDisplayed MQ01 10 1", None)
            .unwrap();
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
            script_source: Some(
                "player.additem 0x0000000F 25\nSetObjectiveDisplayed MQ01 20 1".to_string(),
            ),
        });
        test_quest.objectives.push(fo3_esm::QuestObjective {
            index: 20,
            text: "Speak to Colin Moriarty".to_string(),
        });
        vm.quest_manager.register_quest(test_quest);
        vm.edid_map.insert("MQ01".to_string(), q_mq01);

        // SetStage MQ01 20 を実行 → Result Script は pending_stage_scripts へ積まれる (遅延実行)
        vm.execute_statement("SetStage MQ01 20", None).unwrap();
        assert_eq!(vm.get_stage(q_mq01), 20);

        // pending_stage_scripts を手動で消化 (app.rs の update() が行う処理を模倣)
        while let Some((_quest_id, _stage, script)) = vm.pending_stage_scripts.pop_front() {
            let lines: Vec<String> = script.lines().map(|s| s.to_string()).collect();
            let _ = vm.execute_block(&lines, Some(q_mq01));
        }

        assert_eq!(vm.get_item_count(item_id), 175); // 150 + 25
        assert!(vm.quest_manager.is_objective_displayed(q_mq01, 20));
        assert!(vm
            .quest_manager
            .notifications
            .iter()
            .any(|n| n.contains("Speak to Colin Moriarty")));
    }
}
