//! # サブレコード (Subrecord) パース処理
//!
//! レコード内部に格納される各サブレコードの読み込み、
//! 巨大サブレコード (`XXXX`) のサイズ拡張処理、文字列・数値変換ヘルパー。
//! 参照元: `references/openmw/components/esm4/reader.cpp`

use std::io::{self, Cursor, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::header::SubrecordHeader;
use crate::types::{FourCC, ObjectBounds, SUB_XXXX};

/// 単一のサブレコード。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subrecord {
    pub type_id: FourCC,
    pub data: Vec<u8>,
}

impl Subrecord {
    /// null 終端文字列（ASCII / UTF-8）として解釈。末尾の `\0` は自動除去。
    pub fn as_string(&self) -> String {
        let mut slice = self.data.as_slice();
        if let Some(&0) = slice.last() {
            slice = &slice[..slice.len() - 1];
        }
        String::from_utf8_lossy(slice).to_string()
    }

    /// u32 値として解釈。
    pub fn as_u32(&self) -> io::Result<u32> {
        if self.data.len() < 4 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Subrecord too small for u32"));
        }
        let mut cursor = Cursor::new(&self.data);
        cursor.read_u32::<LittleEndian>()
    }

    /// i32 値として解釈。
    pub fn as_i32(&self) -> io::Result<i32> {
        if self.data.len() < 4 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Subrecord too small for i32"));
        }
        let mut cursor = Cursor::new(&self.data);
        cursor.read_i32::<LittleEndian>()
    }

    /// f32 値として解釈。
    pub fn as_f32(&self) -> io::Result<f32> {
        if self.data.len() < 4 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Subrecord too small for f32"));
        }
        let mut cursor = Cursor::new(&self.data);
        cursor.read_f32::<LittleEndian>()
    }

    /// u16 値として解釈。
    pub fn as_u16(&self) -> io::Result<u16> {
        if self.data.len() < 2 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Subrecord too small for u16"));
        }
        let mut cursor = Cursor::new(&self.data);
        cursor.read_u16::<LittleEndian>()
    }

    /// FormId 値として解釈。
    pub fn as_form_id(&self) -> io::Result<crate::types::FormId> {
        self.as_u32().map(crate::types::FormId)
    }

    /// DATA サブレコードから位置とオイラー角回転 (pos [f32; 3], rot [f32; 3]) を取得 (24 bytes)。
    pub fn as_pos_rot(&self) -> io::Result<([f32; 3], [f32; 3])> {
        if self.data.len() < 24 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Subrecord too small for pos/rot DATA"));
        }
        let mut cursor = Cursor::new(&self.data);
        let px = cursor.read_f32::<LittleEndian>()?;
        let py = cursor.read_f32::<LittleEndian>()?;
        let pz = cursor.read_f32::<LittleEndian>()?;
        let rx = cursor.read_f32::<LittleEndian>()?;
        let ry = cursor.read_f32::<LittleEndian>()?;
        let rz = cursor.read_f32::<LittleEndian>()?;
        Ok(([px, py, pz], [rx, ry, rz]))
    }

    /// OBND 境界ボックス ([i16; 3] x 2) として解釈。
    pub fn as_bounds(&self) -> io::Result<ObjectBounds> {
        if self.data.len() < 12 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Subrecord too small for ObjectBounds"));
        }
        let mut cursor = Cursor::new(&self.data);
        let min_x = cursor.read_i16::<LittleEndian>()?;
        let min_y = cursor.read_i16::<LittleEndian>()?;
        let min_z = cursor.read_i16::<LittleEndian>()?;
        let max_x = cursor.read_i16::<LittleEndian>()?;
        let max_y = cursor.read_i16::<LittleEndian>()?;
        let max_z = cursor.read_i16::<LittleEndian>()?;

        Ok(ObjectBounds {
            min: [min_x, min_y, min_z],
            max: [max_x, max_y, max_z],
        })
    }
}

/// 指定バイト数のバッファからサブレコード列をすべてパースする。
/// `XXXX` によるサイズ拡張を自動解決。
pub fn parse_subrecords<R: Read>(reader: &mut R, total_size: usize) -> io::Result<Vec<Subrecord>> {
    let mut subrecords = Vec::new();
    let mut bytes_read = 0;
    let mut override_size: Option<usize> = None;

    while bytes_read < total_size {
        let header = SubrecordHeader::read(reader)?;
        bytes_read += SubrecordHeader::SIZE;

        let data_len = if let Some(size) = override_size.take() {
            size
        } else {
            header.data_size as usize
        };

        let mut data = vec![0u8; data_len];
        reader.read_exact(&mut data)?;
        bytes_read += data_len;

        if header.type_id == SUB_XXXX {
            if data.len() >= 4 {
                let mut cursor = Cursor::new(&data);
                override_size = Some(cursor.read_u32::<LittleEndian>()? as usize);
            }
        } else {
            subrecords.push(Subrecord {
                type_id: header.type_id,
                data,
            });
        }
    }

    Ok(subrecords)
}
