# Gamebryo 2.6 アルファブレンディング & 半透明パスの順序付け

本ドキュメントは、Gamebryo 2.6 エンジンおよび Fallout 3 におけるアルファブレンディング、カットアウト（アルファテスト）、および半透明メッシュの描画順序（Back-to-Front ソート）の仕様と数学モデルを記録する。

---

## 1. NiAlphaProperty フラグ仕様

参照元:
- `references/nifxml/nif.xml:L1519` (`NiAlphaProperty`)
- `references/nifskope/src/gl/glproperty.cpp:L210-250`

`flags` (16ビット整数):
- **bit 0 (0x0001)**: `Alpha Blend Enable`（アルファブレンド有効フラグ）
  - 1 の場合、半透明パス（`transparent_pipeline`）に分類され、深度書き込みが無効化 (`depth_write_enabled = false`) される。
- **bit 1..4 (0x001E)**: `Src Blend Function`
  - 0: One, 1: Zero, 2: SrcAlpha, 3: OneMinusSrcAlpha, 4: DstAlpha, 5: OneMinusDstAlpha, 6: DstColor, 7: OneMinusDstColor, 8: SrcAlphaSat
- **bit 5..8 (0x01E0)**: `Dst Blend Function`
- **bit 9 (0x0200)**: `Alpha Test Enable`（アルファテスト / カットアウト有効フラグ）
  - 1 の場合、フラグメントシェーダー内で閾値比較が行われ、条件を満たさないピクセルは `discard` される。
- **bit 10..12 (0x1C00)**: `Test Function`
  - 0: Always, 1: Less, 2: Equal, 3: LessEqual, 4: Greater, 5: NotEqual, 6: GreaterEqual, 7: Never
  - 標準的な Fallout 3 のカットアウト（金網、葉、髪先）では通常 `Greater` (4) が使用される。
- **bit 13 (0x2000)**: `No Sorter`（ソート除外フラグ）
  - 1 の場合、カメラ距離によるソートを行わず、常に固定順序で描画される（背景用半透明エフェクトなど）。
- **`threshold` (u8, 0..255)**: アルファテストの参照基準値（通常 128 = 0.5）。

---

## 2. バウンディングスフィアのワールド空間変換 (NiBound Update)

参照元:
- Gamebryo 2.6 `NiBound.h`, `NiAVObject::UpdateWorldBound`
- `references/nifxml/nif.xml:L173` (`NiBound`)

### モデル空間バウンド $\mathcal{B}_{\text{model}} = (C_{\text{model}}, R_{\text{model}})$
メッシュデータ（`NiTriShapeData.common.bounding_sphere`）に記録されたローカル中心座標 $C_{\text{model}}$ および半径 $R_{\text{model}}$。

### ワールド変換 $T_{\text{world}} = (R, S, T)$
- 回転行列: $R$ (3x3)
- 一様スケール: $S$ (スカラー)
- 平行移動: $T$ (3次元ベクトル)

### ワールド空間バウンド $\mathcal{B}_{\text{world}} = (C_{\text{world}}, R_{\text{world}})$ の計算式:
$$
C_{\text{world}} = R \cdot (C_{\text{model}} \times S) + T
$$
$$
R_{\text{world}} = R_{\text{model}} \times S
$$

この $C_{\text{world}}$ を `RenderMesh::world_center` として保持することにより、原点からオフセットした半透明部品（眼鏡、装飾、炎エフェクト、窓ガラス）の正確な幾何学的重心が算出される。

---

## 3. レンダリングパスとソート順序

参照元:
- Gamebryo 2.6 パイプライン設計 (`NiRenderer::RenderClick`)

1. **不透明パス (Opaque Pass)**:
   - 対象: `!is_transparent`
   - パイプライン: 深度書き込み有効 (`depth_write_enabled = true`)、アルファブレンド無効。
   - レンダリング順序: 任意（または Front-to-Back による Early-Z 最適化）。
2. **半透明パス (Transparent Pass)**:
   - 対象: `is_transparent`
   - パイプライン: 深度書き込み無効 (`depth_write_enabled = false`)、アルファブレンド有効 (`BlendState::ALPHA_BLENDING`)。
   - ソート順序: `alpha_sort == true` かつ カメラ座標 $\mathbf{E}$ が与えられている場合、二乗距離 $\|\mathbf{C}_{\text{world}} - \mathbf{E}\|^2$ の**降順（Back-to-Front: 最も遠いオブジェクトから最も近いオブジェクトへ）**で描画。
