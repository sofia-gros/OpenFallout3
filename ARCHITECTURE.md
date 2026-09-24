# OpenFallout3 アーキテクチャ & モジュール責務マップ (Codebase Index)

本ドキュメントは、各クレートおよびファイルの責務を定義したインデックスです。
コードの調査・機能追加・修正時は、まず本ファイルを参照して対象ファイルをピンポイント特定してください。

---

## 1. クレート一覧と責務 (Crates Overview)

| クレート名 | 責務概要 | 主な依存 |
| :--- | :--- | :--- |
| **`fo3_gamebryo_core`** | Gamebryo 2.6 基礎数学・幾何データ構造 (`NiTransform`, `NiBound`, `Color3/4`) | `glam` |
| **`fo3_bsa`** | Bethesda Archive (`.bsa`) の解凍・読み込み・ハッシュ計算 | `flate2`, `byteorder` |
| **`fo3_esm`** | `Fallout3.esm` バイナリパース、全レコード型定義、クエリAPI | `fo3_gamebryo_core` |
| **`fo3_nif`** | Gamebryo 2.6 NIF (`v20.2.0.7`) 3Dメッシュ・骨格バイナリパーサー | `fo3_gamebryo_core` |
| **`fo3_vfs`** | 仮想ファイルシステム (Loose files と BSA アーカイブのマウント優先度解決) | `fo3_bsa` |
| **`fo3_navigation`** | ナビメッシュ (`NAVM`) レコード解析および A* 経路探索アルゴリズム | `fo3_esm`, `glam` |
| **`fo3_physics`** | Havok コリジョン変換 (`bhk*`)、Rapier 3D 物理、キャラクタコントローラ | `fo3_nif`, `rapier3d` |
| **`fo3_render`** | wgpu レンダリングパイプライン (PBR, スキニング, アニメーションKF, UI XML) | `wgpu`, `fo3_nif` |
| **`fo3_script`** | GECK スクリプト構文解析 (AST), VM, イベントディスパッチ, 条件式 (`CTDA`) 評価 | `fo3_esm` |
| **`fo3_viewer`** | 実機 Fallout 3 ゲーム実行エンジン (メインループ, 統合ロード, UI, 音声, カメラ) | 全クレート統合 |
| **`fo3_testbed`** | メッシュ単体検証・スタンドアロン表示用テストバイナリ | `fo3_render`, `fo3_vfs` |

---

## 2. 主要クレート別モジュール詳細マップ

### `crates/fo3_esm` (マスターデータ)
- `src/reader.rs`: ESM レコード・グループの低レベルバイナリ読み込み
- `src/master.rs`: `EsmMasterContext`（全レコードのインメモリハッシュマップ管理）
- `src/query.rs`: FormID / EditorID によるレコード高速検索クエリ
- `src/records/*.rs`: 個別レコード型定義 (`npc.rs`, `cell.rs`, `quest.rs`, `dial.rs`, `scpt.rs` 等)

### `crates/fo3_render` (描画エンジン)
- `src/pipeline.rs`: wgpu レンダリングパイプライン、シェーダー管理、Uniformバッファ
- `src/mesh.rs`: GPU 頂点バッファ・インデックスバッファの構築
- `src/gpu_skin.rs`: ハードウェアボーンスキニング (`NiSkinInstance`) 計算
- `src/animation.rs`: KF アニメーションコントローラー、補間、ポーズ適用
- `src/sequence.rs`: Gamebryo 2.6 `NiControllerSequence` 複数アニメーション合成・ブレンド
- `src/facegen.rs`: EGM / EGT フェイスモーフ・テクスチャ生成
- `src/scene/actor.rs`: アクターインスタンス (`RenderActorInstance`) 階層管理
- `src/scene/bones.rs`: ボーン階層のワールド変換行列計算
- `src/ui/`: フォント描画、テキストバッチ
- `src/ui_xml/`: 実機 HUD XML (`.xml`) / テクスチャアトラス (`.tai`) パース & ランタイム描画

### `crates/fo3_script` (スクリプト & クエスト)
- `src/parser/`: GECK スクリプトソースコードの字句解析 (`lexer.rs`)、AST (`ast.rs`)、構文解析 (`parser.rs`)
- `src/ast_vm.rs`: AST ステートメント (`Set`, `If`, `Call`) の直接評価
- `src/vm.rs`: `ScriptVm`（変数ストア、FormID解決、組み込み関数呼び出し）
- `src/event.rs`: `EventDispatcher`（スクリプトイベント `GameMode`, `OnActivate` 等のキュー処理）
- `src/conditions.rs`: 実機条件式 (`CTDA`) 評価 (`GetStage`, `GetScriptVariable`, `GetIsID` 等)
- `src/quest.rs`: `QuestManager`（クエストステージ遷移、目的ログ管理）
- `src/functions/*.rs`: 組み込み命令の実装 (`actor.rs`, `dialogue.rs`, `movement.rs`, `state.rs` 等)

### `crates/fo3_viewer` (ゲーム実行ランタイム)
- `src/main.rs`: コマンドライン引数解析・実行モード分岐
- `src/app/mod.rs`: `ViewerState` 定義およびサブモジュール再エクスポート
- `src/app/init.rs`: GPU デバイス・サーフェス・シーン初期化 (`ViewerState::new`)
- `src/app/state.rs`: セルスクリプト登録、ウィンドウリサイズ、RTT/UI バインド
- `src/app/update.rs`: 毎フレームの物理・スクリプト・アニメーション・カメラ更新ループ
- `src/app/render.rs`: 3D シーン、HUD、全画面フェード、2D UI 描画パス
- `src/app/handler.rs`: winit `ApplicationHandler` イベントループディスパッチ
- `src/loader/mod.rs`: `load_scene` エントリポイントおよび `LoadedSceneResult` 定義
- `src/loader/finder.rs`: ESM からのターゲットセル・近傍セル・ワールドスペースのセル探索
- `src/loader/cell.rs`: セル内配置メッシュ (REFR)・光源収集および GPU シーン初期化
- `src/loader/actor.rs`: セル内配置アクター (ACHR/NPC_) の解決、装備・髪型・FaceGen・スケルトン生成
- `src/loader/interactable.rs`: ドア・コンテナ・アイテム等の `InteractableObject` 判定
- `src/loader/physics.rs`: セル内地形 (LAND) コライダーおよび REFR 物理剛体・可動部バインディング構築
- `src/streamer.rs`: ワールドセル (WRLD) のグリッド差分ストリーミング
- `src/action/mod.rs`: アクションディスパッチエントリポイントおよび再エクスポート
- `src/action/interact.rs`: フォーカス中オブジェクトに対するインタラクトアクション (`E` キー)
- `src/action/door.rs`: ワールド・セル遷移ドア (XTEL) によるシーン切り替え
- `src/action/teleport.rs`: スクリプト要求によるテレポート移動 (MovetoMarker 等)
- `src/action/package.rs`: アクターの AI パッケージ評価・アイドリングアニメーション解決
- `src/action/playgroup.rs`: スクリプト要求による PlayGroup アニメーション再生ディスパッチ
- `src/ai.rs`: アクターの AI パッケージ評価・移動ターゲット計算
- `src/player/mod.rs`: プレイヤーアクター制御およびロコモーション状態マシンの再エクスポート
- `src/player/locomotion.rs`: ロコモーション状態マシン (`LocomotionStateMachine`, `LocomotionState`)
- `src/player/actor.rs`: プレイヤーアクター実体 (`PlayerActor`, `build_player_actor`)、1人称/3人称メッシュ管理
- `src/camera.rs`: 1人称/3人称カメラ、壁めり込み防止コリジョン
- `src/audio.rs`: `SoundEngine` (BGM, 効果音, 実機ダイアログ `DIAL/INFO` 音声・字幕同期)
- `src/bink_player.rs`: Bink 動画 (`.bik`) の ffmpeg パイプ再生 (30fps 固定タイマー制御)
- `src/hud/mod.rs`: `HudRenderer` 定義およびサブモジュール再エクスポート
- `src/hud/types.rs`: HUD 定数 (`HUD_PHOSPHOR_COLOR`, 呼吸パルス) および頂点・Uniform 定義
- `src/hud/pipeline.rs`: HUD / Bink Video 専用パイプライン・バッファ構築
- `src/hud/crosshair.rs`: 実機クロスヘア・呼吸パルスグローおよび基本矩形スプライト描画
- `src/hud/video.rs`: Bink 動画フレームのテクスチャ全画面描画
- `src/hud/overlay.rs`: 会話ダイアログ・レトロ CRT ターミナル画面オーバーレイ描画
- `src/ui/dialog/mod.rs`: NPC ダイアログ UI 再エクスポート
- `src/ui/dialog/state.rs`: ダイアログ選択肢・フェーズ状態管理
- `src/ui/dialog/render.rs`: ダイアログウィンドウ・トピック描画
- `src/chargen_menu.rs`: キャラメイク・名前入力・性別選択ダイアログ UI
- `src/window_input.rs`: winit キーボード・マウス入力ディスパッチ

---

## 3. 開発・修正時の探索ルール (Quick Lookup Rule)

1. **機能の所在確認**:
   - まず本マップを見て、対象機能がどのクレートのどのファイルにあるかを特定する。
2. **盲目的なファイル探索の禁止**:
   - ファイルを開く前に、必ず `grep_search` (`MatchPerLine: true`) で行番号をピンポイント特定してからアクセスする。
3. **400行上限の維持**:
   - 各ファイルの行数は **400行以下（理想 200〜300行）** に維持する。400行を超えた場合は本マップの責務に従ってサブモジュール分割を実施する。
