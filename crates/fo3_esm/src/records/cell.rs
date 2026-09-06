//! # CELL レコード (セル空間定義)
//!
//! 室内セルや屋外グリッド区画を表現するセル定義レコード。
//! 参照元: `references/openmw/components/esm4/loadcell.hpp`, `loadcell.cpp`

use std::io::{self, Cursor};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, SUB_DATA, SUB_EDID, SUB_FULL, SUB_XCLC};

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
                _ => {}
            }
        }

        Ok(CellRecord {
            form_id: header.form_id,
            edid,
            full_name,
            cell_flags,
            grid,
        })
    }
}
