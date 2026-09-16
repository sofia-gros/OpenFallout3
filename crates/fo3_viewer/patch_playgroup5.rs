use std::fs;

fn main() {
    let mut content = fs::read_to_string("crates/fo3_viewer/src/action.rs").unwrap();
    
    let start = content.find("pub fn process_playgroup_requests").unwrap();
    
    let new_func = r#"pub fn process_playgroup_requests(app: &mut ViewerState) {
    let mut requests = Vec::new();
    std::mem::swap(&mut requests, &mut app.vm.playgroup_queue);

    for (target_fid, anim_name) in requests {
        if let Some(actor) = app.scene.actors.iter_mut().find(|a| a.form_id == target_fid.0) {
            // Actorの場合はそのままkfを適用
            let kf_path = format!("meshes/characters/_male/{}", anim_name);
            if let Ok(buf) = app.vfs.read(&kf_path) {
                if let Ok(kf) = fo3_nif::NifFile::read(&mut std::io::Cursor::new(&buf)) {
                    let kf_arc = std::sync::Arc::new(kf);
                    if let Some(clip) = fo3_render::animation::AnimationClip::from_kf(&kf_arc) {
                        actor.set_animation(kf_arc, std::sync::Arc::new(clip));
                        println!("[Action] Actor {:08X} に PlayGroup: {} (KF: {}) を適用しました", target_fid.0, anim_name, kf_path);
                    }
                }
            }
        } else {
            // 非Actorのアニメーション (gene_projector等) は stub
            println!("[Action] [stub] 3Dオブジェクト {:08X} のアニメーション再生 PlayGroup: {}", target_fid.0, anim_name);
        }
    }
}
"#;

    content.replace_range(start.., new_func);
    fs::write("crates/fo3_viewer/src/action.rs", content).unwrap();
}
