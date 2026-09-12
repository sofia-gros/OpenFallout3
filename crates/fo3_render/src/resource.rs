//! Gamebryo 2.6 準拠 リソースキャッシュマネージャ
//!
//! 参照元: Gamebryo 2.6 `NiStream`, `NiSourceTexture`, OpenMW `components/resource/` (`niffilemanager.hpp`, `imagemanager.hpp`)
//!
//! パース済み NIF AST (`NifFile`) および GPU アップロード済みテクスチャ (`GpuTexture`) を
//! 正規化されたファイルパスをキーとしてメモリ上にキャッシュし、セル間遷移や複数オブジェクト間での
//! ゼロコスト共有を実現します。

use std::collections::HashMap;
use std::sync::Arc;
use fo3_vfs::VfsManager;
use fo3_nif::NifFile;
use crate::texture::GpuTexture;

/// メッシュファイルパスを小文字化・バックスラッシュ統一し、先頭に `meshes\` を補完して正規化する。
pub fn normalize_mesh_path(raw_path: &str) -> String {
    let lower = raw_path.trim().to_ascii_lowercase().replace('/', "\\");
    if lower.starts_with("meshes\\") {
        lower
    } else {
        format!("meshes\\{}", lower)
    }
}

/// テクスチャファイルパスを小文字化・バックスラッシュ統一し、先頭に `textures\` を補完して正規化する。
pub fn normalize_texture_path(raw_path: &str) -> String {
    let lower = raw_path.trim().to_ascii_lowercase().replace('/', "\\");
    if lower.starts_with("textures\\") {
        lower
    } else {
        format!("textures\\{}", lower)
    }
}

/// パース済み NIF ファイルのメモリキャッシュ。
///
/// 参照元: OpenMW `NifFileManager`, Gamebryo 2.6 `NiStream`
#[derive(Default)]
pub struct NifCache {
    cache: HashMap<String, Arc<NifFile>>,
}

impl NifCache {
    /// 新しい空の NIF キャッシュを生成する。
    pub fn new() -> Self {
        Self::default()
    }

    /// キャッシュから NIF ファイルを取得する。存在しない場合は VFS から読み込んでパースし、キャッシュに格納する。
    pub fn get_or_load(&mut self, path: &str, vfs: &mut VfsManager) -> Result<Arc<NifFile>, String> {
        let key = normalize_mesh_path(path);
        if let Some(nif) = self.cache.get(&key) {
            return Ok(Arc::clone(nif));
        }

        let bytes = vfs
            .read(&key)
            .map_err(|e| format!("VFS 読み込み失敗 '{}': {:?}", key, e))?;

        let mut cursor = std::io::Cursor::new(&bytes);
        let nif = NifFile::read(&mut cursor)
            .map_err(|e| format!("NIF パース失敗 '{}': {:?}", key, e))?;

        let arc_nif = Arc::new(nif);
        self.cache.insert(key, Arc::clone(&arc_nif));
        Ok(arc_nif)
    }

    /// 指定キーの NIF がキャッシュに存在するか判定する。
    pub fn contains(&self, path: &str) -> bool {
        self.cache.contains_key(&normalize_mesh_path(path))
    }

    /// キャッシュされている NIF の件数を返す。
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// キャッシュが空であるか判定する。
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

/// GPU メモリ上にアップロードされたテクスチャの共有キャッシュ。
///
/// 参照元: Gamebryo 2.6 `NiSourceTexture`, OpenMW `ImageManager`
#[derive(Default)]
pub struct TextureCache {
    cache: HashMap<String, Arc<GpuTexture>>,
}

impl TextureCache {
    /// 新しい空のテクスチャキャッシュを生成する。
    pub fn new() -> Self {
        Self::default()
    }

    /// キャッシュから GPU テクスチャを取得する。存在しない場合は VFS から DDS を読み込んで GPU にアップロードし、キャッシュに格納する。
    pub fn get_or_load(
        &mut self,
        path: &str,
        vfs: &mut VfsManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Arc<GpuTexture>, String> {
        let key = normalize_texture_path(path);
        if let Some(tex) = self.cache.get(&key) {
            return Ok(Arc::clone(tex));
        }

        let bytes = vfs
            .read(&key)
            .map_err(|e| format!("VFS テクスチャ読み込み失敗 '{}': {:?}", key, e))?;

        let tex = GpuTexture::from_dds_bytes(device, queue, &bytes, Some(&key))
            .map_err(|e| format!("DDS デコード・GPU アップロード失敗 '{}': {:?}", key, e))?;

        let arc_tex = Arc::new(tex);
        self.cache.insert(key, Arc::clone(&arc_tex));
        Ok(arc_tex)
    }

    /// 指定キーのテクスチャがキャッシュに存在するか判定する。
    pub fn contains(&self, path: &str) -> bool {
        self.cache.contains_key(&normalize_texture_path(path))
    }

    /// キャッシュされているテクスチャの件数を返す。
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// キャッシュが空であるか判定する。
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}
