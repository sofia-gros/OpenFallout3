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
pub mod sequence;
pub mod facegen;
pub mod resource;
pub mod gpu_skin;

pub use vertex::{BonePaletteUniform, SkinnedVertex, Vertex, MAX_BONES_PER_PALETTE};
pub use gpu_skin::{GpuBonePalette, create_gpu_skin_mesh_from_partition};
pub use mesh::GpuMesh;
pub use texture::GpuTexture;
pub use resource::{NifCache, TextureCache, normalize_mesh_path, normalize_texture_path};
pub use facegen::{
    decode_dds_to_rgba8, parse_geometry_morph, parse_texture_morph, sample_texture_delta,
    synthesize_head_diffuse, apply_geometry_morph, GeometryMorph, TextureMorph,
};
pub use camera::{CameraUniform, OrbitCamera};
pub use lighting::{GpuPointLight, LightingUniform, PlacedPointLight};
pub use collision::{CollisionVertex, GpuCollisionMesh, extract_collision_lines, HAVOK_SCALE};
pub use pipeline::RenderContext;
pub use sequence::{SequenceManager, SequenceTrack, blend_bone_overrides, blend_poses, blend_multiple_poses};
pub use scene::{
    AnimatedRigidMesh, AnimatedSkinMesh, RenderActorInstance, RenderMesh, RenderScene,
    collect_bone_world_transforms, find_attach_bone_name, recompute_bone_world_map_with_pose,
    recompute_bone_world_maps_with_pose, resolve_bone_world_transforms,
    resolve_bone_world_transforms_by_name,
};
pub use skinning::{apply_skinning_cpu, apply_skinning_cpu_with_bones};
pub use animation::{
    AnimationClip, AnimationPlayer, BoneChannel, CycleType, SkeletonPose, apply_pose,
};
