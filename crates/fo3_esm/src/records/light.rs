//! # LIGH レコード (光源定義)
//!
//! 点光源やスポットライト、照明器具メッシュの定義レコード。
//! 参照元: `references/openmw/components/esm4/loadligh.hpp`, `loadligh.cpp`

use std::io::{self, Cursor, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, SUB_DATA, SUB_EDID, SUB_FULL, SUB_MODL};

/// 光源定義レコード (`LIGH`)。
///
/// 参照元: `references/openmw/components/esm4/loadligh.hpp:L42-87`
#[derive(Clone, Debug, PartialEq)]
pub struct LightRecord {
    pub form_id: FormId,
    /// エディタ用識別名 (EDID)
    pub edid: String,
    /// ゲーム内表示名 (FULL)
    pub full_name: Option<String>,
    /// 照明器具の 3D モデルパス (MODL)
    pub model: Option<String>,
    /// 点灯持続時間 (-1 は常時点灯)
    pub time: i32,
    /// 光の最大到達半径 (World Units)
    pub radius: u32,
    /// 光源色 (RGBA)
    pub colour: [u8; 4],
    /// 光源フラグ (0x01=Dynamic, 0x08=Flicker, 0x200=SpotLight 等)
    pub flags: i32,
    /// 減衰指数 (Falloff)
    pub falloff: f32,
    /// スポットライト照射角 (FOV)
    pub fov: f32,
    /// 価格
    pub value: u32,
    /// 重量
    pub weight: f32,
}

impl LightRecord {
    /// サブレコード列から LightRecord を生成する。
    pub fn from_record(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut edid = String::new();
        let mut full_name = None;
        let mut model = None;
        let mut time = -1;
        let mut radius = 0u32;
        let mut colour = [255, 255, 255, 255];
        let mut flags = 0i32;
        let mut falloff = 1.0f32;
        let mut fov = 90.0f32;
        let mut value = 0u32;
        let mut weight = 0.0f32;

        for sub in subrecords {
            match sub.type_id {
                SUB_EDID => {
                    edid = sub.as_string();
                }
                SUB_FULL => {
                    full_name = Some(sub.as_string());
                }
                SUB_MODL => {
                    model = Some(sub.as_string());
                }
                SUB_DATA => {
                    if sub.data.len() >= 16 {
                        let mut cursor = Cursor::new(&sub.data);
                        time = cursor.read_i32::<LittleEndian>()?;
                        radius = cursor.read_u32::<LittleEndian>()?;
                        cursor.read_exact(&mut colour)?;
                        flags = cursor.read_i32::<LittleEndian>()?;

                        if sub.data.len() >= 32 {
                            falloff = cursor.read_f32::<LittleEndian>()?;
                            fov = cursor.read_f32::<LittleEndian>()?;
                            value = cursor.read_u32::<LittleEndian>()?;
                            weight = cursor.read_f32::<LittleEndian>()?;
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(LightRecord {
            form_id: header.form_id,
            edid,
            full_name,
            model,
            time,
            radius,
            colour,
            flags,
            falloff,
            fov,
            value,
            weight,
        })
    }
}
