# Fallout 3 NIF ジオメトリ & マテリアルブロック詳細バイナリ仕様

- **対象ゲーム**: Fallout 3 (v20.2.0.7 / user_version 11 / bs_version 34)
- **参照元**:
  - `references/nifxml/nif.xml:L3359` (`NiObjectNET`)
  - `references/nifxml/nif.xml:L3440` (`NiAVObject`)
  - `references/nifxml/nif.xml:L4384` (`NiNode`)
  - `references/nifxml/nif.xml:L6131` (`BSFadeNode`)
  - `references/nifxml/nif.xml:L3853` (`NiGeometry`)
  - `references/nifxml/nif.xml:L3872` (`NiTriBasedGeom`)
  - `references/nifxml/nif.xml:L5278` (`NiTriShape`)
  - `references/nifxml/nif.xml:L3876` (`NiGeometryData`)
  - `references/nifxml/nif.xml:L5282` (`NiTriShapeData`)
  - `references/nifxml/nif.xml:L6242` (`BSShaderPPLightingProperty`)
  - `references/nifxml/nif.xml:L6307` (`BSShaderTextureSet`)

---

## 1. 共通基底ブロック

### 1.1 `NiObjectNET`
```rust
pub struct NiObjectNET {
    pub name: u32,                  // IndexString (Header.strings に対するインデックス)
    pub extra_data_list: Vec<i32>,  // Ref<NiExtraData>[]
    pub controller: i32,            // Ref<NiTimeController> (-1 は None)
}
```

### 1.2 `NiAVObject` (`NiObjectNET` を継承)
```rust
pub struct NiAVObject {
    pub net: NiObjectNET,
    pub flags: u32,                 // FO3 (bs_version 34 > 26) では u32
    pub translation: [f32; 3],      // X, Y, Z
    pub rotation: [f32; 9],         // 3x3 回転行列
    pub scale: f32,                 // 均等スケール
    pub properties: Vec<i32>,       // Ref<NiProperty>[]
    pub collision_object: i32,      // Ref<NiCollisionObject> (-1 は None)
}
```

---

## 2. ノード階層ブロック

### 2.1 `NiNode` (`NiAVObject` を継承)
```rust
pub struct NiNode {
    pub av: NiAVObject,
    pub children: Vec<i32>,         // Ref<NiAVObject>[] (子ノード一覧)
    pub effects: Vec<i32>,          // Ref<NiDynamicEffect>[]
}
```

### 2.2 `BSFadeNode` (`NiNode` を継承)
- 追加フィールドなし（バイナリ構造は `NiNode` と完全に同一）。

---

## 3. ジオメトリ & メッシュブロック

### 3.1 `NiGeometry` (`NiAVObject` を継承)
- `data`: `i32` (`Ref<NiGeometryData>`)
- `skin_instance`: `i32` (`Ref<NiSkinInstance>`)
- `material_data`:
  - `num_materials`: `u32`
  - `material_names`: `[u32; num_materials]` (IndexString)
  - `material_extra_data`: `[i32; num_materials]`
  - `active_material`: `i32`
  - `material_needs_update`: `u8` (bool)

### 3.2 `NiTriShape` (`NiGeometry` を継承)
- 追加フィールドなし。

### 3.3 `NiTriShapeData` (`NiGeometryData` を継承)
```rust
pub struct NiTriShapeData {
    // --- NiGeometryData ---
    pub group_id: i32,              // 常に 0
    pub num_vertices: u16,          // 頂点数
    pub keep_flags: u8,
    pub compress_flags: u8,
    pub has_vertices: bool,
    pub vertices: Vec<[f32; 3]>,    // 頂点座標
    pub bs_data_flags: u16,         // ビット 0: UV数 (1), ビット 12 (4096): Tangents/Bitangents 有無
    pub has_normals: bool,
    pub normals: Vec<[f32; 3]>,     // 法線ベクトル
    pub tangents: Vec<[f32; 3]>,    // 接線ベクトル (bs_data_flags & 4096 != 0 の場合)
    pub bitangents: Vec<[f32; 3]>,  // 従法線ベクトル (bs_data_flags & 4096 != 0 の場合)
    pub center: [f32; 3],           // ローカルバウンディングスフィア中心
    pub radius: f32,                // ローカルバウンディングスフィア半径
    pub has_vertex_colors: bool,
    pub vertex_colors: Vec<[f32; 4]>,
    pub uv_sets: Vec<Vec<[f32; 2]>>,// UVテクスチャ座標 (num_uv_sets = bs_data_flags & 1)
    pub consistency_flags: u16,
    pub additional_data: i32,

    // --- NiTriBasedGeomData ---
    pub num_triangles: u16,         // 三角形ポリゴン数

    // --- NiTriShapeData ---
    pub num_triangle_points: u32,   // num_triangles * 3
    pub has_triangles: bool,
    pub triangles: Vec<[u16; 3]>,   // 三角形インデックス配列 (頂点番号タプル)
    pub num_match_groups: u16,
    pub match_groups: Vec<Vec<u16>>,
}
```

---

## 4. マテリアル & テクスチャブロック

### 4.1 `BSShaderTextureSet` (`NiObject` を継承)
```rust
pub struct BSShaderTextureSet {
    pub textures: Vec<String>,      // 通常 6〜8 件の SizedString (len: u32 + chars)
}
```
- スロット割当:
  - `[0]`: Diffuse
  - `[1]`: Normal / Gloss
  - `[2]`: Glow / Rim / Skin
  - `[3]`: Height / Parallax
  - `[4]`: Environment / Cubemap
  - `[5]`: Environment Mask

### 4.2 `BSShaderPPLightingProperty`
- `shader_type`: `u32`
- `shader_flags`: `u32`
- `shader_flags2`: `u32`
- `environment_map_scale`: `f32`
- `texture_clamp_mode`: `u32`
- `texture_set`: `i32` (`Ref<BSShaderTextureSet>`)
- `refraction_strength`: `f32`
- `refraction_fire_period`: `i32`
- `parallax_max_passes`: `f32`
- `parallax_scale`: `f32`
