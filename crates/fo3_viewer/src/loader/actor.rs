//! セル内アクター (NPC_ / ACHR) の装備展開、髪型、FaceGen、アニメーション合成モジュール。
//!
//! 参照元: references/openmw/components/esm4/loadnpc.cpp:60, `knowledge/actor_and_skin_mesh.md`

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;

use fo3_esm::{EsmMasterContext, FormId, RefrRecord};
use fo3_gamebryo_core::NiTransform;
use fo3_nif::NifFile;
use fo3_render::{GpuTexture, NifCache, RenderContext, RenderScene};
use fo3_script::conditions::ConditionContext;
use fo3_vfs::VfsManager;

use crate::types::{
    get_actor_part_paths, is_ghoul_race, resolve_candidate_items,
};

/// セル内で検出された NPC の出現定義。
pub struct CellNpcSpawn {
    pub form_id: u32,
    pub base_form_id: u32,
    pub name: String,
    pub transform: NiTransform,
    pub is_female: bool,
    pub race: FormId,
    pub outfit_model: Option<String>,
    pub head_gear_model: Option<String>,
    pub hand_gear_model: Option<String>,
    pub weapon_model: Option<String>,
    pub hair_model: Option<String>,
    pub hair_color: Option<[u8; 3]>,
    pub has_hat: bool,
    pub hide_hair: bool,
    pub facegen_texture_symmetric: Option<Vec<f32>>,
    pub facegen_geometry_symmetric: Option<Vec<f32>>,
    pub facegen_geometry_asymmetric: Option<Vec<f32>>,
}

/// 単一の REFR が NPC_ レコードの場合、装備・髪型・FaceGen を解決して `CellNpcSpawn` を生成する。
pub fn try_resolve_actor(
    refr: &RefrRecord,
    master_context: &EsmMasterContext,
    world_transform: NiTransform,
) -> Option<CellNpcSpawn> {
    let npc = master_context.npc_map.get(&refr.base_object)?;

    let outfit_map = &master_context.outfit_map;
    let lvli_map = &master_context.lvli_map;
    let armor_map = &master_context.armor_map;
    let model_map = &master_context.model_map;
    let hair_map = &master_context.hair_map;

    let mut raw_items = Vec::new();
    if let Some(doft_id) = npc.default_outfit {
        if let Some(otft) = outfit_map.get(&doft_id) {
            raw_items.extend(otft.inventory.iter().copied());
        } else {
            raw_items.push(doft_id);
        }
    }
    if let Some(wnam) = npc.default_armor {
        raw_items.push(wnam);
    }
    for inv in &npc.inventory {
        raw_items.push(inv.item);
    }
    let candidate_items = resolve_candidate_items(&raw_items, lvli_map);

    let mut outfit_model = None;
    let mut head_gear_model = None;
    let mut hand_gear_model = None;
    let mut weapon_model = None;
    let mut has_hat = false;
    let mut hide_hair = false;

    for item_id in candidate_items {
        if let Some(armo) = armor_map.get(&item_id) {
            let model_path = armo.model_for_gender(npc.is_female);
            if armo.is_head() && head_gear_model.is_none() {
                if let Some(m) = model_path {
                    head_gear_model = Some(m.to_string());
                    if armo.shows_hat() {
                        has_hat = true;
                    }
                    if armo.hides_hair() {
                        hide_hair = true;
                    }
                }
            } else if armo.is_upper_body() && outfit_model.is_none() {
                if let Some(m) = model_path {
                    outfit_model = Some(m.to_string());
                }
            } else if armo.is_hands() && hand_gear_model.is_none() {
                if let Some(m) = model_path {
                    hand_gear_model = Some(m.to_string());
                }
            }
        } else if weapon_model.is_none() {
            if let Some(base_info) = model_map.get(&item_id) {
                let m_lower = base_info.model.to_ascii_lowercase();
                if !base_info.model.is_empty()
                    && (base_info.record_type == fo3_esm::types::REC_WEAP
                        || m_lower.contains("weapons")
                        || m_lower.contains("weapon"))
                {
                    weapon_model = Some(base_info.model.clone());
                }
            }
        }
    }

    let outfit_model = outfit_model.or_else(|| {
        if npc.is_female {
            Some("Armor\\WastelandClothing01\\OutfitF.NIF".to_string())
        } else {
            Some("Armor\\WastelandClothing01\\OutfitM.NIF".to_string())
        }
    });

    let hair_model = if hide_hair {
        None
    } else {
        npc.hair
            .and_then(|hair_id| hair_map.get(&hair_id))
            .and_then(|hair| {
                if !hair.model.is_empty() {
                    Some(hair.model.clone())
                } else {
                    None
                }
            })
            .or_else(|| {
                if npc.is_female {
                    Some("Characters\\Hair\\HairBun.NIF".to_string())
                } else {
                    Some("Characters\\Hair\\HairMessy03.NIF".to_string())
                }
            })
    };

    let hair_color = npc.hair_color.or(Some([65, 45, 30]));
    let name = npc.full_name.clone().unwrap_or_else(|| npc.edid.clone());

    Some(CellNpcSpawn {
        form_id: refr.form_id.0,
        base_form_id: refr.base_object.0,
        name,
        transform: world_transform,
        is_female: npc.is_female,
        race: npc.race,
        outfit_model,
        head_gear_model,
        hand_gear_model,
        weapon_model,
        hair_model,
        hair_color,
        has_hat,
        hide_hair,
        facegen_texture_symmetric: npc.facegen_texture_symmetric.clone(),
        facegen_geometry_symmetric: npc.facegen_geometry_symmetric.clone(),
        facegen_geometry_asymmetric: npc.facegen_geometry_asymmetric.clone(),
    })
}

/// 抽出されたアクター定義から GPU シーンへのアクターインスタンス追加を行う。
pub fn build_scene_actors(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    context: &RenderContext,
    scene: &mut RenderScene,
    cell_npcs: &[CellNpcSpawn],
    vfs: &mut VfsManager,
    master_context: &EsmMasterContext,
    nif_cache: &mut NifCache,
) {
    if cell_npcs.is_empty() {
        return;
    }

    let mut actor_texture_cache = HashMap::new();

    let mut load_nif = |p: &str| vfs.read(p).ok().and_then(|b| NifFile::read(&mut Cursor::new(b)).ok()).map(Arc::new);
    let skel_male = load_nif("meshes\\characters\\_male\\skeleton.nif");
    let skel_female = load_nif("meshes\\characters\\_female\\skeleton.nif");

    let mut load_egm = |p: &str| vfs.read(p).ok().and_then(|b| fo3_render::parse_geometry_morph(&b).ok());
    let egm_male = load_egm("meshes\\characters\\head\\headmale.egm");
    let egm_female = load_egm("meshes\\characters\\head\\headfemale.egm");

    let mut default_idle_arcs: HashMap<
        u32,
        (Option<Arc<NifFile>>, Option<Arc<fo3_render::AnimationClip>>),
    > = HashMap::new();

    for npc in cell_npcs {
        let skeleton = if npc.is_female {
            skel_female.clone().or_else(|| skel_male.clone())
        } else {
            skel_male.clone().or_else(|| skel_female.clone())
        };

        let Some(skel) = skeleton else {
            eprintln!(
                "警告: スケルトン NIF が読み込めないためアクター \"{}\" をスキップします",
                npc.name
            );
            continue;
        };

        let body_path = if let Some(ref model) = npc.outfit_model {
            model.clone()
        } else if npc.is_female {
            "meshes\\characters\\_female\\upperbody.nif".to_string()
        } else {
            "meshes\\characters\\_male\\upperbody.nif".to_string()
        };

        let part_paths = get_actor_part_paths(
            npc.is_female,
            npc.race,
            &body_path,
            npc.head_gear_model.as_deref(),
            npc.hand_gear_model.as_deref(),
            npc.weapon_model.as_deref(),
            npc.hair_model.as_deref(),
            npc.hide_hair,
        );
        let mut parts = Vec::new();
        for path in &part_paths {
            let nif = nif_cache.get_or_load(path, vfs).ok();
            if let Some(p) = nif {
                parts.push(p);
            }
        }

        let head_diffuse_override = if let Some(ref fgts) = npc.facegen_texture_symmetric {
            let egt_path = if npc.is_female {
                "meshes\\characters\\head\\headfemale.egt"
            } else {
                "meshes\\characters\\head\\headmale.egt"
            };
            let default_head_dds = if is_ghoul_race(npc.race) {
                "textures\\characters\\head\\headghoul.dds"
            } else if npc.is_female {
                "textures\\characters\\head\\headhumanfemale.dds"
            } else {
                "textures\\characters\\head\\headhuman.dds"
            };
            let dds_path = if vfs.exists(default_head_dds) {
                default_head_dds
            } else {
                "textures\\characters\\head\\headhuman.dds"
            };

            let synthesized_tex = if let (Ok(egt_bytes), Ok(dds_bytes)) =
                (vfs.read(egt_path), vfs.read(dds_path))
            {
                if let (Ok(morph), Ok((w, h, base_rgba))) = (
                    fo3_render::parse_texture_morph(&egt_bytes),
                    fo3_render::decode_dds_to_rgba8(&dds_bytes),
                ) {
                    let final_rgba =
                        fo3_render::synthesize_head_diffuse(base_rgba, w, h, &morph, fgts);
                    GpuTexture::from_rgba8(
                        device,
                        queue,
                        w,
                        h,
                        &final_rgba,
                        Some(&format!("FaceGen_{:08X}", npc.form_id)),
                    )
                    .ok()
                } else {
                    None
                }
            } else {
                None
            };

            synthesized_tex
        } else {
            None
        };

        let override_ref = head_diffuse_override.as_ref();
        let egm_ref = if npc.is_female {
            egm_female.as_ref().or(egm_male.as_ref())
        } else {
            egm_male.as_ref().or(egm_female.as_ref())
        };
        let fg_sym_ref = npc.facegen_geometry_symmetric.as_deref();
        let fg_asym_ref = npc.facegen_geometry_asymmetric.as_deref();

        let (npc_kf, npc_clip) = {
            let base_id = npc.base_form_id;
            if let Some(cached) = default_idle_arcs.get(&base_id) {
                cached.clone()
            } else {
                let cond_ctx = ConditionContext::default();
                let dummy_vm = fo3_script::ScriptVm::default();
                let resolved = crate::action::resolve_pack_for_actor(
                    master_context,
                    FormId(base_id),
                    &cond_ctx,
                    &dummy_vm,
                )
                .and_then(|pkg| {
                    let mut kf_paths =
                        crate::action::resolve_idle_kf_from_pack(master_context, pkg);
                    if kf_paths.is_empty() {
                        kf_paths
                            .push("meshes\\characters\\_male\\idleanims\\mtidle.kf".to_string());
                    }
                    for path in &kf_paths {
                        if let Ok(bytes) = vfs.read(path) {
                            let mut cursor = Cursor::new(bytes);
                            if let Ok(kf) = NifFile::read(&mut cursor) {
                                if let Some(clip) = fo3_render::AnimationClip::from_kf(&kf) {
                                    return Some((Arc::new(kf), Arc::new(clip)));
                                }
                            }
                        }
                    }
                    None
                });
                let result = match resolved {
                    Some(pair) => (Some(pair.0), Some(pair.1)),
                    None => {
                        if let Ok(bytes) =
                            vfs.read("meshes\\characters\\_male\\idleanims\\mtidle.kf")
                        {
                            let mut cursor = std::io::Cursor::new(bytes);
                            if let Ok(kf) = fo3_nif::NifFile::read(&mut cursor) {
                                if let Some(clip) =
                                    fo3_render::animation::AnimationClip::from_kf(&kf)
                                {
                                    (
                                        Some(std::sync::Arc::new(kf)),
                                        Some(std::sync::Arc::new(clip)),
                                    )
                                } else {
                                    (None, None)
                                }
                            } else {
                                (None, None)
                            }
                        } else {
                            (None, None)
                        }
                    }
                };
                default_idle_arcs.insert(base_id, result.clone());
                result
            }
        };

        scene.add_actor(
            device,
            queue,
            context,
            vfs,
            npc.form_id,
            npc.base_form_id,
            &npc.name,
            &npc.transform,
            skel,
            parts,
            npc_kf,
            npc_clip,
            &mut actor_texture_cache,
            npc.hair_color,
            npc.has_hat,
            override_ref,
            egm_ref,
            fg_sym_ref,
            fg_asym_ref,
        );
    }
}
