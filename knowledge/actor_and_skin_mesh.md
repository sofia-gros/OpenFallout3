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

### 4.6 人型アクターのマルチパーツ構成とアセンブリ (2026-09-11 確定)

**仕様とパーツ構成**:
Fallout 3 の人型アクター（プレイヤーおよび人型 NPC）は、単一の NIF ファイルではなく、共通スケルトン (`skeleton.nif`) に以下の部位別 NIF をアタッチして 1 体のアクターとして描画する:

1. **頭部（首含む）**: `meshes\characters\head\headhuman.nif`
   - メッシュ: `HeadMale` (首の付け根から顔・頭部シェル全体)
   - ボーン: `Bip01 Spine2`, `Bip01 L Clavicle`, `Bip01 R Clavicle`, `Bip01 Neck1`, `Bip01 Head`
2. **胴体・下半身・下着**:
   - 素体（裸）: `meshes\characters\_male\upperbody.nif`
     - メッシュ `Arms:0`: 下着パンツ (`textures\armor\underwear\UnderwearM.dds`)
     - メッシュ `Arms:1`: 素肌胴体＋太もも・すね・足先 (`textures\characters\male\UpperBodyMale.dds`, Z=0.77〜102.12)
   - 衣装/防具装備時: 各防具 NIF（例: `meshes\armor\wastelandclothing01\outfitm.nif`）
3. **手（左右別）**:
   - 右手: `meshes\characters\_male\righthand.nif` (`RightHand:0`)
   - 左手: `meshes\characters\_male\lefthand.nif` (`LeftHand:0`)

**アセンブリ・スキニングパイプライン (`RenderScene::from_actor_parts`)**:
- 共通スケルトン `skeleton.nif` の全階層ボーンのワールド変換行列（`bone_name_world_map: HashMap<String, Mat4>`）を初期姿勢（T-Pose）または FK 更新時に算出。
- 各パーツ NIF のスキンメッシュは、パーツ NIF 自身のボーンブロックインデックスではなく、スケルトンの `bone_name_world_map` から同名ボーンを引き当ててスキニング頂点変形を行う。
- これにより、首元や手首の切れ目なくピッタリ結合し、KF アニメーション再生時にも全身が完全に連動して一体として動く。

### 4.7 剛体アタッチメントパーツ（HeadParts: 目・歯・舌）の結合仕様

Fallout 3 の頭部（`headhuman.nif`）は外皮（顔・頭皮・耳・首）のみで構成されており、眼球や口腔内は別 NIF の HeadPart として定義されている。

1. **パーツ一覧**:
   - 左眼球: `meshes\characters\head\eyelefthuman.nif`
   - 右眼球: `meshes\characters\head\eyerighthuman.nif`
   - 上歯: `meshes\characters\head\teethupperhuman.nif`
   - 下歯: `meshes\characters\head\teethlowerhuman.nif`
   - 舌: `meshes\characters\head\tonguehuman.nif`

2. **バイナリ構造とアタッチメント仕様**:
   - これらはスキンメッシュ（`NiSkinInstance`）を持たない**剛体ジオメトリ（`NiTriStrips`）**である。
   - ルートノード `NiNode` の `extra_data_list` に `NiStringExtraData("Bip01 Head")`（またはターゲットボーン名）を保持している。
   - 参照元: Gamebryo 2.6 `NiNode::AttachChild`（ボーンノードへの子オブジェクトアタッチ）
   - 初期配置時:
     - パーツのルート変換行列（`parent_world`）として、原点ではなくスケルトンのアタッチ対象ボーン（`Bip01 Head`）のワールド変換を適用する。
   - アニメーション更新時:
     - 対象ボーンの現在ワールド変換行列で各メッシュの GPU モデル行列バッファ（ModelUniform）を毎フレーム書き換え、頭部の動きに剛体追従させる。

### 4.8 セル内アクター配置とマルチパーツ動的構築 (Phase 6-C)

1. **ACHR / NPC_ レコードからの検出**:
   - セル内の配置参照（REFR）のうち、ベースオブジェクトが `NPC_` であるもの、またはレコード種別が `ACHR` であるものを検出。
   - 静的 3D メッシュ（STAT, FURN 等）のパイプラインから分離し、アクター生成キューへ登録。
2. **マルチパーツ動的アセンブリ**:
   - 性別（`is_female`）および防具（`default_armor` → `ARMO`）から、頭部、両眼球、上下歯、舌、素体/衣装、両手（男/女別）のパーツパスリストを構築。
   - スケルトン（`skeleton.nif`）およびパーツ NIF を VFS からロード（キャッシュにより同一パーツは再利用）。
3. **ワールド空間トランスフォーム配置 (`RenderActorInstance`)**:
   - アクターのワールド位置（`position`）、回転（`rotation`）、スケール（`scale`）から `world_transform`（$T_{\text{actor}}$）を計算。
   - スキンメッシュの頂点はスケルトン原点基準のローカル変形を行い、GPU 頂点シェーダーで $T_{\text{actor}}$ を乗算。
   - 剛体パーツ（目・歯・舌）はボーンワールド行列 $M_{\text{bone}}(t)$ に $T_{\text{actor}}$ を合成（$T_{\text{actor}} \cdot M_{\text{bone}}(t)$）してモデル Uniform バッファへ毎フレーム書き込み。
4. **独立アイドルアニメーション再生**:
   - 各アクターインスタンスが独立した `AnimationPlayer` を保持し、セル内の複数 NPC が同時に自然なアイドル動作を継続。

### 4.9 剛体ヘッドパーツ（髪・目・口）の 90度回転原因と補正トランスフォーム

1. **現象と原因**:
   - `Bip01 Head` ボーン（3ds Max Biped スケルトン）のローカル座標系は、長手方向（頭の垂直軸）が $X$ 軸、前面が $Y$ 軸、左右が $-Z$ 軸という 90度回転した軸配置になっている。
   - 一方、剛体ヘッドパーツ（髪 `hair*.nif`、目 `eye*.nif`、歯 `teeth*.nif`、舌 `tongue*.nif`）のメッシュ頂点は、すべて頭部中心原点 $(0, 0, 0)$ に対し、$Z$ 軸が上、$Y$ 軸が前、$X$ 軸が右という正立頭部空間で定義されている。
   - `eyelefthuman.nif` 等の NIF ルートノード（Block 0）には、この Biped Head 座標系から正立頭部空間へ変換する逆回転行列 $R_{\text{head\_to\_up}} = \begin{bmatrix} 0 & 0 & 1 \\ 0 & 1 & 0 \\ -1 & 0 & 0 \end{bmatrix}$ が記録されている。
   - しかし、コード側で NIF ルートノードの階層トランスフォームを無視し、子シェイプのローカル行列のみを取得していたこと、および `Hair` NIF のようにルートが単位行列であるパーツに対して $R_{\text{head\_to\_up}}$ を適用していなかったことにより、ボーンの 90度回転が直接頂点に作用して横倒しになっていた。

2. **Gamebryo 2.6 アタッチメント合成仕様**:
   - `Bip01 Head` に剛体アタッチするパーツのワールド変換行列 $M_{\text{rigid}}$ は以下で計算される:
     $$M_{\text{rigid}} = M_{\text{actor}} \cdot M_{\text{bone}} \cdot M_{\text{align}} \cdot M_{\text{nif\_local}}$$
     ここで $M_{\text{align}}$ は、パーツのルートノードに $R_{\text{head\_to\_up}}$ が既に含まれている場合は単位行列、含まれていない場合（または頭部空間正立メッシュ）は $R_{\text{head\_to\_up}}$ を適用して姿勢を整合させる。
     これにより、頭部の首振り・傾き・アイドルアニメーションに対して、髪・目・歯・舌が一切ずれずに完全に追従する。

### 4.10 NPC のインベントリ (CNTO) による防具・衣装解決仕様

1. **Fallout 3 ESM における NPC 装備の一次構造**:
   - 参照元: `references/openmw/components/esm4/loadnpc.cpp:60`, `inventory.hpp:47`
   - `NPC_` レコードにおいて、`DOFT`（Default Outfit）や `WNAM`（Default Armor）が指定されていない NPC（Colin Moriarty, Nova, Gob 等）は、インベントリ `CNTO`（Container Item）サブレコード群を所持している。
   - `CNTO` サブレコードのバイナリレイアウト (8 bytes):
     - `item`: `FormId` (4 bytes, little-endian)
     - `count`: `u32` (4 bytes, little-endian)
2. **装備防具の優先解決順序**:
   1. `DOFT`（Default Outfit レコード → `INAM` アイテムリスト中の `ARMO`）
   2. `WNAM`（直接指定の標準防具 `ARMO`）
   3. `CNTO`（所持品リストの中で、`ARMO` レコードに該当するアイテムの先頭）
   4. フォールバック: 素体（男性: `_male/upperbody.nif`, 女性: `_female/upperbody.nif`）

### 4.11 Bip01 NonAccum ルートモーション蓄積と FK 高さ二重加算防止

1. **バグの根本原因**:
   - Fallout 3 のスケルトン（`skeleton.nif`）では、腰のバインドポーズの高さ $Z \approx 67.77$ が親ノード `Bip01` の translation に格納されており、その子である `Bip01 NonAccum` は単位変換 $(0, 0, 0)$ を持つ。
   - 一方、アニメーションファイル（KF）では、腰のワールド/キャラクタールート空間からの絶対移動量（例: $Z \approx 66.33$）が直接 `Bip01 NonAccum` の translation トラックに記録されている。
   - 通常の FK（順運動学）再帰計算で親のバインドポーズ translation（67.77）と子の KF translation（66.33）を加算すると、$Z = 134.10$ に跳ね上がり、腰・腕・頭部など上半身全体が空中に浮遊し、地面に残った下半身メッシュと引き裂かれる。
2. **Gamebryo 2.6 ルートモーション蓄積仕様**:
   - `Bip01` は蓄積ルート（Accumulation Root）であり、アニメーション再生中、`Bip01 NonAccum` が移動トラックを持つ場合は `Bip01` の translation は $(0, 0, 0)$（ゼロリセット）として処理される。
   - これにより、腰（`Bip01 Pelvis`）の高さは KF の $Z = 66.33$ に正確に収まり、頭部（$Z \approx 111.4$）、手先（$Z \approx 64.7$）が地面基準の完璧な人間プロポーションで整合する。

### 4.12 スキンメッシュと剛体パーツの正確な 1:1 マッピング（曖昧名前検索の廃止）

1. **名前検索（`position`）の破綻原因**:
   - `upperbody.nif` の `Arms:0` / `Arms:1` や、同一メッシュ名・空文字列 `""` を持つ複数のシェイプが存在する場合、`collect_anim_skin_meshes_for_names` での `position(|name| *name == mesh_name)` による名前逆引きは常に先頭のインデックスにマッチする。
   - その結果、2つ目以降のシェイプが `AnimatedSkinMesh` に登録されず、初期の T ポーズのまま放置され、Tポーズの腕とアニメーション中の身体が二重描画・浮遊する。
2. **シーングラフ走査（`traverse_block`）時のダイレクト登録**:
   - メッシュ生成直後に、生成された `mesh_index` と NIF の `skin_instance` / `geo_data` を直接 1:1 で `AnimatedSkinMesh` / `AnimatedRigidMesh` へ登録し、名前による曖昧な検索を完全に廃止する。

### 4.13 NPC の頭部・髪型・種族の多様化仕様

1. **種族別頭部メッシュ解決**:
   - `NPC_` の `RNAM`（Race FormID）を判定。
   - グール（`GhoulRace`: `0x00003B3E` 等）の場合は `meshes\characters\head\headghoul.nif` を適用。
   - 通常人間は `meshes\characters\head\headhuman.nif` を適用。
2. **髪型モデル（HNAM）の解決**:
   - 各 NPC の `HNAM` から対応する `HAIR` レコードのモデルパス（例: `HairMessy02`, `HairMessy03F`, `HairGhoul01` 等）をロード。
3. **性別別手メッシュの適用**:
   - 女性（`is_female == true`）: `femalerighthand.nif` / `femalelefthand.nif`
   - 男性（`is_female == false`）: `righthand.nif` / `lefthand.nif`

---

## 5. 実装ステータス (2026-09-12 更新)

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
| 全身マルチパーツ自動結合 (Actor モード) | 完了（頭部・両目・口内・手先・防具/素体の一括制御） |
| セル内アクター自動配置 & アニメーション再生 (Phase 6-C) | **完了** (`RenderActorInstance`, `scene.add_actor`, `update_actors`) |
| GPU シェーダースキニング（ボーン行列パレット） (Phase 6-D) | **完了** (`GpuBonePalette`, `skinned_shader.wgsl`, `create_gpu_skin_mesh_from_partition`) |

### アクター & スキニング実装ファイル
- `crates/fo3_nif/src/blocks/skin.rs` - `NiSkinData` / `BoneData` / `BoneVertData`
- `crates/fo3_render/src/skinning.rs` - `apply_skinning_cpu` / `apply_skinning_cpu_with_bones` (CPU フォールバック)
- `crates/fo3_render/src/gpu_skin.rs` - `GpuBonePalette` / `create_gpu_skin_mesh_from_partition` (Phase 6-D)
- `crates/fo3_render/src/skinned_shader.wgsl` - 4ボーン LBS GPU 頂点シェーダー (Phase 6-D)
- `crates/fo3_render/src/animation.rs` - `AnimationClip` / `AnimationPlayer` / `apply_pose` / B-Spline 評価
- `crates/fo3_render/src/mesh.rs` - `GpuMesh::from_tri_shape_skinned`
- `crates/fo3_render/src/scene.rs` - `RenderActorInstance` / `add_actor` / `update_actors` / HeadParts 剛体追従
- `crates/fo3_viewer/src/main.rs` - セル内 ACHR 自動検出・マルチパーツ動的構築・アニメーション更新ループ
