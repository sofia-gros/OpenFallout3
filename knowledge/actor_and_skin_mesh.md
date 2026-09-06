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

## 4. 実装ステータス (2026-09-07 更新)

| 項目 | 状態 |
|---|---|
| `NiSkinData` パーサー (`fo3_nif::blocks::skin`) | 完了 |
| `BoneData`, `BoneVertData` パーサー | 完了 |
| `NiSkinInstance` パーサー | 完了 |
| `NiSkinPartition` + `SkinPartition` パーサー | 完了 |
| CPU スキニング (`fo3_render::skinning`) | 完了（バインドポーズ T-Pose） |
| `GpuMesh::from_tri_shape_skinned` | 完了 |
| `scene.rs` スキニング分岐 | 完了 |
| GPU シェーダースキニング（ボーン行列パレット） | 未実装 |
| KF アニメーション再生 | 未実装 |

### CPU スキニング実装ファイル
- `crates/fo3_nif/src/blocks/skin.rs` - `NiSkinData` / `BoneData` / `BoneVertData`
- `crates/fo3_render/src/skinning.rs` - `apply_skinning_cpu`
- `crates/fo3_render/src/mesh.rs` - `GpuMesh::from_tri_shape_skinned`
- `crates/fo3_render/src/scene.rs` - `NiTriShape` スキニング分岐
