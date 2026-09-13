//! # プレイヤーアクター & ロコモーションステートマシン (`PlayerActor`, `LocomotionStateMachine`)
//!
//! Fallout 3 (Gamebryo 2.6) の実機仕様に準拠したプレイヤーアクター。
//! 三人称全身モデル（スケルトン、Vault 101 ジャンプスーツ、頭部、髪、目、手）および一人称腕モデルの
//! パーツ合成、実機 KF 駆動ロコモーションアニメーション、視点モードに応じたカリングを制御する。
//!
//! 参照元:
//! - `Fallout3.esm`: `0x00000007` (`PlayerRef`), `0x00000014` (`Player`)
//! - 実機アーカイブ: `Fallout - Meshes.bsa` (`characters\_male\locomotion\*.kf`, `characters\_1stperson\skeleton.nif`)
//! - Gamebryo 2.6 `NiControllerSequence` & マルチパーツアクター仕様

use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::Arc;
use glam::{Quat, Vec3};
use fo3_gamebryo_core::NiTransform;
use fo3_nif::NifFile;
use fo3_render::animation::{AnimationClip, AnimationPlayer, SkeletonPose};
use fo3_render::scene::actor::RenderActorInstance;
use fo3_render::RenderMesh;
use fo3_vfs::VfsManager;
use crate::camera::CameraViewMode;

/// ロコモーションの移動・行動ステート
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LocomotionState {
    /// 静止待機 (mtidle.kf)
    Idle,
    /// 前進歩行 (mtforward.kf)
    WalkForward,
    /// 前進走行 (mtfastforward.kf)
    RunForward,
    /// 後退歩行 (mtbackward.kf)
    WalkBackward,
    /// 後退走行 (mtfastbackward.kf)
    RunBackward,
    /// 左歩行 (mtleft.kf)
    WalkLeft,
    /// 左走行 (mtfastleft.kf)
    RunLeft,
    /// 右歩行 (mtright.kf)
    WalkRight,
    /// 右走行 (mtfastright.kf)
    RunRight,
    /// 左旋回 (mtturnleft.kf)
    TurnLeft,
    /// 右旋回 (mtturnright.kf)
    TurnRight,
    /// ジャンプ開始 (mtjumpstart.kf)
    JumpStart,
    /// 滞空ループ (mtjumploop.kf)
    JumpLoop,
    /// 着地 (mtjumpland.kf)
    JumpLand,
    /// しゃがみ静止待機 (sneakmtidle.kf)
    SneakIdle,
    /// しゃがみ前進歩行 (sneakmtforward.kf)
    SneakWalkForward,
    /// しゃがみ前進走行 (sneakmtfastforward.kf)
    SneakRunForward,
    /// しゃがみ後退 (sneakmtbackward.kf)
    SneakBackward,
    /// しゃがみ左歩行 (sneakmtleft.kf)
    SneakLeft,
    /// しゃがみ右歩行 (sneakmtright.kf)
    SneakRight,
}

impl LocomotionState {
    /// 対応する実機 KF ファイルの相対パスを返す。
    pub fn kf_relative_path(&self) -> &'static str {
        match self {
            Self::Idle => "meshes\\characters\\_male\\locomotion\\mtidle.kf",
            Self::WalkForward => "meshes\\characters\\_male\\locomotion\\mtforward.kf",
            Self::RunForward => "meshes\\characters\\_male\\locomotion\\mtfastforward.kf",
            Self::WalkBackward => "meshes\\characters\\_male\\locomotion\\mtbackward.kf",
            Self::RunBackward => "meshes\\characters\\_male\\locomotion\\mtfastbackward.kf",
            Self::WalkLeft => "meshes\\characters\\_male\\locomotion\\mtleft.kf",
            Self::RunLeft => "meshes\\characters\\_male\\locomotion\\mtfastleft.kf",
            Self::WalkRight => "meshes\\characters\\_male\\locomotion\\mtright.kf",
            Self::RunRight => "meshes\\characters\\_male\\locomotion\\mtfastright.kf",
            Self::TurnLeft => "meshes\\characters\\_male\\locomotion\\mtturnleft.kf",
            Self::TurnRight => "meshes\\characters\\_male\\locomotion\\mtturnright.kf",
            Self::JumpStart => "meshes\\characters\\_male\\locomotion\\mtjumpstart.kf",
            Self::JumpLoop => "meshes\\characters\\_male\\locomotion\\mtjumploop.kf",
            Self::JumpLand => "meshes\\characters\\_male\\locomotion\\mtjumpland.kf",
            Self::SneakIdle => "meshes\\characters\\_male\\sneakmtidle.kf",
            Self::SneakWalkForward => "meshes\\characters\\_male\\sneakmtforward.kf",
            Self::SneakRunForward => "meshes\\characters\\_male\\sneakmtfastforward.kf",
            Self::SneakBackward => "meshes\\characters\\_male\\sneakmtbackward.kf",
            Self::SneakLeft => "meshes\\characters\\_male\\sneakmtleft.kf",
            Self::SneakRight => "meshes\\characters\\_male\\sneakmtright.kf",
        }
    }
}

/// 実機 KF アニメーションを駆動するロコモーションステートマシン
#[derive(Clone, Debug)]
pub struct LocomotionStateMachine {
    /// 現在の移動ステート
    pub current_state: LocomotionState,
    /// 現在ステートの経過時間 (秒)
    pub state_elapsed: f32,
    /// ステート別アニメーションプレイヤーと KF ファイルのキャッシュ
    pub players: HashMap<LocomotionState, (AnimationPlayer, Arc<NifFile>)>,
    /// 前回のステート（遷移検出用）
    pub prev_state: LocomotionState,
}

impl LocomotionStateMachine {
    /// VFS から実機ロコモーション KF をロードしてステートマシンを初期化する。
    pub fn new(vfs: &mut VfsManager) -> Self {
        let mut players = HashMap::new();
        let states = [
            LocomotionState::Idle,
            LocomotionState::WalkForward,
            LocomotionState::RunForward,
            LocomotionState::WalkBackward,
            LocomotionState::RunBackward,
            LocomotionState::WalkLeft,
            LocomotionState::RunLeft,
            LocomotionState::WalkRight,
            LocomotionState::RunRight,
            LocomotionState::JumpStart,
            LocomotionState::JumpLoop,
            LocomotionState::JumpLand,
            LocomotionState::SneakIdle,
            LocomotionState::SneakWalkForward,
            LocomotionState::SneakRunForward,
            LocomotionState::SneakBackward,
            LocomotionState::SneakLeft,
            LocomotionState::SneakRight,
        ];

        for state in states {
            let path = state.kf_relative_path();
            if let Ok(bytes) = vfs.read(path) {
                if let Ok(kf_nif) = NifFile::read(&mut std::io::Cursor::new(bytes)) {
                    if let Some(clip) = AnimationClip::from_kf(&kf_nif) {
                        let player = AnimationPlayer::new(clip);
                        players.insert(state, (player, Arc::new(kf_nif)));
                    }
                }
            }
        }

        Self {
            current_state: LocomotionState::Idle,
            state_elapsed: 0.0,
            players,
            prev_state: LocomotionState::Idle,
        }
    }

    /// 入力および接地状態から次の移動ステートを決定する。
    pub fn update_state(
        &mut self,
        forward: bool,
        backward: bool,
        left: bool,
        right: bool,
        running: bool,
        jumping: bool,
        sneaking: bool,
        grounded: bool,
    ) {
        let next_state = if !grounded {
            // 空中滞空
            if self.current_state == LocomotionState::JumpStart && self.state_elapsed < 0.3 {
                LocomotionState::JumpStart
            } else {
                LocomotionState::JumpLoop
            }
        } else if jumping {
            LocomotionState::JumpStart
        } else if sneaking {
            // しゃがみ (Sneak) ステート
            if forward {
                if running {
                    LocomotionState::SneakRunForward
                } else {
                    LocomotionState::SneakWalkForward
                }
            } else if backward {
                LocomotionState::SneakBackward
            } else if left {
                LocomotionState::SneakLeft
            } else if right {
                LocomotionState::SneakRight
            } else {
                LocomotionState::SneakIdle
            }
        } else if forward {
            if running {
                LocomotionState::RunForward
            } else {
                LocomotionState::WalkForward
            }
        } else if backward {
            if running {
                LocomotionState::RunBackward
            } else {
                LocomotionState::WalkBackward
            }
        } else if left {
            if running {
                LocomotionState::RunLeft
            } else {
                LocomotionState::WalkLeft
            }
        } else if right {
            if running {
                LocomotionState::RunRight
            } else {
                LocomotionState::WalkRight
            }
        } else {
            LocomotionState::Idle
        };

        if next_state != self.current_state {
            self.prev_state = self.current_state;
            self.current_state = next_state;
            self.state_elapsed = 0.0;
            if let Some((player, _)) = self.players.get_mut(&self.current_state) {
                player.seek(0.0);
            }
        }
    }

    /// アニメーション時間を進め、現在のボーン姿勢 (SkeletonPose) を算出する。
    pub fn sample_pose(&mut self, dt: f32, out_pose: &mut SkeletonPose) {
        self.state_elapsed += dt;
        let state = if self.players.contains_key(&self.current_state) {
            self.current_state
        } else {
            LocomotionState::Idle
        };

        if let Some((player, kf)) = self.players.get_mut(&state) {
            player.update(kf, dt, out_pose);
        }
    }
}

/// プレイヤーアクター制御構造体
pub struct PlayerActor {
    /// プレイヤーの位置 (ワールド座標)
    pub position: Vec3,
    /// プレイヤーの水平向き (Yaw - ラジアン)
    pub yaw: f32,
    /// 目標水平向き (滑らかな旋回用)
    pub target_yaw: f32,
    /// ロコモーションステートマシン
    pub state_machine: LocomotionStateMachine,
    /// 三人称全身アクターの Scene Actors インデックス
    pub third_person_actor_idx: usize,
    /// 一人称腕アクターの Scene Actors インデックス
    pub first_person_arms_actor_idx: Option<usize>,
    /// 三人称全身モデルに属するメッシュインデックス一覧
    pub third_person_mesh_indices: Vec<usize>,
    /// 一人称腕モデルに属するメッシュインデックス一覧
    pub first_person_mesh_indices: Vec<usize>,
    /// 現在の視点モード
    pub view_mode: CameraViewMode,
}

impl PlayerActor {
    /// 新しいプレイヤーアクターを作成する。
    pub fn new(
        position: Vec3,
        yaw: f32,
        vfs: &mut VfsManager,
        third_person_actor_idx: usize,
        first_person_arms_actor_idx: Option<usize>,
        third_person_mesh_indices: Vec<usize>,
        first_person_mesh_indices: Vec<usize>,
    ) -> Self {
        let state_machine = LocomotionStateMachine::new(vfs);
        Self {
            position,
            yaw,
            target_yaw: yaw,
            state_machine,
            third_person_actor_idx,
            first_person_arms_actor_idx,
            third_person_mesh_indices,
            first_person_mesh_indices,
            view_mode: CameraViewMode::FirstPerson,
        }
    }

    /// カメラ視点モードを切り替え、メッシュの可視性を設定する。
    pub fn set_view_mode(&mut self, mode: CameraViewMode, meshes: &mut [RenderMesh]) {
        self.view_mode = mode;
        self.apply_mesh_visibility(meshes);
    }

    /// 現在の視点モードに応じてメッシュの表示・非表示を適用する。
    /// 一人称時: 三人称全身モデルを非表示、一人称腕を表示。
    /// 三人称時: 三人称全身モデルを表示、一人称腕を非表示。
    pub fn apply_mesh_visibility(&self, meshes: &mut [RenderMesh]) {
        match self.view_mode {
            CameraViewMode::FirstPerson => {
                // 三人称全身メッシュを不可視化 (非表示)
                for &idx in &self.third_person_mesh_indices {
                    if idx < meshes.len() {
                        meshes[idx].is_visible = false;
                    }
                }
                // 一人称腕メッシュを可視化
                for &idx in &self.first_person_mesh_indices {
                    if idx < meshes.len() {
                        meshes[idx].is_visible = true;
                    }
                }
            }
            CameraViewMode::ThirdPerson | CameraViewMode::Vanity => {
                // 三人称全身メッシュを可視化
                for &idx in &self.third_person_mesh_indices {
                    if idx < meshes.len() {
                        meshes[idx].is_visible = true;
                    }
                }
                // 一人称腕メッシュを不可視化
                for &idx in &self.first_person_mesh_indices {
                    if idx < meshes.len() {
                        meshes[idx].is_visible = false;
                    }
                }
            }
        }
    }

    /// 毎フレームの更新処理。
    /// 位置、旋回角、アニメーション姿勢、および GPU スキニングバッファを同期更新する。
    pub fn update(
        &mut self,
        dt: f32,
        new_pos: Vec3,
        camera_yaw: f32,
        is_moving: bool,
        move_dir: Option<Vec3>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        actors: &mut [RenderActorInstance],
        meshes: &mut [RenderMesh],
    ) {
        self.position = new_pos;

        // アクターの旋回角制御
        match self.view_mode {
            CameraViewMode::FirstPerson => {
                // 一人称: カメラの向きに完全同期
                self.yaw = camera_yaw;
                self.target_yaw = camera_yaw;
            }
            CameraViewMode::ThirdPerson | CameraViewMode::Vanity => {
                // 三人称: 移動中は移動方向へスムーズに旋回
                if is_moving {
                    if let Some(dir) = move_dir {
                        if dir.length_squared() > 1e-4 {
                            self.target_yaw = dir.y.atan2(dir.x);
                        }
                    }
                }
                // 角度差を -π 〜 +π に収めて補間 (Lerp)
                let diff = (self.target_yaw - self.yaw + PI).rem_euclid(2.0 * PI) - PI;
                self.yaw += diff * (10.0 * dt).min(1.0);
            }
        }

        // ワールド変換行列の構築 (Z-up, Z軸回転)
        // 参照元: Gamebryo 2.6 座標系 (Right=+X, Forward=+Y, Up=+Z)
        // カメラおよび移動方向の基準軸 (+X) とモデルの正面 (+Y) を一致させるため、
        // 水平向きに -90度 (-π/2) の回転オフセットを合成する。
        let model_yaw = self.yaw - std::f32::consts::FRAC_PI_2;
        let world_rot = Quat::from_rotation_z(model_yaw);
        let world_transform = NiTransform {
            rotation: glam::Mat3::from_quat(world_rot),
            translation: self.position,
            scale: 1.0,
        };

        // 三人称アクターの更新
        if self.third_person_actor_idx < actors.len() {
            let actor = &mut actors[self.third_person_actor_idx];
            actor.world_transform = world_transform.clone();

            // 単体 anim_player による姿勢上書きを防止 (ロコモーションステートマシン側で統括管理)
            actor.anim_player = None;

            // ステートマシンからボーン姿勢をサンプリング
            self.state_machine.sample_pose(dt, &mut actor.anim_pose);

            // アクターのボーンマップ再計算 & GPU スキニング更新
            actor.update(0.0, device, queue, meshes);
        }

        // 一人称腕アクターの更新 (カメラ手前に配置)
        if let Some(arm_idx) = self.first_person_arms_actor_idx {
            if arm_idx < actors.len() {
                let arm_actor = &mut actors[arm_idx];
                // 一人称腕も同様に -90度オフセットを適用してカメラ視線正面に位置合わせ
                let arm_yaw = camera_yaw - std::f32::consts::FRAC_PI_2;
                let arm_transform = NiTransform {
                    rotation: glam::Mat3::from_quat(Quat::from_rotation_z(arm_yaw)),
                    translation: self.position,
                    scale: 1.0,
                };
                arm_actor.world_transform = arm_transform;
                arm_actor.update(dt, device, queue, meshes);
            }
        }
    }
}

/// 実機アセットからプレイヤーアクター（三人称全身モデル + 一人称腕モデル）を組み立て、
/// シーンに追加して `PlayerActor` を生成する。
///
/// 参照元:
/// - Fallout 3 `0x00000007` (PlayerRef), `0x00000014` (Player)
/// - Vault 101 ジャンプスーツ、標準男性スケルトン
pub fn build_player_actor(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    context: &fo3_render::RenderContext,
    vfs: &mut VfsManager,
    nif_cache: &mut fo3_render::NifCache,
    texture_cache: &mut HashMap<String, fo3_render::GpuTexture>,
    scene: &mut fo3_render::RenderScene,
    spawn_pos: Vec3,
    spawn_yaw: f32,
) -> Option<PlayerActor> {
    let skel_path = "meshes\\characters\\_male\\skeleton.nif";
    let skeleton_nif = nif_cache.get_or_load(skel_path, vfs).ok()?;

    // 1. 三人称全身モデルのパーツ群 (Fallout 3 実機アセット)
    let candidate_parts = [
        "meshes\\armor\\vaultsuit\\m\\outfit.nif",
        "meshes\\characters\\head\\headhuman.nif",
        "meshes\\characters\\hair\\hairwastelandm.nif",
        "meshes\\characters\\head\\eyelright.nif",
        "meshes\\characters\\head\\eyelleft.nif",
        "meshes\\characters\\head\\teethupper.nif",
        "meshes\\characters\\head\\teethlower.nif",
        "meshes\\characters\\head\\tongue.nif",
        "meshes\\characters\\_male\\righthand.nif",
        "meshes\\characters\\_male\\lefthand.nif",
    ];
    let mut third_person_parts = Vec::new();
    for part_path in candidate_parts {
        if let Ok(nif) = nif_cache.get_or_load(part_path, vfs) {
            third_person_parts.push(nif);
        }
    }

    if third_person_parts.is_empty() {
        return None;
    }

    let third_mesh_start = scene.meshes.len();
    let initial_transform = NiTransform {
        rotation: glam::Mat3::from_quat(Quat::from_rotation_z(spawn_yaw - std::f32::consts::FRAC_PI_2)),
        translation: spawn_pos,
        scale: 1.0,
    };

    // 初期待機アニメーション (mtidle.kf)
    let idle_kf = nif_cache.get_or_load("meshes\\characters\\_male\\locomotion\\mtidle.kf", vfs).ok();
    let idle_clip = idle_kf.as_ref().and_then(|kf| AnimationClip::from_kf(kf)).map(Arc::new);

    let third_actor_idx = scene.add_actor(
        device,
        queue,
        context,
        vfs,
        0x00000007,
        "Player",
        &initial_transform,
        skeleton_nif.clone(),
        third_person_parts,
        idle_kf,
        idle_clip,
        texture_cache,
        Some([70, 50, 35]), // プレイヤー標準髪色 (ダークブラウン)
        false,
        None,
        None,
        None,
        None,
    );
    let third_mesh_end = scene.meshes.len();
    let third_person_mesh_indices: Vec<usize> = (third_mesh_start..third_mesh_end).collect();

    // 2. 一人称腕モデルの組み立て
    let mut first_person_mesh_indices = Vec::new();
    let mut first_actor_idx = None;

    let first_skel_path = "meshes\\characters\\_1stperson\\skeleton.nif";
    if let Ok(first_skel_nif) = nif_cache.get_or_load(first_skel_path, vfs) {
        let first_part_candidates = [
            "meshes\\characters\\_1stperson\\1stpersonarms.nif",
            "meshes\\characters\\_male\\righthand.nif",
            "meshes\\characters\\_male\\lefthand.nif",
            "meshes\\armor\\vaultsuit\\m\\1stpersonoutfit.nif",
            "meshes\\armor\\vaultsuit\\m\\outfit.nif",
        ];
        let mut first_parts = Vec::new();
        for p in first_part_candidates {
            if let Ok(nif) = nif_cache.get_or_load(p, vfs) {
                first_parts.push(nif);
            }
        }

        if !first_parts.is_empty() {
            let first_mesh_start = scene.meshes.len();
            // 実機 Fallout 3 一人称待機 KF (meshes\characters\_1stperson\mtidle.kf)
            let first_idle_kf = nif_cache.get_or_load("meshes\\characters\\_1stperson\\mtidle.kf", vfs).ok();
            let first_idle_clip = first_idle_kf.as_ref().and_then(|kf| AnimationClip::from_kf(kf)).map(Arc::new);

            let first_transform = NiTransform {
                rotation: glam::Mat3::from_quat(Quat::from_rotation_z(spawn_yaw - std::f32::consts::FRAC_PI_2)),
                translation: spawn_pos,
                scale: 1.0,
            };

            let arm_idx = scene.add_actor(
                device,
                queue,
                context,
                vfs,
                0x00000008,
                "PlayerFirstPersonArms",
                &first_transform,
                first_skel_nif,
                first_parts,
                first_idle_kf,
                first_idle_clip,
                texture_cache,
                None,
                false,
                None,
                None,
                None,
                None,
            );
            let first_mesh_end = scene.meshes.len();
            first_person_mesh_indices = (first_mesh_start..first_mesh_end).collect();
            first_actor_idx = Some(arm_idx);
        }
    }

    let player = PlayerActor::new(
        spawn_pos,
        spawn_yaw,
        vfs,
        third_actor_idx,
        first_actor_idx,
        third_person_mesh_indices,
        first_person_mesh_indices,
    );

    // デフォルトの一人称可視性を適用
    player.apply_mesh_visibility(&mut scene.meshes);

    println!(
        "プレイヤーアクター生成完了: 三人称メッシュ {} 件, 一人称メッシュ {} 件",
        player.third_person_mesh_indices.len(),
        player.first_person_mesh_indices.len()
    );

    Some(player)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locomotion_state_transitions() {
        let mut vfs = VfsManager::new();
        let mut sm = LocomotionStateMachine::new(&mut vfs);

        // 初期ステートは Idle
        assert_eq!(sm.current_state, LocomotionState::Idle);

        // 前進歩行 (Wキー、Shiftなし、しゃがみなし)
        sm.update_state(true, false, false, false, false, false, false, true);
        assert_eq!(sm.current_state, LocomotionState::WalkForward);

        // 前進走行 (Wキー + Shift、しゃがみなし)
        sm.update_state(true, false, false, false, true, false, false, true);
        assert_eq!(sm.current_state, LocomotionState::RunForward);

        // しゃがみ待機 (Ctrlキー、移動なし)
        sm.update_state(false, false, false, false, false, false, true, true);
        assert_eq!(sm.current_state, LocomotionState::SneakIdle);

        // しゃがみ前進歩行 (Wキー + Ctrl)
        sm.update_state(true, false, false, false, false, false, true, true);
        assert_eq!(sm.current_state, LocomotionState::SneakWalkForward);

        // ジャンプ (Space)
        sm.update_state(true, false, false, false, true, true, false, true);
        assert_eq!(sm.current_state, LocomotionState::JumpStart);

        // 空中滞空
        sm.state_elapsed = 0.5;
        sm.update_state(true, false, false, false, true, false, false, false);
        assert_eq!(sm.current_state, LocomotionState::JumpLoop);

        // 着地してキー入力なし -> Idle
        sm.update_state(false, false, false, false, false, false, false, true);
        assert_eq!(sm.current_state, LocomotionState::Idle);
    }
}
