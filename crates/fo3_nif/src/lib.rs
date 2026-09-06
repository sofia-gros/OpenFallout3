//! # fo3_nif
//!
//! Fallout 3 NIF (v20.2.0.7) ファイルパーサー。
//!
//! すべての実装は `references/nifxml/nif.xml` および `references/nifskope` の仕様に厳密に準拠しています。

pub mod header;

pub use header::{NifHeader, NifError};
