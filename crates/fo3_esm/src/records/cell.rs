//! # CELL レコード (セル空間定義)
//!
//! 室内セルや屋外グリッド区画を表現するセル定義レコード。
//! 参照元: `references/openmw/components/esm4/loadcell.hpp`, `loadcell.cpp`

use std::io::{self, Cursor, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, SUB_DATA, SUB_EDID, SUB_FULL, SUB_LNAM, SUB_LTMP, SUB_XCLC, SUB_XCLL};

/// セル環境照明パラメータ (`XCLL` サブレコード: 40 バイト)。
///
/// 参照元: `references/openmw/components/esm4/lighting.hpp:L37`, `loadcell.cpp:L181-193`
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellLighting {
    /// アンビエント環境光色 (RGBA 各 0..255)
    pub ambient: [u8; 4],
    /// 指向性光色 (RGBA 各 0..255)
    pub directional: [u8; 4],
    /// フォグ色 (RGBA 各 0..255)
    pub fog_color: [u8; 4],
    /// フォグ近クリップ距離 (World Units)
    pub fog_near: f32,
    /// フォグ遠クリップ距離 (World Units)
    pub fog_far: f32,
    /// 指向性光回転 (XY平面回転角)
    pub rotation_xy: i32,
    /// 指向性光回転 (Z軸回転角)
    pub rotation_z: i32,
    /// フォグ指向性フェード
    pub fog_dir_fade: f32,
    /// フォグクリッピング距離
    pub fog_clip_dist: f32,
    /// フォグ濃度指数
    pub fog_power: f32,
}

impl CellLighting {
    /// サブレコードバイナリ (40 バイト) からパースする。
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut ambient = [0u8; 4];
        reader.read_exact(&mut ambient)?;
        let mut directional = [0u8; 4];
        reader.read_exact(&mut directional)?;
        let mut fog_color = [0u8; 4];
        reader.read_exact(&mut fog_color)?;
        let fog_near = reader.read_f32::<LittleEndian>()?;
        let fog_far = reader.read_f32::<LittleEndian>()?;
        let rotation_xy = reader.read_i32::<LittleEndian>()?;
        let rotation_z = reader.read_i32::<LittleEndian>()?;
        let fog_dir_fade = reader.read_f32::<LittleEndian>()?;
        let fog_clip_dist = reader.read_f32::<LittleEndian>()?;
        let fog_power = reader.read_f32::<LittleEndian>()?;

        Ok(CellLighting {
            ambient,
            directional,
            fog_color,
            fog_near,
            fog_far,
            rotation_xy,
            rotation_z,
            fog_dir_fade,
            fog_clip_dist,
            fog_power,
        })
    }
}

/// セル定義レコード (`CELL`)。
#[derive(Clone, Debug, PartialEq)]
pub struct CellRecord {
    pub form_id: FormId,
    /// エディタ用識別名 (EDID)
    pub edid: String,
    /// ゲーム内表示名 (FULL)
    pub full_name: Option<String>,
    /// セルフラグ (DATA: 0x0001 = Interior, 0x0002 = HasWater 等)
    pub cell_flags: u16,
    /// 室外セルのグリッド座標 (XCLC: X, Y)
    pub grid: Option<(i32, i32)>,
    /// セル環境照明 (XCLL)
    pub lighting: Option<CellLighting>,
    /// ライティングテンプレート参照 FormID (LTMP)
    pub lighting_template: Option<FormId>,
    /// ライティングテンプレートフラグ (LNAM)
    pub lighting_template_flags: Option<u32>,
}

impl CellRecord {
    /// 室内セル（Interior）かどうか。
    pub fn is_interior(&self) -> bool {
        (self.cell_flags & 0x0001) != 0
    }

    /// サブレコード列から CellRecord を生成する。
    pub fn from_record(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut edid = String::new();
        let mut full_name = None;
        let mut cell_flags = 0u16;
        let mut grid = None;
        let mut lighting = None;
        let mut lighting_template = None;
        let mut lighting_template_flags = None;

        for sub in subrecords {
            match sub.type_id {
                SUB_EDID => {
                    edid = sub.as_string();
                }
                SUB_FULL => {
                    full_name = Some(sub.as_string());
                }
                SUB_DATA => {
                    if sub.data.len() >= 2 {
                        let mut cursor = Cursor::new(&sub.data);
                        cell_flags = cursor.read_u16::<LittleEndian>()?;
                    } else if sub.data.len() == 1 {
                        cell_flags = sub.data[0] as u16;
                    }
                }
                SUB_XCLC => {
                    if sub.data.len() >= 8 {
                        let mut cursor = Cursor::new(&sub.data);
                        let x = cursor.read_i32::<LittleEndian>()?;
                        let y = cursor.read_i32::<LittleEndian>()?;
                        grid = Some((x, y));
                    }
                }
                SUB_XCLL => {
                    if sub.data.len() >= 40 {
                        let mut cursor = Cursor::new(&sub.data);
                        if let Ok(lgt) = CellLighting::read(&mut cursor) {
                            lighting = Some(lgt);
                        }
                    }
                }
                SUB_LTMP => {
                    if sub.data.len() >= 4 {
                        let mut cursor = Cursor::new(&sub.data);
                        lighting_template = Some(FormId(cursor.read_u32::<LittleEndian>()?));
                    }
                }
                SUB_LNAM => {
                    if sub.data.len() >= 4 {
                        let mut cursor = Cursor::new(&sub.data);
                        lighting_template_flags = Some(cursor.read_u32::<LittleEndian>()?);
                    }
                }
                _ => {}
            }
        }

        Ok(CellRecord {
            form_id: header.form_id,
            edid,
            full_name,
            cell_flags,
            grid,
            lighting,
            lighting_template,
            lighting_template_flags,
        })
    }
}
