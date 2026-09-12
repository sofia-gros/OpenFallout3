//! # 会話ダイアログ (Dialog UI) 状態管理モジュール
//!
//! Fallout 3 の NPC 会話メニュー状態マシン。
//! NPC のセリフ発言、プレイヤーの選択肢一覧、選択カーソル移動、および会話終了 (Goodbye) を管理する。
//!
//! 参照元:
//! - `menus/dialog/dialog_menu.xml`
//! - `references/openmw/components/esm4/loaddial.hpp`, `loadinfo.hpp`
//! - Gamebryo 2.6 `NiPick` → NPC インタラクト会話遷移

/// プレイヤーが選択可能な会話選択肢（トピック）。
#[derive(Clone, Debug, PartialEq)]
pub struct DialogChoice {
    /// 選択肢プロンプト表示テキスト (例: "あなたの父について教えてくれ")
    pub prompt: String,
    /// 選択時に NPC が返す応答セリフテキスト
    pub response: String,
    /// 選択時に会話を終了するか (Goodbye フラグ)
    pub is_goodbye: bool,
}

/// 会話ダイアログの実行状態。
#[derive(Clone, Debug)]
pub struct DialogState {
    /// 話しかけている NPC の表示名
    #[allow(dead_code)]
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
            },
            DialogChoice {
                prompt: "メガトンについて教えてくれ。".to_string(),
                response: "不発弾の周りにできた街さ。".to_string(),
                is_goodbye: false,
            },
        ];

        let mut dialog = DialogState::new("Colin Moriarty", "何か用か？", choices);
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
    }
}
