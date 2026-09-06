# Fallout 3 地形 (LAND レコード) バイナリ仕様 & ハイトマップ展開

本ドキュメントは Fallout 3 (Gamebryo 2.6) の屋外セルにおける地形（ランドスケープ、ハイトマップ、法線、頂点カラー、テクスチャ）に関する詳細バイナリ仕様を記録した永続化知識ベースです。

---

## 1. 参照文献・一次資料
- `references/openmw/components/esm4/loadland.hpp`, `loadland.cpp`: `LAND` レコード構造・サブレコード
- `references/openmw/components/esm/esmterrain.cpp:39-77`: `LAND` 標高データ (`VHGT`) デコード式
- `references/openmw/components/esm4/loadcell.cpp:111`: `CELL` レコードと `LAND` の階層関係

---

## 2. LAND レコードの階層位置

屋外セル（`Interior == false`）のグリッド区画（サイズ: $4096 \times 4096$ 単位）には、最大 1 個の `LAND` レコードが関連付けられます。
`LAND` はセルの子グループ（`CellTemporaryChildren`, `group_type == 9`）の内部に格納されています。

```
GRUP (CellChildren, type=6, label=cell_form_id)
 └── GRUP (CellTemporaryChildren, type=9, label=cell_form_id)
      ├── LAND レコード (type=b"LAND", form_id=XXXX)
      ├── REFR レコード...
      └── ...
```

---

## 3. LAND レコードのサブレコード構成

| サブレコード | サイズ (bytes) | 説明 |
| :--- | :--- | :--- |
| `DATA` | 4 | ランドスケープフラグ (`u32`) |
| `VNML` | 3267 ($33 \times 33 \times 3$) | 頂点法線ベクトル (`[i8; 3]`、正規化範囲 $-128..127$) |
| `VHGT` | 1096 ($4 + 1089 + 3$) | 標高ハイトマップ差分データ（基準オフセット `f32` + $33 \times 33$ 差分 `i8` + 不明 3 バイト） |
| `VCLR` | 3267 ($33 \times 33 \times 3$) | 頂点カラー (`[u8; 3]`、24bit RGB) |
| `BTXT` | 8 | ベーステクスチャ定義（FormID + クアドラント番号 0..3） |
| `ATXT` | 8 | 追加テクスチャレイヤー定義（FormID + クアドラント + レイヤー番号） |
| `VTXT` | 可変 ($8 \times N$) | テクスチャ不透明度マップ（位置インデックス `u16` + 不透明度 `f32`） |

---

## 4. ハイトマップ標高復元計算式 (VHGT)

一次文献: `references/openmw/components/esm/esmterrain.cpp:52-71`

### パラメータ定義
- グリッド解像度: 一辺 $N = 33$ 頂点（総頂点数 $33 \times 33 = 1089$）
- セルの物理サイズ: $4096.0$ ゲーム単位
- 高さスケール係数: `sHeightScale = 8.0`
- グリッドの 1 マス間隔: $4096.0 / 32 = 128.0$ ゲーム単位

### 標高復元アルゴリズム
```rust
let mut heights = [0.0f32; 33 * 33];
let mut row_offset = vhgt.height_offset;

for y in 0..33 {
    row_offset += vhgt.gradient_data[y * 33] as f32;
    heights[y * 33] = row_offset * 8.0;

    let mut col_offset = row_offset;
    for x in 1..33 {
        col_offset += vhgt.gradient_data[y * 33 + x] as f32;
        heights[y * 33 + x] = col_offset * 8.0;
    }
}
```

### 頂点のワールド座標 $(W_x, W_y, W_z)$
セルのグリッド座標が $(grid_x, grid_y)$ の場合:
$$W_x = grid_x \times 4096.0 + x \times 128.0 \quad (x \in 0..32)$$
$$W_y = grid_y \times 4096.0 + y \times 128.0 \quad (y \in 0..32)$$
$$W_z = \text{heights}[y \times 33 + x]$$

### メッシュインデックス生成
$32 \times 32$ 個のクアッド（四角形）をそれぞれ 2 個の三角形に分割:
- クアッド $(x, y)$:
  - 頂点 0: $(x, y) \to y \times 33 + x$
  - 頂点 1: $(x+1, y) \to y \times 33 + x + 1$
  - 頂点 2: $(x, y+1) \to (y+1) \times 33 + x$
  - 頂点 3: $(x+1, y+1) \to (y+1) \times 33 + x + 1$
- 三角形 1: $(0, 2, 1)$
- 三角形 2: $(1, 2, 3)$
- 総三角形数: $32 \times 32 \times 2 = 2048$ 個
- 総インデックス数: $2048 \times 3 = 6144$ 個

### デフォルトテクスチャ
Fallout 3 の屋外荒野のデフォルトテクスチャ:
- ディフューズ: `textures/landscape/dirtwasteland01.dds`
- ノーマル: `textures/landscape/dirtwasteland01_n.dds`
