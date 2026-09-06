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

---

## 4. シェーダープロパティの継承チェーンとバイナリレイアウト

`BSShaderPPLightingProperty` (Fallout 3 ピクセルパーピクセルライティング) は以下の順序でシリアライズされます:

1. **`NiObjectNET`**:
   - `name`: `u32` (IndexString)
   - `num_extra_data`: `u32`
   - `extra_data`: `[i32; num_extra_data]` (Ref)
   - `controller`: `i32` (Ref)
2. **`NiProperty`**: (追加フィールドなし)
3. **`NiShadeProperty`**:
   - `flags`: `u16` (`ShadeFlags`, FO3 では必ず存在)
4. **`BSShaderProperty`**:
   - `shader_type`: `u32` (`BSShaderType`, 1 = SHADER_DEFAULT)
   - `shader_flags`: `u32` (`BSShaderFlags`)
   - `shader_flags2`: `u32` (`BSShaderFlags2`)
   - `env_map_scale`: `f32`
5. **`BSShaderLightingProperty`**:
   - `texture_clamp_mode`: `u32` (`TexClampMode`)
6. **`BSShaderPPLightingProperty`**:
   - `texture_set`: `i32` (`BSShaderTextureSet` へのブロック参照)
   - `refraction_strength`: `f32`
   - `refraction_fire_period`: `i32`
   - `parallax_max_passes`: `f32`
   - `parallax_scale`: `f32`

---

## 5. ジオメトリデータブロックの構造差異 (`NiTriShapeData` vs `NiTriStripsData`)

Fallout 3 における共通ジオメトリ `NiGeometryData`:
- `group_id`: `i32`
- `num_vertices`: `u16`
- `keep_flags`: `u8`
- `compress_flags`: `u8`
- `has_vertices`: `u8` (bool)
- `vertices`: `[Vector3; num_vertices]`
- `bs_data_flags`: `u16` (0x1001 など。ビット 0: UV数、ビット 12: タンジェント/バイタンジェント有無)
- `has_normals`: `u8` (bool)
- `normals`: `[Vector3; num_vertices]`
- `tangents` & `bitangents`: `[Vector3; num_vertices]` (bs_data_flags & 0x1000 の場合)
- `bounding_sphere`: `center: Vector3, radius: f32`
- `has_vertex_colors`: `u8` (bool)
- `vertex_colors`: `[Color4; num_vertices]`
- `uv_sets`: `[[TexCoord; num_vertices]; bs_data_flags & 1]`
- `consistency_flags`: `u16`
- `additional_data`: `i32`

### 末尾ポリゴンデータ構造:
- **`NiTriShapeData`**:
  - `num_triangles`: `u16`
  - `num_triangle_points`: `u32`
  - `has_triangles`: `u8`
  - `triangles`: `[Triangle(v1, v2, v3: u16); num_triangles]`
  - `num_match_groups`: `u16`
  - `match_groups`: 各グループのカウント (`u16`) + 頂点インデックス (`[u16; count]`)
- **`NiTriStripsData`**:
  - `num_triangles`: `u16`
  - `num_strips`: `u16`
  - `strip_lengths`: `[u16; num_strips]`
  - `has_points`: `u8`
  - `strips`: 各ストリップの頂点配列 (`[u16; strip_lengths[i]]`)
  - **Match Groups は存在しない**（OpenMW `data.cpp:188` に準拠）
