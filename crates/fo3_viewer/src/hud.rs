//! # HUD (ヘッドアップディスプレイ) モジュール
//!
//! Fallout 3 実機アーカイブ (`Fallout - Textures.bsa`) 内の本物 UI スプライト
//! (`interface/hud/crosshair.dds`, `interface/hud/glow_crosshair.dds`) をロードし、
//! 実機仕様（フォスファーグリーン `[0.18, 1.0, 0.48]`、呼吸パルス周期 3.2秒）に基づき描画する。
//!
//! 参照元:
//! - `references/bevyout/src/vsa/prepare/interface.rs:21-24`
//! - `references/bevyout/src/viewer/hud.rs:253-287`, `references/bevyout/src/viewer/fallout_ui.rs:6-12`

use std::sync::Arc;
use fo3_render::GpuTexture;
use fo3_vfs::VfsManager;

/// Fallout 3 実機 HUD のフォスファー発光色 (Phosphor Green)。
/// 参照元: `references/bevyout/src/viewer/fallout_ui.rs:6`
pub const HUD_PHOSPHOR_COLOR: [f32; 4] = [0.18, 1.0, 0.48, 1.0];

/// 呼吸パルス周期（秒単位）。
/// 参照元: `references/bevyout/src/viewer/hud.rs:37`
pub const CROSSHAIR_PULSE_SECONDS: f32 = 3.2;

/// HUD スプライト描画用頂点。
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct HudVertex {
    position: [f32; 2],
    uv: [f32; 2],
}

/// HUD スプライト描画用ユニフォーム (カラー、アルファ)。
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct HudUniform {
    color: [f32; 4],
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

/// HUD レンダラー。
pub struct HudRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    crosshair_bind_group: Option<wgpu::BindGroup>,
    glow_bind_group: Option<wgpu::BindGroup>,
    white_bind_group: wgpu::BindGroup,
}

impl HudRenderer {
    /// HUD レンダラーを初期化し、VFS から本物のクロスヘアテクスチャをロードする。
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        vfs: &mut VfsManager,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("HUD Shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
struct HudUniform {
    color: vec4<f32>,
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u_hud: HudUniform;
@group(0) @binding(1) var t_diffuse: texture_2d<f32>;
@group(0) @binding(2) var s_diffuse: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // 画面中央原点、ピクセル座標から NDC [-1, 1] への変換
    let ndc_x = (in.position.x / u_hud.screen_size.x) * 2.0;
    let ndc_y = (in.position.y / u_hud.screen_size.y) * 2.0;
    out.clip_position = vec4<f32>(ndc_x, -ndc_y, 0.0, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(t_diffuse, s_diffuse, in.uv);
    // Gamebryo HUD テクスチャはアルファチャンネルに形状が焼かれており、
    // RGB カラーを白または色乗算する仕様。
    // 参照元: references/bevyout/src/vsa/prepare/interface.rs:18-20
    let alpha = tex.a * u_hud.color.a;
    return vec4<f32>(u_hud.color.rgb, alpha);
}
"#
                .into(),
            ),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HUD Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HUD Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HUD Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<HudVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("HUD Uniform Buffer"),
            size: std::mem::size_of::<HudUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // クアッド描画用頂点バッファ (2トライアングル = 6頂点)
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("HUD Vertex Buffer"),
            size: (std::mem::size_of::<HudVertex>() * 6) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Fallout 3 実機アーカイブ (Fallout - Textures.bsa) からクロスヘアテクスチャをロード
        // 参照元: textures\interface\hud\crosshair.dds, textures\interface\hud\glow_crosshair.dds
        let crosshair_tex = load_hud_texture(device, queue, vfs, "textures\\interface\\hud\\crosshair.dds");
        let glow_tex = load_hud_texture(device, queue, vfs, "textures\\interface\\hud\\glow_crosshair.dds");

        let crosshair_bind_group = crosshair_tex.as_ref().map(|tex| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Crosshair Bind Group"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&tex.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&tex.sampler),
                    },
                ],
            })
        });

        let glow_bind_group = glow_tex.as_ref().map(|tex| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Glow Bind Group"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&tex.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&tex.sampler),
                    },
                ],
            })
        });

        // 3. 単色描画用白テクスチャのバインドグループ
        let default_white = GpuTexture::create_default_white(device, queue);
        let white_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HUD White Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&default_white.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&default_white.sampler),
                },
            ],
        });

        Self {
            pipeline,
            uniform_buffer,
            vertex_buffer,
            crosshair_bind_group,
            glow_bind_group,
            white_bind_group,
        }
    }

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
        let pulse = (elapsed_seconds * std::f32::consts::TAU / CROSSHAIR_PULSE_SECONDS).sin() * 0.5 + 0.5;
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
                HudVertex { position: [-half, -half], uv: [0.0, 0.0] },
                HudVertex { position: [half, -half], uv: [1.0, 0.0] },
                HudVertex { position: [half, half], uv: [1.0, 1.0] },
                HudVertex { position: [-half, -half], uv: [0.0, 0.0] },
                HudVertex { position: [half, half], uv: [1.0, 1.0] },
                HudVertex { position: [-half, half], uv: [0.0, 1.0] },
            ];
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));

            let uniform = HudUniform {
                color: [HUD_PHOSPHOR_COLOR[0], HUD_PHOSPHOR_COLOR[1], HUD_PHOSPHOR_COLOR[2], glow_alpha],
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
                HudVertex { position: [-half, -half], uv: [0.0, 0.0] },
                HudVertex { position: [half, -half], uv: [1.0, 0.0] },
                HudVertex { position: [half, half], uv: [1.0, 1.0] },
                HudVertex { position: [-half, -half], uv: [0.0, 0.0] },
                HudVertex { position: [half, half], uv: [1.0, 1.0] },
                HudVertex { position: [-half, half], uv: [0.0, 1.0] },
            ];
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));

            let uniform = HudUniform {
                color: [HUD_PHOSPHOR_COLOR[0], HUD_PHOSPHOR_COLOR[1], HUD_PHOSPHOR_COLOR[2], 0.95],
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
            HudVertex { position: [left, top], uv: [0.0, 0.0] },
            HudVertex { position: [right, top], uv: [1.0, 0.0] },
            HudVertex { position: [right, bottom], uv: [1.0, 1.0] },
            HudVertex { position: [left, top], uv: [0.0, 0.0] },
            HudVertex { position: [right, bottom], uv: [1.0, 1.0] },
            HudVertex { position: [left, bottom], uv: [0.0, 1.0] },
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

    /// NPC 会話ダイアログのフレームおよび選択肢オーバーレイを描画する。
    pub fn render_dialog_overlay<'rpass>(
        &'rpass self,
        rpass: &mut wgpu::RenderPass<'rpass>,
        queue: &wgpu::Queue,
        screen_width: f32,
        screen_height: f32,
        dialog: &crate::ui::DialogState,
    ) {
        let box_w = screen_width * 0.85;
        let box_h = screen_height * 0.38;
        let center_y = screen_height * 0.28;
        self.render_rect(rpass, queue, 0.0, center_y, box_w, box_h, [0.0, 0.06, 0.02, 0.88], screen_width, screen_height);

        let border_color = HUD_PHOSPHOR_COLOR;
        self.render_rect(rpass, queue, 0.0, center_y - box_h * 0.5, box_w, 2.0, border_color, screen_width, screen_height);
        self.render_rect(rpass, queue, 0.0, center_y + box_h * 0.5, box_w, 2.0, border_color, screen_width, screen_height);
        self.render_rect(rpass, queue, -box_w * 0.5, center_y, 2.0, box_h, border_color, screen_width, screen_height);
        self.render_rect(rpass, queue, box_w * 0.5, center_y, 2.0, box_h, border_color, screen_width, screen_height);

        let item_h = 24.0f32;
        let sel_y = center_y - box_h * 0.1 + (dialog.selected_index as f32) * (item_h + 4.0);
        if sel_y < center_y + box_h * 0.45 {
            self.render_rect(rpass, queue, 0.0, sel_y, box_w * 0.96, item_h, [0.18, 1.0, 0.48, 0.25], screen_width, screen_height);
        }
    }

    /// ターミナル画面の全画面 CRT レトロオーバーレイを描画する。
    pub fn render_terminal_overlay<'rpass>(
        &'rpass self,
        rpass: &mut wgpu::RenderPass<'rpass>,
        queue: &wgpu::Queue,
        screen_width: f32,
        screen_height: f32,
        term: &crate::ui::TerminalState,
    ) {
        self.render_rect(rpass, queue, 0.0, 0.0, screen_width, screen_height, [0.0, 0.04, 0.015, 0.94], screen_width, screen_height);

        let frame_w = screen_width * 0.92;
        let frame_h = screen_height * 0.90;
        let border_color = HUD_PHOSPHOR_COLOR;
        self.render_rect(rpass, queue, 0.0, -frame_h * 0.5, frame_w, 3.0, border_color, screen_width, screen_height);
        self.render_rect(rpass, queue, 0.0, frame_h * 0.5, frame_w, 3.0, border_color, screen_width, screen_height);
        self.render_rect(rpass, queue, -frame_w * 0.5, 0.0, 3.0, frame_h, border_color, screen_width, screen_height);
        self.render_rect(rpass, queue, frame_w * 0.5, 0.0, 3.0, frame_h, border_color, screen_width, screen_height);

        let header_y = -frame_h * 0.35;
        self.render_rect(rpass, queue, 0.0, header_y, frame_w * 0.95, 2.0, [0.18, 1.0, 0.48, 0.6], screen_width, screen_height);

        if matches!(term.screen, crate::ui::TerminalScreen::Menu) {
            let item_h = 28.0f32;
            let sel_y = header_y + 50.0 + (term.selected_index as f32) * (item_h + 6.0);
            if sel_y < frame_h * 0.45 {
                self.render_rect(rpass, queue, 0.0, sel_y, frame_w * 0.92, item_h, [0.18, 1.0, 0.48, 0.30], screen_width, screen_height);
            }
        }
    }
}

fn load_hud_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    vfs: &mut VfsManager,
    path: &str,
) -> Option<Arc<GpuTexture>> {
    println!("HUD 実機アセットを VFS からロード中: {}", path);
    match vfs.read(path) {
        Ok(bytes) => match GpuTexture::from_dds_bytes(device, queue, &bytes, Some(path)) {
            Ok(tex) => {
                println!("  - HUD テクスチャロード成功: {}", path);
                Some(Arc::new(tex))
            }
            Err(e) => {
                eprintln!("警告: HUD テクスチャ \"{}\" の GPU 転送失敗: {}", path, e);
                None
            }
        },
        Err(e) => {
            eprintln!("警告: HUD テクスチャ \"{}\" の VFS 読み出し失敗: {}", path, e);
            None
        }
    }
}
