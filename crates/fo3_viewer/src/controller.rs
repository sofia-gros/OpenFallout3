//! カメラおよびキャラクタ物理移動（ウォークスルー）コントローラー。
//!
//! 参照元: Gamebryo 2.6 カメラ制御 & Havok キャラクタ移動

use std::time::Instant;
use fo3_physics::{RapierCharacterController, RapierPhysicsWorld};
use fo3_render::OrbitCamera;
use crate::types::CameraMode;

/// 入力と移動状態を保持するコントローラー。
pub struct Controller {
    pub camera_mode: CameraMode,
    pub camera: OrbitCamera,
    pub physics_world: RapierPhysicsWorld,
    pub character_controller: RapierCharacterController,
    pub initial_spawn_point: glam::Vec3,
    pub vertical_velocity: f32,
    pub last_frame_time: Instant,
    // キー入力状態
    pub key_forward: bool,
    pub key_backward: bool,
    pub key_left: bool,
    pub key_right: bool,
    pub key_jump: bool,
    // マウス入力状態
    pub left_mouse_down: bool,
    pub right_mouse_down: bool,
    pub last_mouse_pos: Option<(f64, f64)>,
}

impl Controller {
    pub fn new(
        aspect: f32,
        bounds_center: glam::Vec3,
        bounds_radius: f32,
        spawn_pos: glam::Vec3,
        physics_world: RapierPhysicsWorld,
    ) -> Self {
        let mut camera = OrbitCamera::new(aspect);
        camera.focus(bounds_center, bounds_radius);
        println!(
            "カメラ自動フォーカス: 注視点 {:?}, 距離 {:.1}",
            camera.target, camera.distance
        );

        let character_controller = RapierCharacterController::new(spawn_pos);

        Self {
            camera_mode: CameraMode::Orbit,
            camera,
            physics_world,
            character_controller,
            initial_spawn_point: spawn_pos,
            vertical_velocity: 0.0,
            last_frame_time: Instant::now(),
            key_forward: false,
            key_backward: false,
            key_left: false,
            key_right: false,
            key_jump: false,
            left_mouse_down: false,
            right_mouse_down: false,
            last_mouse_pos: None,
        }
    }

    /// フレームごとの物理・キャラクタ位置更新を行い、dt (秒) を返す。
    pub fn update(&mut self) -> f32 {
        let now = Instant::now();
        let dt = (now - self.last_frame_time).as_secs_f32().clamp(0.001, 0.1);
        self.last_frame_time = now;

        // FPS ウォークスルー歩行モード時の物理シミュレーション
        if self.camera_mode == CameraMode::Walkthrough {
            // 水平面上の移動方向（カメラのヨー角から計算: Z-up 右手系）
            let forward =
                glam::Vec3::new(self.camera.yaw.cos(), self.camera.yaw.sin(), 0.0).normalize();
            let right =
                glam::Vec3::new(self.camera.yaw.sin(), -self.camera.yaw.cos(), 0.0).normalize();

            let mut move_dir = glam::Vec3::ZERO;
            if self.key_forward {
                move_dir += forward;
            }
            if self.key_backward {
                move_dir -= forward;
            }
            if self.key_right {
                move_dir += right;
            }
            if self.key_left {
                move_dir -= right;
            }

            let move_speed = 300.0; // ゲーム単位/秒 (約 4.3 m/s)
            let horiz_velocity = if move_dir.length_squared() > 0.001 {
                move_dir.normalize() * move_speed
            } else {
                glam::Vec3::ZERO
            };

            // 重力とジャンプ
            let gravity = -980.0; // 重力加速度 (約 -14 m/s^2)
            if self.character_controller.is_grounded {
                if self.key_jump {
                    self.vertical_velocity = 350.0; // ジャンプ初速
                } else {
                    self.vertical_velocity = -10.0; // 地面スナップ維持のための微小押し下げ
                }
            } else {
                self.vertical_velocity += gravity * dt;
                self.vertical_velocity = self.vertical_velocity.clamp(-1200.0, 500.0);
            }

            let desired_translation =
                (horiz_velocity + glam::Vec3::new(0.0, 0.0, self.vertical_velocity)) * dt;

            // 物理エンジンによる移動計算 (階段自動昇降・衝突スライド・接地判定)
            self.character_controller.step_move(
                dt,
                desired_translation,
                &self.physics_world.rigid_body_set,
                &self.physics_world.collider_set,
                &self.physics_world.query_pipeline,
            );

            // カメラの目の高さをキャラクタ位置 + 55 単位（プレイヤーアイレベル 約 119）に設定
            let eye_level = self.character_controller.position + glam::Vec3::new(0.0, 0.0, 55.0);
            self.camera.override_eye = Some(eye_level);
        } else {
            self.camera.override_eye = None;
        }

        dt
    }
}
