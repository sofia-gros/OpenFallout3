//! PlayGroup / PlayAnim アニメーション再生要求処理モジュール。
//!
//! 参照元: Gamebryo 2.6 アニメーションコントローラー, GECK PlayGroup スクリプトコマンド

use crate::app::ViewerState;

/// `PlayGroup` や `PlayAnim` によるアニメーション再生要求を処理する。
/// Actorだけでなく、ActivatorやStatic(例: gene_projector.nif)にも適用する。
pub fn process_playgroup_requests(app: &mut ViewerState) {
    let mut requests = Vec::new();
    std::mem::swap(&mut requests, &mut app.vm.playgroup_queue);

    for (target_fid, anim_name) in requests {
        if let Some(actor) = app
            .scene
            .actors
            .iter_mut()
            .find(|a| a.form_id == target_fid.0)
        {
            let mut kf_paths = vec![
                format!("meshes/characters/_male/{}", anim_name),
            ];

            // アクターのモデル名が .nif で終わる場合、同階層の KF パスも探索候補に追加
            if actor.name.to_lowercase().ends_with(".nif") {
                let path = std::path::Path::new(&actor.name);
                if let Some(parent) = path.parent() {
                    let parent_str = parent.to_string_lossy().replace('\\', "/");
                    kf_paths.push(format!("{}/{}", parent_str, anim_name));
                    kf_paths.push(format!("{}/{}", parent_str, anim_name.to_lowercase()));
                }
            }

            let mut kf_loaded = false;
            for kf_path in kf_paths {
                if let Ok(buf) = app.vfs.read(&kf_path) {
                    if let Ok(kf) = fo3_nif::NifFile::read(&mut std::io::Cursor::new(&buf)) {
                        let kf_arc = std::sync::Arc::new(kf);
                        if let Some(clip) = fo3_render::animation::AnimationClip::from_kf(&kf_arc) {
                            actor.set_animation(kf_arc, std::sync::Arc::new(clip));
                            println!(
                                "[Action] Actor {:08X} の PlayGroup: {} (KF: {}) を再生します",
                                target_fid.0, anim_name, kf_path
                            );
                            kf_loaded = true;
                            break;
                        }
                    }
                }
            }
            if !kf_loaded {
                println!(
                    "[Action] [stub] 3Dオブジェクト {:08X} の PlayGroup: {} の KF が見つかりません (モデル: {})",
                    target_fid.0, anim_name, actor.name
                );
            }
        } else {
            println!(
                "[Action] [stub] 3Dオブジェクト {:08X} が存在しないため PlayGroup: {} は失敗しました",
                target_fid.0, anim_name
            );
        }
    }
}
