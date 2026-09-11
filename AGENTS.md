# OpenFallout3 Agent Guidelines & Mandates

本リポジトリは **Fallout 3 の基盤エンジン (Gamebryo 2.6)** を Rust でゼロから愚直に完全エミュレート移植するプロジェクトです。
過去の「推測による独自設計、エラー多発、トークン浪費、AIの逸脱、プロジェクト頓挫」を二度と繰り返さないため、作業にあたるすべての AI エージェントは以下の**絶対的ルール**を遵守しなければなりません。

---

## 1. 開発の基本鉄則（Mandates）

### Rule 1: オリジナル設計・推測による実装の絶対禁止 (Zero Speculation)

- 「こう動くはず」「現代的なゲームエンジンならこう設計する」といった推測やモダンエンジンのパラダイムを絶対に持ち込んではならない。
- すべてのデータ構造、フィールド名、ビットフラグ、親参照・子参照構造、更新順序は、**Gamebryo 2.6 の設計** および `references/nifxml/nif.xml` に定義された仕様に厳密に準拠すること。
- 未知のブロックや挙動に遭遇した場合は、コードを書く前に必ず調査し、文献・コードを特定すること。

### Rule 2: 一次文献・既存実装の参照と引用の義務付け (Evidence-Based Coding)

- 実装する構造体、列挙型、パース関数には、必ず根拠となるリファレンスを日本語 DOC コメントに明記すること。
  - 例: `/// 参照元: references/nifxml/nif.xml:L1234 (NiTriShapeData)`
  - 例: `/// 参照元: references/nifskope/src/spells/mesh.cpp, Gamebryo 2.6 NiAVObject::Update`
- 不明点がある場合は、以下の順序でローカルリファレンスを参照すること:
  1. `references/nifxml/nif.xml` (NIF のバイト構造バイブル)
  2. `references/nifskope/` (NifSkope C++ レンダリング・パース実装)
  3. `references/openmw/components/nif/` (OpenMW C++ 実装)

### Rule 3: トークン浪費防止と知識の永続化義務 (Knowledge-First Workflow)

- **Web 検索の禁止（ローカル優先）**: 外部 Web 検索ではなく、ローカルの `references/` ディレクトリを検索・ピンポイント参照して調査すること（トークン消費最小化・高速）。
- **知識ベース (`knowledge/`) への記録**:
  新しく調査・判明したブロック仕様、トランスフォーム計算式、シェーダーパラメータ、Gamebryo のクラス構造は、実装前に `knowledge/*.md` へ日本語で詳細にドキュメント化すること。

### Rule 4: DOC コメントの日本語義務

- すべての Rust ソースコードの doc コメント (`///`, `//!`) および内部説明コメントは、**必ず日本語で記述**すること。

---

## 2. 実装作業フロー（Workflow Checklist）

1. **Step 1: タスク・Issue 確認 (Task Check)**
   - Memory MCP から直前の作業ログ、未解決 Issue、ブロック中の課題を確認。
2. **Step 2: 仕様特定 (Spec Lookup)**
   - `references/nifxml/nif.xml` で該当ブロックを検索し、Fallout 3 バージョン (`version == 20.2.0.7`, `user_version == 11`, `user_version2 == 34`) の定義を抽出。
3. **Step 3: 知識の永続化 (Knowledge Persistence)**
   - `knowledge/` 配下にメモを作成または追記（レイアウト、型の意味、フラグ値）。
4. **Step 4: 忠実な構造体・パーサー実装 (Faithful Implementation)**
   - `fo3_nif` または `fo3_gamebryo_*` クレートに実装。参照元を日本語 doc コメントに明記。
5. **Step 5: テスト駆動検証 (TDD Verification)**
   - 実アセットまたはダミーバイナリでパースを検証（未読バイトやアライメントエラーの検出）。
6. **Step 6: 進捗の同期 (Memory Update)**
   - 完了したタスク、新規発生した課題・未対応ブロック、設計上の決定事項を Memory MCP に保存。

---

## 3. タスク管理・Memory MCP 原則

- **コード自体のキャッシュ禁止**: コード検索はローカル検索ツールで行い、コード断片や関数一覧をメモリに重複保持させない。
- **タスク進行ログの集約**:
  - `active_task`: 現在着手している具体的なブロックやクレート名
  - `blocked_issues`: 仕様不明や保留中の項目（どの参照文献が足りないか等）
  - `architectural_decisions`: 複数クレート間の依存関係や採用した設計方針の理由
- **セッション間引き継ぎの自動化**: 作業中断・終了時は、次回の作業開始コマンドや着手予定の関数/ブロック名をメモリに書き残すこと。

### 必須スキル

- lookup-nif-spec
- verify-gamebryo-conformance
