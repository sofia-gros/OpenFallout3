//! # インタラクション & オブジェクト操作アクション
//!
//! ドア遷移、コンテナ開閉、アイテム拾得、NPC 会話開始、ターミナル起動アクション。
//! 参照元:
//! - Gamebryo 2.6 セル遷移 & `references/openmw/components/esm4/loadrefr.cpp:103-127`
//! - `references/openmw/components/esm4/loaddial.hpp`, `loadinfo.hpp`
//! - `menus/dialog/dialog_menu.xml`, `menus/terminal/terminal_menu.xml`

use fo3_esm::{EsmMasterContext, FormId, PackRecord};
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
                // 遷移先セルの EDID を解決してスクリプト登録
                let next_cell_edid = match &next_target {
                    ViewerTarget::Cell(edid) => edid.clone(),
                    _ => String::new(),
                };
                app.setup_scripts_for_cell(&next_cell_edid);
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
        InteractableKind::Actor { form_id: actor_form_id, base_form_id, is_dead } => {
            if *is_dead {
                println!("  - アクター \"{}\" の所持品を調べます。", focused.name);
                return;
            }
            println!("  - アクター \"{}\" (Base: 0x{:08X}, REFR: 0x{:08X}) との会話を開始します...", focused.name, base_form_id, actor_form_id);

            let cond_ctx = ConditionContext {
                speaker: Some(FormId(*base_form_id)),
                target: Some(FormId(0x00000014)), // Player FormID
                speaker_pos: focused.position,
                player_pos: app.controller.camera.target,
                quest_stages: app.vm.quest_stages.clone(),
                quest_stage_history: app.vm.quest_manager.get_stage_history_u32(),
                inventory: app.vm.inventory.clone(),
                is_female: app.vm.player_is_female,
            };

            let esm_path = std::path::Path::new(&app.data_dir).join("Fallout3.esm");
            if let Ok(mut reader) = fo3_esm::EsmReader::open(&esm_path) {
                match reader.find_npc_dialogue(FormId(*base_form_id)) {
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

                        let mut dialog_state = DialogState::new(&focused.name, greeting, choices);

                        // VFS から実機 menus/dialog/dialog_menu.xml をロードして MenuRuntime を構築
                        if let Ok(xml_bytes) = app.vfs.read("menus/dialog/dialog_menu.xml") {
                            let xml_str = String::from_utf8_lossy(&xml_bytes);
                            let top_bracket = app.vfs.read("menus/prefabs/top_bracket.xml").ok().map(|b| String::from_utf8_lossy(&b).to_string());
                            let bottom_bracket = app.vfs.read("menus/prefabs/bottom_bracket.xml").ok().map(|b| String::from_utf8_lossy(&b).to_string());
                            let list_box = app.vfs.read("menus/prefabs/list_box.xml").ok().map(|b| String::from_utf8_lossy(&b).to_string());

                            let loader = |prefab_name: &str| -> Option<String> {
                                let clean = prefab_name.to_lowercase();
                                if clean.contains("top_bracket") {
                                    top_bracket.clone()
                                } else if clean.contains("bottom_bracket") {
                                    bottom_bracket.clone()
                                } else if clean.contains("list_box") {
                                    list_box.clone()
                                } else {
                                    None
                                }
                            };
                            let parser = fo3_render::MenuXmlParser::new(Some(&loader));
                            if let Ok(root) = parser.parse(&xml_str) {
                                let mut runtime = fo3_render::MenuRuntime::new(root);
                                if let Ok(tai_bytes) = app.vfs.read("textures/interface/interfaceshared.tai") {
                                    let tai_str = String::from_utf8_lossy(&tai_bytes);
                                    runtime.atlas = Some(fo3_render::TextureAtlas::parse(&tai_str));
                                }
                                dialog_state.menu_runtime = Some(runtime);
                                println!("  - 実機 Menu XML (menus/dialog/dialog_menu.xml) ランタイム初期化完了");
                            }
                        }

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

/// スクリプトからのテレポート移動要求 (MoveTo) を消化し、アクターやプレイヤーを移動させる。
pub fn process_teleport_requests(app: &mut ViewerState) {
    while !app.vm.teleport_requests.is_empty() {
        let (subject, marker) = app.vm.teleport_requests.remove(0);
        let marker_data = match marker.to_ascii_lowercase().as_str() {
            // --- CG00: Vault 101 Infirmary (出産シーン) ---
            "cg00playerstartmarker"     => Some((glam::Vec3::new(-5275.8867, -7148.175, 7542.536), glam::Vec3::new(0.0, 0.0, std::f32::consts::PI))),
            "cg00momstartmarker"        => Some((glam::Vec3::new(-5275.8867, -7250.0, 7542.536), glam::Vec3::new(0.0, 0.0, 0.0))),
            "cg00dadstartmarker"        => Some((glam::Vec3::new(-5360.3623, -7332.082, 7542.536), glam::Vec3::new(0.0, 0.0, 6.19592))),
            "cg00doctorlistartmarker"   => Some((glam::Vec3::new(-5190.0, -7330.0, 7542.536), glam::Vec3::new(0.0, 0.0, 0.0))),
            // --- CG01: Vault 101 Atrium (1歳・幼児期) ---
            // 参照元: Fallout3.esm CELL "Vault101Atrium" REFR CG01PlayerStartMarker (近似座標)
            "cg01playerstartmarker"     => Some((glam::Vec3::new(-5275.8867, -7148.175, 7542.536), glam::Vec3::new(0.0, 0.0, std::f32::consts::PI))),
            "cg01dadstartmarker"        => Some((glam::Vec3::new(-5360.0, -7200.0, 7542.536), glam::Vec3::new(0.0, 0.0, 0.0))),
            // --- CG02: Vault 101 教室 (10歳・GOAT テスト前) ---
            // 参照元: Fallout3.esm CELL "Vault101Classroom" REFR CG02PlayerStartMarker (近似座標)
            "cg02playerstartmarker"     => Some((glam::Vec3::new(-5275.8867, -7148.175, 7542.536), glam::Vec3::new(0.0, 0.0, 0.0))),
            "cg02dadstartmarker"        => Some((glam::Vec3::new(-5360.0, -7200.0, 7542.536), glam::Vec3::new(0.0, 0.0, 0.0))),
            // --- CG03: Vault 101 廊下 (16歳・GOAT テスト結果) ---
            // 参照元: Fallout3.esm CELL "Vault101" REFR CG03PlayerStartMarker (近似座標)
            "cg03playerstartmarker"     => Some((glam::Vec3::new(-5275.8867, -7148.175, 7542.536), glam::Vec3::new(0.0, 0.0, 0.0))),
            "cg03dadstartmarker"        => Some((glam::Vec3::new(-5360.0, -7200.0, 7542.536), glam::Vec3::new(0.0, 0.0, 0.0))),
            // --- CG04: Vault 101 (18歳・Birthday パーティー) ---
            // 参照元: Fallout3.esm CELL "Vault101Atrium" REFR CG04PlayerStartMarker (近似座標)
            "cg04playerstartmarker"     => Some((glam::Vec3::new(-5275.8867, -7148.175, 7542.536), glam::Vec3::new(0.0, 0.0, 0.0))),
            "cg04dadstartmarker"        => Some((glam::Vec3::new(-5360.0, -7200.0, 7542.536), glam::Vec3::new(0.0, 0.0, 0.0))),
            _ => None,
        };

        if let Some((pos, rot)) = marker_data {
            let target_form_id = subject.or_else(|| {
                match marker.to_ascii_lowercase().as_str() {
                    "cg00dadstartmarker" => Some(FormId(0x000290A7)),
                    "cg00doctorlistartmarker" => Some(FormId(0x000290A5)),
                    "cg00momstartmarker" => Some(FormId(0x0005EDE0)),
                    _ => None,
                }
            });

            if let Some(fid) = target_form_id {
                if let Some(actor) = app.scene.actors.iter_mut().find(|a| a.form_id == fid.0) {
                    actor.world_transform.translation = pos;
                    actor.world_transform.rotation = glam::Mat3::from_euler(glam::EulerRot::XYZ, rot.x, rot.y, rot.z);
                    println!("[MoveTo] アクター 0x{:08X} をマーカー \"{}\" (pos={:?}) へ配置完了", fid.0, marker, pos);
                }
            } else {
                app.controller.character_controller.position = pos;
                app.controller.initial_spawn_point = pos;
                app.controller.player_camera.current_eye = pos + glam::Vec3::new(0.0, 0.0, 60.0);
                app.controller.player_camera.yaw = -2.15;
                app.controller.player_camera.pitch = 0.52;
                app.controller.camera.yaw = -2.15;
                app.controller.camera.pitch = 0.52;
                if let Some(ref mut player) = app.controller.player_actor {
                    player.position = pos;
                }
                println!("[MoveTo] プレイヤーをマーカー \"{}\" (pos={:?}) へテレポート完了", marker, pos);
            }
        } else {
            println!("[MoveTo] 未知のマーカー \"{}\" への配置要求 (スキップ)", marker);
        }
    }
}

/// PACK レコードの Idle Collection (IDLA->IDLE->MODL) からロード可能な KF ファイルパス一覧を解決する。
///
/// チェーン: PACK `IDLA` (IDLE FormID リスト) -> IDLE レコード -> `MODL` (KF 相対パス C-String)。
/// 戻り値は VFS 読み取り用に小文字化 + `meshes\` プレフィックス正規化済み。
///
/// 参照元: `references/bevyout/src/vsa/openmw_esm4/actor_support.rs:L659-745` (decode_package_idle_collection),
///         `references/bevyout/src/vsa/openmw_esm4/idle.rs:L120-193` (IDLE の MODL パース),
///         `knowledge/phase11_ai_package_and_quest_progression.md` (セクション 4)
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
///
/// Gamebryo 2.6 では PKID は優先順位付きリストであり、エンジンは先頭から順に PACK の
/// CTDA 条件式を評価し、最初に全条件が成立した PACK を適用する。条件を持たない
/// Start / Default 系 PACK が最終フォールバックとなる (evaluate_conditions が空リスト
/// を真とみなすため)。
///
/// 実機 ESM 検証 (CG00 クエスト): Dad (0x000290A6) / Mom (0x0005EDDF) / DrLi (0x000290A3)
/// 各 NPC の PKID は Section5 -> Section4 -> ... -> Section0 -> Start/Default の順で並び、
/// 各 Section PACK の CTDA は `GetStage(0x0001F388) >= {8,10,20,40,60,80}` 
/// (fnIndex=58, operator=0x60 -> GTE) で統一されている。
///
/// 参照元: `references/openmw/components/esm4/loadnpc.cpp` (PKID),
///         `references/bevyout/src/vsa/openmw_esm4/actor_support.rs:L534-600`,
///         `crates/fo3_script/src/conditions.rs` (evaluate_conditions)
pub(crate) fn resolve_pack_for_actor<'a>(
    ctx: &'a EsmMasterContext,
    base_npc: FormId,
    cond_ctx: &ConditionContext,
) -> Option<&'a PackRecord> {
    let npc = ctx.npc_map.get(&base_npc)?;
    for pkid in &npc.ai_packages {
        if let Some(pkg) = ctx.pack_map.get(pkid) {
            if evaluate_conditions(&pkg.conditions, cond_ctx) {
                return Some(pkg);
            }
        }
    }
    None
}

/// PACK の Idle Collection からロード済み (nif, clip) を組み立てる共通適用処理。
/// 複数 KF 候補を先頭から順に試し、初めて正常に NIF+クリップ化できた候補を返す。
fn load_first_clip(app: &mut ViewerState, kf_paths: &[String]) -> Option<(String, std::sync::Arc<fo3_nif::NifFile>, std::sync::Arc<fo3_render::AnimationClip>)> {
    for path in kf_paths {
        if let Ok(bytes) = app.vfs.read(path) {
            let mut cursor = std::io::Cursor::new(bytes);
            if let Ok(kf) = fo3_nif::NifFile::read(&mut cursor) {
                if let Some(clip) = fo3_render::AnimationClip::from_kf(&kf) {
                    return Some((path.clone(), std::sync::Arc::new(kf), std::sync::Arc::new(clip)));
                }
            }
        }
    }
    None
}

/// EvaluatePackage 要求の消化 & AddScriptPackage 要求の消化 (パッケージアニメーション適用)。
///
/// - **AddScriptPackage**: スクリプトが名前指定した PACK を EDID 一致で解決し、その
///   Idle Collection (IDLA->IDLE->MODL) から KF をロードする。
/// - **EvaluatePackage**: 対象アクターの NPC ベース PKID リストを先頭から評価し、
///   CTDA 条件が最初に成立した PACK の Idle Collection から KF を適用する (データ駆動)。
///   CG00_* 固有のステージ連動 KF ハードコードは廃止し、実機 ESM の
///   `NPC.PKID -> PACK.CTDA (GetStage >= 閾値) -> IDLE.MODL` を直接解決する。
pub fn process_package_requests(app: &mut ViewerState) {
    let mut add_pkgs = Vec::new();
    std::mem::swap(&mut add_pkgs, &mut app.vm.script_package_requests);

    let mut evp_requests = Vec::new();
    std::mem::swap(&mut evp_requests, &mut app.vm.evaluate_package_requests);

    // 1. AddScriptPackage 要求の消化
    for (subject_opt, pkg_name) in add_pkgs {
        let target_fid = subject_opt.unwrap_or(FormId(0x00000014));

        // KF ファイルパスの解決
        let mut kf_paths = Vec::new();

        // 1a. PACK レコードを EDID 一致で解決し、Idle Collection (IDLA->IDLE->MODL) から KF を取得
        if let Some(pkg) = app.master_context
            .pack_map
            .values()
            .find(|p| p.editor_id.as_deref().map(|e| e.eq_ignore_ascii_case(&pkg_name)).unwrap_or(false))
        {
            if let Some(actor_state) = app.ai.actors.get_mut(&target_fid) {
                if !actor_state.script_packages.contains(&pkg.form_id) {
                    actor_state.script_packages.insert(0, pkg.form_id); // 高優先度
                }
            }

            kf_paths.extend(resolve_idle_kf_from_pack(&app.master_context, pkg));
        }

        // 1b. 条件なし PACK (条件式を持たない Start / Default 系 PACK) は
        // AddScriptPackage からは直接適用されない (EvaluatePackage 経由でのみ評価)。
        // 経路を単一化し、推測による `idleanims\{edid}.kf` フォールバックは廃止した。

        if let Some((path, kf, clip)) = load_first_clip(app, &kf_paths) {
            if target_fid == FormId(0x00000014) {
                if let Some(ref player) = app.controller.player_actor {
                    if player.third_person_actor_idx < app.scene.actors.len() {
                        app.scene.actors[player.third_person_actor_idx].set_animation(kf, clip);
                        println!("[Package] プレイヤーにアニメーション \"{}\" を適用", path);
                    }
                }
            } else if let Some(actor) = app.scene.actors.iter_mut().find(|a| a.form_id == target_fid.0) {
                actor.set_animation(kf, clip);
                println!("[Package] アクター 0x{:08X} にアニメーション \"{}\" を適用", target_fid.0, path);
            }
        } else {
            println!("[Package] アニメーション KF が見つかりません: package=\"{}\"", pkg_name);
        }
    }

    // 2. EvaluatePackage (evp) 要求の消化
    //
    // データ駆動解決 (モック排除): REFR FormID -> ベース NPC FormID -> PKID (PACK リスト)
    // -> CTDA 条件式評価 -> 適用 PACK -> Idle Collection (IDLA -> IDLE -> MODL) -> KF ファイル。
    // 従来の CG00_* 固有 KF ハードコード (REFR FormID ベースのステージ連動) は廃止し、
    // Fallout3.esm の NPC.PKID / PACK.CTDA / IDLE.MODL を直接参照して解決する。
    for subject_opt in evp_requests {
        let target_fid = subject_opt.unwrap_or(FormId(0x00000014));

        // REFR FormID -> NPC ベース FormID への解決 (REFR の NAME サブレコード由来)
        let base_npc = app.interactables.iter().find_map(|it| match it.kind {
            InteractableKind::Actor { form_id, base_form_id, .. } if form_id == target_fid.0 => {
                Some(fo3_esm::FormId(base_form_id))
            }
            _ => None,
        });
        let Some(base_npc) = base_npc else { continue };

        // PKID (NPC.ai_packages) を先頭から評価し、最初に CTDA が成立した PACK を決定する。
        // 借用はブロック内で完結させる (pkg は master_context への借用のため)。
        let (pkg_edid, kf_paths) = {
            let mut cond_ctx = ConditionContext::default();
            cond_ctx.speaker = Some(base_npc);
            cond_ctx.target = Some(target_fid);
            cond_ctx.quest_stages = app.vm.quest_stages.clone();
            cond_ctx.quest_stage_history = app.vm.quest_manager.get_stage_history_u32();
            match resolve_pack_for_actor(&app.master_context, base_npc, &cond_ctx) {
                Some(p) => (
                    p.editor_id.clone().unwrap_or_default(),
                    resolve_idle_kf_from_pack(&app.master_context, p),
                ),
                None => (String::new(), Vec::new()),
            }
        };

        // 適用 PACK の Idle Collection 由来 KF をロードしてアクターに適用
        if let Some((path, kf, clip)) = load_first_clip(app, &kf_paths) {
            if let Some(actor) = app.scene.actors.iter_mut().find(|a| a.form_id == target_fid.0) {
                actor.set_animation(kf, clip);
                println!("[AI/EVP] アクター 0x{:08X} (\"{}\") に PACK \"{}\" 由来のアニメーション \"{}\" を適用",
                    target_fid.0, actor.name, pkg_edid, path);
            }
        }
    }
}



/// `PlayGroup` や `PlayAnim` によるアニメーション再生要求を処理する。
/// Actorだけでなく、ActivatorやStatic(例: gene_projector.nif)にも適用する。
pub fn process_playgroup_requests(app: &mut ViewerState) {
    let mut requests = Vec::new();
    std::mem::swap(&mut requests, &mut app.vm.playgroup_queue);

    for (target_fid, anim_name) in requests {
        if let Some(actor) = app.scene.actors.iter_mut().find(|a| a.form_id == target_fid.0) {
            let mut kf_paths = vec![
                format!("meshes/characters/_male/{}", anim_name), // Actor default
            ];
            
            // If actor's name is a model path ending in .nif, add a KF path in its directory
            if actor.name.to_lowercase().ends_with(".nif") {
                let path = std::path::Path::new(&actor.name);
                if let Some(parent) = path.parent() {
                    let parent_str = parent.to_string_lossy().replace("\\", "/");
                    kf_paths.push(format!("{}/{}", parent_str, anim_name));
                    
                    // Also try lowercase anim_name
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
                            println!("[Action] Actor {:08X} の PlayGroup: {} (KF: {}) を再生します", target_fid.0, anim_name, kf_path);
                            kf_loaded = true;
                            break;
                        }
                    }
                }
            }
            if !kf_loaded {
                println!("[Action] [stub] 3Dオブジェクト {:08X} の PlayGroup: {} の KF が見つかりません (モデル: {})", target_fid.0, anim_name, actor.name);
            }
        } else {
            // 非Actorのアニメーション (gene_projector等) は stub
            println!("[Action] [stub] 3Dオブジェクト {:08X} が存在しないため PlayGroup: {} は失敗しました", target_fid.0, anim_name);
        }
    }
}
