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

pub mod animation;
pub mod camera;
pub mod collision;
pub mod facegen;
pub mod gpu_skin;
pub mod lighting;
pub mod mesh;
pub mod pipeline;
pub mod resource;
pub mod scene;
pub mod sequence;
pub mod skinning;
pub mod texture;
pub mod ui;
pub mod ui_xml;
pub mod vertex;

pub use animation::{
    apply_pose, AnimationClip, AnimationPlayer, BoneChannel, CycleType, SkeletonPose,
};
pub use camera::{CameraUniform, OrbitCamera};
pub use collision::{extract_collision_lines, CollisionVertex, GpuCollisionMesh, HAVOK_SCALE};
pub use facegen::{
    apply_geometry_morph, decode_dds_to_rgba8, parse_geometry_morph, parse_texture_morph,
    sample_texture_delta, synthesize_head_diffuse, GeometryMorph, TextureMorph,
};
pub use gpu_skin::{create_gpu_skin_mesh_from_partition, GpuBonePalette};
pub use lighting::{GpuPointLight, LightingUniform, PlacedPointLight};
pub use mesh::GpuMesh;
pub use pipeline::RenderContext;
pub use resource::{normalize_mesh_path, normalize_texture_path, NifCache, TextureCache};
pub use scene::{
    collect_bone_world_transforms, find_attach_bone_name, recompute_bone_world_map_with_pose,
    recompute_bone_world_maps_with_pose, resolve_bone_world_transforms,
    resolve_bone_world_transforms_by_name, AnimatedRigidMesh, AnimatedSkinMesh,
    RenderActorInstance, RenderMesh, RenderScene,
};
pub use sequence::{
    blend_bone_overrides, blend_multiple_poses, blend_poses, SequenceManager, SequenceTrack,
};
pub use skinning::{apply_skinning_cpu, apply_skinning_cpu_with_bones};
pub use texture::GpuTexture;
pub use ui::{colors as ui_colors, BitmapFont, GlyphMetrics, TextBatch, UiRenderer, UiVertex};
pub use ui_xml::{
    AtlasSubTexture, ComputedLayout, ExprOp, MenuNode, MenuRuntime, MenuXmlParser, NodeType,
    TextureAtlas, TraitSource, TraitValue,
};
pub use vertex::{BonePaletteUniform, SkinnedVertex, Vertex, MAX_BONES_PER_PALETTE};
