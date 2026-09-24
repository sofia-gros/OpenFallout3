//! ビューアーの毎フレーム更新処理 (`ViewerState::update`)。
//!
//! 参照元: Gamebryo 2.6 `NiAVObject::Update` / レンダリングパイプライン & スクリプト駆動

use fo3_esm::FormId;
use fo3_render::{LightingUniform, PlacedPointLight};
use crate::interact::{
    find_focused_by_raycast, find_focused_interactable, F_ACTIVATE_PICK_LENGTH,
};
use super::ViewerState;

impl ViewerState {
    /// 毎フレームのワールド状態、物理、スクリプト、アニメーション、カメラの更新を行う。
    pub fn update(&mut self) {
        self.input_manager.update_frame();
        let dt = self.controller.update();

        self.frame_count += 1;
        if self.frame_count <= 5 || self.frame_count % 30 == 0 {
            println!("[ENGINE:HEARTBEAT] Frame {} (dt: {:.3}s, bink: {})", self.frame_count, dt, self.bink_player.is_some());
        }

        // 0. スクリプトからの動画再生要求の消化
        // playBink コマンドはファイル名のみを渡すため、data_dir/Video/ パスに解決する。
        // 参照元: Fallout 3 実機 `playBink "1 year later.bik"` — Data/Video/ フォルダ基準
        // 別ウィンドウは生成せず、ゲームウィンドウ内テクスチャとして描画する。
        while !self.vm.play_bink_queue.is_empty() {
            let bink_name = self.vm.play_bink_queue.remove(0);
            let bink_path = if std::path::Path::new(&bink_name).is_absolute() {
                bink_name.clone()
            } else {
                std::path::Path::new(&self.data_dir)
                    .join("Video")
                    .join(&bink_name)
                    .to_string_lossy()
                    .to_string()
            };
            // 現在再生中のムービーを停止して新しいムービーを開始
            self.bink_video_bind_group = None;
            self.bink_player = crate::bink_player::BinkPlayer::open(
                &bink_path,
                self.size.width,
                self.size.height,
                &self.device,
                self.sound_engine.output_handle(),
            );
            // バインドグループを生成してキャッシュ
            if let Some(ref player) = self.bink_player {
                let bg = self
                    .hud
                    .create_video_bind_group(&self.device, &player.texture_view);
                self.bink_video_bind_group = Some(bg);
            }
        }

        // 再生中の Bink ムービーの次フレームを取得 (終了したら bink_player をクリア)
        if let Some(ref mut player) = self.bink_player {
            if !player.advance_frame(&self.queue) {
                println!("[BinkPlayer] ムービー再生終了");
                self.bink_player = None;
                self.bink_video_bind_group = None;
            }
        }

        // ムービー再生中はゲームワールドのシミュレーションを進めない。
        // 旧 play_bink_video (ffplay 外部ウィンドウ・ブロッキング) と同等の
        // 「動画再生中はゲーム進行を停止」する動作を維持する。
        if self.bink_player.is_some() {
            return;
        }

        // 0.1 スクリプトからのテレポート移動要求 (MoveTo) の消化
        crate::action::process_teleport_requests(self);

        // 0.2 スクリプトからの AI パッケージ操作要求 (AddScriptPackage / evp) を処理
        crate::action::process_package_requests(self);
        crate::action::process_playgroup_requests(self);

        // 0.2.5 差分ロード (WorldStreaming) の処理
        // プレイヤーの現在位置からグリッドを再計算し、ロード／アンロード対象を検出
        let mut load_targets = Vec::new();
        let mut unload_targets = Vec::new();
        if let Some(streamer) = &mut self.streamer {
            let player_pos = self.controller.camera.target;
            if let Some((to_load, to_unload)) = streamer.check_update(player_pos) {
                println!(
                    "[WorldStreamer] ストリーミング更新 (ロード: {}, アンロード: {})",
                    to_load.len(),
                    to_unload.len()
                );
                for g in to_unload {
                    if let Some(cells) = streamer.loaded_cells_by_grid.remove(&g) {
                        unload_targets.extend(cells);
                    }
                }
                load_targets = to_load.clone();
            }
        }

        // アンロードの実行
        for cell_id in unload_targets {
            self.scene.unload_cell(cell_id);
        }

        // ロードの実行
        if !load_targets.is_empty() {
            let world_edid = self.streamer.as_ref().unwrap().world_edid.clone();
            let diff_scene = crate::loader::load_scene(
                &self.device,
                &self.queue,
                &self.context,
                "A:/Project/OpenFallout3",
                &crate::types::ViewerTarget::WorldGrids(world_edid, load_targets.clone()),
                &mut self.vfs,
                &self.master_context,
                &mut fo3_render::NifCache::new(),
                &mut std::collections::HashMap::new(),
            );

            if let Some(streamer) = &mut self.streamer {
                let mut cell_ids = std::collections::HashSet::new();
                for m in &diff_scene.scene.meshes {
                    if let Some(c) = m.cell_id {
                        cell_ids.insert(c);
                    }
                }
                if let Some(first_grid) = load_targets.first() {
                    streamer
                        .loaded_cells_by_grid
                        .insert(*first_grid, cell_ids.into_iter().collect());
                }
            }

            self.scene.append(diff_scene.scene);
            self.placed_lights.extend(diff_scene.placed_lights);
            self.interactables.extend(diff_scene.interactables);
        }

        // AI Package Evaluator を更新
        let mut actor_positions = std::collections::HashMap::new();
        for actor in &self.scene.actors {
            actor_positions.insert(FormId(actor.form_id), actor.world_transform.translation);
        }
        self.ai.update(
            &mut self.vm,
            &self.master_context,
            &self.nav_graph,
            &actor_positions,
        );

        // 0.3 サウンドエンジンを更新 (音声 DIAL/INFO/SOUN 再生)
        self.vm.chargen_menu_active = self.chargen_menu.is_active();
        self.sound_engine
            .update(dt, &mut self.vm, &self.master_context, &mut self.vfs);

        // 0.4 キャラクター作成イベント (GetPlayerName / ShowRaceMenu) のポーリング
        self.chargen_menu.poll_events(&mut self.vm);

        // 開閉アニメーションの進行および物理剛体・GPUメッシュの追従更新
        // 参照元: Gamebryo 2.6 `bhkRigidBody` (MO_SYS_KEYFRAMED) 追従
        for (_form_id, animator) in self.animators.iter_mut() {
            if animator.is_animating() {
                animator.update(dt);
                let part_transforms = animator.compute_part_transforms();
                for (mesh_indices, rigid_bodies, world_mat, pos, rot) in part_transforms {
                    // 1. 可動パーツの物理剛体の位置・回転を同期
                    for rb in rigid_bodies {
                        self.controller
                            .physics_world
                            .set_rigid_body_transform(*rb, pos, rot);
                    }
                    // 2. 可動パーツの GPU 描画メッシュのワールド行列を同期
                    let model_mat = world_mat.to_cols_array_2d();
                    for &mesh_idx in mesh_indices {
                        if let Some(mesh) = self.scene.meshes.get(mesh_idx) {
                            self.queue.write_buffer(
                                &mesh.model_uniform_buffer,
                                0,
                                bytemuck::cast_slice(&[model_mat]),
                            );
                        }
                    }
                }
            }
        }

        let uniform = self.controller.camera.build_uniform();
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[uniform]));

        let mut current_lighting = self.cell_lighting.clone();
        if !self.enable_fog {
            if let Some(ref mut cl) = current_lighting {
                cl.fog_far = 0.0;
            }
        }

        let mut lights = self.placed_lights.clone();
        if self.headlight {
            lights.push(PlacedPointLight {
                position: self.controller.camera.eye_position(),
                radius: self.controller.camera.distance * 2.0 + 2000.0,
                color: [1.0, 0.98, 0.95],
                falloff: 1.0,
            });
        }

        let light_uniform = LightingUniform::from_cell_lighting_with_focus(
            current_lighting.as_ref(),
            &lights,
            self.controller.camera.eye_position(),
            Some(self.controller.camera.target),
        );
        self.queue.write_buffer(
            &self.lighting_buffer,
            0,
            bytemuck::cast_slice(&[light_uniform]),
        );

        // セル配置アクターの更新
        self.scene.update_actors(dt, &self.device, &self.queue);

        // プレイヤーアクター (三人称・一人称) の姿勢・位置・ロコモーション・GPUスキニング更新
        if let Some(ref mut player) = self.controller.player_actor {
            let feet_pos = self.controller.character_controller.feet_position();
            let cam_yaw = self.controller.player_camera.yaw;
            let is_moving = self.controller.key_forward
                || self.controller.key_backward
                || self.controller.key_left
                || self.controller.key_right;
            let fwd = glam::Vec3::new(cam_yaw.cos(), cam_yaw.sin(), 0.0).normalize();
            let rgt = glam::Vec3::new(cam_yaw.sin(), -cam_yaw.cos(), 0.0).normalize();
            let mut m_dir = glam::Vec3::ZERO;
            if self.controller.key_forward {
                m_dir += fwd;
            }
            if self.controller.key_backward {
                m_dir -= fwd;
            }
            if self.controller.key_right {
                m_dir += rgt;
            }
            if self.controller.key_left {
                m_dir -= rgt;
            }
            let move_opt = if is_moving {
                Some(m_dir.normalize())
            } else {
                None
            };

            player.update(
                dt,
                feet_pos,
                cam_yaw,
                is_moving,
                move_opt,
                &self.device,
                &self.queue,
                &mut self.scene.actors,
                &mut self.scene.meshes,
            );
        }

        // 単体 Anim / Actor モード時のスキニング・アニメーション更新
        self.anim
            .update(dt, &self.device, &self.queue, &mut self.scene);

        // 全 NPC アクターのアニメーション・ボーン姿勢・メッシュ更新と NavPath に沿った移動
        let walk_speed = 60.0; // 簡易的な歩行速度 (units/sec)

        for actor in &mut self.scene.actors {
            if let Some(ai_state) = self.ai.actors.get_mut(&FormId(actor.form_id)) {
                if let Some(path) = &ai_state.current_path {
                    if ai_state.path_target_index < path.points.len() {
                        let target_pos = path.points[ai_state.path_target_index];
                        let current_pos = actor.world_transform.translation;
                        let dir = target_pos - current_pos;
                        let dist = dir.length();

                        if dist < 10.0 {
                            ai_state.path_target_index += 1;
                        } else {
                            let move_dist = (walk_speed * dt).min(dist);
                            let move_dir = dir / dist;
                            actor.world_transform.translation += move_dir * move_dist;
                            // Yaw (Z軸回転) を進行方向に向ける
                            let yaw = move_dir.y.atan2(move_dir.x);
                            actor.world_transform.rotation = glam::Mat3::from_rotation_z(yaw);
                        }
                    } else {
                        ai_state.current_path = None;
                    }
                }
            }

            actor.update(dt, &self.device, &self.queue, &mut self.scene.meshes);
            // KF アニメーション完了時、OnAnimationEnd を 1 回だけ発行する
            // 参照元: `references/openmw/apps/openmw/mwlua/engineevents.hpp:63` (OnAnimationEnded)
            if let Some(ref mut player) = actor.anim_player {
                if player.is_finished() && !player.end_dispatched {
                    player.end_dispatched = true;
                    self.dispatcher
                        .push_event(fo3_script::GameEvent::OnAnimationEnd {
                            actor: fo3_esm::types::FormId(actor.form_id),
                        });
                }
            }
        }

        // キャラクター作成・拘束中の固定視点カメラ (SetInCharGen 1 & DisablePlayerControls 時)
        if self.vm.in_chargen && !self.vm.player_controls_enabled {
            let feet = self.controller.character_controller.feet_position();
            let eye = feet + glam::Vec3::new(0.0, 0.0, 22.0);
            self.controller.player_camera.current_eye = eye;
            self.controller.player_camera.yaw = -2.15;
            self.controller.player_camera.pitch = 0.52;
            self.controller.camera.override_eye = Some(eye);
            self.controller.camera.yaw = -2.15;
            self.controller.camera.pitch = 0.52;
        }

        // 毎フレームごとのスクリプト評価 (GameMode 駆動)
        self.vm.delta_time = dt;

        let events: Vec<_> = self.vm.pending_events.drain(..).collect();
        for ev in events {
            self.dispatcher.push_event(ev);
        }
        let var_updates: Vec<_> = self.vm.pending_var_updates.drain(..).collect();
        for (form_id, var_name, value) in var_updates {
            if let Some(instance) = self.dispatcher.get_instance_mut(form_id) {
                instance.local_vars.insert(var_name.clone(), value as f64);
            }
        }

        self.dispatcher.push_event(fo3_script::GameEvent::GameMode);
        let _ = self.dispatcher.process_queue(&mut self.vm);

        // 画面エフェクト (Image Space Modifier: 暗転・ホワイトアウト等のフェード演出)
        // 参照元: GECK `imod` (ImageSpace Modifier) 仕様
        let has_white_fade = self.vm.active_imods.iter().any(|m| {
            let lower = m.to_ascii_lowercase();
            lower.contains("birth") || lower.contains("white") || lower.contains("flash")
        });
        let has_black_fade = self.vm.active_imods.iter().any(|m| {
            let lower = m.to_ascii_lowercase();
            lower.contains("black") || lower.contains("dark")
        });

        if has_white_fade {
            self.screen_fade_color = [1.0, 1.0, 1.0, 1.0];
            if self.screen_fade_alpha > 0.0 {
                self.screen_fade_alpha = (self.screen_fade_alpha - dt * 0.2).max(0.0);
            }
        } else if has_black_fade {
            self.screen_fade_color = [0.0, 0.0, 0.0, 1.0];
            self.screen_fade_alpha = 1.0;
        } else if self.screen_fade_alpha > 0.0 {
            self.screen_fade_alpha = (self.screen_fade_alpha - dt * 1.0).max(0.0);
        }

        // クエスト通知の出力
        for notif in self.vm.quest_manager.notifications.drain(..) {
            println!("[HUD通知] {}", notif);
        }

        // プレイヤー正面の視線レイキャスト & オブジェクト検知
        // 参照元: Gamebryo 2.6 NiPick, FO3 実機インタラクト判定 (GMST fActivatePickLength = 180.0)
        let eye = self.controller.camera.eye_position();
        let forward = self.controller.camera.forward_vector();
        let prev_focus = self.focused_interactable.as_ref().map(|o| o.form_id);

        // 1. 画面中央（クロスヘア）からの物理レイキャストによる精密判定
        let ray_hit = self.controller.physics_world.cast_ray_interaction(
            eye,
            forward,
            F_ACTIVATE_PICK_LENGTH,
        );
        let mut new_focus = find_focused_by_raycast(ray_hit, &self.interactables).cloned();

        // 2. コライダーを持たない一部アイテム/NPCに対する幾何フォールバック (手前に遮蔽壁がない場合のみ)
        if new_focus.is_none() && ray_hit.is_none() {
            new_focus = find_focused_interactable(
                eye,
                forward,
                &self.interactables,
                F_ACTIVATE_PICK_LENGTH,
            )
            .cloned();
        }

        if let Some(ref focused) = new_focus {
            if prev_focus != Some(focused.form_id) {
                println!("[インタラクト検知] {}", focused.prompt_text());
            }
        }
        self.focused_interactable = new_focus;
    }
}
