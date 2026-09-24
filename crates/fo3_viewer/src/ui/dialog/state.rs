//! 会話ダイアログの状態モデルおよび選択・遷移管理。
//!
//! 参照元:
//! - `menus/dialog/dialog_menu.xml`
//! - `references/openmw/components/esm4/loaddial.hpp`, `loadinfo.hpp`

use fo3_esm::FormId;
use fo3_script::ScriptVm;
/// 実機 `menus/dialog/dialog_menu.xml` の `_ShowingText` に対応する画面表示フェーズ。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DialogPhase {
    /// NPC のセリフ発言表示フェーズ (`_ShowingText = true`)
    ShowingSpeech,
    /// プレイヤーのトピック選択肢一覧フェーズ (`_ShowingText = false`)
    ShowingTopics,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DialogChoice {
    /// 選択肢プロンプト表示テキスト (例: "あなたの父について教えてくれ")
    pub prompt: String,
    /// 選択時に NPC が返す応答セリフテキスト
    pub response: String,
    /// 会話を終了する選択肢か (Goodbye フラグ)
    pub is_goodbye: bool,
    /// 選択肢決定時に実行されるスクリプトバイトコード (INFO Result Script)
    pub result_script: Option<String>,
    /// この選択肢が属する INFO レコードの FormID
    pub info_form_id: Option<FormId>,
}

/// 会話ダイアログの実行状態。
///
/// 実機 `menus/dialog/dialog_menu.xml` の階層構造および `_ShowingText` 状態マシンを完全再現。
#[derive(Clone, Debug)]
pub struct DialogState {
    /// 話しかけている NPC の表示名
    pub npc_name: String,
    /// 現在表示されている NPC の発言セリフ
    pub current_speech: String,
    /// プレイヤーの選択肢一覧
    pub choices: Vec<DialogChoice>,
    /// 現在カーソルが当たっている選択肢インデックス
    pub selected_index: usize,
    /// 現在の表示フェーズ (NPCセリフ表示 ⇔ トピック一覧)
    pub phase: DialogPhase,
    /// 直前の選択肢が Goodbye だったか (セリフ表示後に会話終了するためのフラグ)
    pub is_goodbye_pending: bool,
    /// 会話が終了状態（閉じるべき状態）か
    pub is_closed: bool,
    /// 実機 `menus/dialog/dialog_menu.xml` ランタイム (利用可能な場合)
    pub menu_runtime: Option<fo3_render::MenuRuntime>,
}

impl DialogState {
    /// 新しい会話ダイアログ状態を生成する。
    /// 初期状態は NPC の Greeting セリフ表示フェーズ (`DialogPhase::ShowingSpeech`)。
    pub fn new(npc_name: &str, initial_greeting: &str, mut raw_choices: Vec<DialogChoice>) -> Self {
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
            phase: DialogPhase::ShowingSpeech,
            is_goodbye_pending: false,
            is_closed: false,
            menu_runtime: None,
        }
    }

    /// カーソルを上に移動 (W キーまたは上矢印)
    pub fn select_up(&mut self) {
        if self.phase == DialogPhase::ShowingTopics && self.selected_index > 0 {
            self.selected_index -= 1;
        } else if self.phase == DialogPhase::ShowingTopics && !self.choices.is_empty() {
            self.selected_index = self.choices.len() - 1;
        }
    }

    /// カーソルを下に移動 (S キーまたは下矢印)
    pub fn select_down(&mut self) {
        if self.phase == DialogPhase::ShowingTopics && !self.choices.is_empty() {
            if self.selected_index + 1 < self.choices.len() {
                self.selected_index += 1;
            } else {
                self.selected_index = 0;
            }
        }
    }

    /// 次のフェーズへ進める、または選択肢を決定する。
    #[allow(dead_code)]
    pub fn advance(&mut self) -> bool {
        let mut dummy_vm = ScriptVm::new();
        self.advance_with_vm(&mut dummy_vm)
    }

    /// スクリプト VM と連携して次へ進める、またはトピックを決定して Result Script を実行。
    /// 会話が完全に終了した場合は true を返す。
    pub fn advance_with_vm(&mut self, vm: &mut ScriptVm) -> bool {
        match self.phase {
            DialogPhase::ShowingSpeech => {
                if self.is_goodbye_pending {
                    self.is_closed = true;
                    true
                } else {
                    // トピック選択一覧フェーズへ移行
                    self.phase = DialogPhase::ShowingTopics;
                    false
                }
            }
            DialogPhase::ShowingTopics => {
                if let Some(choice) = self.choices.get(self.selected_index) {
                    self.current_speech = choice.response.clone();
                    if let Some(script) = &choice.result_script {
                        let _ = vm.execute_result_script(script, choice.info_form_id);
                    }
                    if choice.is_goodbye {
                        self.is_goodbye_pending = true;
                    }
                    // NPC の応答セリフを表示するフェーズへ遷移
                    self.phase = DialogPhase::ShowingSpeech;
                }
                false
            }
        }
    }

    /// 従来の confirm_selection_with_vm との後方互換インターフェース。
    #[allow(dead_code)]
    pub fn confirm_selection_with_vm(&mut self, vm: &mut ScriptVm) -> bool {
        self.advance_with_vm(vm)
    }

}
