# Gamebryo 2.6 / Fallout 3 レンダリング・シェーダー・マテリアル仕様

## 1. 概要
Fallout 3 のレンダリングパイプラインは、Gamebryo 2.6 のシーングラフ構造およびピクセル単位ライティングシェーダー (`BSShaderPPLightingProperty`) に基づいています。

---

## 2. マテリアルプロパティ (`NiMaterialProperty`)

一次文献:
- `references/nifxml/nif.xml:L4363` (`NiMaterialProperty`)
- `references/nifskope/src/gl/glproperty.cpp:L90-130`

### フィールド構成
| フィールド | 型 | 説明 |
|---|---|---|
| `specular_color` | `Color3` (RGB) | 鏡面反射ハイライトの色 |
| `emissive_color` | `Color3` (RGB) | 自己発光のベース色 |
| `glossiness` | `f32` | 鏡面反射の鋭さ (Blinn-Phong 指数, 0.0〜100.0) |
| `alpha` | `f32` | マテリアル全体の不透明度 (0.0=透明, 1.0=不透明) |
| `emissive_mult` | `f32` | 自己発光の乗算係数 (通常 1.0〜5.0) |

---

## 3. テクスチャセットスロット (`BSShaderTextureSet`)

一次文献:
- `references/nifxml/nif.xml:L6307` (`BSShaderTextureSet`)
- `references/openmw/components/nifosg/nifloader.cpp:L2401-2426`

| スロット | サフィックス | 役割 | 備考 |
|---|---|---|---|
| 0 | なし | Base Color / Diffuse Map | RGB: 基本色, A: アルファテスト/ブレンド透過値 |
| 1 | `_n.dds` | Normal / Gloss Map | RGB: タンジェント空間法線, A: スペキュラ強度 (Gloss) |
| 2 | `_g.dds` / `_glow.dds` | Glow Map / Emissive | RGB: 自己発光マスク (暗所でも減衰しない Unlit) |
| 3 | `_h.dds` / `_p.dds` | Height / Parallax Map | 視差マッピング用グレースケール |
| 4 | `_e.dds` | Environment Map | キューブマップ反射 |
| 5 | `_m.dds` | Environment Mask | キューブマップ反射マスク |

---

## 4. シェーダー計算式

### 4.1. スペキュラ計算 (Blinn-Phong)
Gamebryo 2.6 / Blinn-Phong:
- `H = normalize(L + V)`
- `N_dot_H = max(dot(N, H), 0.0)`
- `spec_factor = pow(N_dot_H, glossiness) * gloss_sample`
- `specular_term = light_color * specular_color * spec_factor`

### 4.2. 自己発光 (Emissive & Glow Map)
蛍光灯や機械のランプ、メーター表示:
- `emissive_base = emissive_color * emissive_mult`
- `has_glow_map == 1` の場合: `emissive_light = glow_sample.rgb * emissive_base`
- `has_glow_map == 0` の場合: `emissive_light = emissive_base` (または 0)
最終色への加算:
- `final_rgb = base_color * diffuse_light + specular_light + emissive_light`
※自己発光項は環境光・平行光・点光源の遮蔽を受けず、常に Unlit として加算される。
