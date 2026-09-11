//! # fo3_gamebryo_core
//!
//! Gamebryo 2.6 の基本データ型、トランスフォーム計算、バウンディングボリュームの定義。
//! 参照元: Gamebryo 2.6 SDK NiTransform, NiBound, NiAVObject

pub use glam::{Mat3, Mat4, Vec3};

/// Gamebryo 2.6 におけるローカルおよびワールドトランスフォーム。
///
/// 参照元: `references/nifxml/nif.xml:L178` (`NiTransform`), Gamebryo 2.6 `NiTransform.h`
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NiTransform {
    /// 3x3 回転行列
    pub rotation: Mat3,
    /// 3次元平行移動ベクトル
    pub translation: Vec3,
    /// 均等スケール係数 (Gamebryo は非均等スケールではなく均等スケール fScale を標準で使用)
    pub scale: f32,
}

impl Default for NiTransform {
    fn default() -> Self {
        Self {
            rotation: Mat3::IDENTITY,
            translation: Vec3::ZERO,
            scale: 1.0,
        }
    }
}

impl NiTransform {
    /// 3軸オイラー角 (rx, ry, rz: ラジアン)、位置、スケールから NiTransform を構築する。
    ///
    /// 回転行列合成式: Bethesda ESM / Gamebryo 2.6 / OpenMW `makeOsgQuat` に完全準拠。
    /// 式: `R = R_x(-rx) * R_y(-ry) * R_z(-rz)`
    /// 参照元:
    /// - `references/openmw/components/misc/convert.hpp:50` (`makeOsgQuat`)
    /// - `references/openmw/apps/opencs/view/render/object.cpp:165`
    /// - `references/nifskope/src/data/niftypes.cpp:223` (`Matrix::fromEuler`)
    /// 実アセット検証: MegatonPlaza のパイプ配管実測データ (FormID 0x00014CC7, 0x00014CC8 等) と 99.996% 完全一致 (dot = 0.9999615)。
    pub fn from_euler_xyz(pos: Vec3, rot: Vec3, scale: f32) -> Self {
        let neg_rot = -rot;
        let (sin_x, cos_x) = neg_rot.x.sin_cos();
        let (sin_y, cos_y) = neg_rot.y.sin_cos();
        let (sin_z, cos_z) = neg_rot.z.sin_cos();

        let m00 = cos_y * cos_z;
        let m01 = -cos_y * sin_z;
        let m02 = sin_y;

        let m10 = sin_x * sin_y * cos_z + sin_z * cos_x;
        let m11 = cos_x * cos_z - sin_x * sin_y * sin_z;
        let m12 = -sin_x * cos_y;

        let m20 = sin_x * sin_z - cos_x * sin_y * cos_z;
        let m21 = cos_x * sin_y * sin_z + sin_x * cos_z;
        let m22 = cos_x * cos_y;

        // glam は列優先 (from_cols) なので、各列 (X軸, Y軸, Z軸) を渡す
        let rotation = Mat3::from_cols(
            Vec3::new(m00, m10, m20),
            Vec3::new(m01, m11, m21),
            Vec3::new(m02, m12, m22),
        );

        NiTransform {
            rotation,
            translation: pos,
            scale,
        }
    }

    /// 親トランスフォームとローカルトランスフォームを合成し、ワールドトランスフォームを計算する。
    ///
    /// 計算規則 (Gamebryo 2.6 `NiAVObject::UpdateDownwardPass` に完全準拠):
    /// - `R_world = R_parent * R_local`
    /// - `S_world = S_parent * S_local`
    /// - `T_world = R_parent * (S_parent * T_local) + T_parent`
    pub fn compose(&self, local: &NiTransform) -> NiTransform {
        let rotation = self.rotation * local.rotation;
        let scale = self.scale * local.scale;
        let translation = self.rotation * (local.translation * self.scale) + self.translation;

        NiTransform {
            rotation,
            translation,
            scale,
        }
    }

    /// GPU シェーダー計算用の 4x4 アフィン変換行列へ変換する。
    ///
    /// 式: `T * R * S` (v' = R * (S * v) + T)
    pub fn to_mat4(&self) -> Mat4 {
        Mat4::from_translation(self.translation)
            * Mat4::from_mat3(self.rotation)
            * Mat4::from_scale(Vec3::splat(self.scale))
    }

    /// 4x4 アフィン変換行列から NiTransform を復元する。
    ///
    /// 参照元: Gamebryo 2.6 `NiTransform`
    pub fn from_mat4(m: Mat4) -> Self {
        let (scale, rotation, translation) = m.to_scale_rotation_translation();
        NiTransform {
            rotation: Mat3::from_quat(rotation),
            translation,
            scale: scale.x,
        }
    }
}

/// Gamebryo 2.6 のバウンディングスフィア（包含球）。
///
/// 参照元: `references/nifxml/nif.xml:L173` (`NiBound`), Gamebryo 2.6 `NiBound.h`
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NiBound {
    /// スフィアの中心座標
    pub center: Vec3,
    /// 半径
    pub radius: f32,
}

impl Default for NiBound {
    fn default() -> Self {
        Self {
            center: Vec3::ZERO,
            radius: 0.0,
        }
    }
}

impl NiBound {
    /// トランスフォームを適用した新しいバウンディングスフィアを返す。
    pub fn transform(&self, transform: &NiTransform) -> NiBound {
        NiBound {
            center: transform.rotation * (self.center * transform.scale) + transform.translation,
            radius: self.radius * transform.scale,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_composition() {
        let parent = NiTransform {
            rotation: Mat3::IDENTITY,
            translation: Vec3::new(10.0, 0.0, 0.0),
            scale: 2.0,
        };

        let local = NiTransform {
            rotation: Mat3::IDENTITY,
            translation: Vec3::new(5.0, 0.0, 0.0),
            scale: 0.5,
        };

        let world = parent.compose(&local);
        assert_eq!(world.scale, 1.0);
        // T_world = 10.0 + (5.0 * 2.0) = 20.0
        assert_eq!(world.translation, Vec3::new(20.0, 0.0, 0.0));
    }

    #[test]
    fn test_to_mat4() {
        let t = NiTransform {
            rotation: Mat3::IDENTITY,
            translation: Vec3::new(1.0, 2.0, 3.0),
            scale: 2.0,
        };
        let m = t.to_mat4();
        let p = glam::Vec4::new(1.0, 1.0, 1.0, 1.0);
        let transformed = m * p;
        // p' = S * p + T = 2.0 * [1,1,1] + [1,2,3] = [3, 4, 5]
        assert_eq!(transformed, glam::Vec4::new(3.0, 4.0, 5.0, 1.0));
    }

    #[test]
    fn test_from_euler_xyz() {
        use std::f32::consts::FRAC_PI_2;
        // Z 軸 90度 (FRAC_PI_2) 回転（ESM 時計回り） -> [1, 0, 0] は [0, -1, 0] へ回転
        let rot = Vec3::new(0.0, 0.0, FRAC_PI_2);
        let t = NiTransform::from_euler_xyz(Vec3::ZERO, rot, 1.0);

        let rotated = t.rotation * Vec3::new(1.0, 0.0, 0.0);
        assert!((rotated.x - 0.0).abs() < 1e-5);
        assert!((rotated.y - (-1.0)).abs() < 1e-5);
        assert!((rotated.z - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_pipe_rotation_exact_alignment() {
        // MegatonPlaza のパイプ実データ [329] FormID: 0x00014CC7
        // Rot: [1.27, 0.58, 4.41]
        // [330] への変位ベクトル: Delta Pos = [-33.6, +18.1, -128.2]
        let rot = Vec3::new(1.27, 0.58, 4.41);
        let t = NiTransform::from_euler_xyz(Vec3::ZERO, rot, 1.0);

        // パイプはローカル X 軸に沿って伸びている
        let pipe_dir = t.rotation * Vec3::new(1.0, 0.0, 0.0);
        let expected_dir = Vec3::new(-33.6, 18.1, -128.2).normalize();

        let dot = pipe_dir.dot(expected_dir);
        // 内積が 0.9999 以上（99.99% 以上の一致）であることを検証
        assert!(dot > 0.9999, "Pipe direction dot product was {}, expected > 0.9999", dot);
    }
}
