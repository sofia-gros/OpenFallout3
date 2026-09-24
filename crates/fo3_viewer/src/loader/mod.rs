//! シーン・アクター・コリジョン・ライティングのロードおよび GPU 初期化モジュール。
//!
//! 参照元: Gamebryo 2.6 セルグラフ構築, `knowledge/actor_and_skin_mesh.md`

mod actor;
mod cell;
mod finder;
mod interactable;
mod physics;

pub use cell::load_cell_targets;
pub use finder::find_cells_for_target;

use std::collections::HashMap;
use std::io::Cursor;
use fo3_esm::EsmMasterContext;
use fo3_esm::CellLighting;
use fo3_nif::collision::extract_collision_data;
use fo3_nif::NifFile;
use fo3_physics::RapierPhysicsWorld;
use fo3_render::{GpuTexture, NifCache, PlacedPointLight, RenderContext, RenderScene};
use fo3_vfs::VfsManager;

use crate::interact::InteractableObject;
use crate::interactive_anim::RefrBinding;
use crate::types::{get_actor_part_paths, ViewerTarget};

/// ロード結果をまとめた構造体。
pub struct LoadedSceneResult {
    pub scene: RenderScene,
    pub cell_lighting: Option<CellLighting>,
    pub placed_lights: Vec<PlacedPointLight>,
    pub clear_color: wgpu::Color,
    pub physics_world: RapierPhysicsWorld,
    pub door_spawn_point: Option<(glam::Vec3, f32)>,
    pub interactables: Vec<InteractableObject>,
    pub refr_bindings: HashMap<u32, RefrBinding>,
    pub markers: HashMap<String, (glam::Vec3, glam::Vec3)>,
}

/// 指定ターゲットのシーン、光源、物理ワールドを一括ロード・構築する。
///
/// 参照元: Gamebryo 2.6 `NiStream`, `NiSourceTexture`, `knowledge/gamebryo_resource_management_and_caching.md`
pub fn load_scene(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    context: &RenderContext,
    data_dir: &str,
    target: &ViewerTarget,
    vfs: &mut VfsManager,
    master_context: &EsmMasterContext,
    nif_cache: &mut NifCache,
    texture_cache: &mut HashMap<String, GpuTexture>,
) -> LoadedSceneResult {
    match target {
        ViewerTarget::Mesh(nif_path) | ViewerTarget::Anim { nif_path, .. } => {
            load_mesh_target(device, queue, context, nif_path, vfs)
        }
        ViewerTarget::Actor {
            outfit_or_naked,
            kf_path,
        } => load_actor_target(
            device,
            queue,
            context,
            outfit_or_naked,
            kf_path,
            vfs,
        ),
        ViewerTarget::Cell(..)
        | ViewerTarget::World(..)
        | ViewerTarget::WorldGrids(..)
        | ViewerTarget::NewGame { .. } => load_cell_targets(
            device,
            queue,
            context,
            data_dir,
            target,
            vfs,
            master_context,
            nif_cache,
            texture_cache,
        ),
    }
}

/// 単体 NIF メッシュ・アニメーションターゲットのロード。
fn load_mesh_target(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    context: &RenderContext,
    nif_path: &str,
    vfs: &mut VfsManager,
) -> LoadedSceneResult {
    println!("VFS から NIF ファイルを取得中: {}", nif_path);
    let nif_bytes = vfs.read(nif_path).expect("Failed to read NIF from VFS");
    let mut cursor = Cursor::new(nif_bytes);
    let nif_file = NifFile::read(&mut cursor).expect("Failed to parse NIF");
    println!(
        "NIF パース成功 (ブロック数: {})。GPU シーンを構築中...",
        nif_file.blocks.len()
    );
    let scene = RenderScene::from_nif(device, queue, context, &nif_file, vfs);

    let mut physics_world = RapierPhysicsWorld::new();
    let col_data = extract_collision_data(&nif_file);
    if !col_data.bodies.is_empty() {
        physics_world.add_nif_collision(&col_data, glam::Vec3::ZERO, glam::Quat::IDENTITY);
        println!(
            "物理ワールド登録: 単体メッシュ コリジョン剛体数 {}",
            col_data.bodies.len()
        );
    }

    LoadedSceneResult {
        scene,
        cell_lighting: None,
        placed_lights: Vec::new(),
        clear_color: wgpu::Color {
            r: 0.1,
            g: 0.12,
            b: 0.15,
            a: 1.0,
        },
        physics_world,
        door_spawn_point: None,
        interactables: Vec::new(),
        refr_bindings: HashMap::new(),
        markers: HashMap::new(),
    }
}

/// 人型アクター全身パーツ合成ターゲットのロード。
fn load_actor_target(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    context: &RenderContext,
    outfit_or_naked: &str,
    kf_path: &str,
    vfs: &mut VfsManager,
) -> LoadedSceneResult {
    println!(
        "人型アクター全身パーツ合成モード (outfit: {}, anim: {})",
        outfit_or_naked, kf_path
    );
    let body_path = if outfit_or_naked.eq_ignore_ascii_case("naked") {
        "meshes\\characters\\_male\\upperbody.nif".to_string()
    } else {
        outfit_or_naked.to_string()
    };

    let is_female = outfit_or_naked.to_ascii_lowercase().contains("female")
        || outfit_or_naked.to_ascii_lowercase().contains("outfitf");
    let part_paths = get_actor_part_paths(
        is_female,
        fo3_esm::FormId(0),
        &body_path,
        None,
        None,
        None,
        None,
        false,
    );

    let mut parts = Vec::new();
    for path in &part_paths {
        println!("VFS からアクターパーツ NIF を取得中: {}", path);
        match vfs.read(path) {
            Ok(bytes) => {
                let mut cursor = Cursor::new(bytes);
                match NifFile::read(&mut cursor) {
                    Ok(nif) => {
                        println!(
                            "  パーツロード成功: {} (ブロック数: {})",
                            path,
                            nif.blocks.len()
                        );
                        parts.push(nif);
                    }
                    Err(e) => eprintln!("  パーツパースエラー {}: {}", path, e),
                }
            }
            Err(e) => eprintln!("  パーツ読み込みエラー {}: {}", path, e),
        }
    }

    let skel_path = "meshes\\characters\\_male\\skeleton.nif";
    println!("スケルトン NIF をロード中: {}", skel_path);
    let skel_bytes = vfs
        .read(skel_path)
        .expect("スケルトン NIF の読み込みに失敗しました");
    let mut skel_cursor = Cursor::new(skel_bytes);
    let skel_nif =
        NifFile::read(&mut skel_cursor).expect("スケルトン NIF のパースに失敗しました");

    let part_refs: Vec<&NifFile> = parts.iter().collect();
    let scene = RenderScene::from_actor_parts(
        device, queue, context, &skel_nif, &part_refs, vfs,
    );

    let mut physics_world = RapierPhysicsWorld::new();
    let col_data = extract_collision_data(&skel_nif);
    if !col_data.bodies.is_empty() {
        physics_world.add_nif_collision(&col_data, glam::Vec3::ZERO, glam::Quat::IDENTITY);
    }

    LoadedSceneResult {
        scene,
        cell_lighting: None,
        placed_lights: Vec::new(),
        clear_color: wgpu::Color {
            r: 0.1,
            g: 0.12,
            b: 0.15,
            a: 1.0,
        },
        physics_world,
        door_spawn_point: None,
        interactables: Vec::new(),
        refr_bindings: HashMap::new(),
        markers: HashMap::new(),
    }
}
