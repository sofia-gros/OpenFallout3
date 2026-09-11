# Gemini Context & Rules for OpenFallout3

- **【最優先命令】コードベースの探索・構造把握・コード調査には必ず CCE (`context_search` ツール等) を最優先で使用すること。ファイルを直接閲覧・全読込するのではなく、CCE のインテリジェント検索を活用して該当コードチャンクを取得すること。**
- **質問への回答や方針決定の前には必ず `session_recall` を呼び出し、決定後は `record_decision` / `record_code_area` でクロスセッション記憶へ記録すること。**
- DOC comment must be in Japanese.
- Gamebryo 2.6 の設計・構造・シーングラフ挙動を愚直に再現すること。
- オリジナル設計や推測によるコード生成は厳禁。
- 仕様の確認は外部 Web ではなく、必ずローカルの `references/nifxml/nif.xml`、`references/nifskope/`、`references/openmw/` を `grep_search` して行うこと。
- 調査結果は `knowledge/` ディレクトリに随時 Markdown メモとして保存・更新すること。
- コード実装時は必ず根拠となる参照文献（`nif.xml` の該当ブロック・行番号等）を日本語 doc コメントに明記すること。

## Context Engine (CCE)

This project uses Code Context Engine for intelligent code retrieval and
cross-session memory.

### Searching the codebase

**Use `context_search` instead of reading files directly** when exploring
the codebase, answering questions about code, or understanding how things
work. `context_search` returns the most relevant code chunks with
confidence scores instead of whole files.

When to use `context_search`:
- Answering questions about the codebase ("how does X work?", "where is Y?")
- Exploring structure or architecture
- Finding related code, functions, or patterns

Other tools:
- `expand_chunk` for full source of a compressed result
- `related_context` for what calls/imports a function
- `session_recall` to recall past decisions

### Cross-session memory

Call `session_recall("topic phrase")` before answering non-trivial questions.
Call `record_decision(decision="...", reason="...")` after making choices.
Call `record_code_area(file_path="...", description="...")` after meaningful work.

### Output style

Respond in compressed style. Drop articles (a, an, the) in prose. Use
sentence fragments over full sentences. Use short synonyms (fix not resolve,
check not investigate). Pattern: [thing] [action] [reason]. [next step].
No filler, hedging, pleasantries, trailing summaries, or restating what
the user said. One sentence if one sentence is enough.

When suggesting code changes, show only the changed lines with 3 lines of
context. Never rewrite entire files. Multiple changes in one file: show each
change separately. Never echo back unchanged code the user already has.

Code blocks, file paths, commands, error messages: always written in full.
Security warnings and destructive action confirmations: use full clarity.
