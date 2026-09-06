# Fallout 3 NIF Havok コリジョンブロック仕様書

本ドキュメントは `references/nifxml/nif.xml` (BSHavok モジュール) および NifSkope / OpenMW 実装に基づき、Fallout 3 (`v20.2.0.7`, `user_version == 11`, `user_version2 == 34`) における Havok コリジョンブロックのバイナリレイアウトと構造を定義した永続記録である。

---

## 1. コリジョン階層構造の概要

Fallout 3 の NIF シーングラフにおいて、コリジョンは root ノード（`NiNode` / `BSFadeNode`）の `collision_object` フィールド（`Ref<NiCollisionObject>`）から始まる。

```text
NiAVObject::collision_object (Ref)
  │
  ▼
bhkCollisionObject (10 bytes)
  │ body (Ref)
  ▼
bhkRigidBody / bhkRigidBodyT (236 bytes)
  │ shape (Ref)
  ▼
bhkMoppBvTreeShape (40 + MOPPバイト)
  │ shape (Ref)
  ▼
bhkPackedNiTriStripsShape (56 bytes)
  │ data (Ref)
  ▼
hkPackedNiTriStripsData (11 + メッシュバイト)
```

### Havok 単位と Gamebryo ゲーム単位のスケール関係
- 参照元: `references/nifskope/src/gl/gltools.cpp:L327-345` (`hkScale660 = 1.0 / 1.42875 * 10.0 = 6.999125...`)
- **`bhkPackedNiTriStripsShape` & `hkPackedNiTriStripsData`**:
  すでにメッシュのローカル座標単位（Gamebryo 単位）で格納されているため、追加のスケール乗算は不要。
- **プリミティブ・凸包形状 (`bhkBoxShape`, `bhkSphereShape`, `bhkCapsuleShape`, `bhkConvexVerticesShape`)**:
  Havok の物理単位（メートル）で格納されている。Gamebryo 単位に変換するには **`hkScale660 = 1.0 / 0.142875 ≈ 6.999125`** を乗算する。
  - 例: 10mmピストルの `bhkConvexVerticesShape` 頂点 X = `2.5614` $\times 6.999125 = 17.927$ GU （実メッシュの Max X = `17.805` GU と完全に一致）。
- **`bhkRigidBodyT` の変換**:
  `translation = Vector3(body.translation * hkScale660)`、`rotation = body.rotation`。

---

## 2. 各ブロックのバイナリ仕様 (Fallout 3: 20.2.0.7 / BSVER 34)

### 2.1 bhkCollisionObject (10 bytes)
`NiCollisionObject` -> `bhkNiCollisionObject` -> `bhkCollisionObject`
- 参照元: `references/nifxml/nif.xml:L3370, L3403, L3420`

| オフセット | 型 | フィールド名 | 説明 |
|---|---|---|---|
| 0 | `i32` | `target` | コリジョンがアタッチされている `NiAVObject` のブロックインデックス (Ptr) |
| 4 | `u16` | `flags` | `bhkCOFlags` (FO3 デフォルト: 0x0001, アニメ静的: 0x0028) |
| 6 | `i32` | `body` | `bhkWorldObject` (`bhkRigidBody`) へのブロックインデックス (Ref) |

合計サイズ: 10 バイト。

---

### 2.2 bhkRigidBody / bhkRigidBodyT (236 bytes)
`bhkWorldObject` -> `bhkEntity` -> `bhkRigidBody`
- 参照元: `references/nifxml/nif.xml:L2765, L2803, L2808, L2931`

#### (1) bhkWorldObject (28 bytes)
| オフセット | 型 | フィールド名 | 説明 |
|---|---|---|---|
| 0 | `i32` | `shape` | コリジョン形状 (`bhkShape`) への参照 (Ref) |
| 4 | `u8` | `havok_filter_layer` | `Fallout3Layer` (1 = Static, 2 = AnimStatic, etc.) |
| 5 | `u8` | `havok_filter_flags` | コリジョンフィルターフラグ |
| 6 | `u16` | `havok_filter_group` | コリジョングループ番号 |
| 8 | `[u8; 4]` | `world_obj_unused01` | パディング/予約 |
| 12 | `u8` | `broad_phase_type` | `BroadPhaseType` (通常 1 = Entity) |
| 13 | `[u8; 3]` | `world_obj_unused02` | パディング/予約 |
| 16 | `u32` | `prop_data` | `bhkWorldObjCInfoProperty::Data` (通常 0) |
| 20 | `u32` | `prop_size` | `bhkWorldObjCInfoProperty::Size` (通常 0) |
| 24 | `u32` | `prop_capacity_and_flags`| `bhkWorldObjCInfoProperty::CapacityAndFlags` (通常 0x80000000) |

#### (2) bhkEntity (4 bytes)
| オフセット | 型 | フィールド名 | 説明 |
|---|---|---|---|
| 28 | `u8` | `collision_response` | `hkResponseType` (通常 1 = SimpleContact) |
| 29 | `u8` | `entity_unused01` | パディング |
| 30 | `u16` | `process_contact_callback_delay` | コールバック遅延フレーム数 (通常 0xFFFF) |

#### (3) bhkRigidBodyCInfo550_660 (196 bytes)
| オフセット | 型 | フィールド名 | 説明 |
|---|---|---|---|
| 32 | `[u8; 4]` | `cinfo_unused01` | 未使用 4 バイト |
| 36 | `u8` | `cinfo_layer` | `Fallout3Layer` |
| 37 | `u8` | `cinfo_filter_flags`| フィルターフラグ |
| 38 | `u16` | `cinfo_group` | フィルターグループ |
| 40 | `[u8; 4]` | `cinfo_unused02` | 未使用 4 バイト |
| 44 | `u8` | `cinfo_collision_response`| 衝突レスポンス |
| 45 | `u8` | `cinfo_unused03` | 未使用 1 バイト |
| 46 | `u16` | `cinfo_process_contact_delay`| コールバック遅延 (0xFFFF) |
| 48 | `[u8; 4]` | `cinfo_unused04` | 未使用 4 バイト |
| 52 | `[f32; 4]`| `translation` | 平行移動ベクトル (X, Y, Z, W) |
| 68 | `[f32; 4]`| `rotation` | 回転クォータニオン (X, Y, Z, W) |
| 84 | `[f32; 4]`| `linear_velocity` | 線形速度 (X, Y, Z, W) |
| 100 | `[f32; 4]`| `angular_velocity`| 角速度 (X, Y, Z, W) |
| 116 | `[f32; 12]`| `inertia_tensor` | 慣性テンソル 4x3 行列 (hkMatrix3) |
| 164 | `[f32; 4]`| `center_of_mass` | 重心 (X, Y, Z, W) |
| 180 | `f32` | `mass` | 質量 (kg)。0.0 は不動 (Static) |
| 184 | `f32` | `linear_damping` | 線形減衰 (デフォルト 0.1) |
| 188 | `f32` | `angular_damping`| 角減衰 (デフォルト 0.05) |
| 192 | `f32` | `friction` | 摩擦係数 (デフォルト 0.5) |
| 196 | `f32` | `restitution` | 反発係数 (デフォルト 0.4) |
| 200 | `f32` | `max_linear_velocity`| 最大線形速度 (通常 104.4) |
| 204 | `f32` | `max_angular_velocity`| 最大角速度 (通常 31.57) |
| 208 | `f32` | `penetration_depth`| 許容貫通深度 (デフォルト 0.15) |
| 212 | `u8` | `motion_system` | `hkMotionType` (動的/キーフレーム/固定) |
| 213 | `u8` | `deactivator_type`| `hkDeactivatorType` (スリープ設定) |
| 214 | `u8` | `solver_deactivation`| `hkSolverDeactivation` |
| 215 | `u8` | `quality_type` | `hkQualityType` (固定/キーフレーム/デブリ等) |
| 216 | `[u8; 12]`| `cinfo_unused05` | 未使用 12 バイト |

#### (4) bhkRigidBody 直下フィールド (8 bytes)
| オフセット | 型 | フィールド名 | 説明 |
|---|---|---|---|
| 228 | `u32` | `num_constraints` | 拘束数 (通常 0) |
| 232 | `u32` | `body_flags` | 風への応答等のフラグ (`BSVER < 76` のため u32) |

合計サイズ: 28 + 4 + 196 + 4 + 0 + 4 = 236 バイト。実アセットと完全一致。

---

### 2.3 bhkMoppBvTreeShape (40 + MOPP バイト)
`bhkShape` -> `bhkBvTreeShape` -> `bhkMoppBvTreeShape`
- 参照元: `references/nifxml/nif.xml:L3134, L3140, L3153`

| オフセット | 型 | フィールド名 | 説明 |
|---|---|---|---|
| 0 | `i32` | `shape` | 内包される形状 (`bhkPackedNiTriStripsShape`) への参照 (Ref) |
| 4 | `[u8; 12]` | `unused01` | 未使用 12 バイト |
| 16 | `f32` | `scale` | スケール (通常 1.0) |
| 20 | `u32` | `mopp_data_size` | 後続 MOPP バイナリのバイト長 |
| 24 | `[f32; 4]` | `mopp_offset` | XYZ: MOPP 座標系原点, W: 量子化係数 (FO3 では Build Type なし) |
| 40 | `[u8; size]`| `mopp_data` | MOPP コードツリーバイト列 |

---

### 2.4 bhkPackedNiTriStripsShape (56 bytes)
`bhkShape` -> `bhkShapeCollection` -> `bhkPackedNiTriStripsShape`
- 参照元: `references/nifxml/nif.xml:L3193`
- FO3 では `until="20.0.0.5"` の `Num Sub Shapes`, `Sub Shapes` は存在しない（`hkPackedNiTriStripsData` 側に存在）。

| オフセット | 型 | フィールド名 | 説明 |
|---|---|---|---|
| 0 | `u32` | `user_data` | ユーザーデータ (通常 0) |
| 4 | `[u8; 4]` | `unused01` | 未使用 4 バイト |
| 8 | `f32` | `radius` | コリジョン半径 (通常 0.1) |
| 12 | `[u8; 4]` | `unused02` | 未使用 4 バイト |
| 16 | `[f32; 4]` | `scale` | スケールベクトル (通常 1.0, 1.0, 1.0, 0.0) |
| 32 | `f32` | `radius_copy` | radius のコピー (通常 0.1) |
| 36 | `[f32; 4]` | `scale_copy` | scale のコピー |
| 52 | `i32` | `data` | `hkPackedNiTriStripsData` へのブロックインデックス (Ref) |

合計サイズ: 56 バイト。実アセットと完全一致。

---

### 2.5 hkPackedNiTriStripsData (可変長)
`bhkShapeCollection` -> `hkPackedNiTriStripsData`
- 参照元: `references/nifxml/nif.xml:L3958`

| オフセット | 型 | フィールド名 | 説明 |
|---|---|---|---|
| 0 | `u32` | `num_triangles` | 三角形数 |
| 4 | `[TriangleData; num_triangles]` | `triangles` | 1 三角形あたり 8 バイト (`v0: u16, v1: u16, v2: u16, weld_info: u16`) |
| 4 + 8*T | `u32` | `num_vertices` | 頂点数 |
| 8 + 8*T | `u8` | `compressed` | 0 = f32 3D ベクトル, 1 = 16bit ハーフ浮動小数点数 (HalfVector3) |
| 9 + 8*T | `Vertices` | `vertices` | `compressed == 0`: 12 バイト/頂点 (`[f32; 3]`)<br>`compressed != 0`: 6 バイト/頂点 (`[u16; 3]` -> IEEE-754 half float) |
| 後続 | `u16` | `num_sub_shapes` | サブシェイプ数 |
| 後続 | `[hkSubPartData; N]` | `sub_shapes` | 1 サブシェイプあたり 12 バイト (`HavokFilter: 4B, num_vertices: u32, material: u32`) |

#### HalfVector3 の f32 へのデコード
16-bit 浮動小数点数（符号 1bit, 指数 5bit, 仮数 10bit）は、IEEE-754 標準の half-precision に準拠しており、以下の変換式で 32-bit `f32` に展開可能である。
- 正規化数: `exp = (e + 112) << 23`, `mant = m << 13`
- 非正規化数: ゼロでない仮数をシフトして正規化指数を算出
- ゼロ / 無限大 / NaN: 符号および特殊ビットパターンを保持
