//! # グローバル変数レコード (`GLOB`) パースモジュール
//!
//! 参照元:
//! - `references/openmw/components/esm4/loadglob.hpp`, `loadglob.cpp`

use std::io::{self, Cursor};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, REC_GLOB};

/// グローバル変数 (`GLOB`) レコード。
#[derive(Clone, Debug, PartialEq)]
pub struct GlobRecord {
    /// レコード FormID
    pub form_id: FormId,
    /// エディタ ID (`EDID`, 例: "GameHour", "GameDaysPassed")
    pub edid: String,
    /// 変数型 (`FNAM`: b's' = short, b'l' = long, b'f' = float)
    pub value_type: u8,
    /// 値 (`FLTV`: f32 値として格納)
    pub value: f32,
}

impl GlobRecord {
    /// サブレコード群から GLOB レコードをパースする。
    pub fn parse(record_header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        if record_header.type_id != REC_GLOB {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected GLOB record, found {:?}", record_header.type_id),
            ));
        }

        let mut edid = String::new();
        let mut value_type = b'f';
        let mut value = 0.0f32;

        for sub in subrecords {
            match &sub.type_id.0 {
                b"EDID" => {
                    edid = sub.as_string();
                }
                b"FNAM" => {
                    if !sub.data.is_empty() {
                        value_type = sub.data[0];
                    }
                }
                b"FLTV" => {
                    if sub.data.len() >= 4 {
                        let mut cur = Cursor::new(&sub.data);
                        value = cur.read_f32::<LittleEndian>()?;
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            form_id: record_header.form_id,
            edid,
            value_type,
            value,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_glob_record() {
        let record_header = RecordHeader {
            type_id: REC_GLOB,
            data_size: 50,
            flags: 0,
            form_id: FormId(0x00000039),
            vc_info: 0,
            form_version: 15,
            vc_info2: 0,
        };

        let subrecords = vec![
            Subrecord { type_id: crate::types::FourCC(*b"EDID"), data: b"GameHour\0".to_vec() },
            Subrecord { type_id: crate::types::FourCC(*b"FNAM"), data: vec![b'f'] },
            Subrecord { type_id: crate::types::FourCC(*b"FLTV"), data: 8.5f32.to_le_bytes().to_vec() },
        ];

        let glob = GlobRecord::parse(&record_header, &subrecords).expect("Failed to parse GLOB");
        assert_eq!(glob.edid, "GameHour");
        assert_eq!(glob.value_type, b'f');
        assert!((glob.value - 8.5).abs() < 1e-5);
    }
}
