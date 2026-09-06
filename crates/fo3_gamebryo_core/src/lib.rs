//! # fo3_gamebryo_core
//!
//! Gamebryo 2.6 の基本データ型、トランスフォーム計算、バウンディングボリュームの定義。
//! 参照元: Gamebryo 2.6 SDK NiTransform, NiBound, NiAVObject

use glam::{Mat3, Mat4, Vec3};

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
    /// ESM 内の REFR に記録されたオイラー角回転は、Gamebryo / Bethesda のワールド描画変換において
    /// 各軸の符号を反転（マイナス）して適用する。
    /// 回転行列合成順序: `R = R_z(-rz) * R_y(-ry) * R_x(-rx)`
    /// 参照元: `references/openmw/apps/opencs/view/render/object.cpp:165-168`, `references/nifskope/src/gl/glcontroller.cpp:489`
    pub fn from_euler_xyz(pos: Vec3, rot: Vec3, scale: f32) -> Self {
        let rx = Mat3::from_rotation_x(-rot.x);
        let ry = Mat3::from_rotation_y(-rot.y);
        let rz = Mat3::from_rotation_z(-rot.z);
        let rotation = rz * ry * rx;

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
        // Z 軸 270度 (3 * FRAC_PI_2) 回転
        let rot = Vec3::new(0.0, 0.0, 3.0 * FRAC_PI_2);
        let t = NiTransform::from_euler_xyz(Vec3::ZERO, rot, 1.0);

        // ベクトル [1.0, 0.0, 0.0] を回転
        // -rz = -270度 = +90度 なので [1, 0, 0] は [0, 1, 0] になる
        let rotated = t.rotation * Vec3::new(1.0, 0.0, 0.0);
        assert!((rotated.x - 0.0).abs() < 1e-5);
        assert!((rotated.y - 1.0).abs() < 1e-5);
        assert!((rotated.z - 0.0).abs() < 1e-5);
    }
}
