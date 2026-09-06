# Fallout 3 ESM / ESP ファイルフォーマット詳細バイナリ仕様

Fallout 3 (Gamebryo 2.6) におけるマスターファイル (`.esm`) およびプラグインファイル (`.esp`) のバイナリレイアウト仕様。
参照元:
- `references/openmw/components/esm4/reader.hpp`
- `references/openmw/components/esm4/reader.cpp`
- `references/openmw/components/esm4/loadtes4.hpp`
- `references/openmw/components/esm4/loadstat.hpp`
- 実ゲームファイル `Fallout3.esm` のバイナリダンプ検証

---

## 1. 全体構造

ESM/ESP ファイルは、先頭の **`TES4` ヘッダーレコード**、それに続く複数の **`GRUP` (グループ)**、およびグループ内に階層化された **レコード (Record)** と **サブレコード (Subrecord)** で構成されます。

```text
[TES4 Record (24 bytes header + data)]
[GRUP (24 bytes header + children)]
    [Record (24 bytes header + data)]
        [Subrecord (6 bytes header + data)]
        [Subrecord ...]
    [GRUP (Nested child group)]
        ...
[GRUP ...]
```

---

## 2. レコードヘッダー (`RecordHeader`) [24 バイト]

Fallout 3 では全レコードヘッダーは **24 バイト** です（Oblivion は 20 バイトでしたが、FO3 では末尾に 4 バイト追加）。

| オフセット | フィールド名 | 型 | 説明 |
| :--- | :--- | :--- | :--- |
| 0x00 | `type_id` | `[u8; 4]` | 4文字シグネチャ (例: `TES4`, `STAT`, `CELL`, `WRLD`, `REFR`, etc.) |
| 0x04 | `data_size` | `u32` (LE) | データ本体のバイトサイズ（24 バイトのヘッダーを含まない） |
| 0x08 | `flags` | `u32` (LE) | レコードフラグ（下記参照） |
| 0x0C | `form_id` | `u32` (LE) | オブジェクトの 32-bit 一意識別子 (FormID) |
| 0x10 | `vc_info` | `u32` (LE) | バージョン管理タイムスタンプ / ユーザーID |
| 0x14 | `form_version`| `u16` (LE) | 内部フォームバージョン (FO3 では通常 15 = 0x000F) |
| 0x16 | `vc_info2` | `u16` (LE) | バージョン管理情報 2 |

### レコードフラグ (`flags`):
- `0x00000001`: **ESM** (マスターファイルフラグ)
- `0x00000200`: **Deleted** (削除されたレコード)
- `0x00000400`: **Constant / Border**
- `0x00040000`: **Compressed** (zlib 圧縮データ)

---

## 3. グループヘッダー (`GroupHeader` / `GRUP`) [24 バイト]

グループは同種レコードや階層セル、ワールドチルドレンを束ねるコンテナです。ヘッダーは **24 バイト**。

| オフセット | フィールド名 | 型 | 説明 |
| :--- | :--- | :--- | :--- |
| 0x00 | `type_id` | `[u8; 4]` | 常に `b"GRUP"` |
| 0x04 | `group_size` | `u32` (LE) | ヘッダーの 24 バイトを含むグループ全体のバイトサイズ |
| 0x08 | `label` | `[u8; 4]` | グループラベル（`group_type` に応じて解釈が変わる） |
| 0x0C | `group_type` | `i32` (LE) | グループ種別（0〜10） |
| 0x10 | `stamp` | `u16` (LE) | 日付タイムスタンプ |
| 0x12 | `unknown1` | `u16` (LE) | 未知/内部フラグ |
| 0x14 | `version` | `u16` (LE) | バージョン |
| 0x16 | `unknown2` | `u16` (LE) | 未知 |

### グループ種別 (`group_type`):
- `0: Top`: トップレベルグループ。`label` は対象レコードシグネチャ（例: `b"STAT"`, `b"WRLD"`, `b"CELL"`）。
- `1: WorldChildren`: ワールド空間の子。`label` は親 `WRLD` の FormID。
- `2: InteriorCellBlock`: 室内セルブロック。`label` はブロック番号 (`i32`)。
- `3: InteriorCellSubBlock`: 室内セルサブブロック。`label` はサブブロック番号 (`i32`)。
- `4: ExteriorCellBlock`: 室外セルブロック。`label` は `grid_y: i16, grid_x: i16`。
- `5: ExteriorCellSubBlock`: 室外セルサブブロック。`label` は `grid_y: i16, grid_x: i16`。
- `6: CellChildren`: セルの子。`label` は親 `CELL` の FormID。
- `7: TopicChildren`: ダイアログトピックの子。
- `8: CellPersistentChildren`: セル内の常駐オブジェクト群。
- `9: CellTemporaryChildren`: セル内の一時オブジェクト群。
- `10: CellVisibleWhenDistant`: 遠景表示セルオブジェクト群。

---

## 4. サブレコード構造 (`Subrecord`)

レコードデータ内部は 1 つ以上のサブレコードが連続して格納されます。

| オフセット | フィールド名 | 型 | 説明 |
| :--- | :--- | :--- | :--- |
| 0x00 | `type_id` | `[u8; 4]` | 4文字サブレコード名 (例: `EDID`, `DATA`, `MODL`) |
| 0x04 | `data_size` | `u16` (LE) | サブレコードデータのバイト数 |
| 0x06 | `data` | `[u8; data_size]` | サブレコードデータ本体 |

### 巨大サブレコード (`XXXX`):
- `data_size` が 65535 (u16) を超える場合、直前に `type_id == b"XXXX"` のサブレコードが置かれます。
- `XXXX` のデータは `u32` であり、直後に続くサブレコードの真のデータサイズを表します。

---

## 5. 圧縮レコードの展開仕様

`record.flags & 0x00040000 != 0` の場合:
1. レコードデータ先頭の 4 バイトから `uncompressed_size: u32` を読み取る。
2. 続くバイト列 (`data_size - 4`) を zlib デコーダー (`flate2::read::ZlibDecoder`) で `uncompressed_size` バイト展開する。
3. 展開されたバイト列に対して通常のサブレコードパースを実行する。

---

## 6. 基本レコード定義

### ① `TES4` (ファイルヘッダーレコード)
- `HEDR`: `version: f32 (0.94)`, `num_records: i32`, `next_object_id: u32` (12 bytes)
- `CNAM`: 作者名文字列 (`ipely\0` 等)
- `SNAM`: ファイル説明文字列
- `MAST`: マスター依存ファイル名文字列 (例: `Fallout3.esm`)
- `DATA`: マスター依存ファイルサイズ (`u64`)

### ② `STAT` (静的メッシュ定義レコード)
- `EDID`: エディタ識別名 (例: `MegatonHouse01\0`)
- `OBND`: 3D オブジェクト境界ボックス (`min: [i16; 3], max: [i16; 3]`)
- `MODL`: 関連付けられた NIF ファイルパス (例: `meshes\architecture\megaton\megatonhouse01.nif\0`)
- `MODB`: バウンディング半径 (`f32`)
