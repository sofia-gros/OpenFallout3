//! # ウィンドウ入力イベント処理サブモジュール
//!
//! Winit ウィンドウイベント (キーボード、マウスホイール、Pip-Boy メニュー、
//! キャラメイク入力、デバッグキー) の処理を担当する。
//!
//! 参照元: `Gamebryo 2.6 NiInputSystem`, `app.rs:WindowEvent`

use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use crate::app::{AppState, ViewerMode};
use crate::controller::CameraMode;

/// ウィンドウの入力イベント (マウスホイール、キーボード) をディスパッチ処理する。
pub fn handle_input_event(state: &mut AppState, event: WindowEvent, event_loop: &ActiveEventLoop) {
    match event {
        WindowEvent::MouseWheel { delta, .. } => {
            let zoom_amount = match delta {
                MouseScrollDelta::LineDelta(_, y) => y,
                MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.05,
            };
            if state.controller.camera_mode == CameraMode::Standard {
                if state.vm.player_controls.pov {
                    state.controller.player_camera.zoom(zoom_amount);
                    if let Some(ref mut player) = state.controller.player_actor {
                        player.set_view_mode(state.controller.player_camera.mode, &mut state.scene.meshes);
                    }
                }
            } else {
                state.controller.camera.zoom(zoom_amount);
            }
            state.window.request_redraw();
        }
        WindowEvent::KeyboardInput { event, .. } => {
            let pressed = event.state == ElementState::Pressed;
            if let PhysicalKey::Code(key) = event.physical_key {
                // キャラクター作成メニュー (RaceSexMenu / NameMenu) のキー・テキスト入力
                if pressed && state.chargen_menu.is_active() {
                    if let Some(text) = &event.text {
                        for c in text.chars() {
                            state.chargen_menu.handle_char(c);
                        }
                    }
                    if state.chargen_menu.handle_key(key, &mut state.vm) {
                        state.window.request_redraw();
                        return;
                    }
                }

                // 実機メッセージダイアログ (MESG / ShowMessage) のボタン選択
                if pressed {
                    if let Some(msg_id) = state.vm.show_messages.first().cloned() {
                        let mesg_opt = state.master_context.mesg_edid_map.get(&msg_id.to_ascii_uppercase())
                            .and_then(|fid| state.master_context.mesg_map.get(fid));
                        if let Some(mesg) = mesg_opt {
                            let btn_idx = match key {
                                KeyCode::Digit1 | KeyCode::Numpad1 => Some(0),
                                KeyCode::Digit2 | KeyCode::Numpad2 => Some(1),
                                KeyCode::Digit3 | KeyCode::Numpad3 => Some(2),
                                KeyCode::Digit4 | KeyCode::Numpad4 => Some(3),
                                _ => None,
                            };
                            if let Some(idx) = btn_idx {
                                if idx < mesg.buttons.len() {
                                    state.vm.show_messages.remove(0);
                                    state.vm.set_button_pressed(idx as i32);
                                    if msg_id.eq_ignore_ascii_case("CG00ChooseSexMessage") {
                                        state.vm.player_is_female = idx == 1;
                                        println!("[Chargen] プレイヤー性別決定: is_female={}", state.vm.player_is_female);
                                    }
                                    println!("[MessageMenu] ボタン選択: {} -> GetButtonPressed", idx);
                                    state.window.request_redraw();
                                    return;
                                }
                            }
                        }
                    }
                }

                if pressed && state.mode.is_ui_active() {
                    if state.mode.handle_key_with_vm(key, &mut state.vm) {
                        state.window.request_redraw();
                        return;
                    }
                }

                // キーマネージャーの押下状態更新
                state.input_manager.handle_key_event(key, pressed);

                // キャラクタ移動・姿勢入力の同期 (実機 DisablePlayerControls 反映)
                let can_move = state.vm.player_controls.movement;
                state.controller.key_forward = can_move && state.input_manager.is_action_down(crate::input::GameAction::Forward);
                state.controller.key_backward = can_move && state.input_manager.is_action_down(crate::input::GameAction::Backward);
                state.controller.key_left = can_move && state.input_manager.is_action_down(crate::input::GameAction::StrafeLeft);
                state.controller.key_right = can_move && state.input_manager.is_action_down(crate::input::GameAction::StrafeRight);
                state.controller.key_jump = can_move && state.input_manager.is_action_down(crate::input::GameAction::Jump);
                state.controller.key_sneak = can_move && state.input_manager.is_action_down(crate::input::GameAction::Sneak);
                state.controller.key_run = !state.input_manager.is_action_down(crate::input::GameAction::Run);

                if pressed {
                    if let Some(action) = state.input_manager.get_action(key) {
                        match action {
                            crate::input::InputCommand::Game(crate::input::GameAction::Activate) => {
                                if state.vm.player_controls.movement && !state.vm.in_chargen {
                                    state.interact_or_teleport();
                                }
                            }
                            crate::input::InputCommand::Game(crate::input::GameAction::TogglePOV) => {
                                if state.vm.player_controls.pov {
                                    state.controller.player_camera.toggle_view_mode();
                                    if let Some(ref mut player) = state.controller.player_actor {
                                        player.set_view_mode(state.controller.player_camera.mode, &mut state.scene.meshes);
                                    }
                                    println!("[視点切替] 現在の視点モード: {:?}", state.controller.player_camera.mode);
                                }
                            }
                            crate::input::InputCommand::Game(crate::input::GameAction::PipBoy) => {
                                if state.vm.player_controls.pipboy {
                                    if state.mode.is_ui_active() {
                                        state.mode = ViewerMode::Exploring;
                                    } else {
                                        println!("[Pip-Boy] メニュー (Tab)");
                                    }
                                }
                            }
                            crate::input::InputCommand::Debug(crate::input::DebugAction::Help) => {
                                println!("{}", state.input_manager.get_guide_text());
                            }
                            crate::input::InputCommand::Debug(crate::input::DebugAction::ToggleCollision) => {
                                state.show_collision = !state.show_collision;
                                println!("Havok コリジョン表示 [F2]: {}", if state.show_collision { "ON" } else { "OFF" });
                            }
                            crate::input::InputCommand::Debug(crate::input::DebugAction::ToggleFog) => {
                                state.enable_fog = !state.enable_fog;
                                println!("セル環境フォグ [F3]: {}", if state.enable_fog { "ON" } else { "OFF" });
                            }
                            crate::input::InputCommand::Debug(crate::input::DebugAction::ToggleHeadlight) => {
                                state.headlight = !state.headlight;
                                println!("ビューア補助ヘッドライト [F4]: {}", if state.headlight { "ON" } else { "OFF" });
                            }
                            crate::input::InputCommand::Debug(crate::input::DebugAction::ResetCamera) => {
                                state.controller.camera.focus(state.scene.bounds_center, state.scene.bounds_radius);
                                state.controller.character_controller.position = state.scene.bounds_center + glam::Vec3::new(0.0, 0.0, 64.0);
                                state.controller.vertical_velocity = 0.0;
                                println!("カメラ・スポーン位置再フォーカス [F7]");
                            }
                            crate::input::InputCommand::Debug(crate::input::DebugAction::ToggleFreeOrbit) => {
                                state.controller.camera_mode = match state.controller.camera_mode {
                                    CameraMode::Standard => {
                                        println!("\n[カメラモード] F12: フリーオービットカメラ (全体俯瞰・回転周回) に切り替えました。");
                                        println!("  左ドラッグ: 回転, 右ドラッグ: 平行移動, ホイール: ズーム, F12: 実機カメラへ復帰");
                                        state.controller.camera.target = state.controller.character_controller.position;
                                        CameraMode::FreeOrbit
                                    }
                                    CameraMode::FreeOrbit => {
                                        println!("\n[カメラモード] F12: Fallout 3 実機標準プレイヤーカメラに復帰しました。");
                                        println!("  WASD: 移動, Space: ジャンプ, Ctrl: しゃがみ, E: 調べる, F/V: 視点切替");
                                        state.controller.character_controller.position = state.controller.initial_spawn_point;
                                        state.controller.vertical_velocity = 0.0;
                                        CameraMode::Standard
                                    }
                                };
                            }
                            _ => {}
                        }
                    } else if key == KeyCode::Escape {
                        event_loop.exit();
                    }
                }
            }
            state.window.request_redraw();
        }
        _ => {}
    }
}
