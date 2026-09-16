# Phase 11: AI パッケージ (PACK) と実機クエスト進行・自律動作アーキテクチャ

## 1. 概要と目的
本ドキュメントは、Fallout 3 (`Fallout3.esm`) における NPC の自律動作・移動・目標追従を司る **AI パッケージ (`PACK` レコード)** のバイナリ構造および、クエストスクリプトと連動した NPC 制御パイプラインを定義する。

---

## 2. PACK (AI Package) レコードのバイナリ構造

### 2.1 ヘッダー
- **レコード四文字コード**: `PACK`
- **FormID**: 32-bit 一意 ID

### 2.2 サブレコード一覧
| サブレコード | サイズ (bytes) | 必須 | 説明 |
|---|---|---|---|
| `EDID` | 可変 (C-String) | ○ | エディタ識別子 (例: `CG00JamesHoldBaby`) |
| `PKDT` | 4, 8, または 12 | ○ | 基本パッケージデータ (フラグ, タイプ) |
| `PLDT` | 12 | △ | 場所データ (タイプ, FormID, 半径) |
| `PTDT` | 12 または 16 | △ | ターゲットデータ (タイプ, FormID, 距離) |
| `PSDT` | 8 | △ | スケジュールデータ (月, 曜日, 日, 時, 継続時間) |
| `CTDA` | 24 | △ | パッケージ適用条件 (複数存在可) |
| `CNAM` | 4 | △ | 戦闘スタイル FormId |
| `QSTI` | 4 | △ | 関連クエスト FormId |

### 2.3 `PKDT` (Package Data) の構造 (FO3 レイアウト)
Fallout 3 における `PKDT` の標準サイズは 8 バイト (または拡張 12 バイト):
- `flags` (`u32` / オフセット 0): パッケージフラグ
- `package_type` (`u8` / オフセット 4): パッケージ動作種別
  - `0`: Find
  - `1`: Follow (対象に追従)
  - `2`: Escort (対象を護衛/先導)
  - `3`: Eat
  - `4`: Sleep
  - `5`: Wander (指定エリア・半径内を徘徊)
  - `6`: Travel (指定地点/マーカーへ移動)
  - `7`: Accompany
  - `8`: UseItemAt
  - `9`: Ambush
  - `10`: FleeNotCombat
  - `11`: CastMagic
  - `12`: Sandbox
  - `13`: UseWeapon
- `unused` (`u8` / オフセット 5)
- `behavior_flags` (`u16` / オフセット 6): 振る舞いフラグ

### 2.4 `PLDT` (Location Data) の構造 (12 バイト)
- `location_type` (`i32` / オフセット 0):
  - `0`: Near reference (指定 Ref 付近)
  - `1`: In cell (指定 Cell 内)
  - `2`: Current location (現在地)
  - `3`: Editor location (エディタ初期位置)
  - `4`: Object ID (FormID で示されるオブジェクト付近)
  - `5`: Object Type
  - `0xFF`: 場所指定なし
- `location_value` (`u32` / オフセット 4): `location_type != 5` の場合は `FormId`
- `radius` (`i32` / オフセット 8): 徘徊/移動の許容半径 (World units)

### 2.5 `PTDT` (Target Data) の構造 (12 または 16 バイト)
- `target_type` (`i32` / オフセット 0):
  - `0`: Specific reference (`FormId`)
  - `1`: Object ID
  - `2`: Object Type
  - `0xFF`: ターゲットなし
- `target_value` (`u32` / オフセット 4): `target_type != 2` の場合は `FormId` (アクターやマーカーの FormId)
- `distance` (`i32` / オフセット 8): 到達目標距離
- (FO3 拡張) `unknown_f32` (`f32` / オフセット 12): 16バイト時のみ存在

---

## 3. クエストスクリプトと AI パッケージの連動フロー

1. **スクリプトによる制御**:
   - `evp` (`EvaluatePackage`): NPC の AI パッケージスタックを再評価させ、現在の条件に合致する最優先パッケージを適用。
   - `MoveTo <RefID>`: NPC を特定の参照マーカーまたはプレイヤー位置へ即座にワープさせる。
   - `Say <TopicID>`: 指定した会話トピックを発話させ、台詞とアニメーションを同期。
2. **パッケージの自動遷移**:
   - クエストステージが `SetStage` で進むと、関連 NPC に設定されたパッケージの `CTDA` 条件（例: `GetStage CG00 == 10`）が成立し、自動的に次の動作（例: `Travel` で赤ちゃんのベッドへ移動）へ移行する。

### 3.1 会話連鎖の制御 (doTalk ラッチ + フレームガード) — 2026-09-16 確定

CG00 開幕イベントの会話連鎖 (父→母→父→...) は、NPC 参照に `doTalk` 変数が立つことで駆動される。実機 ESM 検証の結果、以下のアーキテクチャが確定した。

**制御変数の所有者 (1フレーム中の唯一の書き込み主体)**:
- **CG00SCRIPT (SCPT 0x0003A17C)**: `set CG00DadREF.doTalk to 1` (Stage 10: 父の最初の台詞)
- **INFO ResultScript**: `set CG00DadREF.doTalk to 0` / `set CG00MomREF.doTalk to 1` (台詞終了時のハンドオフ)

つまり `doTalk` は **ラッチ変数** であり、SoundEngine 内で台詞開始直後に自動リセットしてはならない。

**1フレーム休止ルール (`line_ended_this_frame`)**:
- 台詞が終了し ResultScript が実行されたフレームは、`line_ended_this_frame = true` とし、**次の台詞トリガーを 1 フレーム休止**する。
- これにより ResultScript の `setstage CG00 N` が `pending_stage_scripts` へ正常に積まれ、次フレームの GameMode ループ開始時に消化される確実性が得られる (vm.rs `set_stage` 遅延実行仕様と整合)。

**キャラメイク中のダイアログ抑止 (`chargen_menu_active`)**:
- 実機 CG00 Stage 0 の `SetInCharGen 1` は出生シーン全体で 1 を保持するため、`vm.in_chargen` をダイアログ抑止ゲートに使うと Stage 10 の父の台詞が遮断される。
- よって抑止判定は **キャラメイク UI の物理表示状態** (`ChargenMenu::is_active()`) を毎フレーム `vm.chargen_menu_active` へ同期して用いる。

**フォールバック時の既読除外**:
- トピック選択フォールバック検索では `spoken_infos` (既読 INFO) を対象から除外し、トピック全消費時に doTalk フラグをクリアする経路へ確実に到達させる。

**検証**: `crates/fo3_viewer/tests/new_game_simulation_test.rs` — CG00 Stage 0→5→6→8→9→10→18→20→22→38→40→42 を通過。

---

## 4. PACK の Idle Animation Collection と KF パス解決チェーン

### 4.1 PACK → KF パスまでの実機フロー

```
EvaluatePackage (evp)
  └─ 対象アクターのアクティブな PACK レコードを評価
       ├─ PACK
       │   ├─ EDID (例: CG00DadSection00)
       │   └─ IDLA (Idle Animation FormID リスト)
       │        └─ IDLE レコード
       │             └─ MODL (Anim/ 相対パス文字列、例: Characters\_Male\IdleAnims\cg00dadsection00.kf)
       │                  └─ VFS/BSA から KF を読み込み → NiControllerManager::ActivateSequence
```

### 4.2 PACK の Idle Collection サブレコード (参考: `references/bevyout/src/vsa/openmw_esm4/actor_support.rs:L503-511, L659-745`)

| サブレコード | サイズ | 内容 |
|---|---|---|
| `IDLF` | 1 byte | Idle コレクションフラグ |
| `IDLC` | 1 or 4 byte | 宣言アニメーション数 (`data[0]`) |
| `IDLT` | 4 byte (f32) | タイマー秒数 |
| `IDLA` | 4×N byte | Idle アニメーション FormID リスト (IDLE レコードを参照) |

- `IDLC` のカウント不一致は記録致命的でない (diag のみ)。有効にデコードされた ID は破棄されない。
- FormID はプラグインの `resolver` で調整済み値を保持。

### 4.3 IDLE レコード (参考: `references/bevyout/src/vsa/openmw_esm4/idle.rs:L5-22, L150-193`)

| サブレコード | サイズ | 内容 |
|---|---|---|
| `EDID` | C-String | エディタ識別名 |
| `MODL` | C-String | アニメーションファイルパス (KF) — 例 `Characters\_Male\IdleAnims\Swatting.KF` |
| `ANAM` | 8 byte | `parent_form_id` (offset 0), `previous_sibling_form_id` (offset 4) |
| `DATA` | 6 or 8 byte | `group_section_raw`, `loop_min`, `loop_max`, `replay_delay_seconds` (i16), `flags` |
| `CTDA` | 可変 | 適用条件 (opaque bytes、評価器は未実装) |

- 実プレイヤー KF は `MODL` のパスを小文字化し、`meshes\` プレフィックスを付与して VFS に照会する。

### 4.4 NPC → パッケージの紐づけ (現状の制約)

- `fo3_esm::EsmMasterContext::pack_map` (`master.rs:54`) に全 PACK が格納済み。`NpcRecord.edid` (`npc.rs:34`) からアクター名を取得可能。
- `NpcRecord` はまだ AI パッケージリスト (`PKID`) をパースしていないため、「NPC → デフォルトパッケージ」の直接解決は未実装。
- 実装では PACK の `EDID` (`cg00dadsection00` 形式) とアクター `EDID` (`CG00DadREF`) の名称から関連パッケージを選択し、最終段に stage ベースのフォールバック (cg00dadsection0N.kf / cg00momsection0N.kf 等) を残す。

---

## 5. アニメーション完了イベント (Phase N-F)

### 5.1 概要
Fallout 3 の一部 AI パッケージは「特定 KF の再生完了後に次ステージへ進む」動作を持つ。
`AnimationPlayer` (`fo3_render::animation`) は `CycleType::Clamp` のクリップが終端
(`stop time`) に達したことを `finished: bool` フィールドで検知するが、これをゲーム側イベント
としてスクリプトに通知する機構が未実装だった。

### 5.2 根拠リファレンス
- **OpenMW の `OnAnimationEnded`**: `references/openmw/apps/openmw/mwlua/engineevents.hpp:63`
  (`ref_animation_end` を `OnAnimationEnded` として Lua へ暴露)
- **Gamebryo 2.6 の `NiControllerSequence`**: `CycleType::Clamp` は終端で再生を停止し、
  最終姿勢を保持する (`references/nifskope/src/nifmodel.cpp` の key/channel 評価ロジック相当)。

### 5.3 設計決定
- `AnimationPlayer` に `is_finished() -> bool` メソッドを追加 (`finished` フィールドを返す)。
- 完了通知を**1 回だけ**発行するため `end_dispatched: bool` ラッチを追加。
  `AnimationPlayer::new()` / `seek()` で false に戻し、`app.rs` の NPC 更新ループで
  `is_finished() && !end_dispatched` の時にのみ `GameEvent::OnAnimationEnd { actor }` を発行する。
- `GameEvent::OnAnimationEnd` は GECK の公式 Begin イベントとして実在しないため、
  スクリプト側はカスタムイベント名 (`Begin OnAnimationEnd`) として `ScriptEventType::Custom`
  にフォールバックされ、ディスパッチャは大文字小文字を無視して照合する。

---

## 6. NPC Locomotion (Phase N-E)

### 6.1 概要
NPC が `MoveTo` 等で移動している最中は歩行 KF (`mtforward.kf` 相当)、静止時は idle KF を
自動選択する。既存プレイヤーの `LocomotionStateMachine` (`fo3_viewer::player`) が持つ
「移動中 / 静止」判定と同様の仕組みを NPC へ拡張する。

### 6.2 設計決定
- NPC 位置の更新 (MoveTo / テレポート) は `process_teleport_requests()` で行われるため、
  `RenderActorInstance.world_transform.translation` の**前後フレーム差分**でモーション判定を行う。
- 移動差分が閾値以上の間は walk KF へ、差分がゼロ (MoveTo 完了) に戻ったら idle KF へ切替。

---

## 7. AI パッケージ CTDA 条件評価によるデータ駆動アイドル解決

### 7.1 概要
CG00_* 固有のステージ連動 KF ハードコード (action.rs で REFR FormID に一致する match 分岐)
を廃止し、実機 ESM の `NPC.PKID -> PACK.CTDA -> IDLE.MODL` チェーンを直接解決する
汎用エンジンに置換した。

### 7.2 実機 ESM 検証結果 (Fallout3.esm)
| NPC (EDID) | FormID (NPC_) | PKID 数 | PACK 順 (優先度順) |
|---|---|---|---|
| `CG00Dad` ("Dad") | 0x000290A6 | 7 | Section5 → Section4 → Section3 → Section2 → Section1 → Section0 → Start |
| `CG00Mom` ("Catherine") | 0x0005EDDF | 7 | Section5 → Section4 → Section3 → Section2 → Section1 → Section0 → Default |
| `CG00DoctorLi` ("Doctor Li") | 0x000290A3 | 7 | Section5 → Section4 → Section3 → Section2 → Section1 → Section0 → Start |

**各 Section PACK の CTDA** (28 バイト / TargetCondition::parse で 20 バイトまで抽出):
```
operator: 0x60  →  op_type = (0x60 >> 5) & 7 = 3  →  >= (Greater Than or Equal To)
fn_index: 0x3A  =  58  →  FUN_GetStage
param1:   0x0001F388  →  CG00 クエスト FormID
param2:   0x00000000
```

| Section | compValue (f32) | 閾値 (i32) | IDLE FormID | IDLE MODL (KF パス) |
|---|---|---|---|---|
| Section0 | 8.0 | >= 8 | 0x00084439 (Dad) / 0x0008443A (DrLi) / 0x0008443B (Mom) | `CG00*Section00.kf` |
| Section1 | 10.0 | >= 10 | 0x00068AB0 (Dad) / 0x00068AB1 (DrLi) / 0x00069EF4 (Mom) | `CG00*Section01.kf` |
| Section2 | 20.0 | >= 20 | 0x00069EEC (Dad) / 0x00069EF0 (DrLi) / 0x00069EF5 (Mom) | `CG00*Section02.kf` |
| Section3 | 40.0 | >= 40 | 0x00069EED (Dad) / 0x00069EF1 (DrLi) / 0x00069EF6 (Mom) | `CG00*Section03.kf` |
| Section4 | 60.0 | >= 60 | 0x00069EEE (Dad) / 0x00069EF2 (DrLi) / 0x00069EF7 (Mom) | `CG00*Section04.kf` |
| Section5 | 80.0 | >= 80 | 0x00069EEF (Dad) / 0x00069EF3 (DrLi) / 0x00069EF8 (Mom) | `CG00*Section05.kf` |

**Start / Default**: CTDA なし (条件なしパッケージ)。idle_collection も None。
→ ステージが条件を満たさない場合 (例: stage 0-7) は Start が選択され、アイドルアニメーションなし (静止姿勢)。

### 7.3 旧ハードコードとの差異
| ステージ | 旧モック (REFR FormID ベース, 間違った閾値) | 実機データ駆動 |
|---|---|---|
| 0 | `cg00dadsection00.kf` | Start (静止姿勢) |
| 5 | `cg00dadsection00.kf` | Start (静止姿勢) |
| 15 | `cg00dadsection01.kf` | `CG00DadSection01.kf` (>= 10) |
| 35 | `cg00dadsection03.kf` | `CG00DadSection02.kf` (>= 20) |
| 45 | `cg00dadsection04.kf` | `CG00DadSection03.kf` (>= 40) |
| 65 | なし | `CG00DadSection04.kf` (>= 60) |

### 7.4 リファレンス
- `references/openmw/components/esm4/loadnpc.cpp:71-73` (PKID パース)
- `references/openmw/components/esm4/loadpack.hpp:77-88` (PACK CTDA 構造体)
- `references/openmw/components/esm4/script.hpp:100` (FUN_GetStage = 58)
- `references/openmw/components/esm4/loadinfo.cpp:81-105` (CTDA サイズ分岐: 20/24/28 バイト)
- `crates/fo3_script/src/conditions.rs:34-103` (evaluate_single_condition, operator ビットレイアウト)
- `crates/fo3_esm/src/records/dial.rs:69-121` (TargetCondition::parse, CTDA フォーマット)
- `crates/fo3_viewer/src/action.rs:493-537` (resolve_pack_for_actor, resolve_idle_kf_from_pack)
- `crates/fo3_viewer/src/loader.rs:698-805` (アクター配置時のデフォルトアイドル PKID 解決)
