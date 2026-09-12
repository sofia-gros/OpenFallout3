//! # UI (ユーザーインターフェース) サブシステム
//!
//! Fallout 3 実機アセットおよび Gamebryo 2.6 アーキテクチャに準拠した UI 状態マシン。
//!
//! - 通常探索 (`Exploring`): カメラ操作、一人称/三人称移動、HUD表示
//! - 会話モード (`Dialog`): NPC 対話画面、トピック選択肢一覧、セリフ応答
//! - ターミナル画面 (`Terminal`): レトロ CRT ディスプレイ、階層メニュー、ログ表示

pub mod dialog;
pub mod terminal;

pub use dialog::{DialogChoice, DialogState};
pub use terminal::{TerminalScreen, TerminalState};

/// ビューアの実行モード。
#[derive(Clone, Debug)]
pub enum ViewerMode {
    /// 通常探索モード（一人称・三人称歩行、物理コリジョン、自由視点移動）
    Exploring,
    /// NPC 会話ダイアログモード（カメラ固定、UI 表示、トピック選択）
    Dialog(DialogState),
    /// ターミナル画面モード（全画面 CRT UI 表示、階層メニュー操作）
    Terminal(TerminalState),
}

impl Default for ViewerMode {
    fn default() -> Self {
        Self::Exploring
    }
}

impl ViewerMode {
    /// 現在のモードが通常探索モード以外（UI表示中）か判定。
    pub fn is_ui_active(&self) -> bool {
        !matches!(self, ViewerMode::Exploring)
    }

    /// UI 表示中のキー入力を処理する。UI によって消費された場合は true を返す。
    /// 参照元: Fallout 3 操作系 (会話終了・ターミナル退出は T キーまたは Escape)
    pub fn handle_key(&mut self, key: winit::keyboard::KeyCode) -> bool {
        match self {
            ViewerMode::Exploring => false,
            ViewerMode::Dialog(ref mut state) => {
                use winit::keyboard::KeyCode;
                match key {
                    KeyCode::KeyW | KeyCode::ArrowUp => {
                        state.select_up();
                        true
                    }
                    KeyCode::KeyS | KeyCode::ArrowDown => {
                        state.select_down();
                        true
                    }
                    KeyCode::KeyE | KeyCode::Enter => {
                        if state.confirm_selection() {
                            *self = ViewerMode::Exploring;
                        }
                        true
                    }
                    KeyCode::KeyT | KeyCode::Escape => {
                        *self = ViewerMode::Exploring;
                        println!("[会話UI] 会話を終了しました。");
                        true
                    }
                    _ => true, // UI 開いている間は他の操作を吸収
                }
            }
            ViewerMode::Terminal(ref mut state) => {
                use winit::keyboard::KeyCode;
                match key {
                    KeyCode::KeyW | KeyCode::ArrowUp => {
                        state.select_up();
                        true
                    }
                    KeyCode::KeyS | KeyCode::ArrowDown => {
                        state.select_down();
                        true
                    }
                    KeyCode::KeyE | KeyCode::Enter => {
                        state.confirm_selection();
                        if state.is_closed {
                            *self = ViewerMode::Exploring;
                            println!("[ターミナルUI] ログアウトしました。");
                        }
                        true
                    }
                    KeyCode::KeyT | KeyCode::Escape => {
                        state.back();
                        if state.is_closed {
                            *self = ViewerMode::Exploring;
                            println!("[ターミナルUI] ターミナルを終了しました。");
                        }
                        true
                    }
                    _ => true, // UI 開いている間は他の操作を吸収
                }
            }
        }
    }
}
