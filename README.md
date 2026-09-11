# OpenFallout3

<p align="center">
  <img src="docs/5.png" alt="OpenFallout3 Character & NPC Assembly" width="850">
</p>

<p align="center">
  <strong>Fallout 3 (Gamebryo 2.6) エンジンを Rust でゼロから完全エミュレート再実装するオープンソースプロジェクト</strong>
</p>

<p align="center">
  <strong>日本語</strong> | <a href="README.en.md">English</a>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-yellow.svg" alt="License: MIT"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.80%2B-orange.svg?logo=rust" alt="Rust 1.80+"></a>
  <a href="https://wgpu.rs/"><img src="https://img.shields.io/badge/Graphics-wgpu%20(Vulkan%2FDX12%2FMetal)-blue.svg?logo=webgpu" alt="wgpu"></a>
  <a href="https://rapier.rs/"><img src="https://img.shields.io/badge/Physics-Rapier3D-red.svg" alt="Rapier3D"></a>
  <a href="AGENTS.md"><img src="https://img.shields.io/badge/Architecture-Gamebryo%202.6-success.svg" alt="Gamebryo 2.6"></a>
  <img src="https://img.shields.io/badge/Tests-44%2F44%20Passing-brightgreen.svg" alt="Tests: 44/44 Passing">
</p>

---

## 概要 (Overview)

**OpenFallout3** は、Bethesda Softworks の金字塔 RPG『Fallout 3』を最新の PC 環境で快適・堅牢・高フレームレートでプレイできるようにすることを目指し、ゲーム基盤エンジン（Gamebryo 2.6）を Rust 言語によってゼロから愚直に再実装するオープンソースプロジェクトです。

Morrowind の再実装プロジェクトである OpenMW などの先例に学びつつ、オリジナルのバイナリ形式（マスターファイル `Fallout3.esm`、アーカイブ `*.bsa`、メッシュ `*.nif`、キーフレームアニメーション `*.kf`）を直接読み込み、レンダリング、骨格スキニング、剛体アタッチメント、レベルドアイテム解決、Havok 物理演算、リアルタイムキャラクターコントローラーを最新の低レベルグラフィックス API（wgpu）上に構築しています。

---

## 主な機能とアーキテクチャ (Key Features)

### 1. アセット・バイナリパーサー (`crates/fo3_*`)
- **fo3_esm**: `Fallout3.esm` の直接ストリーミング走査。`CELL`, `REFR`, `LAND`, `STAT`, `DOOR`, `LIGHT`, `NPC_`, `ARMO`, `HAIR`, `LVLI` レコードの完全デシリアライズ。
- **レベルドアイテム (LVLI) 再帰展開エンジン**: 多重にネストされた配給リスト（武器・防具・弾薬）を幅優先探索 (BFS) で末端の具象アイテムまで完全自動解決。
- **fo3_bsa**: Bethesda Archive 形式 (BSA v104) のハッシュインデックス検索、zlib リアルタイムストリーミング解凍。
- **fo3_vfs**: 複数の BSA アーカイブとルーズファイルを統合し、Windows/POSIX 間の大文字小文字・パスセパレータ差分を透過的に吸収する仮想ファイルシステム。
- **fo3_nif**: Gamebryo 2.6 NIF (`v20.2.0.7`, `User Version 11`, `User Version 2 34`) のバイナリブロック完全解析。`NiNode`, `BSFadeNode`, `NiTriShape`, `NiTriStrips`, `NiAlphaProperty`, `bhkRigidBody`, `bhkPackedNiTriStripsShape`, `NiSkinInstance`, `BSDismemberSkinInstance` を忠実に再現。

<p align="center">
  <img src="docs/1.png" alt="Capital Wasteland World Space" width="750">
</p>

### 2. GPU レンダリングエンジン (`crates/fo3_render`, `crates/fo3_viewer`)
- **wgpu (Vulkan / DirectX 12 / Metal)**: 高効率かつモダンな低レベル描画パイプライン。
- **マテリアル & シェーダー表現**: ディフューズマップ、法線マップ（Tangent / Bitangent 従法線ベクトル計算対応）、グローマップのサンプリング。
- **ランドスケープ（地形）マルチテクスチャリング**: セル単位のベーステクスチャ（BTXT）および標高・傾斜に基づくブレンドレイヤー（ATXT）のアルファ合成スプラット。
- **Gamebryo レンダリングシーケンス準拠**: 不透明パス（深度書き込みあり）と、カメラ距離ソートを伴う半透明（アルファブレンド / アルファテスト）パスの厳格な分離。
- **ライティング・環境再現**: ディレクショナルライト（太陽光）、セルアンビエントライト、環境フォグ、配置点光源（LIGHT レコード）を再現。

<p align="center">
  <img src="docs/2.png" alt="Megaton Interior Cell Walkthrough" width="750">
</p>

### 3. 物理エンジン・キャラクタコントローラー (`crates/fo3_physics`)
- **Havok コリジョン変換**: NIF 内の Havok 物理ブロック（Box, Sphere, Capsule, ConvexHull, TriMesh）および ESM の LAND 標高グリッドを物理エンジンへマッピング。
- **剛体衝突 & ワイヤーフレーム可視化**: 複雑な複合形状（Compound）や複数メッシュの結合処理。デバッグ用の物理ワイヤーフレーム描画をリアルタイムトグル可能。
- **Kinematic Character Controller (KCC)**: 重力加速度、斜面登坂、階段の自動昇降（ステップイン）、接地判定、衝突スライド移動を実装。
- **ハイブリッドカメラ**: 全体周回用オービットカメラと、一人称ウォークスルー歩行モードのワンキー切り替え。

<p align="center">
  <img src="docs/3.png" alt="Havok Collision Wireframes" width="750">
</p>

### 4. キャラクター・スキニング & アニメーション & 装備システム (`crates/fo3_render`, `crates/fo3_esm`)
- **NiSkinData / BSDismemberSkinInstance スキニング**: スケルトンボーン階層への頂点ウェイト変形、バインドポーズ逆行列計算、四肢切断ゴアキャップの初期カリング。
- **Gamebryo KF キーフレーム再生**: `NiTransformInterpolator`（線形/Slerp補間）および B-Spline 補間（Cox-de Boor基底）による骨格姿勢制御。
- **ボーンオーバーライド保全 (`BoneTransformOverride`)**: 移動キーのない関節の並進量を潰さず、手首・指関節の正確な関節長・スパン（約 9.8 units）を数学的に維持。
- **マルチパーツアクター完全合成**:
  - **頭部**: 人種別頭部メッシュ、眼球（左右）、口内（上下歯・舌）の剛体追従（`Bip01 Head`）。
  - **衣装・防具**: 胴体服、帽子、ヘルメット、手袋のスロット自動判定。ヘルメット時の頭髪非表示、帽子着用時の刈り込み髪型（`Hat` シェイプ）の排他カリング。
  - **手部**: 手袋装備時の手袋 NIF 優先ロード、男女別素手メッシュの自動割り当て。
  - **武器マウント**: 右手配下の `"Weapon"` ボーン（位置: X=49, Y=1.8, Z=81）へのライフル・ピストル・警棒等の自動アタッチ。
  - **髪色ティント**: `HCLR` カラー定義および自然なフォールバックパレットによる正確な髪色再現。

<p align="center">
  <img src="docs/4.png" alt="Exterior Exploration and Terrain Physics" width="750">
</p>

---

## 主な実装更新履歴 (Major Updates & Changelog)

| 日付 | マイルストーン・実装内容 | 主な更新コンポーネント |
| :--- | :--- | :--- |
| **2026-09-12** | **装備品解決エンジンの完成 & アクタービジュアル完全修正**<br>・**レベルドアイテム (`LVLI`) 再帰展開**: ネストされた配給リストを BFS 走査し、Vault 101 警備員のヘルメット・服・武器や Lucas Simms の中国軍アサルトライフル等の装備欠損を完全解決<br>・**Havok コリジョン誤認識防止**: 武器 NIF の `ColGroupInfo` 誤認を排除し、右手の `"Weapon"` ボーンへ銃・近接武器を正確にマウント<br>・**剛体 Uniform 初期同期**: 帽子・ヘルメット・髪型・武器の初期姿勢バッファ同期<br>・**指先潰れ解消 (`BoneTransformOverride`)**: 移動キー非保持ボーンのバインドポーズ関節長を100%維持<br>・**髪色ティント補正**: `HCLR` 未定義アクターへの自然色フォールバック | `fo3_esm`<br>`fo3_render`<br>`fo3_viewer` |
| **2026-09-11** | **Phase 6-C: セル内アクター配置 & 自動全身合成 (In-Cell NPC Assembly)**<br>・室内・屋外セル内の `ACHR` / `NPC_` レコードを自動検出し、男女別スケルトン・素体・頭部・目・口内・手先を一括アセンブリ<br>・アイドルアニメーション (`ttnpchappysubtlelistena.kf`) の自動同期再生<br>・四肢切断ゴアキャップの初期状態カリング | `fo3_render`<br>`fo3_viewer` |
| **2026-09-08** | **Phase 6-B: キーフレームアニメーションプレイヤー (`AnimationPlayer`)**<br>・`NiTransformInterpolator`（線形・Slerp補間）および B-Spline（Cox-de Boor基底）姿勢補間<br>・スケルトンボーンの階層的 FK（順運動学）再計算パイプライン構築 | `fo3_render` |
| **2026-09-07** | **Phase 6-A: スキニングエンジン & NIF アニメーションブロック解析**<br>・`NiSkinData`, `NiSkinPartition`, `BSDismemberSkinInstance` の完全解析と CPU スキニング<br>・バインドポーズ逆行列計算と頂点ブレンド | `fo3_nif`<br>`fo3_render` |
| **2026-09-05** | **Phase 5: 物理シミュレーション統合 (Rapier3D / KCC)**<br>・NIF 内 Havok 物理形状および ESM 地形標高グリッドの剛体変換<br>・リアルタイム Kinematic Character Controller、重力・登坂・接地判定・衝突スライド、FPS 歩行モード実装 | `fo3_physics`<br>`fo3_viewer` |
| **2026-09-03** | **Phase 4: ESM 空間配置 & マルチセルストリーミング**<br>・`Fallout3.esm` 直接走査による CELL / REFR / LAND / STAT / DOOR / LIGHT 空間配置再現<br>・メガトンおよびキャピタル・ウェイストランドの広域マルチセルストリーミング表示 | `fo3_esm`<br>`fo3_viewer` |
| **2026-09-01** | **Phase 1〜3: ファイル基盤・NIF ジオメトリ解析・wgpu レンダリング**<br>・BSA v104 解凍、仮想ファイルシステム (VFS)<br>・NIF ジオメトリ解析、法線マップ、半透明ソート、地形スプラット | `fo3_bsa`<br>`fo3_vfs`<br>`fo3_nif`<br>`fo3_render` |

---

## ワークスペース構成 (Workspace Architecture)

```
OpenFallout3/
├── crates/
│   ├── fo3_gamebryo_core/  # Gamebryo 2.6 基本型、数学、トランスフォーム計算
│   ├── fo3_esm/            # ESM / ESP マスターファイルパーサー (LVLI 再帰展開対応)
│   ├── fo3_bsa/            # BSA アーカイブリーダー (v104, zlib 解凍)
│   ├── fo3_vfs/            # 仮想ファイルシステム (Virtual File System)
│   ├── fo3_nif/            # NIF ジオメトリ & コリジョンブロックパーサー
│   ├── fo3_physics/        # 物理シミュレーション (Rapier3D / KCC キャラクタコントローラー)
│   ├── fo3_render/         # wgpu レンダリングパイプライン、シーングラフ、スキニング、アニメ
│   └── fo3_viewer/         # インタラクティブ 3D セル / ワールド / アクタービューアー
├── docs/                   # スクリーンショット・技術仕様書
├── knowledge/              # バイナリ仕様書・Gamebryo 設計ドキュメント
└── references/             # 一次文献 (nif.xml, NifSkope, OpenMW C++ ソース)
```

---

## 必要環境・ビルド方法 (Prerequisites & Build)

### 動作要件
- **Rust**: 1.80 以上 (最新の stable ツールチェーン推奨)
- **Fallout 3**: Steam 版または GOG 版の正規インストールデータ（`Fallout3.esm` および `Data/*.bsa`）
- **GPU**: Vulkan 1.2、DirectX 12、または Metal に対応したグラフィックス環境

### ビルド
```bash
git clone https://github.com/sofia-gros/OpenFallout3.git
cd OpenFallout3
cargo build --release
```

### 全単体テストの実行 (44/44 件合格)
```bash
cargo test --workspace
```

---

## 実行方法 (Viewer Usage)

`fo3_viewer` を使用して、室内セル、広域ワールドスペース、単体アクター、NIF メッシュを直接探索できます。

### 1. 室内セル (Interior Cell) の読み込み
アクター（NPC）、光源、ドア、静的オブジェクト、コリジョンが一括生成されます。

```bash
# Vault 101 エントランス (警備員のヘルメット・制服・武器・住民を完全描画)
cargo run --release -p fo3_viewer -- cell "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "Vault101a"

# メガトン・モリアティの酒場 (Colin Moriarty, Gob, Nova 等の住民と服装・髪型)
cargo run --release -p fo3_viewer -- cell "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "MegatonMoriartysSaloon"

# スプリングベール小学校
cargo run --release -p fo3_viewer -- cell "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "SpringvaleSchool01"
```

### 2. ワールドスペース (World Space) の読み込み
メガトンやキャピタル・ウェイストランドの広域地形、建物、配置 NPC を探索できます。

```bash
# メガトン (Lucas Simms 保安官の帽子・コート・中国軍アサルトライフル保持)
cargo run --release -p fo3_viewer -- world "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "MegatonWorld"

# キャピタル・ウェイストランド広域 (Wasteland)
cargo run --release -p fo3_viewer -- world "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "Wasteland" -1 2

# ワシントンD.C. 廃墟地区 (DCWorld01)
cargo run --release -p fo3_viewer -- world "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "DCWorld01"
```

### 3. フルアクター全身結合 & アニメーション再生 (Actor)
```bash
# ウェイストランド防具 + アイドルアニメーション（頭部・両目・口内・手先・指が完全連動）
cargo run --release -p fo3_viewer -- actor "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "meshes\armor\wastelandclothing01\outfitm.nif" "meshes\characters\_male\idleanims\ttnpchappysubtlelistena.kf"
```

### 4. NIF メッシュ単体の読み込み
```bash
cargo run --release -p fo3_viewer -- "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "meshes\weapons\1handpistol\10mmpistol.nif"
```

---

## 操作方法 (Controls)

| キー / マウス操作 | 機能 |
| :--- | :--- |
| **Tab / M** | カメラモード切替（オービット周回 ⇔ FPS 歩行モード） |
| **W / A / S / D** | 前進 / 左移動 / 後退 / 右移動（FPS 歩行モード時） |
| **Space** | ジャンプ / 上昇（FPS 歩行モード時） |
| **マウス移動** | 視線回転（Look） |
| **マウス左ドラッグ** | カメラ回転（オービットモード時） |
| **マウス右ドラッグ** | カメラ平行移動（Pan）（オービットモード時） |
| **マウスホイール** | ズームイン / ズームアウト（オービットモード時） |
| **C** | Havok コリジョンワイヤーフレーム表示切替 (Collision ON/OFF) |
| **F** | セル環境フォグ表示切替 (Fog ON/OFF) |
| **L** | ビューアー補助ヘッドライト切替 (Light ON/OFF) |
| **R** | カメラ注視点自動再フォーカス |
| **Esc** | ビューアー終了 |

---

## 開発ロードマップ (Roadmap)

| フェーズ | 目標と概要 | 状態 |
| :--- | :--- | :--- |
| **Phase 1: ファイル基盤** | BSA v104 解凍、仮想ファイルシステム (VFS) | **完了** |
| **Phase 2: 3Dメッシュパース** | Gamebryo 2.6 NIF コアノード、ジオメトリ、マテリアル完全解析 | **完了** |
| **Phase 3: レンダリング** | wgpu、法線マップ、スペキュラ、半透明ソート、地形スプラット | **完了** |
| **Phase 4: ESM セル配置** | CELL、REFR、STAT、LAND、LIGHT レコードによる空間配置再現 | **完了** |
| **Phase 5: コリジョン・物理** | Havok コリジョン変換、Rapier3D 統合、リアルタイム KCC 移動 | **完了** |
| **Phase 6-A/B: スキニング・アニメ** | NiSkinData スキニング、KF 補間、マルチパーツアクター結合 | **完了** |
| **Phase 6-C: セル内アクター配置 & 装備解決** | 室内・屋外セル内での全 NPC 自動組み立て、LVLI 再帰展開、帽子・武器・服スロット解決 | **完了** |
| **Phase 6-D: GPU スキニング** | 頂点シェーダー内でのボーンパレット参照による描画負荷低減 | 予定 |
| **Phase 7: インタラクション** | ドアテレポート (XTEL)、コンテナ、アクティベーター作動 | 予定 |
| **Phase 8: キャラクター & カメラ** | プレイヤーアクター、三人称/一人称モデル統合、ステートマシン | 予定 |
| **Phase 9: スクリプト VM & 会話** | SCPT バイトコード実行仮想マシン、ダイアログ (DIAL/INFO) UI | 予定 |

---

## 注意事項・免責事項 (Disclaimer)

- 本ソフトウェアはオープンソースのゲームエンジン再実装であり、Fallout 3 のゲームアセット（テクスチャ、メッシュ、サウンド、マスターファイル等）は一切含まれていません。動作にはユーザー自身が所有する正規のゲームデータが必要です。
- 本プロジェクトは Bethesda Softworks または ZeniMax Media と提携、承認、支援されたものではありません。
- すべての商標および登録商標は、それぞれの所有者に帰属します。
