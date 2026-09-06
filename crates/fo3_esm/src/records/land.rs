//! # LAND (地形ランドスケープ) レコード
//!
//! セルの地形標高ハイトマップ、法線、頂点カラー、テクスチャ定義を保持。
//! 参照元: `references/openmw/components/esm4/loadland.hpp`, `loadland.cpp`, `references/openmw/components/esm/esmterrain.cpp:39-77`

use std::io::{self, Cursor};
use byteorder::{LittleEndian, ReadBytesExt};

use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{
    FormId, SUB_ATXT, SUB_BTXT, SUB_DATA, SUB_VCLR, SUB_VHGT, SUB_VNML, SUB_VTXT,
};

/// 地形の一辺あたりの頂点数 (Gamebryo 2.6 / Fallout 3 定数)。
pub const LAND_VERTS_PER_SIDE: usize = 33;
/// 地形の総頂点数 (33 * 33 = 1089)。
pub const LAND_NUM_VERTS: usize = LAND_VERTS_PER_SIDE * LAND_VERTS_PER_SIDE;
/// セルのワールド物理サイズ (4096 ゲーム単位)。
pub const LAND_REAL_SIZE: f32 = 4096.0;
/// 地形の高さスケール係数 (8.0)。
pub const LAND_HEIGHT_SCALE: f32 = 8.0;

/// 追加テクスチャレイヤー (`ATXT` + `VTXT`)。
/// 参照元: `references/openmw/components/esm4/loadland.hpp:L84-106`
#[derive(Clone, Debug, PartialEq)]
pub struct LandTextureLayer {
    /// テクスチャ (`LTEX`) の FormID
    pub form_id: FormId,
    /// クアドラント (0: 左下, 1: 右下, 2: 左上, 3: 右上)
    pub quadrant: u8,
    /// レイヤーインデックス (0..7)
    pub layer_index: u16,
    /// 頂点透過度リスト (頂点位置 0..288 [17x17], 透過度 0.0..1.0)
    pub opacities: Vec<(u16, f32)>,
}

/// セル地形 (LAND) レコード。
///
/// 参照元: `references/openmw/components/esm4/loadland.hpp`
#[derive(Clone, Debug, PartialEq)]
pub struct LandRecord {
    /// レコード FormID
    pub form_id: FormId,
    /// ランドスケープフラグ (DATA サブレコード)
    pub land_flags: u32,
    /// 標高基準オフセット (VHGT サブレコード)
    pub height_offset: f32,
    /// 累積勾配標高差分データ (33 * 33 = 1089 要素)
    pub gradient_data: Vec<i8>,
    /// 頂点法線ベクトル (33 * 33 要素、各成分 -128..127)
    pub normals: Option<Vec<[i8; 3]>>,
    /// 頂点カラー (33 * 33 要素、RGB 各 0..255)
    pub vertex_colors: Option<Vec<[u8; 3]>>,
    /// 4 クアドラントのベーステクスチャ FormID (BTXT)
    pub base_textures: [FormId; 4],
    /// 追加テクスチャレイヤーリスト (ATXT / VTXT)
    pub layers: Vec<LandTextureLayer>,
}

impl LandRecord {
    /// レコードヘッダーとサブレコード列から LAND レコードをパース。
    ///
    /// 参照元: `references/openmw/components/esm4/loadland.cpp:90-225`
    pub fn parse(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut land_flags = 0u32;
        let mut height_offset = 0.0f32;
        let mut gradient_data = Vec::new();
        let mut normals = None;
        let mut vertex_colors = None;
        let mut base_textures = [FormId(0); 4];
        let mut layers = Vec::new();
        let mut current_layer: Option<LandTextureLayer> = None;

        for sub in subrecords {
            match sub.type_id {
                SUB_DATA => {
                    if sub.data.len() >= 4 {
                        let mut cursor = Cursor::new(&sub.data);
                        land_flags = cursor.read_u32::<LittleEndian>()?;
                    }
                }
                SUB_VHGT => {
                    // VHGT: float heightOffset (4 bytes) + int8 gradientData[1089] + 3 bytes unknown
                    if sub.data.len() >= 4 + LAND_NUM_VERTS {
                        let mut cursor = Cursor::new(&sub.data);
                        height_offset = cursor.read_f32::<LittleEndian>()?;
                        let mut grad = vec![0i8; LAND_NUM_VERTS];
                        // i8 は符号付きバイト
                        for val in grad.iter_mut() {
                            *val = cursor.read_i8()?;
                        }
                        gradient_data = grad;
                    }
                }
                SUB_VNML => {
                    // VNML: 33 * 33 * 3 bytes (法線ベクトル)
                    if sub.data.len() >= LAND_NUM_VERTS * 3 {
                        let mut cursor = Cursor::new(&sub.data);
                        let mut norms = Vec::with_capacity(LAND_NUM_VERTS);
                        for _ in 0..LAND_NUM_VERTS {
                            let nx = cursor.read_i8()?;
                            let ny = cursor.read_i8()?;
                            let nz = cursor.read_i8()?;
                            norms.push([nx, ny, nz]);
                        }
                        normals = Some(norms);
                    }
                }
                SUB_VCLR => {
                    // VCLR: 33 * 33 * 3 bytes (頂点カラー RGB)
                    if sub.data.len() >= LAND_NUM_VERTS * 3 {
                        let mut cursor = Cursor::new(&sub.data);
                        let mut cols = Vec::with_capacity(LAND_NUM_VERTS);
                        for _ in 0..LAND_NUM_VERTS {
                            let r = cursor.read_u8()?;
                            let g = cursor.read_u8()?;
                            let b = cursor.read_u8()?;
                            cols.push([r, g, b]);
                        }
                        vertex_colors = Some(cols);
                    }
                }
                SUB_BTXT => {
                    // BTXT: FormId (4 bytes) + quadrant (1 byte) + unknown (3 bytes)
                    if sub.data.len() >= 5 {
                        let mut cursor = Cursor::new(&sub.data);
                        let fid = FormId(cursor.read_u32::<LittleEndian>()?);
                        let quad = cursor.read_u8()? as usize;
                        if quad < 4 {
                            base_textures[quad] = fid;
                        }
                    }
                }
                SUB_ATXT => {
                    // ATXT: FormId (4 bytes) + quadrant (1 byte) + unknown (1 byte) + layer_index (2 bytes)
                    if let Some(prev) = current_layer.take() {
                        layers.push(prev);
                    }
                    if sub.data.len() >= 8 {
                        let mut cursor = Cursor::new(&sub.data);
                        let form_id = FormId(cursor.read_u32::<LittleEndian>()?);
                        let quadrant = cursor.read_u8()?;
                        let _unknown = cursor.read_u8()?;
                        let layer_index = cursor.read_u16::<LittleEndian>()?;
                        current_layer = Some(LandTextureLayer {
                            form_id,
                            quadrant,
                            layer_index,
                            opacities: Vec::new(),
                        });
                    }
                }
                SUB_VTXT => {
                    // VTXT: 1要素 8 bytes (position: u16, unknown1: u8, unknown2: u8, opacity: f32)
                    if let Some(ref mut layer) = current_layer {
                        let count = sub.data.len() / 8;
                        let mut cursor = Cursor::new(&sub.data);
                        for _ in 0..count {
                            let position = cursor.read_u16::<LittleEndian>()?;
                            let _u1 = cursor.read_u8()?;
                            let _u2 = cursor.read_u8()?;
                            let opacity = cursor.read_f32::<LittleEndian>()?;
                            layer.opacities.push((position, opacity));
                        }
                    }
                    if let Some(finished) = current_layer.take() {
                        layers.push(finished);
                    }
                }
                _ => {}
            }
        }

        if let Some(last) = current_layer {
            layers.push(last);
        }

        Ok(Self {
            form_id: header.form_id,
            land_flags,
            height_offset,
            gradient_data,
            normals,
            vertex_colors,
            base_textures,
            layers,
        })
    }

    /// 累積勾配データから各頂点の標高 (Z 座標) 配列 [f32; 1089] を復元計算する。
    ///
    /// 計算規則:
    /// - 参照元: `references/openmw/components/esm/esmterrain.cpp:52-71`
    /// - `row_offset += gradient_data[y * 33]`
    /// - `col_offset += gradient_data[y * 33 + x]`
    /// - `height = col_offset * 8.0`
    pub fn compute_heights(&self) -> [f32; LAND_NUM_VERTS] {
        let mut heights = [0.0f32; LAND_NUM_VERTS];
        if self.gradient_data.len() < LAND_NUM_VERTS {
            return heights;
        }

        let mut row_offset = self.height_offset;
        for y in 0..LAND_VERTS_PER_SIDE {
            row_offset += self.gradient_data[y * LAND_VERTS_PER_SIDE] as f32;
            let height_y = row_offset * LAND_HEIGHT_SCALE;
            heights[y * LAND_VERTS_PER_SIDE] = height_y;

            let mut col_offset = row_offset;
            for x in 1..LAND_VERTS_PER_SIDE {
                col_offset += self.gradient_data[y * LAND_VERTS_PER_SIDE + x] as f32;
                let height_x = col_offset * LAND_HEIGHT_SCALE;
                heights[y * LAND_VERTS_PER_SIDE + x] = height_x;
            }
        }

        heights
    }
}
