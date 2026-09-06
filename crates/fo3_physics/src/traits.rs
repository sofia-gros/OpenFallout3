//! # 物理エンジン抽象化トレイト
//!
//! 将来的な物理エンジン（Havok 完全エミュレーション / Bullet Physics 等）への
//! 切り替えを可能にするインターフェース定義。
//!
//! 参照元: `knowledge/physics_engine_evaluation_and_architecture.md`

use glam::Vec3;

/// 物理エンジンのバックエンド種別。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PhysicsBackend {
    /// 純粋 Rust 実装 Rapier3D (デフォルト)
    #[default]
    Rapier3D,
    /// Gamebryo 2.6 / Havok 完全エミュレーション (将来対応)
    HavokEmu,
    /// Bullet Physics (将来対応)
    Bullet,
}

/// レイキャスト交差結果。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayIntersection {
    /// ヒットしたワールド座標位置 [X, Y, Z]
    pub point: Vec3,
    /// ヒット面の法線ベクトル [Nx, Ny, Nz]
    pub normal: Vec3,
    /// レイの始点からの距離
    pub distance: f32,
}

/// キャラクタコントローラーのシミュレーション状態。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterState {
    /// 地面に接地しているか
    pub is_grounded: bool,
    /// 障害物・スロープ滑り・衝突を考慮した実際の移動変位ベクトル
    pub effective_translation: Vec3,
}
