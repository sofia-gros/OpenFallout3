//! winit イベントハンドラおよびアプリケーションループ (`App`)。
//!
//! 参照元: Gamebryo 2.6 レンダリングパイプライン & winit イベントループ

use pollster::FutureExt;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use crate::types::{CameraMode, ViewerTarget};
use super::ViewerState;

/// winit アプリケーションハンドラ。
pub struct App {
    pub data_dir: String,
    pub target: ViewerTarget,
    pub state: Option<ViewerState>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title("OpenFallout3 Viewer")
                .with_inner_size(PhysicalSize::new(1280, 720));
            let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
            let state = ViewerState::new(window.clone(), &self.data_dir, &self.target).block_on();
            self.state = Some(state);
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = match self.state.as_mut() {
            Some(s) => s,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                state.resize(physical_size);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                state.update();
                match state.render() {
                    Ok(_) => {
                        state.window.request_redraw();
                    }
                    // サーフェスが再構成を要求している状態からの復帰。
                    // wgpu 仕様: Outdated / Lost は「ウィンドウリサイズや表示状態変化等により
                    // サーフェス構成が古くなった」ことを示し、`Surface::configure` の再実行で回復する。
                    // 放置すると毎フレーム失敗し続けるため、現在の実ウィンドウサイズで再構成して再描画を要求する。
                    Err(wgpu::SurfaceError::Outdated) | Err(wgpu::SurfaceError::Lost) => {
                        let live = state.window.inner_size();
                        if live.width > 0 && live.height > 0 {
                            state.resize(live);
                        }
                        state.window.request_redraw();
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                    Err(e) => eprintln!("レンダリングエラー: {:?}", e),
                }
            }
            WindowEvent::MouseInput {
                button,
                state: btn_state,
                ..
            } => {
                let pressed = btn_state == ElementState::Pressed;
                if pressed && button == MouseButton::Left && state.mode.is_ui_active() {
                    let _ = state
                        .mode
                        .handle_key_with_vm(winit::keyboard::KeyCode::Space, &mut state.vm);
                }
                match button {
                    MouseButton::Left => state.controller.left_mouse_down = pressed,
                    MouseButton::Right => state.controller.right_mouse_down = pressed,
                    _ => {}
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if state.mode.is_ui_active() {
                    state.controller.last_mouse_pos = Some((position.x, position.y));
                    return;
                }
                if let Some((last_x, last_y)) = state.controller.last_mouse_pos {
                    let dx = (position.x - last_x) as f32;
                    let dy = (position.y - last_y) as f32;

                    match state.controller.camera_mode {
                        CameraMode::FreeOrbit => {
                            if state.controller.left_mouse_down {
                                state.controller.camera.rotate(dx, dy);
                                state.window.request_redraw();
                            } else if state.controller.right_mouse_down {
                                state.controller.camera.pan(dx, dy);
                                state.window.request_redraw();
                            }
                        }
                        CameraMode::Standard => {
                            if state.vm.player_controls.looking
                                && (state.controller.left_mouse_down
                                    || state.controller.right_mouse_down)
                            {
                                state
                                    .controller
                                    .player_camera
                                    .rotate(dx * 0.003, dy * 0.003);
                                state.window.request_redraw();
                            }
                        }
                    }
                }
                state.controller.last_mouse_pos = Some((position.x, position.y));
            }
            WindowEvent::MouseWheel { .. } | WindowEvent::KeyboardInput { .. } => {
                crate::window_input::handle_input_event(state, event, event_loop);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(ref state) = self.state {
            state.window.request_redraw();
        }
    }
}
