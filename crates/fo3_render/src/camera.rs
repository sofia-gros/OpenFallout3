//! # オービットカメラ (Orbit Camera)
//!
//! Fallout 3 (Gamebryo) の Z-up 右手座標系に準拠したオービット（周回）カメラ。
//! マウスによる回転・パン・ズームおよび対象メッシュへの自動フォーカスを実装。

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use std::f32::consts::{FRAC_PI_2, PI};

/// シェーダーのユニフォームバッファに転送するカメラデータ。
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct CameraUniform {
    /// ビュー射影合成行列 (Projection * View)
    pub view_proj: [f32; 16],
    /// カメラワールド座標 (XYZ, W=1.0)
    pub camera_pos: [f32; 4],
}

/// Z-up 右手座標系オービットカメラ。
#[derive(Clone, Debug)]
pub struct OrbitCamera {
    /// 注視点 (Target)
    pub target: Vec3,
    /// カメラと注視点の距離
    pub distance: f32,
    /// 仰角 (Pitch) - ラジアン
    pub pitch: f32,
    /// 方位角 (Yaw) - ラジアン
    pub yaw: f32,
    /// 垂直画角 (Field of view) - ラジアン
    pub fov_y: f32,
    /// アスペクト比 (Width / Height)
    pub aspect: f32,
    /// 前方クリップ距離
    pub z_near: f32,
    /// 後方クリップ距離
    pub z_far: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            distance: 100.0,
            pitch: 0.3,
            yaw: -FRAC_PI_2,
            fov_y: 60.0_f32.to_radians(),
            aspect: 16.0 / 9.0,
            z_near: 0.1,
            z_far: 10000.0,
        }
    }
}

impl OrbitCamera {
    pub fn new(aspect: f32) -> Self {
        Self {
            aspect,
            ..Default::default()
        }
    }

    /// カメラの 3D ワールド座標を計算。
    pub fn eye_position(&self) -> Vec3 {
        let x = self.distance * self.pitch.cos() * self.yaw.cos();
        let y = self.distance * self.pitch.cos() * self.yaw.sin();
        let z = self.distance * self.pitch.sin();
        self.target + Vec3::new(x, y, z)
    }

    /// ビュー行列 (Z-up) を計算。
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye_position(), self.target, Vec3::Z)
    }

    /// 透視投影行列を計算。
    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, self.aspect, self.z_near, self.z_far)
    }

    /// ビュー射影行列 (Projection * View) を計算。
    pub fn view_proj_matrix(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// シェーダー送信用 Uniform 構造体を生成。
    pub fn build_uniform(&self) -> CameraUniform {
        let vp = self.view_proj_matrix();
        let eye = self.eye_position();
        CameraUniform {
            view_proj: vp.to_cols_array(),
            camera_pos: [eye.x, eye.y, eye.z, 1.0],
        }
    }

    /// 方位角・仰角をマウス移動量に応じて更新。
    pub fn rotate(&mut self, delta_x: f32, delta_y: f32) {
        let sensitivity = 0.005;
        self.yaw += delta_x * sensitivity;
        self.pitch += delta_y * sensitivity;

        // ジンバルロック防止のため、真上・真下付近を制限
        let limit = FRAC_PI_2 - 0.01;
        self.pitch = self.pitch.clamp(-limit, limit);

        // yaw を 0 〜 2π に正規化
        if self.yaw > PI * 2.0 {
            self.yaw -= PI * 2.0;
        } else if self.yaw < 0.0 {
            self.yaw += PI * 2.0;
        }
    }

    /// ズーム（距離の変更）。
    pub fn zoom(&mut self, delta: f32) {
        let zoom_factor = 1.1_f32;
        if delta > 0.0 {
            self.distance /= zoom_factor;
        } else if delta < 0.0 {
            self.distance *= zoom_factor;
        }
        self.distance = self.distance.clamp(1.0, 5000.0);
    }

    /// カメラの注視点をスクリーン平面に沿ってパン移動。
    pub fn pan(&mut self, delta_x: f32, delta_y: f32) {
        let factor = self.distance * 0.002;
        // カメラの右方向ベクトル
        let right = Vec3::new(-self.yaw.sin(), self.yaw.cos(), 0.0).normalize();
        // カメラの上方向ベクトル (Z-up 空間における右ベクトルと視線ベクトルの外積)
        let forward = (self.target - self.eye_position()).normalize();
        let up = right.cross(forward).normalize();

        self.target -= right * (delta_x * factor);
        self.target += up * (delta_y * factor);
    }

    /// メッシュのバウンディングボリュームに合わせてカメラの注視点と距離を自動設定。
    pub fn focus(&mut self, center: Vec3, radius: f32) {
        self.target = center;
        let r = radius.max(10.0);
        // 画角内に収まる距離を計算
        self.distance = r / (self.fov_y * 0.5).sin() * 1.5;
        self.pitch = 0.3;
        self.yaw = -FRAC_PI_2;
    }
}
