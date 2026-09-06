# Gamebryo 2.6 クラス階層・シーングラフ仕様

- **対象エンジン**: Gamebryo 2.6 (NetImmerse 系統)
- **参照元**:
  - `references/openmw/components/nif/node.hpp`
  - `references/nifskope/src/spells/mesh.cpp`
  - Gamebryo 2.6 SDK アーキテクチャリファレンス

---

## 1. 主要クラス継承ツリー

```
NiRefObject
 └── NiObject
      └── NiObjectNET
           ├── NiProperty (レンダリングステート基底)
           │    ├── NiMaterialProperty
           │    ├── NiTexturingProperty
           │    ├── NiAlphaProperty
           │    ├── NiZBufferProperty
           │    └── BSShaderPPLightingProperty (Bethesda独自ピクセルシェーダーマテリアル)
           ├── NiTimeController (アニメーション制御基底)
           └── NiAVObject (可視・シーングラフ配置可能オブジェクト基底)
                ├── NiNode (子を持つ階層ノード)
                │    ├── BSFadeNode (LOD距離フェード制御ノード)
                │    └── BSOrderedNode (半透明ソート固定ノード)
                └── NiGeometry (描画プリミティブ基底)
                     └── NiTriBasedGeom
                          ├── NiTriShape (インデックス付き三角形メッシュ)
                          └── NiTriStrips (トライアングルストリップスメッシュ)
```

---

## 2. トランスフォーム伝播規則 (`NiAVObject::UpdateDownwardPass`)

シーングラフのワールドトランスフォーム計算は、親から子へのトップダウンで行われます。

- **ローカルトランスフォーム**:
  - `R_local`: $3 \times 3$ 回転行列 (`NiMatrix3`)
  - `T_local`: 3次元平行移動ベクトル (`NiPoint3`)
  - `S_local`: 均等スケーリング係数 (`f32`)

- **ワールドトランスフォーム計算式**:
  $$R_{world} = R_{parent} \times R_{local}$$
  $$S_{world} = S_{parent} \times S_{local}$$
  $$T_{world} = R_{parent} \times (S_{parent} \times T_{local}) + T_{parent}$$

---

## 3. バウンディングボリューム更新規則 (`NiAVObject::UpdateUpwardPass`)

バウンディングボリューム（包含球: `NiBound { center: Vec3, radius: f32 }`）は、
リーフ（`NiGeometry`）からルートノードへとボトムアップに包含計算されます。

1. `NiGeometry` のローカル包含球（頂点群の最小包含球）に `world_transform` を適用し、`world_bound` を算出。
2. `NiNode` は自身が持つ全子要素の `world_bound` を包含する最小球を計算して自身の `world_bound` とする。

---

## 4. プロパティカスケード規則 (`NiPropertyState`)

Gamebryo では、マテリアルやアルファ設定などの描画ステートが親ノードから子ノードへと継承されます。

1. ツリーをトラバースする際、親ノードに設定されている `NiProperty` は子ノードへ引き継がれる。
2. 子ノード自身が同じ型のプロパティ（例: `NiAlphaProperty`）を持つ場合は、子ノードの設定が優先（上書き）される。
3. 描画時には、最終的な `NiGeometry` に適用されている累積プロパティステートに基づいてグラフィックスパイプラインを設定する。
