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

- **Web 検索の禁止（ローカル優先）**: 外部 Web 検索ではなく、ローカルの `references/` ディレクトリを `grep_search` や必要最小限のファイル参照で調査すること（トークン消費ゼロ・高速）。
- **知識ベース (`knowledge/`) への記録**:
  新しく調査・判明したブロック仕様、トランスフォーム計算式、シェーダーパラメータ、Gamebryo のクラス関係は、コードを書く前に必ず `knowledge/*.md` に日本語で詳細に記録すること。
- **メモリ（Memories / Knowledge Graph）の活用**:
  セッションを跨ぐ仕様や構造体の関係性（どのモジュールがどの責務を持つか）は、ファイル全体を再読み込みせず、メモリ（Memories）を検索して最小限のスコープで把握すること。

### Rule 4: DOC コメントの日本語義務

- すべての Rust ソースコードの doc コメント (`///`, `//!`) および内部説明コメントは、ユーザーの指定通り**必ず日本語で記述**すること。

---

## 2. 実装作業フロー（Workflow Checklist）

新しい NIF ブロックや Gamebryo 機能を実装する際は、必ず以下のステップを順に踏むこと:

1. **Step 1: 仕様特定 (Spec Lookup)**
   - `references/nifxml/nif.xml` で該当ブロック名を検索し、Fallout 3 バージョン (`version == 20.2.0.7`, `user_version == 11`, `user_version2 == 34`) のフィールド定義を抽出。
2. **Step 2: 知識の保存/読み込み (Knowledge Persistence)**
   - `knowledge/` 配下にメモを作成または追記（ブロックのバイナリレイアウト、型の意味、フラグ値）。
3. **Step 3: 忠実な構造体・パーサー実装 (Faithful Implementation)**
   - `fo3_nif` または `fo3_gamebryo_*` クレートに実装。参照元を日本語 doc コメントに明記。
4. **Step 4: テスト駆動検証 (TDD Verification)**
   - 実アセットまたはダミーバイナリを用いてパースを検証し、余計なバイト残り（バッファ未読）やアライメントエラーがないことを確認。

---

## 3. 参照・メモリ原則（Codebase & Memory Access）

- **ピンポイント参照の徹底**: コードやドキュメントを確認する際は、ファイル全体を漫然とコンテキストに入れず、関数名やキーワードで検索し、必要な最小限のブロック・構造のみを対象とすること。
- **構造と関係性の記憶**: 行数などの変動しやすい情報ではなく、どの構造体・関数がどの責務を持ち、どう結合しているかという「関係性（リレーション）」を中心にメモリ（Memories）に蓄積・活用すること。

### スキルを適切に使用すること

- lookup-nif-spec
- verify-gamebryo-conformance
