# Gamebryo 2.6 / Fallout 3 法線マップ & スペキュラ反射仕様書

## 1. 参照文献 (Primary References)
- `references/openmw/components/nifosg/nifloader.cpp:L2401-2426` (`handleTextureSet`)
- `references/openmw/components/nif/property.hpp:L167` (`BSShaderPPLightingProperty`)
- `references/openmw/components/nif/data.hpp` (`BSShaderTextureSet`)
- `references/nifxml/nif.xml:L1234` (`NiTriShapeData`, `BSShaderTextureSet`)
- Gamebryo 2.6 `NiTexturingProperty`, `NiSpecularProperty`, `NiMaterialProperty`

---

## 2. テクスチャスロット仕様 (`BSShaderTextureSet`)

Fallout 3 のマテリアルプロパティ `BSShaderPPLightingProperty` に紐付く `BSShaderTextureSet` は、固定のテクスチャスロット配列を保持する。

| スロット | 役割 | 命名規則例 | 内容とチャンネル構成 |
| :--- | :--- | :--- | :--- |
| **Slot 0** | **Diffuse Map** | `*_d.dds` | RGB: 拡散反射色, A: 透過度 (Alpha Test / Blend) |
| **Slot 1** | **Normal Map** | `*_n.dds` | **RGB: タンジェント空間法線** ($[-1, 1]$ への展開: $2.0 \times C - 1.0$)<br>**A: グロス / スペキュラ強度** (Gloss / Specular Mask) |
| **Slot 2** | **Glow / Emissive**| `*_g.dds` | RGB: 自己発光色 (暗所でも発光するランプや計器パネル) |
| **Slot 3** | 未使用 / Parallax | `*_p.dds` | 視差遮蔽 (Fallout 3 ではほぼ不使用) |
| **Slot 4** | **Environment Cube**| `*_e.dds` | 反射環境マップ (キューブマップ) |
| **Slot 5** | **Environment Mask**| `*_m.dds` | 環境マップ反射強度マスク |

---

## 3. タンジェント空間ベクトル (`NiTriShapeData` / `NiTriStripsData`)

Fallout 3 の NIF ジオメトリデータでは、`BS Data Flags & 4096 != 0` の場合、頂点ごとにタンジェントベクトル (`tangents: [f32; 3]`) および従法線/ビットタンジェントベクトル (`bitangents: [f32; 3]`) が格納されている。

### TBN 行列の構築
ワールド空間への変換行列 $M_{\text{normal}}$ (ワールド行列の 3x3 成分) を用いて:
$$T = \text{normalize}(M_{\text{normal}} \cdot \text{tangent})$$
$$B = \text{normalize}(M_{\text{normal}} \cdot \text{bitangent})$$
$$N = \text{normalize}(M_{\text{normal}} \cdot \text{normal})$$

TBN 基底行列:
$$\text{TBN} = \begin{bmatrix} T_x & B_x & N_x \\ T_y & B_y & N_y \\ T_z & B_z & N_z \end{bmatrix}$$

法線マップからのサンプリング値 $S \in [0, 1]^3$:
$$N_{\text{tangent}} = \text{normalize}(S \times 2.0 - 1.0)$$
ワールド空間法線:
$$N_{\text{world}} = \text{normalize}(\text{TBN} \cdot N_{\text{tangent}})$$

もし頂点にタンジェントが格納されていない場合（$|T| < 0.001$）は、幾何法線 $N$ をそのままワールド法線とする。

---

## 4. スペキュラ反射計算 (Blinn-Phong)

Gamebryo 2.6 のスペキュラ反射は Blinn-Phong モデルに準拠する。

### 計算式
- カメラ視線方向: $V = \text{normalize}(\text{camera\_pos} - \text{world\_pos})$
- 光源方向: $L = \text{normalize}(\text{light\_pos} - \text{world\_pos})$
- ハーフベクトル: $H = \text{normalize}(L + V)$
- 法線との内積: $N \cdot H = \max(N_{\text{world}} \cdot H, 0.0)$
- 鏡面反射係数:
  $$\text{specular} = (N \cdot H)^{\text{shininess}} \cdot \text{gloss} \cdot (N \cdot L > 0)$$
  - $\text{shininess}$: デフォルト $32.0$ (NiMaterialProperty の glossiness)
  - $\text{gloss}$: 法線マップの Alpha チャンネル ($A$)
- 合成:
  $$\text{light\_color} \times (\text{diffuse} \cdot (N \cdot L) + \text{specular})$$
