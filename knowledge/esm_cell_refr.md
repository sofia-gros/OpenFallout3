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

## 5. ESM 配置参照 (REFR) のオイラー角回転における Gamebryo 2.6 / NifSkope 仕様

一次文献: `references/nifskope/src/data/niftypes.cpp:223` (`Matrix::fromEuler`), `references/nifskope/src/data/niftypes.h:961`

NifSkope / Gamebryo 2.6 におけるオイラー角からの回転行列構築は、**$R = R_x(\text{rot}_x) \cdot R_y(\text{rot}_y) \cdot R_z(\text{rot}_z)$** の順序で計算されます。

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

代数展開により、この行列は列ベクトル規約において $R_x(x) \cdot R_y(y) \cdot R_z(z)$ と厳密に一致します。
※ $R_z \cdot R_y \cdot R_x$ の逆順で掛けると、3軸すべてが回転している配管や手すり、階段などのピースが全くあさっての方向を向いて宙に浮くことになります。
※ OpenMW の `object.cpp` で見られる符号反転は、OSG のクォータニオン乗算順序と右手/左手系の相互作用によるものであり、Gamebryo / NifSkope のネイティブ回転行列式 `fromEuler` を直接使用することで、符号反転を行わずとも正しい姿勢が得られます。

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
