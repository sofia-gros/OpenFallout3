//! # プレイヤーカメラコントローラー (`PlayerCamera`)
//!
//! Fallout 3 (Gamebryo 2.6) の実機仕様に準拠した統合カメラシステム。
//! 一人称視点、三人称肩越し追従視点、バニティモード、および物理レイキャストによる壁クリッピング回避を実装。
//!
//! 参照元:
//! - `references/openmw/apps/openmw/mwrender/camera.hpp`, `camera.cpp`
//! - Fallout 3 実機 GMST: `f1stPersonCameraHeight` (124.0), `fChaseCameraMax` (400.0),
//!   `fVanityModeMinDist` (70.0), `fVanityModeMaxDist` (300.0), `fOverShoulderPosX` (30.0)

use std::f32::consts::{FRAC_PI_2, PI};
use glam::{Mat4, Vec3};
use fo3_physics::RapierPhysicsWorld;
use fo3_render::CameraUniform;

/// 一人称時のプレイヤー目線高さ (Gamebryo 2.6 GMST: f1stPersonCameraHeight)
pub const F_1ST_PERSON_CAMERA_HEIGHT: f32 = 124.0;

/// 三人称追従カメラの最大距離 (Gamebryo 2.6 GMST: fChaseCameraMax)
pub const F_CHASE_CAMERA_MAX: f32 = 400.0;

/// 三人称追従カメラの既定距離
pub const F_CHASE_CAMERA_DEFAULT: f32 = 150.0;

/// ズーム切り替え最小距離 (Gamebryo 2.6 GMST: fVanityModeMinDist)
/// これより近づくと自動的に一人称視点へ遷移する。
pub const F_VANITY_MODE_MIN_DIST: f32 = 70.0;

/// バニティモードの最大ズーム距離 (Gamebryo 2.6 GMST: fVanityModeMaxDist)
pub const F_VANITY_MODE_MAX_DIST: f32 = 300.0;

/// 肩越しカメラの水平オフセット (Gamebryo 2.6 GMST: fOverShoulderPosX, 右肩)
pub const F_OVER_SHOULDER_POS_X: f32 = 30.0;

/// 肩越しカメラの垂直オフセット (Gamebryo 2.6 GMST: fOverShoulderPosZ)
pub const F_OVER_SHOULDER_POS_Z: f32 = 0.0;

/// 三人称カメラのプレイヤーめり込み防止用最小距離
pub const F_CAMERA_MIN_THIRD_PERSON_DIST: f32 = 45.0;

/// 壁衝突（クリッピング）回避時の安全マージン距離
pub const CAMERA_CLIP_MARGIN: f32 = 10.0;

/// カメラの視点モード
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraViewMode {
    /// 一人称視点 (First Person) - プレイヤーの全身は非表示、一人称腕のみ描画
    FirstPerson,
    /// 三人称肩越し視点 (Third Person) - プレイヤー全身を描画、追従肩越しカメラ
    ThirdPerson,
    /// バニティ・自撮りモード (Vanity) - キャラクターの向きを固定したまま 360 度周回
    Vanity,
}

/// Fallout 3 準拠のプレイヤーカメラコントローラー
#[derive(Clone, Debug)]
pub struct PlayerCamera {
    /// 現在の視点モード
    pub mode: CameraViewMode,
    /// 水平角 (Yaw) - ラジアン (0 〜 2π)
    pub yaw: f32,
    /// 仰角 (Pitch) - ラジアン (-89° 〜 +89°)
    pub pitch: f32,
    /// 三人称カメラの目標距離
    pub distance: f32,
    /// 実際のカメラワールド位置 (壁クリッピング処理適用後)
    pub current_eye: Vec3,
    /// 垂直視野角 (FOV) - ラジアン
    pub fov_y: f32,
    /// 画面アスペクト比 (Width / Height)
    pub aspect: f32,
    /// 前方クリップ距離
    pub z_near: f32,
    /// 後方クリップ距離
    pub z_far: f32,
    /// バニティモード時の周回オフセット角 (Yaw, Pitch)
    pub vanity_yaw: f32,
    pub vanity_pitch: f32,
}

impl PlayerCamera {
    /// 新しいプレイヤーカメラを作成する。
    pub fn new(aspect: f32) -> Self {
        Self {
            mode: CameraViewMode::FirstPerson,
            yaw: -FRAC_PI_2,
            pitch: 0.0,
            distance: F_CHASE_CAMERA_DEFAULT,
            current_eye: Vec3::ZERO,
            fov_y: 75.0_f32.to_radians(), // Fallout 3 デフォルト FOV 75度
            aspect,
            z_near: 1.0,
            z_far: 100000.0,
            vanity_yaw: 0.0,
            vanity_pitch: 0.0,
        }
    }

    /// マウス移動による視線回転を適用する。
    pub fn rotate(&mut self, delta_yaw: f32, delta_pitch: f32) {
        match self.mode {
            CameraViewMode::FirstPerson | CameraViewMode::ThirdPerson => {
                self.yaw += delta_yaw;
                self.pitch -= delta_pitch;
                // ピッチ角を -89度 〜 +89度にクランプ
                let limit = 89.0_f32.to_radians();
                self.pitch = self.pitch.clamp(-limit, limit);
                // ヨー角を 0 〜 2π に正規化
                self.yaw = self.yaw.rem_euclid(2.0 * PI);
            }
            CameraViewMode::Vanity => {
                self.vanity_yaw += delta_yaw;
                self.vanity_pitch -= delta_pitch;
                let limit = 85.0_f32.to_radians();
                self.vanity_pitch = self.vanity_pitch.clamp(-limit, limit);
                self.vanity_yaw = self.vanity_yaw.rem_euclid(2.0 * PI);
            }
        }
    }

    /// マウスホイールによるズーム操作。
    /// 一人称時に手前に引くと三人称へ、三人称で近づきすぎると一人称へ切り替わる。
    pub fn zoom(&mut self, delta: f32) {
        match self.mode {
            CameraViewMode::FirstPerson => {
                if delta < 0.0 {
                    // 手前にスクロール: 三人称モードへ移行
                    self.mode = CameraViewMode::ThirdPerson;
                    self.distance = F_VANITY_MODE_MIN_DIST + 10.0;
                }
            }
            CameraViewMode::ThirdPerson => {
                self.distance -= delta * 15.0;
                if self.distance < F_VANITY_MODE_MIN_DIST {
                    // プレイヤーに近すぎる場合は一人称へ遷移
                    self.mode = CameraViewMode::FirstPerson;
                    self.distance = F_VANITY_MODE_MIN_DIST;
                } else if self.distance > F_CHASE_CAMERA_MAX {
                    self.distance = F_CHASE_CAMERA_MAX;
                }
            }
            CameraViewMode::Vanity => {
                self.distance -= delta * 15.0;
                self.distance = self.distance.clamp(F_VANITY_MODE_MIN_DIST, F_VANITY_MODE_MAX_DIST);
            }
        }
    }

    /// 一人称 ⇄ 三人称のトグル切り替え (Fキー / Vキー)。
    pub fn toggle_view_mode(&mut self) {
        match self.mode {
            CameraViewMode::FirstPerson => {
                self.mode = CameraViewMode::ThirdPerson;
                self.distance = F_CHASE_CAMERA_DEFAULT;
            }
            CameraViewMode::ThirdPerson | CameraViewMode::Vanity => {
                self.mode = CameraViewMode::FirstPerson;
            }
        }
    }

    /// カメラの視線注視点 (頭部・目線位置) を取得する。
    /// 引数 `feet_pos` はアクターの足元（地面接地）ワールド座標。
    /// Gamebryo 2.6 GMST: `f1stPersonCameraHeight` (124.0) を加算して目線位置とする。
    #[inline]
    pub fn focal_point(&self, feet_pos: Vec3) -> Vec3 {
        feet_pos + Vec3::new(0.0, 0.0, F_1ST_PERSON_CAMERA_HEIGHT)
    }

    /// カメラの前方注視単位ベクトルを計算する。
    pub fn forward_vector(&self) -> Vec3 {
        let (y, p) = match self.mode {
            CameraViewMode::Vanity => (self.yaw + self.vanity_yaw, self.pitch + self.vanity_pitch),
            _ => (self.yaw, self.pitch),
        };
        Vec3::new(p.cos() * y.cos(), p.cos() * y.sin(), p.sin()).normalize()
    }

    /// カメラの右方向単位ベクトルを計算する。
    pub fn right_vector(&self) -> Vec3 {
        let fwd = self.forward_vector();
        fwd.cross(Vec3::Z).normalize()
    }

    /// 物理ワールドを参照し、壁クリッピング（壁オクルージョン回避）を行った後の
    /// 最終カメラ位置およびターゲット位置を計算・更新する。
    /// 引数 `feet_pos` はアクターの足元接地面座標。
    pub fn update(
        &mut self,
        feet_pos: Vec3,
        physics_world: Option<&RapierPhysicsWorld>,
    ) {
        let focal = self.focal_point(feet_pos);

        match self.mode {
            CameraViewMode::FirstPerson => {
                // 一人称視点: 目線位置そのものがカメラ位置
                self.current_eye = focal;
            }
            CameraViewMode::ThirdPerson => {
                let forward = self.forward_vector();
                let right = self.right_vector();

                // 肩越しオフセット (右肩方向 + Zオフセット)
                let shoulder_offset = right * F_OVER_SHOULDER_POS_X + Vec3::Z * F_OVER_SHOULDER_POS_Z;
                let origin = focal + shoulder_offset;

                // 理想のカメラ位置 (後方へ distance 離れた位置)
                let ideal_eye = origin - forward * self.distance;

                // 物理レイキャストによる壁クリッピング判定
                let mut actual_dist = self.distance;
                if let Some(physics) = physics_world {
                    let dir = (ideal_eye - origin).normalize();
                    let max_toi = self.distance;
                    if let Some(hit) = physics.cast_ray(origin, dir, max_toi) {
                        // 壁に衝突した場合、衝突点の手前（CAMERA_CLIP_MARGIN）に制限。
                        // ただしプレイヤーの身体内部へめり込まないよう最小距離ガードを適用。
                        let clipped_dist = (hit.distance - CAMERA_CLIP_MARGIN).max(F_CAMERA_MIN_THIRD_PERSON_DIST);
                        actual_dist = actual_dist.min(clipped_dist);
                    }
                }

                self.current_eye = origin - forward * actual_dist;
            }
            CameraViewMode::Vanity => {
                let forward = self.forward_vector();
                let ideal_eye = focal - forward * self.distance;

                let mut actual_dist = self.distance;
                if let Some(physics) = physics_world {
                    let dir = (ideal_eye - focal).normalize();
                    let max_toi = self.distance;
                    if let Some(hit) = physics.cast_ray(focal, dir, max_toi) {
                        let clipped_dist = (hit.distance - CAMERA_CLIP_MARGIN).max(F_CAMERA_MIN_THIRD_PERSON_DIST);
                        actual_dist = actual_dist.min(clipped_dist);
                    }
                }

                self.current_eye = focal - forward * actual_dist;
            }
        }
    }

    /// ビュー行列 (Z-up 右手系) を計算する。
    pub fn view_matrix(&self, player_pos: Vec3) -> Mat4 {
        let focal = self.focal_point(player_pos);
        match self.mode {
            CameraViewMode::FirstPerson => {
                let forward = self.forward_vector();
                Mat4::look_at_rh(self.current_eye, self.current_eye + forward, Vec3::Z)
            }
            CameraViewMode::ThirdPerson => {
                let forward = self.forward_vector();
                let right = self.right_vector();
                let shoulder_offset = right * F_OVER_SHOULDER_POS_X + Vec3::Z * F_OVER_SHOULDER_POS_Z;
                let target = focal + shoulder_offset + forward * 1000.0;
                Mat4::look_at_rh(self.current_eye, target, Vec3::Z)
            }
            CameraViewMode::Vanity => {
                Mat4::look_at_rh(self.current_eye, focal, Vec3::Z)
            }
        }
    }

    /// 透視投影行列を計算する。
    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, self.aspect, self.z_near, self.z_far)
    }

    /// ビュー射影行列 (Projection * View) を計算する。
    pub fn view_proj_matrix(&self, player_pos: Vec3) -> Mat4 {
        self.projection_matrix() * self.view_matrix(player_pos)
    }

    /// GPU シェーダー送信用 Uniform 構造体を生成する。
    pub fn build_uniform(&self, player_pos: Vec3) -> CameraUniform {
        let vp = self.view_proj_matrix(player_pos);
        CameraUniform {
            view_proj: vp.to_cols_array(),
            camera_pos: [self.current_eye.x, self.current_eye.y, self.current_eye.z, 1.0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_camera_first_person_eye_and_vectors() {
        let mut cam = PlayerCamera::new(16.0 / 9.0);
        let player_pos = Vec3::new(100.0, 200.0, 50.0);

        cam.update(player_pos, None);

        // 一人称のカメラ位置は player_pos.z + F_1ST_PERSON_CAMERA_HEIGHT
        assert_eq!(cam.current_eye, Vec3::new(100.0, 200.0, 50.0 + F_1ST_PERSON_CAMERA_HEIGHT));

        // 初期 yaw = -PI/2, pitch = 0 -> 前方ベクトルは -Y 方向
        let fwd = cam.forward_vector();
        assert!((fwd.x).abs() < 1e-4);
        assert!((fwd.y - (-1.0)).abs() < 1e-4);
        assert!((fwd.z).abs() < 1e-4);

        let vm = cam.view_matrix(player_pos);
        assert!(vm.is_finite());
        let pm = cam.projection_matrix();
        assert!(pm.is_finite());
    }

    #[test]
    fn test_player_camera_third_person_and_zoom() {
        let mut cam = PlayerCamera::new(16.0 / 9.0);
        let player_pos = Vec3::ZERO;

        // 一人称から三人称へのトグル切り替え
        cam.toggle_view_mode();
        assert_eq!(cam.mode, CameraViewMode::ThirdPerson);
        assert_eq!(cam.distance, F_CHASE_CAMERA_DEFAULT);

        cam.update(player_pos, None);
        // 三人称のカメラ位置は後方に離れていること
        assert!(cam.current_eye.distance(Vec3::new(0.0, 0.0, F_1ST_PERSON_CAMERA_HEIGHT)) > 100.0);

        // ズームインして最小距離を下回ると一人称へ切り替わること
        cam.zoom(10.0); // 150 - 150 = 0 -> 最小距離下回り
        assert_eq!(cam.mode, CameraViewMode::FirstPerson);

        // 一人称で手前に引くと三人称へ切り替わること
        cam.zoom(-1.0);
        assert_eq!(cam.mode, CameraViewMode::ThirdPerson);
    }

    #[test]
    fn test_player_camera_wall_clipping() {
        use fo3_nif::collision::{CollisionShape, NifCollisionData, RigidBodyData};
        use fo3_nif::{Fallout3HavokMaterial, Fallout3Layer};

        let mut cam = PlayerCamera::new(16.0 / 9.0);
        let player_pos = Vec3::ZERO;
        cam.mode = CameraViewMode::ThirdPerson;
        cam.distance = 200.0;

        let mut physics = RapierPhysicsWorld::new();
        // プレイヤー後方 Y=80.0 の位置に壁コライダー (Box: 100 x 10 x 100) を配置
        let wall_body = RigidBodyData {
            shape: CollisionShape::Box {
                half_extents: [50.0, 5.0, 50.0],
                center: [0.0, 0.0, 0.0],
                material: Fallout3HavokMaterial::Stone,
            },
            mass: 0.0,
            friction: 0.5,
            restitution: 0.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            layer: Fallout3Layer::Static,
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        };
        let col_data = NifCollisionData {
            bodies: vec![wall_body],
        };
        physics.add_nif_collision(
            &col_data,
            Vec3::new(0.0, 80.0, F_1ST_PERSON_CAMERA_HEIGHT),
            glam::Quat::IDENTITY,
        );

        cam.update(player_pos, Some(&physics));

        // 理想距離は 200.0 だが、Y=80.0 に壁があるため、カメラは壁手前に制限されていること
        let focal = cam.focal_point(player_pos);
        let actual_dist = cam.current_eye.distance(focal);
        assert!(actual_dist < 100.0, "壁に遮蔽されてカメラ距離が短縮されていること: actual_dist={}", actual_dist);
    }
}
