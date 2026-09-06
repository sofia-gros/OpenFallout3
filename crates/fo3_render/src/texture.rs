//! # GPU テクスチャ管理
//!
//! DDS (DirectDraw Surface) テクスチャの読み込みと wgpu へのアップロード、
//! およびフォールバック用デフォルトテクスチャの生成。

use std::io::Cursor;
use ddsfile::{Dds, PixelFormatFlags};

/// GPU 上のテクスチャリソース。
pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl GpuTexture {
    /// DDS バイナリデータから GPU テクスチャを生成する。
    /// BC1 (DXT1), BC2 (DXT3), BC3 (DXT5) の圧縮テクスチャをダイレクトに GPU 転送。
    pub fn from_dds_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut cursor = Cursor::new(bytes);
        let dds = Dds::read(&mut cursor)?;

        let orig_width = dds.header.width;
        let orig_height = dds.header.height;

        if orig_width < 4 || orig_height < 4 {
            return Err("Texture dimensions too small for BC compression (< 4px)".into());
        }

        // BC 圧縮ブロックサイズ (4x4) にアライメント
        let width = ((orig_width + 3) / 4) * 4;
        let height = ((orig_height + 3) / 4) * 4;

        // BC 圧縮テクスチャの場合、4x4 未満のミップレベルは wgpu のブロック制約を満たせないため除外
        let total_mips = dds.get_num_mipmap_levels().max(1);
        let mut valid_mips = 0;
        for mip in 0..total_mips {
            let mip_w = width >> mip;
            let mip_h = height >> mip;
            if mip_w >= 4 && mip_h >= 4 {
                valid_mips += 1;
            } else {
                break;
            }
        }
        let mip_levels = valid_mips.max(1);

        // フォーマット判定
        let (format, block_size) = if let Some(fourcc) = dds.header.spf.fourcc.as_ref() {
            match &fourcc.0.to_le_bytes() {
                b"DXT1" => (wgpu::TextureFormat::Bc1RgbaUnorm, 8),
                b"DXT3" => (wgpu::TextureFormat::Bc2RgbaUnorm, 16),
                b"DXT5" => (wgpu::TextureFormat::Bc3RgbaUnorm, 16),
                _ => return Err(format!("Unsupported FourCC: {:?}", fourcc).into()),
            }
        } else if dds.header.spf.flags.contains(PixelFormatFlags::RGB) {
            // 非圧縮 RGB/RGBA のフォールバック
            return Err("Uncompressed DDS format not supported directly yet".into());
        } else {
            return Err("Unknown DDS pixel format".into());
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // ミップマップレベルごとに書き込み
        let mut offset = 0;
        for mip in 0..mip_levels {
            let mip_w = ((width >> mip) + 3) / 4 * 4;
            let mip_h = ((height >> mip) + 3) / 4 * 4;

            let blocks_x = mip_w / 4;
            let blocks_y = mip_h / 4;
            let bytes_per_row = blocks_x * block_size;
            let level_size = (bytes_per_row * blocks_y) as usize;

            if offset + level_size <= dds.data.len() {
                let slice = &dds.data[offset..offset + level_size];
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: mip,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    slice,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_row),
                        rows_per_image: Some(blocks_y),
                    },
                    wgpu::Extent3d {
                        width: mip_w,
                        height: mip_h,
                        depth_or_array_layers: 1,
                    },
                );
                offset += level_size;
            } else {
                break;
            }
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Ok(GpuTexture {
            texture,
            view,
            sampler,
        })
    }

    /// テクスチャ未設定または読み込み失敗時のフォールバック用 2x2 白色テクスチャを生成。
    pub fn create_default_white(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let size = 2;
        let white_pixels: [u8; 16] = [255; 16]; // 2x2 RGBA (各 4 バイト)

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Default White Texture"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &white_pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size * 4),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        GpuTexture {
            texture,
            view,
            sampler,
        }
    }

    /// 法線マップ未設定時のフォールバック用フラット法線テクスチャ (Z=+1, Specular=0) を生成。
    /// RGBA: [128, 128, 255, 0] (タンジェント空間法線 (0,0,1)、グロス 0)
    pub fn create_default_normal(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let size = 2;
        let normal_pixels: [u8; 16] = [
            128, 128, 255, 0,
            128, 128, 255, 0,
            128, 128, 255, 0,
            128, 128, 255, 0,
        ];

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Default Flat Normal Texture"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
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
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &normal_pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size * 4),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        GpuTexture {
            texture,
            view,
            sampler,
        }
    }
}
