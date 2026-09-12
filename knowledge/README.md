# Fallout 3 / Gamebryo 2.6 永続化知識ベース (Knowledge Base)

本ディレクトリは、AI が調査した仕様・バイナリレイアウト・クラス構造を永続化するための場所です。
調査結果をここに集約することで、チャットコンテキストの喪失による再調査（トークンの無駄）を防ぎ、実装時の絶対的な仕様書として機能します。

## 知識ベースの運用ルール

1. **調査即記録**: 仕様を調べたら、コードを書く前に必ずここに日本語でまとめること。
2. **一次情報の明記**: 各メモには参照元（`references/nifxml/nif.xml` の行番号や NifSkope のクラス名など）を必ず記載すること。
3. **推測の排除**: 仕様が不明瞭な場合は推測で補完せず、「未確認 / 要実データ検証」と明記すること。
4. **検索・参照の原則**: コードやドキュメントを読む際は、ファイル全体をコンテキストに入れず、関数名やキーワードで検索し、必要な最小限のブロックのみを対象とすること。構造と関係性の記憶：行数などの変動しやすい情報ではなく、どの構造体・関数がどの責務を持ち、どう結合しているかという「関係性」をメモリに蓄積すること。

## ドキュメント一覧

### 1. アーキテクチャ & ロードマップ
- [development_roadmap_openmw_order.md](./development_roadmap_openmw_order.md): OpenMW の歴史的実装順序に学ぶ Fallout 3 開発ロードマップ
- [gamebryo_class_hierarchy.md](./gamebryo_class_hierarchy.md): Gamebryo 2.6 のコアクラス階層・トランスフォーム計算規則
- [gamebryo_resource_management_and_caching.md](./gamebryo_resource_management_and_caching.md): Gamebryo 2.6 準拠 多層リソースキャッシュ・メモリ管理仕様
- [environment_config.md](./environment_config.md): Fallout 3 実ゲームアセット環境設定

### 2. ファイルフォーマット (BSA / NIF / KF / ESM)
- [bsa_v104_format.md](./bsa_v104_format.md): BSA アーカイブフォーマット (v104 / Fallout 3) 詳細バイナリ仕様
- [nif_v20_2_0_7_format.md](./nif_v20_2_0_7_format.md): NIF ファイルフォーマット v20.2.0.7 のヘッダー、ストリーム、ブロック参照構造
- [nif_blocks_geometry.md](./nif_blocks_geometry.md): Fallout 3 NIF ジオメトリ & マテリアルブロック詳細バイナリ仕様
- [nif_blocks_extra_and_materials.md](./nif_blocks_extra_and_materials.md): エクストラデータ、シェーダープロパティバイナリ仕様
- [animation_kf_format.md](./animation_kf_format.md): KF (Keyframe) アニメーションファイル構造と補間仕様
- [esm_file_format.md](./esm_file_format.md): Fallout 3 ESM / ESP ファイルフォーマット詳細バイナリ仕様
- [esm_cell_refr.md](./esm_cell_refr.md): Fallout 3 CELL & REFR レコード・シーン配置バイナリ仕様
- [worldspace_cells.md](./worldspace_cells.md): WRLD / CELL / グリッド座標系とマルチセルロード仕様

### 3. レンダリング・グラフィックス
- [rendering_pipeline_and_viewer.md](./rendering_pipeline_and_viewer.md): wgpu レンダリングパイプラインとメッシュビューアーの設計仕様
- [rendering_shaders_and_materials.md](./rendering_shaders_and_materials.md): WGSL シェーダー設計と PBR/Phong マテリアル再現
- [normal_mapping_and_specular.md](./normal_mapping_and_specular.md): 法線マップ・スペキュラマップ・接空間ライティング
- [alpha_property.md](./alpha_property.md): NiAlphaProperty フラグ仕様とブレンド・テスト設定
- [alpha_blending_and_sorting.md](./alpha_blending_and_sorting.md): 半透明メッシュの深度ソートアルゴリズム
- [lighting_cell_records.md](./lighting_cell_records.md): セル環境光・ディレクショナルライト・点光源パイプライン
- [landscape.md](./landscape.md): LAND レコード・16ビットハイトマップ頂点生成仕様
- [landscape_multitexturing.md](./landscape_multitexturing.md): 地形マルチテクスチャ・スプラットマップ合成仕様

### 4. スキニング・アニメーション & アクター
- [actor_and_skin_mesh.md](./actor_and_skin_mesh.md): マルチパーツ NPC 組み立て、NiSkinData 骨格バインド、FaceGen モーフ仕様
- [animation_sequence_blending.md](./animation_sequence_blending.md): NiControllerSequence、アニメーションブレンド、優先度制御

### 5. 物理 & コリジョン
- [physics_engine_evaluation_and_architecture.md](./physics_engine_evaluation_and_architecture.md): 物理エンジン選定・Havok ⇔ Rapier3D アーキテクチャ
- [bhk_collision.md](./bhk_collision.md): bhkCollisionObject / bhkRigidBody / 形状ブロックバイナリ仕様
- [havok_collision_blocks.md](./havok_collision_blocks.md): 各種 Havok プリミティブ形状・マテリアル・レイヤー仕様

## Knowledge Graph Memory Server

Please follow these steps for each interaction:

1. User Identification:

- Assume you are interacting with the default user.
- If you are unable to identify the default user, actively attempt to do so.

2. Memory Retrieval:

- Always begin chats with the phrase "I'm remembering..." and retrieve all relevant information from the knowledge graph.
- Always refer to the knowledge graph as "Memories."

3. Memory

- During conversations with users, pay attention to new information that falls into the following categories:

a) Basic PC environment (language, programming language, Windows 11 OS, 32GB memory, etc.)
b) Coding details (what was implemented, why it was done this way, coding style, etc.)
c) Documentation (which files were looked at, why they were looked at, etc.)
d) Goals (goals, targets, aspirations, etc.)
e) Certainty (whether it followed AGENTS.md, Gemini.md, etc.)

4. Memory Update:

- If new information is obtained during the conversation, update your memory as follows:

a) Create entities for recurring files, functions, documents, and important events.
b) Use relationships to connect them to existing entities.
c) Save the facts about them as observations.
