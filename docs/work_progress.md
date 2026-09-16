# 作業進捗: NPC アニメーション完全実装 (Phase N)

## 概要
`docs/plan.md` の NPC アニメーション実装プラン (Phase N-A〜N-F) に沿って進行中。
`.agents/skills/` の制約 (知識ベース先行記録、Gamebryo 2.6 準拠、1000行超リファクタ) を遵守。

---

## Phase O-A: CG00 開幕イベント完全進行 (doTalk ラッチ化) ✅ 完了

**問題**: ニューゲーム開始 (CG00) が Stage 10 の父親の台詞で停止。doTalk フラグが台詞開始直後に即時リセットされ、INFO トピックによる台詞連鎖と setstage 進行が遮断されていた

**修正内容**:
1. **`crates/fo3_viewer/src/audio.rs`** — doTalk ラッチ化とフレームガード:
   - doTalk push 後の即時リセットを削除 (ラッチ変数化。実機 CG00SCRIPT の `set ...doTalk to 1` と INFO ResultScript の `set ...doTalk to 0` のみが制御)
   - `line_ended_this_frame` フラグを追加 (字幕終了フレームは次の台詞トリガーを 1 フレーム休止し、ResultScript の setstage を確定させてから doTalk 再評価)
   - フォールバック検索で `spoken_infos` (既読) を除外 — トピック全消費時に doTalk クリアへ確実に到達
2. **`crates/fo3_script/src/vm.rs`** — `chargen_menu_active: bool` フィールド追加
3. **`crates/fo3_viewer/src/app.rs`** — 毎フレーム `vm.chargen_menu_active = self.chargen_menu.is_active()` で UI 表示状態と同期
   - ダイアログ抑止ゲートを `in_chargen` (実機 stage 0 で `SetInCharGen 1` のため出生シーン全体で 1) から `chargen_menu_active` へ変更

**検証**: `cargo test --workspace` ✅ 全 100% 通過
- `new_game_simulation_test`: CG00 Stage 0→5→6→8→9→10→(父台詞 INFO 0x0001F387)→18→20→22→(性別認知 0x0001F385)→(母台詞 0x0005EDD8)→38→40→(名前入力)→42 まで完全通過

---

## Phase N-A: EvaluatePackage subject 解決修正 ✅ 完了

**問題**: vm.rs の dot 記法で `effective_self_id` が計算されるもハンドラで未使用 (N-6)、`match cmd {` 欠落でコンパイル不能

**修正内容** (`crates/fo3_script/src/vm.rs`):
- `let (effective_self_id, cmd, cmd_parts)` を `let (self_id, cmd, cmd_parts)` に変更 → 全ハンドラが解決済み subject を使用
- HEAD から存在する `match cmd {` 欠落 (コンパイル不能状態) を復元

**検証**: `cargo check -p fo3_script` ✅ / `cargo test -p fo3_script` ✅ (4件)

---

## Phase N-C: set_animation T-ポーズリセット ✅ 完了

**問題**: NPC アニメーション遷移時に旧アニメーション姿勢が残留 (N-3)

**修正内容** (`crates/fo3_render/src/scene/actor.rs`):
- `set_animation()` に旧アニメーションプレイヤー保持 (`blend_from` フィールド) を追加
- `player.seek(0.0)` で新アニメーションを先頭にリセット
- 初回適用時は `SkeletonPose::default()` (T-ポーズ) でリセット

**検証**: `cargo check -p fo3_render` ✅ / `cargo test -p fo3_render` ✅ (35件)

---

## Phase N-D: クロスフェードブレンド遷移 ✅ 完了

**問題**: アニメーション切替時に一瞬 T-ポーズが挿入される (N-3 続編)

**修正内容** (`crates/fo3_render/src/scene/actor.rs`):
- `RenderActorInstance` に `blend_from: Option<(AnimationPlayer, Arc<NifFile>, f32)>` 追加
- `blend_total: f32` (デフォルト 0.3 秒) フィールド追加
- `update()` で旧・新ポーズを alpha ブレンド (`crate::blend_poses` 使用)

**生成箇所修正** (`crates/fo3_render/src/scene/tests.rs`, `actor.rs:694`):
- 新フィールドの初期化コードを追加

**検証**: `cargo check -p fo3_render` ✅ / `cargo test -p fo3_render` ✅ (35件)

---

## Phase N-B: PACK レコード → KF パス解決 🔄 進行中

### Step 1: fo3_esm 拡張 ✅ 完了

**変更ファイル**:

| ファイル | 変更内容 |
|---|---|
| `crates/fo3_esm/src/types.rs` | `REC_IDLE`, `SUB_IDLA`, `SUB_IDLC`, `SUB_IDLT`, `SUB_IDLF`, `SUB_ANAM` 定数追加 |
| `crates/fo3_esm/src/records/idle.rs` | **新規作成** — IDLE レコード (MODL=KF パス, ANAM=parent/sibling, DATA=6/8byte) パーサー |
| `crates/fo3_esm/src/records/pack.rs` | `PackIdleCollection` 構造体追加 (flags, timer_seconds, animation_form_ids) + IDLA/IDLC/IDLT/IDLF サブレコード解析 |
| `crates/fo3_esm/src/records/mod.rs` | `pub mod idle;` + 再エクスポート追加 |
| `crates/fo3_esm/src/lib.rs` | `IdleRecord`, `PackRecord`, `PackIdleCollection` をルート再エクスポートに追加 |
| `crates/fo3_esm/src/reader.rs` | `read_all_idles_map()` メソッド追加 (IDLE レコードの一括走査 → `HashMap<FormId, IdleRecord>`) |
| `crates/fo3_esm/src/master.rs` | `idle_map: HashMap<FormId, IdleRecord>` フィールド追加 + `load_from_reader` での初期化 |

**検証**: `cargo check -p fo3_esm` ✅ / `cargo test -p fo3_esm` ✅ (6件)

### Step 2: action.rs EVP ハンドラ汎用化 ✅ 完了

**修正内容** (`crates/fo3_viewer/src/action.rs`):

1. **ヘルパー関数追加**:
   - `resolve_idle_kf_from_pack(ctx, pkg) -> Vec<String>` — PACK→IDLA→IDLE→MODL KF パス解決
   - `find_pack_for_actor(ctx, base_npc) -> Option<&PackRecord>` — NPC EDID による PACK 探索 (暫定: PKID 未パースのため EDID 名称一致)
   - `load_first_clip(app, kf_paths) -> Option<(path, nif, clip)>` — KF 候補リストから先頭成功を返す

2. **AddScriptPackage ハンドラ** (N-2 修正):
   - PACK EDID 一致 → `resolve_idle_kf_from_pack` で KF 解決を優先
   - `idleanims\{edid}.kf` のフォールバックを維持

3. **EvaluatePackage ハンドラ** (N-1 修正):
   - CG00_* 固有アクター (Dad/Mom/DrLi/Player) は従来のクエストステージ連動を維持
   - 他アクター: `InteractableObject.base_form_id` → `npc_map` → `find_pack_for_actor` → `resolve_idle_kf_from_pack` による汎用解決
   - REFR→base NPC マッピング: `app.interactables` から `InteractableKind::Actor` を検索

**検証**: `cargo check -p fo3_viewer` ✅

---

## 知識ベース更新 ✅ 完了

`knowledge/phase11_ai_package_and_quest_progression.md` に以下を追記:
- セクション4「PACK Idle Animation Collection と KF パス解決チェーン」
- セクション5「アニメーション完了イベント (Phase N-F)」— `OnAnimationEnd` 設計・1回通知ラッチ・Custom イベント照合
- セクション6「NPC Locomotion (Phase N-E)」— 前後フレーム差分によるモーション判定方針

---

## Phase N-F: アニメーション完了コールバック ✅ 完了

**問題**: `AnimationPlayer` にアニメーション終了検知がなく、パッケージの「再生完了後に次ステージへ進む」動作を実現できなかった

**修正内容**:
1. **`crates/fo3_render/src/animation.rs`**:
   - `is_finished() -> bool` メソッド追加 (`finished` フィールドを返す)
   - `end_dispatched: bool` ラッチ追加 (完了通知を 1 回だけ発行するため)。`new()` / `seek()` で false にリセット
   - 参照元: OpenMW `OnAnimationEnded` (`engineevents.hpp:63`)
2. **`crates/fo3_script/src/event.rs`**:
   - `GameEvent::OnAnimationEnd { actor }` バリアント追加
   - ディスパッチ arm 追加 — `ScriptEventType::Custom` を大文字小文字無視で `"onanimationend"` 照合 (GECK 公式イベントでないため)
3. **`crates/fo3_viewer/src/app.rs`**:
   - NPC 更新ループ内で `player.is_finished() && !player.end_dispatched` 時に `GameEvent::OnAnimationEnd` を 1 回だけプッシュ

**検証**: `cargo check --workspace` ✅ / `cargo test --workspace` ✅ (今回変更クレート: 35 + 4 件成功。統合テスト CG00 失敗は事前から存在する既知問題)

---

## Phase N-E: NPC Locomotion 🔄 保留 (設計判断待ち)

**問題**: NPC が MoveTo 等で移動中の歩行 KF への自動切り替えがない

**調査結果**:
- `process_teleport_requests()` (action.rs:395) は**瞬間テレポート**であり NPC 位置の補間移動が存在しないため、前後フレーム差分はテレポートの 1 フレームだけ非ゼロになる
- CG00 の NPC ステージ KF は KF 内に歩行動作を含むため「静止 + ステージアニメ再生」が正しい状態
- plan.md の差分判定では視覚的な歩行として実効しないため、**ユーザー判断により保留**とする (実装する場合は MoveTo 補間の検討が必要)

---

## 残りの既知問題

### 統合テスト失敗 (✅ 2026-09-16 解消: Phase O-A)

**テスト**: `crates/fo3_viewer/tests/new_game_simulation_test.rs::test_new_game_cg00_progression_simulation`

**原因**:
1. `pending_stage_scripts` の遅延キュー未消化 (→ Stage 0→5 連鎖が起動しない) — **修正済み** (テスト側に app.rs 相当の 1件/フレーム消化クロージャ `digest_pending` を追加)
2. 単一フレーム dt=10.5 で stages 6→7→8→9 が同一フレーム内で連鎖発生 → Stage 8 の 2 秒タイマーまで超過

**根因**:
- `vm.set_stage()` は `quest_stages.insert` で Stage 値を即時更新
- Quest GameMode スクリプトが同一フレーム内で `if getstage==N && timer<=0` を連続評価
- **Phase O-A で解消**: doTalk ラッチ化→台詞連鎖→ResultScript setstage の遅延消化が正しく機能し、フレーム毎のタイマー減算チェーン (Stage 5→6→8→9→10→...) が単一フレーム連鎖せず進行

### 既知の猶予事項
- `pending_stage_scripts` の消化はテスト内 `digest_pending` (1件/フレーム) で模擬。app.rs 実機ループの消化処理と等価であることの確認は継続

### HEAD コンパイル不能 (Phase N-A で修正済み)
- `crates/fo3_script/src/vm.rs:299` の `match cmd {` 欠落 → Phase N-A で復元済み

---

## 次のステップ

| Phase | 内容 | 優先度 | 状態 |
|---|---|---|---|
| N-F | アニメーション完了コールバック (animation.rs + event.rs + app.rs) | 中 | ✅ 完了 |
| N-E | NPC Locomotion walk アニメーション | 中 | 保留 (MoveTo 瞬間テレポートのため実効性に課題) |
| CTDA 評価 | PACK CTDA 条件式の厳密な演算子マッピング検証 (0x60 以外の op の観測) | 低 | 要検証 (現状 CG00 は 0x60 のみ観測) |
| 統合テスト修正 | GameMode per-frame シミュレーションにテスト書き換え | 高 | ✅ 完了 (Phase O-A) |

---

## 更新日

- 2026-09-16: **Phase O-A 完了** — doTalk ラッチ化 + line_ended_this_frame ガード + chargen_menu_active ゲート。統合テスト `new_game_simulation_test` が CG00 Stage 0→42 まで完全通過 (cargo test --workspace 100%)
- 2026-09-15: Phase N-A, N-C, N-D 完了、Phase N-B fo3_esm 拡張完了、Phase N-B action.rs 汎用化完了、知識ベース更新完了
- 2026-09-15: Phase N-F 完了 (is_finished + OnAnimationEnd)、Phase N-E 保留 (MoveTo 補間問題のためユーザー判断)
- 2026-09-15: **モック排除完了** — CG00 ステージ連動 KF ハードコード (action.rs) 廃止、PKID→CTDA→IDLE→KF データ駆動解決へ置換 (fo3_esm/action.rs/loader.rs)
