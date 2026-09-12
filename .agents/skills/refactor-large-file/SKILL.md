---
name: refactor-large-file
description: Refactor files exceeding 1,000 lines into cohesive, modular submodules conforming to Gamebryo 2.6 architecture and zero-speculation mandates.
---

# Refactor Large File Skill

本スキルは、**1,000行を超える Rust ソースファイル**を検知した場合に、安全かつ体系的に機能別モジュールへと分割・リファクタリングするための厳格な手順書です。

---

## 1. 発動条件（Trigger Conditions）

- 編集対象のファイルが **1,000行** を超えている場合。
- 新機能の追加によってファイルが 1,000行 を超過することが見込まれる場合。
- 主な対象:
  - `crates/fo3_viewer/src/main.rs` (1,800+ 行)
  - `crates/fo3_testbed/src/main.rs` (2,000+ 行)
  - `crates/fo3_esm/src/reader.rs` (1,100+ 行)
  - `crates/fo3_nif/src/blocks/bhk.rs` (1,000+ 行)

---

## 2. リファクタリング基本方針

1. **公開 API・動作互換性の完全維持**:
   - 既存の構造体名、関数名、公開モジュールパス（`pub use ...`）を壊さない。
   - `cargo test --workspace` が常に 100% 通過することを維持する。
2. **Gamebryo 2.6 / 責務別のモジュール分割**:
   - 抽象的すぎるラッパーを作るのではなく、Gamebryo 2.6 のクラス責務やデータフローに即して分割する。
   - 例: `scene.rs` (3,000行) -> `mod.rs`, `mesh.rs`, `traversal.rs`, `bones.rs`, `actor.rs`, `tests.rs`
3. **日本語 DOC コメントの継承**:
   - 移動元の関数・構造体に付与された日本語 DOC コメント、一次文献引用（`references/...`）は一切削らず完全に保持・移行する。

---

## 3. 分割実行手順（Step-by-Step Workflow）

### Step 1: 責務分析と分割計画の立案
- 対象ファイル内の構造体・関数群を 3〜6 個の凝集度の高い機能グループに分類。
- ディレクトリ構造を決定（例: `foo.rs` -> `foo/mod.rs`, `foo/types.rs`, `foo/parser.rs` など）。

### Step 2: サブモジュールファイルの作成
- 分割先の新ファイルを作成し、関連する型・関数を移動。
- 移動元ファイルと同じクレート内での可視性 (`pub(crate)`) を適切に設定。

### Step 3: ルートモジュール (`mod.rs`) の再エクスポート
- `mod sub_module;` を宣言し、外部公開が必要なシンボルを `pub use sub_module::*;` で再エクスポート。

### Step 4: TDD & 型検査
- `cargo check --workspace` でコンパイルエラーを解消。
- `cargo test --workspace` で全テストが通過することを確認。

---

## 4. 遵守チェックリスト

- [ ] 分割後の各ファイルが 800行以下（最大でも 1,000行未満）に収まっているか。
- [ ] すべての関数・構造体に日本語 DOC コメントと一次文献引用が残されているか。
- [ ] `cargo test --workspace` が 100% 成功しているか。
- [ ] Memory MCP にリファクタリング実施ログを記録したか。
