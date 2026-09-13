//! # 会話ダイアログ (Dialog UI) 状態管理モジュール
//!
//! Fallout 3 の NPC 会話メニュー状態マシン。
//! NPC のセリフ発言、プレイヤーの選択肢一覧、選択カーソル移動、および会話終了 (Goodbye) を管理する。
//!
//! 参照元:
//! - `menus/dialog/dialog_menu.xml`
//! - `references/openmw/components/esm4/loaddial.hpp`, `loadinfo.hpp`
//! - Gamebryo 2.6 `NiPick` → NPC インタラクト会話遷移

use fo3_esm::FormId;
use fo3_render::{ui_colors, BitmapFont, TextBatch};
use fo3_script::ScriptVm;

/// プレイヤーが選択可能な会話選択肢（トピック）。
#[derive(Clone, Debug, PartialEq)]
pub struct DialogChoice {
    /// 選択肢プロンプト表示テキスト (例: "あなたの父について教えてくれ")
    pub prompt: String,
    /// 選択時に NPC が返す応答セリフテキスト
    pub response: String,
    /// 選択時に会話を終了するか (Goodbye フラグ)
    pub is_goodbye: bool,
    /// 選択時に実行される Result Script ソース文字列
    pub result_script: Option<String>,
    /// 選択肢に関連する INFO レコード FormID
    pub info_form_id: Option<FormId>,
}

/// 会話ダイアログの実行状態。
#[derive(Clone, Debug)]
pub struct DialogState {
    /// 話しかけている NPC の表示名
    pub npc_name: String,
    /// 現在画面上部に表示されている NPC の発言セリフ
    pub current_speech: String,
    /// プレイヤーの選択肢一覧
    pub choices: Vec<DialogChoice>,
    /// 現在カーソルが当たっている選択肢インデックス
    pub selected_index: usize,
    /// 会話が終了状態（閉じるべき状態）か
    pub is_closed: bool,
}

impl DialogState {
    /// 新しい会話ダイアログ状態を生成する。
    pub fn new(
        npc_name: &str,
        initial_greeting: &str,
        mut raw_choices: Vec<DialogChoice>,
    ) -> Self {
        // 会話終了 ("さようなら" / Goodbye) 選択肢が末尾に無ければ自動付加
        if !raw_choices.iter().any(|c| c.is_goodbye) {
            raw_choices.push(DialogChoice {
                prompt: "さようなら。".to_string(),
                response: "またな。".to_string(),
                is_goodbye: true,
                result_script: None,
                info_form_id: None,
            });
        }

        Self {
            npc_name: npc_name.to_string(),
            current_speech: initial_greeting.to_string(),
            choices: raw_choices,
            selected_index: 0,
            is_closed: false,
        }
    }

    /// カーソルを上に移動 (W キーまたは上矢印)
    pub fn select_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else if !self.choices.is_empty() {
            self.selected_index = self.choices.len() - 1;
        }
    }

    /// カーソルを下に移動 (S キーまたは下矢印)
    pub fn select_down(&mut self) {
        if !self.choices.is_empty() {
            if self.selected_index + 1 < self.choices.len() {
                self.selected_index += 1;
            } else {
                self.selected_index = 0;
            }
        }
    }

    /// 現在の選択肢を決定 (Enter または E キー)
    /// 会話が終了する場合は true を返す。
    #[allow(dead_code)]
    pub fn confirm_selection(&mut self) -> bool {
        if let Some(choice) = self.choices.get(self.selected_index) {
            self.current_speech = choice.response.clone();
            if choice.is_goodbye {
                self.is_closed = true;
                return true;
            }
        }
        false
    }

    /// スクリプト VM と連携して選択肢を決定し、Result Script を実行。
    pub fn confirm_selection_with_vm(&mut self, vm: &mut ScriptVm) -> bool {
        if let Some(choice) = self.choices.get(self.selected_index) {
            self.current_speech = choice.response.clone();
            if let Some(script) = &choice.result_script {
                let _ = vm.execute_result_script(script, choice.info_form_id);
            }
            if choice.is_goodbye {
                self.is_closed = true;
                return true;
            }
        }
        false
    }

    /// 実機 `dialog_menu.xml` 準拠の 2D UI レンダリングバッチを生成。
    ///
    /// 参照元: `menus/dialog/dialog_menu.xml`
    pub fn render_to_batch(&self, batch: &mut TextBatch, font: &BitmapFont, screen_w: f32, screen_h: f32) {
        let margin_x = (screen_w * 0.1).max(20.0);
        let content_w = screen_w - margin_x * 2.0;

        // 1. 画面上部〜中央: NPC 名および応答セリフ (Subtitle ボックス)
        let sub_box_y = (screen_h * 0.15).max(30.0);
        let sub_box_h = 100.0;
        batch.add_rect(margin_x - 10.0, sub_box_y - 10.0, content_w + 20.0, sub_box_h, ui_colors::BG_OVERLAY);

        // NPC 名 (高輝度グリーン/白)
        let name_label = format!("[ {} ]", self.npc_name);
        batch.add_text(font, &name_label, margin_x, sub_box_y, 1.2, ui_colors::HIGHLIGHT_WHITE);

        // セリフ本文
        batch.add_text(font, &self.current_speech, margin_x, sub_box_y + 30.0, 1.1, ui_colors::PIPBOY_GREEN);

        // 2. 画面下部: トピック選択肢一覧
        let list_box_y = screen_h * 0.60;
        let list_box_h = screen_h * 0.35;
        batch.add_rect(margin_x - 10.0, list_box_y - 10.0, content_w + 20.0, list_box_h, ui_colors::BG_OVERLAY);

        let item_height = 24.0;
        for (i, choice) in self.choices.iter().enumerate() {
            let item_y = list_box_y + i as f32 * item_height;
            if item_y + item_height > list_box_y + list_box_h {
                break; // 画面外オーバーフロー防止
            }

            let is_selected = i == self.selected_index;
            if is_selected {
                // 選択中背景ハイライトバー
                batch.add_rect(margin_x - 5.0, item_y - 2.0, content_w + 10.0, item_height, [0.1, 0.4, 0.2, 0.6]);
                let label = format!("> {}", choice.prompt);
                batch.add_text(font, &label, margin_x, item_y, 1.0, ui_colors::HIGHLIGHT_WHITE);
            } else {
                let label = format!("  {}", choice.prompt);
                batch.add_text(font, &label, margin_x, item_y, 1.0, ui_colors::PIPBOY_GREEN);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dialog_navigation_and_selection() {
        let choices = vec![
            DialogChoice {
                prompt: "父親を探している。".to_string(),
                response: "ジェームズのことか？".to_string(),
                is_goodbye: false,
                result_script: Some("player.additem 0x0000000F 10".to_string()),
                info_form_id: Some(FormId(0x00012345)),
            },
            DialogChoice {
                prompt: "メガトンについて教えてくれ。".to_string(),
                response: "不発弾の周りにできた街さ。".to_string(),
                is_goodbye: false,
                result_script: None,
                info_form_id: None,
            },
        ];

        let mut dialog = DialogState::new("Colin Moriarty", "何か用か？", choices.clone());
        // 自動で末尾に Goodbye が追加されて合計 3 選択肢
        assert_eq!(dialog.choices.len(), 3);
        assert_eq!(dialog.selected_index, 0);

        // 下移動
        dialog.select_down();
        assert_eq!(dialog.selected_index, 1);
        dialog.select_down();
        assert_eq!(dialog.selected_index, 2);
        // ループして先頭へ
        dialog.select_down();
        assert_eq!(dialog.selected_index, 0);

        // 上移動で末尾へループ
        dialog.select_up();
        assert_eq!(dialog.selected_index, 2);

        // Goodbye の決定
        let finished = dialog.confirm_selection();
        assert!(finished);
        assert!(dialog.is_closed);
        assert_eq!(dialog.current_speech, "またな。");

        // 2. VM 連携と Result Script 実行
        let mut dialog2 = DialogState::new("Colin Moriarty", "何か用か？", choices);
        let mut vm = ScriptVm::new();
        dialog2.confirm_selection_with_vm(&mut vm);
        assert_eq!(vm.get_item_count(FormId(0x0000000F)), 10);

        // 3. UI レンダリングバッチ生成
        let font = BitmapFont::create_embedded_fallback();
        let mut batch = TextBatch::default();
        dialog2.render_to_batch(&mut batch, &font, 1280.0, 720.0);
        assert!(!batch.vertices.is_empty());
        assert!(!batch.indices.is_empty());
    }
}
