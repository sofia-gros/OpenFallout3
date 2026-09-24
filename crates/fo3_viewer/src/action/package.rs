//! AI パッケージ評価・スクリプトパッケージ適用モジュール。
//!
//! 参照元: Gamebryo 2.6 AI パッケージシステム,
//!         `references/bevyout/src/vsa/openmw_esm4/actor_support.rs:L659-745` (decode_package_idle_collection),
//!         `references/openmw/components/esm4/loadnpc.cpp` (PKID)

use fo3_esm::{EsmMasterContext, FormId, PackRecord};
use fo3_script::{evaluate_conditions, ConditionContext};

use crate::app::ViewerState;
use crate::interact::InteractableKind;

/// PACK レコードの Idle Collection (IDLA->IDLE->MODL) からロード可能な KF ファイルパス一覧を解決する。
///
/// チェーン: PACK `IDLA` (IDLE FormID リスト) -> IDLE レコード -> `MODL` (KF 相対パス C-String)。
/// 戻り値は VFS 読み取り用に小文字化 + `meshes\` プレフィックス正規化済み。
pub(crate) fn resolve_idle_kf_from_pack(ctx: &EsmMasterContext, pkg: &PackRecord) -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(col) = &pkg.idle_collection {
        for fid in &col.animation_form_ids {
            if let Some(idle) = ctx.idle_map.get(fid) {
                if let Some(model) = &idle.model_path {
                    let mut lower = model.to_ascii_lowercase().replace('/', "\\");
                    if !lower.starts_with("meshes\\") {
                        lower = format!("meshes\\{}", lower);
                    }
                    if !lower.ends_with(".kf") {
                        lower.push_str(".kf");
                    }
                    paths.push(lower);
                }
            }
        }
    }
    paths
}

/// NPC レコードの AI パッケージリスト (PKID) から、CTDA 条件を満たす最初の PACK を選択する。
pub(crate) fn resolve_pack_for_actor<'a>(
    ctx: &'a EsmMasterContext,
    base_npc: FormId,
    cond_ctx: &ConditionContext,
    vm: &fo3_script::ScriptVm,
) -> Option<&'a PackRecord> {
    let npc = ctx.npc_map.get(&base_npc)?;
    for pkid in &npc.ai_packages {
        if let Some(pkg) = ctx.pack_map.get(pkid) {
            if evaluate_conditions(&pkg.conditions, cond_ctx, vm) {
                return Some(pkg);
            }
        }
    }
    None
}

/// PACK の Idle Collection からロード済み (nif, clip) を組み立てる共通適用処理。
/// 複数 KF 候補を先頭から順に試し、初めて正常に NIF+クリップ化できた候補を返す。
fn load_first_clip(
    app: &mut ViewerState,
    kf_paths: &[String],
) -> Option<(
    String,
    std::sync::Arc<fo3_nif::NifFile>,
    std::sync::Arc<fo3_render::AnimationClip>,
)> {
    for path in kf_paths {
        if let Ok(bytes) = app.vfs.read(path) {
            let mut cursor = std::io::Cursor::new(bytes);
            if let Ok(kf) = fo3_nif::NifFile::read(&mut cursor) {
                if let Some(clip) = fo3_render::AnimationClip::from_kf(&kf) {
                    return Some((
                        path.clone(),
                        std::sync::Arc::new(kf),
                        std::sync::Arc::new(clip),
                    ));
                }
            }
        }
    }
    None
}

/// EvaluatePackage 要求の消化 & AddScriptPackage 要求の消化 (パッケージアニメーション適用)。
pub fn process_package_requests(app: &mut ViewerState) {
    let mut add_pkgs = Vec::new();
    std::mem::swap(&mut add_pkgs, &mut app.vm.script_package_requests);

    let mut evp_requests = Vec::new();
    std::mem::swap(&mut evp_requests, &mut app.vm.evaluate_package_requests);

    // 1. AddScriptPackage 要求の消化
    for (subject_opt, pkg_name) in add_pkgs {
        let target_fid = subject_opt.unwrap_or(FormId(0x00000014));

        let mut kf_paths = Vec::new();

        // 1a. PACK レコードを EDID 一致で解決し、Idle Collection (IDLA->IDLE->MODL) から KF を取得
        if let Some(pkg) = app.master_context.pack_map.values().find(|p| {
            p.editor_id
                .as_deref()
                .map(|e| e.eq_ignore_ascii_case(&pkg_name))
                .unwrap_or(false)
        }) {
            if let Some(actor_state) = app.ai.actors.get_mut(&target_fid) {
                if !actor_state.script_packages.contains(&pkg.form_id) {
                    actor_state.script_packages.insert(0, pkg.form_id); // 高優先度
                }
            }

            kf_paths.extend(resolve_idle_kf_from_pack(&app.master_context, pkg));
        }

        if let Some((path, kf, clip)) = load_first_clip(app, &kf_paths) {
            if target_fid == FormId(0x00000014) {
                if let Some(ref player) = app.controller.player_actor {
                    if player.third_person_actor_idx < app.scene.actors.len() {
                        app.scene.actors[player.third_person_actor_idx].set_animation(kf, clip);
                        println!("[Package] プレイヤーにアニメーション \"{}\" を適用", path);
                    }
                }
            } else if let Some(actor) = app
                .scene
                .actors
                .iter_mut()
                .find(|a| a.form_id == target_fid.0)
            {
                actor.set_animation(kf, clip);
                println!(
                    "[Package] アクター 0x{:08X} にアニメーション \"{}\" を適用",
                    target_fid.0, path
                );
            }
        } else {
            println!(
                "[Package] アニメーション KF が見つかりません: package=\"{}\"",
                pkg_name
            );
        }
    }

    // 2. EvaluatePackage (evp) 要求の消化
    for subject_opt in evp_requests {
        let target_fid = subject_opt.unwrap_or(FormId(0x00000014));

        // REFR FormID -> NPC ベース FormID への解決
        let base_npc = app.interactables.iter().find_map(|it| match it.kind {
            InteractableKind::Actor {
                form_id,
                base_form_id,
                ..
            } if form_id == target_fid.0 => Some(fo3_esm::FormId(base_form_id)),
            _ => None,
        });
        let Some(base_npc) = base_npc else { continue };

        let (pkg_edid, kf_paths) = {
            let mut cond_ctx = ConditionContext::default();
            cond_ctx.speaker = Some(base_npc);
            cond_ctx.target = Some(target_fid);
            cond_ctx.quest_stages = app.vm.quest_stages.clone();
            cond_ctx.quest_stage_history = app.vm.quest_manager.get_stage_history_u32();
            match resolve_pack_for_actor(&app.master_context, base_npc, &cond_ctx, &app.vm) {
                Some(p) => (
                    p.editor_id.clone().unwrap_or_default(),
                    resolve_idle_kf_from_pack(&app.master_context, p),
                ),
                None => (String::new(), Vec::new()),
            }
        };

        if let Some((path, kf, clip)) = load_first_clip(app, &kf_paths) {
            if let Some(actor) = app
                .scene
                .actors
                .iter_mut()
                .find(|a| a.form_id == target_fid.0)
            {
                actor.set_animation(kf, clip);
                println!("[AI/EVP] アクター 0x{:08X} (\"{}\") に PACK \"{}\" 由来のアニメーション \"{}\" を適用",
                    target_fid.0, actor.name, pkg_edid, path);
            }
        }
    }
}
