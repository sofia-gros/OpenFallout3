//! Bink 動画フレームの HUD レンダリングおよびバインドグループ生成モジュール。
//!
//! 参照元: Gamebryo 2.6 BinkVideo — ゲームウィンドウ内描画仕様

use super::types::{HudUniform, HudVertex};
use super::HudRenderer;

impl HudRenderer {
    /// Bink 動画フレームテクスチャをゲームウィンドウ全面に描画する。
    ///
    /// `video_bind_group` は `HudRenderer::create_video_bind_group()` で作成したもの。
    /// カラーは `[1.0, 1.0, 1.0, 1.0]`（テクスチャの色をそのまま表示）。
    ///
    /// 参照元: Gamebryo 2.6 BinkVideo — ゲームウィンドウ内描画仕様
    pub fn render_video_frame<'rpass>(
        &'rpass self,
        rpass: &mut wgpu::RenderPass<'rpass>,
        queue: &wgpu::Queue,
        video_bind_group: &'rpass wgpu::BindGroup,
        screen_width: f32,
        screen_height: f32,
    ) {
        let half_w = screen_width * 0.5;
        let half_h = screen_height * 0.5;
        let vertices = [
            HudVertex {
                position: [0.0 - half_w, 0.0 - half_h],
                uv: [0.0, 0.0],
            },
            HudVertex {
                position: [0.0 + half_w, 0.0 - half_h],
                uv: [1.0, 0.0],
            },
            HudVertex {
                position: [0.0 + half_w, 0.0 + half_h],
                uv: [1.0, 1.0],
            },
            HudVertex {
                position: [0.0 - half_w, 0.0 - half_h],
                uv: [0.0, 0.0],
            },
            HudVertex {
                position: [0.0 + half_w, 0.0 + half_h],
                uv: [1.0, 1.0],
            },
            HudVertex {
                position: [0.0 - half_w, 0.0 + half_h],
                uv: [0.0, 1.0],
            },
        ];
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));

        // カラーは白 (1,1,1,1) → テクスチャ色をそのまま出力
        let uniform = HudUniform {
            color: [1.0, 1.0, 1.0, 1.0],
            screen_size: [screen_width, screen_height],
            _pad: [0.0, 0.0],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniform]));

        rpass.set_pipeline(&self.video_pipeline);
        rpass.set_bind_group(0, video_bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.draw(0..6, 0..1);
    }

    /// 動画テクスチャ用のバインドグループを生成する。
    ///
    /// `BinkPlayer` が持つ `texture_view` とサンプラーを HUD パイプラインに結合する。
    /// `render_video_frame()` に渡して動画フレームを描画する。
    pub fn create_video_bind_group(
        &self,
        device: &wgpu::Device,
        texture_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("BinkPlayer Video Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BinkPlayer Video Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        })
    }
}
