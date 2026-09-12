//! # fo3_physics
//!
//! Fallout 3 (Gamebryo 2.6 / Havok) 向け物理エンジン統合およびキャラクタコントローラー基盤。
//!
//! 将来的な物理エンジン（Havok 完全エミュレーション / Bullet Physics 等）の切り替えに対応した
//! 疎結合アーキテクチャを提供し、現段階では純粋 Rust 実装の **Rapier3D** を標準バックエンドとして統合しています。
//!
//! 参照元: `knowledge/physics_engine_evaluation_and_architecture.md`, `references/nifxml/nif.xml:L2287-3207`

pub mod collision_layers;
pub mod traits;
pub mod rapier;

pub use collision_layers::layer_to_interaction_groups;
pub use traits::{CharacterState, InteractionRayHit, PhysicsBackend, RayIntersection};
pub use rapier::{
    collision_shape_to_rapier, rigid_body_data_to_rapier,
    RapierCharacterController, RapierPhysicsWorld,
};
pub use rapier3d::prelude::RigidBodyHandle;

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Quat, Vec3};
    use fo3_nif::collision::{CollisionShape, NifCollisionData, RigidBodyData};
    use fo3_nif::{Fallout3HavokMaterial, Fallout3Layer};

    #[test]
    fn test_box_collider_conversion_and_raycast() {
        let mut world = RapierPhysicsWorld::new();

        // 床となる大きな Box 剛体 (Static) を生成: 1000 x 1000 x 10
        let floor_shape = CollisionShape::Box {
            half_extents: [500.0, 500.0, 5.0],
            center: [0.0, 0.0, 0.0],
            material: Fallout3HavokMaterial::Stone,
        };

        let floor_body = RigidBodyData {
            shape: floor_shape,
            mass: 0.0, // Static
            friction: 0.5,
            restitution: 0.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            layer: Fallout3Layer::Static,
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        };

        let col_data = NifCollisionData {
            bodies: vec![floor_body],
        };

        world.add_nif_collision(&col_data, Vec3::ZERO, Quat::IDENTITY);

        // 床の上空 Z = 100 から下向き (0, 0, -1) にレイキャスト
        let hit = world.cast_ray(Vec3::new(0.0, 0.0, 100.0), Vec3::new(0.0, 0.0, -1.0), 200.0);
        assert!(hit.is_some(), "床にレイがヒットしませんでした");
        let hit = hit.unwrap();
        // 床の上面は Z = 5.0
        assert!((hit.point.z - 5.0).abs() < 0.01, "ヒット位置が床の上面 Z=5.0 と一致しません: {:?}", hit.point);
        assert_eq!(hit.normal, Vec3::new(0.0, 0.0, 1.0), "床の法線が上向き (0, 0, 1) ではありません");
    }

    #[test]
    fn test_character_controller_grounding_and_gravity() {
        let mut world = RapierPhysicsWorld::new();

        // 床の生成 (Z = 0.0)
        let floor_shape = CollisionShape::Box {
            half_extents: [500.0, 500.0, 10.0],
            center: [0.0, 0.0, -10.0], // 上面が Z = 0.0
            material: Fallout3HavokMaterial::Stone,
        };
        let floor_body = RigidBodyData {
            shape: floor_shape,
            mass: 0.0,
            friction: 0.5,
            restitution: 0.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            layer: Fallout3Layer::Static,
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        };
        world.add_nif_collision(&NifCollisionData { bodies: vec![floor_body] }, Vec3::ZERO, Quat::IDENTITY);

        // キャラクタを地上 Z = 100.0 に配置 (全高 約 128, 半身 44 + 半径 20 = 64)
        // 底面は 100.0 - 64.0 = 36.0 (まだ空中)
        let mut controller = RapierCharacterController::new(Vec3::new(0.0, 0.0, 100.0));

        // 空中状態の確認 (下向きにわずかに移動してもまだ接地しない)
        let state = controller.step_move(
            1.0 / 60.0,
            Vec3::new(0.0, 0.0, -10.0),
            &world.rigid_body_set,
            &world.collider_set,
            &world.query_pipeline,
        );
        assert!(!state.is_grounded, "空中で接地フラグが立っています");

        // 重力落下を模して大きく下向きに移動 (床 Z = 0.0 に衝突して止まるはず)
        let state = controller.step_move(
            1.0 / 60.0,
            Vec3::new(0.0, 0.0, -100.0),
            &world.rigid_body_set,
            &world.collider_set,
            &world.query_pipeline,
        );

        // 床に接地したはず
        assert!(state.is_grounded, "床に到達したのに接地フラグが立っていません");
        // 底面が床 Z = 0.0 に接するため、中心 Z は 64.0 付近
        assert!((controller.position.z - 64.0).abs() < 1.0, "キャラクタの接地中心高度が不正です: {}", controller.position.z);
    }

    /// インタラクション用レイキャストのターゲット検出および遮蔽（オクルージョン）判定の検証。
    #[test]
    fn test_interaction_raycast_and_occlusion() {
        let mut world = RapierPhysicsWorld::new();

        // 1. 手前の遮蔽壁 (X = 50 に配置, user_data = 0)
        let wall_shape = CollisionShape::Box {
            half_extents: [5.0, 100.0, 100.0],
            center: [0.0, 0.0, 0.0],
            material: Fallout3HavokMaterial::Stone,
        };
        let wall_body = RigidBodyData {
            shape: wall_shape,
            mass: 0.0,
            friction: 0.5,
            restitution: 0.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            layer: Fallout3Layer::Static,
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        };
        let wall_data = NifCollisionData { bodies: vec![wall_body] };
        world.add_nif_collision_with_user_data(&wall_data, Vec3::new(50.0, 0.0, 0.0), Quat::IDENTITY, 0);

        // 2. 奥のアイテム (X = 100 に配置, user_data = 0x00012345)
        let item_shape = CollisionShape::Box {
            half_extents: [10.0, 10.0, 10.0],
            center: [0.0, 0.0, 0.0],
            material: Fallout3HavokMaterial::Metal,
        };
        let item_body = RigidBodyData {
            shape: item_shape,
            mass: 0.0,
            friction: 0.5,
            restitution: 0.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            layer: Fallout3Layer::Static,
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        };
        let item_data = NifCollisionData { bodies: vec![item_body] };
        world.add_nif_collision_with_user_data(&item_data, Vec3::new(100.0, 0.0, 0.0), Quat::IDENTITY, 0x00012345);

        // 始点 (0, 0, 0) から +X 方向へレイキャスト (距離 180.0)
        let hit = world.cast_ray_interaction(Vec3::ZERO, Vec3::X, 180.0);
        assert!(hit.is_some(), "レイがヒットしませんでした");
        let hit_res = hit.unwrap();
        // 手前の壁 (X=50, 半径5で表面 X=45) にヒットするため user_data は 0 (壁) であり、奥のアイテムは遮蔽される
        assert_eq!(hit_res.user_data, 0, "手前の壁で遮蔽されず奥のアイテムにヒットしてしまいました");
        assert!((hit_res.point.x - 45.0).abs() < 1e-3, "壁の手前表面にヒットしていません");

        // 壁の無い方向 (+Y 方向) に別アイテム (Y = 100, user_data = 0x00054321) を配置
        world.add_nif_collision_with_user_data(&item_data, Vec3::new(0.0, 100.0, 0.0), Quat::IDENTITY, 0x00054321);

        // 始点 (0, 0, 0) から +Y 方向へレイキャスト
        let hit_y = world.cast_ray_interaction(Vec3::ZERO, Vec3::Y, 180.0);
        assert!(hit_y.is_some(), "+Y 方向のアイテムにヒットしませんでした");
        assert_eq!(hit_y.unwrap().user_data, 0x00054321, "遮蔽物のないアイテムの FormID が取得できませんでした");

        // 距離超過テスト: 最大距離 50.0 では Y=100 のアイテムに届かない
        let hit_short = world.cast_ray_interaction(Vec3::ZERO, Vec3::Y, 50.0);
        assert!(hit_short.is_none(), "最大距離を超過しているのにヒットしてしまいました");
    }
}
