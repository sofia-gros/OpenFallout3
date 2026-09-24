//! 実機 `dialog_menu.xml` 準拠の 2D 会話 UI レンダリングバッチ生成モジュール。
//!
//! 参照元:
//! - `menus/dialog/dialog_menu.xml` (`DM_SpeakerNameLabel`, `DM_CenterHeight`, `DM_TextBackground`, `DM_SpeakerText`, `DM_TopicList`)

use fo3_render::ui::{BitmapFont, TextBatch};
use fo3_render::{ui_colors, TextureAtlas};
use super::state::{DialogPhase, DialogState};

impl DialogState {
    /// 実機 `dialog_menu.xml` 準拠の 2D UI レンダリングバッチを生成。
    ///
    /// 参照元:
    /// - `menus/dialog/dialog_menu.xml` (`DM_SpeakerNameLabel`, `DM_CenterHeight`, `DM_TextBackground`, `DM_SpeakerText`, `DM_TopicList`)
    pub fn render_to_batch(
        &self,
        batch: &mut TextBatch,
        font: &BitmapFont,
        screen_w: f32,
        screen_h: f32,
    ) {
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
        batch.add_text(
            font,
            name_label,
            label_x,
            label_y,
            1.15,
            ui_colors::PIPBOY_GREEN,
        );

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
                batch.add_text(
                    font,
                    &self.current_speech,
                    text_x,
                    text_y,
                    1.05,
                    ui_colors::PIPBOY_GREEN,
                );
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
        render_dialog_brackets(
            batch,
            atlas,
            bg_x,
            box_y,
            box_y + box_h,
            bg_w,
            ui_colors::PIPBOY_GREEN,
        );

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
            batch.add_text(
                font,
                "^",
                bg_x + 10.0,
                box_y + 12.0,
                1.2,
                ui_colors::PIPBOY_GREEN,
            );
        }
        if end_idx < self.choices.len() {
            batch.add_text(
                font,
                "v",
                bg_x + 10.0,
                box_y + box_h - 22.0,
                1.2,
                ui_colors::PIPBOY_GREEN,
            );
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
                let box_color = [
                    ui_colors::PIPBOY_GREEN[0],
                    ui_colors::PIPBOY_GREEN[1],
                    ui_colors::PIPBOY_GREEN[2],
                    0.95,
                ];
                // top
                batch.add_rect(
                    content_x - 10.0,
                    item_y - 3.0,
                    content_w,
                    border_thick,
                    box_color,
                );
                // bottom
                batch.add_rect(
                    content_x - 10.0,
                    item_y + item_h - 6.0,
                    content_w,
                    border_thick,
                    box_color,
                );
                // left
                batch.add_rect(
                    content_x - 10.0,
                    item_y - 3.0,
                    border_thick,
                    item_h - 3.0,
                    box_color,
                );
                // right
                batch.add_rect(
                    content_x - 10.0 + content_w - border_thick,
                    item_y - 3.0,
                    border_thick,
                    item_h - 3.0,
                    box_color,
                );
                // 極薄半透明フィル (_fill_alpha: 40)
                batch.add_rect(
                    content_x - 10.0,
                    item_y - 3.0,
                    content_w,
                    item_h - 3.0,
                    [0.05, 0.25, 0.12, 0.12],
                );

                // 選択項目テキスト (鮮やかな実機 Pip-Boy Green, プレフィックスなし)
                let text_color = [0.22, 1.0, 0.45, 1.0];
                batch.add_text(
                    font,
                    &choice.prompt,
                    content_x + 6.0,
                    item_y + 2.0,
                    1.05,
                    text_color,
                );
            } else {
                // 非選択項目テキスト (実機 _line_alpha: 128 準拠の半透明 Pip-Boy Green)
                let text_color = [0.18, 0.88, 0.38, 0.60];
                batch.add_text(
                    font,
                    &choice.prompt,
                    content_x + 6.0,
                    item_y + 2.0,
                    1.0,
                    text_color,
                );
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
        batch.add_rect(
            x + width - line_thick,
            top_y,
            line_thick,
            bracket_vert_h * 0.5,
            color,
        );
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
        batch.add_rect(
            x,
            bottom_y - bracket_vert_h * 0.5,
            line_thick,
            bracket_vert_h * 0.5,
            color,
        );
        batch.add_rect(
            x + width - line_thick,
            bottom_y - bracket_vert_h * 0.5,
            line_thick,
            bracket_vert_h * 0.5,
            color,
        );
    }
}
