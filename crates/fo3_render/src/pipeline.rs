//! # wgpu レンダーパイプライン
//!
//! シェーダー、バインドグループレイアウト、ブレンド設定、深度テストの初期化および管理。

use bytemuck::{Pod, Zeroable};
use crate::vertex::Vertex;

/// モデル、マテリアル、発光およびアルファテスト用 Uniform バッファ構造体 (112 バイト)。
///
/// 参照元:
/// - `references/nifskope/build/nif.xml:L1518` (`AlphaFlags`), `references/nifskope/src/gl/glproperty.cpp:L204`
/// - `references/nifxml/nif.xml:L4363` (`NiMaterialProperty`)
/// - `references/nifxml/nif.xml:L6307` (`BSShaderTextureSet`)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ModelUniform {
    /// ワールド変換行列 (4x4, 64バイト)
    pub world: [f32; 16],
    /// スペキュラ反射色 (RGB) および光沢度指数 (W: glossiness, 16バイト)
    pub specular_color: [f32; 4],
    /// 自己発光色 (RGB) および発光乗算係数 (W: emissive_mult, 16バイト)
    pub emissive_color: [f32; 4],
    /// ティント色 (RGB) および乗算有効度 (W: 1.0=有効, 16バイト)
    /// 髪色 (HCLR) や瞳色等の乗算カラー。デフォルトは [1.0, 1.0, 1.0, 1.0]
    pub tint_color: [f32; 4],
    /// アルファテスト有効化フラグ (0: 無効, 1: 有効)
    pub alpha_test: u32,
    /// アルファテスト比較関数 (0..7: TestFunction)
    pub alpha_test_func: u32,
    /// アルファテスト閾値 (0.0 .. 1.0)
    pub alpha_threshold: f32,
    /// グローマップ有効化フラグ (0: 無効, 1: 有効)
    pub has_glow_map: u32,
}

impl ModelUniform {
    /// ワールド変換行列、NiAlphaProperty、NiMaterialProperty から ModelUniform を構築する（ティントなし）。
    pub fn new(
        world_mat: glam::Mat4,
        alpha_prop: Option<&fo3_nif::NiAlphaProperty>,
        material_prop: Option<&fo3_nif::NiMaterialProperty>,
        has_glow_map: bool,
    ) -> Self {
        Self::new_with_tint(world_mat, alpha_prop, material_prop, has_glow_map, [1.0, 1.0, 1.0, 1.0])
    }

    /// ワールド変換行列、NiAlphaProperty、NiMaterialProperty、およびティントカラーから ModelUniform を構築する。
    pub fn new_with_tint(
        world_mat: glam::Mat4,
        alpha_prop: Option<&fo3_nif::NiAlphaProperty>,
        material_prop: Option<&fo3_nif::NiMaterialProperty>,
        has_glow_map: bool,
        tint_color: [f32; 4],
    ) -> Self {
        let (alpha_test, alpha_test_func, alpha_threshold) = if let Some(alpha) = alpha_prop {
            if alpha.is_test_enabled() {
                (1, alpha.test_func() as u32, alpha.threshold_normalized())
            } else {
                (0, 0, 0.0)
            }
        } else {
            (0, 0, 0.0)
        };

        let (specular_color, emissive_color) = if let Some(mat) = material_prop {
            (
                [
                    mat.specular_color.r,
                    mat.specular_color.g,
                    mat.specular_color.b,
                    if mat.glossiness > 0.0 { mat.glossiness } else { 32.0 },
                ],
                [
                    mat.emissive_color.r,
                    mat.emissive_color.g,
                    mat.emissive_color.b,
                    if mat.emissive_mult > 0.0 { mat.emissive_mult } else { 1.0 },
                ],
            )
        } else {
            ([1.0, 1.0, 1.0, 32.0], [0.0, 0.0, 0.0, 1.0])
        };

        Self {
            world: world_mat.to_cols_array(),
            specular_color,
            emissive_color,
            tint_color,
            alpha_test,
            alpha_test_func,
            alpha_threshold,
            has_glow_map: if has_glow_map { 1 } else { 0 },
        }
    }
}

pub struct RenderContext {
    pub pipeline: wgpu::RenderPipeline,
    pub transparent_pipeline: wgpu::RenderPipeline,
    pub collision_pipeline: wgpu::RenderPipeline,
    pub camera_bind_group_layout: wgpu::BindGroupLayout,
    pub model_bind_group_layout: wgpu::BindGroupLayout,
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
    pub depth_format: wgpu::TextureFormat,
}

impl RenderContext {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Mesh Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        // Group 0: Camera Uniform (binding 0) + Lighting Uniform (binding 1)
        // 参照元: Gamebryo 2.6 NiCamera, NiLight, NiDirectionalLight, NiPointLight
        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera & Lighting Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Group 1: Model Uniform
        let model_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Model Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Group 2: Diffuse Texture (0) + Sampler (1) + Normal Map Texture (2) + Glow Map Texture (3)
        // 参照元: references/openmw/components/nifosg/nifloader.cpp:L2401-2426, references/nifxml/nif.xml:L6307
        let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Texture Bind Group Layout (Diffuse + Normal + Glow)"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[
                &camera_bind_group_layout,
                &model_bind_group_layout,
                &texture_bind_group_layout,
            ],
            push_constant_ranges: &[ ],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Mesh Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
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
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // 両面描画（髪や服の裏面も表示するため）
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Self::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // 半透明メッシュ用描画パイプライン (深度書き込み無効、アルファブレンド有効)
        // 参照元: references/nifskope/src/gl/glproperty.cpp:L250-255 (glEnable(GL_BLEND))
        let transparent_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Transparent Mesh Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
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
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Self::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let collision_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Collision Line Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("collision_shader.wgsl").into()),
        });

        let collision_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Collision Pipeline Layout"),
            bind_group_layouts: &[
                &camera_bind_group_layout,
                &model_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let collision_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Collision Line Render Pipeline"),
            layout: Some(&collision_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &collision_shader,
                entry_point: Some("vs_main"),
                buffers: &[crate::collision::CollisionVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &collision_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Self::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        RenderContext {
            pipeline,
            transparent_pipeline,
            collision_pipeline,
            camera_bind_group_layout,
            model_bind_group_layout,
            texture_bind_group_layout,
            depth_format: Self::DEPTH_FORMAT,
        }
    }

    /// ウィンドウサイズに合わせた深度テクスチャビューを作成。
    pub fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }
}

#[cfg(test)]
mod tests {
    /// WGSL シェーダーの構文バリデーションテスト。
    /// 予約語や構文エラーをコンパイル時・CI時に即座に検知する。
    #[test]
    fn test_shader_wgsl_validity() {
        let shader_src = include_str!("shader.wgsl");
        let parse_res = wgpu::naga::front::wgsl::parse_str(shader_src);
        assert!(
            parse_res.is_ok(),
            "shader.wgsl の構文検証エラー: {:?}",
            parse_res.err()
        );
    }

    /// コリジョンライン描画用 WGSL シェーダーの構文バリデーションテスト。
    #[test]
    fn test_collision_shader_wgsl_validity() {
        let shader_src = include_str!("collision_shader.wgsl");
        let parse_res = wgpu::naga::front::wgsl::parse_str(shader_src);
        assert!(
            parse_res.is_ok(),
            "collision_shader.wgsl の構文検証エラー: {:?}",
            parse_res.err()
        );
    }
}

