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
  u8            priority               // BSSTREAM フラグ付き
  string        node_name              // ヘッダ文字列テーブルへの u32 インデックス (FO3)
  string        property_type          // ヘッダ文字列テーブルへの u32 インデックス (FO3)
  string        controller_type        // ヘッダ文字列テーブルへの u32 インデックス (FO3)
  string        controller_id          // ヘッダ文字列テーブルへの u32 インデックス (FO3)
  string        interpolator_id        // ヘッダ文字列テーブルへの u32 インデックス (FO3)
}
// NiControllerSequence 追加フィールド
f32            weight
Ref            text_keys              // NiTextKeyExtraData
u32            cycle_type             (0=LOOP, 1=REVERSE, 2=CLAMP)
f32            frequency
f32            start_time
f32            stop_time
Ptr            manager                (-1 in KF)
string         accum_root_name        // ヘッダ文字列テーブルへの u32 インデックス (FO3)
// FO3 (BSVER=34 > 28) では StringPalette なし
u16            num_anim_note_arrays
[num_anim_note_arrays] Ref           // BSAnimNotes への参照配列
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

## 3. ControlledBlock の文字列解決

FO3 (NIF v20.2.0.7) では各 ControlledBlock の文字列（ノード名等）は **ヘッダ文字列テーブルへの u32 インデックス** として格納される。

- 10.2.0.0 〜 20.1.0.0: NiStringPalette + offset 方式
- **20.1.0.1 以降 (FO3 を含む)**: 文字列直接格納（20.1.0.3 以降は NiFixedString = ヘッダ文字列テーブルの u32 インデックス）

```
ControlledBlock (FO3):
  Ref     interpolator
  Ref     controller
  u8      priority
  u32     node_name           // ヘッダ文字列テーブルのインデックス
  u32     property_type       // ヘッダ文字列テーブルのインデックス
  u32     controller_type     // ヘッダ文字列テーブルのインデックス
  u32     controller_id       // ヘッダ文字列テーブルのインデックス
  u32     interpolator_id     // ヘッダ文字列テーブルのインデックス
```

---

## 4. 実装優先順位

| 優先 | ブロック | 理由 |
|:---:|---|---|
| 1 | `NiTransformInterpolator` | すべての静止ボーンが使用 |
| 2 | `NiTransformData` | 線形・四元数キーフレーム |
| 3 | `NiControllerSequence` | ボーン名→インターポレータのマッピング |
| 4 | `NiBSplineCompTransformInterpolator` | FO3 で広く使用される圧縮補間 |
| 5 | `NiBSplineData` / `NiBSplineBasisData` | 4 に必要 |
| 6 | `NiControllerManager` | 複数シーケンスの管理（将来的に必要） |

---

## 5. アニメーション更新ループ（設計方針・実装済み 2026-09-08）

KF のボーン適用と Forward Kinematics の更新を以下の API で実装した
(`crates/fo3_render/src/animation.rs`, `crates/fo3_render/src/scene.rs`)。

```
1. KF ロード → AnimationClip::from_kf(kf)      // ControlledBlock からボーン名→interpolator のマップ
2. apply_pose(kf, &mut pose, time)             // 各ボーンのローカル NiTransform を pose.overrides に評価
   - NiControllerSequence.controlled_blocks を走査
   - cb.node_name_index をヘッダ文字列テーブルから解決 → ノード名
   - interpolator を評価 (NiTransformInterpolator / NiBSplineCompTransformInterpolator)
   - pose.overrides[node_name] = local_transform
3. recompute_bone_world_map_with_pose(skel_nif, &pose, &mut bone_world_map)
   // FK 再計算: 各 NiNode/BSFadeNode のローカル変換を pose で上書きしルートから再合成
4. resolve_bone_world_transforms(inst, &bone_world_map) → &[Mat4]
5. apply_skinning_cpu_with_bones(geo, inst, nif, Some(&bone_transforms)) // 変形
```

- `SkeletonPose.overrides: HashMap<String, NiTransform>`：ノード名 → バインドポーズからの上書き値
- `apply_pose` は姿勢が変わったノード名の `Vec<String>` を返す（差分更新の判定用）
- ポーズに含まれないボーンはバインドポーズの世界行列のまま維持
- Fo3 (v20.2.0.7) での文字列はヘッダ文字列テーブルの u32 インデックス
  (`ControlledBlock.node_name_index` → `NifFile::get_string`)

### ControlledBlock 文字列解決 (FO3)
- 10.2.0.0 〜 20.1.0.0: NiStringPalette + offset 方式
- **20.1.0.1 以降 (FO3 を含む)**: 文字列直接格納（ヘッダ文字列テーブルの u32 インデックス）

```
ControlledBlock (FO3):
  Ref     interpolator
  Ref     controller
  u8      priority
  u32     node_name           // ヘッダ文字列テーブルのインデックス
  u32     property_type
  u32     controller_type
  u32     controller_id
  u32     interpolator_id
```

### 注意 (B-Spline)
- `NiBSplineCompTransformInterpolator` は FO3 で広く使用される。スプライン補間は
  `sample_bspline_transform_interpolator` (`animation.rs`) で実装済み（Cox-de Boor 基底評価、
  セクション 5.5 参照）。チャンネルハンドルが無効 (`0xFFFF`) の場合は基礎 `NiQuatTransform` に
  フォールバックする。

---

## 5.5 B-Spline 圧縮補間 (NiBSplineCompTransformInterpolator) 仕様

参照実装: `references/nifskope/src/gl/glcontroller.cpp` (`bsplineinterpolate` L676, `compute_point` L661, `blend` L625, `compute_intervals` L648)

### バイナリ構造 (既存パーサー済み `fo3_nif::blocks::animation`)
- `NiBSplineData`: `float_control_points: Vec<f32>`, `compact_control_points: Vec<i16>` (符号付き short)
- `NiBSplineBasisData`: `num_control_points: u32`
- `NiBSplineCompTransformInterpolator`:
  - `start_time`, `stop_time`
  - `spline_data`: Ref → NiBSplineData (compact control points を使用)
  - `basis_data`: Ref → NiBSplineBasisData
  - `transform: NiQuatTransform` (基礎姿勢)
  - `translation_handle / rotation_handle / scale_handle: u32` (NiBSplineData 内の制御点オフセット, `USHRT_MAX/0xFFFF` = 無効)
  - `*_offset`, `*_half_range: f32`

### 評価アルゴリズム (NifSkope 準拠)
`degree = 3` (固定, 三次 B-Spline)

```
interval = ((time - start) / (stop - start)) * (nCtrl - degree)   // nCtrl = num_control_points
```

各制御点の生データ → 復元:
```
raw[i] = compact_control_points[handle + k*l + i]                // l = 要素数 (translation=3, rotation=4, scale=1)
```

Cox-de Boor 基底で重み付け合成 (`blend(k, t, u, v)`):
- `t = degree + 1 = 4`
- `n = nCtrl - 1`
- `u[j]` ノット列: `compute_intervals` (open uniform) — `j < t` → 0, `t <= j <= n` → `j - t + 1`, `j > n` → `n - t + 2`
- 基底 `blend(k,t,u,v)` は C-D-B 再帰 (L625)

```
output[i] = Σ(k=0..n) blend(k, t, u, interval) * (compact[handle + k*l + i] / SHRT_MAX)   // Compute: mult=blend
output[i] = output[i] * half_range + offset                                               // Adjust
```

端点 (interval >= nCtrl - degree): 最後の制御点セットをそのまま使用

### 重要ポイント
- ハンドル = NiBSplineData.compact_control_points の開始インデックス (short 単位)
- `blend` に渡す compact 値は `c / SHRT_MAX` で [-1,1] に正規化
- `Compute` の `mult` 引数はブレンド重み、`Adjust` の `mult` は half_range
- 各チャンネル (translation / rotation / scale) は独立したハンドル・offset・half_range を持つ

---

## 5.6 簡易アニメーションプレイヤー (`AnimationPlayer` 実装済み 2026-09-08)

`crates/fo3_render/src/animation.rs` に単一ループ再生のプレイヤーを追加した。

### CycleType (Gamebryo `CycleType` enum, nif.xml L1022)
| 値 | 名称 | 動作 |
|---|---|---|
| 0 | `CYCLE_LOOP` | `[start, stop]` を繰り返し (`scaled % duration`) |
| 1 | `CYCLE_REVERSE` | 往復再生。周期 `2*duration` で前進→後進を交互に繰り返す |
| 2 | `CYCLE_CLAMP` | 終端 `stop_time` で停止。終端到達で `finished = true` |

`AnimationClip::evaluate_time(elapsed)` は `elapsed * frequency` をスケーリングし、
CycleType に応じてシーケンス内時間へ正規化する。

### API
```
AnimationPlayer::new(clip)                      // 再生開始状態で構築 (current_time = start_time)
player.update(kf, dt, &mut pose) -> Vec<String>  // dt 秒進行 + apply_pose。変更ボーン名集合を返す
player.seek(elapsed_seconds)                    // 任意時刻へシーク
player.set_playing(bool)                        // 一時停止/再開
player.current_time                             // 現在のシーケンス内時間
player.finished                                 // CLAMP で終端到達したか
```

### 再生フロー (毎フレーム)
```
1. player.update(kf, dt, &mut pose)         // 時間進行 + ボーンローカル変換を pose.overrides へ
2. recompute_bone_world_map_with_pose(skel_nif, &pose, &mut bone_world_map)  // FK 再合成
3. resolve_bone_world_transforms(inst, &bone_world_map) → &[Mat4]
4. apply_skinning_cpu_with_bones(geo, inst, nif, Some(&bone_transforms))     // 変形
```

- 再生停止中 (`playing = false`) は時間を進めず空集合を返す（一時停止相当）。
- `evaluate_time` の REPEAT / REVERSE は Gamebryo 2.6 `NiControllerSequence::ComputeScaledTime`
  （`cyclestarttime` ベースの相対化）相当の簡易実装。将来 `NiControllerManager` で
  複数シーケンスブレンドを扱う際に本格化する。

---

## 6. 参照文献

- `references/nifxml/nif.xml:L4327` (NiKeyframeData)
- `references/nifxml/nif.xml:L5274` (NiTransformData)
- `references/nifxml/nif.xml:L3248` (NiTransformInterpolator)
- `references/nifxml/nif.xml:L4214` (NiControllerSequence)
- `references/nifxml/nif.xml:L1919` (ControlledBlock)
- `references/nifxml/nif.xml:L1543` (TimeControllerFlags)
- `references/nifxml/nif.xml:L399` (KeyType)
- `references/nifxml/nif.xml:L2007` (KeyGroup)
- `references/nifxml/nif.xml:L1998` (Key)
- `references/nifxml/nif.xml:L2014` (QuatKey)
- `references/nifxml/nif.xml:L4141` (NiBSplineCompTransformInterpolator)
- `references/nifxml/nif.xml:L4154` (NiBSplineData)
- `references/nifxml/nif.xml:L4103` (NiBSplineBasisData)
- `references/nifxml/nif.xml:L6878` (BSAnimNote)
- `references/openmw/components/nif/controller.hpp` (OpenMW の実装参考)
- `references/openmw/components/nif/controller.cpp` (ControlledBlock 読み込み)
- `references/openmw/components/nif/data.cpp` (NiKeyframeData 読み込み)
- `references/nifskope/src/gl/glcontroller.cpp` (補間・時間制御)
- `references/nifskope/src/gl/controllers.cpp` (キーフレーム/トランスフォーム/マネージャ更新)
