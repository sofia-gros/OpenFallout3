//! # インタラクション & オブジェクト操作アクション
//!
//! ドア遷移、コンテナ開閉、アイテム拾得、NPC 会話開始、ターミナル起動アクション。
//! 参照元:
//! - Gamebryo 2.6 セル遷移 & `references/openmw/components/esm4/loadrefr.cpp:103-127`
//! - `references/openmw/components/esm4/loaddial.hpp`, `loadinfo.hpp`
//! - `menus/dialog/dialog_menu.xml`, `menus/terminal/terminal_menu.xml`

use fo3_esm::FormId;
use fo3_script::{evaluate_conditions, ConditionContext};
use crate::anim::AnimState;
use crate::app::ViewerState;
use crate::interact::InteractableKind;
use crate::interactive_anim::InteractiveAnimator;
use crate::loader::load_scene;
use crate::types::{update_window_title, ViewerTarget};
use crate::ui::{DialogChoice, DialogState, TerminalState, ViewerMode};

/// フォーカス中オブジェクトに対するインタラクトアクション (`E` キー) を実行する。
pub fn perform_interact(app: &mut ViewerState) {
    let focused = match &app.focused_interactable {
        Some(f) => f.clone(),
        None => {
            println!("[インタラクト] 正面に操作可能なオブジェクトがありません。");
            return;
        }
    };

    println!("[インタラクト実行] {}", focused.prompt_text());

    // Phase 10: オブジェクトスクリプト (OnActivate) の実行とアクティベート抑制判定
    // 参照元: references/openmw/apps/openmw/mwworld/refdata.cpp (Flag_SuppressActivate)
    let target_id = FormId(focused.form_id);
    let player_id = FormId(0x00000014);
    if let Ok(suppressed) = app.dispatcher.dispatch_activate(target_id, player_id, &mut app.vm) {
        if suppressed {
            println!("  - スクリプト (OnActivate) によりアクティベートが抑制・処理されました。");
            return;
        }
    }

    match &focused.kind {
        InteractableKind::Door { teleport, lock, is_open } => {
            if let Some(l) = lock {
                if !app.vm.is_unlocked(FormId(focused.form_id)) {
                    println!("  - このドアは施錠されています (難易度: {})。鍵または開錠が必要です。", l.lock_level);
                    return;
                }
            }
            if let Some(ref tp) = teleport {
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
                    println!("  - テレポート先セルを解決: \"{}\" (FormID: 0x{:08X})", c.edid, c.form_id.0);
                    if c.is_interior() || parent_world.is_none() {
                        ViewerTarget::Cell(c.edid.clone())
                    } else {
                        let world = parent_world.as_ref().unwrap();
                        println!("  - 所属ワールドスペースを解決: \"{}\" (FormID: 0x{:08X}, 親: {:?})", world.edid, world.form_id.0, world.parent_world);
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
                let spawn_pos = if let Some(hit) = loaded.physics_world.cast_ray(ray_origin, ray_dir, 200.0) {
                    println!("  - テレポート先床面コリジョン検出: Z = {:.1} -> スポーン中心 Z = {:.1}", hit.point.z, hit.point.z + 65.0);
                    glam::Vec3::new(marker_pos.x, marker_pos.y, hit.point.z + 65.0)
                } else {
                    println!("  - テレポート先床面レイキャスト未ヒット: デフォルトオフセット (+65.0) で配置");
                    marker_pos + glam::Vec3::new(0.0, 0.0, 65.0)
                };

                app.controller.physics_world = loaded.physics_world;
                app.controller.initial_spawn_point = spawn_pos;
                app.controller.character_controller.position = spawn_pos;
                app.controller.character_controller.is_grounded = true;
                app.controller.vertical_velocity = 0.0;
                app.controller.camera.yaw = tp.dest_rot[2];
                app.controller.camera.pitch = tp.dest_rot[0].clamp(-1.2, 1.2);
                app.controller.camera.distance = 150.0;
                app.controller.camera.target = spawn_pos;

                update_window_title(&app.window, &next_target);
                app.setup_scripts_for_cell();
                println!("  - セル間テレポート完了: プレイヤー座標 {:?}", spawn_pos);
            } else {
                let currently_open = *is_open;
                let target_open = !currently_open;
                println!(
                    "  - 通常ドア開閉アニメーション起動: {} (現在: {})",
                    if target_open { "開く" } else { "閉じる" },
                    if currently_open { "開" } else { "閉" }
                );
                let form_id = focused.form_id;
                if let Some(binding) = app.refr_bindings.get(&form_id) {
                    let anim = app.animators.entry(form_id).or_insert_with(|| {
                        InteractiveAnimator::from_binding(binding, currently_open)
                    });
                    anim.toggle();
                }

                // インタラクティブ対象の状態を更新
                if let Some(obj) = app.interactables.iter_mut().find(|o| o.form_id == form_id) {
                    if let InteractableKind::Door { ref mut is_open, .. } = obj.kind {
                        *is_open = target_open;
                    }
                }
                if let Some(ref mut obj) = app.focused_interactable {
                    if obj.form_id == form_id {
                        if let InteractableKind::Door { ref mut is_open, .. } = obj.kind {
                            *is_open = target_open;
                        }
                    }
                }
            }
        }
        InteractableKind::Container { form_id: _, lock, is_open } => {
            if let Some(l) = lock {
                if !app.vm.is_unlocked(FormId(focused.form_id)) {
                    println!("  - このコンテナは施錠されています (難易度: {})。鍵または開錠が必要です。", l.lock_level);
                    return;
                }
            }
            let currently_open = *is_open;
            let target_open = !currently_open;
            println!(
                "  - コンテナ開閉アニメーション起動: \"{}\" -> {}",
                focused.name,
                if target_open { "開く" } else { "閉じる" }
            );
            let form_id = focused.form_id;
            if let Some(binding) = app.refr_bindings.get(&form_id) {
                let anim = app.animators.entry(form_id).or_insert_with(|| {
                    InteractiveAnimator::from_binding(binding, currently_open)
                });
                anim.toggle();
            }

            // インタラクティブ対象の状態を更新
            if let Some(obj) = app.interactables.iter_mut().find(|o| o.form_id == form_id) {
                if let InteractableKind::Container { ref mut is_open, .. } = obj.kind {
                    *is_open = target_open;
                }
            }
            if let Some(ref mut obj) = app.focused_interactable {
                if obj.form_id == form_id {
                    if let InteractableKind::Container { ref mut is_open, .. } = obj.kind {
                        *is_open = target_open;
                    }
                }
            }
        }
        InteractableKind::Item { form_id: base_form_id } => {
            // アイテムをプレイヤーインベントリおよび VM 所持品へ追加
            app.inventory.add_item(FormId(*base_form_id), 1, &focused.name);
            app.vm.add_item(FormId(*base_form_id), 1);
            let total = app.inventory.get_count(FormId(*base_form_id));
            println!(
                "  - アイテム取得: \"{}\" (FormID: 0x{:08X}) をインベントリに追加 (所持数: {})",
                focused.name, base_form_id, total
            );

            // 物理ワールドから剛体を削除
            if let Some(binding) = app.refr_bindings.get(&focused.form_id) {
                for rb in &binding.static_rigid_bodies {
                    app.controller.physics_world.remove_rigid_body(*rb);
                }
                for part in &binding.moving_parts {
                    for rb in &part.rigid_bodies {
                        app.controller.physics_world.remove_rigid_body(*rb);
                    }
                }
                // GPU メッシュをスケール 0 にして不可視化
                let mut all_meshes = binding.static_mesh_indices.clone();
                for part in &binding.moving_parts {
                    all_meshes.extend_from_slice(&part.mesh_indices);
                }
                for mesh_idx in all_meshes {
                    if let Some(mesh) = app.scene.meshes.get(mesh_idx) {
                        app.queue.write_buffer(
                            &mesh.model_uniform_buffer,
                            0,
                            bytemuck::cast_slice(&[[[0.0f32; 4]; 4]]),
                        );
                    }
                }
            }

            // インタラクト候補から削除
            app.interactables.retain(|obj| obj.form_id != focused.form_id);
            app.focused_interactable = None;
        }
        InteractableKind::Actor { form_id: actor_form_id, is_dead } => {
            if *is_dead {
                println!("  - アクター \"{}\" の所持品を調べます。", focused.name);
                return;
            }
            println!("  - アクター \"{}\" (FormID: 0x{:08X}) との会話を開始します...", focused.name, actor_form_id);

            let cond_ctx = ConditionContext {
                speaker: Some(FormId(*actor_form_id)),
                target: Some(FormId(0x00000014)), // Player FormID
                speaker_pos: focused.position,
                player_pos: app.controller.camera.target,
                quest_stages: app.vm.quest_stages.clone(),
                quest_stage_history: app.vm.quest_manager.get_stage_history_u32(),
                inventory: app.vm.inventory.clone(),
            };

            let esm_path = std::path::Path::new(&app.data_dir).join("Fallout3.esm");
            if let Ok(mut reader) = fo3_esm::EsmReader::open(&esm_path) {
                match reader.find_npc_dialogue(FormId(*actor_form_id)) {
                    Ok((greeting_opt, topics)) => {
                        let greeting = greeting_opt
                            .as_ref()
                            .map(|g| g.response_text.as_str())
                            .unwrap_or("何か用か？");

                        let choices: Vec<DialogChoice> = topics
                            .into_iter()
                            .filter_map(|(dial, infos)| {
                                // CTDA 条件式を満たす INFO を検索
                                let valid_info = infos.into_iter().find(|info| {
                                    evaluate_conditions(&info.conditions, &cond_ctx)
                                });

                                valid_info.map(|info| {
                                    let prompt = dial.prompt.clone().unwrap_or(dial.edid.clone());
                                    DialogChoice {
                                        prompt,
                                        response: info.response_text.clone(),
                                        is_goodbye: info.is_goodbye(),
                                        result_script: info.result_script_source.clone(),
                                        info_form_id: Some(info.form_id),
                                    }
                                })
                            })
                            .collect();

                        let dialog_state = DialogState::new(&focused.name, greeting, choices);
                        println!("  - 会話UIモードへ遷移: 挨拶「{}」 (有効選択肢: {} 件)", greeting, dialog_state.choices.len());
                        app.mode = ViewerMode::Dialog(dialog_state);
                    }
                    _ => {
                        println!("  - アクター \"{}\" には利用可能な会話データがありません。", focused.name);
                    }
                }
            }
        }
        InteractableKind::Terminal { form_id: term_form_id, lock } => {
            if let Some(l) = lock {
                if !app.vm.is_unlocked(FormId(focused.form_id)) {
                    println!("  - このターミナルは施錠されています (難易度: {})。ハッキングまたは開錠が必要です。", l.lock_level);
                    return;
                }
            }
            println!("  - ターミナル \"{}\" (FormID: 0x{:08X}) を起動します...", focused.name, term_form_id);
            let esm_path = std::path::Path::new(&app.data_dir).join("Fallout3.esm");
            if let Ok(mut reader) = fo3_esm::EsmReader::open(&esm_path) {
                match reader.find_terminal(FormId(*term_form_id)) {
                    Ok(Some(term_rec)) => {
                        let term_state = TerminalState::from_record(&term_rec);
                        println!("  - ターミナルUIモードへ遷移: \"{}\" (項目: {} 件)", term_state.title, term_state.menu_items.len());
                        app.mode = ViewerMode::Terminal(term_state);
                    }
                    _ => {
                        println!("  - ターミナルデータが見つかりませんでした。");
                    }
                }
            }
        }
        InteractableKind::Activator { .. } => {
            println!("  - アクティベーター \"{}\" を作動させました。", focused.name);
        }
    }
}
