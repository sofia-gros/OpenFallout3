# NPC アニメーション完全実装プラン

## 現状の構造 (調査結果)

```
fo3_render/src/scene/actor.rs
  └─ RenderActorInstance
       ├─ anim_player: Option<AnimationPlayer>    // 単体 KF 再生
       ├─ kf_nif: Option<Arc<NifFile>>
       ├─ sequence_manager: Option<SequenceManager> // マルチトラック合成 (実装済み)
       ├─ anim_pose: SkeletonPose
       ├─ anim_skin_meshes: Vec<AnimatedSkinMesh>  // CPU スキニング
       └─ anim_rigid_meshes: Vec<AnimatedRigidMesh> // 剛体ボーン追従

fo3_viewer/src/action.rs: process_package_requests()
  ├─ AddScriptPackage → KF パスをハードコード解決 → set_animation()
  └─ EvaluatePackage/EVP → FormID ハードコード (Dad/Mom/DrLi/Player のみ) → KF ステージ分岐
```

### 現在の問題点

| # | 問題 | 影響 |
|---|------|------|
| **N-1** | `EvaluatePackage` の KF パスがハードコード | Dad/Mom/DrLi 以外の NPC (Jonas/Amata 等) にアニメーション適用不可 |
| **N-2** | `AddScriptPackage` の KF 解決が `idleanims/` フォルダのみ | PACK レコードの `AI_Package` から KF パスを参照していない |
| **N-3** | NPC アニメーション遷移 (ブレンド) が未実装 | アニメーション切替が瞬間ポップ (クロスフェードなし) |
| **N-4** | 歩行・移動アニメーション (Locomotion) が NPC に未適用 | NPC が AI パッケージで移動してもアニメーション変化しない |
| **N-5** | ESM PACK レコードから KF/アニメーションパスを解決していない | 実機準拠の AI 行動ループが不可能 |
| **N-6** | `EvaluatePackage` で `self_id=None` を送っている | どの NPC に適用するか判断不能 |
| **N-7** | アニメーション再生後の完了コールバック/ResultScript が未実装 | アニメーション終了でステージ進行できない |

---

## 実装方針 (Gamebryo 2.6 準拠)

```
実機フロー:
  Script: CG00DadREF.evp
    → EvaluatePackage (subject=CG00DadREF FormID)
      → AI Package 評価 (ESM PACK レコード)
        → IdleAnimation/PlayGroup から KF パス解決
          → NiControllerManager::ActivateSequence
            → アクターの sequence_manager に KF を積む
              → 毎フレーム update() でスキニング更新
```

---

## Phase N-A: EvaluatePackage の subject 解決修正 (P0)

### 問題
`vm.rs` の `evaluatepackage` コマンド処理が `self_id` (None) のみを `evaluate_package_requests` に積んでいる。
実機では `CG00DadREF.evp` のように **呼び出し元オブジェクトの FormID** がサブジェクト。

### 修正
**[MODIFY] [`vm.rs`](file:///a:/Project/OpenFallout3/crates/fo3_script/src/vm.rs)**

`execute_statement` 内の `evaluatepackage`/`evp` ハンドラ:
```rust
// 修正前: self_id のみ積む
self.evaluate_package_requests.push(self_id);

// 修正後: subject が dot 記法の場合 (CG00DadREF.evp) は prefix を解決
// 例: "cg00dadref.evp" → resolve_form_id("CG00DadREF") → FormId(0x000290A7)
```

dot 記法のオブジェクト参照 (`CG00DadREF.evaluatepackage`) は `execute_statement` の先頭で prefix 分割して `subject_id` を解決する。これは `moveto` 等でも同様。

---

## Phase N-B: ESM PACK レコードから KF パスを解決 (P1)

### 問題
現在の `process_package_requests` の EvaluatePackage ハンドラは FormID ハードコードで KF パスを返している。
実機では ESM の `PACK` (AI Package) レコードを参照して行動を決定する。

### 調査が必要な項目
- `fo3_esm::EsmMasterContext` に `pack_map: HashMap<FormId, PackRecord>` があるか確認
- `PackRecord` に `Idle` アニメーションパスが含まれるか

**[MODIFY] [`action.rs`](file:///a:/Project/OpenFallout3/crates/fo3_viewer/src/action.rs#L522)**

EVP 処理を `match target_fid.0` ハードコードから汎用ロジックに変更:

```rust
// 修正後 (擬似コード):
for subject_opt in evp_requests {
    let actor_fid = match subject_opt {
        Some(fid) => fid,
        None => continue, // subject 不明はスキップ
    };

    // 1. ESM の PACK レコードから AI パッケージを解決
    let kf_path = resolve_kf_from_ai_package(actor_fid, &app.master_context, stage);

    // 2. KF ロード & set_animation
    if let Some(path) = kf_path {
        load_and_apply_kf(app, actor_fid, &path);
    }
}
```

**[NEW] `resolve_kf_from_ai_package()` 関数**

```rust
fn resolve_kf_from_ai_package(
    actor_fid: FormId,
    master: &EsmMasterContext,
    quest_stage: u32,
) -> Option<String> {
    // 1. NPC の現在パッケージリストを参照
    // 2. パッケージのアイドルアニメーションパスを返す
    // 3. なければデフォルト (idle.kf) を返す
    todo!()
}
```

---

## Phase N-C: アニメーション遅延適用 (1フレーム後) (P1)

### 問題
`evp` → 即 `set_animation()` → 次フレームの `actor.update()` でポーズ反映 (これは問題なし)

しかし `actor.update()` は毎フレーム全アクターを更新しており、  
`set_animation` のタイミングとボーン FK 計算のタイミングにずれがある。

### 修正 (小)
アニメーションが切り替わった最初のフレームは `time=0.0` から確実に開始するよう `AnimationPlayer::reset()` を呼ぶ。

**[MODIFY] [`actor.rs:set_animation`](file:///a:/Project/OpenFallout3/crates/fo3_render/src/scene/actor.rs#L227)**

```rust
pub fn set_animation(&mut self, kf: Arc<NifFile>, clip: Arc<AnimationClip>) {
    self.kf_nif = Some(kf);
    let mut player = AnimationPlayer::new((*clip).clone());
    player.time = 0.0;  // 確実に先頭から再生
    self.anim_player = Some(player);
    self.sequence_manager = None;
    self.anim_pose = SkeletonPose::default(); // T-ポーズにリセット (ポップ防止)
}
```

---

## Phase N-D: アニメーションブレンド遷移 (クロスフェード) (P2)

### 問題
KF 切り替え時に前の姿勢から新しい姿勢へ瞬間ポップ。

実機 Gamebryo 2.6 では `NiControllerManager::ActivateSequence(blend_time=0.3)` で  
**クロスフェード** を行う。

### 実装
**[MODIFY] [`actor.rs`](file:///a:/Project/OpenFallout3/crates/fo3_render/src/scene/actor.rs)**

`RenderActorInstance` に遷移状態を追加:
```rust
// 追加フィールド:
pub blend_from: Option<(AnimationPlayer, Arc<NifFile>, f32)>, // (旧プレイヤー, 旧KF, 残りブレンド時間)
pub blend_total: f32,  // ブレンド総時間 (秒)
```

`update()` でのブレンド合成:
```rust
if let Some((ref mut old_player, ref old_kf, ref mut blend_remaining)) = self.blend_from {
    let alpha = 1.0 - (*blend_remaining / self.blend_total).clamp(0.0, 1.0);
    // 旧ポーズと新ポーズを alpha でブレンド
    let old_pose = old_player.evaluate(old_kf, ...);
    let new_pose = new_player.evaluate(new_kf, ...);
    self.anim_pose = blend_poses(&old_pose, 1.0 - alpha, &new_pose, alpha);
    *blend_remaining -= dt;
    if *blend_remaining <= 0.0 { self.blend_from = None; }
}
```

---

## Phase N-E: NPC 歩行アニメーション (Locomotion) (P2)

### 問題
`app.rs:560` の NPC update ループで全 NPC の KF が毎フレーム更新されているが、  
NPC が MoveTo 等で移動している場合の**歩行/走行 KF** への自動切り替えがない。

### 実装
**[MODIFY] [`app.rs`](file:///a:/Project\OpenFallout3\crates\fo3_viewer\src\app.rs)**

NPC アニメーション更新ループに Locomotion 判定を追加:
```rust
for actor in &mut self.scene.actors {
    // NPC が移動中 (前フレームと position が異なる) なら walk KF に切り替え
    // NPC が静止 (MoveTo 完了) なら idle KF に戻す
    actor.update(dt, &self.device, &self.queue, &mut self.scene.meshes);
}
```

ただし NPC 位置の更新 (MoveTo 処理) は `process_teleport_requests()` で行われるため、  
`actor.world_transform.translation` の前後フレーム差分でモーション判定する。

---

## Phase N-F: アニメーション完了コールバック (P2)

### 問題
Fallout 3 の一部パッケージは「アニメーション再生完了後に次のステージへ進む」動作をするが、  
`AnimationPlayer` にアニメーション終了検知がない。

### 実装
**[MODIFY] [`animation.rs`](file:///a:/Project/OpenFallout3/crates/fo3_render/src/animation.rs)**

`AnimationPlayer` に `is_finished()` メソッドを追加:
```rust
pub fn is_finished(&self) -> bool {
    !self.looping && self.time >= self.clip.stop_time
}
```

**[MODIFY] [`app.rs`](file:///a:/Project/OpenFallout3/crates/fo3_viewer/src/app.rs)**

NPC update ループ後に完了イベントをディスパッチ:
```rust
for actor in &mut self.scene.actors {
    actor.update(dt, ...);
    if let Some(ref player) = actor.anim_player {
        if player.is_finished() {
            self.dispatcher.push_event(GameEvent::OnAnimationEnd {
                actor: FormId(actor.form_id),
            });
        }
    }
}
```

---

## 実装優先度マップ

| Phase | 内容 | ファイル | 優先度 |
|-------|------|---------|--------|
| **N-A** | EVP subject 解決 (dot 記法) | `vm.rs` | **P0** |
| **N-B** | PACK レコードから KF パス解決 | `action.rs`, `fo3_esm` 確認 | **P1** |
| **N-C** | `set_animation` での T-ポーズリセット | `actor.rs:227` | **P1** |
| **N-D** | クロスフェードブレンド遷移 | `actor.rs` | **P2** |
| **N-E** | NPC Locomotion 歩行アニメーション | `app.rs` | **P2** |
| **N-F** | アニメーション完了コールバック | `animation.rs`, `app.rs` | **P2** |

---

## Open Questions

> [!IMPORTANT]
> 実装前に確認が必要:

1. **`fo3_esm::EsmMasterContext` に `pack_map` はあるか?**
   → `grep_search "pack_map" in crates/fo3_esm/src` で確認が必要
   → なければ Phase N-B は ESM パーサー拡張から開始する

2. **`PACK` レコードの `Idle` フィールドに KF パスが含まれるか?**
   → `references/nifxml/nif.xml` ではなく `Fallout3.esm` の PACK 構造を確認
   → CG00 の `CG00DadSection00` パッケージのフィールドを確認

3. **dot 記法 prefix 解決は `vm.rs` の全コマンドで共通化すべきか?**
   → `moveto`, `evaluatepackage`, `addscriptpackage` は全て同様の処理が必要
   → `execute_statement` の先頭で一括処理することを推奨

> [!WARNING]
> Phase N-B は ESM の `PACK` レコードパーサーが `EsmMasterContext` に統合されていない場合、
> `fo3_esm` クレートへの大規模追加が必要になる可能性がある。
> その場合は Phase N-A / N-C を先行させ、N-B は次フェーズとする。
