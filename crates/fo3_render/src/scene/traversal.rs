//! # シーングラフトラバースおよびカリング・ノード走査
//!
//! NIF シーングラフを DFS 走査し、各ノードのワールド変換合成、
//! マテリアル・アルファプロパティ継承、および描画メッシュ (`RenderMesh`) の生成を行う。
//! 参照元: Gamebryo 2.6 `NiAVObject::UpdateDownwardPass`, `NiNode::AttachChild`

use std::collections::HashMap;
use glam::Mat4;

use fo3_gamebryo_core::NiTransform;
use fo3_nif::{NifBlock, NifFile};
use fo3_vfs::VfsManager;

use crate::mesh::GpuMesh;
use crate::pipeline::RenderContext;
use crate::skinning::apply_skinning_cpu_with_bones;
use crate::texture::GpuTexture;

use crate::scene::actor::{AnimatedRigidMesh, AnimatedSkinMesh};
use crate::scene::bones::{
    collect_shape_transforms, compute_rigid_part_local_transform,
    resolve_bone_world_transforms, resolve_bone_world_transforms_by_name,
};
use crate::scene::mesh::{
    create_render_mesh, find_alpha_property, find_material_property, to_core_transform, RenderMesh,
};

/// アクターパーツのメッシュ走査時に、生成された RenderMesh と
/// スキン情報 (`AnimatedSkinMesh`) または剛体情報 (`AnimatedRigidMesh`) を 1:1 でダイレクト登録するコレクター。
///
/// 従来のメッシュ名逆引き (`position`) による同名・空文字列シェイプの未更新（Tポーズ腕残留・指先欠損）を完全に排除する。
/// 参照元: Gamebryo 2.6 `NiSkinInstance::Update`, `knowledge/actor_and_skin_mesh.md` (セクション 4.12)
pub struct ActorPartAnimCollector<'a> {
    pub part_index: usize,
    pub attach_bone: Option<&'a str>,
    pub is_hair: bool,
    pub has_hat: bool,
    pub tint_color: Option<[f32; 4]>,
    pub shape_transforms: HashMap<usize, Mat4>,
    pub anim_skins: &'a mut Vec<AnimatedSkinMesh>,
    pub anim_rigids: &'a mut Vec<AnimatedRigidMesh>,
}

impl<'a> ActorPartAnimCollector<'a> {
    pub fn new(
        part_index: usize,
        attach_bone: Option<&'a str>,
        is_hair: bool,
        has_hat: bool,
        tint_color: Option<[f32; 4]>,
        part_nif: &NifFile,
        anim_skins: &'a mut Vec<AnimatedSkinMesh>,
        anim_rigids: &'a mut Vec<AnimatedRigidMesh>,
    ) -> Self {
        let mut shape_transforms = HashMap::new();
        if attach_bone.is_some() {
            collect_shape_transforms(0, &NiTransform::default(), part_nif, &mut shape_transforms);
        }
        Self {
            part_index,
            attach_bone,
            is_hair,
            has_hat,
            tint_color,
            shape_transforms,
            anim_skins,
            anim_rigids,
        }
    }
}

/// NIF ブロック階層を再帰トラバースし、描画可能メッシュを `out_meshes` に収集する。
pub fn traverse_block(
    block_index: i32,
    parent_world: &NiTransform,
    parent_alpha: Option<&fo3_nif::NiAlphaProperty>,
    parent_material: Option<&fo3_nif::NiMaterialProperty>,
    nif: &NifFile,
    vfs: &mut VfsManager,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    context: &RenderContext,
    out_meshes: &mut Vec<RenderMesh>,
    texture_cache: &mut HashMap<String, GpuTexture>,
    bone_world_map: &mut HashMap<i32, Mat4>,
    skeleton_bone_name_map: Option<&HashMap<String, Mat4>>,
    default_texture: &GpuTexture,
    default_normal_texture: &GpuTexture,
    default_glow_texture: &GpuTexture,
    mut anim_collector: Option<&mut ActorPartAnimCollector>,
) {
    if block_index < 0 || block_index as usize >= nif.blocks.len() {
        return;
    }

    let block = &nif.blocks[block_index as usize];
    match block {
        NifBlock::NiNode(node) => {
            if is_node_hidden(&node.av, nif) {
                return;
            }
            let local_transform = to_core_transform(&node.av);
            let world_transform = parent_world.compose(&local_transform);
            // ボーン階層構築: 各 NiNode のワールド変換行列を block_index → Mat4 で蓄積
            // 参照元: Gamebryo 2.6 NiAVObject::UpdateDownwardPass
            bone_world_map.insert(block_index, world_transform.to_mat4());
            let current_alpha = find_alpha_property(&node.av.properties, nif).or(parent_alpha);
            let current_material = find_material_property(&node.av.properties, nif).or(parent_material);
            for &child in &node.children {
                traverse_block(
                    child,
                    &world_transform,
                    current_alpha,
                    current_material,
                    nif,
                    vfs,
                    device,
                    queue,
                    context,
                    out_meshes,
                    texture_cache,
                    bone_world_map,
                    skeleton_bone_name_map,
                    default_texture,
                    default_normal_texture,
                    default_glow_texture,
                    anim_collector.as_deref_mut(),
                );
            }
        }
        NifBlock::BSFadeNode(fade) => {
            if is_node_hidden(&fade.node.av, nif) {
                return;
            }
            let local_transform = to_core_transform(&fade.node.av);
            let world_transform = parent_world.compose(&local_transform);
            // BSFadeNode もボーン階層に含まれる場合があるため蓄積
            bone_world_map.insert(block_index, world_transform.to_mat4());
            let current_alpha = find_alpha_property(&fade.node.av.properties, nif).or(parent_alpha);
            let current_material = find_material_property(&fade.node.av.properties, nif).or(parent_material);
            for &child in &fade.node.children {
                traverse_block(
                    child,
                    &world_transform,
                    current_alpha,
                    current_material,
                    nif,
                    vfs,
                    device,
                    queue,
                    context,
                    out_meshes,
                    texture_cache,
                    bone_world_map,
                    skeleton_bone_name_map,
                    default_texture,
                    default_normal_texture,
                    default_glow_texture,
                    anim_collector.as_deref_mut(),
                );
            }
        }
        NifBlock::NiTriShape(shape) => {
            if is_node_hidden(&shape.geom.av, nif) {
                return;
            }
            // 四肢切断ゴアメッシュ（全パーティションが editor_visible=false の切断キャップ）は初期状態で非表示
            // 参照元: references/nifskope/build/nif.xml:L2530, knowledge/actor_and_skin_mesh.md (セクション 4.5)
            if is_dismember_hidden(shape.geom.skin_instance, nif) {
                return;
            }
            let name = nif.get_string(shape.geom.av.net.name_index).unwrap_or("").to_string();

            // 髪の毛パーツにおける帽子 (Hat) / 通常頭髪 (NoHat) の選択的カリング
            // 参照元: Fallout 3 髪 NIF 仕様 (hairbun.nif 等), knowledge/actor_and_skin_mesh.md
            if let Some(ref collector) = anim_collector {
                if collector.is_hair {
                    let name_lower = name.to_ascii_lowercase();
                    if collector.has_hat {
                        // 帽子着用時は通常髪 (nohat) を非表示にし、刈り込まれた hat を表示
                        if name_lower.contains("nohat") {
                            return;
                        }
                    } else {
                        // 帽子未着用時は hat を非表示にし、完全な nohat のみを表示
                        if name_lower == "hat" || (name_lower.contains("hat") && !name_lower.contains("nohat")) {
                            return;
                        }
                    }
                }
            }

            let local_transform = to_core_transform(&shape.geom.av);
            let world_transform = parent_world.compose(&local_transform);

            if shape.geom.data >= 0 && (shape.geom.data as usize) < nif.blocks.len() {
                if let NifBlock::NiTriShapeData(ref data) = nif.blocks[shape.geom.data as usize] {
                    // NiSkinInstance / BSDismemberSkinInstance が存在する場合は CPU スキニングを適用する
                    // 参照元: knowledge/actor_and_skin_mesh.md, Gamebryo 2.6 NiSkinInstance::Update
                    let gpu_mesh = if shape.geom.skin_instance >= 0 {
                        let inst_idx = shape.geom.skin_instance as usize;
                        if inst_idx < nif.blocks.len() {
                            let skin_inst_ref = match &nif.blocks[inst_idx] {
                                NifBlock::NiSkinInstance(ref inst) => Some(inst),
                                NifBlock::BSDismemberSkinInstance(ref bdsi) => Some(&bdsi.skin_instance),
                                _ => None,
                            };
                            if let Some(inst) = skin_inst_ref {
                                // スケルトンボーン名マップが指定されていれば優先引き当て、なければローカル block_index で解決
                                let bone_transforms = if let Some(name_map) = skeleton_bone_name_map {
                                    resolve_bone_world_transforms_by_name(inst, nif, name_map, Some(bone_world_map))
                                } else {
                                    resolve_bone_world_transforms(inst, bone_world_map)
                                };
                                let bone_refs = if bone_transforms.is_empty() {
                                    None
                                } else {
                                    Some(bone_transforms.as_slice())
                                };
                                if let Some((pos, nrm)) = apply_skinning_cpu_with_bones(data, inst, nif, bone_refs) {
                                    GpuMesh::from_tri_shape_skinned(device, data, &pos, &nrm)
                                } else {
                                    GpuMesh::from_tri_shape(device, data)
                                }
                            } else {
                                GpuMesh::from_tri_shape(device, data)
                            }
                        } else {
                            GpuMesh::from_tri_shape(device, data)
                        }
                    } else {
                        GpuMesh::from_tri_shape(device, data)
                    };

                    if let Some(gpu_mesh) = gpu_mesh {
                        let tint = anim_collector.as_ref().and_then(|c| c.tint_color);
                        let render_mesh = create_render_mesh(
                            device,
                            context,
                            &name,
                            gpu_mesh,
                            &world_transform,
                            &shape.geom.av.properties,
                            parent_alpha,
                            parent_material,
                            nif,
                            vfs,
                            queue,
                            texture_cache,
                            default_texture,
                            default_normal_texture,
                            default_glow_texture,
                            tint,
                        );
                        out_meshes.push(render_mesh);

                        // アクターパーツ走査時: 生成されたメッシュとスキン/剛体情報を 1:1 でダイレクト登録
                        if let Some(ref mut collector) = anim_collector {
                            let mesh_index = out_meshes.len() - 1;
                            if shape.geom.skin_instance >= 0 {
                                collector.anim_skins.push(AnimatedSkinMesh {
                                    mesh_index,
                                    part_index: collector.part_index,
                                    geo_data_block: shape.geom.data,
                                    skin_instance_block: shape.geom.skin_instance,
                                });
                            } else if let Some(bone_name) = collector.attach_bone {
                                let local_transform = compute_rigid_part_local_transform(
                                    block_index as usize,
                                    bone_name,
                                    &collector.shape_transforms,
                                    &shape.geom.av,
                                );
                                collector.anim_rigids.push(AnimatedRigidMesh {
                                    mesh_index,
                                    bone_name: bone_name.to_string(),
                                    local_transform,
                                });
                            }
                        }
                    }
                }
            }
        }

        NifBlock::NiTriStrips(strips) => {
            if is_node_hidden(&strips.geom.av, nif) {
                return;
            }
            let local_transform = to_core_transform(&strips.geom.av);
            let world_transform = parent_world.compose(&local_transform);
            let name = nif.get_string(strips.geom.av.net.name_index).unwrap_or("").to_string();

            if strips.geom.data >= 0 && (strips.geom.data as usize) < nif.blocks.len() {
                if let NifBlock::NiTriStripsData(ref data) = nif.blocks[strips.geom.data as usize] {
                    if let Some(gpu_mesh) = GpuMesh::from_tri_strips(device, data) {
                        let tint = anim_collector.as_ref().and_then(|c| c.tint_color);
                        let render_mesh = create_render_mesh(
                            device,
                            context,
                            &name,
                            gpu_mesh,
                            &world_transform,
                            &strips.geom.av.properties,
                            parent_alpha,
                            parent_material,
                            nif,
                            vfs,
                            queue,
                            texture_cache,
                            default_texture,
                            default_normal_texture,
                            default_glow_texture,
                            tint,
                        );
                        out_meshes.push(render_mesh);

                        // 剛体パーツ (目・歯・舌・髪) のダイレクト登録
                        if let Some(ref mut collector) = anim_collector {
                            let mesh_index = out_meshes.len() - 1;
                            if let Some(bone_name) = collector.attach_bone {
                                let local_transform = compute_rigid_part_local_transform(
                                    block_index as usize,
                                    bone_name,
                                    &collector.shape_transforms,
                                    &strips.geom.av,
                                );
                                collector.anim_rigids.push(AnimatedRigidMesh {
                                    mesh_index,
                                    bone_name: bone_name.to_string(),
                                    local_transform,
                                });
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// ノードが非表示（App Culled / エディタマーカー）であるかを判定する。
///
/// 判定基準:
/// 1. `NiAVObject::flags` のビット 0 (`flags & 0x0001 != 0`): `Hidden` / `App Culled`
///    参照元: `references/openmw/components/nif/node.hpp:77`, `references/nifskope/src/gl/glnode.cpp:456`
/// 2. ノード名が `"EditorMarker"` や `"Marker"` で始まる (Creation Kit マーカー)
///    参照元: `references/nifskope/src/gl/glmesh.cpp:760`
pub fn is_node_hidden(av: &fo3_nif::NiAVObject, nif: &NifFile) -> bool {
    // 1. App Culled (Hidden) フラグ
    if av.flags & 0x0001 != 0 {
        return true;
    }

    // 2. エディタマーカーノードの判定
    if let Some(name) = nif.get_string(av.net.name_index) {
        if name.starts_with("EditorMarker")
            || name.starts_with("Marker")
            || name.starts_with("Sound Marker")
            || name.starts_with("Light Marker")
            || name.starts_with("NorthMarker")
            || name.starts_with("HavokEdit")
        {
            return true;
        }
    }

    false
}

/// BSDismemberSkinInstance を持つメッシュが、初期状態（非切断・intact 状態）において非表示であるかを判定する。
///
/// Fallout 3 / Gamebryo 2.6 では、手足や首の切断面（ゴア・肉片キャップ）は
/// `BSDismemberSkinInstance` 内のパーティションフラグ `part_flag` で管理される。
/// `part_flag & 0x0001 != 0` (`PF_EDITOR_VISIBLE`) を持つパーティションのみが通常時に表示され、
/// すべてのパーティションで `editor_visible == false` であるメッシュノード（例: `bodycaps`, `limbcaps`, `meatneck01`）は
/// 肢体切断イベント発生まで非表示（Hidden）として扱われる。
///
/// 参照元:
/// - `references/nifskope/build/nif.xml:L2530-2541` (`BSPartFlag::PF_EDITOR_VISIBLE`, `BodyPartList`)
/// - `references/bevyout/src/vsa/assets/blender_script.py:L1760-1779` (`prune_hidden_actor_partitions`)
/// - `knowledge/actor_and_skin_mesh.md` (セクション 4.5)
pub fn is_dismember_hidden(skin_instance_block: i32, nif: &NifFile) -> bool {
    if skin_instance_block < 0 || (skin_instance_block as usize) >= nif.blocks.len() {
        return false;
    }
    if let NifBlock::BSDismemberSkinInstance(ref bdsi) = nif.blocks[skin_instance_block as usize] {
        if bdsi.partitions.is_empty() {
            return false;
        }
        let any_visible = bdsi.partitions.iter().any(|p| (p.part_flag & 0x0001) != 0);
        return !any_visible;
    }
    false
}
