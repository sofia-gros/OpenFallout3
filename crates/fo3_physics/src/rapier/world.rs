//! # Rapier 物理ワールドマネージャー
//!
//! 剛体シミュレーション、NIF コリジョンの追加、レイキャスト、シミュレーションステップ実行を統括。
//!
//! 参照元: `knowledge/physics_engine_evaluation_and_architecture.md`

use glam::{Quat, Vec3};
use rapier3d::na::{Point3, Vector3};
use rapier3d::prelude::*;
use fo3_nif::collision::NifCollisionData;
use crate::rapier::adapter::rigid_body_data_to_rapier;
use crate::traits::RayIntersection;

/// Rapier3D 物理ワールド。
pub struct RapierPhysicsWorld {
    pub gravity: Vector3<Real>,
    pub integration_parameters: IntegrationParameters,
    pub physics_pipeline: PhysicsPipeline,
    pub island_manager: IslandManager,
    pub broad_phase: DefaultBroadPhase,
    pub narrow_phase: NarrowPhase,
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,
    pub impulse_joint_set: ImpulseJointSet,
    pub multibody_joint_set: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
    pub query_pipeline: QueryPipeline,
}

impl Default for RapierPhysicsWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl RapierPhysicsWorld {
    /// 新しい物理ワールドを初期化する。
    ///
    /// Fallout 3 の重力 (Gamebryo 単位: 1 unit $\approx 1.4$cm、重力 $g \approx -9.81$ m/s$^2 \approx -700$ units/s$^2$)
    pub fn new() -> Self {
        // Fallout 3 は右手系 Z-up のため、重力は Z 軸負方向
        let gravity = Vector3::new(0.0, 0.0, -700.0);

        Self {
            gravity,
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            query_pipeline: QueryPipeline::new(),
        }
    }

    /// 物理シミュレーションを 1 ステップ進める。
    pub fn step(&mut self, dt: f32) {
        self.integration_parameters.dt = dt;

        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            Some(&mut self.query_pipeline),
            &(),
            &(),
        );

        // クエリパイプラインの更新
        self.query_pipeline.update(&self.collider_set);
    }

    /// NIF コリジョンデータを物理ワールドに登録する。
    pub fn add_nif_collision(
        &mut self,
        nif_col: &NifCollisionData,
        world_pos: Vec3,
        world_rot: Quat,
    ) -> Vec<(RigidBodyHandle, ColliderHandle)> {
        let mut handles = Vec::new();

        for body_data in &nif_col.bodies {
            if let Some((rb_builder, col_builder)) =
                rigid_body_data_to_rapier(body_data, world_pos, world_rot)
            {
                let rb_handle = self.rigid_body_set.insert(rb_builder);
                let col_handle = self.collider_set.insert_with_parent(
                    col_builder,
                    rb_handle,
                    &mut self.rigid_body_set,
                );
                handles.push((rb_handle, col_handle));
            }
        }

        // クエリパイプラインの即時更新
        self.query_pipeline.update(&self.collider_set);
        handles
    }

    /// 地形 (LAND) の 33x33 標高データから静的コライダー (TriMesh) を生成・登録する。
    ///
    /// 参照元: `fo3_esm::LAND_VERTS_PER_SIDE = 33`, `fo3_esm::LAND_REAL_SIZE = 4096.0`
    pub fn add_land_collision(
        &mut self,
        heights: &[f32],
        grid_x: i32,
        grid_y: i32,
    ) -> Option<(RigidBodyHandle, ColliderHandle)> {
        const VERTS_PER_SIDE: usize = 33;
        const REAL_SIZE: f32 = 4096.0;
        if heights.len() < VERTS_PER_SIDE * VERTS_PER_SIDE {
            return None;
        }

        let step = REAL_SIZE / (VERTS_PER_SIDE - 1) as f32; // 128.0
        let origin_x = grid_x as f32 * REAL_SIZE;
        let origin_y = grid_y as f32 * REAL_SIZE;

        let mut points = Vec::with_capacity(VERTS_PER_SIDE * VERTS_PER_SIDE);
        for y in 0..VERTS_PER_SIDE {
            for x in 0..VERTS_PER_SIDE {
                let idx = y * VERTS_PER_SIDE + x;
                let wx = origin_x + x as f32 * step;
                let wy = origin_y + y as f32 * step;
                let wz = heights[idx];
                points.push(Point3::new(wx, wy, wz));
            }
        }

        let num_quads = VERTS_PER_SIDE - 1; // 32
        let mut indices = Vec::with_capacity(num_quads * num_quads * 2);
        for y in 0..num_quads {
            for x in 0..num_quads {
                let i0 = (y * VERTS_PER_SIDE + x) as u32;
                let i1 = (y * VERTS_PER_SIDE + x + 1) as u32;
                let i2 = ((y + 1) * VERTS_PER_SIDE + x) as u32;
                let i3 = ((y + 1) * VERTS_PER_SIDE + x + 1) as u32;

                indices.push([i0, i2, i1]);
                indices.push([i1, i2, i3]);
            }
        }

        let shape = SharedShape::trimesh(points, indices);
        let rb_builder = RigidBodyBuilder::fixed();
        let col_builder = ColliderBuilder::new(shape)
            .collision_groups(crate::collision_layers::layer_to_interaction_groups(
                fo3_nif::blocks::Fallout3Layer::Static,
            ));

        let rb_handle = self.rigid_body_set.insert(rb_builder);
        let col_handle = self.collider_set.insert_with_parent(
            col_builder,
            rb_handle,
            &mut self.rigid_body_set,
        );

        self.query_pipeline.update(&self.collider_set);
        Some((rb_handle, col_handle))
    }

    /// ワールドに対してレイキャストを実行する。
    pub fn cast_ray(&self, origin: Vec3, dir: Vec3, max_toi: f32) -> Option<RayIntersection> {
        let ray_origin = Point3::new(origin.x, origin.y, origin.z);
        let ray_dir = Vector3::new(dir.x, dir.y, dir.z);
        let ray = Ray::new(ray_origin, ray_dir);

        let filter = QueryFilter::default();

        if let Some((_, hit)) = self.query_pipeline.cast_ray_and_get_normal(
            &self.rigid_body_set,
            &self.collider_set,
            &ray,
            max_toi,
            true,
            filter,
        ) {
            let hit_point = origin + dir * hit.time_of_impact;
            let normal = Vec3::new(hit.normal.x, hit.normal.y, hit.normal.z);
            Some(RayIntersection {
                point: hit_point,
                normal,
                distance: hit.time_of_impact,
            })
        } else {
            None
        }
    }
}
