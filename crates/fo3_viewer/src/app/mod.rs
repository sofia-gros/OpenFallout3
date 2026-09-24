//! ビューアーのメインアプリケーションループおよび GPU レンダリング状態。
//!
//! 参照元: Gamebryo 2.6 レンダリングパイプライン & winit イベントループ

mod handler;
mod init;
mod render;
mod state;
mod update;

pub use handler::App;
pub use crate::ui::ViewerMode;

/// `window_input.rs` などが `crate::app::AppState` としてインポートするための型エイリアス。
pub type AppState = ViewerState;

use fo3_esm::{CellLighting, EsmMasterContext};
use fo3_render::{
    GpuTexture, NifCache, PlacedPointLight, RenderContext, RenderScene,
};
use fo3_vfs::VfsManager;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::anim::AnimState;
use crate::controller::Controller;
use crate::hud::HudRenderer;
use crate::interact::InteractableObject;
use crate::interactive_anim::{InteractiveAnimator, RefrBinding};
use crate::inventory::PlayerInventory;

/// ビューアーのメイン実行状態および GPU リソース保持構造体。
pub struct ViewerState {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: PhysicalSize<u32>,
    pub context: RenderContext,
    pub camera_buffer: wgpu::Buffer,
    pub mode: ViewerMode,
    pub camera_bind_group: wgpu::BindGroup,
    pub lighting_buffer: wgpu::Buffer,
    pub cell_lighting: Option<CellLighting>,
    pub placed_lights: Vec<PlacedPointLight>,
    pub clear_color: wgpu::Color,
    pub depth_view: wgpu::TextureView,
    pub scene: RenderScene,
    pub streamer: Option<crate::streamer::WorldStreamer>,
    pub show_collision: bool,
    pub enable_fog: bool,
    pub headlight: bool,
    pub controller: Controller,
    pub anim: AnimState,
    pub hud: HudRenderer,
    pub interactables: Vec<InteractableObject>,
    pub focused_interactable: Option<InteractableObject>,
    pub markers: HashMap<String, (glam::Vec3, glam::Vec3)>,
    pub refr_bindings: HashMap<u32, RefrBinding>,
    pub animators: HashMap<u32, InteractiveAnimator>,
    pub inventory: PlayerInventory,
    pub data_dir: String,
    pub start_time: Instant,
    pub vfs: VfsManager,
    pub ui_rtt: fo3_render::GpuTexture,
    pub active_rtt_bindings: Vec<(usize, wgpu::BindGroup)>,
    pub master_context: Arc<EsmMasterContext>,
    pub nif_cache: NifCache,
    pub texture_cache: HashMap<String, GpuTexture>,
    pub vm: fo3_script::ScriptVm,
    pub ai: crate::ai::AiManager,
    pub nav_graph: fo3_navigation::NavGraph,
    pub dispatcher: fo3_script::EventDispatcher,
    pub ui_renderer: fo3_render::UiRenderer,
    /// キーバインド・入力管理マネージャー (Fallout 3 実機標準 + F1-F12 デバッグ)
    pub input_manager: crate::input::InputManager,
    /// 全画面フェードエフェクト色 (暗転・ホワイトアウト用)
    pub screen_fade_color: [f32; 4],
    /// 全画面フェードエフェクトの不透明度 (0.0=透明, 1.0=完全不透明)
    pub screen_fade_alpha: f32,
    /// オーディオ再生及びダイアログ・字幕進行管理
    pub sound_engine: crate::audio::SoundEngine,
    /// キャラクター作成画面 (RaceSexMenu / NameMenu)
    pub chargen_menu: crate::chargen_menu::ChargenMenu,
    /// ゲームウィンドウ内 Bink ムービープレイヤー (再生中のみ Some)
    pub bink_player: Option<crate::bink_player::BinkPlayer>,
    /// Bink テクスチャ用バインドグループキャッシュ (bink_player が Some の間保持)
    pub bink_video_bind_group: Option<wgpu::BindGroup>,
    /// レンダリングフレーム総数
    pub frame_count: u64,
}
