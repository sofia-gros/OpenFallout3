//! # NIF ヘッダーパーサー (Fallout 3 / v20.2.0.7)
//!
//! 参照元:
//! - `references/nifxml/nif.xml:L1963-L1983` (`Header`)
//! - `references/nifxml/nif.xml:L1952-L1960` (`BSStreamHeader`)
//! - `references/nifxml/nif.xml:L1889-L1893` (`ExportString`)
//! - `knowledge/nif_v20_2_0_7_format.md`

use std::io::{self, BufRead, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use thiserror::Error;

/// NIF パース時のエラー型。
#[derive(Debug, Error)]
pub enum NifError {
    #[error("I/O エラー: {0}")]
    Io(#[from] io::Error),

    #[error("不正な NIF ヘッダ文字列: {0}")]
    InvalidHeaderString(String),

    #[error("未対応の NIF バージョン: {0:#X} (期待値: 0x14020007 / 20.2.0.7)")]
    UnsupportedVersion(u32),

    #[error("未対応のエンディアン種別: {0} (0 = Little Endian 期待)")]
    UnsupportedEndian(u8),

    #[error("未対応の User Version: {0} (Fallout 3 は 11 期待)")]
    UnsupportedUserVersion(u32),

    #[error("不正な文字列エンコーディング: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
}

/// Bethesda 固有のエクスポート文字列構造体。
///
/// 参照元: `references/nifxml/nif.xml:L1889` (`ExportString`)
#[derive(Clone, Debug, PartialEq)]
pub struct ExportString {
    pub value: String,
}

impl ExportString {
    /// ストリームから ExportString を読み込む。
    /// 1 バイトの長さプレフィックスの後、null 終端を含む文字列データが続く。
    pub fn read<R: Read>(reader: &mut R) -> Result<Self, NifError> {
        let len = reader.read_u8()? as usize;
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf)?;

        // 末尾の null バイトを除去
        if let Some(&0) = buf.last() {
            buf.pop();
        }

        let value = String::from_utf8_lossy(&buf).to_string();
        Ok(ExportString { value })
    }
}

/// Bethesda 固有のエクスポーターヘッダー情報。
///
/// 参照元: `references/nifxml/nif.xml:L1952` (`BSStreamHeader`)
#[derive(Clone, Debug, PartialEq)]
pub struct BSStreamHeader {
    /// Bethesda バージョン (Fallout 3 では通常 34)
    pub bs_version: u32,
    /// 著者情報
    pub author: ExportString,
    /// 処理スクリプト (BS Version < 131 の場合存在)
    pub process_script: Option<ExportString>,
    /// エクスポートスクリプト
    pub export_script: ExportString,
}

impl BSStreamHeader {
    pub fn read<R: Read>(reader: &mut R) -> Result<Self, NifError> {
        let bs_version = reader.read_u32::<LittleEndian>()?;
        let author = ExportString::read(reader)?;
        let process_script = if bs_version < 131 {
            Some(ExportString::read(reader)?)
        } else {
            None
        };
        let export_script = ExportString::read(reader)?;

        Ok(BSStreamHeader {
            bs_version,
            author,
            process_script,
            export_script,
        })
    }
}

/// 長さ付き文字列 (`SizedString`)。
/// 4 バイト符号なし整数 (LE) の長さプレフィックスの後、文字列バイトが続く。
fn read_sized_string<R: Read>(reader: &mut R) -> Result<String, NifError> {
    let len = reader.read_u32::<LittleEndian>()? as usize;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).to_string())
}

/// Fallout 3 NIF ヘッダー構造体。
///
/// 参照元: `references/nifxml/nif.xml:L1963` (`Header`)
#[derive(Clone, Debug)]
pub struct NifHeader {
    /// ヘッダー文字列 (例: "Gamebryo File Format, Version 20.2.0.7\n")
    pub header_string: String,
    /// NIF バージョン (Fallout 3: 0x14020007)
    pub version: u32,
    /// エンディアン種別 (0: Little Endian)
    pub endian_type: u8,
    /// ユーザーバージョン (Fallout 3: 11)
    pub user_version: u32,
    /// ファイル内に含まれる全ブロック数
    pub num_blocks: u32,
    /// Bethesda ストリームヘッダー
    pub bs_header: BSStreamHeader,
    /// ファイル内で使用されるブロック型名テーブル
    pub block_types: Vec<String>,
    /// 各ブロックの型インデックス配列 (`[block_index] -> block_type_index`)
    pub block_type_indices: Vec<u16>,
    /// 各ブロックのバイナリサイズ配列
    pub block_sizes: Vec<u32>,
    /// グローバル文字列プール配列
    pub strings: Vec<String>,
}

impl NifHeader {
    /// バッファリーダーから Fallout 3 NIF ヘッダーをパースする。
    pub fn read<R: BufRead>(reader: &mut R) -> Result<Self, NifError> {
        // 1. Header String ('\n' 終端)
        let mut header_bytes = Vec::new();
        reader.read_until(b'\n', &mut header_bytes)?;
        let header_string = String::from_utf8_lossy(&header_bytes).to_string();

        if !header_string.starts_with("Gamebryo File Format, Version 20.2.0.7") {
            return Err(NifError::InvalidHeaderString(header_string));
        }

        // 2. Version
        let version = reader.read_u32::<LittleEndian>()?;
        if version != 0x14020007 {
            return Err(NifError::UnsupportedVersion(version));
        }

        // 3. Endian Type (0: BIG, 1: LITTLE - 参照元: nif.xml:L1228 EndianType)
        let endian_type = reader.read_u8()?;
        if endian_type != 1 {
            return Err(NifError::UnsupportedEndian(endian_type));
        }

        // 4. User Version
        let user_version = reader.read_u32::<LittleEndian>()?;
        if user_version != 11 {
            return Err(NifError::UnsupportedUserVersion(user_version));
        }

        // 5. Num Blocks
        let num_blocks = reader.read_u32::<LittleEndian>()?;

        // 6. BS Header (#BSSTREAMHEADER# 条件合致: 20.2.0.7 && user_version 11)
        let bs_header = BSStreamHeader::read(reader)?;

        // 7. Num Block Types
        let num_block_types = reader.read_u16::<LittleEndian>()? as usize;

        // 8. Block Types
        let mut block_types = Vec::with_capacity(num_block_types);
        for _ in 0..num_block_types {
            block_types.push(read_sized_string(reader)?);
        }

        // 9. Block Type Index (各ブロックの型)
        let mut block_type_indices = Vec::with_capacity(num_blocks as usize);
        for _ in 0..num_blocks {
            block_type_indices.push(reader.read_u16::<LittleEndian>()?);
        }

        // 10. Block Size
        let mut block_sizes = Vec::with_capacity(num_blocks as usize);
        for _ in 0..num_blocks {
            block_sizes.push(reader.read_u32::<LittleEndian>()?);
        }

        // 11. Num Strings
        let num_strings = reader.read_u32::<LittleEndian>()? as usize;
        // 12. Max String Length
        let _max_string_len = reader.read_u32::<LittleEndian>()?;

        // 13. Strings (グローバル文字列プール)
        let mut strings = Vec::with_capacity(num_strings);
        for _ in 0..num_strings {
            strings.push(read_sized_string(reader)?);
        }

        // 14. Num Groups (通常 0)
        let num_groups = reader.read_u32::<LittleEndian>()?;
        for _ in 0..num_groups {
            let _ = reader.read_u32::<LittleEndian>()?;
        }

        Ok(NifHeader {
            header_string,
            version,
            endian_type,
            user_version,
            num_blocks,
            bs_header,
            block_types,
            block_type_indices,
            block_sizes,
            strings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use byteorder::WriteBytesExt;

    #[test]
    fn test_nif_header_roundtrip() {
        let mut data = Vec::new();
        // 1. Header string
        data.extend_from_slice(b"Gamebryo File Format, Version 20.2.0.7\n");
        // 2. Version
        data.write_u32::<LittleEndian>(0x14020007).unwrap();
        // 3. Endian type (1: LITTLE)
        data.push(1);
        // 4. User version
        data.write_u32::<LittleEndian>(11).unwrap();
        // 5. Num blocks
        data.write_u32::<LittleEndian>(2).unwrap();
        // 6. BS Header
        // bs_version
        data.write_u32::<LittleEndian>(34).unwrap();
        // author (ExportString: len + chars with null)
        data.push(4);
        data.extend_from_slice(b"FO3\0");
        // process_script (bs_version < 131)
        data.push(1);
        data.push(0);
        // export_script
        data.push(1);
        data.push(0);
        // 7. Num block types
        data.write_u16::<LittleEndian>(1).unwrap();
        // 8. Block types: SizedString ("NiNode")
        data.write_u32::<LittleEndian>(6).unwrap();
        data.extend_from_slice(b"NiNode");
        // 9. Block type index (2 blocks)
        data.write_u16::<LittleEndian>(0).unwrap();
        data.write_u16::<LittleEndian>(0).unwrap();
        // 10. Block sizes (2 blocks)
        data.write_u32::<LittleEndian>(64).unwrap();
        data.write_u32::<LittleEndian>(128).unwrap();
        // 11. Num strings
        data.write_u32::<LittleEndian>(1).unwrap();
        // 12. Max string length
        data.write_u32::<LittleEndian>(4).unwrap();
        // 13. Strings
        data.write_u32::<LittleEndian>(4).unwrap();
        data.extend_from_slice(b"Root");
        // 14. Num groups (0)
        data.write_u32::<LittleEndian>(0).unwrap();

        let mut cursor = Cursor::new(data);
        let header = NifHeader::read(&mut cursor).expect("ヘッダーパースに成功すること");

        assert_eq!(header.version, 0x14020007);
        assert_eq!(header.user_version, 11);
        assert_eq!(header.bs_header.bs_version, 34);
        assert_eq!(header.bs_header.author.value, "FO3");
        assert_eq!(header.num_blocks, 2);
        assert_eq!(header.block_types, vec!["NiNode"]);
        assert_eq!(header.block_type_indices, vec![0, 0]);
        assert_eq!(header.block_sizes, vec![64, 128]);
        assert_eq!(header.strings, vec!["Root"]);
    }
}
