//! HUD レンダラーのパイプライン・シェーダーおよびバインディング初期化モジュール。
//!
//! 参照元: Gamebryo 2.6 UI レンダリング, `references/bevyout/src/vsa/prepare/interface.rs:18-24`

use std::sync::Arc;

use fo3_render::GpuTexture;
use fo3_vfs::VfsManager;

use super::types::{HudUniform, HudVertex};

/// 初期化された HUD GPU リソース群。
pub struct HudPipelineResources {
    pub pipeline: wgpu::RenderPipeline,
    pub uniform_buffer: wgpu::Buffer,
    pub vertex_buffer: wgpu::Buffer,
    pub crosshair_bind_group: Option<wgpu::BindGroup>,
    pub glow_bind_group: Option<wgpu::BindGroup>,
    pub white_bind_group: wgpu::BindGroup,
    pub video_pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

/// HUD レンダラーのパイプラインおよびテクスチャ・バインディングを構築する。
pub fn init_hud_pipelines(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    surface_format: wgpu::TextureFormat,
    vfs: &mut VfsManager,
) -> HudPipelineResources {
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
    let ndc_x = (in.position.x / u_hud.screen_size.x) * 2.0;
    let ndc_y = (in.position.y / u_hud.screen_size.y) * 2.0;
    out.clip_position = vec4<f32>(ndc_x, -ndc_y, 0.0, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(t_diffuse, s_diffuse, in.uv);
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

    let video_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Bink Video Shader"),
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
    let ndc_x = (in.position.x / u_hud.screen_size.x) * 2.0;
    let ndc_y = (in.position.y / u_hud.screen_size.y) * 2.0;
    out.clip_position = vec4<f32>(ndc_x, -ndc_y, 0.0, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_video_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(t_diffuse, s_diffuse, in.uv);
    return vec4<f32>(tex.rgb * u_hud.color.rgb, 1.0);
}
"#
            .into(),
        ),
    });

    let video_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Bink Video Render Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &video_shader,
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
            module: &video_shader,
            entry_point: Some("fs_video_main"),
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

    let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("HUD Vertex Buffer"),
        size: (std::mem::size_of::<HudVertex>() * 6) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let crosshair_tex = load_hud_texture(
        device,
        queue,
        vfs,
        "textures\\interface\\hud\\crosshair.dds",
    );
    let glow_tex = load_hud_texture(
        device,
        queue,
        vfs,
        "textures\\interface\\hud\\glow_crosshair.dds",
    );

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

    HudPipelineResources {
        pipeline,
        uniform_buffer,
        vertex_buffer,
        crosshair_bind_group,
        glow_bind_group,
        white_bind_group,
        video_pipeline,
        bind_group_layout,
    }
}

pub fn load_hud_texture(
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
            eprintln!(
                "警告: HUD テクスチャ \"{}\" の VFS 読み出し失敗: {}",
                path, e
            );
            None
        }
    }
}
