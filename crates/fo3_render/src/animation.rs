//! # アニメーション再生およびスケルトン姿勢更新
//!
//! Gamebryo 2.6 のシーケンス評価およびボーン階層更新を再現する。
//!
//! 参照元:
//! - Gamebryo 2.6 `NiControllerSequence::Update`
//! - Gamebryo 2.6 `NiTransformInterpolator::Update`
//! - Gamebryo 2.6 `NiAVObject::UpdateDownwardPass`

use std::collections::HashMap;
use std::i16;
use glam::{Mat3, Quat, Vec3};
use fo3_gamebryo_core::NiTransform;
use fo3_nif::{
    KeyGroup, NiBSplineCompTransformInterpolator, NiQuatTransform, NiTransformInterpolator,
    NifBlock, NifFile, QuatKey,
};

/// クォータニオンキーフレーム列から指定時刻の回転をサンプリング（Slerp 補間）する。
pub fn sample_quaternion(keys: &[QuatKey], time: f32) -> Option<Quat> {
    if keys.is_empty() {
        return None;
    }
    if keys.len() == 1 || time <= keys[0].time {
        return Some(keys[0].value.to_glam());
    }
    let last = keys.len() - 1;
    if time >= keys[last].time {
        return Some(keys[last].value.to_glam());
    }

    // 二分探索で区間 [idx, idx + 1] を特定
    let idx = match keys.binary_search_by(|k| k.time.partial_cmp(&time).unwrap()) {
        Ok(exact) => return Some(keys[exact].value.to_glam()),
        Err(insert) => insert.saturating_sub(1),
    };
    let k0 = &keys[idx];
    let k1 = &keys[idx + 1];
    let span = k1.time - k0.time;
    let alpha = if span > 1e-6 {
        (time - k0.time) / span
    } else {
        0.0
    };

    let q0 = k0.value.to_glam();
    let q1 = k1.value.to_glam();
    Some(q0.slerp(q1, alpha))
}

/// 3次元ベクトルキーフレーム列から指定時刻の値をサンプリング（線形補間）する。
pub fn sample_vector3(group: &KeyGroup<fo3_nif::types::Vector3>, time: f32) -> Option<Vec3> {
    let keys = &group.keys;
    if keys.is_empty() {
        return None;
    }
    let to_vec3 = |v: &fo3_nif::types::Vector3| Vec3::new(v.x, v.y, v.z);

    if keys.len() == 1 || time <= keys[0].time {
        return Some(to_vec3(&keys[0].value));
    }
    let last = keys.len() - 1;
    if time >= keys[last].time {
        return Some(to_vec3(&keys[last].value));
    }

    let idx = match keys.binary_search_by(|k| k.time.partial_cmp(&time).unwrap()) {
        Ok(exact) => return Some(to_vec3(&keys[exact].value)),
        Err(insert) => insert.saturating_sub(1),
    };
    let k0 = &keys[idx];
    let k1 = &keys[idx + 1];
    let span = k1.time - k0.time;
    let alpha = if span > 1e-6 {
        (time - k0.time) / span
    } else {
        0.0
    };

    let v0 = to_vec3(&k0.value);
    let v1 = to_vec3(&k1.value);
    Some(v0.lerp(v1, alpha))
}

/// 浮動小数点キーフレーム列から指定時刻の値をサンプリング（線形補間）する。
pub fn sample_float(group: &KeyGroup<f32>, time: f32) -> Option<f32> {
    let keys = &group.keys;
    if keys.is_empty() {
        return None;
    }
    if keys.len() == 1 || time <= keys[0].time {
        return Some(keys[0].value);
    }
    let last = keys.len() - 1;
    if time >= keys[last].time {
        return Some(keys[last].value);
    }

    let idx = match keys.binary_search_by(|k| k.time.partial_cmp(&time).unwrap()) {
        Ok(exact) => return Some(keys[exact].value),
        Err(insert) => insert.saturating_sub(1),
    };
    let k0 = &keys[idx];
    let k1 = &keys[idx + 1];
    let span = k1.time - k0.time;
    let alpha = if span > 1e-6 {
        (time - k0.time) / span
    } else {
        0.0
    };

    Some(k0.value + (k1.value - k0.value) * alpha)
}

/// Cox-de Boor 基底関数 (開一様 B-Spline) の評価。
///
/// 参照元:
/// - `references/nifskope/src/gl/glcontroller.cpp:L625` (`blend`)
/// - `references/nifskope/src/gl/glcontroller.cpp:L648` (`compute_intervals`)
#[inline]
fn bspline_blend(k: usize, t: usize, u: &[f32], v: f32) -> f32 {
    if t == 1 {
        // 基本ケース
        if u[k] <= v && v < u[k + 1] {
            1.0
        } else {
            0.0
        }
    } else {
        // 分母が 0 の分岐 (重複ノット)
        if (u[k + t - 1] == u[k]) && (u[k + t] == u[k + 1]) {
            0.0
        } else if u[k + t - 1] == u[k] {
            // 片方の項の分母が 0 なので他方のみ使用
            (u[k + t] - v) / (u[k + t] - u[k + 1]) * bspline_blend(k + 1, t - 1, u, v)
        } else if u[k + t] == u[k + 1] {
            (v - u[k]) / (u[k + t - 1] - u[k]) * bspline_blend(k, t - 1, u, v)
        } else {
            (v - u[k]) / (u[k + t - 1] - u[k]) * bspline_blend(k, t - 1, u, v)
                + (u[k + t] - v) / (u[k + t] - u[k + 1]) * bspline_blend(k + 1, t - 1, u, v)
        }
    }
}

/// 開一様ノット列 (open uniform knot vector) を構築する。
///
/// 参照元: `references/nifskope/src/gl/glcontroller.cpp:L648` (`compute_intervals`)
fn compute_knots(n: usize, t: usize) -> Vec<f32> {
    let mut u = vec![0.0f32; n + t + 1];
    for j in 0..=(n + t) {
        if j < t {
            u[j] = 0.0;
        } else if t <= j && j <= n {
            u[j] = (j - t + 1) as f32;
        } else {
            u[j] = (n - t + 2) as f32;
        }
    }
    u
}

/// 圧縮制御点の B-Spline 評価 (NifSkope `bsplineinterpolate` / `compute_point` 準拠)。
///
/// - `handle`: NiBSplineData.compact_control_points への開始オフセット (short 単位)
/// - `num_points`: 基底制御点数 = nCtrl (NiBSplineBasisData.Num Control Points)
/// - `components`: チャンネル要素数 (translation=3, rotation=4, scale=1)
/// - `half_range` / `offset`: 圧縮値の復元パラメータ
///
/// 参照元:
/// - `references/nifskope/src/gl/glcontroller.cpp:L660` (`compute_point`)
/// - `references/nifskope/src/gl/glcontroller.cpp:L676` (`bsplineinterpolate`)
fn evaluate_bspline_channel(
    compact: &[i16],
    handle: u32,
    degree: usize,
    interval: f32,
    num_points: u32,
    components: usize,
    half_range: f32,
    offset: f32,
) -> Option<Vec<f32>> {
    // ハンドル無効 (USHRT_MAX) の場合は None
    if handle == u16::MAX as u32 {
        return None;
    }
    let handle = handle as usize;
    let t = degree + 1;
    let n = num_points as usize - 1;
    let l = components;
    if interval >= (num_points as f32 - degree as f32) {
        // 端点: 最後の制御点セットをそのまま使用
        let mut out: Vec<f32> = vec![0.0; l];
        let base = handle + n * l;
        for i in 0..l {
            let c = compact.get(base + i).copied().unwrap_or(0) as f32 / i16::MAX as f32;
            out[i] = c;
        }
        out.iter_mut().for_each(|v| *v = *v * half_range + offset);
        return Some(out);
    }

    let u = compute_knots(n, t);
    let mut out: Vec<f32> = vec![0.0; l];
    for k in 0..=n {
        let weight = bspline_blend(k, t, &u, interval);
        let base = handle + k * l;
        for i in 0..l {
            let c = compact.get(base + i).copied().unwrap_or(0) as f32 / i16::MAX as f32;
            out[i] += c * weight;
        }
    }
    out.iter_mut().for_each(|v| *v = *v * half_range + offset);
    Some(out)
}

/// `NiBSplineCompTransformInterpolator` から指定時刻のトランスフォームを B-Spline 評価する。
///
/// 参照元:
/// - `references/nifskope/src/gl/glcontroller.cpp:L789` (`updateTransform`)
/// - `references/nifskope/src/gl/glcontroller.cpp:L676` (`bsplineinterpolate`)
/// - `references/nifxml/nif.xml:L4141` (`NiBSplineCompTransformInterpolator`)
fn sample_bspline_transform_interpolator(
    bsp: &NiBSplineCompTransformInterpolator,
    time: f32,
    kf: &NifFile,
) -> Option<NiTransform> {
    // スプライン・基底データを解決
    let (spline_idx, basis_idx) = (bsp.spline_data, bsp.basis_data);
    if spline_idx < 0 || basis_idx < 0
        || spline_idx as usize >= kf.blocks.len()
        || basis_idx as usize >= kf.blocks.len()
    {
        return None;
    }
    let spline = match &kf.blocks[spline_idx as usize] {
        NifBlock::NiBSplineData(d) => d,
        _ => return None,
    };
    let basis = match &kf.blocks[basis_idx as usize] {
        NifBlock::NiBSplineBasisData(d) => d,
        _ => return None,
    };

    let n_control = basis.num_control_points;
    if n_control == 0 {
        return None;
    }

    // 基礎姿勢 (NiQuatTransform)
    let base_trans = if is_valid_float(bsp.transform.translation.x) {
        bsp.transform.translation.x
    } else {
        0.0
    };
    let base_rot = bsp.transform.rotation.to_glam();
    let base_scale = if is_valid_float(bsp.transform.scale) && bsp.transform.scale > 0.001 {
        bsp.transform.scale
    } else {
        1.0
    };

    let degree = 3;
    let span = bsp.stop_time - bsp.start_time;
    let interval = if span > 1e-6 {
        ((time - bsp.start_time) / span) * (n_control as f32 - degree as f32)
    } else {
        0.0
    };

    // 各チャンネルを B-Spline 評価 (ハンドル有効時のみ、無効なら基礎値を使用)
    let mut trans = Vec3::new(base_trans, base_trans, base_trans);
    if let Some(v) = evaluate_bspline_channel(
        &spline.compact_control_points,
        bsp.translation_handle,
        degree,
        interval,
        n_control,
        3,
        bsp.translation_half_range,
        bsp.translation_offset,
    ) {
        trans = Vec3::new(v[0], v[1], v[2]);
    } else if is_valid_float(bsp.transform.translation.x)
        && is_valid_float(bsp.transform.translation.y)
        && is_valid_float(bsp.transform.translation.z)
    {
        // ハンドル無効時は NiQuatTransform の translation を使用
        trans = Vec3::new(
            bsp.transform.translation.x,
            bsp.transform.translation.y,
            bsp.transform.translation.z,
        );
    }

    let mut rot = base_rot;
    if let Some(q) = evaluate_bspline_channel(
        &spline.compact_control_points,
        bsp.rotation_handle,
        degree,
        interval,
        n_control,
        4,
        bsp.rotation_half_range,
        bsp.rotation_offset,
    ) {
        rot = Quat::from_xyzw(q[1], q[2], q[3], q[0]); // 制御点は (w,x,y,z) 順
    }

    let mut scale = base_scale;
    if let Some(s) = evaluate_bspline_channel(
        &spline.compact_control_points,
        bsp.scale_handle,
        degree,
        interval,
        n_control,
        1,
        bsp.scale_half_range,
        bsp.scale_offset,
    ) {
        scale = s[0];
    }

    Some(NiTransform {
        rotation: Mat3::from_quat(rot),
        translation: trans,
        scale,
    })
}

/// 有効な浮動小数点値であるか判定する（#INV_FLT# や 極大値でないか）。
fn is_valid_float(f: f32) -> bool {
    f.is_finite() && f > -1e30 && f < 1e30
}

/// アニメーションサンプリング用の単一ボーンチャンネル情報。
#[derive(Clone, Debug)]
pub struct BoneChannel {
    pub bone_name: String,
    pub interpolator_index: i32,
}

/// ロードされたアニメーションシーケンスプレイヤー。
#[derive(Clone, Debug)]
pub struct AnimationClip {
    pub name: String,
    pub start_time: f32,
    pub stop_time: f32,
    pub duration: f32,
    pub cycle_type: u32,
    pub frequency: f32,
    /// ボーン名ごとの制御チャンネル
    pub channels: HashMap<String, BoneChannel>,
}

impl AnimationClip {
    /// KF ファイルから最初のアニメーションシーケンスを取り出してクリップを構築する。
    pub fn from_kf(kf: &NifFile) -> Option<Self> {
        // ルート（または最初）の NiControllerSequence を検索
        let seq = kf.blocks.iter().find_map(|b| match b {
            NifBlock::NiControllerSequence(s) => Some(s),
            _ => None,
        })?;

        let name = kf.get_string(seq.name_index as u32).unwrap_or("").to_string();
        let duration = (seq.stop_time - seq.start_time).max(0.0);

        let mut channels = HashMap::new();
        for cb in &seq.controlled_blocks {
            if let Some(bone_name) = kf.get_string(cb.node_name_index as u32) {
                channels.insert(
                    bone_name.to_string(),
                    BoneChannel {
                        bone_name: bone_name.to_string(),
                        interpolator_index: cb.interpolator,
                    },
                );
            }
        }

        Some(AnimationClip {
            name,
            start_time: seq.start_time,
            stop_time: seq.stop_time,
            duration,
            cycle_type: seq.cycle_type,
            frequency: if seq.frequency > 0.01 { seq.frequency } else { 1.0 },
            channels,
        })
    }

    /// 指定時刻（経過秒数）における正規化時間（ループ内時刻）を計算する。
    pub fn evaluate_time(&self, elapsed_seconds: f32) -> f32 {
        if self.duration <= 1e-5 {
            return self.start_time;
        }
        let scaled_time = elapsed_seconds * self.frequency;
        // サイクルタイプ: 0=LOOP, 1=REVERSE, 2=CLAMP
        match self.cycle_type {
            0 => {
                // ループ再生
                self.start_time + (scaled_time % self.duration)
            }
            _ => {
                // デフォルトはループまたはクランプ
                self.start_time + (scaled_time % self.duration)
            }
        }
    }

    /// 指定ボーンの時刻 `time` におけるローカルトランスフォームを評価する。
    pub fn sample_bone_transform(
        &self,
        bone_name: &str,
        time: f32,
        kf: &NifFile,
    ) -> Option<NiTransform> {
        let channel = self.channels.get(bone_name)?;
        if channel.interpolator_index < 0 || channel.interpolator_index as usize >= kf.blocks.len() {
            return None;
        }

        match &kf.blocks[channel.interpolator_index as usize] {
            NifBlock::NiTransformInterpolator(interp) => {
                sample_transform_interpolator(interp, time, kf)
            }
            NifBlock::NiBSplineCompTransformInterpolator(bsp) => {
                // B-Spline 圧縮補間を評価 (FO3 で主流)
                sample_bspline_transform_interpolator(bsp, time, kf)
                    .or_else(|| sample_quat_transform(&bsp.transform))
            }
            _ => None,
        }
    }
}

/// `NiTransformInterpolator` からトランスフォームをサンプリングする。
fn sample_transform_interpolator(
    interp: &NiTransformInterpolator,
    time: f32,
    kf: &NifFile,
) -> Option<NiTransform> {
    // 基礎トランスフォーム
    let mut trans = if is_valid_float(interp.transform.translation.x) {
        Vec3::new(
            interp.transform.translation.x,
            interp.transform.translation.y,
            interp.transform.translation.z,
        )
    } else {
        Vec3::ZERO
    };

    let mut rot = if is_valid_float(interp.transform.rotation.w) {
        interp.transform.rotation.to_glam()
    } else {
        Quat::IDENTITY
    };

    let mut scale = if is_valid_float(interp.transform.scale) && interp.transform.scale > 0.001 {
        interp.transform.scale
    } else {
        1.0
    };

    // キーフレームデータがある場合は上書きサンプリング
    if interp.data >= 0 && (interp.data as usize) < kf.blocks.len() {
        if let NifBlock::NiTransformData(data) = &kf.blocks[interp.data as usize] {
            // 回転サンプリング
            if let Some(sampled_rot) = sample_quaternion(&data.quaternion_keys, time) {
                rot = sampled_rot;
            }
            // 移動サンプリング
            if let Some(sampled_trans) = sample_vector3(&data.translations, time) {
                trans = sampled_trans;
            }
            // スケールサンプリング
            if let Some(sampled_scale) = sample_float(&data.scales, time) {
                scale = sampled_scale;
            }
        }
    }

    Some(NiTransform {
        rotation: Mat3::from_quat(rot),
        translation: trans,
        scale,
    })
}

/// `NiQuatTransform` から `NiTransform` を抽出する。
fn sample_quat_transform(qt: &NiQuatTransform) -> Option<NiTransform> {
    let trans = if is_valid_float(qt.translation.x) {
        Vec3::new(qt.translation.x, qt.translation.y, qt.translation.z)
    } else {
        Vec3::ZERO
    };

    let rot = if is_valid_float(qt.rotation.w) {
        qt.rotation.to_glam()
    } else {
        Quat::IDENTITY
    };

    let scale = if is_valid_float(qt.scale) && qt.scale > 0.001 {
        qt.scale
    } else {
        1.0
    };

    Some(NiTransform {
        rotation: Mat3::from_quat(rot),
        translation: trans,
        scale,
    })
}

/// 各ボーンのローカルトランスフォーム（バインドポーズからの偏差）を保持する姿勢。
///
/// `NiAnimEvaluator` で各ボーンに設定された値を基準とし、アニメーションで
/// 上書きされるボーンのみローカル変換を差し替える。
#[derive(Clone, Debug, Default)]
pub struct SkeletonPose {
    /// ノード名 → ローカルトランスフォーム（バインドポーズに対する上書き値）
    pub overrides: HashMap<String, NiTransform>,
}

/// `NiControllerSequence` のボーン適用ループ。
///
/// KF の各 `ControlledBlock` からノード名を解決し、対応するインターポレータを
/// 現在時刻 `time` で評価して、そのボーンのローカルトランスフォームを `pose` に
/// 記録する。戻り値はアニメーションによって姿勢が変化したノード名の集合。
///
/// 参照元:
/// - Gamebryo 2.6 `NiControllerSequence::Update`
/// - `knowledge/animation_kf_format.md` (セクション 5: アニメーション更新ループ)
pub fn apply_pose(
    kf: &NifFile,
    pose: &mut SkeletonPose,
    time: f32,
) -> Vec<String> {
    let mut updated = Vec::new();
    // KF の中核は NiControllerSequence
    let Some(seq) = kf.blocks.iter().find_map(|b| match b {
        NifBlock::NiControllerSequence(s) => Some(s),
        _ => None,
    }) else {
        return updated;
    };

    for cb in &seq.controlled_blocks {
        let Some(node_name) = kf.get_string(cb.node_name_index as u32) else {
            continue;
        };
        let idx = cb.interpolator;
        if idx < 0 || idx as usize >= kf.blocks.len() {
            continue;
        }
        // ボーン名・プロパティ名を含まない純粋なボーン姿勢制御のみを対象とする
        // (NiTransformInterpolator は NiNode に直接適用されるボーン制御に使用される)
        let transform = match &kf.blocks[idx as usize] {
            NifBlock::NiTransformInterpolator(interp) => {
                sample_transform_interpolator(interp, time, kf)
            }
            NifBlock::NiBSplineCompTransformInterpolator(bsp) => {
                // B-Spline 圧縮補間を評価 (FO3 で主流)
                sample_bspline_transform_interpolator(bsp, time, kf)
                    .or_else(|| sample_quat_transform(&bsp.transform))
            }
            _ => None,
        };

        if let Some(t) = transform {
            pose.overrides.insert(node_name.to_string(), t);
            updated.push(node_name.to_string());
        }
    }

    updated
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 単純な NiTransformInterpolator (キーデータなしの固定姿勢) を持つ KF 上で、
    /// `apply_pose` がボーン名を解決してローカルトランスフォームを評価できることを検証する。
    #[test]
    fn test_apply_pose_from_transform_interpolator() {
        use fo3_nif::blocks::NiTransformInterpolator;
        use fo3_nif::header::{BSStreamHeader, ExportString, NifHeader};
        use fo3_nif::types::{NiQuatTransform, Quaternion, Vector3};
        use fo3_nif::{ControlledBlock, NifFile, NiControllerSequence};

        // NiTransformInterpolator: 固定姿勢 (translation (5, 6, 7), identity rot, scale 1)
        let interp = NiTransformInterpolator {
            transform: NiQuatTransform {
                translation: Vector3 {
                    x: 5.0,
                    y: 6.0,
                    z: 7.0,
                },
                rotation: Quaternion::default(),
                scale: 1.0,
            },
            data: -1,
        };

        // NiControllerSequence: ControlledBlock は "Bip01" (文字列インデックス 1) を指す
        let seq = NiControllerSequence {
            name_index: 0,
            array_grow_by: 1,
            controlled_blocks: vec![ControlledBlock {
                interpolator: 1,
                controller: -1,
                priority: 0,
                node_name_index: 1,
                property_type_index: -1,
                controller_type_index: -1,
                controller_id_index: -1,
                interpolator_id_index: -1,
            }],
            weight: 1.0,
            text_keys: -1,
            cycle_type: 0,
            frequency: 1.0,
            start_time: 0.0,
            stop_time: 1.0,
            manager: -1,
            accum_root_name_index: -1,
            anim_note_arrays: vec![],
        };

        let kf = NifFile {
            header: NifHeader {
                header_string: "Gamebryo File Format, Version 20.2.0.7\n".to_string(),
                version: 0x14020007,
                endian_type: 1,
                user_version: 11,
                num_blocks: 0,
                bs_header: BSStreamHeader {
                    bs_version: 34,
                    author: ExportString { value: String::new() },
                    process_script: None,
                    export_script: ExportString { value: String::new() },
                },
                block_types: vec![],
                block_type_indices: vec![],
                block_sizes: vec![],
                strings: vec![
                    String::new(),
                    "Bip01".to_string(),
                ],
            },
            blocks: vec![NifBlock::NiControllerSequence(seq), NifBlock::NiTransformInterpolator(interp)],
        };

        let mut pose = SkeletonPose::default();
        let updated = apply_pose(&kf, &mut pose, 0.25);

        assert_eq!(updated, vec!["Bip01".to_string()]);
        let t = pose.overrides.get("Bip01").expect("Bip01 should be overridden");
        assert_eq!(t.translation, Vec3::new(5.0, 6.0, 7.0));
        assert_eq!(t.scale, 1.0);
    }
}
