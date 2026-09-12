//! # FaceGen EGT テクスチャモーフィングおよび合成
//!
//! Fallout 3 のアクター（NPC）の顔面化粧・肌色テクスチャを、
//! `FREGT003` フォーマットの EGT ファイルと `NPC_` レコードの `FGTS` (対称テクスチャ 50 係数) から動的合成する。
//! 参照元:
//! - `references/bevyout/src/vsa/prepare/facegen.rs` (`parse_texture_morph`, `synthesize_head_diffuse`, `sample_texture_delta`)
//! - `references/openmw/components/esm4/loadnpc.cpp:L208-215`

use std::io::Cursor;
use ddsfile::{Dds, PixelFormatFlags};

const EGT_MAGIC: &[u8; 8] = b"FREGT003";
const EGT_HEADER_BYTES: usize = 8 + 4 + 4 + 4 + 4 + 4 + 36;
const EGM_MAGIC: &[u8; 8] = b"FREGM002";
const EGM_HEADER_BYTES: usize = 8 + 4 + 4 + 4 + 4 + 40;
const MAX_FACEGEN_MODES: usize = 256;
const MAX_FACEGEN_TEXTURE_PIXELS: usize = 16_777_216;

/// FaceGen EGM 幾何形状モーフデータ構造体。
/// 参照元: `references/bevyout/src/vsa/prepare/facegen.rs:L28-35`
#[derive(Debug, Clone)]
pub struct GeometryMorph {
    pub vertex_count: usize,
    pub symmetric_count: usize,
    pub asymmetric_count: usize,
    pub modes: Vec<Vec<[f32; 3]>>,
}

/// FaceGen EGT テクスチャモーフデータ構造体。
#[derive(Debug, Clone)]
pub struct TextureMorph {
    pub width: u32,
    pub height: u32,
    pub symmetric_count: usize,
    pub modes: Vec<Vec<[f32; 3]>>,
}

/// EGT バイナリリーダー。
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let end = self.offset.checked_add(count)?;
        let slice = self.bytes.get(self.offset..end)?;
        self.offset = end;
        Some(slice)
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn i16(&mut self) -> Option<i16> {
        Some(i16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }

    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
}

/// EGM (FREGM002) バイナリバッファをパースする。
///
/// 参照元: `references/bevyout/src/vsa/prepare/facegen.rs:L110-217`
pub fn parse_geometry_morph(bytes: &[u8]) -> Result<GeometryMorph, String> {
    if bytes.len() < EGM_HEADER_BYTES {
        return Err("EGM ファイルが短すぎます".to_string());
    }
    let mut reader = Reader::new(bytes);
    if reader.take(8) != Some(EGM_MAGIC.as_slice()) {
        return Err("無効な EGM マジックヘッダーです（期待値: FREGM002）".to_string());
    }
    let vertex_count = reader.u32().ok_or("vertex_count 読み込み失敗")? as usize;
    let symmetric_count = reader.u32().ok_or("symmetric_count 読み込み失敗")? as usize;
    let asymmetric_count = reader.u32().ok_or("asymmetric_count 読み込み失敗")? as usize;
    let _basis_version = reader.u32().ok_or("basis_version 読み込み失敗")?;
    reader.take(40).ok_or("パディング読み込み失敗")?;

    if symmetric_count != 50 || asymmetric_count != 30 {
        return Err(format!(
            "想定外のモーフモード数です（期待値: 50 対称 / 30 非対称, 実際: {} / {}）",
            symmetric_count, asymmetric_count
        ));
    }
    let mode_count = symmetric_count + asymmetric_count;
    let per_mode = 4 + vertex_count * 6;
    let expected = EGM_HEADER_BYTES + per_mode * mode_count;
    if bytes.len() != expected {
        return Err(format!(
            "EGM ファイルサイズ不一致（期待値: {} bytes, 実際: {} bytes）",
            expected,
            bytes.len()
        ));
    }

    let mut modes = Vec::with_capacity(mode_count);
    for _ in 0..mode_count {
        let scale = reader.f32().ok_or("scale 読み込み失敗")?;
        if !scale.is_finite() {
            return Err("非有限数のスケール値です".to_string());
        }
        let mut deltas = Vec::with_capacity(vertex_count);
        for _ in 0..vertex_count {
            let x = reader.i16().ok_or("dx 読み込み失敗")?;
            let y = reader.i16().ok_or("dy 読み込み失敗")?;
            let z = reader.i16().ok_or("dz 読み込み失敗")?;
            deltas.push([
                (x as f32) * scale,
                (y as f32) * scale,
                (z as f32) * scale,
            ]);
        }
        modes.push(deltas);
    }

    Ok(GeometryMorph {
        vertex_count,
        symmetric_count,
        asymmetric_count,
        modes,
    })
}

/// 頂点配列に対して FaceGen 幾何形状モーフ（EGM）を適用する。
///
/// `sym_coefficients` (50次元: FGGS) および `asym_coefficients` (30次元: FGGA) を用いて
/// 頂点座標に変形オフセットを加算する。
/// 参照元: `references/bevyout/src/vsa/prepare/facegen.rs:L520-560`
pub fn apply_geometry_morph(
    positions: &mut [[f32; 3]],
    morph: &GeometryMorph,
    sym_coefficients: &[f32],
    asym_coefficients: &[f32],
) {
    let n = positions.len().min(morph.vertex_count);
    for (vertex_idx, pos) in positions.iter_mut().take(n).enumerate() {
        let mut delta = [0.0f32; 3];

        // 1. 対称モーフモード (50 係数)
        for (mode, &coeff) in morph
            .modes
            .iter()
            .take(morph.symmetric_count)
            .zip(sym_coefficients.iter())
        {
            let d = mode[vertex_idx];
            delta[0] += d[0] * coeff;
            delta[1] += d[1] * coeff;
            delta[2] += d[2] * coeff;
        }

        // 2. 非対称モーフモード (30 係数)
        for (mode, &coeff) in morph
            .modes
            .iter()
            .skip(morph.symmetric_count)
            .take(morph.asymmetric_count)
            .zip(asym_coefficients.iter())
        {
            let d = mode[vertex_idx];
            delta[0] += d[0] * coeff;
            delta[1] += d[1] * coeff;
            delta[2] += d[2] * coeff;
        }

        pos[0] += delta[0];
        pos[1] += delta[1];
        pos[2] += delta[2];
    }
}

/// EGT バイナリバイト列から `TextureMorph` をパースする。
/// 参照元: `references/bevyout/src/vsa/prepare/facegen.rs:L224-317`
pub fn parse_texture_morph(bytes: &[u8]) -> Result<TextureMorph, Box<dyn std::error::Error>> {
    if bytes.len() < EGT_HEADER_BYTES {
        return Err("EGT file truncated before header".into());
    }
    let mut reader = Reader::new(bytes);
    if reader.take(8) != Some(EGT_MAGIC) {
        return Err("Invalid EGT magic: expected FREGT003".into());
    }

    let height = reader.u32().ok_or("truncated EGT height")?;
    let width = reader.u32().ok_or("truncated EGT width")?;
    let symmetric_count = reader.u32().ok_or("truncated symmetric mode count")? as usize;
    let asymmetric_count = reader.u32().ok_or("truncated asymmetric mode count")? as usize;
    let _basis_version = reader.u32().ok_or("truncated basis version")?;
    reader.take(36).ok_or("truncated EGT reserved padding")?;

    if width == 0 || width > 16_384 || height == 0 || height > 16_384 {
        return Err("EGT width/height outside reasonable bounds".into());
    }
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or("texture pixel count overflow")?;
    if pixels == 0 || pixels > MAX_FACEGEN_TEXTURE_PIXELS {
        return Err("texture dimensions outside bounded range".into());
    }
    if symmetric_count > MAX_FACEGEN_MODES || asymmetric_count != 0 {
        return Err(format!(
            "unsupported EGT mode counts: symmetric={}, asymmetric={}",
            symmetric_count, asymmetric_count
        )
        .into());
    }

    let mode_count = symmetric_count;
    let per_mode = 4usize
        .checked_add(pixels.checked_mul(3).ok_or("mode size overflow")?)
        .ok_or("mode size overflow")?;
    let expected = EGT_HEADER_BYTES
        .checked_add(per_mode.checked_mul(mode_count).ok_or("size overflow")?)
        .ok_or("size overflow")?;

    if bytes.len() != expected {
        return Err(format!(
            "EGT size mismatch: expected {} bytes, got {}",
            expected,
            bytes.len()
        )
        .into());
    }

    let mut modes = Vec::with_capacity(mode_count);
    for _ in 0..mode_count {
        let scale = reader.f32().ok_or("truncated mode scale")?;
        if !scale.is_finite() {
            return Err("non-finite texture morph scale".into());
        }
        let mut mode = vec![[0.0f32; 3]; pixels];
        for channel in 0..3 {
            for pixel in &mut mode {
                let val_byte = reader.take(1).ok_or("truncated delta byte")?[0];
                let value = val_byte as i8;
                pixel[channel] = (value as f32) * scale;
            }
        }
        modes.push(mode);
    }

    Ok(TextureMorph {
        width,
        height,
        symmetric_count,
        modes,
    })
}

/// DXT1 / DXT3 / DXT5 または非圧縮 DDS データを RGBA8 (各ピクセル 4 バイト) ピクセル配列にデコードする。
pub fn decode_dds_to_rgba8(dds_bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), Box<dyn std::error::Error>> {
    let mut cursor = Cursor::new(dds_bytes);
    let dds = Dds::read(&mut cursor)?;

    let width = dds.header.width;
    let height = dds.header.height;
    if width == 0 || height == 0 {
        return Err("Zero dimension in DDS header".into());
    }

    let fourcc_bytes = dds.header.spf.fourcc.as_ref().map(|f| f.0.to_le_bytes());
    if let Some(fourcc) = fourcc_bytes {
        match &fourcc {
            b"DXT1" => {
                let mut rgba = vec![0u8; (width * height * 4) as usize];
                decode_bc1(&dds.data, width, height, &mut rgba)?;
                return Ok((width, height, rgba));
            }
            b"DXT3" => {
                let mut rgba = vec![0u8; (width * height * 4) as usize];
                decode_bc2(&dds.data, width, height, &mut rgba)?;
                return Ok((width, height, rgba));
            }
            b"DXT5" => {
                let mut rgba = vec![0u8; (width * height * 4) as usize];
                decode_bc3(&dds.data, width, height, &mut rgba)?;
                return Ok((width, height, rgba));
            }
            _ => return Err(format!("Unsupported DDS FourCC: {:?}", fourcc).into()),
        }
    }

    if dds.header.spf.flags.contains(PixelFormatFlags::RGB) && dds.header.spf.rgb_bit_count == Some(32) {
        let expected_size = (width * height * 4) as usize;
        if dds.data.len() >= expected_size {
            return Ok((width, height, dds.data[..expected_size].to_vec()));
        }
    }

    Err("Unsupported DDS format for FaceGen software decoding".into())
}

/// BC1 (DXT1) ソフトウェアデコーダー
fn decode_bc1(data: &[u8], width: u32, height: u32, out_rgba: &mut [u8]) -> Result<(), Box<dyn std::error::Error>> {
    let blocks_x = (width + 3) / 4;
    let blocks_y = (height + 3) / 4;
    let mut offset = 0;

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            if offset + 8 > data.len() {
                break;
            }
            let c0_raw = u16::from_le_bytes([data[offset], data[offset + 1]]);
            let c1_raw = u16::from_le_bytes([data[offset + 2], data[offset + 3]]);
            let bits = u32::from_le_bytes([data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]]);
            offset += 8;

            let c0 = rgb565_to_rgb8(c0_raw);
            let c1 = rgb565_to_rgb8(c1_raw);

            let mut palette = [[0u8; 4]; 4];
            palette[0] = [c0[0], c0[1], c0[2], 255];
            palette[1] = [c1[0], c1[1], c1[2], 255];
            if c0_raw > c1_raw {
                palette[2] = [
                    ((2 * c0[0] as u16 + c1[0] as u16) / 3) as u8,
                    ((2 * c0[1] as u16 + c1[1] as u16) / 3) as u8,
                    ((2 * c0[2] as u16 + c1[2] as u16) / 3) as u8,
                    255,
                ];
                palette[3] = [
                    ((c0[0] as u16 + 2 * c1[0] as u16) / 3) as u8,
                    ((c0[1] as u16 + 2 * c1[1] as u16) / 3) as u8,
                    ((c0[2] as u16 + 2 * c1[2] as u16) / 3) as u8,
                    255,
                ];
            } else {
                palette[2] = [
                    ((c0[0] as u16 + c1[0] as u16) / 2) as u8,
                    ((c0[1] as u16 + c1[1] as u16) / 2) as u8,
                    ((c0[2] as u16 + c1[2] as u16) / 2) as u8,
                    255,
                ];
                palette[3] = [0, 0, 0, 0];
            }

            for py in 0..4 {
                let y = by * 4 + py;
                if y >= height { continue; }
                for px in 0..4 {
                    let x = bx * 4 + px;
                    if x >= width { continue; }
                    let code_idx = ((bits >> (2 * (py * 4 + px))) & 0x03) as usize;
                    let color = palette[code_idx];
                    let dst_idx = ((y * width + x) * 4) as usize;
                    out_rgba[dst_idx..dst_idx + 4].copy_from_slice(&color);
                }
            }
        }
    }
    Ok(())
}

/// BC2 (DXT3) ソフトウェアデコーダー
fn decode_bc2(data: &[u8], width: u32, height: u32, out_rgba: &mut [u8]) -> Result<(), Box<dyn std::error::Error>> {
    let blocks_x = (width + 3) / 4;
    let blocks_y = (height + 3) / 4;
    let mut offset = 0;

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            if offset + 16 > data.len() { break; }
            let alpha_bytes = &data[offset..offset + 8];
            let c0_raw = u16::from_le_bytes([data[offset + 8], data[offset + 9]]);
            let c1_raw = u16::from_le_bytes([data[offset + 10], data[offset + 11]]);
            let bits = u32::from_le_bytes([data[offset + 12], data[offset + 13], data[offset + 14], data[offset + 15]]);
            offset += 16;

            let c0 = rgb565_to_rgb8(c0_raw);
            let c1 = rgb565_to_rgb8(c1_raw);

            let mut palette = [[0u8; 3]; 4];
            palette[0] = c0;
            palette[1] = c1;
            palette[2] = [
                ((2 * c0[0] as u16 + c1[0] as u16) / 3) as u8,
                ((2 * c0[1] as u16 + c1[1] as u16) / 3) as u8,
                ((2 * c0[2] as u16 + c1[2] as u16) / 3) as u8,
            ];
            palette[3] = [
                ((c0[0] as u16 + 2 * c1[0] as u16) / 3) as u8,
                ((c0[1] as u16 + 2 * c1[1] as u16) / 3) as u8,
                ((c0[2] as u16 + 2 * c1[2] as u16) / 3) as u8,
            ];

            for py in 0..4 {
                let y = by * 4 + py;
                let py_u = py as usize;
                let a_row = u16::from_le_bytes([alpha_bytes[py_u * 2], alpha_bytes[py_u * 2 + 1]]);
                for px in 0..4 {
                    let x = bx * 4 + px;
                    if x >= width { continue; }
                    let a_4bit = ((a_row >> (px * 4)) & 0x0F) as u8;
                    let alpha = (a_4bit << 4) | a_4bit;
                    let code_idx = ((bits >> (2 * (py * 4 + px))) & 0x03) as usize;
                    let rgb = palette[code_idx];
                    let dst_idx = ((y * width + x) * 4) as usize;
                    out_rgba[dst_idx..dst_idx + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], alpha]);
                }
            }
        }
    }
    Ok(())
}

/// BC3 (DXT5) ソフトウェアデコーダー
fn decode_bc3(data: &[u8], width: u32, height: u32, out_rgba: &mut [u8]) -> Result<(), Box<dyn std::error::Error>> {
    let blocks_x = (width + 3) / 4;
    let blocks_y = (height + 3) / 4;
    let mut offset = 0;

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            if offset + 16 > data.len() { break; }
            let a0 = data[offset];
            let a1 = data[offset + 1];
            let a_bits = [
                data[offset + 2], data[offset + 3], data[offset + 4],
                data[offset + 5], data[offset + 6], data[offset + 7],
            ];
            let mut a_indices = [0u8; 16];
            let a_val64 = u64::from_le_bytes([a_bits[0], a_bits[1], a_bits[2], a_bits[3], a_bits[4], a_bits[5], 0, 0]);
            for i in 0..16 {
                a_indices[i] = ((a_val64 >> (i * 3)) & 0x07) as u8;
            }

            let mut a_pal = [0u8; 8];
            a_pal[0] = a0;
            a_pal[1] = a1;
            if a0 > a1 {
                for i in 1..=6 {
                    a_pal[i + 1] = (((7 - i) as u16 * a0 as u16 + i as u16 * a1 as u16) / 7) as u8;
                }
            } else {
                for i in 1..=4 {
                    a_pal[i + 1] = (((5 - i) as u16 * a0 as u16 + i as u16 * a1 as u16) / 5) as u8;
                }
                a_pal[6] = 0;
                a_pal[7] = 255;
            }

            let c0_raw = u16::from_le_bytes([data[offset + 8], data[offset + 9]]);
            let c1_raw = u16::from_le_bytes([data[offset + 10], data[offset + 11]]);
            let bits = u32::from_le_bytes([data[offset + 12], data[offset + 13], data[offset + 14], data[offset + 15]]);
            offset += 16;

            let c0 = rgb565_to_rgb8(c0_raw);
            let c1 = rgb565_to_rgb8(c1_raw);

            let mut palette = [[0u8; 3]; 4];
            palette[0] = c0;
            palette[1] = c1;
            palette[2] = [
                ((2 * c0[0] as u16 + c1[0] as u16) / 3) as u8,
                ((2 * c0[1] as u16 + c1[1] as u16) / 3) as u8,
                ((2 * c0[2] as u16 + c1[2] as u16) / 3) as u8,
            ];
            palette[3] = [
                ((c0[0] as u16 + 2 * c1[0] as u16) / 3) as u8,
                ((c0[1] as u16 + 2 * c1[1] as u16) / 3) as u8,
                ((c0[2] as u16 + 2 * c1[2] as u16) / 3) as u8,
            ];

            for py in 0..4 {
                let y = by * 4 + py;
                if y >= height { continue; }
                for px in 0..4 {
                    let x = bx * 4 + px;
                    if x >= width { continue; }
                    let p_idx = (py * 4 + px) as usize;
                    let alpha = a_pal[a_indices[p_idx] as usize];
                    let code_idx = ((bits >> (2 * p_idx)) & 0x03) as usize;
                    let rgb = palette[code_idx];
                    let dst_idx = ((y * width + x) * 4) as usize;
                    out_rgba[dst_idx..dst_idx + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], alpha]);
                }
            }
        }
    }
    Ok(())
}

#[inline]
fn rgb565_to_rgb8(v: u16) -> [u8; 3] {
    let r5 = (v >> 11) & 0x1F;
    let g6 = (v >> 5) & 0x3F;
    let b5 = v & 0x1F;
    [
        ((r5 << 3) | (r5 >> 2)) as u8,
        ((g6 << 2) | (g6 >> 4)) as u8,
        ((b5 << 3) | (b5 >> 2)) as u8,
    ]
}

/// EGT モーフデータから指定座標 (output_x, output_y) の RGB 差分値をバイリニア補間サンプリングする。
/// 参照元: `references/bevyout/src/vsa/prepare/facegen.rs:L455-495`
pub fn sample_texture_delta(
    mode: &[[f32; 3]],
    morph_width: u32,
    morph_height: u32,
    output_x: u32,
    output_y: u32,
    output_width: u32,
    output_height: u32,
) -> [f32; 3] {
    // Fallout 3 のネイティブテクスチャ UV 座標系は左下原点 (bottom-origin) であるのに対し、
    // デコードされた画像配列および EGT ラスタは上段原点であるため、垂直反転 (height - 1 - y) してサンプリングする
    let flipped_y = output_height - 1 - output_y;
    if morph_width == output_width && morph_height == output_height {
        let index = (flipped_y * output_width + output_x) as usize;
        return mode[index];
    }

    let x = ((output_x as f32 + 0.5) * morph_width as f32 / output_width as f32) - 0.5;
    let y = ((flipped_y as f32 + 0.5) * morph_height as f32 / output_height as f32) - 0.5;
    let x0 = x.floor().clamp(0.0, morph_width.saturating_sub(1) as f32) as u32;
    let y0 = y.floor().clamp(0.0, morph_height.saturating_sub(1) as f32) as u32;
    let x1 = (x0 + 1).min(morph_width.saturating_sub(1));
    let y1 = (y0 + 1).min(morph_height.saturating_sub(1));
    let x_weight = (x - x0 as f32).clamp(0.0, 1.0);
    let y_weight = (y - y0 as f32).clamp(0.0, 1.0);

    let idx = |x: u32, y: u32| (y * morph_width + x) as usize;
    let tl = mode[idx(x0, y0)];
    let tr = mode[idx(x1, y0)];
    let bl = mode[idx(x0, y1)];
    let br = mode[idx(x1, y1)];

    let mut result = [0.0f32; 3];
    for ch in 0..3 {
        let top = tl[ch] * (1.0 - x_weight) + tr[ch] * x_weight;
        let bottom = bl[ch] * (1.0 - x_weight) + br[ch] * x_weight;
        result[ch] = top * (1.0 - y_weight) + bottom * y_weight;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_geometry_morph_and_apply() {
        // 50 対称 + 30 非対称 = 80 モード、各モード 2 頂点のダミー EGM バイナリ
        let vertex_count = 2u32;
        let symmetric_count = 50u32;
        let asymmetric_count = 30u32;
        let basis_version = 0u32;

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"FREGM002");
        bytes.extend_from_slice(&vertex_count.to_le_bytes());
        bytes.extend_from_slice(&symmetric_count.to_le_bytes());
        bytes.extend_from_slice(&asymmetric_count.to_le_bytes());
        bytes.extend_from_slice(&basis_version.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 40]); // padding

        let mode_count = (symmetric_count + asymmetric_count) as usize;
        for m in 0..mode_count {
            let scale = 0.1f32;
            bytes.extend_from_slice(&scale.to_le_bytes());
            for v in 0..vertex_count {
                let dx = ((m + 1) as i16) * 10;
                let dy = ((v + 1) as i16) * 5;
                let dz = -2i16;
                bytes.extend_from_slice(&dx.to_le_bytes());
                bytes.extend_from_slice(&dy.to_le_bytes());
                bytes.extend_from_slice(&dz.to_le_bytes());
            }
        }

        let morph = parse_geometry_morph(&bytes).expect("Failed to parse dummy EGM");
        assert_eq!(morph.vertex_count, 2);
        assert_eq!(morph.symmetric_count, 50);
        assert_eq!(morph.asymmetric_count, 30);
        assert_eq!(morph.modes.len(), 80);

        let mut positions = vec![[0.0, 0.0, 0.0], [10.0, 20.0, 30.0]];
        let mut sym_coeffs = vec![0.0f32; 50];
        let asym_coeffs = vec![0.0f32; 30];

        // 最初の対称モードに係数 1.0 を設定
        sym_coeffs[0] = 1.0;
        // モード 0: scale 0.1, v0 dx = 10 -> delta = 1.0, dy = 5 -> delta = 0.5, dz = -2 -> delta = -0.2
        apply_geometry_morph(&mut positions, &morph, &sym_coeffs, &asym_coeffs);

        assert!((positions[0][0] - 1.0).abs() < 1e-5);
        assert!((positions[0][1] - 0.5).abs() < 1e-5);
        assert!((positions[0][2] - (-0.2)).abs() < 1e-5);

        // v1: x=10 + 1.0 = 11.0, y=20 + (2*5)*0.1 = 21.0, z=30 - 0.2 = 29.8
        assert!((positions[1][0] - 11.0).abs() < 1e-5);
        assert!((positions[1][1] - 21.0).abs() < 1e-5);
        assert!((positions[1][2] - 29.8).abs() < 1e-5);
    }
}


/// ベースの頭部ディフューズ画像 (RGBA8) に FaceGen EGT モーフィング差分を加算合成する。
/// 参照元: `references/bevyout/src/vsa/prepare/facegen.rs:L710-796`
pub fn synthesize_head_diffuse(
    mut base_rgba: Vec<u8>,
    width: u32,
    height: u32,
    morph: &TextureMorph,
    coefficients: &[f32],
) -> Vec<u8> {
    let num_coeffs = morph.symmetric_count.min(coefficients.len());

    for y in 0..height {
        for x in 0..width {
            let p_idx = ((y * width + x) * 4) as usize;
            let mut r = base_rgba[p_idx] as f32;
            let mut g = base_rgba[p_idx + 1] as f32;
            let mut b = base_rgba[p_idx + 2] as f32;

            for mode_idx in 0..num_coeffs {
                let coeff = coefficients[mode_idx];
                if coeff.abs() < 1e-5 {
                    continue;
                }
                let delta = sample_texture_delta(
                    &morph.modes[mode_idx],
                    morph.width,
                    morph.height,
                    x,
                    y,
                    width,
                    height,
                );
                r += delta[0] * coeff;
                g += delta[1] * coeff;
                b += delta[2] * coeff;
            }

            base_rgba[p_idx] = r.round().clamp(0.0, 255.0) as u8;
            base_rgba[p_idx + 1] = g.round().clamp(0.0, 255.0) as u8;
            base_rgba[p_idx + 2] = b.round().clamp(0.0, 255.0) as u8;
        }
    }

    base_rgba
}
