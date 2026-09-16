use std::fs;

fn main() {
    let mut content = fs::read_to_string("crates/fo3_viewer/src/action.rs").unwrap();
    
    let old = r#"    for (target_fid, anim_name) in requests {
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
    }"#;
    
    let new = r#"    for (target_fid, anim_name) in requests {
        if let Some(actor) = app.scene.actors.iter_mut().find(|a| a.form_id == target_fid) {
            // Actorの場合はそのままkfを適用
            let kf_path = format!("meshes/characters/_male/{}", anim_name);
            if let Ok(buf) = app.vfs.read(&kf_path) {
                if let Ok(kf) = fo3_nif::NifFile::read(&mut std::io::Cursor::new(&buf)) {
                    actor.instance.set_animation(&kf);
                    println!("[Action] Actor {:08X} に PlayGroup: {} (KF: {}) を適用しました", target_fid.0, anim_name, kf_path);
                }
            }
        } else {
            // 非Actorのアニメーション (gene_projector等) は stub
            println!("[Action] [stub] 3Dオブジェクト {:08X} のアニメーション再生 PlayGroup: {}", target_fid.0, anim_name);
        }
    }"#;
    
    if content.contains("app.scene.actors.get_mut(&target_fid)") {
        content = content.replace(old, new);
        fs::write("crates/fo3_viewer/src/action.rs", content).unwrap();
    }
}
