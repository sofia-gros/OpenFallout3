# BSA アーカイブフォーマット (v104 / Fallout 3) 詳細バイナリ仕様

- **対象ゲーム**: Fallout 3 / Oblivion / New Vegas
- **BSA バージョン**: `104` (`0x68`)
- **マジック**: `BSA\0` (`0x00415342` LE)
- **参照元**:
  - `references/openmw/components/bsa/compressedbsafile.hpp`
  - `references/openmw/components/bsa/compressedbsafile.cpp`

---

## 1. ヘッダー構造 (`Header`, 36 bytes)

| フィールド名 | 型 | サイズ | 説明 |
| :--- | :--- | :--- | :--- |
| `format` | `[u8; 4]` | 4 | `"BSA\0"` |
| `version` | `u32` (LE) | 4 | `104` (`0x68`) |
| `folders_offset` | `u32` (LE) | 4 | ディレクトリレコード開始オフセット（通常 36） |
| `flags` | `u32` (LE) | 4 | アーカイブフラグ（ビットマスク） |
| `folder_count` | `u32` (LE) | 4 | 含まれるディレクトリの総数 |
| `file_count` | `u32` (LE) | 4 | 含まれるファイルの総数 |
| `folder_names_length` | `u32` (LE) | 4 | ディレクトリ名全体のバイト長（null終端・長さバイト含む） |
| `file_names_length` | `u32` (LE) | 4 | ファイル名全体のバイト長（null終端含む） |
| `file_flags` | `u32` (LE) | 4 | ファイル種別フラグ (NIF, DDS, WAV 等) |

### アーカイブフラグ (`ArchiveFlags`)
- `0x0001`: `FolderNames`（ディレクトリ名が含まれる）
- `0x0002`: `FileNames`（ファイル名が含まれる）
- `0x0004`: `Compress`（アーカイブ全体がデフォルトで圧縮されている）
- `0x0100`: `EmbeddedNames`（ファイルデータ先頭に名前プレフィックスが埋め込まれている）

---

## 2. ディレクトリレコード配列 (`FolderRecord[]`)

ヘッダー直後（`folders_offset`）から、`folder_count` 個のレコードが連続します。

```rust
pub struct FolderRecord {
    pub hash: u64,
    pub count: u32,
    pub offset: u32,
}
```

---

## 3. ファイルレコードブロック群

ディレクトリレコード配列の直後から、各ディレクトリに対応するブロックが連続します。
各ブロックは以下で構成されます:

1. **ディレクトリ名 (Folder Name)** (`flags & FolderNames != 0` の場合):
   - `len: u8` (文字列長 + null終端のバイト数)
   - `name: [u8; len - 1]` (null終端を除く ASCII 文字列)
   - 末尾 `0x00` をスキップ
2. **ファイルレコード配列 (`FileRecord[count]`)**:
   ```rust
   pub struct FileRecord {
       pub hash: u64,
       pub size: u32,   // 最上位ビット 0x40000000 は圧縮トグルフラグ
       pub offset: u32, // ファイルデータへの絶対オフセット
   }
   ```

---

## 4. ファイル名テーブル (`FileNameTable`)

全ディレクトリのファイルレコードブロックの直後から、`file_names_length` バイトの null 終端文字列が全ファイル分（`file_count` 個）並んでいます。
読み込んだファイル名とディレクトリ名を `\` で結合することで、完全なファイルパス（例: `meshes\weapons\pistol.nif`）が復元されます。

---

## 5. ファイルデータの抽出と解凍

各 `FileRecord` のオフセット位置からデータを読み出します。

### 圧縮判定:
- `raw_size = file_record.size & 0x3FFF_FFFF`
- `has_size_flag = (file_record.size & 0x4000_0000) != 0`
- `is_compressed = has_size_flag ^ (header.flags & Compress != 0)`

### 解凍処理:
- `is_compressed == true` の場合:
  1. 先頭 4 バイト: 解凍後の展開サイズ (`uncompressed_size: u32` LE)
  2. 残り `raw_size - 4` バイト: zlib 圧縮データストリーム
  3. `flate2::read::ZlibDecoder` を用いて `uncompressed_size` バイトに解凍展開。
- `is_compressed == false` の場合:
  - `raw_size` バイトを生データとしてそのまま読み出す。

---

## 6. ハッシュ計算関数 (`generate_hash`)
OpenMW C++ 実装（`compressedbsafile.cpp:L330-L368`）に完全準拠:
- パス区切り文字 `/` は `\` に正規化。
- 末尾文字、長さ、先頭文字、および中間文字列の多項式ハッシュ (`hash * 0x1003F + c`)。
- 拡張子（`.nif` -> `| 0x8000`, `.dds` -> `| 0x8080` 等）のビットフラグ合成。
