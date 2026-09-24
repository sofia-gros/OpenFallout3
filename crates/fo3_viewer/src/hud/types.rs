//! HUD 関連の定数および頂点・Uniform 構造体定義。
//!
//! 参照元: `references/bevyout/src/viewer/hud.rs:37`, `references/bevyout/src/viewer/fallout_ui.rs:6-12`

/// Fallout 3 実機 HUD のフォスファー発光色 (Phosphor Green)。
/// 参照元: `references/bevyout/src/viewer/fallout_ui.rs:6`
pub const HUD_PHOSPHOR_COLOR: [f32; 4] = [0.18, 1.0, 0.48, 1.0];

/// 呼吸パルス周期（秒単位）。
/// 参照元: `references/bevyout/src/viewer/hud.rs:37`
pub const CROSSHAIR_PULSE_SECONDS: f32 = 3.2;

/// HUD スプライト描画用頂点。
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HudVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
}

/// HUD スプライト描画用ユニフォーム (カラー、アルファ)。
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HudUniform {
    pub color: [f32; 4],
    pub screen_size: [f32; 2],
    pub _pad: [f32; 2],
}
