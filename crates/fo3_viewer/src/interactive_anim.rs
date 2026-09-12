//! # インタラクティブオブジェクト開閉アニメーション & 物理同期コントローラー
//!
//! Fallout 3 (Gamebryo 2.6) のドア（Vault ギアドア、スライディングドア、ヒンジドア）やコンテナの開閉アニメーションと、
//! それに伴う Rapier 剛体（`MO_SYS_KEYFRAMED` 相当）のトランスフォーム追従同期を管理する。
//!
//! 参照元:
//! - `references/nifskope/src/gl/controllers.cpp:L50-120` (`ControllerManager::setSequence`)
//! - Gamebryo 2.6 `NiAVObject::Update`, `NiControllerSequence`
//! - Gamebryo 2.6 `bhkRigidBody` (`MO_SYS_KEYFRAMED`) 位置同期
//! - `knowledge/gamebryo_resource_management_and_caching.md`

use glam::{Mat4, Quat, Vec3};
use fo3_physics::RigidBodyHandle;
use fo3_render::animation::AnimationClip;
use fo3_nif::NifFile;
use std::f32::consts::PI;
use std::sync::Arc;

/// オブジェクトの開閉状態。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenState {
    /// 完全に閉じている
    Closed,
    /// 開くアニメーション中
    Opening,
    /// 完全に開いている
    Open,
    /// 閉じるアニメーション中
    Closing,
}

/// NIF 内部の可動パーツ情報（ドア板、ギア、ボルト、蓋など）。
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct MovingPartBinding {
    /// この可動パーツに属するメッシュインデックス一覧
    pub mesh_indices: Vec<usize>,
    /// この可動パーツに属する物理剛体一覧
    pub rigid_bodies: Vec<RigidBodyHandle>,
    /// 制御対象ノード名（例: "VDoorDoor01", "VDoorGear01", "Lid" 等）
    pub node_name: String,
    /// ノードのローカル初期バインドポーズ
    pub local_bind_matrix: Mat4,
    /// ピボット中心位置（ローカル座標）
    pub pivot_offset: Vec3,
    /// 回転軸（ローカル座標、ヒンジ回転用）
    pub rotation_axis: Vec3,
    /// 最大回転角（ラジアン）
    pub max_angle: f32,
}

/// 単一配置オブジェクト (REFR) に紐付くメッシュインデックスおよび物理剛体。
#[derive(Clone, Debug, Default)]
pub struct RefrBinding {
    /// REFR の FormID
    pub form_id: u32,
    /// 固定（動かない）メッシュのインデックス一覧（ドア枠、コンテナ本体など）
    pub static_mesh_indices: Vec<usize>,
    /// 固定（動かない）物理剛体一覧（ドア枠のコライダーなど）
    pub static_rigid_bodies: Vec<RigidBodyHandle>,
    /// 可動パーツ群
    pub moving_parts: Vec<MovingPartBinding>,
    /// 初期配置位置
    pub base_translation: Vec3,
    /// 初期配置回転
    pub base_rotation: Quat,
    /// NIF 組み込みの "Open" アニメーションクリップ（存在する場合）
    pub open_clip: Option<AnimationClip>,
    /// NIF 組み込みの "Close" アニメーションクリップ（存在する場合）
    pub close_clip: Option<AnimationClip>,
    /// NIF ファイルデータ（キーフレーム補間計算用）
    pub nif: Option<Arc<NifFile>>,
}

/// 単一の開閉インタラクティブオブジェクトのアニメーション追従状態。
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct InteractiveAnimator {
    /// 対象 REFR の FormID
    pub form_id: u32,
    /// 現在の開閉状態
    pub state: OpenState,
    /// 開閉の進行度 (0.0 = 密閉/閉, 1.0 = 全開)
    pub progress: f32,
    /// アニメーション所要時間（秒）
    pub duration: f32,
    /// 固定メッシュ一覧（動かさない）
    pub static_mesh_indices: Vec<usize>,
    /// 固定剛体一覧（動かさない）
    pub static_rigid_bodies: Vec<RigidBodyHandle>,
    /// 可動パーツ群
    pub moving_parts: Vec<MovingPartBinding>,
    /// 配置時の初期位置
    pub base_translation: Vec3,
    /// 配置時の初期回転
    pub base_rotation: Quat,
    /// "Open" シーケンスクリップ
    pub open_clip: Option<AnimationClip>,
    /// "Close" シーケンスクリップ
    pub close_clip: Option<AnimationClip>,
    /// NIF データ
    pub nif: Option<Arc<NifFile>>,
}

impl InteractiveAnimator {
    /// RefrBinding から開閉アニメーターを構築する。
    pub fn from_binding(binding: &RefrBinding, is_open: bool) -> Self {
        let (state, progress) = if is_open {
            (OpenState::Open, 1.0)
        } else {
            (OpenState::Closed, 0.0)
        };

        // アニメーション所要時間を決定（NIF の Open シーケンスの duration を優先、なければ 0.8 秒）
        let duration = if let Some(ref clip) = binding.open_clip {
            clip.duration.max(0.2)
        } else {
            0.8
        };

        Self {
            form_id: binding.form_id,
            state,
            progress,
            duration,
            static_mesh_indices: binding.static_mesh_indices.clone(),
            static_rigid_bodies: binding.static_rigid_bodies.clone(),
            moving_parts: binding.moving_parts.clone(),
            base_translation: binding.base_translation,
            base_rotation: binding.base_rotation,
            open_clip: binding.open_clip.clone(),
            close_clip: binding.close_clip.clone(),
            nif: binding.nif.clone(),
        }
    }

    /// 現在アニメーション中（開閉動作中）かどうかを判定する。
    pub fn is_animating(&self) -> bool {
        matches!(self.state, OpenState::Opening | OpenState::Closing)
    }

    /// 開閉をトグルする（閉じている/閉動作中なら開く、開いている/開動作中なら閉じる）。
    pub fn toggle(&mut self) {
        match self.state {
            OpenState::Closed | OpenState::Closing => {
                self.state = OpenState::Opening;
            }
            OpenState::Open | OpenState::Opening => {
                self.state = OpenState::Closing;
            }
        }
    }

    /// 経過秒数 `dt` を進め、進行度を更新する。
    /// 状態が変化（移動）した場合は `true` を返す。
    pub fn update(&mut self, dt: f32) -> bool {
        let speed = 1.0 / self.duration.max(0.01);
        match self.state {
            OpenState::Opening => {
                self.progress += speed * dt;
                if self.progress >= 1.0 {
                    self.progress = 1.0;
                    self.state = OpenState::Open;
                }
                true
            }
            OpenState::Closing => {
                self.progress -= speed * dt;
                if self.progress <= 0.0 {
                    self.progress = 0.0;
                    self.state = OpenState::Closed;
                }
                true
            }
            OpenState::Closed | OpenState::Open => false,
        }
    }

    /// 各可動パーツの現在のワールド変換行列（位置・回転）を計算する。
    /// 戻り値: 各パーツごとの `(mesh_indices, rigid_bodies, world_matrix, world_pos, world_rot)`
    pub fn compute_part_transforms(&self) -> Vec<(&[usize], &[RigidBodyHandle], Mat4, Vec3, Quat)> {
        let mut results = Vec::with_capacity(self.moving_parts.len());

        for part in &self.moving_parts {
            // 1. NIF シーケンスによる補間が利用可能か判定
            let (part_pos, part_rot) = if let (Some(ref clip), Some(ref nif)) = (&self.open_clip, &self.nif) {
                // シーケンス内タイムライン時刻
                let t = self.progress * clip.duration;
                if let Some(transform) = clip.sample_bone_transform(&part.node_name, t, nif) {
                    let local_rot = transform.rotation.unwrap_or(Quat::IDENTITY);
                    let local_pos = transform.translation.unwrap_or(Vec3::ZERO);
                    let world_rot = self.base_rotation * local_rot;
                    let world_pos = self.base_translation + (self.base_rotation * local_pos);
                    (world_pos, world_rot)
                } else {
                    self.evaluate_hinge(part)
                }
            } else {
                self.evaluate_hinge(part)
            };

            let world_mat = Mat4::from_rotation_translation(part_rot, part_pos);
            results.push((
                part.mesh_indices.as_slice(),
                part.rigid_bodies.as_slice(),
                world_mat,
                part_pos,
                part_rot,
            ));
        }

        results
    }

    /// ヒンジ回転フォールバックによるパーツトランスフォーム評価。
    fn evaluate_hinge(&self, part: &MovingPartBinding) -> (Vec3, Quat) {
        // スムーズなイージング (Sine)
        let smooth_t = (self.progress * PI * 0.5).sin();
        let current_angle = part.max_angle * smooth_t;

        let local_rot = Quat::from_axis_angle(part.rotation_axis, current_angle);
        let world_rot = self.base_rotation * local_rot;

        let rotated_pivot = world_rot * part.pivot_offset;
        let original_pivot = self.base_rotation * part.pivot_offset;
        let world_pos = self.base_translation + (original_pivot - rotated_pivot);

        (world_pos, world_rot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interactive_animator_toggle_and_update() {
        let binding = RefrBinding {
            form_id: 0x1234,
            static_mesh_indices: vec![],
            static_rigid_bodies: vec![],
            moving_parts: vec![MovingPartBinding {
                mesh_indices: vec![0],
                rigid_bodies: vec![],
                node_name: "Door".to_string(),
                local_bind_matrix: Mat4::IDENTITY,
                pivot_offset: Vec3::ZERO,
                rotation_axis: Vec3::Z,
                max_angle: -PI * 0.5,
            }],
            base_translation: Vec3::new(10.0, 20.0, 30.0),
            base_rotation: Quat::IDENTITY,
            open_clip: None,
            close_clip: None,
            nif: None,
        };
        let mut anim = InteractiveAnimator::from_binding(&binding, false);

        assert_eq!(anim.state, OpenState::Closed);
        assert_eq!(anim.progress, 0.0);

        // トグルで開く
        anim.toggle();
        assert_eq!(anim.state, OpenState::Opening);

        // 0.4 秒更新 (進行度 0.5)
        let changed = anim.update(0.4);
        assert!(changed);
        assert!((anim.progress - 0.5).abs() < 0.01);

        // さらに 0.5 秒更新で全開到達
        anim.update(0.5);
        assert_eq!(anim.state, OpenState::Open);
        assert_eq!(anim.progress, 1.0);

        // 開いた状態では更新なし
        assert!(!anim.update(0.1));

        // 再度トグルで閉じる
        anim.toggle();
        assert_eq!(anim.state, OpenState::Closing);

        // 1.0 秒更新で全閉到達
        anim.update(1.0);
        assert_eq!(anim.state, OpenState::Closed);
        assert_eq!(anim.progress, 0.0);
    }
}
