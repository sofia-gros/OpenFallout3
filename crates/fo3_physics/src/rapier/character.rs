//! # Rapier キャラクタコントローラー
//!
//! Rapier の `KinematicCharacterController` をラップし、
//! プレイヤーの階段昇降 (オートステップ)、坂道スロープ、接地判定、衝突押し出しを提供します。
//!
//! 参照元: `knowledge/physics_engine_evaluation_and_architecture.md`

use glam::Vec3;
use rapier3d::control::{CharacterAutostep, CharacterLength, KinematicCharacterController};
use rapier3d::na::{Isometry3, Point3, Translation3, UnitQuaternion, Vector3};
use rapier3d::prelude::*;
use crate::traits::CharacterState;

/// キャラクタコントローラー設定および状態保持。
pub struct RapierCharacterController {
    /// Rapier KCC エンジンインスタンス
    pub kcc: KinematicCharacterController,
    /// キャラクタのカプセルコライダー半身高 (通常 Z 方向)
    pub half_height: f32,
    /// キャラクタのカプセル半径
    pub radius: f32,
    /// 現在のワールド位置 [X, Y, Z]
    pub position: Vec3,
    /// 前回のフレームで接地していたか
    pub is_grounded: bool,
}

impl RapierCharacterController {
    /// Fallout 3 標準プレイヤーサイズ (全高 約 128 ゲーム単位 $\approx 1.8$m) でコントローラーを生成。
    ///
    /// 半径 20.0 (約 28cm), 半身高 44.0 (全高 $44 \times 2 + 20 \times 2 = 128$)
    pub fn new(initial_pos: Vec3) -> Self {
        let half_height = 44.0;
        let radius = 20.0;

        let mut kcc = KinematicCharacterController::default();
        // Fallout 3 / Gamebryo は右手系 Z-up のため、上方向ベクトルを +Z に設定
        kcc.up = Vector3::z_axis();
        kcc.offset = CharacterLength::Absolute(0.1);
        // 階段オートステップ設定 (高さ 20 単位まで登れる)
        kcc.autostep = Some(CharacterAutostep {
            max_height: CharacterLength::Absolute(20.0),
            min_width: CharacterLength::Absolute(10.0),
            include_dynamic_bodies: false,
        });
        // 最大登坂傾斜角 (45度 $\approx 0.785$ rad)
        kcc.max_slope_climb_angle = 45.0f32.to_radians();
        // 登坂不能斜面の滑り落ち
        kcc.min_slope_slide_angle = 46.0f32.to_radians();
        // 地面スナップ吸着距離
        kcc.snap_to_ground = Some(CharacterLength::Absolute(5.0));

        Self {
            kcc,
            half_height,
            radius,
            position: initial_pos,
            is_grounded: false,
        }
    }

    /// カプセル形状を取得。
    pub fn shape(&self) -> SharedShape {
        let p1 = Point3::new(0.0, 0.0, -self.half_height);
        let p2 = Point3::new(0.0, 0.0, self.half_height);
        SharedShape::capsule(p1, p2, self.radius)
    }

    /// キャラクタの現在姿勢 (Isometry3) を取得。
    pub fn isometry(&self) -> Isometry3<Real> {
        Isometry3::from_parts(
            Translation3::new(self.position.x, self.position.y, self.position.z),
            UnitQuaternion::identity(),
        )
    }

    /// 移動シミュレーションを実行し、位置を更新する。
    pub fn step_move(
        &mut self,
        dt: f32,
        desired_translation: Vec3,
        bodies: &RigidBodySet,
        colliders: &ColliderSet,
        queries: &QueryPipeline,
    ) -> CharacterState {
        let char_shape = self.shape();
        let char_pos = self.isometry();
        let desired_vec = Vector3::new(
            desired_translation.x,
            desired_translation.y,
            desired_translation.z,
        );

        let filter = QueryFilter::default().groups(InteractionGroups::all());

        let movement = self.kcc.move_shape(
            dt,
            bodies,
            colliders,
            queries,
            char_shape.as_ref(),
            &char_pos,
            desired_vec,
            filter,
            |_| {},
        );

        let effective = Vec3::new(
            movement.translation.x,
            movement.translation.y,
            movement.translation.z,
        );

        self.position += effective;
        self.is_grounded = movement.grounded;

        CharacterState {
            is_grounded: self.is_grounded,
            effective_translation: effective,
        }
    }
}
