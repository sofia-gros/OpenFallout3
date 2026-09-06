//! # fo3_vfs
//!
//! Fallout 3 仮想ファイルシステム (Virtual File System) ライブラリ。
//!
//! 参照元:
//! - `references/openmw/components/vfs`

pub mod error;
pub mod manager;

pub use error::VfsError;
pub use manager::{normalize_path, VfsManager};
