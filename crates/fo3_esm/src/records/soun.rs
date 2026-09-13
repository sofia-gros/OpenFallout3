//! サウンドレコード (`SOUN`) のパース処理。
//!
//! 参照元: `references/openmw/components/esm4/loadsoun.hpp:41-98`, `Fallout3.esm:SOUN`

use std::io;

use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, REC_SOUN};

/// サウンドレコード (`SOUN`)。
///
/// スクリプトの `PlaySound` やオブジェクト（ドア、コンテナ、アイテム）の開閉音・取得音などで参照される。
#[derive(Clone, Debug, PartialEq)]
pub struct SounRecord {
    /// レコード FormID
    pub form_id: FormId,
    /// エディタ ID (`EDID`)
    pub edid: String,
    /// サウンドファイル相対パス (`FNAM`)
    pub sound_file: String,
    /// 最小減衰距離 (`SNDX.min_attenuation`)
    pub min_attenuation: u8,
    /// 最大減衰距離 (`SNDX.max_attenuation`)
    pub max_attenuation: u8,
    /// 周波数調整パーセント (`SNDX.freq_adjustment`)
    pub freq_adjustment: i8,
    /// サウンドフラグ (`SNDX.flags`)
    pub flags: u16,
    /// 静的減衰量 (`SNDX.static_attenuation`)
    pub static_attenuation: u16,
}

impl SounRecord {
    /// サブレコード群から SOUN レコードをパースする。
    pub fn parse(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        if header.type_id != REC_SOUN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected SOUN record, found {:?}", header.type_id),
            ));
        }

        let mut edid = String::new();
        let mut sound_file = String::new();
        let mut min_attenuation = 0u8;
        let mut max_attenuation = 0u8;
        let mut freq_adjustment = 0i8;
        let mut flags = 0u16;
        let mut static_attenuation = 0u16;

        for sub in subrecords {
            match &sub.type_id.0 {
                b"EDID" => {
                    edid = sub.as_string();
                }
                b"FNAM" => {
                    sound_file = sub.as_string();
                }
                b"SNDX" => {
                    if sub.data.len() >= 10 {
                        min_attenuation = sub.data[0];
                        max_attenuation = sub.data[1];
                        freq_adjustment = sub.data[2] as i8;
                        flags = u16::from_le_bytes(sub.data[4..6].try_into().unwrap_or([0, 0]));
                        static_attenuation = u16::from_le_bytes(sub.data[8..10].try_into().unwrap_or([0, 0]));
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            form_id: header.form_id,
            edid,
            sound_file,
            min_attenuation,
            max_attenuation,
            freq_adjustment,
            flags,
            static_attenuation,
        })
    }
}
