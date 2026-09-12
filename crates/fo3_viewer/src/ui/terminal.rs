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
    TextView {
        title: String,
        body: String,
    },
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
        let welcome_text = record
            .description
            .clone()
            .unwrap_or_else(|| "ROBCO INDUSTRIES (TM) TERMLINK PROTOCOL\nENTER PASSWORD NOW".to_string());

        let mut items = record.menu_items.clone();
        // ログアウト項目がなければ追加
        if !items.iter().any(|it| it.item_text.contains("LOGOUT") || it.item_text.contains("ログアウト")) {
            items.push(TermMenuItem {
                item_text: "ログアウト".to_string(),
                target_form_id: None,
                result_text: None,
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
    pub fn confirm_selection(&mut self) {
        match &self.screen {
            TerminalScreen::Menu => {
                if let Some(item) = self.menu_items.get(self.selected_index) {
                    if item.item_text.contains("ログアウト") || item.item_text.contains("LOGOUT") {
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
            },
            TermMenuItem {
                item_text: "ログアウト".to_string(),
                result_text: None,
                target_form_id: None,
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
        // 項目 0 (売上記録) を決定 -> TextView 画面へ遷移
        term.confirm_selection();
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

        // 下へ移動して "ログアウト" を選択
        term.select_down();
        assert_eq!(term.selected_index, 1);
        term.confirm_selection();
        assert!(term.is_closed);
    }
}
