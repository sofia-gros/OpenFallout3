//! # fo3_nif
//!
//! Fallout 3 NIF (v20.2.0.7) ファイルパーサー。
//!
//! すべての実装は `references/nifxml/nif.xml` および `references/nifskope` の仕様に厳密に準拠しています。

pub mod blocks;
pub mod file;
pub mod header;
pub mod types;

pub use blocks::*;
pub use file::NifFile;
pub use header::{NifError, NifHeader};
pub use types::*;
