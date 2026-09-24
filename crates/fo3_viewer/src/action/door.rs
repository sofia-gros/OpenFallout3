//! ワールド・セル遷移ドア (XTEL) によるシーン切り替えモジュール。
//!
//! 参照元: Gamebryo 2.6 セル遷移 & `references/openmw/components/esm4/loadrefr.cpp:103-127`

use fo3_esm::TeleportDoor;

use crate::anim::AnimState;
use crate::app::ViewerState;
use crate::loader::load_scene;
use crate::types::{update_window_title, ViewerTarget};

/// テレポートドアを通過し、接続先のセルまたはワールドスペースをロードしてシーンを置換する。
pub fn handle_door_transition(app: &mut ViewerState, tp: &TeleportDoor) {
    println!(
        "  - テレポートドア起動: 遷移先ドア FormID 0x{:08X}, 出現座標: {:?}, 出現回転: {:?}",
        tp.dest_door.0, tp.dest_pos, tp.dest_rot
    );

    // Fallout3.esm から dest_door の親セルおよび所属ワールドを逆引き検索
    let esm_path = std::path::Path::new(&app.data_dir).join("Fallout3.esm");
    let dest_cell_info = if let Ok(mut reader) = fo3_esm::EsmReader::open(&esm_path) {
        match reader.find_cell_containing_refr(tp.dest_door) {
            Ok(Some((cell, _, _, parent_world))) => Some((cell, parent_world)),
            _ => None,
        }
    } else {
        None
    };

    let next_target = if let Some((ref c, ref parent_world)) = dest_cell_info {
        println!(
            "  - テレポート先セルを解決: \"{}\" (FormID: 0x{:08X})",
            c.edid, c.form_id.0
        );
        if c.is_interior() || parent_world.is_none() {
            ViewerTarget::Cell(c.edid.clone())
        } else {
            let world = parent_world.as_ref().unwrap();
            println!(
                "  - 所属ワールドスペースを解決: \"{}\" (FormID: 0x{:08X}, 親: {:?})",
                world.edid, world.form_id.0, world.parent_world
            );
            ViewerTarget::World(world.edid.clone(), c.grid)
        }
    } else {
        println!("  - 相手側ドアからセル特定不能のため、出現座標グリッドからロードを試行します");
        let gx = (tp.dest_pos[0] / 4096.0).floor() as i32;
        let gy = (tp.dest_pos[1] / 4096.0).floor() as i32;
        ViewerTarget::World("Wasteland".to_string(), Some((gx, gy)))
    };

    println!("  - 次のセルをロード中: {:?}...", next_target);
    let loaded = load_scene(
        &app.device,
        &app.queue,
        &app.context,
        &app.data_dir,
        &next_target,
        &mut app.vfs,
        &app.master_context,
        &mut app.nif_cache,
        &mut app.texture_cache,
    );

    // シーンおよびワールド状態の置換
    app.scene = loaded.scene;
    app.cell_lighting = loaded.cell_lighting;
    app.placed_lights = loaded.placed_lights;
    app.clear_color = loaded.clear_color;
    app.interactables = loaded.interactables;
    app.refr_bindings = loaded.refr_bindings;
    app.animators.clear();
    app.focused_interactable = None;
    app.anim = AnimState::new(&next_target, &mut app.vfs);

    // プレイヤーの出現位置および姿勢角の設定
    let marker_pos = glam::Vec3::new(tp.dest_pos[0], tp.dest_pos[1], tp.dest_pos[2]);
    let ray_origin = marker_pos + glam::Vec3::new(0.0, 0.0, 100.0);
    let ray_dir = glam::Vec3::new(0.0, 0.0, -1.0);
    let spawn_pos = if let Some(hit) =
        loaded.physics_world.cast_ray(ray_origin, ray_dir, 200.0)
    {
        println!(
            "  - レイキャスト接地成功: {:.1} -> {:.1}",
            marker_pos.z, hit.point.z
        );
        hit.point + glam::Vec3::new(0.0, 0.0, 5.0)
    } else {
        marker_pos
    };

    app.controller.character_controller.position = spawn_pos;
    app.controller.initial_spawn_point = spawn_pos;
    app.controller.player_camera.current_eye =
        spawn_pos + glam::Vec3::new(0.0, 0.0, 60.0);
    app.controller.player_camera.yaw = tp.dest_rot[2];
    app.controller.player_camera.pitch = 0.0;
    app.controller.camera.yaw = tp.dest_rot[2];
    app.controller.camera.pitch = 0.0;
    if let Some(ref mut player) = app.controller.player_actor {
        player.position = spawn_pos;
    }

    // 物理ワールドの再初期化
    app.controller.physics_world = loaded.physics_world;

    println!("  - セル遷移完了。新しいシーンを描画開始します。");
    update_window_title(&app.window, &next_target);
}
