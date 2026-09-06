# Fallout 3 CELL & REFR レコード・シーン配置バイナリ仕様

本ドキュメントは Fallout 3 (Gamebryo 2.6) の空間構築（セルおよびオブジェクト配置参照）に関する詳細バイナリ仕様を記録した永続化知識ベースです。

---

## 1. 参照文献・一次資料
- `references/openmw/components/esm4/loadcell.hpp`, `loadcell.cpp`: `CELL` レコード構造・フラグ
- `references/openmw/components/esm4/loadrefr.hpp`, `loadrefr.cpp`: `REFR` レコード構造
- `references/openmw/components/esm4/grouptype.hpp`: グループ種別定義
- `references/openmw/components/esm/position.hpp`: `Position` 構造体 (`pos: [f32; 3]`, `rot: [f32; 3]`)
- `references/nifskope/src/gl/glcontroller.cpp:489`: Gamebryo オイラー角回転合成式
- `references/nifskope/src/data/niftypes.h:961`: Euler angles `(Z, Y, X)` manner

---

## 2. セルのグループ階層遷移構造

Fallout 3 の ESM では、セルとその配置オブジェクトは多層のグループ（`GRUP`）によって木構造化されています。

```
GRUP (Top, type=0, label=b"CELL")
 └── GRUP (InteriorCellBlock, type=2, label=block_num: i32)
      └── GRUP (InteriorCellSubBlock, type=3, label=sub_block_num: i32)
           ├── CELL レコード (type=b"CELL", form_id=XXXX)
           └── GRUP (CellChildren, type=6, label=cell_form_id)
                ├── GRUP (CellPersistentChildren, type=8, label=cell_form_id)
                │    └── REFR, ACHR, etc.
                └── GRUP (CellTemporaryChildren, type=9, label=cell_form_id)
                     └── REFR, ACHR, etc.
```

### グループ種別 (`group_type`):
- `0: Top`
- `2: InteriorCellBlock` (`label`: ブロック番号 `i32`)
- `3: InteriorCellSubBlock` (`label`: サブブロック番号 `i32`)
- `6: CellChildren` (`label`: 親 `CELL` の FormID `u32`)
- `8: CellPersistentChildren` (`label`: 親 `CELL` の FormID `u32`)
- `9: CellTemporaryChildren` (`label`: 親 `CELL` の FormID `u32`)

---

## 3. レコードバイナリ構造

### ① `CELL` レコード (シグネチャ: `b"CELL"`)
空間（部屋や屋外区画）を定義する基本レコード。

| サブレコード | 型 | 説明 |
| :--- | :--- | :--- |
| `EDID` | String (null-term) | エディタ用識別名 (例: `"Vault101a"`, `"MegatonCommonHouse"`) |
| `FULL` | String (null-term) | ゲーム内表示名 (例: `"Vault 101"`, `"メガトン共同住宅"`) |
| `DATA` | `u16` / `u8` | セルフラグ（`0x0001`: Interior, `0x0002`: HasWater, `0x0020`: Public 等） |
| `XCLC` | `i32, i32, (u32)` | 屋外セルのグリッド座標 $(X, Y)$ およびフラグ |
| `XCLL` | 40 bytes | ライティング設定 (環境光、ディレクショナル光、フォグ等) |

### ② `REFR` レコード (シグネチャ: `b"REFR"`)
セル内に配置された具体的な 3D オブジェクトの参照レコード。

| サブレコード | 型 | 説明 |
| :--- | :--- | :--- |
| `EDID` | String (null-term) | 配置参照のエディタ識別名 (省略される場合あり) |
| `NAME` | `FormId` (4 bytes) | **配置元のベースオブジェクト FormId**（`STAT`, `DOOR`, `CONT`, `FURN` 等） |
| `DATA` | 24 bytes | **位置およびオイラー角回転**<br/>`pos: [f32; 3]` ($X, Y, Z$)<br/>`rot: [f32; 3]` ($RotX, RotY, RotZ$：ラジアン) |
| `XSCL` | `f32` (4 bytes) | 3D スケール倍率（省略時は `1.0`） |
| `XTEL` | 28/32/36 bytes | ドアのテレポート先情報（行き先ドア FormId, 出現座標・回転等） |

---

## 4. Gamebryo 2.6 オイラー角回転行列合成式

Fallout 3 / Gamebryo 2.6 におけるオブジェクトの姿勢回転は、XYZ の 3 軸オイラー角から以下の順序で合成されます。

一次文献 (`references/nifskope/src/gl/glcontroller.cpp:489`):
```cpp
value = Matrix::euler( 0, 0, r[2] ) * Matrix::euler( 0, r[1], 0 ) * Matrix::euler( r[0], 0, 0 );
```

数式:
$$R = R_z(\text{rot}_z) \cdot R_y(\text{rot}_y) \cdot R_x(\text{rot}_x)$$

各軸の基本回転行列:
$$
R_x(\theta) = \begin{bmatrix}
1 & 0 & 0 \\
0 & \cos\theta & -\sin\theta \\
0 & \sin\theta & \cos\theta
\end{bmatrix}, \quad
R_y(\theta) = \begin{bmatrix}
\cos\theta & 0 & \sin\theta \\
0 & 1 & 0 \\
-\sin\theta & 0 & \cos\theta
\end{bmatrix}, \quad
R_z(\theta) = \begin{bmatrix}
\cos\theta & -\sin\theta & 0 \\
\sin\theta & \cos\theta & 0 \\
0 & 0 & 1
\end{bmatrix}
$$

### モデル行列（ワールド変換行列 $M$）:
$$M = T(\text{pos}) \cdot R \cdot S(\text{scale})$$

---

## 5. ESM 配置参照 (REFR) のオイラー角回転における Gamebryo 2.6 / ESM 仕様

一次文献:
- `references/nifskope/src/data/niftypes.cpp:223` (`Matrix::fromEuler`), `references/nifskope/src/data/niftypes.h:961`
- `references/openmw/components/misc/convert.hpp:50` (`makeOsgQuat`)
- `references/openmw/apps/opencs/view/render/object.cpp:165`

### ① 回転行列の合成順序と符号仕様
Bethesda ESM の `REFR` レコードに格納されているオイラー角 `rot: [f32; 3]` は、Creation Kit / Gamebryo 座標系において**各軸周りの時計回り回転（あるいは $-axis$ 方向回転）**として記録されています。
したがって、標準的な右手系・列ベクトル規約（WGPU / glam / OpenGL）で正しい姿勢行列 $R$ を得るには、各角度の符号を反転した**$R = R_x(-\text{rot}_x) \cdot R_y(-\text{rot}_y) \cdot R_z(-\text{rot}_z)$** を用いて合成します。

```cpp
void Matrix::fromEuler( float x, float y, float z )
{
	float sinX = sin( x ); float cosX = cos( x );
	float sinY = sin( y ); float cosY = cos( y );
	float sinZ = sin( z ); float cosZ = cos( z );

	m[0][0] = cosY * cosZ;
	m[0][1] = -cosY * sinZ;
	m[0][2] = sinY;
	m[1][0] = sinX * sinY * cosZ + sinZ * cosX;
	m[1][1] = cosX * cosZ - sinX * sinY * sinZ;
	m[1][2] = -sinX * cosY;
	m[2][0] = sinX * sinZ - cosX * sinY * cosZ;
	m[2][1] = cosX * sinY * sinZ + sinX * cosZ;
	m[2][2] = cosX * cosY;
}
```
※ 入力角度として $(-rx, -ry, -rz)$ を渡すことで、列ベクトル形式 $M \cdot v$ における正規の姿勢回転行列が得られます。

### ② 実アセット配管データによる数学的検証結果
メガトンプラザ（`MegatonPlaza`）の実ゲーム配管チェーン（`MegatonPipe*`）の配置座標と姿勢角度から算出した結果：
- **FormID 0x00014CC7 (直管パイプ) と 0x00014CC8 (後続パイプ)**:
  - 配置変位ベクトル: $\Delta Pos = [-33.6, +18.1, -128.2]$ (正規化: $[-0.251, +0.135, -0.958]$)
  - パイプメッシュの長手方向: ローカル $X$ 軸 $(1, 0, 0)$
  - 旧実装（符号 $+$）による方向ベクトル: `Vec3(-0.249, -0.439, -0.863)` $\to$ 内積 $\approx 0.86$ (あらぬ方向を向く)
  - 新実装（符号 $-$）による方向ベクトル: `Vec3(-0.249, +0.127, -0.960)` $\to$ **内積 $0.9999615$ (99.996% の完全一致！)**
- **FormID 0x00014CBF (直管パイプ) と 0x00014CC0**:
  - 新実装（符号 $-$）による方向ベクトルとの同一直線性: **内積 $-0.99929976$ (99.93% で同一直線上に接続！)**
これにより、オイラー角の符号反転 $(-rot)$ かつ乗算順序 $R_x \cdot R_y \cdot R_z$ が Gamebryo 2.6 / Fallout 3 の絶対的一致解であることが数学的に実証されました。

---

## 6. NIF 内部ノードにおけるエディタマーカーおよび非表示フラグの判定

家具（`FURN`）、ドア（`DOOR`）、エフェクト等の NIF メッシュには、Creation Kit / エディタ専用の配置マーカー（座る位置、ドアを開ける位置等）や不可視ジオメトリが含まれています。これらを誤ってレンダリングすると、「緑の枠」「開口部の白い板」となって現れます。

一次文献:
- `references/nifskope/src/gl/glmesh.cpp:760`:
  `if ( !scene->hasOption(Scene::ShowMarkers) && name.startsWith( "EditorMarker" ) ) return;`
- `references/nifskope/src/gl/glnode.cpp:456`:
  `if ( flags.node.hidden ) return true;`
- `references/openmw/components/nif/node.hpp:77`:
  `Flag_Hidden = 0x0001; bool isHidden() const { return mFlags & Flag_Hidden; }`

### 除外ルール:
1. **ノード名**:
   - `name.to_ascii_lowercase().starts_with("editormarker")`
   - `name.to_ascii_lowercase().starts_with("marker")`
   上記に該当するノードおよびその子孫ノードは描画をスキップする。
2. **`NiAVObject::flags` ビット 0 (`0x0001`)**:
   - `flags & 0x0001 != 0` の場合、`Hidden` (App Culled: アプリケーション側で非表示指定) であるため描画をスキップする。
ゲーム内の座標系は右手系 Z-up ($X$: 東/右, $Y$: 北/前, $Z$: 上) であり、NIF 内部のジオメトリ座標系と一致します。

---

## 7. REFR / ACHR / ACRE 拡張サブレコード詳細仕様

配置参照レコード（`REFR`）およびアクター・クリーチャー配置（`ACHR`, `ACRE`）に付与される主要サブレコードのバイナリレイアウト。

一次文献:
- `references/openmw/components/esm4/loadrefr.hpp`, `loadrefr.cpp`
- `references/openmw/components/esm4/loadachr.hpp`, `loadachr.cpp`
- `references/openmw/components/esm4/reference.hpp`

### ① `XTEL` (ドアのテレポート先定義, 28 または 32 バイト)
ドアオブジェクト（`DOOR`）を通過した際の遷移先ドアおよび出現位置。
- `dest_door`: `FormId` (4 bytes, 遷移先のドア `REFR` の FormId)
- `dest_pos`: `[f32; 3]` (12 bytes, 出現位置 XYZ)
- `dest_rot`: `[f32; 3]` (12 bytes, 出現姿勢オイラー角 XYZ ラジアン)
- `flags`: `u32` (4 bytes, 32 バイト時のみ。`0x01`: No Alarm 等)

### ② `XLOC` (施錠データ, 12 / 16 / 20 バイト)
ドアやコンテナなどの施錠情報。
- `lock_level`: `u8` (バイト 0: 0=Very Easy, 25=Easy, 50=Average, 75=Hard, 100=Very Hard, 255=要キー/施錠解除不可)
- `unused`: `[u8; 3]` (バイト 1..3)
- `key`: `FormId` (バイト 4..7: 解錠キーアイテムの FormId。0 の場合はキー不要)
- `flags`: `u32` (バイト 8..11: 施錠フラグ)

### ③ `XESP` (Enable Parent, 8 バイト)
親オブジェクトの有効化・無効化状態と連動する設定。
- `parent`: `FormId` (4 bytes: 連動親オブジェクトの FormId)
- `flags`: `u32` (4 bytes: `0x01` = Inversed: 逆連動, `0x02` = PopIn)

### ④ `XMRK` / `TNAM` (マップマーカー)
ファストトラベルやマップ上に表示されるマーカー。
- `XMRK`: 0 バイト（存在することで MapMarker であることを示すフラグ）
- `TNAM`: 2 バイト (`u16`) または FormId（マーカーの種別・アイコン定義）

### ⑤ `XOWN` / `XRNK` (所有権情報)
- `XOWN`: `FormId` (4 bytes: 所有者 NPC または Faction の FormId)
- `XRNK`: `i32` (4 bytes: 必要ファクションランク)

### ⑥ `XCNT` (配置スタック数)
- `count`: `i32` (4 bytes: ワールドに配置されたアイテムの個数。省略時は 1)

