# Fallout 3 / Gamebryo 2.6 アクター配置とスキンメッシュ仕様

## 1. アクターの階層構造 (ESM)

Fallout 3 のセル内に配置される NPC やクリーチャーは以下の ESM レコードリレーションを持つ。

```
Cell (CELL)
  └── Placed NPC (ACHR)
        ├── DATA: ワールド配置座標 (XYZ), 回転角 (XYZ)
        └── NAME: ベースオブジェクト FormID (NPC_)
              └── NPC_ (Non-Player Character)
                    ├── ACBS: 性別フラグ (0x01: Female), 基本設定
                    ├── CNAM: クラス FormID
                    ├── RNAM: 種族 FormID (RACE)
                    ├── HCLF: 髪色 FormID
                    ├── PNAM: 頭部パーツ (HeadParts)
                    ├── WNAM: 装備防具 FormID (ARMO)
                    └── インベントリ (CNTO): 装備品やアイテム
```

### 防具 (ARMO) とボディメッシュ
- `ARMO` レコード:
  - `MODL` / `MOD2`: 男性装備 NIF パス（例: `meshes\armor\wastelandclothing01\outfitm.nif`）
  - `MOD3` / `MOD4`: 女性装備 NIF パス（例: `meshes\armor\wastelandclothing01\outfitf.nif`）
- `RACE` レコード:
  - 男/女のスケルトン NIF パス（例: `meshes\characters\_male\skeleton.nif`）
  - 基本素体メッシュ（手、足、胴体、頭部）

---

## 2. スキンメッシュの NIF ブロック構造

参照元: `references/nifxml/nif.xml:L5076` (`NiSkinInstance`), `L5093` (`NiSkinPartition`), `L2143` (`SkinPartition`)

### 1. `NiGeometry` (`NiTriShape`)
- `skin_instance: i32`: スキンインスタンスブロックへのインデックス（非スキンメッシュは -1）。

### 2. `NiSkinInstance` (module="NiMain")
```rust
pub struct NiSkinInstance {
    pub data: i32,             // Ref<NiSkinData>
    pub skin_partition: i32,   // Ref<NiSkinPartition> (v10.1.0.101 以降: ハードウェアスキニング用)
    pub skeleton_root: i32,    // Ptr<NiNode> (通常 "Bip01" ノード)
    pub num_bones: u32,
    pub bones: Vec<i32>,       // Ptr<NiNode> 各ボーンノードへの参照配列
}
```

### 3. `NiSkinPartition` (module="NiMain")
ハードウェアスキニングに最適化されたサブメッシュ分割データ。
Fallout 3 (`version == 20.2.0.7`, `user_version == 11`, `user_version2 == 34`) では以下の構造をとる:

```rust
pub struct NiSkinPartition {
    pub num_partitions: u32,
    pub partitions: Vec<SkinPartition>,
}

pub struct SkinPartition {
    pub num_vertices: u16,
    pub num_triangles: u16,
    pub num_bones: u16,
    pub num_strips: u16,
    pub num_weights_per_vertex: u16, // Gamebryo 2.6 では常に 4
    pub bones: Vec<u16>,             // NiSkinInstance.bones のインデックス配列
    pub has_vertex_map: bool,        // true
    pub vertex_map: Vec<u16>,        // 親 NiTriShapeData 頂点インデックスへのマップ
    pub has_vertex_weights: bool,    // true
    pub vertex_weights: Vec<[f32; 4]>, // 頂点ごとのボーン重み (合計 1.0)
    pub strip_lengths: Vec<u16>,
    pub has_faces: bool,             // true
    pub strips: Vec<Vec<u16>>,       // num_strips != 0 の場合
    pub triangles: Vec<[u16; 3]>,    // num_strips == 0 の場合 (通常こちら)
    pub has_bone_indices: bool,      // true
    pub bone_indices: Vec<[u8; 4]>,  // 各頂点のボーン番号 (上記 `bones` のインデックス)
}
```

---

## 3. Gamebryo 2.6 スキニング変形計算式

参照元: Gamebryo 2.6 `NiSkinInstance::Update`

頂点 $v$ のワールド空間座標 $v'$ は、各影響ボーンのトランスフォームにより重み付け合成される:

$$v' = \sum_{i=0}^{3} w_i \cdot M_{\text{bone}[i]} \cdot B_{\text{bone}[i]} \cdot v$$

- $w_i$: `vertex_weights[i]` (重み)
- $B_{\text{bone}[i]}$: 初期バインドポーズの逆行列（`skin_to_bone` 変換行列、`NiSkinData` に格納）
- $M_{\text{bone}[i]}$: 現在のボーンノードのワールド変換行列（スケルトン階層から算出）

---

## 4. ボーン階層のワールド変換解決 (`fo3_render::scene`)

### 4.1 ボーン世界変換マップ (`bone_world_map`)

スキンメッシュの変形に必要な各ボーンのワールド変換行列 (スケルトンルート基準) は
`HashMap<i32, Mat4>` (`bone_world_map`) で `block_index → Mat4` として管理する。
`NiSkinInstance.bones[i]` のブロックインデックスからこのマップを参照し、
`apply_skinning_cpu_with_bones` に `&[Mat4]` として渡す。

```rust
fn resolve_bone_world_transforms(skin_instance, bone_world_map) -> Vec<Mat4>
// マップに無いボーンは Mat4::IDENTITY で補完 (バインドポーズ T-Pose 相当)
```

### 4.2 トラバーサル順序問題とプレパス (2026-09-07 確定)

**問題**: 装備 NIF (`meshes\armor\leatherarmor\m\outfitm.nif` など) では
NiTriShape (メッシュ) ブロックがボーン NiNode ブロックよりファイル先頭側・
子ノード配列の手前に配置されている場合がある (例: mesh=`[001]`, bones=`[009]`〜`[015]`)。
単一パス深さ優先トラバース (`traverse_block`) では、メッシュ処理時にボーン未登録となり
全ボーンが `Mat4::IDENTITY` へフォールバックしてしまう (T-Pose 表示は正しいが、
アニメーション適用時に動的変換を反映できない)。

**解決**: メッシュビルドに先立ち、スケルトン階層全体を走査してワールド変換を
事前登録するプレパス `collect_bone_world_transforms()` を導入した。

```
処理順:
  1. bone_world_map.clear()                          (NIF ファイルごと)
  2. collect_bone_world_transforms(root, ...)        ← プレパス: NiNode/BSFadeNode 全登録
  3. traverse_block(root, ...)                       ← 本パス: メッシュビルド + プロパティ解決
  4. resolve_bone_world_transforms(inst, map)        ← NiTriShape 内で解決
```

参照元: Gamebryo 2.6 `NiAVObject::UpdateDownwardPass`, `NiSkinInstance::Update`
実装: `crates/fo3_render/src/scene.rs` (`collect_bone_world_transforms`, `resolve_bone_world_transforms`)

### 4.3 スキニング計算式と行列転置の数学的整合性 (2026-09-11 修正完了)

- 本実装（修正後）: `v' = Σ(w_i * (root_mat * M_bone[i] * B_bone[i])) * v`
  - `B_bone[i]`: `NiSkinData.bone_list[i].skin_transform` (逆バインド skin→bone 行列)
  - `M_bone[i]`: ボーンの現ワールド行列 (スケルトン空間)
  - `root_mat`: `NiSkinData.skin_transform` (ルートオフセット行列)
  - 乗算順序: `root_mat * M_bone[i] * B_bone[i]` (列ベクトル形式 $M \cdot v$)
    - OpenMW (行ベクトル形式 $v \cdot A \cdot B$) の `B_bone * M_bone * root_mat` を列ベクトル形式へ厳密に転置したもの。
    - 旧実装の `root_mat_inv * M_bone * B_bone` や行ベクトル順のまま掛けた式では、座標がボーンローカル座標のまま原点周辺に粉々に飛び散る原因となっていた。
  - Matrix33 転置:
    - NIF の `Matrix33` は行優先 (row-major: `m[row][col]`) 格納。
    - glam の列優先 `Mat3::from_cols_array_2d` と組み合わせる際、`.transpose()` を呼ぶことで正しい数学行列を復元。
    - バインドポーズ（T-Pose）における実測検証で、入力頂点 BBox とスキニング後頂点 BBox が誤差 0.00001 未満（100.000%）で完全一致することを確認済み。

### 4.4 スケルトン分離とボーン名マッピング (2026-09-11 確定)

**問題**: キャラクタ装備やボディパーツ（`upperbody.nif`, `outfitm.nif` 等）の NIF 内に含まれるボーンノードは、スキニング用のダミー配置であり、親子階層構造（`children`）を持たない（すべて子ノード数 0）。
パーツ NIF 単体に対して FK（順運動学）を再計算しても、親ボーン（肩や肘など）の回転・移動が子ボーン（手首や指）に伝達されず、各ボーンが原点直下で孤立回転するためメッシュが粉々に崩壊する。

**解決**:
1. 真のボーン階層ツリーは `meshes\characters\_male\skeleton.nif`（またはクリーチャー固有のスケルトン NIF）にのみ存在する。
2. KF アニメーションはスケルトン NIF に対して適用し、スケルトン側の階層ツリーに対して FK を再計算する。
3. パーツメッシュ（`upperbody.nif`）のスキニング変形時は、パーツ NIF の `NiSkinInstance.bones` が参照するボーンノード名（`Bip01 Pelvis` 等）をもとに、スケルトン側で計算された同名ボーンのワールド変換行列（`name_to_world: HashMap<String, Mat4>`）を引き当ててスキニング計算を行う。

参照元: Gamebryo 2.6 `NiSkinInstance::Update`, `NiAVObject::UpdateDownwardPass`

### 4.5 四肢切断ゴアメッシュと通常表示判定 (2026-09-11 確定)

**問題**: `upperbody.nif` や各種アクターパーツには、V.A.T.S. や爆発による四肢切断イベント（Dismemberment）発生時にのみ表示される切断面の肉片・骨キャップメッシュ（`bodycaps`, `limbcaps`, `meatneck01`, `meathead01` 等、テクスチャ: `textures\gore\MeatCapGore01.dds`）が同一 NIF 内に同居している。これらを無条件に描画すると、首元や肩・腰・手足の切断面が赤い血肉の塊として露出してしまう。

**仕様と判定基準**:
- 参照元: `references/nifskope/build/nif.xml:L2530-2541` (`BSPartFlag`, `BodyPartList`), `L1262-1326` (`BSDismemberBodyPartType`)
- 参照元: `references/bevyout/src/vsa/assets/blender_script.py:L1760-1779` (`partition_is_editor_visible`)
- `BSDismemberSkinInstance.partitions[i].part_flag`:
  - ビット 0 (`part_flag & 0x0001 != 0`): `PF_EDITOR_VISIBLE` (通常・エディタ表示フラグ)
  - `editor_visible == true`: 通常状態（非切断）で表示される正規パーツ（例: `Arms:1` の各パーティションは `0x0101` または `0x0001`）
  - `editor_visible == false`: 四肢切断時にのみ表示されるゴア・切断面キャップ（例: `bodycaps`, `limbcaps`, `meatneck01`, `meathead01` のパーティションは `0x0100` または `0x0000`、body_part=101, 103, 105 等の `BP_SECTIONCAP_*`）
- **初期表示ルール**:
  - `BSDismemberSkinInstance` を持つメッシュにおいて、全パーティションが `editor_visible == false` であるメッシュノードは通常時非表示（Hidden）としてスキップする。

---

## 5. 実装ステータス (2026-09-11 更新)

| 項目 | 状態 |
|---|---|
| `NiSkinData` パーサー (`fo3_nif::blocks::skin`) | 完了 (NiTransform 順序修正済み) |
| `BoneData`, `BoneVertData` パーサー | 完了 |
| `NiSkinInstance` パーサー | 完了 |
| `NiSkinPartition` + `SkinPartition` パーサー | 完了 |
| CPU スキニング (`fo3_render::skinning`) | 完了（行列積順序 & Matrix33 転置修正済み） |
| `GpuMesh::from_tri_shape_skinned` | 完了 |
| `scene.rs` スキニング分岐 | 完了 |
| ボーン階層プレパス (`collect_bone_world_transforms`) | 完了 |
| 動的ボーン行列のスキニング反映 | 完了（`apply_skinning_cpu_with_bones` に `&[Mat4]` を供給） |
| FK 再計算 (`recompute_bone_world_map_with_pose`) | 完了 |
| KF ボーン適用 (`apply_pose`) | 完了 |
| 時間更新ループ + 簡易プレイヤー (`AnimationPlayer`) | 完了（`fo3_render::animation`） |
| B-Spline 圧縮補間 (NiBSplineCompTransformInterpolator) | 完了（Cox-de Boor 基底評価） |
| 実アセットでの T-Pose スキニング確認 | 完了（実測誤差 0.00001 未満・完全一致） |
| 実アセットでのアニメーション再生確認 | 完了（fo3_viewer `-- anim` モード稼働） |
| GPU シェーダースキニング（ボーン行列パレット） | 未実装 |

### CPU スキニング実装ファイル
- `crates/fo3_nif/src/blocks/skin.rs` - `NiSkinData` / `BoneData` / `BoneVertData`
- `crates/fo3_render/src/skinning.rs` - `apply_skinning_cpu` / `apply_skinning_cpu_with_bones`
- `crates/fo3_render/src/animation.rs` - `AnimationClip` / `AnimationPlayer` / `apply_pose` / B-Spline 評価
- `crates/fo3_render/src/mesh.rs` - `GpuMesh::from_tri_shape_skinned`
- `crates/fo3_render/src/scene.rs` - `NiTriShape` スキニング分岐 / FK 再計算
