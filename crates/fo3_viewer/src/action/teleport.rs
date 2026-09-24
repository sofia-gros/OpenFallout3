//! スクリプトからのテレポート移動要求 (MoveTo) 処理モジュール。
//!
//! 参照元: Gamebryo 2.6 セルグラフ移動, `references/openmw/`

use crate::app::ViewerState;

/// スクリプトからのテレポート移動要求 (MoveTo) を消化し、アクターやプレイヤーを移動させる。
pub fn process_teleport_requests(app: &mut ViewerState) {
    while !app.vm.teleport_requests.is_empty() {
        let (subject, marker) = app.vm.teleport_requests.remove(0);
        let marker_key = marker.to_ascii_lowercase();
        let marker_data = app.markers.get(&marker_key).copied();

        if let Some((pos, rot)) = marker_data {
            if let Some(fid) = subject {
                if let Some(actor) = app.scene.actors.iter_mut().find(|a| a.form_id == fid.0) {
                    actor.world_transform.translation = pos;
                    actor.world_transform.rotation =
                        glam::Mat3::from_euler(glam::EulerRot::XYZ, rot.x, rot.y, rot.z);
                    println!(
                        "[MoveTo] アクター 0x{:08X} をマーカー \"{}\" (pos={:?}) へ配置完了",
                        fid.0, marker, pos
                    );
                }
            } else {
                app.controller.character_controller.position = pos;
                app.controller.initial_spawn_point = pos;
                app.controller.player_camera.current_eye = pos + glam::Vec3::new(0.0, 0.0, 60.0);
                app.controller.player_camera.yaw = rot.z;
                app.controller.camera.yaw = rot.z;
                if let Some(ref mut player) = app.controller.player_actor {
                    player.position = pos;
                }
                println!(
                    "[MoveTo] プレイヤーをマーカー \"{}\" (pos={:?}) へテレポート完了",
                    marker, pos
                );
            }
        } else {
            println!(
                "[MoveTo] 未知のマーカー \"{}\" への配置要求 (スキップ)",
                marker
            );
        }
    }
}
