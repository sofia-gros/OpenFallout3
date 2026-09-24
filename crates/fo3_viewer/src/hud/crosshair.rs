//! クロスヘアおよび基本矩形スプライト描画モジュール。
//!
//! 参照元: `references/bevyout/src/viewer/hud.rs:253-287`, `815-824`

use super::types::{HudUniform, HudVertex, CROSSHAIR_PULSE_SECONDS, HUD_PHOSPHOR_COLOR};
use super::HudRenderer;

impl HudRenderer {
    /// 画面中央にクロスヘアおよびグローを描画する。
    /// 参照元: `references/bevyout/src/viewer/hud.rs:253-287`, `815-824`
    pub fn render_crosshair<'rpass>(
        &'rpass self,
        rpass: &mut wgpu::RenderPass<'rpass>,
        queue: &wgpu::Queue,
        screen_width: f32,
        screen_height: f32,
        elapsed_seconds: f32,
    ) {
        if screen_width <= 0.0 || screen_height <= 0.0 {
            return;
        }

        // 呼吸パルスのアルファ計算 (0.65 .. 0.95)
        // 参照元: references/bevyout/src/viewer/hud.rs:815-824
        let pulse =
            (elapsed_seconds * std::f32::consts::TAU / CROSSHAIR_PULSE_SECONDS).sin() * 0.5 + 0.5;
        let glow_alpha = 0.50 + pulse * 0.35;

        // 画面高さに対する相対サイズ (vh 基準)
        // 実機比率: コア 1.7vh (~24px @ 1440p), グロー 3.4vh (~48px @ 1440p)
        // 参照元: references/bevyout/src/viewer/hud.rs:254-263
        let core_size = (screen_height * 0.024).clamp(16.0, 32.0);
        let glow_size = core_size * 2.0;

        rpass.set_pipeline(&self.pipeline);

        // 1. グローの描画
        if let Some(ref bg) = self.glow_bind_group {
            let half = glow_size * 0.5;
            let vertices = [
                HudVertex {
                    position: [-half, -half],
                    uv: [0.0, 0.0],
                },
                HudVertex {
                    position: [half, -half],
                    uv: [1.0, 0.0],
                },
                HudVertex {
                    position: [half, half],
                    uv: [1.0, 1.0],
                },
                HudVertex {
                    position: [-half, -half],
                    uv: [0.0, 0.0],
                },
                HudVertex {
                    position: [half, half],
                    uv: [1.0, 1.0],
                },
                HudVertex {
                    position: [-half, half],
                    uv: [0.0, 1.0],
                },
            ];
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));

            let uniform = HudUniform {
                color: [
                    HUD_PHOSPHOR_COLOR[0],
                    HUD_PHOSPHOR_COLOR[1],
                    HUD_PHOSPHOR_COLOR[2],
                    glow_alpha,
                ],
                screen_size: [screen_width, screen_height],
                _pad: [0.0, 0.0],
            };
            queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniform]));

            rpass.set_bind_group(0, bg, &[]);
            rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            rpass.draw(0..6, 0..1);
        }

        // 2. コアクロスヘアの描画
        if let Some(ref bg) = self.crosshair_bind_group {
            let half = core_size * 0.5;
            let vertices = [
                HudVertex {
                    position: [-half, -half],
                    uv: [0.0, 0.0],
                },
                HudVertex {
                    position: [half, -half],
                    uv: [1.0, 0.0],
                },
                HudVertex {
                    position: [half, half],
                    uv: [1.0, 1.0],
                },
                HudVertex {
                    position: [-half, -half],
                    uv: [0.0, 0.0],
                },
                HudVertex {
                    position: [half, half],
                    uv: [1.0, 1.0],
                },
                HudVertex {
                    position: [-half, half],
                    uv: [0.0, 1.0],
                },
            ];
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));

            let uniform = HudUniform {
                color: [
                    HUD_PHOSPHOR_COLOR[0],
                    HUD_PHOSPHOR_COLOR[1],
                    HUD_PHOSPHOR_COLOR[2],
                    0.95,
                ],
                screen_size: [screen_width, screen_height],
                _pad: [0.0, 0.0],
            };
            queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniform]));

            rpass.set_bind_group(0, bg, &[]);
            rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            rpass.draw(0..6, 0..1);
        }
    }

    /// 画面上に長方形（ボックスまたは枠線）を描画する。
    pub fn render_rect<'rpass>(
        &'rpass self,
        rpass: &mut wgpu::RenderPass<'rpass>,
        queue: &wgpu::Queue,
        center_x: f32,
        center_y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        screen_width: f32,
        screen_height: f32,
    ) {
        let half_w = width * 0.5;
        let half_h = height * 0.5;
        let left = center_x - half_w;
        let right = center_x + half_w;
        let top = center_y - half_h;
        let bottom = center_y + half_h;

        let vertices = [
            HudVertex {
                position: [left, top],
                uv: [0.0, 0.0],
            },
            HudVertex {
                position: [right, top],
                uv: [1.0, 0.0],
            },
            HudVertex {
                position: [right, bottom],
                uv: [1.0, 1.0],
            },
            HudVertex {
                position: [left, top],
                uv: [0.0, 0.0],
            },
            HudVertex {
                position: [right, bottom],
                uv: [1.0, 1.0],
            },
            HudVertex {
                position: [left, bottom],
                uv: [0.0, 1.0],
            },
        ];
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));

        let uniform = HudUniform {
            color,
            screen_size: [screen_width, screen_height],
            _pad: [0.0, 0.0],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniform]));

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.white_bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.draw(0..6, 0..1);
    }
}
