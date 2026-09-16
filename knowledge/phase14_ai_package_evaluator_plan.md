# AI Package Evaluator 実装プラン (audio.rs モック撤廃)

## 課題の背景
現在の `fo3_viewer/src/audio.rs` には、CG00等のクエスト進行においてNPCを喋らせるために、特定の FormID (`CG00DadREF` など) やトピック名 (`CG00DadSpeech`) を直接条件分岐に組み込んだハードコード（モック）が存在しています。
これは「推測による実装の禁止 (Zero Speculation)」および「Gamebryoアーキテクチャの厳格な準拠」というプロジェクトの絶対ルール (Rule 1, Rule 5) に違反しており、汎用的なエンジンとして機能しません。

本来の Fallout 3 (Gamebryo) では、スクリプトが NPC の変数 (例: `doTalk`) を変更すると、AI Package Evaluator がその変数を条件 (CTDA) として評価し、条件を満たした Dialogue パッケージが自動的にトピックを発話キューに積む仕組みとなっています。

## 提案する実装ステップ

### 1. ESM パーサーの拡張 (`fo3_esm/src/records/pack.rs`)
- `PACK` (AIパッケージ) レコードの解析処理において、発話トピックの指定に使用されるサブレコード (FO3では `TNAM` 等と推測) をパースし、`PackRecord` 構造体に `topic_id: Option<FormId>` などを追加して保持させます。
- 実装時に FO3Edit や `nif.xml`、既存リファレンス (`openmw/components/esm4/loadpack.cpp`) を用いてバイト配置を正確に検証します。

### 2. AI Package Evaluator の新設 (`fo3_viewer/src/ai.rs` または `actor.rs`)
- `Actor` (NPC) の更新ループ内で、自身が持つ AI パッケージ群（NPCレコードに定義されたベースパッケージ群、およびスクリプトで追加されたパッケージ群）を定期的に評価するロジックを新設します。
- 各パッケージの `conditions` (CTDA) を `vm.eval_condition()` に渡し、すべての条件が `true` となる最初のパッケージを「アクティブパッケージ」として選択します。

### 3. 発話アクション (Dialogue) の自動キック
- パッケージの評価によってアクティブパッケージが切り替わった際、そのパッケージが発話用トピック (`topic_id`) を持っていれば、自動的に `vm.say_queue.push((actor_id, topic_edid))` を実行します。
- これにより、「スクリプトがフラグを立てる」→「AIが条件を評価してパッケージを切り替える」→「パッケージが発話アクションを実行する」という正しいデータ駆動フローが実現します。

### 4. ハードコード（モック）の完全撤廃 (`fo3_viewer/src/audio.rs`)
- `audio.rs` に存在する `dad_talking`, `mom_talking`, `drli_talking` などの特定のクエスト固有の判定ロジックと `push` 処理を**すべて削除**します。
- 以降、`audio.rs` は純粋なオーディオ再生バックエンドとしてのみ機能し、ゲームロジックには一切関与しないアーキテクチャに戻します。

## テストと検証
- 現在の `full_playthrough_test` が、このモック撤廃と正しいパッケージ評価を経ても正常に Stage 8, Stage 10 と進行するかを確認します。
- CTDA 評価や `TNAM` 抽出に不備があると進行不能が再発するため、細かく単体テストを挟みながら実装を行います。
