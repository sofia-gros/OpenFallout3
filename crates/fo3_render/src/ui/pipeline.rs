//! # 2D UI レンダリングパイプライン
//!
//! スクリーン空間ピクセル座標によるテキストおよび矩形の最前面オーバーレイ描画。
//! 参照元: `Fallout - Misc.bsa` (`menus\dialog\dialog_menu.xml`)

use super::font::{BitmapFont, TextBatch, UiVertex};
use wgpu::util::DeviceExt;

/// UI 画面解像度 Uniform。
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiResolutionUniform {
    pub screen_size: [f32; 2],
    pub _padding: [f32; 2],
}

/// 2D UI レンダラ
pub struct UiRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform_bind_group_layout: wgpu::BindGroupLayout,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    uniform_bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,

    // Texture caches
    font_bind_group: wgpu::BindGroup,
    solid_color_bind_group: wgpu::BindGroup,
    dynamic_bind_groups: std::collections::HashMap<String, wgpu::BindGroup>,
    default_sampler: wgpu::Sampler,

    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    index_count: u32,
    commands: Vec<crate::ui::font::DrawCommand>,
    font: BitmapFont,
}

impl UiRenderer {
    /// 新規 UI レンダラを生成
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let font = BitmapFont::create_embedded_fallback();

        // フォントテクスチャ生成
        let font_texture_size = wgpu::Extent3d {
            width: font.texture_width,
            height: font.texture_height,
            depth_or_array_layers: 1,
        };
        let font_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("UI Font Texture"),
            size: font_texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &font_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &font.texture_rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(font.texture_width * 4),
                rows_per_image: Some(font.texture_height),
            },
            font_texture_size,
        );
        let font_view = font_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // 単色テクスチャ生成 (SolidColor)
        let solid_color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("UI Solid Color Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &solid_color_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let solid_color_view =
            solid_color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let default_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("UI Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // 共通 Uniform バッファ
        let resolution = UiResolutionUniform {
            screen_size: [1280.0, 720.0],
            _padding: [0.0, 0.0],
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("UI Resolution Buffer"),
            contents: bytemuck::cast_slice(&[resolution]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Group 0: Uniform (Resolution)
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("UI Uniform Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("UI Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Group 1: Texture
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("UI Texture Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let font_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("UI Font Texture Bind Group"),
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&font_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&default_sampler),
                },
            ],
        });

        let solid_color_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("UI Solid Color Texture Bind Group"),
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&solid_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&default_sampler),
                },
            ],
        });

        let shader_source = r#"
struct ResolutionUniform {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u_res: ResolutionUniform;
@group(1) @binding(0) var t_tex: texture_2d<f32>;
@group(1) @binding(1) var s_tex: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // 画面サイズで NDC (-1..1, 1..-1) 変換
    let x = (model.position.x / u_res.screen_size.x) * 2.0 - 1.0;
    let y = 1.0 - (model.position.y / u_res.screen_size.y) * 2.0;
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.tex_coord = model.tex_coord;
    out.color = model.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_tex, s_tex, in.tex_coord);
    return in.color * tex_color;
}
"#;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UI Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("UI Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UI Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[UiVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            uniform_bind_group_layout,
            texture_bind_group_layout,
            uniform_bind_group,
            uniform_buffer,
            font_bind_group,
            solid_color_bind_group,
            dynamic_bind_groups: std::collections::HashMap::new(),
            default_sampler,
            vertex_buffer: None,
            index_buffer: None,
            index_count: 0,
            commands: Vec::new(),
            font,
        }
    }

    /// フォント参照を取得。
    pub fn font(&self) -> &BitmapFont {
        &self.font
    }

    /// 画面サイズを更新。
    pub fn update_resolution(&self, queue: &wgpu::Queue, width: f32, height: f32) {
        let uniform = UiResolutionUniform {
            screen_size: [width, height],
            _padding: [0.0, 0.0],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniform]));
    }

    /// 頂点データを GPU バッファへ転送し、必要なテクスチャをロード
    pub fn upload_batch(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        batch: &TextBatch,
        vfs: &mut fo3_vfs::VfsManager,
    ) {
        self.index_count = batch.indices.len() as u32;
        self.commands = batch.commands.clone();

        if self.index_count == 0 {
            return;
        }

        // 必要なテクスチャをロード
        for cmd in &self.commands {
            if let crate::ui::font::UiTexture::Image(ref path) = cmd.texture {
                if !self.dynamic_bind_groups.contains_key(path) {
                    if let Ok(bytes) = vfs.read(path) {
                        if let Ok(gpu_tex) = crate::texture::GpuTexture::from_dds_bytes(
                            device,
                            queue,
                            &bytes,
                            Some(path),
                        ) {
                            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                                label: Some(path),
                                layout: &self.texture_bind_group_layout,
                                entries: &[
                                    wgpu::BindGroupEntry {
                                        binding: 0,
                                        resource: wgpu::BindingResource::TextureView(&gpu_tex.view),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 1,
                                        resource: wgpu::BindingResource::Sampler(
                                            &self.default_sampler,
                                        ),
                                    },
                                ],
                            });
                            self.dynamic_bind_groups.insert(path.clone(), bind_group);
                        } else {
                            println!("Failed to parse DDS for UI: {}", path);
                        }
                    } else {
                        println!("Failed to load UI texture: {}", path);
                    }
                }
            }
        }

        self.vertex_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("UI Vertex Buffer"),
                contents: bytemuck::cast_slice(&batch.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );

        self.index_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("UI Index Buffer"),
                contents: bytemuck::cast_slice(&batch.indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
        );
    }

    /// UI バッチを描画
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if self.index_count == 0 {
            return;
        }
        if let (Some(vb), Some(ib)) = (&self.vertex_buffer, &self.index_buffer) {
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, vb.slice(..));
            render_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);

            for cmd in &self.commands {
                let bind_group = match &cmd.texture {
                    crate::ui::font::UiTexture::SolidColor => &self.solid_color_bind_group,
                    crate::ui::font::UiTexture::Font => &self.font_bind_group,
                    crate::ui::font::UiTexture::Image(path) => self
                        .dynamic_bind_groups
                        .get(path)
                        .unwrap_or(&self.solid_color_bind_group),
                };
                render_pass.set_bind_group(1, bind_group, &[]);
                render_pass.draw_indexed(
                    cmd.index_start..(cmd.index_start + cmd.index_count),
                    0,
                    0..1,
                );
            }
        }
    }
}
