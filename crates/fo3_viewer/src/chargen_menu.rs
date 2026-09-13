//! # キャラクター作成および実機ダイアログメニュー (Chargen & Message UI)
//!
//! 参照元:
//! - Fallout 3 実機メニュー: `menus\race_sex_menu.xml`, `menus\text_input_menu.xml`, `menus\message_menu.xml`
//! - GECK スクリプトコマンド: `ShowRaceMenu`, `GetPlayerName`, `ShowMessage`
//! - 人種仕様: Caucasian, African American, Asian, Hispanic

use winit::keyboard::KeyCode;
use fo3_render::{TextBatch, UiRenderer};
use fo3_script::ScriptVm;

/// キャラクター作成メニューの状態
#[derive(Clone, Debug, PartialEq)]
pub enum ChargenMode {
    /// メニュー非表示
    None,
    /// 名前入力ダイアログ (`GetPlayerName`)
    NameInput {
        name: String,
    },
    /// 人種・性別・容姿カスタマイズ (`ShowRaceMenu`)
    RaceSex {
        /// 現在選択中の項目インデックス (0: Sex, 1: Race, 2: Hair, 3: [DONE])
        cursor: usize,
        is_female: bool,
        race_index: usize,
        hair_index: usize,
    },
}

pub struct ChargenMenu {
    pub mode: ChargenMode,
    pub selected_button: usize,
}

impl Default for ChargenMenu {
    fn default() -> Self {
        Self {
            mode: ChargenMode::None,
            selected_button: 0,
        }
    }
}

impl ChargenMenu {
    pub fn new() -> Self {
        Self::default()
    }

    /// メニューが開いているか判定。
    pub fn is_active(&self) -> bool {
        self.mode != ChargenMode::None
    }

    /// スクリプトイベントキュー (`chargen_events`) をポーリングしてメニューを開始する。
    pub fn poll_events(&mut self, vm: &mut ScriptVm) {
        while !vm.chargen_events.is_empty() {
            let evt = vm.chargen_events.remove(0);
            match evt.as_str() {
                "GetPlayerName" => {
                    let default_name = if vm.player_is_female { "Catherine" } else { "James" };
                    self.mode = ChargenMode::NameInput {
                        name: default_name.to_string(),
                    };
                    println!("[ChargenMenu] 名前入力ダイアログ開始: default=\"{}\"", default_name);
                }
                "ShowRaceMenu" => {
                    self.mode = ChargenMode::RaceSex {
                        cursor: 0,
                        is_female: vm.player_is_female,
                        race_index: 0,
                        hair_index: 0,
                    };
                    println!("[ChargenMenu] キャラメイク画面 (RaceSexMenu) 開始");
                }
                _ => {}
            }
        }
    }

    /// キーボード入力を処理する。
    pub fn handle_key(&mut self, key: KeyCode, vm: &mut ScriptVm) -> bool {
        match &mut self.mode {
            ChargenMode::None => false,
            ChargenMode::NameInput { name } => {
                match key {
                    KeyCode::Backspace => {
                        name.pop();
                        true
                    }
                    KeyCode::Enter | KeyCode::NumpadEnter => {
                        println!("[ChargenMenu] プレイヤー名決定: \"{}\"", name);
                        // 名前決定後、Stage 42 へ進行
                        let cg00_id = fo3_esm::FormId(0x0001F388);
                        vm.set_stage(cg00_id, 42);
                        self.mode = ChargenMode::None;
                        true
                    }
                    _ => false,
                }
            }
            ChargenMode::RaceSex { cursor, is_female, race_index, hair_index } => {
                const RACES: &[&str] = &["Caucasian", "African American", "Asian", "Hispanic"];
                const HAIRS: &[&str] = &["Default", "Wasteland", "Clean Cut", "Rough", "Ponytail"];

                match key {
                    KeyCode::KeyW | KeyCode::ArrowUp => {
                        if *cursor > 0 { *cursor -= 1; } else { *cursor = 3; }
                        true
                    }
                    KeyCode::KeyS | KeyCode::ArrowDown => {
                        if *cursor < 3 { *cursor += 1; } else { *cursor = 0; }
                        true
                    }
                    KeyCode::KeyA | KeyCode::ArrowLeft => {
                        match *cursor {
                            0 => *is_female = !*is_female,
                            1 => {
                                if *race_index > 0 { *race_index -= 1; } else { *race_index = RACES.len() - 1; }
                            }
                            2 => {
                                if *hair_index > 0 { *hair_index -= 1; } else { *hair_index = HAIRS.len() - 1; }
                            }
                            _ => {}
                        }
                        true
                    }
                    KeyCode::KeyD | KeyCode::ArrowRight => {
                        match *cursor {
                            0 => *is_female = !*is_female,
                            1 => {
                                *race_index = (*race_index + 1) % RACES.len();
                            }
                            2 => {
                                *hair_index = (*hair_index + 1) % HAIRS.len();
                            }
                            _ => {}
                        }
                        true
                    }
                    KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => {
                        if *cursor == 3 {
                            println!("[ChargenMenu] キャラメイク完了: sex={}, race={}, hair={}",
                                if *is_female { "Female" } else { "Male" },
                                RACES[*race_index],
                                HAIRS[*hair_index],
                            );
                            vm.player_is_female = *is_female;
                            // キャラメイク完了後、Stage 62 -> 65 へ進行
                            let cg00_id = fo3_esm::FormId(0x0001F388);
                            vm.set_stage(cg00_id, 62);
                            let _ = vm.execute_statement("player.addScriptPackage CG00PlayerSection4", None);
                            vm.set_stage(cg00_id, 65);
                            let _ = vm.execute_statement("setstage CG00 80", None);
                            self.mode = ChargenMode::None;
                        } else {
                            // 項目送り
                            match *cursor {
                                0 => *is_female = !*is_female,
                                1 => *race_index = (*race_index + 1) % RACES.len(),
                                2 => *hair_index = (*hair_index + 1) % HAIRS.len(),
                                _ => {}
                            }
                        }
                        true
                    }
                    _ => false,
                }
            }
        }
    }

    /// 文字入力を処理する (テキストフィールド用)。
    pub fn handle_char(&mut self, c: char) -> bool {
        if let ChargenMode::NameInput { name } = &mut self.mode {
            if !c.is_control() && name.len() < 24 {
                name.push(c);
                return true;
            }
        }
        false
    }

    /// Pip-Boy 実機スタイルのメニュー画面を描画バッチに追加する。
    pub fn render(
        &self,
        ui_renderer: &UiRenderer,
        batch: &mut TextBatch,
        screen_w: f32,
        screen_h: f32,
    ) {
        match &self.mode {
            ChargenMode::None => {}
            ChargenMode::NameInput { name } => {
                let box_w = 480.0;
                let box_h = 220.0;
                let bx = (screen_w - box_w) * 0.5;
                let by = (screen_h - box_h) * 0.5;

                // 背景パネル (暗緑色半透明)
                batch.add_rect(bx, by, box_w, box_h, [0.02, 0.08, 0.03, 0.88]);
                // 外枠 (Pip-Boy グリーン)
                batch.add_rect(bx, by, box_w, 2.0, [0.2, 1.0, 0.4, 1.0]);
                batch.add_rect(bx, by + box_h - 2.0, box_w, 2.0, [0.2, 1.0, 0.4, 1.0]);
                batch.add_rect(bx, by, 2.0, box_h, [0.2, 1.0, 0.4, 1.0]);
                batch.add_rect(bx + box_w - 2.0, by, 2.0, box_h, [0.2, 1.0, 0.4, 1.0]);

                // タイトル
                batch.add_text(
                    ui_renderer.font(),
                    "VAULT-TEC CITIZEN REGISTRATION",
                    bx + 25.0,
                    by + 25.0,
                    1.2,
                    [0.2, 1.0, 0.4, 1.0],
                );
                batch.add_text(
                    ui_renderer.font(),
                    "ENTER NAME FOR YOUR BABY:",
                    bx + 25.0,
                    by + 65.0,
                    1.0,
                    [0.8, 1.0, 0.8, 0.9],
                );

                // テキスト入力枠
                let field_x = bx + 25.0;
                let field_y = by + 95.0;
                let field_w = box_w - 50.0;
                let field_h = 36.0;
                batch.add_rect(field_x, field_y, field_w, field_h, [0.05, 0.18, 0.08, 0.95]);
                batch.add_rect(field_x, field_y, field_w, 1.0, [0.3, 1.0, 0.5, 1.0]);
                batch.add_rect(field_x, field_y + field_h - 1.0, field_w, 1.0, [0.3, 1.0, 0.5, 1.0]);

                let display_text = format!("{}_", name);
                batch.add_text(
                    ui_renderer.font(),
                    &display_text,
                    field_x + 10.0,
                    field_y + 8.0,
                    1.25,
                    [0.2, 1.0, 0.4, 1.0],
                );

                // 決定プロンプト
                batch.add_text(
                    ui_renderer.font(),
                    "[PRESS ENTER TO CONFIRM]",
                    bx + 120.0,
                    by + 160.0,
                    1.1,
                    [0.3, 1.0, 0.5, 1.0],
                );
            }
            ChargenMode::RaceSex { cursor, is_female, race_index, hair_index } => {
                const RACES: &[&str] = &["Caucasian", "African American", "Asian", "Hispanic"];
                const HAIRS: &[&str] = &["Default", "Wasteland", "Clean Cut", "Rough", "Ponytail"];

                let box_w = 540.0;
                let box_h = 360.0;
                let bx = (screen_w - box_w) * 0.5;
                let by = (screen_h - box_h) * 0.5;

                // 背景パネル
                batch.add_rect(bx, by, box_w, box_h, [0.02, 0.08, 0.03, 0.92]);
                // 外枠
                batch.add_rect(bx, by, box_w, 3.0, [0.2, 1.0, 0.4, 1.0]);
                batch.add_rect(bx, by + box_h - 3.0, box_w, 3.0, [0.2, 1.0, 0.4, 1.0]);
                batch.add_rect(bx, by, 3.0, box_h, [0.2, 1.0, 0.4, 1.0]);
                batch.add_rect(bx + box_w - 3.0, by, 3.0, box_h, [0.2, 1.0, 0.4, 1.0]);

                // タイトル
                batch.add_text(
                    ui_renderer.font(),
                    "GENE PROJECTION SIMULATOR (RACE & SEX)",
                    bx + 30.0,
                    by + 25.0,
                    1.25,
                    [0.2, 1.0, 0.4, 1.0],
                );
                batch.add_rect(bx + 30.0, by + 55.0, box_w - 60.0, 1.0, [0.2, 1.0, 0.4, 0.6]);

                let items = [
                    format!("SEX:         < {} >", if *is_female { "FEMALE" } else { "MALE" }),
                    format!("RACE:        < {} >", RACES[*race_index]),
                    format!("HAIR STYLE:  < {} >", HAIRS[*hair_index]),
                    " [DONE - ACCEPT PROJECTION] ".to_string(),
                ];

                let mut cy = by + 85.0;
                for (idx, text) in items.iter().enumerate() {
                    let is_selected = idx == *cursor;
                    if is_selected {
                        // 反転選択ハイライト背景
                        batch.add_rect(bx + 30.0, cy - 4.0, box_w - 60.0, 32.0, [0.2, 0.8, 0.3, 0.35]);
                        let label = format!("> {}", text);
                        batch.add_text(ui_renderer.font(), &label, bx + 35.0, cy, 1.2, [1.0, 1.0, 0.4, 1.0]);
                    } else {
                        let label = format!("  {}", text);
                        batch.add_text(ui_renderer.font(), &label, bx + 35.0, cy, 1.15, [0.2, 1.0, 0.4, 0.85]);
                    }
                    cy += 48.0;
                }

                // ヘルプ操作説明
                batch.add_text(
                    ui_renderer.font(),
                    "W/S: Select Item    A/D: Change Value    ENTER: Confirm",
                    bx + 40.0,
                    by + box_h - 40.0,
                    0.95,
                    [0.6, 0.9, 0.6, 0.7],
                );
            }
        }
    }
}
