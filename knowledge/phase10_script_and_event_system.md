# Phase 10: イベント駆動スクリプトシステム & クエスト進行 仕様書

**作成日**: 2026-09-13  
**方針**: Zero Speculation / Evidence-Based / Gamebryo 2.6 GECK スクリプト完全準拠

---

## 1. Fallout 3 スクリプトアーキテクチャ概要

Fallout 3 は Skyrim や Fallout 4 の Papyrus スクリプトエンジンとは異なり、**Gamebryo 2.6 / Oblivion 系統の GECK スクリプト言語** を使用する。

### 1.1 主要レコードとサブレコード (`SCPT`)
参照元: `references/openmw/components/esm4/script.hpp:346-378`, `references/openmw/components/esm4/loadscpt.hpp`

- `SCPT`: スクリプトレコード
  - `EDID`: スクリプトエディタID (例: `MQ01MoriartyScript`, `DefaultDoorScript`)
  - `SCHR`: スクリプトヘッダー (固定 20 バイト)
    - `unused`: 4 bytes
    - `ref_count`: 4 bytes (参照オブジェクト `SCRO` 数)
    - `compiled_size`: 4 bytes (コンパイル済みバイトコード `SCDA` サイズ)
    - `variable_count`: 4 bytes (ローカル変数 `SLSD` 数)
    - `type`: 2 bytes (`0`: Object, `1`: Quest, `0x100`: Effect)
    - `flag`: 2 bytes (`0x01`: Enabled)
  - `SCDA`: コンパイル済み生バイトコード (トークン列)
  - `SCTX`: ソーステキスト (人間可読な GECK スクリプト本文)
  - `SLSD` / `SCVR`: ローカル変数定義 (インデックス、型、変数名)
  - `SCRO`: スクリプト内で参照される外部オブジェクトの FormID リスト

---

## 2. イベントブロック構文 (`Begin ... End`)

Fallout 3 のオブジェクトスクリプトおよびクエストスクリプトは、特定のエンジンイベントによってトリガーされる複数のブロック（`Begin [EventName] [TargetRef]` 〜 `End`）から構成される。

### 2.1 主要イベント一覧とトリガー条件
参照元: GECK Wiki / `references/openmw/apps/openmw/mwworld/refdata.cpp`

| イベント名 | 引数 | トリガー契機 | 優先度 |
|---|---|---|---|
| `OnActivate` | `[ActorRef]` (省略時は誰でも) | プレイヤーまたは NPC がオブジェクトを操作 (Eキー) | **最高** |
| `OnAdd` | `[ContainerRef]` | アイテムがインベントリに追加された時 | 高 |
| `OnEquip` | `[ActorRef]` | アイテムが装備された時 | 中 |
| `OnUnequip` | `[ActorRef]` | アイテムの装備が解除された時 | 中 |
| `OnTrigger` / `OnTriggerEnter` | `[ActorRef]` | トリガーボリューム境界に侵入した時 | 中 |
| `OnTriggerLeave` | `[ActorRef]` | トリガーボリューム境界から退出した時 | 低 |
| `GameMode` | なし | ゲームプレイ中、毎フレーム（または設定間隔）実行 | 高 |
| `MenuMode` | `[MenuType]` | ダイアログやターミナル等の UI 開放中実行 | 中 |
| `OnDeath` | `[KillerRef]` | アクターが死亡した時 | 中 |

### 2.2 アクティベート抑制メカニズム (`Flag_SuppressActivate`)
- スクリプト付きオブジェクト（ドア、コンテナ、端末等）に `OnActivate` ブロックが存在する場合、エンジンはデフォルトのインタラクト（ドア開放やコンテナ表示）を自動的に実行せず、`OnActivate` イベントをスクリプトにディスパッチする。
- スクリプト内部で明示的に `Activate` (または `Activate player`) が呼ばれると、抑制フラグが解除され、本来のオブジェクト動作が実行される。
- これにより、「鍵が必要」「パスワード端末経由」「特定クエストステージでのみ開く」といった制御が実現される。

---

## 3. スクリプト実行エンジン (`fo3_script`) アーキテクチャ

```
crates/fo3_script/
├── src/
│   ├── lib.rs            # クレート公開 API
│   ├── opcodes.rs        # GECK 関数・命令コード定義 (Opcode, NativeFunc)
│   ├── conditions.rs     # 24バイト CTDA 条件式評価エンジン
│   ├── parser.rs         # SCTX ソーステキスト & イベントブロック構文解析器
│   ├── event.rs          # イベントキュー・ディスパッチャー (EventDispatcher)
│   ├── vm.rs             # スクリプト実行コンテキスト (ScriptVm)
│   └── natives.rs        # ゲーム世界操作ネイティブ関数群 (SetStage, AddItem, etc.)
```

### 3.1 ネイティブ関数の責務分担
- **クエスト進行**:
  - `SetStage [QuestFormID] [StageNumber]`
  - `GetStage [QuestFormID]`
  - `GetStageDone [QuestFormID] [StageNumber]`
- **ワールドオブジェクト操作**:
  - `[Ref].Activate [ActionRef]`
  - `[Ref].Enable` / `[Ref].Disable`
  - `[Ref].Lock [Level]` / `[Ref].Unlock`
  - `[Ref].GetOpenState` / `[Ref].SetOpenState [0|1]`
- **所持品操作**:
  - `[ActorRef].AddItem [ItemFormID] [Count]`
  - `[ActorRef].RemoveItem [ItemFormID] [Count]`
  - `[ActorRef].GetItemCount [ItemFormID]`
- **UI & 演出**:
  - `ShowMessage [MessageFormID]`
  - `PlaySound [SoundFormID]`
- **フロー制御**:
  - `Return`
  - `If [Condition]` / `ElseIf` / `Else` / `EndIf`

---

## 4. Phase 10 サブフェーズ分割計画

### Phase 10-A: オブジェクトスクリプトのアタッチ & イベントブロック解析
1. `REFR` / `CONT` / `DOOR` / `ACTI` / `TERM` / `NPC_` レコードの `SCRI` サブレコード (FormID) を走査・解決。
2. スクリプト (`SCPT`) の `SCTX` からイベントブロック (`Begin OnActivate` ... `End`) を抽出し、実行可能ブロックとしてキャッシュ。
3. ローカル変数 (`SLSD` / `SCVR`) をオブジェクトインスタンスごとに独立確保する状態管理構造 (`ScriptInstanceContext`) を新設。

### Phase 10-B: イベントディスパッチャー & インタラクト連携
1. `EventDispatcher` を新設し、エンジン側で発生したイベント（`OnActivate`, `OnAdd` 等）をキューイング・即時ディスパッチ。
2. 既存の Phase 7 インタラクト（Eキー操作）と統合:
   - スクリプトに `OnActivate` があればイベントを優先起動。
   - スクリプト内の `Activate` 呼び出しにより本来のインタラクト（ドア開放等）を遅延発火。

### Phase 10-C: スクリプト VM 命令セット拡張 & 制御構造
1. `If` / `ElseIf` / `Else` / `EndIf` のネスト可能な制御フロー評価器を実装。
2. 変数代入 (`Set [Var] To [Expr]`) および算術演算 (`+`, `-`, `*`, `/`) のサポート。
3. 参照修飾プレフィックス (`Ref.Command`) の実行パイプライン構築。

### Phase 10-D: 会話 Result Script & クエストステージ連携
1. 会話選択肢 (`INFO`) 決定時の Result Script を実行し、即座にクエストステージを更新。
2. `QUST` (Quest) レコードのパースを強化し、クエスト目標、ステージ進行状態、ログエントリーを管理する `QuestManager` を構築。
3. 会話終了フラグ（`Goodbye`）処理により、会話完了後のスクリプト連動（NPC移動やアイテム付与）を完遂。

#### Phase 10-D 実証結果 (2026-09-16, CG00 通過) — 台詞連鎖とステージ連動の確定事項
- **doTalk はラッチ変数**: SoundEngine が台詞開始直後に自動リセットしない。書き込み主体は CG00SCRIPT の `set ...doTalk to 1` と INFO ResultScript の `set ...doTalk to 0/1` のみ (`knowledge/phase11_ai_package_and_quest_progression.md` 3.1 節参照)。
- **Result Script の遅延実行**: `vm.set_stage` は Result Script を `pending_stage_scripts` へ積み、app.rs 実機ループが 1 件/フレームで消化する。`new_game_simulation_test` はテスト内で同処理を `digest_pending` クロージャ (1件/フレーム) として模擬。
- **会話連鎖の検証済み INFO**:
  - 0x0001F387「Let's see... Are you a boy or a girl?」(Stage 10→父 → ResultScript `setstage CG00 18`)
  - 0x0001F385 (Stage 22→父の性別認知 → `set dad.doTalk 0; set mom.doTalk 1`)
  - 0x0005EDD8 (母の台詞 → `set dad.doTalk 1`)
- **フレーム休止**: 台詞終了フレーム (`line_ended_this_frame`) は次台词を 1 フレーム休止し、ResultScript の setstage 確定を保証。

### QUST レコードのバイナリ構造仕様 (実機 ESM 検証済)
- `EDID`: クエスト EditorID (例: `"MQ01"`, `"MS11"`)
- `FULL`: クエスト表示名称 (例: `"Following in His Footsteps"`)
- `DATA`: 8バイト。flags (u8), priority (u8), padding (u16), questDelay (f32)
- `SCRI`: FormID (u32)。クエストに常駐アタッチされる Quest Script の FormID
- ステージブロック群:
  - `INDX`: 2バイト (u16)。ステージ番号（例: 10, 20, 30, 100 等）
  - `QSDT`: 1バイト (u8)。ステージフラグ (0x01: Complete/Done)
  - `SCHR` / `SCDA` / `SCTX`: ステージ突入時に実行される Result Script (埋め込みスクリプト)
- 目標 (Objective) ブロック群:
  - `QOBJ`: 4バイト (u32)。目標インデックス (例: 10, 20)
  - `NNAM`: 文字列。目標テキスト (例: `"Speak to Colin Moriarty"`)
  - `QSTA`: 8バイト。目標ターゲット/ステータスデータ

### Phase 10-E: 実機クエスト「Following in His Footsteps」垂直スライス検証
1. メガトン「Moriarty's Saloon」における実機クエストシナリオを実証:
   - コリン・モリアティとの会話 (`MQ01MoriartyTopic`) によるステージ 10 → 20 進行。
   - 100キャップ支払いによる情報開示またはターミナル閲覧による情報入手。
   - モリアティのオフィスキャビネット解錠による情報入手。
2. 世界がプレイヤーの行動に応じて動的に変化・進行することを確認。

