//! # ターミナル画面 (Terminal UI) 状態管理モジュール
//!
//! Fallout 3 のレトロコンピュータ・ターミナル画面状態マシン。
//! タイトル表示、ウェルカムメッセージ、階層メニュー選択、ログ閲覧、およびログアウトを管理する。
//!
//! 参照元:
//! - `menus/computers_menu.xml`
//! - `references/openmw/components/esm4/loadterm.hpp`, `loadterm.cpp`

use fo3_esm::{TermMenuItem, TermRecord};

/// ターミナルの表示画面モード。
#[derive(Clone, Debug, PartialEq)]
pub enum TerminalScreen {
    /// メインメニューまたはサブメニュー一覧
    Menu,
    /// ログまたは記事の本文閲覧画面
    TextView { title: String, body: String },
}

/// ターミナル画面の実行状態。
#[derive(Clone, Debug)]
pub struct TerminalState {
    /// ターミナル表示名
    pub title: String,
    /// ターミナル起動時の歓迎メッセージ
    #[allow(dead_code)]
    pub welcome_text: String,
    /// 現在表示中のメニュー項目一覧
    pub menu_items: Vec<TermMenuItem>,
    /// 現在の選択インデックス
    pub selected_index: usize,
    /// 現在の表示画面
    pub screen: TerminalScreen,
    /// ターミナルが終了（ログアウト）したか
    pub is_closed: bool,
}

impl TerminalState {
    /// TermRecord からターミナル状態を初期化する。
    pub fn from_record(record: &TermRecord) -> Self {
        let title = record
            .full_name
            .clone()
            .unwrap_or_else(|| "ROBCO INDUSTRIES UNIFIED OPERATING SYSTEM".to_string());
        let welcome_text = record.description.clone().unwrap_or_else(|| {
            "ROBCO INDUSTRIES (TM) TERMLINK PROTOCOL\nENTER PASSWORD NOW".to_string()
        });

        let mut items = record.menu_items.clone();
        // ログアウト項目がなければ追加
        if !items
            .iter()
            .any(|it| it.item_text.contains("LOGOUT") || it.item_text.contains("ログアウト"))
        {
            items.push(TermMenuItem {
                item_text: "ログアウト".to_string(),
                target_form_id: None,
                result_text: None,
                result_script_bytecode: None,
                result_script_source: None,
            });
        }

        Self {
            title,
            welcome_text,
            menu_items: items,
            selected_index: 0,
            screen: TerminalScreen::Menu,
            is_closed: false,
        }
    }

    /// カーソルを上に移動 (W キーまたは上矢印)
    pub fn select_up(&mut self) {
        if matches!(self.screen, TerminalScreen::Menu) {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            } else if !self.menu_items.is_empty() {
                self.selected_index = self.menu_items.len() - 1;
            }
        }
    }

    /// カーソルを下に移動 (S キーまたは下矢印)
    pub fn select_down(&mut self) {
        if matches!(self.screen, TerminalScreen::Menu) {
            if !self.menu_items.is_empty() {
                if self.selected_index + 1 < self.menu_items.len() {
                    self.selected_index += 1;
                } else {
                    self.selected_index = 0;
                }
            }
        }
    }

    /// 現在の項目を決定 (Enter または E キー)
    #[allow(dead_code)]
    pub fn confirm_selection(&mut self) {
        match &self.screen {
            TerminalScreen::Menu => {
                if let Some(item) = self.menu_items.get(self.selected_index) {
                    if item.item_text.contains("ログアウト") || item.item_text.contains("LOGOUT")
                    {
                        self.is_closed = true;
                    } else if let Some(ref res) = item.result_text {
                        self.screen = TerminalScreen::TextView {
                            title: item.item_text.clone(),
                            body: res.clone(),
                        };
                    } else {
                        // 結果テキストが未定義の場合でも本文閲覧として開く
                        self.screen = TerminalScreen::TextView {
                            title: item.item_text.clone(),
                            body: "[データなし - アクセス権限承認済]".to_string(),
                        };
                    }
                }
            }
            TerminalScreen::TextView { .. } => {
                // 本文表示画面から決定キーでメニューに戻る
                self.screen = TerminalScreen::Menu;
            }
        }
    }

    /// スクリプト VM と連携して項目を決定。
    pub fn confirm_selection_with_vm(&mut self, vm: &mut fo3_script::ScriptVm) {
        match &self.screen {
            TerminalScreen::Menu => {
                if let Some(item) = self.menu_items.get(self.selected_index) {
                    if let Some(script) = &item.result_script_source {
                        let _ = vm.execute_result_script(script, item.target_form_id);
                    }
                    if item.item_text.contains("ログアウト") || item.item_text.contains("LOGOUT")
                    {
                        self.is_closed = true;
                    } else if let Some(ref res) = item.result_text {
                        self.screen = TerminalScreen::TextView {
                            title: item.item_text.clone(),
                            body: res.clone(),
                        };
                    } else {
                        self.screen = TerminalScreen::TextView {
                            title: item.item_text.clone(),
                            body: "[データなし - アクセス権限承認済]".to_string(),
                        };
                    }
                }
            }
            TerminalScreen::TextView { .. } => {
                self.screen = TerminalScreen::Menu;
            }
        }
    }

    /// 実機 `terminal_menu.xml` 準拠のレトロ CRT 画面レンダリングバッチを生成。
    ///
    /// 参照元: `menus/terminal/terminal_menu.xml`
    pub fn render_to_batch(
        &self,
        batch: &mut fo3_render::TextBatch,
        font: &fo3_render::BitmapFont,
        screen_w: f32,
        screen_h: f32,
    ) {
        use fo3_render::ui_colors;

        // 全面レトロ CRT 背景 (わずかに緑がかった黒)
        batch.add_rect(0.0, 0.0, screen_w, screen_h, [0.01, 0.04, 0.02, 0.95]);

        let pad_x = (screen_w * 0.08).max(30.0);
        let content_w = screen_w - pad_x * 2.0;

        // 1. CRT 上部ヘッダー
        let header_y = 40.0;
        batch.add_text(
            font,
            "ROBCO INDUSTRIES (TM) TERMLINK PROTOCOL",
            pad_x,
            header_y,
            1.2,
            ui_colors::TERMINAL_GREEN,
        );
        batch.add_text(
            font,
            &format!("SYSTEM: {}", self.title),
            pad_x,
            header_y + 25.0,
            1.1,
            ui_colors::TERMINAL_GREEN,
        );

        // 区切り線 (ベタ塗り矩形)
        batch.add_rect(
            pad_x,
            header_y + 55.0,
            content_w,
            2.0,
            ui_colors::TERMINAL_GREEN,
        );

        // 2. 画面コンテンツ
        let body_y = header_y + 75.0;

        match &self.screen {
            TerminalScreen::Menu => {
                // ウェルカムメッセージ
                batch.add_text(
                    font,
                    &self.welcome_text,
                    pad_x,
                    body_y,
                    1.0,
                    ui_colors::MUTED_GREEN,
                );

                // メニュー項目リスト
                let list_y = body_y + 60.0;
                let item_h = 28.0;

                for (i, item) in self.menu_items.iter().enumerate() {
                    let cur_y = list_y + i as f32 * item_h;
                    if cur_y + item_h > screen_h - 60.0 {
                        break;
                    }

                    let is_selected = i == self.selected_index;
                    if is_selected {
                        // 選択中反転ハイライトバー
                        batch.add_rect(
                            pad_x - 5.0,
                            cur_y - 2.0,
                            content_w + 10.0,
                            item_h,
                            [0.1, 0.5, 0.2, 0.8],
                        );
                        let label = format!("> {}", item.item_text);
                        batch.add_text(font, &label, pad_x, cur_y, 1.1, ui_colors::HIGHLIGHT_WHITE);
                    } else {
                        let label = format!("  {}", item.item_text);
                        batch.add_text(font, &label, pad_x, cur_y, 1.1, ui_colors::TERMINAL_GREEN);
                    }
                }
            }
            TerminalScreen::TextView { title, body } => {
                // サブタイトル
                batch.add_text(
                    font,
                    &format!("-- {} --", title),
                    pad_x,
                    body_y,
                    1.2,
                    ui_colors::HIGHLIGHT_WHITE,
                );

                // 本文行
                let mut line_y = body_y + 35.0;
                for line in body.lines() {
                    batch.add_text(font, line, pad_x, line_y, 1.0, ui_colors::TERMINAL_GREEN);
                    line_y += 22.0;
                    if line_y > screen_h - 80.0 {
                        break;
                    }
                }

                // 下部案内
                batch.add_text(
                    font,
                    "[ E / T / Esc : 戻る ]",
                    pad_x,
                    screen_h - 50.0,
                    1.0,
                    ui_colors::MUTED_GREEN,
                );
            }
        }
    }

    /// 前の画面に戻る、またはログアウト (T キーまたは Esc キー)
    pub fn back(&mut self) {
        match self.screen {
            TerminalScreen::TextView { .. } => {
                self.screen = TerminalScreen::Menu;
            }
            TerminalScreen::Menu => {
                self.is_closed = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_navigation_and_subscreen() {
        let items = vec![
            TermMenuItem {
                item_text: "酒場の売上記録".to_string(),
                result_text: Some("今週の売上: 450 キャップ".to_string()),
                target_form_id: None,
                result_script_source: Some("player.additem 0x0000000F 25".to_string()),
                result_script_bytecode: None,
            },
            TermMenuItem {
                item_text: "ログアウト".to_string(),
                result_text: None,
                target_form_id: None,
                result_script_source: None,
                result_script_bytecode: None,
            },
        ];

        let mut term = TerminalState {
            title: "Moriarty's Saloon Terminal".to_string(),
            welcome_text: "ROBCO INDUSTRIES UNIFIED OPERATING SYSTEM".to_string(),
            menu_items: items,
            selected_index: 0,
            screen: TerminalScreen::Menu,
            is_closed: false,
        };

        assert_eq!(term.selected_index, 0);

        // VM 連携と Result Script 実行
        let mut vm = fo3_script::ScriptVm::new();
        term.confirm_selection_with_vm(&mut vm);
        assert_eq!(vm.get_item_count(fo3_esm::FormId(0x0000000F)), 25);

        match &term.screen {
            TerminalScreen::TextView { title, body } => {
                assert_eq!(title, "酒場の売上記録");
                assert_eq!(body, "今週の売上: 450 キャップ");
            }
            _ => panic!("Expected TextView screen"),
        }

        // 戻る (T キーまたは Esc) -> Menu 画面へ復帰
        term.back();
        assert!(matches!(term.screen, TerminalScreen::Menu));
        assert!(!term.is_closed);

        // レンダリングバッチ生成
        let font = fo3_render::BitmapFont::create_embedded_fallback();
        let mut batch = fo3_render::TextBatch::default();
        term.render_to_batch(&mut batch, &font, 1280.0, 720.0);
        assert!(!batch.vertices.is_empty());
        assert!(!batch.indices.is_empty());

        // ログアウト決定
        term.selected_index = 1;
        term.confirm_selection();
        assert!(term.is_closed);
    }
}
