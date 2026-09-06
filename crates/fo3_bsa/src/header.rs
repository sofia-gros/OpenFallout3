//! # BSA ヘッダー定義モジュール
//!
//! 参照元: `references/openmw/components/bsa/compressedbsafile.hpp:L39-L90`

use std::io::{self, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use thiserror::Error;

/// BSA パース時のエラー型。
#[derive(Debug, Error)]
pub enum BsaError {
    #[error("I/O エラー: {0}")]
    Io(#[from] io::Error),

    #[error("不正な BSA マジックコード: {0:?} (期待値: b\"BSA\\0\")")]
    InvalidMagic([u8; 4]),

    #[error("未対応の BSA バージョン: {0} (Fallout 3 は 104 / 0x68 期待)")]
    UnsupportedVersion(u32),

    #[error("ファイルが見つかりません: {0}")]
    FileNotFound(String),

    #[error("zlib 解凍エラー: {0}")]
    DecompressError(String),
}

/// BSA アーカイブフラグビットマスク。
///
/// 参照元: OpenMW `CompressedBSAFile::ArchiveFlags`
pub mod archive_flags {
    pub const FOLDER_NAMES: u32 = 0x0001;
    pub const FILE_NAMES: u32 = 0x0002;
    pub const COMPRESS: u32 = 0x0004;
    pub const RETAIN_DIR: u32 = 0x0008;
    pub const RETAIN_NAME: u32 = 0x0010;
    pub const RETAIN_FILE_OFFSET: u32 = 0x0020;
    pub const XBOX360: u32 = 0x0040;
    pub const STARTUP_STR: u32 = 0x0080;
    pub const EMBEDDED_NAMES: u32 = 0x0100;
    pub const XMEM: u32 = 0x0200;
}

/// ファイルサイズフラグ（最上位ビットで圧縮状態をトグル）。
pub const FILE_SIZE_FLAG_COMPRESSION: u32 = 0x4000_0000;

/// Fallout 3 BSA (v104) ヘッダー構造体 (36 bytes)。
///
/// 参照元: OpenMW `CompressedBSAFile::Header`
#[derive(Clone, Debug, PartialEq)]
pub struct BsaHeader {
    /// マジックコード (b"BSA\0")
    pub format: [u8; 4],
    /// バージョン (104 / 0x68)
    pub version: u32,
    /// ディレクトリレコード群の開始オフセット
    pub folders_offset: u32,
    /// アーカイブフラグ (`archive_flags`)
    pub flags: u32,
    /// ディレクトリ総数
    pub folder_count: u32,
    /// ファイル総数
    pub file_count: u32,
    /// ディレクトリ名文字列全体のバイト長
    pub folder_names_length: u32,
    /// ファイル名文字列全体のバイト長
    pub file_names_length: u32,
    /// ファイル種別フラグ (NIF, DDS 等)
    pub file_flags: u32,
}

impl BsaHeader {
    /// ストリームからヘッダーを読み込む。
    pub fn read<R: Read>(reader: &mut R) -> Result<Self, BsaError> {
        let mut format = [0u8; 4];
        reader.read_exact(&mut format)?;
        if &format != b"BSA\0" {
            return Err(BsaError::InvalidMagic(format));
        }

        let version = reader.read_u32::<LittleEndian>()?;
        if version != 104 {
            return Err(BsaError::UnsupportedVersion(version));
        }

        let folders_offset = reader.read_u32::<LittleEndian>()?;
        let flags = reader.read_u32::<LittleEndian>()?;
        let folder_count = reader.read_u32::<LittleEndian>()?;
        let file_count = reader.read_u32::<LittleEndian>()?;
        let folder_names_length = reader.read_u32::<LittleEndian>()?;
        let file_names_length = reader.read_u32::<LittleEndian>()?;
        let file_flags = reader.read_u32::<LittleEndian>()?;

        Ok(BsaHeader {
            format,
            version,
            folders_offset,
            flags,
            folder_count,
            file_count,
            folder_names_length,
            file_names_length,
            file_flags,
        })
    }
}
