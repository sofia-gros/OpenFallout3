//! # BSA アーカイブリーダーおよびファイル抽出
//!
//! 参照元:
//! - `references/openmw/components/bsa/compressedbsafile.cpp:L67-L328`
//! - `knowledge/bsa_v104_format.md`

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use byteorder::{LittleEndian, ReadBytesExt};
use flate2::read::ZlibDecoder;

use crate::hash::{hash_filename, hash_folder};
use crate::header::{archive_flags, BsaError, BsaHeader, FILE_SIZE_FLAG_COMPRESSION};

/// アーカイブ内の単一ファイルレコード。
#[derive(Clone, Debug)]
pub struct BsaFileEntry {
    /// 64-bit ファイル名ハッシュ
    pub hash: u64,
    /// ファイルサイズ（最上位ビットは圧縮トグルフラグ）
    pub size: u32,
    /// ファイルデータ開始位置（アーカイブ先頭からの絶対オフセット）
    pub offset: u32,
    /// フルパス（例: "meshes\\weapons\\10mmpistol\\10mmpistol.nif"）
    pub path: String,
}

/// ディレクトリ内部の情報。
#[derive(Clone, Debug)]
struct BsaFolderEntry {
    #[allow(dead_code)]
    pub name: String,
    /// ファイルハッシュ -> ファイルエントリ
    pub files: HashMap<u64, BsaFileEntry>,
}

/// BSA アーカイブリーダー。
pub struct BsaArchive {
    reader: BufReader<File>,
    header: BsaHeader,
    /// フォルダハッシュ -> フォルダエントリ
    folders: HashMap<u64, BsaFolderEntry>,
    /// 全ファイルパスのリスト
    all_files: Vec<String>,
}

impl BsaArchive {
    /// 指定されたパスの BSA アーカイブを開き、ファイルインデックスを展開する。
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, BsaError> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        // 1. ヘッダー読み込み
        let header = BsaHeader::read(&mut reader)?;

        // 2. ディレクトリレコードの読み込み
        reader.seek(SeekFrom::Start(header.folders_offset as u64))?;

        struct TempFolder {
            hash: u64,
            count: u32,
            _offset: u32,
        }

        let mut temp_folders = Vec::with_capacity(header.folder_count as usize);
        for _ in 0..header.folder_count {
            let hash = reader.read_u64::<LittleEndian>()?;
            let count = reader.read_u32::<LittleEndian>()?;
            let offset = reader.read_u32::<LittleEndian>()?;
            temp_folders.push(TempFolder { hash, count, _offset: offset });
        }

        // 3. 各フォルダの名前とファイルレコード群の読み込み
        struct FolderWithFiles {
            hash: u64,
            name: String,
            files: Vec<(u64, u32, u32)>, // (hash, size, offset)
        }

        let mut folders_with_files = Vec::with_capacity(header.folder_count as usize);

        for tf in temp_folders {
            let folder_name = if (header.flags & archive_flags::FOLDER_NAMES) != 0 {
                let name_len = reader.read_u8()? as usize;
                if name_len > 0 {
                    let mut name_buf = vec![0u8; name_len];
                    reader.read_exact(&mut name_buf)?;
                    // null終端文字を除去
                    if let Some(&0) = name_buf.last() {
                        name_buf.pop();
                    }
                    String::from_utf8_lossy(&name_buf).to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            let mut files = Vec::with_capacity(tf.count as usize);
            for _ in 0..tf.count {
                let file_hash = reader.read_u64::<LittleEndian>()?;
                let file_size = reader.read_u32::<LittleEndian>()?;
                let file_offset = reader.read_u32::<LittleEndian>()?;
                files.push((file_hash, file_size, file_offset));
            }

            folders_with_files.push(FolderWithFiles {
                hash: tf.hash,
                name: folder_name,
                files,
            });
        }

        // 4. ファイル名テーブルの読み込み（flags & FILE_NAMES != 0 の場合）
        let mut all_files = Vec::with_capacity(header.file_count as usize);
        let mut folders = HashMap::with_capacity(header.folder_count as usize);

        if (header.flags & archive_flags::FILE_NAMES) != 0 {
            // ファイル名テーブルから null 終端文字列を連続読み込み
            for fw in folders_with_files {
                let mut file_map = HashMap::with_capacity(fw.files.len());

                for (f_hash, f_size, f_offset) in fw.files {
                    let mut name_bytes = Vec::new();
                    loop {
                        let b = reader.read_u8()?;
                        if b == 0 {
                            break;
                        }
                        name_bytes.push(b);
                    }
                    let file_name = String::from_utf8_lossy(&name_bytes).to_string();

                    let full_path = if fw.name.is_empty() {
                        file_name
                    } else {
                        format!("{}\\{}", fw.name, file_name)
                    };

                    all_files.push(full_path.clone());

                    file_map.insert(
                        f_hash,
                        BsaFileEntry {
                            hash: f_hash,
                            size: f_size,
                            offset: f_offset,
                            path: full_path,
                        },
                    );
                }

                folders.insert(
                    fw.hash,
                    BsaFolderEntry {
                        name: fw.name,
                        files: file_map,
                    },
                );
            }
        } else {
            // ファイル名が含まれない場合
            for fw in folders_with_files {
                let mut file_map = HashMap::with_capacity(fw.files.len());
                for (f_hash, f_size, f_offset) in fw.files {
                    file_map.insert(
                        f_hash,
                        BsaFileEntry {
                            hash: f_hash,
                            size: f_size,
                            offset: f_offset,
                            path: String::new(),
                        },
                    );
                }
                folders.insert(
                    fw.hash,
                    BsaFolderEntry {
                        name: fw.name,
                        files: file_map,
                    },
                );
            }
        }

        Ok(BsaArchive {
            reader,
            header,
            folders,
            all_files,
        })
    }

    /// ヘッダー情報を取得する。
    pub fn header(&self) -> &BsaHeader {
        &self.header
    }

    /// アーカイブ内に含まれる全ファイルパス一覧を返す。
    pub fn list_files(&self) -> &[String] {
        &self.all_files
    }

    /// 相対パスを指定してファイルデータを抽出し、必要に応じて解凍してバッファで返す。
    ///
    /// 参照元: OpenMW `CompressedBSAFile::getFile`
    pub fn extract_file(&mut self, relative_path: &str) -> Result<Vec<u8>, BsaError> {
        let normalized = relative_path.replace('/', "\\");
        let (folder_part, file_part) = match normalized.rfind('\\') {
            Some(pos) => (&normalized[..pos], &normalized[pos + 1..]),
            None => ("", normalized.as_str()),
        };

        let f_hash = hash_folder(folder_part);
        let folder_entry = self
            .folders
            .get(&f_hash)
            .ok_or_else(|| BsaError::FileNotFound(relative_path.to_string()))?;

        let file_hash = hash_filename(file_part);
        let file_entry = folder_entry
            .files
            .get(&file_hash)
            .ok_or_else(|| BsaError::FileNotFound(relative_path.to_string()))?;

        let mut size = (file_entry.size & !FILE_SIZE_FLAG_COMPRESSION) as usize;
        let compressed = (file_entry.size & FILE_SIZE_FLAG_COMPRESSION != 0)
            ^ ((self.header.flags & archive_flags::COMPRESS) != 0);

        self.reader.seek(SeekFrom::Start(file_entry.offset as u64))?;

        // EmbeddedNames フラグがある場合、先頭のファイル名バイトをスキップ
        if (self.header.flags & archive_flags::EMBEDDED_NAMES) != 0 {
            let name_len = self.reader.read_u8()? as usize;
            let mut skip_buf = vec![0u8; name_len];
            self.reader.read_exact(&mut skip_buf)?;
            size = size.saturating_sub(name_len + 1);
        }

        if compressed {
            // 先頭 4 バイトは展開後サイズ
            let uncompressed_size = self.reader.read_u32::<LittleEndian>()? as usize;
            size = size.saturating_sub(4);

            let mut compressed_buf = vec![0u8; size];
            self.reader.read_exact(&mut compressed_buf)?;

            let mut decoder = ZlibDecoder::new(&compressed_buf[..]);
            let mut output = Vec::with_capacity(uncompressed_size);
            decoder
                .read_to_end(&mut output)
                .map_err(|e| BsaError::DecompressError(e.to_string()))?;

            Ok(output)
        } else {
            let mut raw_buf = vec![0u8; size];
            self.reader.read_exact(&mut raw_buf)?;
            Ok(raw_buf)
        }
    }
}
