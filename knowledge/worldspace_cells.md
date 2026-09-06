# Fallout 3 / Gamebryo 2.6 ワールドスペース (WRLD) & 外部セル (CELL) 構造仕様

## 1. 概要

Fallout 3 (Gamebryo 2.6) における屋外環境（Capital Wasteland, Megaton, The Pitt, Point Lookout 等）は、**WRLD (World Space)** レコードとその配下に階層化された **外部セル (CELL)** 群によって構成される。

- 一次文献:
  - references/openmw/components/esm4/loadwrld.hpp / loadwrld.cpp
  - references/openmw/components/esm4/loadcell.hpp / loadcell.cpp
  - references/openmw/components/esm4/grouptype.hpp
  - references/openmw/components/esm4/reader.cpp

---

## 2. ESM グループ階層構造 (World Space & Exterior Cells)

```
Top-Level GRUP (Type 0: REC_WRLD)
  │
  ├── WRLD Record (FormID, EDID="Wasteland", "MegatonWorld", "DCWorld01", etc.)
  │
  └── GRUP (Type 1: World Children, Label = WRLD FormID)
        ├── GRUP (Type 4: Exterior Cell Block, Label = [y: i16, x: i16])
        │     └── GRUP (Type 5: Exterior Sub-Cell Block, Label = [y: i16, x: i16])
        │           ├── CELL Record (FormID, EDID, XCLC: [grid_x: i32, grid_y: i32])
        │           └── GRUP (Type 9: Cell Temporary Children / Type 6: Cell Children, Label = CELL FormID)
        │                 ├── LAND Record (FormID, VHGT: 標高, BTXT: 下地, ATXT/VTXT: レイヤー)
        │                 ├── REFR Record (配置オブジェクト 1)
        │                 ├── REFR Record (配置オブジェクト 2)
        │                 └── ...
        │
        └── GRUP (Type 8: World Persistent Children, Label = WRLD FormID)
              └── (ワールド全体で常に常駐する重要な REFR 群)
```

---

## 3. グリッド座標系と実空間座標系

Gamebryo / Fallout 3 の外部セルは、一辺が 4096.0 Gamebryo ユニット (GU) の正方形グリッドとして管理される。

- 1 セル = 4096.0 x 4096.0 GU
- セルグリッド座標 (X, Y) のワールド原点:
  origin_x = X * 4096.0
  origin_y = Y * 4096.0
- セル (X, Y) の境界範囲:
  [X * 4096.0, (X + 1) * 4096.0] x [Y * 4096.0, (Y + 1) * 4096.0]
- 3 x 3 セル読み込み範囲（中心セル (Xc, Yc)、半径 R=1）:
  X in [Xc - 1, Xc + 1], Y in [Yc - 1, Yc + 1] (計 9 セル)
  総面積: 12288.0 x 12288.0 GU (約 175 x 175 メートル)

---

## 4. 主なワールドスペース EDID 一覧 (Fallout 3)

- Wasteland: 首都キャピタル・ウェイストランド全域（最大ワールド）
- MegatonWorld: メガトン内部（核爆弾を取り囲む集落全体）
- DCWorld01 〜 DCWorld18: ワシントンD.C. 廃墟市街地・モール周辺
- RivetCityWorld: リベットシティ（空母外観・甲板）
- OasisWorld: オアシス
- ParadiseFallsWorld: パラダイス・フォールズ
- TenpennyTowerWorld: テンペニータワー周辺

---

## 5. 地形マルチテクスチャブレンディング (ATXT + VTXT) レイヤリング

1. 下地 (BTXT):
   - 各クアドラント q in {0, 1, 2, 3} に 1 つのベーステクスチャ FormID (LTEX) が割り当てられる。
   - 不透明パスで深度書き込みありで描画。
2. 追加レイヤー (ATXT + VTXT):
   - 各レイヤーは ATXT (form_id: LTEX, quadrant: 0..3, layer_index: 0..7) で指定。
   - VTXT は 17x17 頂点（0..288）の局所インデックスと透過度 opacity: f32 (0.0〜1.0) を持つ。
   - 頂点カラーのアルファチャンネル (color.a = opacity) に透過度を設定し、半透明合成パイプライン (depth_write_enabled: false, depth_compare: LessEqual) で下地メッシュの上に重ねて描画する。
