//! # BSA 64-bit ハッシュ生成モジュール
//!
//! 参照元: `references/openmw/components/bsa/compressedbsafile.cpp:L330-L368`
//!
//! Bethesda BSA (v104) におけるディレクトリ名およびファイル名の 64-bit ハッシュを計算します。

/// 単一の文字列と拡張子から BSA 互換の 64-bit ハッシュを計算する。
///
/// 参照元: OpenMW `CompressedBSAFile::generateHash`
pub fn generate_hash(str: &str, extension: &str) -> u64 {
    let s = str.to_ascii_lowercase();
    let ext = extension.to_ascii_lowercase();

    if s.is_empty() {
        return 0;
    }

    let bytes = s.as_bytes();
    let len = bytes.len();

    // '/' を '\\' に置換して文字取得
    let at = |i: usize| -> u8 {
        let b = bytes[i];
        if b == b'/' {
            b'\\'
        } else {
            b
        }
    };

    let mut result: u64 = at(len - 1) as u64;
    if len >= 3 {
        result |= (at(len - 2) as u64) << 8;
    }
    result |= (len as u64) << 16;
    result |= (at(0) as u64) << 24;

    if len >= 4 {
        let mut hash: u32 = 0;
        for i in 1..=(len - 3) {
            hash = hash.wrapping_mul(0x1003F).wrapping_add(at(i) as u32);
        }
        result = result.wrapping_add((hash as u64) << 32);
    }

    if ext.is_empty() {
        return result;
    }

    // 拡張子フラグ
    match ext.as_str() {
        ".kf" => result |= 0x80,
        ".nif" => result |= 0x8000,
        ".dds" => result |= 0x8080,
        ".wav" => result |= 0x8000_0000,
        _ => {}
    }

    let mut ext_hash: u32 = 0;
    for &b in ext.as_bytes() {
        ext_hash = ext_hash.wrapping_mul(0x1003F).wrapping_add(b as u32);
    }
    result = result.wrapping_add((ext_hash as u64) << 32);

    result
}

/// ディレクトリパスのハッシュを計算する。
pub fn hash_folder(folder_path: &str) -> u64 {
    generate_hash(folder_path, "")
}

/// ファイル名（stem + ext）のハッシュを計算する。
/// 例: `"10mmpistol.nif"` -> stem `"10mmpistol"`, ext `".nif"`
pub fn hash_filename(filename: &str) -> u64 {
    if let Some(pos) = filename.rfind('.') {
        let stem = &filename[..pos];
        let ext = &filename[pos..];
        generate_hash(stem, ext)
    } else {
        generate_hash(filename, "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_calculation() {
        // 基本的なハッシュが非ゼロで一貫して算出されることを確認
        let h1 = hash_folder("meshes");
        assert_ne!(h1, 0);

        let h2 = hash_folder("meshes\\weapons");
        assert_ne!(h2, 0);
        assert_ne!(h1, h2);

        // 大文字小文字の正規化
        assert_eq!(hash_folder("Meshes\\Weapons"), hash_folder("meshes\\weapons"));
        // スラッシュの正規化
        assert_eq!(hash_folder("meshes/weapons"), hash_folder("meshes\\weapons"));

        // ファイル名ハッシュ
        let f_hash = hash_filename("10mmpistol.nif");
        assert_ne!(f_hash, 0);
        assert_eq!(hash_filename("10MMPISTOL.NIF"), f_hash);
    }
}
