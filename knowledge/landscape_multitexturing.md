# Fallout 3 / Gamebryo 2.6 地形マルチテクスチャブレンディング (Landscape Multitexturing) 仕様

## 1. 概要

Fallout 3 の屋外景観 (`LAND`) は、ハイトマップ（33x33 頂点グリッド）上に最大複数レイヤーの地形テクスチャ（岩、土、枯れ草、アスファルト等）をブレンドして描画する。

- 一次文献:
  - `references/openmw/components/esm4/loadland.hpp` / `loadland.cpp`
  - `references/openmw/components/esm4/loadltex.hpp` / `loadltex.cpp`
  - `references/openmw/components/esm4/loadtxst.hpp` / `loadtxst.cpp`
  - `references/openmw/components/esm/esmterrain.cpp`

---

## 2. ESM レコード参照関係

```
LAND (地形レコード)
  ├── BTXT (下地テクスチャ FormId) ──────────┐
  └── ATXT + VTXT (追加レイヤー FormId) ─────┤
                                             ▼
                                      LTEX (Land Texture)
                                        └── TNAM (FormId) ───► TXST (Texture Set)
                                                                 ├── TX00 (Diffuse DDS)
                                                                 └── TX01 (Normal Map DDS)
```

---

## 3. レコード構造

### ① TXST (Texture Set) レコード
- `EDID`: エディタ ID
- `TX00`: ディフューズテクスチャ画像パス (`textures\landscape\*.dds`)
- `TX01`: 法線マップ画像パス (`textures\landscape\*_n.dds`)

### ② LTEX (Land Texture) レコード
- `EDID`: エディタ ID
- `TNAM`: `TXST` レコードの FormID (4 bytes)
- `HNAM`: Havok 物理マテリアル、摩擦係数、反発係数 (3 bytes)
- `SNAM`: スペキュラ強度 (1 byte)
- `GNAM`: 生育草 FormID (可変)

### ③ LAND (Landscape) レコード
4 クアドラント (Quadrant 0..3: 各 16x16 クワッド、17x17 頂点)
- `BTXT`: 各クアドラントのベース（下地）テクスチャ FormID (`LTEX`)
- `ATXT`: 追加レイヤーのテクスチャ FormID (`LTEX`) + レイヤー番号 (`layer_index`)
- `VTXT`: 各頂点のブレンド透過度 (`opacity: f32`, 0.0〜1.0)
