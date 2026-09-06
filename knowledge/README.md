# Fallout 3 / Gamebryo 2.6 永続化知識ベース (Knowledge Base)

本ディレクトリは、AI が調査した仕様・バイナリレイアウト・クラス構造を永続化するための場所です。
調査結果をここに集約することで、チャットコンテキストの喪失による再調査（トークンの無駄）を防ぎ、実装時の絶対的な仕様書として機能します。

## 知識ベースの運用ルール
1. **調査即記録**: 仕様を調べたら、コードを書く前に必ずここに日本語でまとめること。
2. **一次情報の明記**: 各メモには参照元（`references/nifxml/nif.xml` の行番号や NifSkope のクラス名など）を必ず記載すること。
3. **推測の排除**: 仕様が不明瞭な場合は推測で補完せず、「未確認 / 要実データ検証」と明記すること。

## ドキュメント一覧
- [nif_v20_2_0_7_format.md](./nif_v20_2_0_7_format.md): NIF ファイルフォーマット v20.2.0.7 のヘッダー、ストリーム、ブロック参照構造
- [gamebryo_class_hierarchy.md](./gamebryo_class_hierarchy.md): Gamebryo 2.6 のコアクラス階層・トランスフォーム計算規則
- [environment_config.md](./environment_config.md): Fallout 3 実ゲームアセット環境設定
- [development_roadmap_openmw_order.md](./development_roadmap_openmw_order.md): OpenMW の歴史的実装順序に学ぶ Fallout 3 開発ロードマップ
