//! # 2D UI およびテキストレンダリングモジュール
//!
//! Fallout 3 実機アセットおよび Gamebryo 2.6 UI 仕様に基づくテキスト・HUD・ダイアログオーバーレイ描画。
//! 参照元: `Fallout - Misc.bsa` (`menus\dialog\dialog_menu.xml`, `menus\terminal\terminal_menu.xml`)

pub mod font;
pub mod pipeline;

pub use font::{BitmapFont, GlyphMetrics, TextBatch, UiVertex};
pub use pipeline::UiRenderer;

/// UI カラー定数 (Fallout 3 実機カラーパレット)
pub mod colors {
    /// Pip-Boy 標準グリーン (`#1aff80`)
    pub const PIPBOY_GREEN: [f32; 4] = [0.10, 1.0, 0.50, 1.0];
    /// ターミナル アンバー (`#ffb642`)
    pub const TERMINAL_AMBER: [f32; 4] = [1.0, 0.71, 0.26, 1.0];
    /// ターミナル グリーン (`#33ff66`)
    pub const TERMINAL_GREEN: [f32; 4] = [0.20, 1.0, 0.40, 1.0];
    /// 選択時ハイライト（純白）
    pub const HIGHLIGHT_WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    /// 非選択/薄文字（半透明グリーン）
    pub const MUTED_GREEN: [f32; 4] = [0.10, 1.0, 0.50, 0.5];
    /// 背景半透明黒（ダイアログ/ターミナル背景）
    pub const BG_OVERLAY: [f32; 4] = [0.0, 0.0, 0.0, 0.75];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_font_and_text_batch() {
        let font = BitmapFont::create_embedded_fallback();
        assert_eq!(font.texture_width, 128);
        assert_eq!(font.texture_height, 128);
        assert!(font.glyphs[b'A' as usize].width > 0.0);

        let mut batch = TextBatch::default();
        batch.add_rect(10.0, 20.0, 100.0, 50.0, colors::BG_OVERLAY);
        assert_eq!(batch.vertices.len(), 4);
        assert_eq!(batch.indices.len(), 6);

        batch.add_text(&font, "TALK", 15.0, 25.0, 1.0, colors::PIPBOY_GREEN);
        assert_eq!(batch.vertices.len(), 4 + 4 * 4);
        assert_eq!(batch.indices.len(), 6 + 4 * 6);
    }
}
