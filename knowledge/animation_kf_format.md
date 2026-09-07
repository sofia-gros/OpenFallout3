# Phase B: アニメーション再生基盤 — KF ファイル構造と実装ノート

## 調査日: 2026-09-07

---

## 1. KF ファイルの概要

Fallout 3 の KF ファイルは NIF フォーマットと同じヘッダーを持つ。
ルートブロックは `NiControllerSequence`（version 20.2.0.7 以降）。

### 実アセット確認: `meshes\characters\_male\idleanims\ttnpchappysubtlelistena.kf`
ブロック構成:
- `[000] NiControllerSequence` — アニメーション全体のシーケンス（ボーン名→インターポレータのマッピング）
- `[001] NiTransformInterpolator` (size=36) — バインドポーズ姿勢のみ（データなし）
- `[002] NiTransformData` (size=56) — 少量のキーフレーム（静止）
- `[003..] NiBSplineCompTransformInterpolator` (size=84) — B-Spline 圧縮変換（Fallout 3 で主流）
- `[004] NiBSplineData` — 圧縮制御点データ
- `[005] NiBSplineBasisData` — スプライン基底データ

---

## 2. バイナリレイアウト

### NiTransformInterpolator (nif.xml:L3248)
```
NiQuatTransform:
  Vector3    translation    (12 bytes)
  Quaternion rotation       (16 bytes)
  f32        scale          (4 bytes)
i32          data_ref       (NiTransformData への Ref, -1 = なし)
合計 36 bytes
```

### NiTransformData (nif.xml:L5274, NiKeyframeData:L4327 を継承)
```
u32          num_rotation_keys
if num_rotation_keys > 0:
  u32        rotation_type  (1=LINEAR, 2=QUADRATIC, 4=XYZ_ROTATION)
  if rotation_type != 4:
    [num_rotation_keys] QuatKey { f32 time; Quat value; [tangents if QUADRATIC] }
  if rotation_type == 4:
    [3] KeyGroup<f32>  // XYZ 各軸の float キー群
KeyGroup<Vector3>   translations  (u32 num; if num > 0: u32 type; [num] key)
KeyGroup<f32>       scales        (u32 num; if num > 0: u32 type; [num] key)
```

### KeyGroup<T> (nif.xml:L2007)
```
u32    num_keys
if num_keys > 0:
  u32  interpolation  (KeyType: 1=LINEAR, 2=QUADRATIC, 3=TBC, 5=CONST)
  [num_keys] Key<T>
```

### Key<T> (KeyType に応じて変化)
```
LINEAR: f32 time; T value;
QUADRATIC: f32 time; T value; T forward; T backward;
TBC: f32 time; T value; f32 tension; f32 bias; f32 continuity;
CONST: f32 time; T value;
```

### NiControllerSequence (nif.xml:L4214, NiSequence:L4201 を継承)
```
// NiSequence フィールド
string         name
u32            num_controlled_blocks
u32            array_grow_by
[num_controlled_blocks] ControlledBlock {
  Ref           interpolator           // NiTransformInterpolator or NiBSpline... への参照
  Ref           controller             // NiTimeController (スケルトン NIF 側) への参照 (-1)
  u8            priority
  string palette (ノード名とプロパティ名は StringPalette 経由)
  i16           node_name_offset       // StringPalette 内オフセット
  i16           prop_type_offset
  i16           controller_type_offset
  i16           variable1_offset
  i16           variable2_offset
}
// NiControllerSequence 追加フィールド
f32            weight
Ref            text_keys              // NiTextKeyExtraData
u32            cycle_type             (0=LOOP, 1=REVERSE, 2=CLAMP)
f32            frequency
f32            start_time
f32            stop_time
Ptr            manager                (-1 in KF)
string         accum_root_name
Ref            string_palette         // NiStringPalette への参照 (FO3 では存在)
Ref            anim_notes             // vercond: BSVER 24-28
```

### NiBSplineCompTransformInterpolator (size=84, nif.xml 参照)
```
f32    start_time
f32    stop_time
Ref    spline_data        (NiBSplineData)
Ref    basis_data         (NiBSplineBasisData)
Vector3 translation       (12 bytes)
Quaternion rotation       (16 bytes)
f32    scale
u32    translation_handle (NiBSplineData 内オフセット)
u32    rotation_handle
u32    scale_handle
f32    translation_half_range
f32    rotation_half_range
f32    scale_half_range
合計: 4+4+4+4+12+16+4+4+4+4+4+4+4 = 84 bytes
```

---

## 3. ControlledBlock の StringPalette 解決

FO3 の KF では各 ControlledBlock の文字列（ノード名等）は `NiStringPalette` を通じて解決される。
`NiStringPalette` は文字列をゼロ終端で連結したバッファで、offset で個別の文字列を参照する。

```
NiStringPalette:
  u32    palette_length
  char[] palette          (ゼロ終端文字列を連結したバッファ)
```

ControlledBlock 内のオフセットは `node_name_offset` 等として格納される。

---

## 4. 実装優先順位

| 優先 | ブロック | 理由 |
|:---:|---|---|
| 1 | `NiTransformInterpolator` | すべての静止ボーンが使用 |
| 2 | `NiTransformData` | 線形・四元数キーフレーム |
| 3 | `NiControllerSequence` | ボーン名→インターポレータのマッピング |
| 4 | `NiStringPalette` | ControlledBlock のボーン名解決に必要 |
| 5 | `NiBSplineCompTransformInterpolator` | FO3 で広く使用される圧縮補間 |
| 6 | `NiBSplineData` / `NiBSplineBasisData` | 5 に必要 |

---

## 5. アニメーション更新ループ（設計方針）

1. KF ファイルをロードし `NiControllerSequence` を解析
2. ControlledBlock からボーン名→インターポレータのマップを構築
3. スケルトン NIF から `NiNode` ツリーを構築（ノード名→インデックスのマップ）
4. 毎フレーム: 経過時間 `t` を元に各インターポレータからボーンのローカル変換を評価
5. ボーン階層を再帰的に更新してワールド変換行列を計算
6. `apply_skinning_cpu` の引数としてボーンワールド行列を渡す（将来的にはGPU）

---

## 6. 参照文献

- `references/nifxml/nif.xml:L4327` (NiKeyframeData)
- `references/nifxml/nif.xml:L5274` (NiTransformData)
- `references/nifxml/nif.xml:L3248` (NiTransformInterpolator)
- `references/nifxml/nif.xml:L4214` (NiControllerSequence)
- `references/nifxml/nif.xml:L4154` (NiBSplineData)
- `references/openmw/components/nif/controller.hpp` (OpenMW の実装参考)
