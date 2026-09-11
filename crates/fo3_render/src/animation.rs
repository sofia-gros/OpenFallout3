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
/// 各ボーンのアニメーション上書きトランスフォーム（移動・回転・スケールの独立チャンネル）。
///
/// Gamebryo 2.6 では、アニメーションで指定されたチャンネルのみが上書きされ、
/// 未指定（キーなし、または無効値）のチャンネルは元のバインドポーズの値がそのまま保持される。
/// 参照元: Gamebryo 2.6 `NiTransformInterpolator::Update`, `NiBSplineCompTransformInterpolator::Update`
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoneTransformOverride {
    /// ローカル並進移動（キーが存在する場合のみ）
    pub translation: Option<Vec3>,
    /// ローカル回転四元数（キーが存在する場合のみ）
    pub rotation: Option<Quat>,
    /// ローカルスケール（キーが存在する場合のみ）
    pub scale: Option<f32>,
}

impl BoneTransformOverride {
    /// バインドポーズの `base` に対して、指定されているチャンネルのみを上書き適用する。
    pub fn apply_to(&self, mut base: NiTransform) -> NiTransform {
        if let Some(t) = self.translation {
            base.translation = t;
        }
        if let Some(r) = self.rotation {
            base.rotation = Mat3::from_quat(r);
        }
        if let Some(s) = self.scale {
            base.scale = s;
        }
        base
    }

    /// 完全な `NiTransform` から `BoneTransformOverride` を構築する。
    pub fn from_transform(t: NiTransform) -> Self {
        Self {
            translation: Some(t.translation),
            rotation: Some(Quat::from_mat3(&t.rotation)),
            scale: Some(t.scale),
        }
    }
}

impl From<NiTransform> for BoneTransformOverride {
    fn from(t: NiTransform) -> Self {
        Self::from_transform(t)
    }
}

/// B-Spline 圧縮トランスフォーム補間子 (`NiBSplineCompTransformInterpolator`) をサンプリングする。
///
/// 参照元:
/// - Gamebryo 2.6 `NiBSplineCompTransformInterpolator::Update`
/// - `references/nifskope/src/gl/glcontroller.cpp:L676` (`bsplineinterpolate`)
/// - `references/nifxml/nif.xml:L4141` (`NiBSplineCompTransformInterpolator`)
fn sample_bspline_transform_interpolator(
    bsp: &NiBSplineCompTransformInterpolator,
    time: f32,
    kf: &NifFile,
) -> Option<BoneTransformOverride> {
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

    let degree = 3;
    let span = bsp.stop_time - bsp.start_time;
    let interval = if span > 1e-6 {
        ((time - bsp.start_time) / span) * (n_control as f32 - degree as f32)
    } else {
        0.0
    };

    // 並進移動チャンネル評価 (ハンドル有効時のみ B-Spline 評価、無効なら基礎姿勢から取得、いずれも無効なら None)
    let trans = if let Some(v) = evaluate_bspline_channel(
        &spline.compact_control_points,
        bsp.translation_handle,
        degree,
        interval,
        n_control,
        3,
        bsp.translation_half_range,
        bsp.translation_offset,
    ) {
        Some(Vec3::new(v[0], v[1], v[2]))
    } else if is_valid_float(bsp.transform.translation.x)
        && is_valid_float(bsp.transform.translation.y)
        && is_valid_float(bsp.transform.translation.z)
    {
        Some(Vec3::new(
            bsp.transform.translation.x,
            bsp.transform.translation.y,
            bsp.transform.translation.z,
        ))
    } else {
        None
    };

    // 回転チャンネル評価
    let rot = if let Some(q) = evaluate_bspline_channel(
        &spline.compact_control_points,
        bsp.rotation_handle,
        degree,
        interval,
        n_control,
        4,
        bsp.rotation_half_range,
        bsp.rotation_offset,
    ) {
        Some(Quat::from_xyzw(q[1], q[2], q[3], q[0])) // 制御点は (w,x,y,z) 順
    } else if is_valid_float(bsp.transform.rotation.w) {
        Some(bsp.transform.rotation.to_glam())
    } else {
        None
    };

    // スケールチャンネル評価
    let scale = if let Some(s) = evaluate_bspline_channel(
        &spline.compact_control_points,
        bsp.scale_handle,
        degree,
        interval,
        n_control,
        1,
        bsp.scale_half_range,
        bsp.scale_offset,
    ) {
        Some(s[0])
    } else if is_valid_float(bsp.transform.scale) && bsp.transform.scale > 0.001 {
        Some(bsp.transform.scale)
    } else {
        None
    };

    if trans.is_none() && rot.is_none() && scale.is_none() {
        None
    } else {
        Some(BoneTransformOverride {
            translation: trans,
            rotation: rot,
            scale,
        })
    }
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

/// アニメーションのループ種別 (Gamebryo `CycleType`)。
///
/// 参照元: `references/nifxml/nif.xml:L1022` (CycleType enum)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CycleType {
    /// CYCLE_LOOP  (0): 時間が `[start, stop]` を繰り返し再生される。
    Loop,
    /// CYCLE_REVERSE (1): 往復再生（前進→後進を交互に繰り返す）。
    Reverse,
    /// CYCLE_CLAMP (2): 最後のキーフレームで停止する。
    Clamp,
}

impl CycleType {
    /// 生の uint 値から `CycleType` へ変換する。
    /// 参照元: `references/nifxml/nif.xml:L1022-L1026`
    pub fn from_u32(v: u32) -> Self {
        match v {
            0 => CycleType::Loop,
            1 => CycleType::Reverse,
            _ => CycleType::Clamp,
        }
    }
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

    /// 指定時刻（経過秒数）を `CycleType` に従ってシーケンス内時間へ正規化する。
    ///
    /// - `CycleType::Loop`: `[start, stop]` を繰り返し (`scaled % duration`)。
    /// - `CycleType::Reverse`: 往復再生。周期 `2*duration` で前進・後進を交互に繰り返す。
    /// - `CycleType::Clamp`: 終端で停止 (`scaled.clamp(0, duration)`)。
    ///
    /// 参照元: Gamebryo 2.6 `NiControllerSequence::ComputeScaledTime`,
    /// `references/nifxml/nif.xml:L1022` (CycleType enum)
    pub fn evaluate_time(&self, elapsed_seconds: f32) -> f32 {
        if self.duration <= 1e-5 {
            return self.start_time;
        }
        let scaled_time = (elapsed_seconds * self.frequency).max(0.0);
        match CycleType::from_u32(self.cycle_type) {
            CycleType::Loop => self.start_time + (scaled_time % self.duration),
            CycleType::Reverse => {
                let period = self.duration * 2.0;
                let t = scaled_time % period;
                if t > self.duration {
                    self.start_time + (period - t)
                } else {
                    self.start_time + t
                }
            }
            CycleType::Clamp => self.start_time + scaled_time.min(self.duration),
        }
    }

    /// 指定ボーンの時刻 `time` におけるローカルトランスフォーム上書きを評価する。
    pub fn sample_bone_transform(
        &self,
        bone_name: &str,
        time: f32,
        kf: &NifFile,
    ) -> Option<BoneTransformOverride> {
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

/// 単一ループの簡易アニメーションプレイヤー。
///
/// 1 本の `AnimationClip`（KF の `NiControllerSequence`）を再生し、フレーム毎の
/// 経過時間 `dt` からシーケンス内時間を進行・ループ制御し、`SkeletonPose` を更新する。
///
/// 使い方の流れ (Gamebryo 2.6 のボーン適用ループ):
///
/// ```ignore
/// let mut player = AnimationPlayer::new(clip);
/// let mut pose = SkeletonPose::default();
/// loop {
///     let updated = player.update(kf, dt, &mut pose); // ボーン名集合を返す
///     recompute_bone_world_map_with_pose(skeleton_nif, &pose, &mut bone_world_map);
///     // rebind 後のスキニングは apply_skinning_cpu_with_bones へ bone_world_map を渡す
/// }
/// ```
///
/// 参照元:
/// - Gamebryo 2.6 `NiControllerSequence::Update`
/// - Gamebryo 2.6 `NiTimeController::Update` (時間進行)
/// - `knowledge/animation_kf_format.md` (セクション 5)
#[derive(Clone, Debug)]
pub struct AnimationPlayer {
    /// 再生対象のクリップ。
    pub clip: AnimationClip,
    /// ループ種別 (`AnimationClip.cycle_type` から導出)。
    pub cycle_type: CycleType,
    /// 再生中フラグ (`false` で一時停止)。
    pub playing: bool,
    /// 実経過秒数 (`frequency` 適用前)。
    elapsed: f32,
    /// 現在のシーケンス内時間 (最後の `update` で確定)。
    pub current_time: f32,
    /// `CycleType::Clamp` で終端に達したか (再生が完了したか)。
    pub finished: bool,
}

impl AnimationPlayer {
    /// クリップからプレイヤーを構築する。再生は開始状態で、時刻は `start_time` に初期化される。
    pub fn new(clip: AnimationClip) -> Self {
        let cycle_type = CycleType::from_u32(clip.cycle_type);
        let current_time = clip.start_time;
        Self {
            clip,
            cycle_type,
            playing: true,
            elapsed: 0.0,
            current_time,
            finished: false,
        }
    }

    /// 経過秒数 `elapsed_seconds` へ直接シークする。以降の `update` はその時刻から進行する。
    pub fn seek(&mut self, elapsed_seconds: f32) {
        self.elapsed = elapsed_seconds.max(0.0);
        self.current_time = self.clip.evaluate_time(self.elapsed);
        self.finished = false;
    }

    /// 一時停止／再開を切り替える。
    pub fn set_playing(&mut self, playing: bool) {
        self.playing = playing;
    }

    /// フレーム毎の経過時間 `dt` (秒) で時間を進行させ、`pose` を更新する。
    ///
    /// 戻り値はこのフレームで姿勢が変化したボーン名の集合 (`apply_pose` の結果)。
    /// 再生停止中は時間を進めず空の集合を返す。
    pub fn update(&mut self, kf: &NifFile, dt: f32, pose: &mut SkeletonPose) -> Vec<String> {
        if !self.playing {
            return Vec::new();
        }
        self.elapsed += dt.max(0.0);
        let scaled = self.elapsed * self.clip.frequency;
        // CycleType::Clamp では終端 (stop_time) 到達で完了とする
        if self.cycle_type == CycleType::Clamp && scaled >= self.clip.duration {
            self.finished = true;
        }
        self.current_time = self.clip.evaluate_time(self.elapsed);
        apply_pose(kf, pose, self.current_time)
    }
}

fn sample_transform_interpolator(
    interp: &NiTransformInterpolator,
    time: f32,
    kf: &NifFile,
) -> Option<BoneTransformOverride> {
    let mut trans = None;
    let mut rot = None;
    let mut scale = None;

    // キーフレームデータがある場合は上書きサンプリング
    if interp.data >= 0 && (interp.data as usize) < kf.blocks.len() {
        if let NifBlock::NiTransformData(data) = &kf.blocks[interp.data as usize] {
            // 回転サンプリング
            if let Some(sampled_rot) = sample_quaternion(&data.quaternion_keys, time) {
                rot = Some(sampled_rot);
            }
            // 移動サンプリング
            if let Some(sampled_trans) = sample_vector3(&data.translations, time) {
                trans = Some(sampled_trans);
            }
            // スケールサンプリング
            if let Some(sampled_scale) = sample_float(&data.scales, time) {
                scale = Some(sampled_scale);
            }
        }
    }

    // キーフレームデータ未定義時は基礎姿勢からフォールバック
    if trans.is_none()
        && is_valid_float(interp.transform.translation.x)
        && is_valid_float(interp.transform.translation.y)
        && is_valid_float(interp.transform.translation.z)
    {
        trans = Some(Vec3::new(
            interp.transform.translation.x,
            interp.transform.translation.y,
            interp.transform.translation.z,
        ));
    }

    if rot.is_none() && is_valid_float(interp.transform.rotation.w) {
        rot = Some(interp.transform.rotation.to_glam());
    }

    if scale.is_none() && is_valid_float(interp.transform.scale) && interp.transform.scale > 0.001 {
        scale = Some(interp.transform.scale);
    }

    if trans.is_none() && rot.is_none() && scale.is_none() {
        None
    } else {
        Some(BoneTransformOverride {
            translation: trans,
            rotation: rot,
            scale,
        })
    }
}

/// `NiQuatTransform` から `BoneTransformOverride` を抽出する。
fn sample_quat_transform(qt: &NiQuatTransform) -> Option<BoneTransformOverride> {
    let trans = if is_valid_float(qt.translation.x)
        && is_valid_float(qt.translation.y)
        && is_valid_float(qt.translation.z)
    {
        Some(Vec3::new(qt.translation.x, qt.translation.y, qt.translation.z))
    } else {
        None
    };

    let rot = if is_valid_float(qt.rotation.w) {
        Some(qt.rotation.to_glam())
    } else {
        None
    };

    let scale = if is_valid_float(qt.scale) && qt.scale > 0.001 {
        Some(qt.scale)
    } else {
        None
    };

    if trans.is_none() && rot.is_none() && scale.is_none() {
        None
    } else {
        Some(BoneTransformOverride {
            translation: trans,
            rotation: rot,
            scale,
        })
    }
}

/// 各ボーンのローカルトランスフォーム（バインドポーズからの偏差）を保持する姿勢。
///
/// `NiAnimEvaluator` で各ボーンに設定された値を基準とし、アニメーションで
/// 上書きされるチャンネル（並進・回転・スケール）のみローカル変換を差し替える。
#[derive(Clone, Debug, Default)]
pub struct SkeletonPose {
    /// ノード名 → チャンネル別上書きトランスフォーム
    pub overrides: HashMap<String, BoneTransformOverride>,
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
    use fo3_nif::header::{BSStreamHeader, ExportString, NifHeader};
    use fo3_nif::{NifBlock, NifFile};

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
        assert_eq!(t.translation, Some(Vec3::new(5.0, 6.0, 7.0)));
        assert_eq!(t.scale, Some(1.0));
    }

    /// `AnimationClip::evaluate_time` が CycleType に応じてシーケンス内時間を正しく
    /// 正規化 (LOOP / REVERSE / CLAMP) できることを検証する。
    #[test]
    fn test_evaluate_time_cycle_types() {
        // start=0, stop=10, duration=10, frequency=1.0
        let base = AnimationClip {
            name: "t".to_string(),
            start_time: 0.0,
            stop_time: 10.0,
            duration: 10.0,
            cycle_type: 0,
            frequency: 1.0,
            channels: HashMap::new(),
        };

        // LOOP (0): 25 秒 → 25 % 10 = 5
        let loop_clip = AnimationClip { cycle_type: 0, ..base.clone() };
        assert!((loop_clip.evaluate_time(25.0) - 5.0).abs() < 1e-5);
        assert!((loop_clip.evaluate_time(10.0) - 0.0).abs() < 1e-5);

        // REVERSE (1): 周期 20。15 秒 → 15 > 10 なので 20-15 = 5
        let rev = AnimationClip { cycle_type: 1, ..base.clone() };
        assert!((rev.evaluate_time(5.0) - 5.0).abs() < 1e-5); // 前進
        assert!((rev.evaluate_time(15.0) - 5.0).abs() < 1e-5); // 後進
        assert!((rev.evaluate_time(25.0) - 5.0).abs() < 1e-5); // 再び前進

        // CLAMP (2): 12 秒 → 終端 10 で停止
        let clamp = AnimationClip { cycle_type: 2, ..base.clone() };
        assert!((clamp.evaluate_time(3.0) - 3.0).abs() < 1e-5);
        assert!((clamp.evaluate_time(12.0) - 10.0).abs() < 1e-5);
    }

    /// `AnimationPlayer::update` が dt ごとにシーケンス内時間を進行させ、姿勢を更新することを検証する。
    #[test]
    fn test_animation_player_update_progresses_time() {
        // LOOP, start=0, stop=10
        let clips = AnimationClip {
            name: "idle".to_string(),
            start_time: 0.0,
            stop_time: 10.0,
            duration: 10.0,
            cycle_type: 0,
            frequency: 1.0,
            channels: HashMap::new(),
        };
        let mut player = AnimationPlayer::new(clips);
        assert!((player.current_time - 0.0).abs() < 1e-5);

        let mut pose = SkeletonPose::default();
        // 空の KF (ボーンなし) でも姿勢更新が空集合を返す
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
                strings: vec![],
            },
            blocks: vec![],
        };

        let updated = player.update(&kf, 2.5, &mut pose);
        assert!(updated.is_empty());
        assert!((player.current_time - 2.5).abs() < 1e-5);
        // 合計 12.5 秒 → LOOP で 12.5 % 10 = 2.5
        player.update(&kf, 10.0, &mut pose);
        assert!((player.current_time - 2.5).abs() < 1e-5);
    }

    /// `AnimationPlayer` が CycleType::CLAMP で終端に達したとき `finished` を立てることを検証する。
    #[test]
    fn test_animation_player_clamp_finishes() {
        let clips = AnimationClip {
            name: "clamp".to_string(),
            start_time: 0.0,
            stop_time: 10.0,
            duration: 10.0,
            cycle_type: 2, // CLAMP
            frequency: 1.0,
            channels: HashMap::new(),
        };
        let mut player = AnimationPlayer::new(clips);
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
                strings: vec![],
            },
            blocks: vec![],
        };
        let mut pose = SkeletonPose::default();

        player.update(&kf, 4.0, &mut pose);
        assert!(!player.finished);
        assert!((player.current_time - 4.0).abs() < 1e-5);

        player.update(&kf, 8.0, &mut pose); // 合計 12 >= duration 10
        assert!(player.finished);
        // CLAMP なので終端 10 に固定
        assert!((player.current_time - 10.0).abs() < 1e-5);
    }
}
