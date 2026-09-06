//! # fo3_gamebryo_core
//!
//! Gamebryo 2.6 の基本データ型、トランスフォーム計算、バウンディングボリュームの定義。
//! 参照元: Gamebryo 2.6 SDK NiTransform, NiBound, NiAVObject

use glam::{Mat3, Vec3};

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
}
