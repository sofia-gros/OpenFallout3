use std::fs;

fn main() {
    let mut content = fs::read_to_string("crates/fo3_viewer/src/action.rs").unwrap();
    
    let new_func = r#"
/// `PlayGroup` や `PlayAnim` によるアニメーション再生要求を処理する。
/// Actorだけでなく、ActivatorやStatic(例: gene_projector.nif)にも適用する。
pub fn process_playgroup_requests(app: &mut ViewerState) {
    let mut requests = Vec::new();
    std::mem::swap(&mut requests, &mut app.vm.playgroup_queue);

    for (target_fid, anim_name) in requests {
        let nif_name_opt = {
            if let Some(actor) = app.scene.actors.get_mut(&target_fid) {
                // Actorの場合はそのままkfを適用
                let kf_path = format!("meshes/characters/_male/{}", anim_name);
                if let Some(buf) = app.vfs.read_file(&kf_path) {
                    if let Ok(kf) = fo3_nif::parse_nif(&buf) {
                        actor.instance.set_animation(&kf);
                        println!("[Action] Actor {:08X} に PlayGroup: {} (KF: {}) を適用しました", target_fid.0, anim_name, kf_path);
                    }
                }
                None
            } else if let Some(instance) = app.scene.instances.iter_mut().find(|i| i.form_id == target_fid) {
                Some((instance.nif_path.clone(), instance.id))
            } else {
                None
            }
        };

        if let Some((nif_path, instance_id)) = nif_name_opt {
            // NIF名に基づいたKFのパスを解決 (例: architecture/megaton/gene_projector.nif -> architecture/megaton/gene_projector.kf)
            // anim_name が KF ファイル名やグループ名として機能する
            let base_kf = nif_path.to_ascii_lowercase().replace(".nif", &format!("_{}.kf", anim_name.to_ascii_lowercase()));
            let alt_kf = nif_path.to_ascii_lowercase().replace(".nif", ".kf");
            
            let mut buf_opt = app.vfs.read_file(&base_kf);
            let mut final_kf = base_kf.clone();
            if buf_opt.is_none() {
                buf_opt = app.vfs.read_file(&alt_kf);
                final_kf = alt_kf.clone();
            }

            if let Some(buf) = buf_opt {
                if let Ok(kf) = fo3_nif::parse_nif(&buf) {
                    if let Some(instance) = app.scene.instances.iter_mut().find(|i| i.id == instance_id) {
                        if let Some(ref mut model) = instance.model {
                            // model は fo3_render::scene::ModelInstance
                            if let Some(ref mut root) = model.root_node {
                                // アニメーションをModelInstanceに追加するロジックが必要だが、fo3_render は Actor 用しか持っていない？
                                // fo3_render の ModelInstance は KF を受け取る機構があるか？
                                // TODO: fo3_renderにModelInstanceへのアニメーション適用を実装する
                                println!("[Action] [stub] 3Dオブジェクト {:08X} のアニメーション再生: {}", target_fid.0, final_kf);
                            }
                        }
                    }
                }
            } else {
                println!("[Action] PlayGroup: KFファイルが見つかりません: {} (for {:08X})", base_kf, target_fid.0);
            }
        }
    }
}
"#;
    content.push_str(new_func);
    fs::write("crates/fo3_viewer/src/action.rs", content).unwrap();
    
    let mut app_content = fs::read_to_string("crates/fo3_viewer/src/app.rs").unwrap();
    let old = "        crate::action::process_teleport_requests(self);\n\n        // 0.2 スクリプトの AI パッケージ・アニメーション要求 (AddScriptPackage / evp) の処理\n        crate::action::process_package_requests(self);";
    let new = "        crate::action::process_teleport_requests(self);\n\n        // 0.2 スクリプトの AI パッケージ・アニメーション要求 (AddScriptPackage / evp) の処理\n        crate::action::process_package_requests(self);\n        crate::action::process_playgroup_requests(self);";
    
    if app_content.contains("process_package_requests") {
        app_content = app_content.replace(old, new);
        // also replace in CRLF format if needed
        let old2 = "        crate::action::process_teleport_requests(self);\r\n\r\n        // 0.2 スクリプトの AI パッケージ・アニメーション要求 (AddScriptPackage / evp) の処理\r\n        crate::action::process_package_requests(self);";
        let new2 = "        crate::action::process_teleport_requests(self);\r\n\r\n        // 0.2 スクリプトの AI パッケージ・アニメーション要求 (AddScriptPackage / evp) の処理\r\n        crate::action::process_package_requests(self);\r\n        crate::action::process_playgroup_requests(self);";
        app_content = app_content.replace(old2, new2);
        fs::write("crates/fo3_viewer/src/app.rs", app_content).unwrap();
    }
}
