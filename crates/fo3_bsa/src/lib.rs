//! # fo3_bsa
//!
//! Fallout 3 (BSA v104) アーカイブパーサーおよびファイル抽出ライブラリ。
//!
//! 参照元:
//! - `references/openmw/components/bsa/compressedbsafile.hpp`
//! - `references/openmw/components/bsa/compressedbsafile.cpp`
//! - `knowledge/bsa_v104_format.md`

pub mod hash;
pub mod header;
pub mod reader;

pub use hash::{generate_hash, hash_filename, hash_folder};
pub use header::{archive_flags, BsaError, BsaHeader};
pub use reader::{BsaArchive, BsaFileEntry};
