//! # VFS エラー型定義
//!
//! 参照元: OpenMW `components/vfs`

use std::io;
use fo3_bsa::BsaError;
use thiserror::Error;

/// 仮想ファイルシステム操作時のエラー型。
#[derive(Debug, Error)]
pub enum VfsError {
    #[error("I/O エラー: {0}")]
    Io(#[from] io::Error),

    #[error("BSA アーカイブエラー: {0}")]
    Bsa(#[from] BsaError),

    #[error("VFS 内にファイルが見つかりません: {0}")]
    NotFound(String),
}
