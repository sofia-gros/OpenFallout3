//! # UI フォントおよびグリフレンダリング基盤
//!
//! 参照元:
//! - `references/openmw/components/fontloader/fontloader.cpp:394-440` (Gamebryo/Bethesda .fnt バイナリ形式)
//! - `Fallout - Misc.bsa` (`menus\dialog\dialog_menu.xml`, `menus\terminal\terminal_menu.xml`)

use bytemuck::{Pod, Zeroable};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{self, Read};

/// UI 描画用頂点。
/// スクリーンピクセル座標 (X, Y)、テクスチャ座標 (U, V)、RGBA カラーを持つ。
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Pod, Zeroable)]
pub struct UiVertex {
    /// 画面座標 (ピクセル単位: 0..Width, 0..Height)
    pub position: [f32; 2],
    /// テクスチャ UV (0.0..1.0)
    pub tex_coord: [f32; 2],
    /// 頂点カラー (RGBA, 0.0..1.0)
    pub color: [f32; 4],
}

impl UiVertex {
    /// 頂点レイアウト記述子を生成。
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UiVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position: vec2<f32>
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // tex_coord: vec2<f32>
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color: vec4<f32>
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// 単一文字のグリフメトリクス情報。
#[derive(Clone, Debug, Default)]
pub struct GlyphMetrics {
    /// アトラス内での UV 範囲 (左上 U, V, 右下 U, V)
    pub uv_rect: [f32; 4],
    /// 文字の幅 (ピクセル)
    pub width: f32,
    /// 文字の高さ (ピクセル)
    pub height: f32,
    /// 次の文字までの送り幅 (Advance X)
    pub advance_x: f32,
    /// ベースラインからのオフセット Y
    pub ascent: f32,
}

/// 実機 .fnt または内製ビットマップアトラスから構築されるフォント。
pub struct BitmapFont {
    /// 基準フォントサイズ
    pub font_size: f32,
    /// 各文字 (0..255) のメトリクス
    pub glyphs: [GlyphMetrics; 256],
    /// フォントテクスチャ幅
    pub texture_width: u32,
    /// フォントテクスチャ高さ
    pub texture_height: u32,
    /// フォントテクスチャ生ピクセル (RGBA8)
    pub texture_rgba: Vec<u8>,
}

impl BitmapFont {
    /// 実機 `.fnt` バイナリデータからフォントメトリクスをパースする。
    ///
    /// 参照元: `references/openmw/components/fontloader/fontloader.cpp:410-440`
    pub fn parse_fnt(
        mut reader: impl Read,
        tex_width: u32,
        tex_height: u32,
        texture_rgba: Vec<u8>,
    ) -> io::Result<Self> {
        let font_size = reader.read_f32::<LittleEndian>()?;
        let magic1 = reader.read_i32::<LittleEndian>()?;
        let magic2 = reader.read_i32::<LittleEndian>()?;
        if magic1 != 1 || magic2 != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid .fnt magic header",
            ));
        }

        let mut name_buf = [0u8; 284];
        reader.read_exact(&mut name_buf)?;

        let mut glyphs: [GlyphMetrics; 256] = std::array::from_fn(|_| GlyphMetrics::default());

        for glyph in glyphs.iter_mut() {
            let _unused = reader.read_f32::<LittleEndian>()?;
            let tl_x = reader.read_f32::<LittleEndian>()?;
            let tl_y = reader.read_f32::<LittleEndian>()?;
            let _tr_x = reader.read_f32::<LittleEndian>()?;
            let _tr_y = reader.read_f32::<LittleEndian>()?;
            let _bl_x = reader.read_f32::<LittleEndian>()?;
            let _bl_y = reader.read_f32::<LittleEndian>()?;
            let br_x = reader.read_f32::<LittleEndian>()?;
            let br_y = reader.read_f32::<LittleEndian>()?;
            let width = reader.read_f32::<LittleEndian>()?;
            let height = reader.read_f32::<LittleEndian>()?;
            let _kerning_left = reader.read_f32::<LittleEndian>()?;
            let _kerning_right = reader.read_f32::<LittleEndian>()?;
            let ascent = reader.read_f32::<LittleEndian>()?;

            glyph.uv_rect = [tl_x, tl_y, br_x, br_y];
            glyph.width = width;
            glyph.height = height;
            glyph.advance_x = width + 1.0;
            glyph.ascent = ascent;
        }

        Ok(Self {
            font_size,
            glyphs,
            texture_width: tex_width,
            texture_height: tex_height,
            texture_rgba,
        })
    }

    /// 文字列全体の描画幅（ピクセル）をグリフメトリクスに基づいて厳密に計算する。
    pub fn measure_text_width(&self, text: &str, scale: f32) -> f32 {
        let mut width = 0.0;
        for c in text.chars() {
            let b = normalize_ui_char(c) as usize;
            if b < self.glyphs.len() {
                width += self.glyphs[b].advance_x * scale;
            }
        }
        width
    }

    /// 組み込みのフォールバック・レトロビットマップフォント (8x12 等幅) を生成。
    /// 外部アセット非依存でターミナル・会話テキストを 100% 確実に描画するための安全ネット。
    pub fn create_embedded_fallback() -> Self {
        // 128x128 のテクスチャアトラス (16x8 文字グリッド, 各 8x16 ピクセル)
        let tex_width = 128u32;
        let tex_height = 128u32;
        let mut texture_rgba = vec![0u8; (tex_width * tex_height * 4) as usize];

        // 1x1 の純白ピクセルを (0,0) に配置（ベタ塗り矩形描画用）
        for c in 0..4 {
            texture_rgba[c] = 255;
        }

        let mut glyphs: [GlyphMetrics; 256] = std::array::from_fn(|_| GlyphMetrics::default());

        // ベタ塗りクワッド用ダミーグリフ (インデックス 0)
        glyphs[0] = GlyphMetrics {
            uv_rect: [0.0, 0.0, 1.0 / tex_width as f32, 1.0 / tex_height as f32],
            width: 8.0,
            height: 12.0,
            advance_x: 8.0,
            ascent: 10.0,
        };

        // ASCII 32..127 のシンプルなビットマップ描画
        for ch in 32..127usize {
            let col = (ch % 16) as u32;
            let row = (ch / 16) as u32;
            let cell_x = col * 8;
            let cell_y = row * 16;

            // 簡易グリフパターンを描画
            render_simple_char(ch as u8, cell_x, cell_y, tex_width, &mut texture_rgba);

            let u0 = cell_x as f32 / tex_width as f32;
            let v0 = cell_y as f32 / tex_height as f32;
            let u1 = (cell_x + 8) as f32 / tex_width as f32;
            let v1 = (cell_y + 12) as f32 / tex_height as f32;

            glyphs[ch] = GlyphMetrics {
                uv_rect: [u0, v0, u1, v1],
                width: 8.0,
                height: 12.0,
                advance_x: 8.5,
                ascent: 10.0,
            };
        }

        Self {
            font_size: 14.0,
            glyphs,
            texture_width: tex_width,
            texture_height: tex_height,
            texture_rgba,
        }
    }
}

/// 8x12 の簡易アスキーフォントビットマップラスタライズ。
fn render_simple_char(ch: u8, x: u32, y: u32, stride: u32, buf: &mut [u8]) {
    // 5x7 ドットフォントの簡易テーブル
    let rows: [u8; 7] = match ch {
        b'A'..=b'Z' => get_alpha_pattern(ch),
        b'a'..=b'z' => get_alpha_pattern(ch - 32),
        b'0'..=b'9' => get_digit_pattern(ch),
        b' ' => [0, 0, 0, 0, 0, 0, 0],
        b':' => [0, 0b01000, 0, 0, 0b01000, 0, 0],
        b'.' => [0, 0, 0, 0, 0, 0b01000, 0],
        b'>' => [0b10000, 0b01000, 0b00100, 0b01000, 0b10000, 0, 0],
        b'<' => [0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0, 0],
        b'[' => [0b01100, 0b01000, 0b01000, 0b01000, 0b01100, 0, 0],
        b']' => [0b01100, 0b00100, 0b00100, 0b00100, 0b01100, 0, 0],
        b'-' => [0, 0, 0b11110, 0, 0, 0, 0],
        b'=' => [0, 0b11110, 0, 0b11110, 0, 0, 0],
        b'?' => [0b01100, 0b10010, 0b00100, 0b01000, 0, 0b01000, 0],
        b'!' => [0b01000, 0b01000, 0b01000, 0b01000, 0, 0b01000, 0],
        _ => [0b11111, 0b10001, 0b10001, 0b10001, 0b11111, 0, 0],
    };

    for (r, &row_bits) in rows.iter().enumerate() {
        for c in 0..5 {
            if (row_bits & (1 << (4 - c))) != 0 {
                let px = x + c as u32 + 1;
                let py = y + r as u32 + 2;
                let idx = ((py * stride + px) * 4) as usize;
                if idx + 3 < buf.len() {
                    buf[idx] = 255;
                    buf[idx + 1] = 255;
                    buf[idx + 2] = 255;
                    buf[idx + 3] = 255;
                }
            }
        }
    }
}

fn get_digit_pattern(ch: u8) -> [u8; 7] {
    match ch {
        b'0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        b'1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        b'2' => [
            0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111,
        ],
        b'3' => [
            0b11110, 0b00001, 0b00010, 0b01100, 0b00010, 0b00001, 0b11110,
        ],
        b'4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        b'5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        b'6' => [
            0b01110, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        b'7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        b'8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        b'9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        _ => [0; 7],
    }
}

fn get_alpha_pattern(ch: u8) -> [u8; 7] {
    match ch {
        b'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        b'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        b'C' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        b'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        b'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        b'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        b'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ],
        b'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        b'I' => [
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        b'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100,
        ],
        b'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        b'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        b'M' => [
            0b10001, 0b11011, 0b10101, 0b10001, 0b10001, 0b10001, 0b10001,
        ],
        b'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        b'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        b'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        b'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10011, 0b01111,
        ],
        b'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        b'S' => [
            0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110,
        ],
        b'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        b'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        b'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        b'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001,
        ],
        b'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        b'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        b'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        _ => [0; 7],
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum UiTexture {
    SolidColor,
    Font,
    Image(String),
}

impl Default for UiTexture {
    fn default() -> Self {
        UiTexture::SolidColor
    }
}

#[derive(Clone, Debug)]
pub struct DrawCommand {
    pub texture: UiTexture,
    pub index_start: u32,
    pub index_count: u32,
}

/// 2D 描画のための頂点とインデックスのバッチ
pub struct TextBatch {
    pub vertices: Vec<UiVertex>,
    pub indices: Vec<u32>,
    pub commands: Vec<DrawCommand>,
    pub current_texture: UiTexture,
}

impl Default for TextBatch {
    fn default() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            commands: Vec::new(),
            current_texture: UiTexture::SolidColor,
        }
    }
}

impl TextBatch {
    /// バッチをクリア
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.commands.clear();
        self.current_texture = UiTexture::SolidColor;
    }

    /// テクスチャの切り替え (DrawCommandの区切り)
    pub fn set_texture(&mut self, texture: UiTexture) {
        if self.current_texture != texture {
            self.current_texture = texture;
        }
    }

    fn push_indices(&mut self, new_indices: &[u32]) {
        let start = self.indices.len() as u32;
        self.indices.extend_from_slice(new_indices);
        let count = new_indices.len() as u32;

        if let Some(last_cmd) = self.commands.last_mut() {
            if last_cmd.texture == self.current_texture {
                last_cmd.index_count += count;
                return;
            }
        }

        self.commands.push(DrawCommand {
            texture: self.current_texture.clone(),
            index_start: start,
            index_count: count,
        });
    }

    /// 単色矩形 (SolidColor) を追加
    pub fn add_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.set_texture(UiTexture::SolidColor);
        self.add_textured_rect_internal(x, y, w, h, [0.0, 0.0, 0.0, 0.0], color);
    }

    /// テクスチャ付き矩形 (現在のテクスチャ) を追加
    pub fn add_textured_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        uv: [f32; 4], // [u0, v0, u1, v1]
        color: [f32; 4],
    ) {
        self.add_textured_rect_internal(x, y, w, h, uv, color);
    }

    fn add_textured_rect_internal(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        uv: [f32; 4],
        color: [f32; 4],
    ) {
        let base_idx = self.vertices.len() as u32;
        let [u0, v0, u1, v1] = uv;

        self.vertices.push(UiVertex {
            position: [x, y],
            tex_coord: [u0, v0],
            color,
        });
        self.vertices.push(UiVertex {
            position: [x + w, y],
            tex_coord: [u1, v0],
            color,
        });
        self.vertices.push(UiVertex {
            position: [x + w, y + h],
            tex_coord: [u1, v1],
            color,
        });
        self.vertices.push(UiVertex {
            position: [x, y + h],
            tex_coord: [u0, v1],
            color,
        });

        self.push_indices(&[
            base_idx,
            base_idx + 1,
            base_idx + 2,
            base_idx,
            base_idx + 2,
            base_idx + 3,
        ]);
    }

    /// フォントテキスト (Font) を追加
    pub fn add_text(
        &mut self,
        font: &BitmapFont,
        text: &str,
        mut x: f32,
        y: f32,
        scale: f32,
        color: [f32; 4],
    ) {
        self.set_texture(UiTexture::Font);
        for c in text.chars() {
            let b = normalize_ui_char(c);
            let metric = &font.glyphs[b as usize];
            let gw = metric.width * scale;
            let gh = metric.height * scale;

            let base_idx = self.vertices.len() as u32;
            let [u0, v0, u1, v1] = metric.uv_rect;

            self.vertices.push(UiVertex {
                position: [x, y],
                tex_coord: [u0, v0],
                color,
            });
            self.vertices.push(UiVertex {
                position: [x + gw, y],
                tex_coord: [u1, v0],
                color,
            });
            self.vertices.push(UiVertex {
                position: [x + gw, y + gh],
                tex_coord: [u1, v1],
                color,
            });
            self.vertices.push(UiVertex {
                position: [x, y + gh],
                tex_coord: [u0, v1],
                color,
            });

            self.push_indices(&[
                base_idx,
                base_idx + 1,
                base_idx + 2,
                base_idx,
                base_idx + 2,
                base_idx + 3,
            ]);

            x += metric.advance_x * scale;
        }
    }
}

/// UI テキストに含まれるスマートクォートやダッシュ記号を ASCII グリフへ正規化する。
/// Fallout 3 ESM 内の Windows-1252 / Unicode 文字列による文字化けを防止する。
pub fn normalize_ui_char(c: char) -> u8 {
    match c {
        '’' | '‘' | '`' | '´' | '\u{91}' | '\u{92}' => b'\'',
        '“' | '”' | '«' | '»' | '\u{93}' | '\u{94}' => b'"',
        '–' | '—' | '−' | '\u{96}' | '\u{97}' => b'-',
        '…' | '\u{85}' => b'.',
        '•' | '\u{95}' => b'*',
        c if (c as u32) < 128 => c as u8,
        _ => b' ',
    }
}
