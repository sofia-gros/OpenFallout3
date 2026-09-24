//! # HUD (ヘッドアップディスプレイ) モジュール
//!
//! Fallout 3 実機アーカイブ (`Fallout - Textures.bsa`) 内の本物 UI スプライト
//! (`interface/hud/crosshair.dds`, `interface/hud/glow_crosshair.dds`) をロードし、
//! 実機仕様（フォスファーグリーン `[0.18, 1.0, 0.48]`、呼吸パルス周期 3.2秒）に基づき描画する。
//!
//! 参照元:
//! - `references/bevyout/src/vsa/prepare/interface.rs:21-24`
//! - `references/bevyout/src/viewer/hud.rs:253-287`, `references/bevyout/src/viewer/fallout_ui.rs:6-12`

mod crosshair;
mod overlay;
mod pipeline;
mod types;
mod video;

pub use types::{HudUniform, HudVertex, CROSSHAIR_PULSE_SECONDS, HUD_PHOSPHOR_COLOR};

use fo3_vfs::VfsManager;

use self::pipeline::init_hud_pipelines;

/// HUD レンダラー。
pub struct HudRenderer {
    pub(crate) pipeline: wgpu::RenderPipeline,
    pub(crate) uniform_buffer: wgpu::Buffer,
    pub(crate) vertex_buffer: wgpu::Buffer,
    pub(crate) crosshair_bind_group: Option<wgpu::BindGroup>,
    pub(crate) glow_bind_group: Option<wgpu::BindGroup>,
    pub(crate) white_bind_group: wgpu::BindGroup,
    /// Bink 動画フレーム描画用パイプライン (テクスチャ RGB をそのまま出力する専用シェーダー)
    pub(crate) video_pipeline: wgpu::RenderPipeline,
    /// 動画テクスチャ用バインドグループ生成のために保持するレイアウト
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl HudRenderer {
    /// HUD レンダラーを初期化し、VFS から本物のクロスヘアテクスチャをロードする。
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        vfs: &mut VfsManager,
    ) -> Self {
        let res = init_hud_pipelines(device, queue, surface_format, vfs);
        Self {
            pipeline: res.pipeline,
            uniform_buffer: res.uniform_buffer,
            vertex_buffer: res.vertex_buffer,
            crosshair_bind_group: res.crosshair_bind_group,
            glow_bind_group: res.glow_bind_group,
            white_bind_group: res.white_bind_group,
            video_pipeline: res.video_pipeline,
            bind_group_layout: res.bind_group_layout,
        }
    }
}
