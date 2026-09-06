//! # 仮想ファイルシステム (VFS) マネージャー
//!
//! 参照元:
//! - `references/openmw/components/vfs/manager.hpp`
//! - `references/openmw/components/vfs/manager.cpp`

use std::fs;
use std::path::{Path, PathBuf};
use fo3_bsa::{BsaArchive, BsaError};
use crate::error::VfsError;

/// パス区切り文字を正規化し、小文字化する。
pub fn normalize_path(path: &str) -> String {
    path.replace('/', "\\").to_ascii_lowercase()
}

/// 仮想ファイルシステム (VFS) マネージャー。
///
/// ディスク上のルーズファイル（最優先）と、複数の BSA アーカイブ（マウント順）を
/// 透過的に統合し、単一のファイルツリーとしてアクセスを提供します。
pub struct VfsManager {
    /// ルーズファイル検索ルートパス一覧（優先度順）
    loose_roots: Vec<PathBuf>,
    /// マウントされた BSA アーカイブ一覧（後に追加されたものが優先）
    bsa_archives: Vec<BsaArchive>,
}

impl Default for VfsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl VfsManager {
    /// 空の VFS マネージャーを生成する。
    pub fn new() -> Self {
        Self {
            loose_roots: Vec::new(),
            bsa_archives: Vec::new(),
        }
    }

    /// ルーズファイル用のルートディレクトリを追加する（先に追加されたディレクトリが優先）。
    pub fn add_loose_root<P: AsRef<Path>>(&mut self, root: P) {
        self.loose_roots.push(root.as_ref().to_path_buf());
    }

    /// BSA アーカイブをマウントする（後に追加されたアーカイブが優先）。
    pub fn add_bsa(&mut self, archive: BsaArchive) {
        self.bsa_archives.push(archive);
    }

    /// 指定された相対パスのファイルが存在するか判定する。
    pub fn exists(&self, relative_path: &str) -> bool {
        let normalized = normalize_path(relative_path);

        // 1. ルーズファイルをチェック
        for root in &self.loose_roots {
            if Self::find_loose_file(root, &normalized).is_some() {
                return true;
            }
        }

        // 2. BSA アーカイブを逆順（新しい順）にチェック
        for bsa in self.bsa_archives.iter().rev() {
            for f in bsa.list_files() {
                if normalize_path(f) == normalized {
                    return true;
                }
            }
        }

        false
    }

    /// 相対パスを指定してファイルデータを読み込む。
    /// ルーズファイル（ディスク）を最優先で探索し、存在しない場合はマウントされた BSA から抽出・解凍します。
    pub fn read(&mut self, relative_path: &str) -> Result<Vec<u8>, VfsError> {
        let normalized = normalize_path(relative_path);

        // 1. ルーズファイルを探索
        for root in &self.loose_roots {
            if let Some(actual_path) = Self::find_loose_file(root, &normalized) {
                return Ok(fs::read(actual_path)?);
            }
        }

        // 2. BSA アーカイブを逆順（優先順）に探索
        for bsa in self.bsa_archives.iter_mut().rev() {
            match bsa.extract_file(&normalized) {
                Ok(data) => return Ok(data),
                Err(BsaError::FileNotFound(_)) => continue,
                Err(e) => return Err(VfsError::Bsa(e)),
            }
        }

        Err(VfsError::NotFound(relative_path.to_string()))
    }

    /// ディスク上で大文字小文字を区別せずルーズファイルを検索する。
    fn find_loose_file(root: &Path, normalized_relative: &str) -> Option<PathBuf> {
        let full = root.join(normalized_relative);
        if full.exists() {
            return Some(full);
        }

        // 大文字小文字の違いに対応するため、ディレクトリ階層を順番に走査
        let components: Vec<&str> = normalized_relative.split('\\').filter(|c| !c.is_empty()).collect();
        let mut current = root.to_path_buf();

        for component in components {
            if !current.is_dir() {
                return None;
            }

            let entries = fs::read_dir(&current).ok()?;
            let mut matched = None;
            for entry in entries.flatten() {
                let name = entry.file_name();
                if name.to_string_lossy().to_ascii_lowercase() == component {
                    matched = Some(entry.path());
                    break;
                }
            }

            match matched {
                Some(next_path) => current = next_path,
                None => return None,
            }
        }

        if current.is_file() {
            Some(current)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_vfs_loose_override() {
        let temp_dir = std::env::temp_dir().join("fo3_vfs_test");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("meshes")).unwrap();

        let test_file = temp_dir.join("meshes").join("test.nif");
        let mut f = fs::File::create(&test_file).unwrap();
        f.write_all(b"LOOSE_DATA").unwrap();

        let mut vfs = VfsManager::new();
        vfs.add_loose_root(&temp_dir);

        // スラッシュ・大文字小文字混在の検索
        assert!(vfs.exists("Meshes/Test.NIF"));
        let data = vfs.read("Meshes/Test.NIF").unwrap();
        assert_eq!(&data, b"LOOSE_DATA");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
