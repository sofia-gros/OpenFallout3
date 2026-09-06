# NIF ファイルフォーマット (v20.2.0.7 / Fallout 3) 詳細バイナリ仕様

- **対象ゲーム**: Fallout 3
- **NIF バージョン**: `20.2.0.7` (16進数: `0x14020007`)
- **User Version**: `11`
- **User Version 2 (BS Version)**: `34`
- **エンディアン**: Little Endian (`0`)
- **参照元**:
  - `references/nifxml/nif.xml:L1963-L1983` (`Header`)
  - `references/nifxml/nif.xml:L1952-L1960` (`BSStreamHeader`)
  - `references/nifxml/nif.xml:L1889-L1893` (`ExportString`)
  - `references/nifskope/src/niftypes.h`

---

## 1. ヘッダーバイナリレイアウト

Fallout 3 の NIF ファイルの先頭は、以下の順序で厳密に配置されています。

| フィールド名 | 型 | サイズ (bytes) | 説明・値 |
| :--- | :--- | :--- | :--- |
| **Header String** | ASCII (`\n` 終端) | 可変 (~39B) | `'Gamebryo File Format, Version 20.2.0.7\n'` |
| **Version** | `u32` (LE) | 4 | `0x14020007` (20.2.0.7) |
| **Endian Type** | `u8` | 1 | `0`: ENDIAN_BIG, `1`: ENDIAN_LITTLE (PC / x86) |
| **User Version** | `u32` (LE) | 4 | `11` (Bethesda Fallout 3 / New Vegas) |
| **Num Blocks** | `u32` (LE) | 4 | ファイル内に含まれる全ブロック数 |
| **BS Header** | `BSStreamHeader` | 可変 | Bethesda 固有エクスポートヘッダー（下記参照） |
| **Num Block Types** | `u16` (LE) | 2 | このファイル内で使用されているブロック型の総数 |
| **Block Types** | `SizedString[]` | 可変 | 使用ブロック型名文字列配列（各文字列は `len: u32` + `bytes`） |
| **Block Type Index** | `u16[]` | `Num Blocks * 2` | 各ブロックがどの型であるかを示すインデックス配列 |
| **Block Size** | `u32[]` | `Num Blocks * 4` | 各ブロックのバイナリサイズ配列（v20.2.0.5以降で追加） |
| **Num Strings** | `u32` (LE) | 4 | グローバル文字列プール内の文字列数 (v20.1.0.1以降) |
| **Max String Length** | `u32` (LE) | 4 | グローバル文字列プール内の最長文字列の長さ |
| **Strings** | `SizedString[]` | 可変 | グローバル文字列プール配列 |
| **Num Groups** | `u32` (LE) | 4 | 通常 `0` |

### 1.1 `BSStreamHeader` の詳細 (`#BSSTREAMHEADER#` 条件合致)
Fallout 3 では `ver == 20.2.0.7 && user_version == 11` であるため、必ず存在します。

| フィールド名 | 型 | 説明 |
| :--- | :--- | :--- |
| **BS Version** | `u32` (LE) | 通常 `34` (Fallout 3) |
| **Author** | `ExportString` | `len: u8` + `chars: [u8; len]` (末尾 null 含む) |
| **Process Script** | `ExportString` | `BS Version < 131` のため存在 |
| **Export Script** | `ExportString` | エクスポートスクリプト名 |

---

## 2. ブロック参照（`Ref<T>` / `Ptr<T>`）の仕様

Gamebryo の NIF は、オブジェクト間のポインタを整数インデックスとしてシリアライズします。

- **`Ref<T>` / `Ptr<T>`**: `i32` (32-bit 符号付き整数)
  - `>= 0`: `Header.Block Type Index` のインデックス位置にあるブロックへの参照。
  - `-1`: `None` (Null ポインタ)。

---

## 3. グローバル文字列インデックス (`IndexString`)

Fallout 3 (v20.1.0.1以降) では、各ブロック内のノード名やテクスチャ名などの文字列は、直接文字列として格納されず、`Header.Strings` に対する `u32` インデックスとして格納されます。

- **`IndexString` / `string`**: `u32`
  - `Header.Strings[index]` から実際の文字列を取得。
