# Fallout 3 セル環境光・照明レコード仕様書 (`XCLL` / `LIGHT` / `LGTM`)

本ドキュメントは `references/openmw/components/esm4/` (loadcell, loadligh, loadlgtm, lighting) および `Fallout3.esm` バイナリ構造に基づき、セル環境照明および配置光源のバイナリレイアウトとシェーダー適用計算を定義した永続記録である。

---

## 1. CELL 環境光サブレコード (`XCLL`: 40 バイト)

室内セル（Interior CELL）または特定の天候を持つセルにおいて、`XCLL` サブレコードが定義される。
- 参照元: `references/openmw/components/esm4/loadcell.cpp:L181-193`, `lighting.hpp:L37`

| オフセット | 型 | フィールド名 | 説明 |
|---|---|---|---|
| 0 | `[u8; 4]` | `ambient` | 環境光色 (RGBA 各 0..255) |
| 4 | `[u8; 4]` | `directional` | 指向性光色 (RGBA 各 0..255) |
| 8 | `[u8; 4]` | `fog_color` | フォグ色 (RGBA 各 0..255) |
| 12 | `f32` | `fog_near` | フォグ開始距離 (World Units) |
| 16 | `f32` | `fog_far` | フォグ最大距離 (World Units) |
| 20 | `i32` | `rotation_xy` | 指向性光の水平回転角 (度/10 または角度) |
| 24 | `i32` | `rotation_z` | 指向性光の垂直回転角 (度/10 または角度) |
| 28 | `f32` | `fog_dir_fade` | フォグ指向性減衰係数 |
| 32 | `f32` | `fog_clip_dist` | カメラ遠方クリッピング距離 |
| 36 | `f32` | `fog_power` | フォグ濃度指数 (Fallout 3 / FNV で追加) |

合計サイズ: 40 バイト。

### 1.1 ライティングテンプレート (`LTMP` / `LGTM`)
セルに `XCLL` が直接記述されていない、または `LTMP` (Lighting Template) サブレコードが存在する場合:
- `LTMP`: `FormId` (対象 `LGTM` レコードへの参照)
- `LNAM`: `u32` (オーバーライドフラグ)
- `LGTM` レコードの `DATA` サブレコードは、上記 `XCLL` と全く同じ 40 バイトの `Lighting` 構造体を保持する。

---

## 2. 光源基本レコード (`LIGHT`: 32 バイト DATA)

ゲーム世界に配置される照明器具や光源オブジェクト（ランプ、焚き火、天井蛍光灯、環境発光体）の基本レコード。
- 参照元: `references/openmw/components/esm4/loadligh.cpp:L52-84`, `loadligh.hpp:L58`

### 2.1 サブレコード構成
- `EDID`: エディタID文字列
- `FULL`: 表示名文字列
- `MODL`: 3Dモデルパス（照明器具のメッシュ。透明な光源の場合は空）
- `DATA`: 光源物理パラメータ（Fallout 3 では 32 バイト）

### 2.2 DATA サブレコード詳細 (32 バイト)
| オフセット | 型 | フィールド名 | 説明 |
|---|---|---|---|
| 0 | `i32` | `time` | 点灯持続時間 (-1 は常時点灯) |
| 4 | `u32` | `radius` | 光の到達最大半径 (World Units) |
| 8 | `[u8; 4]` | `colour` | 光源色 (RGBA 各 0..255) |
| 12 | `i32` | `flags` | フラグビット (下記参照) |
| 16 | `f32` | `falloff` | 減衰指数 (Falloff exponent, 通常 1.0) |
| 20 | `f32` | `fov` | スポットライト照射角 (FOV 度数) |
| 24 | `u32` | `value` | 価格 (ゴールド/キャップ) |
| 28 | `f32` | `weight` | 重量 |

#### フラグ値 (`flags`):
- `0x0001`: Dynamic (動的光源)
- `0x0002`: Can be Carried (持ち運び可能)
- `0x0004`: Negative (光を吸収する影光源)
- `0x0008`: Flicker (高速ちらつき)
- `0x0020`: Off by default (デフォルト消灯)
- `0x0040`: Flicker Slow (低速ちらつき)
- `0x0080`: Pulse (パルス明滅)
- `0x0100`: Pulse Slow (低速パルス明滅)
- `0x0200`: Spot Light (スポットライト)
- `0x0400`: Spot Shadow (スポットシャドウ)

---

## 3. CELL 内での配置と Gamebryo レンダリング統合

### 3.1 CELL 内の REFR によるインスタンス化
- `REFR` の `NAME` サブレコードが `LIGHT` レコードの FormID を参照している場合、その配置位置 `pos: [f32; 3]` に点光源（Point Light）が生成される。
- メッシュ（`MODL`）が存在する場合は照明器具の 3D オブジェクトを描画し、さらにワールド座標 `pos` を中心とする点光源としてシェーダーにバインドする。

### 3.2 Gamebryo 2.6 点光源の減衰計算式
- 参照元: `references/openmw/files/shaders/lib/light/util.glsl:L45-52, L80-97`
距離 $d = \|\mathbf{x}_{\text{pos}} - \mathbf{x}_{\text{vertex}}\|$, 最大半径 $R = \text{radius}$:
$$d > R \implies \text{attenuation} = 0.0$$
$$d \le R \implies \text{attenuation} = \left( 1.0 - \frac{d}{R} \right)^{\text{falloff}}$$
ランバート拡散反射:
$$I_{\text{diffuse}} = \mathbf{C}_{\text{light}} \cdot \max(0.0, \mathbf{N} \cdot \mathbf{L}) \cdot \text{attenuation}$$
アンビエント合成:
$$I_{\text{total}} = I_{\text{cell\_ambient}} + I_{\text{dir}} \cdot \max(0.0, \mathbf{N} \cdot \mathbf{L}_{\text{dir}}) + \sum_{k} I_{\text{point\_light}, k}$$
