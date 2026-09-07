//! # fo3_render
//!
//! Fallout 3 (Gamebryo 2.6) 向け wgpu レンダリングコアライブラリ。
//!
//! - `vertex`: GPU 頂点レイアウト (`Vertex`)
//! - `mesh`: NIF ジオメトリから GPU バッファへの変換 (`GpuMesh`)
//! - `texture`: DDS テクスチャ読み込みと GPU アップロード (`GpuTexture`)
//! - `camera`: Z-up 座標系オービットカメラ (`OrbitCamera`)
//! - `pipeline`: wgpu パイプライン・シェーダー管理 (`RenderContext`)
//! - `scene`: NIF シーングラフトラバース & 描画ノード構築 (`RenderScene`)

pub mod vertex;
pub mod mesh;
pub mod texture;
pub mod camera;
pub mod lighting;
pub mod collision;
pub mod pipeline;
pub mod scene;
pub mod skinning;
pub mod animation;

pub use vertex::Vertex;
pub use mesh::GpuMesh;
pub use texture::GpuTexture;
pub use camera::{CameraUniform, OrbitCamera};
pub use lighting::{GpuPointLight, LightingUniform, PlacedPointLight};
pub use collision::{CollisionVertex, GpuCollisionMesh, extract_collision_lines, HAVOK_SCALE};
pub use pipeline::RenderContext;
pub use scene::{RenderMesh, RenderScene, recompute_bone_world_map_with_pose};
pub use skinning::{apply_skinning_cpu, apply_skinning_cpu_with_bones};
pub use animation::{
    AnimationClip, AnimationPlayer, BoneChannel, CycleType, SkeletonPose, apply_pose,
};
