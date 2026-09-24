//! # 会話ダイアログ UI (`DialogState`, `DialogChoice`, `DialogPhase`)
//!
//! Fallout 3 実機 `menus/dialog/dialog_menu.xml` のレイアウトと動作を忠実に再現したダイアログ UI。
//! 話者名、セリフ、選択肢トピック一覧の 2 フェーズ進行およびレンダリングを管理する。
//!
//! 参照元:
//! - `menus/dialog/dialog_menu.xml`
//! - `references/openmw/components/esm4/loaddial.hpp`, `loadinfo.hpp`

mod render;
mod state;

#[cfg(test)]
mod tests;

pub use state::{DialogChoice, DialogPhase, DialogState};
