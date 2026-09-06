# Fallout 3 Havok コリジョンブロック詳細仕様

本ドキュメントは Fallout 3 (Gamebryo 2.6 / Havok 物理) の NIF ファイル内に埋め込まれる物理コリジョンブロック群のバイナリ構造、継承階層、およびジオメトリ抽出仕様を記録した永続化知識ベースです。

---

## 1. 参照文献・一次資料
- `references/nifxml/nif.xml:L2284` (`HavokFilter`)
- `references/nifxml/nif.xml:L2301` (`hkSubPartData`)
- `references/nifxml/nif.xml:L2765` (`bhkWorldObject`)
- `references/nifxml/nif.xml:L2790` (`bhkSimpleShapePhantom`)
- `references/nifxml/nif.xml:L2803` (`bhkEntity`)
- `references/nifxml/nif.xml:L2808` (`bhkRigidBodyCInfo550_660`)
- `references/nifxml/nif.xml:L2931` (`bhkRigidBody`)
- `references/nifxml/nif.xml:L3029` (`bhkTransformShape`)
- `references/nifxml/nif.xml:L3066` (`bhkSphereShape`)
- `references/nifxml/nif.xml:L3079` (`bhkCapsuleShape`)
- `references/nifxml/nif.xml:L3088` (`bhkBoxShape`)
- `references/nifxml/nif.xml:L3134` (`bhkBvTreeShape`)
- `references/nifxml/nif.xml:L3153` (`bhkMoppBvTreeShape`)
- `references/nifxml/nif.xml:L3165` (`bhkListShape`)
- `references/nifxml/nif.xml:L3193` (`bhkPackedNiTriStripsShape`)
- `references/nifxml/nif.xml:L3207` (`bhkNiTriStripsShape`)
- `references/nifxml/nif.xml:L3403` (`bhkNiCollisionObject`)
- `references/nifxml/nif.xml:L3420` (`bhkCollisionObject`)
- `references/nifxml/nif.xml:L3424` (`bhkBlendCollisionObject`)
- `references/nifxml/nif.xml:L3436` (`bhkSPCollisionObject`)
- `references/nifxml/nif.xml:L3958` (`hkPackedNiTriStripsData`)

---

## 2. Havok コリジョンブロックのクラス継承ツリー

```
NiObject
 ├── NiCollisionObject
 │    └── bhkNiCollisionObject
 │         ├── bhkCollisionObject (通常の剛体アタッチ)
 │         │    └── bhkBlendCollisionObject (スケルトンボーン追従剛体)
 │         └── bhkPCollisionObject
 │              └── bhkSPCollisionObject (Shape Phantom 用アタッチ)
 ├── bhkSerializable
 │    ├── bhkWorldObject (Havok ワールド所属オブジェクト基底)
 │    │    ├── bhkEntity
 │    │    │    └── bhkRigidBody / bhkRigidBodyT (剛体インスタンス)
 │    │    └── bhkPhantom
 │    │         └── bhkShapePhantom
 │    │              └── bhkSimpleShapePhantom (トリガーゾーン・任意シェイプ)
 │    └── bhkShape (コリジョン形状基底)
 │         ├── bhkTransformShape (ローカル変換付きシェイプ)
 │         ├── bhkSphereShape (球)
 │         ├── bhkBoxShape (直方体)
 │         ├── bhkCapsuleShape (カプセル)
 │         ├── bhkConvexVerticesShape (凸包ポリゴン)
 │         ├── bhkBvTreeShape
 │         │    └── bhkMoppBvTreeShape (MOPP 木加速静的メッシュ)
 │         └── bhkShapeCollection
 │              ├── bhkListShape (複合シェイプ)
 │              ├── bhkConvexListShape (凸複合シェイプ)
 │              ├── bhkPackedNiTriStripsShape (パック済みストリップメッシュ)
 │              │    └── データ本体: hkPackedNiTriStripsData
 │              └── bhkNiTriStripsShape (NiTriStripsData 参照シェイプ)
```

---

## 3. 各コリジョン形状ブロックのバイナリレイアウト

### ① `bhkTransformShape` (`nif.xml:L3029`)
子シェイプに対してローカル $4 \times 4$ 変換行列を適用するラッパーシェイプ。
- `shape`: `i32` (Ref template="bhkShape")
- `material`: `u32` (`Fallout3HavokMaterial`)
- `radius`: `f32` (通常 0.1)
- `unused_01`: `[u8; 8]`
- `transform`: `[f32; 16]` ($4 \times 4$ 変換行列、行優先/列優先は Gamebryo 規約)

### ② `bhkNiTriStripsShape` (`nif.xml:L3207`)
`NiTriStripsData` をジオメトリデータとして直接参照する静的メッシュコリジョン。
- `material`: `u32` (`Fallout3HavokMaterial`)
- `radius`: `f32` (0.1)
- `unused_01`: `[u8; 20]`
- `grow_by`: `u32` (1)
- `scale`: `[f32; 4]`
- `num_strips_data`: `u32`
- `strips_data`: `Vec<i32>` (`num_strips_data` 個の `NiTriStripsData` ブロック参照)
- `num_filters`: `u32`
- `filters`: `Vec<u32>` (`num_filters` 個の `HavokFilter`)

### ③ `bhkSimpleShapePhantom` (`nif.xml:L2790`)
トリガーゾーンやイベント発生領域として使用される非物理ファントムオブジェクト。
- `common`: `BhkWorldObjectCommon`
  - `shape`: `i32` (Ref template="bhkShape")
  - `havok_filter_layer`: `u8`
  - `havok_filter_flags`: `u8`
  - `havok_filter_group`: `u16`
  - `broad_phase_type`: `u8`
  - `prop_data`: `u32`
  - `prop_size`: `u32`
  - `prop_capacity_and_flags`: `u32`
- `unused_01`: `[u8; 8]`
- `transform`: `[f32; 16]` ($4 \times 4$ 変換行列)

### ④ `bhkSPCollisionObject` (`nif.xml:L3436`)
`bhkCollisionObject` と完全に同一バイナリ構造を持つファントム用アタッチオブジェクト。
- `target`: `i32` (Ptr `NiAVObject`)
- `flags`: `u16` (`bhkCOFlags`)
- `body`: `i32` (Ref `bhkSimpleShapePhantom`)

---

## 4. 物理エンジン（Rapier / Havok）連携用共通表現

物理エンジンへの流し込みおよびレイキャスト走査のため、NIF ツリーから以下の統一列挙型へ変換して取り出します。

```rust
pub enum CollisionShapeGeometry {
    Box { half_extents: [f32; 3] },
    Sphere { radius: f32 },
    Capsule { radius: f32, p1: [f32; 3], p2: [f32; 3] },
    ConvexHull { vertices: Vec<[f32; 3]> },
    TriMesh {
        vertices: Vec<[f32; 3]>,
        indices: Vec<[u32; 3]>,
    },
    Compound(Vec<(Transform, CollisionShapeGeometry)>),
}
```
これにより、Rapier やカスタム Havok 物理エンジンなどのバックエンドを完全に交換可能なアーキテクチャを実現します。
