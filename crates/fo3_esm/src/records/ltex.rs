//! # LTEX (Land Texture) レコードパーサー
//!
//! 景観テクスチャの定義レコード。
//! エディタID、テクスチャ画像パス (ICON)、スペキュラ、Havok物理マテリアルを保持。
//! 参照元: `references/openmw/components/esm4/loadltex.hpp`, `loadltex.cpp`

use std::io;
use byteorder::{LittleEndian, ReadBytesExt};
use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, SUB_EDID, SUB_GNAM, SUB_HNAM, SUB_ICON, SUB_SNAM, SUB_TNAM};

/// 景観テクスチャレコード (`LTEX`)。
#[derive(Clone, Debug, PartialEq)]
pub struct LtexRecord {
    /// 一意の FormID
    pub form_id: FormId,
    /// エディタ ID (`EDID`)
    pub edid: String,
    /// ディフューズテクスチャ画像ファイル相対パス (`ICON` - Oblivion用フォールバック)
    pub texture_path: String,
    /// テクスチャセットレコード (`TXST`) の FormID (`TNAM` - Fallout 3 / Skyrim)
    pub texture_set: FormId,
    /// Havok 物理マテリアル ID (`HNAM`)
    pub havok_material: u8,
    /// 摩擦係数 (`HNAM`)
    pub friction: u8,
    /// 反発係数 (`HNAM`)
    pub restitution: u8,
    /// スペキュラ強度 (`SNAM`)
    pub specular: u8,
    /// 生育する草の FormID リスト (`GNAM`)
    pub grass: Vec<FormId>,
}

impl LtexRecord {
    /// レコードヘッダーとサブレコード群から LTEX レコードを構築する。
    pub fn read(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut edid = String::new();
        let mut texture_path = String::new();
        let mut texture_set = FormId(0);
        let mut havok_material = 0u8;
        let mut friction = 0u8;
        let mut restitution = 0u8;
        let mut specular = 30u8;
        let mut grass = Vec::new();

        for sub in subrecords {
            match sub.type_id {
                SUB_EDID => {
                    edid = sub.as_string();
                }
                SUB_ICON => {
                    texture_path = sub.as_string();
                }
                SUB_TNAM => {
                    if sub.data.len() >= 4 {
                        let mut cursor = std::io::Cursor::new(&sub.data);
                        texture_set = FormId(cursor.read_u32::<LittleEndian>()?);
                    }
                }
                SUB_HNAM => {
                    if sub.data.len() >= 3 {
                        havok_material = sub.data[0];
                        friction = sub.data[1];
                        restitution = sub.data[2];
                    } else if sub.data.len() >= 2 {
                        friction = sub.data[0];
                        restitution = sub.data[1];
                    }
                }
                SUB_SNAM => {
                    if !sub.data.is_empty() {
                        specular = sub.data[0];
                    }
                }
                SUB_GNAM => {
                    if sub.data.len() >= 4 {
                        let mut cursor = std::io::Cursor::new(&sub.data);
                        grass.push(FormId(cursor.read_u32::<LittleEndian>()?));
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            form_id: header.form_id,
            edid,
            texture_path,
            texture_set,
            havok_material,
            friction,
            restitution,
            specular,
            grass,
        })
    }
}
