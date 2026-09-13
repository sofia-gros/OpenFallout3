//! # 会話ダイアログ (Dialog UI) 状態管理モジュール
//!
//! Fallout 3 の NPC 会話メニュー状態マシン。
//! NPC のセリフ発言、プレイヤーのトピック選択肢一覧、選択カーソル移動、および会話終了 (Goodbye) を管理する。
//!
//! 参照元:
//! - `menus/dialog/dialog_menu.xml` (`_ShowingText`, `DM_SpeakerNameLabel`, `DM_CenterHeight`, `DM_TextBackground`, `DM_SpeakerText`, `DM_TopicList`)
//! - `references/openmw/components/esm4/loaddial.hpp`, `loadinfo.hpp`
//! - Gamebryo 2.6 `NiPick` → NPC インタラクト会話遷移

use fo3_esm::FormId;
use fo3_render::{ui_colors, BitmapFont, TextBatch, TextureAtlas};
use fo3_script::ScriptVm;

/// 実機 `menus/dialog/dialog_menu.xml` の `_ShowingText` に対応する画面表示フェーズ。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DialogPhase {
    /// NPC のセリフ発言表示フェーズ (`_ShowingText = true`)
    /// - 画面右上: NPC 話者名 (font 7, 右寄せ `[ NOVA ]`)
    /// - 画面下部中央 (Y=80% 基準線): セリフ本文 (font 6, wrapwidth: 960, 背景テクスチャ `Interface\Shared\Background\solid_black.dds`)
    ShowingSpeech,
    /// プレイヤーのトピック選択肢一覧フェーズ (`_ShowingText = false`)
    /// - 画面下部中央: トピック選択肢一覧 (font 6, `list_box.xml`, 幅 1010)
    /// - トピック一覧の上下に実機ブラケット装飾 (`top_bracket.xml`, `bottom_bracket.xml`)
    ShowingTopics,
}

/// プレイヤーが選択可能な会話選択肢（トピック）。
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

    /// 実機 `dialog_menu.xml` 準拠の 2D UI レンダリングバッチを生成。
    ///
    /// 参照元:
    /// - `menus/dialog/dialog_menu.xml` (`DM_SpeakerNameLabel`, `DM_CenterHeight`, `DM_TextBackground`, `DM_SpeakerText`, `DM_TopicList`)
    pub fn render_to_batch(&self, batch: &mut TextBatch, font: &BitmapFont, screen_w: f32, screen_h: f32) {
        let atlas = self.menu_runtime.as_ref().and_then(|rt| rt.atlas.as_ref());

        // 実機 dialog_menu.xml: 画面中央 72% 幅 (最大 900px, 最小 560px)
        let bg_w = (screen_w * 0.72).min(900.0).max(560.0);
        let bg_x = (screen_w - bg_w) * 0.5;
        let center_y = screen_h * 0.80;

        // 1. 画面右上: NPC 話者名ラベル (DM_SpeakerNameLabel, font 7, 実機通り角括弧なし・原名表記)
        let name_label = &self.npc_name;
        let label_w = font.measure_text_width(name_label, 1.15);
        let label_x = (screen_w - label_w - 70.0).max(bg_x);
        let label_y = (screen_h * 0.10).max(35.0);
        batch.add_text(font, name_label, label_x, label_y, 1.15, ui_colors::PIPBOY_GREEN);

        match self.phase {
            DialogPhase::ShowingSpeech => {
                // 2. セリフ表示フェーズ (DM_SpeakerText, font 6)
                // 実機 dialog_menu.xml: セリフ表示時はブラケット非表示 (<visible><not src="io()" trait="_ShowingText"/>...)
                let text_w = font.measure_text_width(&self.current_speech, 1.05);
                let box_w = (text_w + 60.0).min(bg_w);
                let box_x = (screen_w - box_w) * 0.5;
                let box_h = 70.0;
                let box_y = center_y - box_h * 0.5;

                // 背景 (DM_TextBackground: Interface\Shared\Background\solid_black.dds)
                batch.add_rect(box_x, box_y, box_w, box_h, [0.0, 0.0, 0.0, 0.75]);

                // NPC セリフ本文
                let text_x = box_x + 30.0;
                let text_y = box_y + (box_h - 16.0) * 0.5;
                batch.add_text(font, &self.current_speech, text_x, text_y, 1.05, ui_colors::PIPBOY_GREEN);
            }
            DialogPhase::ShowingTopics => {
                self.render_topics_list(batch, font, atlas, bg_x, bg_w, center_y);
            }
        }
    }

    /// トピック選択一覧 (`DM_TopicList` / `list_box.xml` / `box.xml` / `list_box_template.xml`) の描画処理。
    pub fn render_topics_list(
        &self,
        batch: &mut TextBatch,
        font: &BitmapFont,
        atlas: Option<&TextureAtlas>,
        bg_x: f32,
        bg_w: f32,
        center_y: f32,
    ) {
        let item_h = 32.0;
        let max_display = 6.min(self.choices.len());
        let box_h = (max_display as f32 * item_h + 30.0).max(90.0);
        let box_y = center_y - box_h * 0.5;

        // 背景 (DM_TextBackground: Interface\Shared\Background\solid_black.dds)
        batch.add_rect(bg_x, box_y, bg_w, box_h, [0.0, 0.0, 0.0, 0.75]);

        // 上下ブラケット (top_bracket.xml / bottom_bracket.xml 準拠)
        render_dialog_brackets(batch, atlas, bg_x, box_y, box_y + box_h, bg_w, ui_colors::PIPBOY_GREEN);

        // スクロール計算
        let max_visible = 6;
        let start_idx = if self.selected_index >= max_visible {
            self.selected_index - max_visible + 1
        } else {
            0
        };
        let end_idx = (start_idx + max_visible).min(self.choices.len());

        // スクロールインジケーター矢印 (実機 list_box.xml: 左端)
        if start_idx > 0 {
            batch.add_text(font, "^", bg_x + 10.0, box_y + 12.0, 1.2, ui_colors::PIPBOY_GREEN);
        }
        if end_idx < self.choices.len() {
            batch.add_text(font, "v", bg_x + 10.0, box_y + box_h - 22.0, 1.2, ui_colors::PIPBOY_GREEN);
        }

        let list_start_y = box_y + 16.0;
        let content_x = bg_x + 35.0;
        let content_w = bg_w - 55.0;

        for (slot, i) in (start_idx..end_idx).enumerate() {
            let choice = &self.choices[i];
            let item_y = list_start_y + slot as f32 * item_h;
            let is_selected = i == self.selected_index;

            if is_selected {
                // 選択項目ハイライト (実機 list_box.xml -> box.xml: 4辺の緑色枠線 + 極薄フィル)
                let border_thick = 1.5;
                let box_color = [ui_colors::PIPBOY_GREEN[0], ui_colors::PIPBOY_GREEN[1], ui_colors::PIPBOY_GREEN[2], 0.95];
                // top
                batch.add_rect(content_x - 10.0, item_y - 3.0, content_w, border_thick, box_color);
                // bottom
                batch.add_rect(content_x - 10.0, item_y + item_h - 6.0, content_w, border_thick, box_color);
                // left
                batch.add_rect(content_x - 10.0, item_y - 3.0, border_thick, item_h - 3.0, box_color);
                // right
                batch.add_rect(content_x - 10.0 + content_w - border_thick, item_y - 3.0, border_thick, item_h - 3.0, box_color);
                // 極薄半透明フィル (_fill_alpha: 40)
                batch.add_rect(content_x - 10.0, item_y - 3.0, content_w, item_h - 3.0, [0.05, 0.25, 0.12, 0.12]);

                // 選択項目テキスト (鮮やかな実機 Pip-Boy Green, プレフィックスなし)
                let text_color = [0.22, 1.0, 0.45, 1.0];
                batch.add_text(font, &choice.prompt, content_x + 6.0, item_y + 2.0, 1.05, text_color);
            } else {
                // 非選択項目テキスト (実機 _line_alpha: 128 準拠の半透明 Pip-Boy Green)
                let text_color = [0.18, 0.88, 0.38, 0.60];
                batch.add_text(font, &choice.prompt, content_x + 6.0, item_y + 2.0, 1.0, text_color);
            }
        }
    }
}

/// 実機 Fallout 3 の上下ブラケット (`top_bracket.xml` / `bottom_bracket.xml`) を描画する。
/// 完全な長方形ではなく、上下の横線とフェードする縦端線から成る開いたブラケット形状を忠実に再現する。
fn render_dialog_brackets(
    batch: &mut TextBatch,
    atlas: Option<&TextureAtlas>,
    x: f32,
    top_y: f32,
    bottom_y: f32,
    width: f32,
    color: [f32; 4],
) {
    let line_thick = 2.0;
    let bracket_vert_h = 35.0;

    // 1. Top Bracket (top_bracket.xml)
    // MainLine (上端横線)
    batch.add_rect(x, top_y, width, line_thick, color);

    // LeftVert / RightVert (下向きフェードライン: fade_to_bottom.dds)
    if let Some((_atlas_name, sub)) = atlas.and_then(|a| a.lookup("fade_to_bottom.dds")) {
        batch.add_textured_rect(
            x,
            top_y,
            line_thick,
            bracket_vert_h,
            [sub.u_min, sub.v_min, sub.u_max, sub.v_max],
            color,
        );
        batch.add_textured_rect(
            x + width - line_thick,
            top_y,
            line_thick,
            bracket_vert_h,
            [sub.u_min, sub.v_min, sub.u_max, sub.v_max],
            color,
        );
    } else {
        // フォールバック: 上端の垂直バー (決して全高を囲まない)
        batch.add_rect(x, top_y, line_thick, bracket_vert_h * 0.5, color);
        batch.add_rect(x + width - line_thick, top_y, line_thick, bracket_vert_h * 0.5, color);
    }

    // 2. Bottom Bracket (bottom_bracket.xml)
    // MainLine (下端横線)
    batch.add_rect(x, bottom_y - line_thick, width, line_thick, color);

    // LeftVert / RightVert (上向きフェードライン: fade_to_top.dds)
    if let Some((_atlas_name, sub)) = atlas.and_then(|a| a.lookup("fade_to_top.dds")) {
        batch.add_textured_rect(
            x,
            bottom_y - bracket_vert_h,
            line_thick,
            bracket_vert_h,
            [sub.u_min, sub.v_min, sub.u_max, sub.v_max],
            color,
        );
        batch.add_textured_rect(
            x + width - line_thick,
            bottom_y - bracket_vert_h,
            line_thick,
            bracket_vert_h,
            [sub.u_min, sub.v_min, sub.u_max, sub.v_max],
            color,
        );
    } else {
        // フォールバック: 下端の垂直バー (決して全高を囲まない)
        batch.add_rect(x, bottom_y - bracket_vert_h * 0.5, line_thick, bracket_vert_h * 0.5, color);
        batch.add_rect(x + width - line_thick, bottom_y - bracket_vert_h * 0.5, line_thick, bracket_vert_h * 0.5, color);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dialog_two_phase_navigation_and_selection() {
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
        // 初期状態はセリフ表示フェーズ
        assert_eq!(dialog.phase, DialogPhase::ShowingSpeech);
        assert_eq!(dialog.choices.len(), 3); // Goodbye 自動付加で3個
        assert_eq!(dialog.selected_index, 0);

        // 1回目の advance でトピック一覧へ遷移
        let closed = dialog.advance();
        assert!(!closed);
        assert_eq!(dialog.phase, DialogPhase::ShowingTopics);

        // トピック選択: 下移動
        dialog.select_down();
        assert_eq!(dialog.selected_index, 1);
        dialog.select_down();
        assert_eq!(dialog.selected_index, 2); // Goodbye
        dialog.select_down();
        assert_eq!(dialog.selected_index, 0); // ループ

        // トピック決定: advance_with_vm でセリフ表示へ遷移
        let mut vm = ScriptVm::new();
        let closed = dialog.advance_with_vm(&mut vm);
        assert!(!closed);
        assert_eq!(dialog.phase, DialogPhase::ShowingSpeech);
        assert_eq!(dialog.current_speech, "ジェームズのことか？");

        // もう一度 advance でトピック一覧へ戻る
        dialog.advance();
        assert_eq!(dialog.phase, DialogPhase::ShowingTopics);

        // Goodbye 選択肢 (index 2) を選ぶ
        dialog.selected_index = 2;
        let closed = dialog.advance_with_vm(&mut vm);
        assert!(!closed);
        assert_eq!(dialog.phase, DialogPhase::ShowingSpeech);
        assert_eq!(dialog.current_speech, "またな。");
        assert!(dialog.is_goodbye_pending);

        // Goodbye セリフ表示後に advance を呼ぶと終了
        let closed = dialog.advance();
        assert!(closed);
        assert!(dialog.is_closed);
    }
}
