//! Gamebryo 2.6 準拠のアニメーション複数シーケンス合成およびトラックマネージャー。
//!
//! ボディモーション（歩行・待機など）とフェイシャル表情・リップシンク（発話）の各 KF シーケンスを
//! 優先度（Priority）とウェイト（Weight）に基づいてブレンド合成する。
//!
//! 参照元:
//! - `references/nifxml/nif.xml:L4195` (`NiControllerManager`)
//! - `references/nifxml/nif.xml:L4214` (`NiControllerSequence`)
//! - Gamebryo 2.6 `NiControllerManager.h`, `NiControllerSequence.h`
//! - `knowledge/animation_sequence_blending.md`

use std::sync::Arc;

use fo3_nif::NifFile;
use crate::animation::{AnimationPlayer, BoneTransformOverride, SkeletonPose};

/// アニメーションシーケンスの再生トラック情報。
///
/// 参照元: Gamebryo 2.6 `NiControllerSequence`, references/nifxml/nif.xml:L4214
#[derive(Clone, Debug)]
pub struct SequenceTrack {
    /// シーケンス識別名 (例: "Idle", "Walk", "FacialBlink", "LipSync")
    pub name: String,
    /// 再生進行用プレイヤー
    pub player: AnimationPlayer,
    /// アニメーション KF データ
    pub kf_nif: Arc<NifFile>,
    /// 合成ウェイト (0.0 .. 1.0)
    pub weight: f32,
    /// 優先度 (Priority: より大きな値のシーケンスが同一ボーンで優先される)
    pub priority: i32,
    /// 有効フラグ
    pub active: bool,
}

impl SequenceTrack {
    /// 新しいシーケンストラックを構築する。
    pub fn new(
        name: impl Into<String>,
        player: AnimationPlayer,
        kf_nif: Arc<NifFile>,
        weight: f32,
        priority: i32,
    ) -> Self {
        Self {
            name: name.into(),
            player,
            kf_nif,
            weight: weight.clamp(0.0, 1.0),
            priority,
            active: true,
        }
    }
}

/// 2つのボーン変換オーバーライドをウェイト比率 `alpha` (0.0 = a, 1.0 = b) で線形・球面補間合成する。
///
/// 参照元: Gamebryo 2.6 `NiTransformInterpolator::Interpolate`
pub fn blend_bone_overrides(
    a: &BoneTransformOverride,
    b: &BoneTransformOverride,
    alpha: f32,
) -> BoneTransformOverride {
    let alpha = alpha.clamp(0.0, 1.0);

    // 平行移動: LERP
    let translation = match (a.translation, b.translation) {
        (Some(ta), Some(tb)) => Some(ta.lerp(tb, alpha)),
        (Some(ta), None) => Some(ta),
        (None, Some(tb)) => Some(tb),
        (None, None) => None,
    };

    // 回転: SLERP (クォータニオン内積が負の場合は最短弧を選択)
    let rotation = match (a.rotation, b.rotation) {
        (Some(ra), Some(rb)) => {
            let mut rb_adj = rb;
            if ra.dot(rb) < 0.0 {
                rb_adj = -rb;
            }
            Some(ra.slerp(rb_adj, alpha).normalize())
        }
        (Some(ra), None) => Some(ra),
        (None, Some(tb)) => Some(tb),
        (None, None) => None,
    };

    // スケール: LERP
    let scale = match (a.scale, b.scale) {
        (Some(sa), Some(sb)) => Some(sa + (sb - sa) * alpha),
        (Some(sa), None) => Some(sa),
        (None, Some(sb)) => Some(sb),
        (None, None) => None,
    };

    BoneTransformOverride {
        translation,
        rotation,
        scale,
    }
}

/// 2つの `SkeletonPose` を指定ウェイト比率でブレンドする。
///
/// `weight_a` と `weight_b` から正規化比率 `alpha = weight_b / (weight_a + weight_b)` を算出し、
/// 各ボーンのチャンネルごとに補間合成を行う。
pub fn blend_poses(
    pose_a: &SkeletonPose,
    weight_a: f32,
    pose_b: &SkeletonPose,
    weight_b: f32,
) -> SkeletonPose {
    let total_weight = weight_a + weight_b;
    if total_weight <= 0.0 {
        return SkeletonPose::default();
    }
    let alpha = weight_b / total_weight;

    let mut result = SkeletonPose::default();

    // pose_a のボーン
    for (name, over_a) in &pose_a.overrides {
        if let Some(over_b) = pose_b.overrides.get(name) {
            result.overrides.insert(name.clone(), blend_bone_overrides(over_a, over_b, alpha));
        } else {
            result.overrides.insert(name.clone(), *over_a);
        }
    }

    // pose_b にのみ存在するボーン (フェイシャルやリップシンク特有の口・眉ボーンなど)
    for (name, over_b) in &pose_b.overrides {
        if !result.overrides.contains_key(name) {
            result.overrides.insert(name.clone(), *over_b);
        }
    }

    result
}

/// 複数のシーケンストラックからポーズを優先度 (Priority) とウェイトに基づいて合成する。
///
/// 1. ボーンごとに、それを制御しているアクティブトラック群を抽出。
/// 2. 各ボーンにおいて**最大優先度** (`priority`) を持つトラック群のみを合成対象とする。
///    （例: リップシンクが口・顎ボーンを優先度 20 で制御している場合、優先度 0 の歩行シーケンスの口ボーンは上書きされる）
/// 3. 最大優先度グループ内の各トラックのウェイトを正規化して LERP/SLERP 合成。
///
/// 参照元: Gamebryo 2.6 `NiControllerManager::Update`, `NiControllerSequence`
pub fn blend_multiple_poses(
    tracks: &[(&SkeletonPose, f32, i32)], // (pose, weight, priority)
) -> SkeletonPose {
    let mut result = SkeletonPose::default();
    if tracks.is_empty() {
        return result;
    }

    // 全トラックで制御されているボーン名の集合を収集
    let mut all_bone_names = std::collections::HashSet::new();
    for (pose, weight, _) in tracks {
        if *weight > 0.0 {
            for name in pose.overrides.keys() {
                all_bone_names.insert(name.as_str());
            }
        }
    }

    // ボーンごとに最大優先度を特定し、そのグループ内でウェイト合成
    for bone_name in all_bone_names {
        // このボーンに影響するトラックを収集
        let mut bone_tracks = Vec::new();
        let mut max_priority = i32::MIN;

        for &(pose, weight, priority) in tracks {
            if weight > 0.0 {
                if let Some(over) = pose.overrides.get(bone_name) {
                    if priority > max_priority {
                        max_priority = priority;
                    }
                    bone_tracks.push((over, weight, priority));
                }
            }
        }

        // 最大優先度を持つトラックのみにフィルタリング
        let top_tracks: Vec<(&BoneTransformOverride, f32)> = bone_tracks
            .into_iter()
            .filter(|&(_, _, p)| p == max_priority)
            .map(|(o, w, _)| (o, w))
            .collect();

        if top_tracks.is_empty() {
            continue;
        }

        let total_weight: f32 = top_tracks.iter().map(|&(_, w)| w).sum();
        if total_weight <= 0.0 {
            if let Some(&(first_over, _)) = top_tracks.first() {
                result.overrides.insert(bone_name.to_string(), *first_over);
            }
            continue;
        }

        // 逐次加重ブレンド (累積ブレンド)
        let mut acc_over = *top_tracks[0].0;
        let mut acc_weight = top_tracks[0].1;

        for &(next_over, next_weight) in top_tracks.iter().skip(1) {
            let combined_weight = acc_weight + next_weight;
            if combined_weight > 0.0 {
                let alpha = next_weight / combined_weight;
                acc_over = blend_bone_overrides(&acc_over, next_over, alpha);
                acc_weight = combined_weight;
            }
        }

        result.overrides.insert(bone_name.to_string(), acc_over);
    }

    result
}

/// 複数のアニメーションシーケンスを管理・同期・ブレンドするマネージャー。
///
/// Gamebryo 2.6 の `NiControllerManager` に対応し、ボディ歩行/待機、フェイシャル表情、
/// リップシンク発話のマルチトラック再生を統合制御する。
///
/// 参照元: Gamebryo 2.6 `NiControllerManager.h`, references/nifxml/nif.xml:L4195
#[derive(Clone, Debug, Default)]
pub struct SequenceManager {
    /// 登録されているシーケンストラック一覧
    pub tracks: Vec<SequenceTrack>,
}

impl SequenceManager {
    /// 空のマネージャーを生成する。
    pub fn new() -> Self {
        Self { tracks: Vec::new() }
    }

    /// 新しいトラックを追加する。
    pub fn add_track(&mut self, track: SequenceTrack) -> usize {
        let idx = self.tracks.len();
        self.tracks.push(track);
        idx
    }

    /// トラック名で検索してウェイトを設定する。
    pub fn set_weight(&mut self, name: &str, weight: f32) {
        if let Some(track) = self.tracks.iter_mut().find(|t| t.name == name) {
            track.weight = weight.clamp(0.0, 1.0);
        }
    }

    /// トラック名で検索して有効/無効を設定する。
    pub fn set_active(&mut self, name: &str, active: bool) {
        if let Some(track) = self.tracks.iter_mut().find(|t| t.name == name) {
            track.active = active;
        }
    }

    /// 全アクティブトラックの時間を `dt` 進め、各ポーズを評価してブレンドされた最終ポーズを返す。
    ///
    /// 参照元: Gamebryo 2.6 `NiControllerManager::Update`
    pub fn update(&mut self, dt: f32) -> SkeletonPose {
        let mut track_poses = Vec::new();

        for track in &mut self.tracks {
            if !track.active || track.weight <= 0.0 {
                continue;
            }
            let mut pose = SkeletonPose::default();
            track.player.update(&track.kf_nif, dt, &mut pose);
            track_poses.push((pose, track.weight, track.priority));
        }

        let ref_tracks: Vec<(&SkeletonPose, f32, i32)> = track_poses
            .iter()
            .map(|(p, w, pr)| (p, *w, *pr))
            .collect();

        blend_multiple_poses(&ref_tracks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Quat, Vec3};

    #[test]
    fn test_blend_bone_overrides_lerp_slerp() {
        let over_a = BoneTransformOverride {
            translation: Some(Vec3::new(0.0, 0.0, 0.0)),
            rotation: Some(Quat::IDENTITY),
            scale: Some(1.0),
        };

        let rot_y_90 = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let over_b = BoneTransformOverride {
            translation: Some(Vec3::new(10.0, 20.0, 30.0)),
            rotation: Some(rot_y_90),
            scale: Some(2.0),
        };

        // 50% ブレンド
        let blended = blend_bone_overrides(&over_a, &over_b, 0.5);

        // 平行移動: [5, 10, 15]
        let t = blended.translation.unwrap();
        assert!((t.x - 5.0).abs() < 1e-5);
        assert!((t.y - 10.0).abs() < 1e-5);
        assert!((t.z - 15.0).abs() < 1e-5);

        // スケール: 1.5
        assert!((blended.scale.unwrap() - 1.5).abs() < 1e-5);

        // 回転: 45度
        let r = blended.rotation.unwrap();
        let expected_rot_45 = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
        assert!((r.dot(expected_rot_45).abs() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_blend_multiple_poses_priority_override() {
        // ボディポーズ (Priority: 0): Pelvis と Head を制御
        let mut body_pose = SkeletonPose::default();
        body_pose.overrides.insert("Bip01 Pelvis".to_string(), BoneTransformOverride {
            translation: Some(Vec3::new(0.0, 0.0, 100.0)),
            rotation: Some(Quat::IDENTITY),
            scale: Some(1.0),
        });
        body_pose.overrides.insert("Bip01 Head".to_string(), BoneTransformOverride {
            translation: Some(Vec3::new(0.0, 0.0, 150.0)),
            rotation: Some(Quat::IDENTITY),
            scale: Some(1.0),
        });

        // リップシンクポーズ (Priority: 10): Head と Jaw (顎) を制御
        // リップシンクの高優先度により、Head の回転はリップシンク側の値が優先され、
        // かつボディにしかない Pelvis はそのまま残る
        let mut lipsync_pose = SkeletonPose::default();
        let head_talk_rot = Quat::from_rotation_x(0.1);
        lipsync_pose.overrides.insert("Bip01 Head".to_string(), BoneTransformOverride {
            translation: Some(Vec3::new(0.0, 0.0, 150.0)),
            rotation: Some(head_talk_rot),
            scale: Some(1.0),
        });
        lipsync_pose.overrides.insert("Bip01 Jaw".to_string(), BoneTransformOverride {
            translation: Some(Vec3::new(0.0, -5.0, 0.0)),
            rotation: Some(Quat::IDENTITY),
            scale: Some(1.0),
        });

        let tracks = vec![
            (&body_pose, 1.0, 0),
            (&lipsync_pose, 1.0, 10),
        ];

        let final_pose = blend_multiple_poses(&tracks);

        // 1. Pelvis はボディポーズのまま
        assert!(final_pose.overrides.contains_key("Bip01 Pelvis"));
        assert_eq!(final_pose.overrides["Bip01 Pelvis"].translation.unwrap(), Vec3::new(0.0, 0.0, 100.0));

        // 2. Jaw はリップシンクポーズから合成
        assert!(final_pose.overrides.contains_key("Bip01 Jaw"));
        assert_eq!(final_pose.overrides["Bip01 Jaw"].translation.unwrap(), Vec3::new(0.0, -5.0, 0.0));

        // 3. Head は優先度 10 のリップシンクポーズが優先される
        assert!(final_pose.overrides.contains_key("Bip01 Head"));
        let head_rot = final_pose.overrides["Bip01 Head"].rotation.unwrap();
        assert!((head_rot.dot(head_talk_rot).abs() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_sequence_manager_full_update() {
        let mut mgr = SequenceManager::new();
        assert_eq!(mgr.tracks.len(), 0);

        // 空のマネージャー更新では空ポーズが返る
        let empty_pose = mgr.update(0.016);
        assert!(empty_pose.overrides.is_empty());
    }
}
