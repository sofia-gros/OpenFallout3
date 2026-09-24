//! ビューアーのフレーム描画パイプライン (`ViewerState::render`)。
//!
//! 参照元: Gamebryo 2.6 レンダリングパイプライン & wgpu パス

use crate::ui::ViewerMode;
use super::ViewerState;

impl ViewerState {
    /// 3D シーン、コリジョン、HUD、全画面フェード、2D UI、字幕、メッセージボックスの描画を行う。
    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            self.scene.render_with_camera_pos(
                &mut render_pass,
                &self.context,
                Some(self.controller.camera.eye_position()),
            );

            if self.show_collision {
                self.scene.render_collision(&mut render_pass, &self.context);
            }

            // Fallout 3 実機 HUD / UI オーバーレイ描画
            let elapsed = self.start_time.elapsed().as_secs_f32();
            if matches!(self.mode, ViewerMode::Exploring) && self.screen_fade_alpha < 0.99 {
                self.hud.render_crosshair(
                    &mut render_pass,
                    &self.queue,
                    self.size.width as f32,
                    self.size.height as f32,
                    elapsed,
                );
            }

            // 全画面フェードエフェクト (暗転・ホワイトアウト・視界開放)
            if self.screen_fade_alpha > 0.001 {
                self.hud.render_rect(
                    &mut render_pass,
                    &self.queue,
                    0.0,
                    0.0,
                    self.size.width as f32,
                    self.size.height as f32,
                    [
                        self.screen_fade_color[0],
                        self.screen_fade_color[1],
                        self.screen_fade_color[2],
                        self.screen_fade_alpha,
                    ],
                    self.size.width as f32,
                    self.size.height as f32,
                );
            }

            // Bink ムービー再生中: 動画フレームテクスチャをゲームウィンドウ全面に描画する。
            // 参照元: Gamebryo 2.6 BinkVideo — ゲームウィンドウ内描画仕様
            // 3D シーン描画の上に重ねて表示し、スクリーンフェードの下に位置する。
            if let Some(ref bg) = self.bink_video_bind_group {
                self.hud.render_video_frame(
                    &mut render_pass,
                    &self.queue,
                    bg,
                    self.size.width as f32,
                    self.size.height as f32,
                );
            }
        }

        // Phase 9: 最前面 2D UI (会話テキスト・選択肢・ターミナル画面) の描画パス
        let mut ui_batch = fo3_render::TextBatch::default();
        self.mode.populate_batch(
            &mut ui_batch,
            self.ui_renderer.font(),
            self.size.width as f32,
            self.size.height as f32,
        );

        // RTT Render Pass
        if !ui_batch.indices.is_empty() && !self.active_rtt_bindings.is_empty() {
            self.ui_renderer
                .update_resolution(&self.queue, 1024.0, 1024.0);
            self.ui_renderer
                .upload_batch(&self.device, &self.queue, &ui_batch, &mut self.vfs);

            let mut rtt_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("RTT Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.ui_rtt.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.ui_renderer.render(&mut rtt_pass);

            // RTT用に構築したので一旦クリア
            ui_batch.clear();
        }

        // Draw active subtitles
        let mut y_offset = self.size.height as f32 - 100.0;
        for (sub, _) in self.sound_engine.active_subtitles.values() {
            let subtitle_text = format!("{}: {}", sub.speaker, sub.text);
            let screen_w = self.size.width as f32;

            ui_batch.add_text(
                self.ui_renderer.font(),
                &subtitle_text,
                screen_w * 0.1,
                y_offset,
                1.2,
                [0.2, 1.0, 0.4, 1.0],
            );
            y_offset -= 30.0;
        }

        // 実機メッセージメニュー (MESG / ShowMessage) の描画
        if let Some(msg_id) = self.vm.show_messages.first() {
            let mesg_opt = self
                .master_context
                .mesg_edid_map
                .get(&msg_id.to_ascii_uppercase())
                .and_then(|fid| self.master_context.mesg_map.get(fid))
                .or_else(|| {
                    let hex_str = msg_id.trim_start_matches("0x").trim_start_matches("0X");
                    if let Ok(val) = u32::from_str_radix(hex_str, 16) {
                        self.master_context.mesg_map.get(&fo3_esm::FormId(val))
                    } else {
                        None
                    }
                });

            if let Some(mesg) = mesg_opt {
                let screen_w = self.size.width as f32;
                let screen_h = self.size.height as f32;
                let box_w = 460.0;
                let box_h = 140.0 + (mesg.buttons.len() as f32 * 36.0);
                let bx = (screen_w - box_w) * 0.5;
                let by = (screen_h - box_h) * 0.5;

                // Pip-Boy ウィンドウ背景 & 外枠
                ui_batch.add_rect(bx, by, box_w, box_h, [0.02, 0.08, 0.03, 0.92]);
                ui_batch.add_rect(bx, by, box_w, 2.0, [0.2, 1.0, 0.4, 1.0]);
                ui_batch.add_rect(bx, by + box_h - 2.0, box_w, 2.0, [0.2, 1.0, 0.4, 1.0]);
                ui_batch.add_rect(bx, by, 2.0, box_h, [0.2, 1.0, 0.4, 1.0]);
                ui_batch.add_rect(bx + box_w - 2.0, by, 2.0, box_h, [0.2, 1.0, 0.4, 1.0]);

                let mut cy = by + 25.0;
                if !mesg.text.is_empty() {
                    ui_batch.add_text(
                        self.ui_renderer.font(),
                        &mesg.text,
                        bx + 30.0,
                        cy,
                        1.2,
                        [1.0, 0.9, 0.2, 1.0],
                    );
                    cy += 45.0;
                }

                for (idx, btn_text) in mesg.buttons.iter().enumerate() {
                    let btn_y = cy;
                    ui_batch.add_rect(
                        bx + 30.0,
                        btn_y - 2.0,
                        box_w - 60.0,
                        28.0,
                        [0.05, 0.18, 0.08, 0.85],
                    );
                    let label = format!("  [{}] {}", idx + 1, btn_text);
                    ui_batch.add_text(
                        self.ui_renderer.font(),
                        &label,
                        bx + 35.0,
                        btn_y + 3.0,
                        1.15,
                        [0.2, 1.0, 0.4, 1.0],
                    );
                    cy += 34.0;
                }
            }
        }

        // キャラクター作成画面 (RaceSexMenu / NameMenu) の描画
        let screen_w = self.size.width as f32;
        let screen_h = self.size.height as f32;
        self.chargen_menu
            .render(&self.ui_renderer, &mut ui_batch, screen_w, screen_h);

        // 通常の画面UI描画 (RTTに描画しなかったもの、またはRTT描画後のメッセージ等)
        if !ui_batch.indices.is_empty() {
            self.ui_renderer.update_resolution(
                &self.queue,
                self.size.width as f32,
                self.size.height as f32,
            );
            self.ui_renderer
                .upload_batch(&self.device, &self.queue, &ui_batch, &mut self.vfs);

            let mut ui_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("2D UI Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.ui_renderer.render(&mut ui_pass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
