//! NPC 会話ダイアログおよびターミナル画面のオーバーレイ描画モジュール。
//!
//! 参照元: `references/bevyout/src/viewer/fallout_ui.rs`

use super::types::HUD_PHOSPHOR_COLOR;
use super::HudRenderer;

impl HudRenderer {
    /// NPC 会話ダイアログのフレームおよび選択肢オーバーレイを描画する。
    pub fn render_dialog_overlay<'rpass>(
        &'rpass self,
        rpass: &mut wgpu::RenderPass<'rpass>,
        queue: &wgpu::Queue,
        screen_width: f32,
        screen_height: f32,
        dialog: &crate::ui::DialogState,
    ) {
        let box_w = screen_width * 0.85;
        let box_h = screen_height * 0.38;
        let center_y = screen_height * 0.28;
        self.render_rect(
            rpass,
            queue,
            0.0,
            center_y,
            box_w,
            box_h,
            [0.0, 0.06, 0.02, 0.88],
            screen_width,
            screen_height,
        );

        let border_color = HUD_PHOSPHOR_COLOR;
        self.render_rect(
            rpass,
            queue,
            0.0,
            center_y - box_h * 0.5,
            box_w,
            2.0,
            border_color,
            screen_width,
            screen_height,
        );
        self.render_rect(
            rpass,
            queue,
            0.0,
            center_y + box_h * 0.5,
            box_w,
            2.0,
            border_color,
            screen_width,
            screen_height,
        );
        self.render_rect(
            rpass,
            queue,
            -box_w * 0.5,
            center_y,
            2.0,
            box_h,
            border_color,
            screen_width,
            screen_height,
        );
        self.render_rect(
            rpass,
            queue,
            box_w * 0.5,
            center_y,
            2.0,
            box_h,
            border_color,
            screen_width,
            screen_height,
        );

        let item_h = 24.0f32;
        let sel_y = center_y - box_h * 0.1 + (dialog.selected_index as f32) * (item_h + 4.0);
        if sel_y < center_y + box_h * 0.45 {
            self.render_rect(
                rpass,
                queue,
                0.0,
                sel_y,
                box_w * 0.96,
                item_h,
                [0.18, 1.0, 0.48, 0.25],
                screen_width,
                screen_height,
            );
        }
    }

    /// ターミナル画面の全画面 CRT レトロオーバーレイを描画する。
    pub fn render_terminal_overlay<'rpass>(
        &'rpass self,
        rpass: &mut wgpu::RenderPass<'rpass>,
        queue: &wgpu::Queue,
        screen_width: f32,
        screen_height: f32,
        term: &crate::ui::TerminalState,
    ) {
        self.render_rect(
            rpass,
            queue,
            0.0,
            0.0,
            screen_width,
            screen_height,
            [0.0, 0.04, 0.015, 0.94],
            screen_width,
            screen_height,
        );

        let frame_w = screen_width * 0.92;
        let frame_h = screen_height * 0.90;
        let border_color = HUD_PHOSPHOR_COLOR;
        self.render_rect(
            rpass,
            queue,
            0.0,
            -frame_h * 0.5,
            frame_w,
            3.0,
            border_color,
            screen_width,
            screen_height,
        );
        self.render_rect(
            rpass,
            queue,
            0.0,
            frame_h * 0.5,
            frame_w,
            3.0,
            border_color,
            screen_width,
            screen_height,
        );
        self.render_rect(
            rpass,
            queue,
            -frame_w * 0.5,
            0.0,
            3.0,
            frame_h,
            border_color,
            screen_width,
            screen_height,
        );
        self.render_rect(
            rpass,
            queue,
            frame_w * 0.5,
            0.0,
            3.0,
            frame_h,
            border_color,
            screen_width,
            screen_height,
        );

        let header_y = -frame_h * 0.35;
        self.render_rect(
            rpass,
            queue,
            0.0,
            header_y,
            frame_w * 0.95,
            2.0,
            [0.18, 1.0, 0.48, 0.6],
            screen_width,
            screen_height,
        );

        if matches!(term.screen, crate::ui::TerminalScreen::Menu) {
            let item_h = 28.0f32;
            let sel_y = header_y + 50.0 + (term.selected_index as f32) * (item_h + 6.0);
            if sel_y < frame_h * 0.45 {
                self.render_rect(
                    rpass,
                    queue,
                    0.0,
                    sel_y,
                    frame_w * 0.92,
                    item_h,
                    [0.18, 1.0, 0.48, 0.30],
                    screen_width,
                    screen_height,
                );
            }
        }
    }
}
