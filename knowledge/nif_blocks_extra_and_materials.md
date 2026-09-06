# Fallout 3 NIF ExtraData & マテリアルプロパティ仕様書

本ドキュメントは `references/nifxml/nif.xml` に基づき、Fallout 3 (`v20.2.0.7`, `user_version == 11`, `user_version2 == 34`) における主要な ExtraData ブロックおよび未対応マテリアルブロックのバイナリレイアウトを記録した永続ドキュメントである。

---

## 1. ExtraData ブロック群 (NiExtraData 派生)

### 1.1 NiExtraData (基底クラス)
- 参照元: `references/nifxml/nif.xml:L3220`
- `Name`: `string` (`since="20.1.0.3"` のため `NiFixedString` = 文字列プールへのインデックス `u32`)

### 1.2 NiStringExtraData
- 参照元: `references/nifxml/nif.xml:L5163`
- 継承: `NiExtraData`
- `name_index`: `u32`
- `string_data_index`: `u32` (`since="20.1.0.3"` のため `NiFixedString` = 文字列プールへのインデックス)

### 1.3 NiIntegerExtraData / BSXFlags
- 参照元: `references/nifxml/nif.xml:L4293, L4298`
- 継承: `NiExtraData` -> `NiIntegerExtraData` -> `BSXFlags`
- `name_index`: `u32`
- `integer_data`: `u32` (ビットフラグ)

#### BSXFlags ビット定義:
- Bit 0: `enable_havok` (物理有効)
- Bit 1: `enable_collision` (コリジョン有効)
- Bit 2: `is_skeleton` (スケルトン NIF)
- Bit 3: `enable_animation` (アニメーション有効)
- Bit 4: `flame_nodes_present`
- Bit 5: `editor_markers_present` (エディタマーカー表示)
- Bit 6: `is_dynamic` (動的オブジェクト)
- Bit 7: `is_articulated`
- Bit 8: `needs_transform_updates`

### 1.4 NiFloatExtraData
- 参照元: `references/nifxml/nif.xml:L4264`
- 継承: `NiExtraData`
- `name_index`: `u32`
- `float_data`: `f32`

### 1.5 BSBound
- 参照元: `references/nifxml/nif.xml:L3932`
- 継承: `NiExtraData`
- `name_index`: `u32`
- `center`: `Vector3` (3x f32: 中心座標)
- `dimensions`: `Vector3` (3x f32: ハーフエクステント / 半径)

---

## 2. マテリアル・プロパティブロック群

### 2.1 BSShaderNoLightingProperty
- 参照元: `references/nifxml/nif.xml:L6220, L6228, L6233`
- 継承: `NiProperty` -> `NiShadeProperty` -> `BSShaderProperty` -> `BSShaderLightingProperty` -> `BSShaderNoLightingProperty`

| フィールド | 型 | 説明 |
|---|---|---|
| `net` | `NiObjectNET` | 名前インデックス、追加データ参照リスト、コントローラー |
| `shade_flags` | `u16` | スムースシェーディング等フラグ |
| `shader_type` | `u32` | シェーダー種別 (`BSShaderType`) |
| `shader_flags` | `u32` | シェーダーフラグ 1 |
| `shader_flags2` | `u32` | シェーダーフラグ 2 |
| `env_map_scale` | `f32` | 環境マップ強度 |
| `texture_clamp_mode` | `u32` | テクスチャクランプモード (`TexClampMode`) |
| `file_name` | `SizedString` | 発光テクスチャファイルパス |
| `falloff_start_angle` | `f32` | フォールオフ開始角 |
| `falloff_stop_angle` | `f32` | フォールオフ終了角 |
| `falloff_start_opacity` | `f32` | 開始不透明度 |
| `falloff_stop_opacity` | `f32` | 終了不透明度 |

### 2.2 NiStencilProperty
- 参照元: `references/nifxml/nif.xml:L5147, L1572`
- 継承: `NiProperty` -> `NiStencilProperty`

| フィールド | 型 | 説明 |
|---|---|---|
| `net` | `NiObjectNET` | 名前インデックス、追加データリスト、コントローラー |
| `flags` | `u16` | `StencilFlags`: Bit 0: Enable, Bit 10-11: Draw Mode (0=CCW, 1=CW, 2/3=Both/両面) |
| `stencil_ref` | `u32` | ステンシル参照値 |
| `stencil_mask` | `u32` | ステンシルビットマスク (通常 0xFFFFFFFF) |
