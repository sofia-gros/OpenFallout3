//! プレイヤーロコモーションステートマシンモジュール。
//!
//! 参照元: Gamebryo 2.6 キャラクター移動 & `meshes/characters/_male/locomotion/*.kf`

use fo3_nif::NifFile;
use fo3_render::animation::{AnimationClip, AnimationPlayer, SkeletonPose};
use fo3_vfs::VfsManager;
use std::collections::HashMap;
use std::sync::Arc;

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
