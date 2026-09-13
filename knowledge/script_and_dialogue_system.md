# Fallout 3 / Gamebryo 2.6 スクリプト VM & 会話・UI アーキテクチャ

本ドキュメントは、Fallout 3 (Gamebryo 2.6) におけるスクリプティングエンジン (`SCPT`)、会話ダイアログシステム (`DIAL` / `INFO` / `CTDA`)、ターミナル (`TERM`)、および UI フォントレンダリングの実機バイブル仕様を永続化する知識ベースである。

---

## 1. スクリプティングエンジン (`SCPT`) の構造仕様

### 1.1 一次文献
- `references/openmw/components/esm4/loadscpt.hpp`, `loadscpt.cpp`
- `references/openmw/components/esm4/script.hpp`
- `references/bevyout/src/vsa/scripts/record.rs`
- `references/bevyout/docs/plans/M7_SCRIPTING_ARCHITECTURE_ROADMAP.md`

### 1.2 レコード構成サブレコード
- `EDID`: エディタ識別子文字列 (Null 終端)
- `SCHR`: スクリプトヘッダー (固定 20 バイト)
  ```rust
  #[repr(C, packed)]
  pub struct ScriptHeader {
      pub unused: u32,         // 未使用 (0)
      pub ref_count: u32,      // 参照オブジェクト (SCRO) 数
      pub compiled_size: u32,  // コンパイル済みバイトコード (SCDA) サイズ
      pub variable_count: u32, // ローカル変数 (SLSD) 数
      pub script_type: u16,    // 0: Object, 1: Quest, 0x100: Effect
      pub flags: u16,          // 0x01: Enabled
  }
  ```
- `SCDA`: コンパイル済みバイトコード列 (サイズ: `compiled_size`)
- `SCTX`: スクリプトソースコード文字列 (テキスト)
- `SLSD`: ローカル変数ヘッダー (固定 24 バイト)
  - 変数インデックス (u32), 型 (u32: 0=int/short, 1=float) 等
- `SCVR`: ローカル変数名文字列
- `SCRO` / `SCRV`: スクリプト内で参照される外部 FormID (u32)

### 1.3 スクリプト実行ブロック (Script Blocks)
Fallout 3 スクリプトは複数のイベントハンドラブロックから構成される:
- `Begin GameMode`: 毎フレームまたは一定周期（クエスト更新周期）で実行
- `Begin OnActivate [Target]`: オブジェクトがアクティベートされた時に実行
- `Begin OnEquip [Actor]`, `Begin OnUnequip [Actor]`: 装備時/脱衣時
- `Begin OnDeath [Killer]`: 死亡時
- `Begin OnTalk`: NPC との会話開始時
- `Script Result`: 会話選択時やターミナル選択時に即座に 1 回実行されるスクリプトコード

---

## 2. 会話ダイアログシステム (`DIAL`, `INFO`, `CTDA`)

### 2.1 一次文献
- `references/openmw/components/esm4/loaddial.hpp`, `loaddial.cpp`
- `references/openmw/components/esm4/loadinfo.hpp`, `loadinfo.cpp`

### 2.2 `DIAL` (Dialogue Topic)
- トピック全体のメタデータ。エディタID (`EDID`, 例: `GREETING`, `MegatonMoriartyDad`)、プレイヤーのトピック名 (`FULL`)、所属クエスト (`QSTI` / `QSTR`) を保持。
- 階層として複数の `INFO` レコードを従える。

### 2.3 `INFO` (Dialogue Response / Choice)
- `NAM1`: NPC の応答セリフテキスト (Subtitle)
- `DATA`: 会話フラグ (ビットフラグ: 0x01 = Goodbye [会話終了], 0x02 = Random, 0x04 = Say Once 等)
- `CTDA`: 条件式サブレコード (固定 20 / 24 / 28 バイト)
  ```rust
  pub struct TargetCondition {
      pub operator: u8,      // 0: ==, 1: !=, 2: >, 3: >=, 4: <, 5: <=, 0x80: OR フラグ
      pub unused: [u8; 3],
      pub comparison_value: f32, // 比較対象値 (例: クエストステージ番号、1.0 等)
      pub function_index: u16,   // 評価関数 ID (例: 0x0048 = GetStage, 0x0046 = GetIsID)
      pub param1: u32,           // 第1パラメータ (例: 対象 FormID)
      pub param2: u32,           // 第2パラメータ
      pub run_on: u32,           // 0: Subject (話者), 1: Target (プレイヤー), 2: Reference
      pub reference: u32,        // 対象リファレンス FormID
  }
  ```
- 埋め込み Result Script:
  `SCHR`, `SCDA`, `SCTX`, `SCRO` サブレコードが直接埋め込まれ、このセリフが選ばれた瞬間に実行される。

---

## 3. ターミナルシステム (`TERM`)

### 3.1 一次文献
- `references/openmw/components/esm4/loadterm.hpp`, `loadterm.cpp`

### 3.2 構成
- `FULL`: ターミナル名 (ヘッダー表示)
- `DESC`: ウェルカムテキスト / 説明文
- `DNAM`: ハッキング難易度 (0: Very Easy, 1: Easy, 2: Average, 3: Hard, 4: Very Hard)
- メニュー項目 (`ITXT`, `INAM`, `RNAM`):
  - `ITXT`: 選択肢テキスト
  - `INAM`: 遷移先 FormID (サブターミナルまたは Note)
  - `RNAM`: 選択時実行結果テキスト
  - 選択時 Result Script (`SCDA`/`SCTX`)

---

## 4. フォント & 2D UI レンダリング

### 4.1 一次文献
- `Fallout - Misc.bsa` (`menus\dialog\dialog_menu.xml`, `menus\terminal\terminal_menu.xml`)
- `references/openmw/components/fontloader/fontloader.cpp`

### 4.2 実機仕様
- フォントはビットマップフォントアトラス (`.fnt` メトリクス + `.dds` アトラステクスチャ) または同等のプロポーショナル/等幅フォントラスタライズで描画される。
- UI 頂点形式: スクリーン座標 $X, Y$ (0..Width, 0..Height)、テクスチャ座標 $U, V$、カラー値 $R, G, B, A$。
- カラーパレット:
  - HUD / 会話標準: Pip-Boy グリーン `vec4(0.1, 1.0, 0.5, 1.0)`
  - ターミナル標準: グリーン `vec4(0.2, 1.0, 0.4, 1.0)` または アンバー `vec4(1.0, 0.71, 0.26, 1.0)`
  - 選択ハイライト: 背景反転または高輝度白
