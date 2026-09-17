//! # キーマネージャー & 入力バインディングシステム (`InputManager`)
//!
//! Fallout 3 PC 実機準拠の操作系（`Fallout.ini` `[Controls]`）および
//! F1〜F12 ファンクションキーによるデバッグ機能を統括する。
//! 参照元: Gamebryo 2.6 入力マッピング, Fallout 3 PC Default Keybindings

use std::collections::{HashMap, HashSet};
use winit::keyboard::KeyCode;

/// ゲームプレイアクション (Fallout 3 実機準拠)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameAction {
    /// 前進 (W, Up)
    Forward,
    /// 後退 (S, Down)
    Backward,
    /// 左平行移動 (A, Left)
    StrafeLeft,
    /// 右平行移動 (D, Right)
    StrafeRight,
    /// ジャンプ (Space)
    Jump,
    /// しゃがみ / スニーク (LeftCtrl, RightCtrl)
    Sneak,
    /// 走る / 歩くトグル (LeftShift, RightShift)
    Run,
    /// アクティベート / 調べる / 話す (E)
    Activate,
    /// 武器構え / リロード (R)
    ReadyWeapon,
    /// 視点切替 (1人称 ⇔ 3人称) (F, V)
    TogglePOV,
    /// Pip-Boy / メニュー / 戻る (Tab)
    PipBoy,
    /// 待機メニュー (T)
    Wait,
}

/// 開発・デバッグ用ファンクションアクション (F1 〜 F12)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DebugAction {
    /// F1: 操作キーヘルプ表示
    Help,
    /// F2: Havok コリジョンワイヤーフレーム表示切替
    ToggleCollision,
    /// F3: セル環境フォグ ON/OFF
    ToggleFog,
    /// F4: ビューア補助ヘッドライト ON/OFF
    ToggleHeadlight,
    /// F5: クイックセーブ (拡張用)
    QuickSave,
    /// F6: スクリプト / AI パッケージ再評価
    ReloadScripts,
    /// F7: カメラ位置・注視点自動再フォーカス (Reset)
    ResetCamera,
    /// F8: メッシュワイヤーフレーム表示切替
    ToggleWireframe,
    /// F9: クイックロード (拡張用)
    QuickLoad,
    /// F10: HUD / UI 表示切替
    ToggleHud,
    /// F11: フルスクリーン表示切替
    ToggleFullscreen,
    /// F12: 俯瞰フリーオービットカメラ切替 (Orbit ⇔ Standard)
    ToggleFreeOrbit,
}

/// 入力コマンドの分類
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputCommand {
    Game(GameAction),
    Debug(DebugAction),
}

/// キー入力とバインドの統合管理マネージャー。
#[derive(Clone, Debug)]
pub struct InputManager {
    /// キーコードから割り当てられたアクションへのマップ
    bindings: HashMap<KeyCode, Vec<InputCommand>>,
    /// 現在押下状態の物理キーセット
    pressed_keys: HashSet<KeyCode>,
    /// 当該フレームで押下されたゲームアクション (立ち上がりエッジ)
    just_pressed_game: HashSet<GameAction>,
    /// 当該フレームで押下されたデバッグアクション (立ち上がりエッジ)
    just_pressed_debug: HashSet<DebugAction>,
    /// 当該フレームで離されたゲームアクション (立ち下がりエッジ)
    just_released_game: HashSet<GameAction>,
    /// 当該フレームで離されたデバッグアクション (立ち下がりエッジ)
    just_released_debug: HashSet<DebugAction>,
}

impl Default for InputManager {
    fn default() -> Self {
        Self::new()
    }
}

impl InputManager {
    /// Fallout 3 PC 実機標準バインディングおよび F1〜F12 デバッグバインディングで初期化する。
    pub fn new() -> Self {
        let mut mgr = Self {
            bindings: HashMap::new(),
            pressed_keys: HashSet::new(),
            just_pressed_game: HashSet::new(),
            just_pressed_debug: HashSet::new(),
            just_released_game: HashSet::new(),
            just_released_debug: HashSet::new(),
        };

        // --- Fallout 3 PC 実機標準コントロール ---
        // 移動
        mgr.bind_game(KeyCode::KeyW, GameAction::Forward);
        mgr.bind_game(KeyCode::ArrowUp, GameAction::Forward);
        mgr.bind_game(KeyCode::KeyS, GameAction::Backward);
        mgr.bind_game(KeyCode::ArrowDown, GameAction::Backward);
        mgr.bind_game(KeyCode::KeyA, GameAction::StrafeLeft);
        mgr.bind_game(KeyCode::ArrowLeft, GameAction::StrafeLeft);
        mgr.bind_game(KeyCode::KeyD, GameAction::StrafeRight);
        mgr.bind_game(KeyCode::ArrowRight, GameAction::StrafeRight);

        // アクション
        mgr.bind_game(KeyCode::Space, GameAction::Jump);
        mgr.bind_game(KeyCode::ControlLeft, GameAction::Sneak);
        mgr.bind_game(KeyCode::ControlRight, GameAction::Sneak);
        mgr.bind_game(KeyCode::ShiftLeft, GameAction::Run);
        mgr.bind_game(KeyCode::ShiftRight, GameAction::Run);
        mgr.bind_game(KeyCode::KeyE, GameAction::Activate);
        mgr.bind_game(KeyCode::KeyR, GameAction::ReadyWeapon);
        mgr.bind_game(KeyCode::KeyF, GameAction::TogglePOV);
        mgr.bind_game(KeyCode::KeyV, GameAction::TogglePOV);
        mgr.bind_game(KeyCode::Tab, GameAction::PipBoy);
        mgr.bind_game(KeyCode::KeyT, GameAction::Wait);

        // --- デバッグファンクションキー (F1 〜 F12) ---
        mgr.bind_debug(KeyCode::F1, DebugAction::Help);
        mgr.bind_debug(KeyCode::F2, DebugAction::ToggleCollision);
        mgr.bind_debug(KeyCode::F3, DebugAction::ToggleFog);
        mgr.bind_debug(KeyCode::F4, DebugAction::ToggleHeadlight);
        mgr.bind_debug(KeyCode::F5, DebugAction::QuickSave);
        mgr.bind_debug(KeyCode::F6, DebugAction::ReloadScripts);
        mgr.bind_debug(KeyCode::F7, DebugAction::ResetCamera);
        mgr.bind_debug(KeyCode::F8, DebugAction::ToggleWireframe);
        mgr.bind_debug(KeyCode::F9, DebugAction::QuickLoad);
        mgr.bind_debug(KeyCode::F10, DebugAction::ToggleHud);
        mgr.bind_debug(KeyCode::F11, DebugAction::ToggleFullscreen);
        mgr.bind_debug(KeyCode::F12, DebugAction::ToggleFreeOrbit);

        mgr
    }

    /// ゲームアクションにキーを割り当てる。
    pub fn bind_game(&mut self, key: KeyCode, action: GameAction) {
        let cmd = InputCommand::Game(action);
        let list = self.bindings.entry(key).or_default();
        if !list.contains(&cmd) {
            list.push(cmd);
        }
    }

    /// デバッグアクションにキーを割り当てる。
    pub fn bind_debug(&mut self, key: KeyCode, action: DebugAction) {
        let cmd = InputCommand::Debug(action);
        let list = self.bindings.entry(key).or_default();
        if !list.contains(&cmd) {
            list.push(cmd);
        }
    }

    /// 指定キーの割り当てを解除する。
    pub fn unbind_key(&mut self, key: KeyCode) {
        self.bindings.remove(&key);
    }

    /// 全バインドをクリアする。
    pub fn clear_bindings(&mut self) {
        self.bindings.clear();
    }

    /// 押下キーに対応する主要なアクションを取得する。
    pub fn get_action(&self, key: KeyCode) -> Option<InputCommand> {
        self.bindings
            .get(&key)
            .and_then(|list| list.first().copied())
    }

    /// フレーム開始時の入力エッジ状態リセット。
    pub fn update_frame(&mut self) {
        self.just_pressed_game.clear();
        self.just_pressed_debug.clear();
        self.just_released_game.clear();
        self.just_released_debug.clear();
    }

    /// 物理キーの押下・解放イベントを処理する。
    pub fn handle_key_event(&mut self, key: KeyCode, pressed: bool) {
        let was_pressed = self.pressed_keys.contains(&key);

        if pressed {
            self.pressed_keys.insert(key);
            if !was_pressed {
                if let Some(cmds) = self.bindings.get(&key) {
                    for cmd in cmds {
                        match cmd {
                            InputCommand::Game(action) => {
                                self.just_pressed_game.insert(*action);
                            }
                            InputCommand::Debug(action) => {
                                self.just_pressed_debug.insert(*action);
                            }
                        }
                    }
                }
            }
        } else {
            self.pressed_keys.remove(&key);
            if was_pressed {
                if let Some(cmds) = self.bindings.get(&key) {
                    for cmd in cmds {
                        match cmd {
                            InputCommand::Game(action) => {
                                self.just_released_game.insert(*action);
                            }
                            InputCommand::Debug(action) => {
                                self.just_released_debug.insert(*action);
                            }
                        }
                    }
                }
            }
        }
    }

    /// ゲームアクションが現在押下中（ホールド状態）であるかを判定。
    pub fn is_action_down(&self, action: GameAction) -> bool {
        let target = InputCommand::Game(action);
        self.pressed_keys.iter().any(|k| {
            self.bindings
                .get(k)
                .map(|list| list.contains(&target))
                .unwrap_or(false)
        })
    }

    /// デバッグアクションが現在押下中であるかを判定。
    pub fn is_debug_down(&self, action: DebugAction) -> bool {
        let target = InputCommand::Debug(action);
        self.pressed_keys.iter().any(|k| {
            self.bindings
                .get(k)
                .map(|list| list.contains(&target))
                .unwrap_or(false)
        })
    }

    /// 当該フレームでゲームアクションが新たに押されたか（立ち上がりエッジ）。
    pub fn is_action_just_pressed(&self, action: GameAction) -> bool {
        self.just_pressed_game.contains(&action)
    }

    /// 当該フレームでデバッグアクションが新たに押されたか（立ち上がりエッジ）。
    pub fn is_debug_just_pressed(&self, action: DebugAction) -> bool {
        self.just_pressed_debug.contains(&action)
    }

    /// 操作説明ガイドテキストを生成して返す。
    pub fn get_guide_text(&self) -> String {
        let mut s = String::new();
        s.push_str("\n============================================================\n");
        s.push_str("★ Fallout 3 実機標準操作体系 (InputManager)\n");
        s.push_str("============================================================\n");
        s.push_str("  [ゲーム操作 (Fallout 3 実機準拠)]\n");
        s.push_str("    W, A, S, D / 矢印 : 移動 (前後・左右平行移動)\n");
        s.push_str("    Space             : ジャンプ\n");
        s.push_str("    Ctrl              : しゃがみ / スニーク (カメラアイレベル追従)\n");
        s.push_str("    Shift             : 歩き / 走り切替\n");
        s.push_str("    E                 : 調べる / 話す / 扉を開ける (Activate)\n");
        s.push_str("    R                 : 武器構え / リロード\n");
        s.push_str("    F / V             : 1人称 ⇔ 3人称 視点切替 (POV Toggle)\n");
        s.push_str("    Tab               : Pip-Boy / 会話終了 / キャンセル\n");
        s.push_str("    T                 : 待機 (Wait)\n");
        s.push_str("    マウス移動        : 視線変更 (Look)\n");
        s.push_str("    マウスホイール    : 1人称/3人称 ズームイン・アウト\n");
        s.push_str("  [デバッグ・検証操作 (F1 〜 F12)]\n");
        s.push_str("    F1                : この操作ガイドを表示\n");
        s.push_str("    F2                : Havok コリジョンワイヤーフレーム表示切替\n");
        s.push_str("    F3                : セル環境フォグ ON/OFF 切替\n");
        s.push_str("    F4                : ビューア補助ヘッドライト ON/OFF 切替\n");
        s.push_str("    F7                : カメラ・スポーン位置の自動再フォーカス (Reset)\n");
        s.push_str("    F12               : 俯瞰フリーオービットカメラ切替 (Orbit ⇔ Standard)\n");
        s.push_str("    Esc               : 終了\n");
        s.push_str("============================================================\n");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_manager_defaults_and_edges() {
        let mut mgr = InputManager::new();

        // 初期状態
        assert!(!mgr.is_action_down(GameAction::Forward));
        assert!(!mgr.is_action_just_pressed(GameAction::Forward));

        // KeyW 押下
        mgr.handle_key_event(KeyCode::KeyW, true);
        assert!(mgr.is_action_down(GameAction::Forward));
        assert!(mgr.is_action_just_pressed(GameAction::Forward));

        // 次フレーム
        mgr.update_frame();
        assert!(mgr.is_action_down(GameAction::Forward));
        assert!(!mgr.is_action_just_pressed(GameAction::Forward));

        // KeyW 解放
        mgr.handle_key_event(KeyCode::KeyW, false);
        assert!(!mgr.is_action_down(GameAction::Forward));

        // F12 デバッグキー
        mgr.handle_key_event(KeyCode::F12, true);
        assert!(mgr.is_debug_down(DebugAction::ToggleFreeOrbit));
        assert!(mgr.is_debug_just_pressed(DebugAction::ToggleFreeOrbit));
    }

    #[test]
    fn test_input_manager_rebinding() {
        let mut mgr = InputManager::new();
        // KeyW を解除して KeyI に再割り当て
        mgr.unbind_key(KeyCode::KeyW);
        assert!(!mgr.is_action_down(GameAction::Forward));

        mgr.handle_key_event(KeyCode::KeyW, true);
        assert!(!mgr.is_action_down(GameAction::Forward));

        mgr.bind_game(KeyCode::KeyI, GameAction::Forward);
        mgr.handle_key_event(KeyCode::KeyI, true);
        assert!(mgr.is_action_down(GameAction::Forward));
    }
}
