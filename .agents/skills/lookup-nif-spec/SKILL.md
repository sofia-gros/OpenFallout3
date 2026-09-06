---
name: lookup-nif-spec
description: >-
  Look up NIF block specifications directly from references/nifxml/nif.xml and references/nifskope
  for Fallout 3 (v20.2.0.7, User Version 11, User Version 2 34), extract binary layouts,
  and document findings in knowledge/*.md before writing code.
---

# Lookup NIF Specification Skill

このスキルは、Fallout 3 の NIF ブロック仕様をローカルリファレンスから抽出し、
推測のない正確な知識メモ（`knowledge/*.md`）を作成・更新するための標準手順です。

## 対象バージョン定数 (Fallout 3)
- NIF Version: `20.2.0.7` (ヘッダー表記: `0x14020007`)
- User Version: `11`
- User Version 2: `34`

## 実行手順

### Step 1: `references/nifxml/nif.xml` からの定義検索
1. `grep_search` を使用して、ブロック名（例: `<compound name="NiNode"` または `<niobject name="NiNode"`) を検索。
2. 検索パス: `a:/Project/OpenFallout3/references/nifxml/nif.xml`
3. 該当の開始タグから終了タグ（`</niobject>` / `</compound>`）までを `view_file` で確認。

### Step 2: バージョン条件の判定
各フィールドの `ver1`, `ver2`, `userver`, `userver2` 条件式を確認:
- 例: `ver1="20.2.0.7"` や `userver="11"` などの条件に Fallout 3 が合致するかを判定。
- 条件に合致しないフィールドは Fallout 3 では読み飛ばしまたは存在しないため除外。

### Step 3: 親クラスの継承ツリーの確認
`inherit="NiAVObject"` などの継承属性がある場合、親クラスの定義も遡って確認。
- 例: `NiNode` -> `NiAVObject` -> `NiObjectNET` -> `NiObject`

### Step 4: `references/nifskope` / `references/openmw` での実装照合
必要に応じて、NifSkope や OpenMW の C++ 実装で実際にどのように読み込まれ、どう使用されているかを確認:
- NifSkope: `references/nifskope/src/` 配下を検索
- OpenMW: `references/openmw/components/nif/` 配下を検索

### Step 5: `knowledge/` への知識保存
判明したバイナリレイアウト、型、フィールド順を `knowledge/` 配下の該当 Markdown ファイルに日本語で記録。
- ファイル例: `knowledge/nif_blocks_geometry.md` や `knowledge/nif_blocks_nodes.md`
- 参照元ファイル・行番号を必ず明記。
