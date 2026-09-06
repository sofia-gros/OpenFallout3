# wgpu レンダリングパイプラインとメッシュビューアーの設計

Gamebryo 2.6 のシーングラフおよび NIF ジオメトリを GPU (WebGPU / wgpu) 上で描画するパイプラインとビューアー (`fo3_render`, `fo3_viewer`) の仕様メモ。

---

## 1. 座標系 (Coordinate System)

- **Fallout 3 / Gamebryo**: **Z-up (右手系)**
  - `+X`: 右 (East)
  - `+Y`: 前 (North)
  - `+Z`: 上 (Up)
- **カメラ行列の設計**:
  - 一般的なゲームエンジン（Y-up）に合わせてモデルを回転させると、将来の物理エンジン (Havok) やアニメーション (KF) との整合性が破綻するため、モデル側の座標変換は行わない。
  - カメラの Up ベクトルを `Vec3::Z` としたオービットカメラ (`OrbitCamera`) を使用し、Z-up 空間で直接ビュー行列 (`Mat4::look_at_rh`) を計算する。

---

## 2. トランスフォーム伝播 (Transform Cascading)

Gamebryo 2.6 の `NiAVObject::UpdateDownwardPass` に完全準拠:
```rust
R_world = R_parent * R_local
S_world = S_parent * S_local
T_world = R_parent * (S_parent * T_local) + T_parent
```
GPU シェーダーに渡すワールド行列:
```rust
world_mat = Mat4::from_translation(translation)
    * Mat4::from_mat3(rotation)
    * Mat4::from_scale(Vec3::splat(scale));
```

---

## 3. ジオメトリから GPU バッファへの変換

- **`NiTriShapeData`**:
  - 各三角形の頂点インデックス (`v1, v2, v3: u16`) をそのまま `TriangleList` としてバッファ化。
- **`NiTriStripsData`**:
  - トライアングルストリップ列を三角形リストに展開（OpenMW `nifloader.cpp:L1613-1620` 準拠）。
  - 縮退三角形 (`a == b || b == c || a == c`) は除外。

---

## 4. BC 圧縮テクスチャ (DDS) と wgpu の制約事項

- **DXT1 (BC1), DXT3 (BC2), DXT5 (BC3)** は `wgpu::TextureFormat::Bc1RgbaUnorm` / `Bc2` / `Bc3` で GPU に直接アップロード。
- **重要な wgpu / WebGPU 制約**:
  - BC 圧縮テクスチャのコピーサイズ（`width`, `height`）は**必ずブロック幅（4 ピクセル）の倍数**でなければならない。
  - DDS 内のミップマップのうち、幅または高さが 4 未満（2x2 や 1x1）のミップレベルは wgpu の `write_texture` ではコピーできないため、`mip_w >= 4 && mip_h >= 4` のレベルのみをテクスチャの `mip_level_count` として確保・転送する。
