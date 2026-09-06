# Gemini Context & Rules for OpenFallout3

- DOC comment must be in Japanese.
- Gamebryo 2.6 の設計・構造・シーングラフ挙動を愚直に再現すること。
- オリジナル設計や推測によるコード生成は厳禁。
- 仕様の確認は外部 Web ではなく、必ずローカルの `references/nifxml/nif.xml`、`references/nifskope/`、`references/openmw/` を `grep_search` して行うこと。
- 調査結果は `knowledge/` ディレクトリに随時 Markdown メモとして保存・更新すること。
- コード実装時は必ず根拠となる参照文献（`nif.xml` の該当ブロック・行番号等）を日本語 doc コメントに明記すること。
