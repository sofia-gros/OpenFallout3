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
