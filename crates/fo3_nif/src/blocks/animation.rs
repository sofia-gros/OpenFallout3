//! # アニメーションブロック群のパーサー
//!
//! Fallout 3 のアニメーションファイル (.kf) および NIF に含まれる
//! コントローラー、インターポレーター、キーフレームデータブロック群。
//!
//! 参照元:
//! - `references/nifxml/nif.xml:L3248` (`NiTransformInterpolator`)
//! - `references/nifxml/nif.xml:L5274` (`NiTransformData`)
//! - `references/nifxml/nif.xml:L4327` (`NiKeyframeData`)
//! - `references/nifxml/nif.xml:L5169` (`NiStringPalette`)
//! - `references/nifxml/nif.xml:L1919` (`ControlledBlock`)
//! - `references/nifxml/nif.xml:L4214` (`NiControllerSequence`)

use std::io::{self, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::types::{
    KeyGroup, KeyType, NiQuatTransform, QuatKey, Vector3,
};


/// 文字列パレット（0x00 区切りの文字列バッファ）。
///
/// 参照元: `references/nifxml/nif.xml:L5169` (`NiStringPalette`), `L1985` (`StringPalette`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiStringPalette {
    /// 0x00 区切りの生文字列バッファ
    pub palette: Vec<u8>,
}

impl NiStringPalette {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let palette_len = reader.read_u32::<LittleEndian>()? as usize;
        let mut palette = vec![0u8; palette_len];
        reader.read_exact(&mut palette)?;
        // 繰り返し長 (length) フィールド
        let _repeated_len = reader.read_u32::<LittleEndian>()?;
        Ok(NiStringPalette { palette })
    }

    /// 指定オフセットからゼロ終端文字列を取得する。
    pub fn get_string_at(&self, offset: u32) -> Option<String> {
        let start = offset as usize;
        if start >= self.palette.len() {
            return None;
        }
        let end = self.palette[start..]
            .iter()
            .position(|&b| b == 0)
            .map(|pos| start + pos)
            .unwrap_or(self.palette.len());
        Some(String::from_utf8_lossy(&self.palette[start..end]).to_string())
    }
}

/// トランスフォームインターポレーター。
///
/// 参照元: `references/nifxml/nif.xml:L3248` (`NiTransformInterpolator`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiTransformInterpolator {
    /// デフォルト姿勢 / 固定トランスフォーム
    pub transform: NiQuatTransform,
    /// キーフレームデータ (`NiTransformData`) への参照 (Ref)
    pub data: i32,
}

impl NiTransformInterpolator {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let transform = NiQuatTransform::read(reader)?;
        let data = reader.read_i32::<LittleEndian>()?;
        Ok(NiTransformInterpolator { transform, data })
    }
}

/// トランスフォームアニメーションキーフレームデータ。
///
/// 参照元: `references/nifxml/nif.xml:L5274` (`NiTransformData`), `L4327` (`NiKeyframeData`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiTransformData {
    /// 回転補間タイプ
    pub rotation_type: KeyType,
    /// クォータニオン回転キーフレーム（rotation_type != XyzRotation の場合）
    pub quaternion_keys: Vec<QuatKey>,
    /// XYZ 各軸個別回転キーフレーム（rotation_type == XyzRotation の場合）
    pub xyz_rotations: [KeyGroup<f32>; 3],
    /// 移動キーフレーム
    pub translations: KeyGroup<Vector3>,
    /// スケールキーフレーム
    pub scales: KeyGroup<f32>,
}

impl NiTransformData {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let num_rot_keys = reader.read_u32::<LittleEndian>()? as usize;
        let mut rotation_type = KeyType::Linear;
        let mut quaternion_keys = Vec::new();
        let mut xyz_rotations = [KeyGroup::default(), KeyGroup::default(), KeyGroup::default()];

        if num_rot_keys > 0 {
            rotation_type = KeyType::from_u32(reader.read_u32::<LittleEndian>()?);
            if rotation_type != KeyType::XyzRotation {
                quaternion_keys.reserve(num_rot_keys);
                for _ in 0..num_rot_keys {
                    quaternion_keys.push(QuatKey::read(reader, rotation_type)?);
                }
            } else {
                // X, Y, Z 各軸の KeyGroup<f32>
                for i in 0..3 {
                    xyz_rotations[i] = KeyGroup::<f32>::read(reader)?;
                }
            }
        }

        let translations = KeyGroup::<Vector3>::read(reader)?;
        let scales = KeyGroup::<f32>::read(reader)?;

        Ok(NiTransformData {
            rotation_type,
            quaternion_keys,
            xyz_rotations,
            translations,
            scales,
        })
    }
}

/// 制御対象ボーンとインターポレーターの関連付け構造体。
///
/// 参照元: `references/nifxml/nif.xml:L1919` (`ControlledBlock`)
/// 注意: Fallout 3 (バージョン 20.2.0.7) では文字列がヘッダーの文字列テーブルインデックスとして格納される。
#[derive(Clone, Debug, PartialEq)]
pub struct ControlledBlock {
    /// インターポレーターへの参照 (Ref)
    pub interpolator: i32,
    /// コントローラーへの参照 (Ref)
    pub controller: i32,
    /// 優先度 (0-255)
    pub priority: u8,
    /// 制御対象ノード名（ヘッダー文字列インデックス）
    pub node_name_index: i32,
    /// プロパティ種別（ヘッダー文字列インデックス）
    pub property_type_index: i32,
    /// コントローラー種別（ヘッダー文字列インデックス）
    pub controller_type_index: i32,
    /// コントローラー ID（ヘッダー文字列インデックス）
    pub controller_id_index: i32,
    /// インターポレーター ID（ヘッダー文字列インデックス）
    pub interpolator_id_index: i32,
}

impl ControlledBlock {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let interpolator = reader.read_i32::<LittleEndian>()?;
        let controller = reader.read_i32::<LittleEndian>()?;
        let priority = reader.read_u8()?;
        let node_name_index = reader.read_i32::<LittleEndian>()?;
        let property_type_index = reader.read_i32::<LittleEndian>()?;
        let controller_type_index = reader.read_i32::<LittleEndian>()?;
        let controller_id_index = reader.read_i32::<LittleEndian>()?;
        let interpolator_id_index = reader.read_i32::<LittleEndian>()?;

        Ok(ControlledBlock {
            interpolator,
            controller,
            priority,
            node_name_index,
            property_type_index,
            controller_type_index,
            controller_id_index,
            interpolator_id_index,
        })
    }
}

/// アニメーションシーケンス（KF ファイルのルートブロック）。
///
/// 参照元: `references/nifxml/nif.xml:L4214` (`NiControllerSequence`), `L4201` (`NiSequence`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiControllerSequence {
    /// シーケンス名（グローバル文字列プールへのインデックス）
    pub name_index: i32,
    /// アレイ拡張単位
    pub array_grow_by: u32,
    /// 制御対象ブロック配列
    pub controlled_blocks: Vec<ControlledBlock>,
    /// ブレンド重み
    pub weight: f32,
    /// テキストキー (`NiTextKeyExtraData`) への参照 (Ref)
    pub text_keys: i32,
    /// サイクルタイプ (0: Loop, 1: Reverse, 2: Clamp)
    pub cycle_type: u32,
    /// 周波数 / 再生速度スケール
    pub frequency: f32,
    /// 開始時刻 (秒)
    pub start_time: f32,
    /// 停止時刻 (秒)
    pub stop_time: f32,
    /// マネージャー (`NiControllerManager`) への参照 (Ptr)
    pub manager: i32,
    /// 累積ルートノード名（グローバル文字列プールへのインデックス）
    pub accum_root_name_index: i32,
    /// アニメーションノート参照配列
    pub anim_note_arrays: Vec<i32>,
}

impl NiControllerSequence {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        // NiSequence 基本フィールド
        // NIF >= 20.1.0.1 では string は 4 バイトの文字列インデックス
        let name_index = reader.read_i32::<LittleEndian>()?;
        let num_controlled_blocks = reader.read_u32::<LittleEndian>()? as usize;
        let array_grow_by = reader.read_u32::<LittleEndian>()?;

        let mut controlled_blocks = Vec::with_capacity(num_controlled_blocks);
        for _ in 0..num_controlled_blocks {
            controlled_blocks.push(ControlledBlock::read(reader)?);
        }

        // NiControllerSequence 追加フィールド
        let weight = reader.read_f32::<LittleEndian>()?;
        let text_keys = reader.read_i32::<LittleEndian>()?;
        let cycle_type = reader.read_u32::<LittleEndian>()?;
        let frequency = reader.read_f32::<LittleEndian>()?;
        let start_time = reader.read_f32::<LittleEndian>()?;
        let stop_time = reader.read_f32::<LittleEndian>()?;
        let manager = reader.read_i32::<LittleEndian>()?;
        let accum_root_name_index = reader.read_i32::<LittleEndian>()?;

        // Fallout 3 (BSVersion 34 > 28): Num Anim Note Arrays (ushort) + Ref[]
        let num_anim_notes = reader.read_u16::<LittleEndian>()? as usize;
        let mut anim_note_arrays = Vec::with_capacity(num_anim_notes);
        for _ in 0..num_anim_notes {
            anim_note_arrays.push(reader.read_i32::<LittleEndian>()?);
        }

        Ok(NiControllerSequence {
            name_index,
            array_grow_by,
            controlled_blocks,
            weight,
            text_keys,
            cycle_type,
            frequency,
            start_time,
            stop_time,
            manager,
            accum_root_name_index,
            anim_note_arrays,
        })
    }
}

/// Bスプライン基底データ。
///
/// 参照元: `references/nifxml/nif.xml:L4103` (`NiBSplineBasisData`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiBSplineBasisData {
    /// 制御点数
    pub num_control_points: u32,
}

impl NiBSplineBasisData {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let num_control_points = reader.read_u32::<LittleEndian>()?;
        Ok(NiBSplineBasisData { num_control_points })
    }
}

/// Bスプライン制御点データ配列（浮動小数点またはコンパクト整数）。
///
/// 参照元: `references/nifxml/nif.xml:L4154` (`NiBSplineData`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiBSplineData {
    /// 浮動小数点制御点配列
    pub float_control_points: Vec<f32>,
    /// コンパクト制御点配列（符号付き16bit整数, 0-1 を SHRT_MAX でスケール）
    pub compact_control_points: Vec<i16>,
}

impl NiBSplineData {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let num_floats = reader.read_u32::<LittleEndian>()? as usize;
        let mut float_control_points = Vec::with_capacity(num_floats);
        for _ in 0..num_floats {
            float_control_points.push(reader.read_f32::<LittleEndian>()?);
        }

        let num_compact = reader.read_u32::<LittleEndian>()? as usize;
        let mut compact_control_points = Vec::with_capacity(num_compact);
        for _ in 0..num_compact {
            compact_control_points.push(reader.read_i16::<LittleEndian>()?);
        }

        Ok(NiBSplineData {
            float_control_points,
            compact_control_points,
        })
    }
}

/// コンパクト制御点を用いた Bスプライン トランスフォームインターポレーター。
///
/// 参照元:
/// - `references/nifxml/nif.xml:L4141` (`NiBSplineCompTransformInterpolator`)
/// - `references/nifxml/nif.xml:L4132` (`NiBSplineTransformInterpolator`)
/// - `references/nifxml/nif.xml:L3342` (`NiBSplineInterpolator`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiBSplineCompTransformInterpolator {
    // NiBSplineInterpolator
    pub start_time: f32,
    pub stop_time: f32,
    pub spline_data: i32,
    pub basis_data: i32,

    // NiBSplineTransformInterpolator
    pub transform: NiQuatTransform,
    pub translation_handle: u32,
    pub rotation_handle: u32,
    pub scale_handle: u32,

    // NiBSplineCompTransformInterpolator
    pub translation_offset: f32,
    pub translation_half_range: f32,
    pub rotation_offset: f32,
    pub rotation_half_range: f32,
    pub scale_offset: f32,
    pub scale_half_range: f32,
}

impl NiBSplineCompTransformInterpolator {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        // NiBSplineInterpolator
        let start_time = reader.read_f32::<LittleEndian>()?;
        let stop_time = reader.read_f32::<LittleEndian>()?;
        let spline_data = reader.read_i32::<LittleEndian>()?;
        let basis_data = reader.read_i32::<LittleEndian>()?;

        // NiBSplineTransformInterpolator
        let transform = NiQuatTransform::read(reader)?;
        let translation_handle = reader.read_u32::<LittleEndian>()?;
        let rotation_handle = reader.read_u32::<LittleEndian>()?;
        let scale_handle = reader.read_u32::<LittleEndian>()?;

        // NiBSplineCompTransformInterpolator
        let translation_offset = reader.read_f32::<LittleEndian>()?;
        let translation_half_range = reader.read_f32::<LittleEndian>()?;
        let rotation_offset = reader.read_f32::<LittleEndian>()?;
        let rotation_half_range = reader.read_f32::<LittleEndian>()?;
        let scale_offset = reader.read_f32::<LittleEndian>()?;
        let scale_half_range = reader.read_f32::<LittleEndian>()?;

        Ok(NiBSplineCompTransformInterpolator {
            start_time,
            stop_time,
            spline_data,
            basis_data,
            transform,
            translation_handle,
            rotation_handle,
            scale_handle,
            translation_offset,
            translation_half_range,
            rotation_offset,
            rotation_half_range,
            scale_offset,
            scale_half_range,
        })
    }
}


