# OpenFallout3

OpenFallout3 は、Bethesda Softworks の RPG『Fallout 3』を最新環境で快適にプレイできるようにすることを目指し、ゲームエンジンを Rust 言語でオープンソースとして再実装するプロジェクトです。

Morrowind のオープンソース再実装プロジェクトである OpenMW などの取り組みを参考に、オリジナルのアセット形式（ESM、BSA、NIF）を直接読み込み、レンダリングや物理演算、ゲームプレイシステムを現代的なアーキテクチャで構築しています。

![Capital Wasteland World Space](docs/1.png)

---

## 主な機能と実装状況

### 1. アセット・バイナリパーサー (`crates/fo3_*`)

- **fo3_esm**: Fallout 3 マスターファイル (`Fallout3.esm`) の直接ストリーミング走査。CELL、REFR、LAND、STAT、DOOR、LIGHT、NPC\_、ARMO レコードの完全デシリアライズ。
- **fo3_bsa**: Bethesda Archive 形式の高速インデックス解決、zlib リアルタイム解凍。
- **fo3_vfs**: BSA アーカイブ群とルーズファイルを統合し、大文字小文字・セパレータ差分を吸収する仮想ファイルシステム。
- **fo3_nif**: Gamebryo 2.6 NIF (`v20.2.0.7`, `User Version 11`, `User Version 2 34`) のブロックパーサー。NiNode、NiTriShape、NiTriStrips、NiAlphaProperty、bhkRigidBody、bhkPackedNiTriStripsShape、NiSkinInstance 等を忠実に再現。

![Megaton Interior Cell Walkthrough](docs/2.png)

### 2. GPU レンダリングエンジン (`fo3_render`, `fo3_viewer`)

- **現代的低レベルグラフィックス API**: wgpu (Vulkan / DirectX 12 / Metal) による高効率描画パイプライン。
- **テクスチャブレンディング**: ディフューズマップ、法線マップ（Tangent / Bitangent 従法線ベクトル計算対応）、グローマップのサンプリング。
- **ランドスケープ（地形）マルチテクスチャリング**: セル単位のベーステクスチャ（BTXT）および追加ブレンドレイヤー（ATXT）のアルファ合成。
- **Gamebryo レンダリングシーケンス**: 深度書き込みを伴う不透明パスと、カメラ距離ソートを伴う半透明（アルファブレンド / アルファテスト）パスの分離。
- **環境再現**: セル固有のディレクショナルライト、アンビエントライト、環境フォグ、配置点光源（LIGHT レコード）の再現。

![Havok Collision Wireframes](docs/3.png)

### 3. 物理エンジン・キャラクタコントローラー (`fo3_physics`)

- **Havok コリジョン変換**: NIF 内の Havok 物理ブロック（Box、Sphere、Capsule、ConvexHull、TriMesh）および ESM の LAND 標高グリッドを物理エンジンへマッピング。
- **Parry3D 複合形状対応**: Compound 内の複合ネストや複数 TriMesh の結合処理により、安定した剛体登録を実現。
- **リアルタイム KCC (Kinematic Character Controller)**: 重力加速度、斜面登坂、階段の自動昇降（ステップイン）、接地判定、衝突スライド移動を実装。
- **ハイブリッドカメラ**: 全体俯瞰用オービットカメラと、FPS ウォークスルー歩行モードのワンキー切り替え。

![Exterior Exploration and Terrain Physics](docs/4.png)

### 4. アクター・スキニング & キーフレームアニメーション (`fo3_render`, `fo3_nif`)

- **NiSkinData / NiSkinPartition スキニング**: スケルトンボーン階層への頂点ウェイト変形とバインドポーズ逆行列計算。
- **Gamebryo KF キーフレーム再生**: `NiTransformInterpolator`（線形/Slerp補間）および B-Spline 補間（Cox-de Boor基底）による骨格姿勢制御。
- **マルチパーツアクター自動結合**: スケルトン、衣装/素体メッシュ、頭部、両手の一括バインドポーズ解決。
- **剛体 HeadParts アタッチメント**: Fallout 3 仕様に準拠し、眼球（両目）や口内（上下歯・舌）の剛体メッシュを頭部ボーン（`Bip01 Head`）へリアルタイム追従結合。

---

## ワークスペース構成

本プロジェクトは Cargo ワークスペースによるモジュール分離設計を採用しています。

```
OpenFallout3/
├── crates/
│   ├── fo3_gamebryo_core/  # Gamebryo 基本型、数学、トランスフォーム計算
│   ├── fo3_esm/            # ESM / ESP マスターファイルパーサー
│   ├── fo3_bsa/            # BSA アーカイブリーダー
│   ├── fo3_vfs/            # 仮想ファイルシステム (Virtual File System)
│   ├── fo3_nif/            # NIF ジオメトリ & コリジョンブロックパーサー
│   ├── fo3_physics/        # 物理シミュレーション (Rapier3D / KCC 統合)
│   ├── fo3_render/         # wgpu レンダリングパイプライン & シーングラフ
│   └── fo3_viewer/         # インタラクティブ 3D セル / ワールドビューアー
├── docs/                   # スクリーンショット・技術仕様書
├── knowledge/              # 調査済みバイナリ仕様・設計ドキュメント
└── references/             # 一次文献 (nif.xml, NifSkope, OpenMW 等)
```

---

## 必要環境・ビルド方法

### 動作要件

- **Rust**: 1.80 以上 (最新の stable ツールチェーンを推奨)
- **Fallout 3**: Steam 版または GOG 版の正規インストールデータ（`Fallout3.esm` および `Data/*.bsa`）
- **GPU**: Vulkan 1.2、DirectX 12、または Metal に対応したグラフィックス環境

### リポジトリのクローンとビルド

```bash
git clone https://github.com/sofia-gros/OpenFallout3.git
cd OpenFallout3
cargo build --release
```

### 単体テストの実行

```bash
cargo test
```

---

## 実行方法 (Viewer)

`fo3_viewer` を使用して、メッシュ単体、室内セル、または広域ワールドスペースを直接読み込んで探索できます。

### 1. 室内セル (Interior Cell) の読み込み

```bash
cargo run --release -p fo3_viewer -- cell "<Fallout 3 Data ディレクトリのパス>" "<Cell EDID>"
```

例:

```bash
# Vault 101 エントランス
cargo run --release -p fo3_viewer -- cell "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "Vault101a"

# スプリングベール小学校
cargo run --release -p fo3_viewer -- cell "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "SpringvaleSchool01"

# メガトン・モリアティの酒場
cargo run --release -p fo3_viewer -- cell "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "MegatonMoriartysSaloon"
```

### 2. ワールドスペース (World Space) の読み込み

```bash
cargo run --release -p fo3_viewer -- world "<Fallout 3 Data ディレクトリのパス>" "<World EDID>" [GridX] [GridY]
```

例:

```bash
# メガトン (MegatonWorld)
cargo run --release -p fo3_viewer -- world "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "MegatonWorld"

# キャピタル・ウェイストランド広域 (Wasteland)
cargo run --release -p fo3_viewer -- world "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "Wasteland" -1 2

# ワシントンD.C. 廃墟地区 (DCWorld01)
cargo run --release -p fo3_viewer -- world "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "DCWorld01"
```

### 3. NIF メッシュ単体の読み込み

```bash
cargo run --release -p fo3_viewer -- "<Fallout 3 Data ディレクトリのパス>" "<NIF ファイル相対パス>"
```

例:

```bash
cargo run --release -p fo3_viewer -- "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "meshes\weapons\1handpistol\10mmpistol.nif"
```

### 4. 単体メッシュアニメーションの読み込み (Anim)

```bash
cargo run -p fo3_viewer -- anim "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "meshes\characters\_male\upperbody.nif" "meshes\characters\_male\idleanims\ttnpchappysubtlelistena.kf"
```

### 5. フルアクター結合 & アニメーション再生 (Actor)

頭部、両眼球、上下歯、舌、素体/衣装、両手を一括結合し、ボーンおよび剛体アタッチメントを連動させてアニメーションを再生します。

```bash
cargo run -p fo3_viewer -- actor "<Fallout 3 Data ディレクトリのパス>" "<防具NIF相対パス または naked>" "<KF アニメーション相対パス>"
```

例:

```bash
# 裸体素体 + アイドルアニメーション（頭部・両目・口内・手先が完全結合）
cargo run -p fo3_viewer -- actor "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" naked "meshes\characters\_male\idleanims\ttnpchappysubtlelistena.kf"

# ウェイストランド防具 + アイドルアニメーション
cargo run -p fo3_viewer -- actor "A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data" "meshes\armor\wastelandclothing01\outfitm.nif" "meshes\characters\_male\idleanims\ttnpchappysubtlelistena.kf"
```

---

## 操作方法

| キー / マウス操作    | 機能                                                        |
| :------------------- | :---------------------------------------------------------- |
| **Tab / M**          | カメラモード切替（オービット周回 ⇔ FPS 歩行モード）         |
| **W / A / S / D**    | 前進 / 左移動 / 後退 / 右移動（FPS 歩行モード時）           |
| **Space**            | ジャンプ / 上昇（FPS 歩行モード時）                         |
| **マウス移動**       | 視線回転（Look）                                            |
| **マウス左ドラッグ** | カメラ回転（オービットモード時）                            |
| **マウス右ドラッグ** | カメラ平行移動（Pan）（オービットモード時）                 |
| **マウスホイール**   | ズームイン / ズームアウト（オービットモード時）             |
| **C**                | Havok コリジョンワイヤーフレーム表示切替 (Collision ON/OFF) |
| **F**                | セル環境フォグ表示切替 (Fog ON/OFF)                         |
| **L**                | ビューアー補助ヘッドライト切替 (Light ON/OFF)               |
| **R**                | カメラ注視点自動再フォーカス                                |
| **Esc**              | ビューアー終了                                              |

---

## 開発ロードマップと実装計画 (Roadmap)

本プロジェクトは OpenMW のアーキテクチャ成熟プロセスと Gamebryo 2.6 の仕様に準拠した段階的マイルストーンに沿って開発を進めています。

| フェーズ | 目標と概要 | 状態 |
| :--- | :--- | :--- |
| **Phase 1: ファイル基盤** | BSA v104 解凍、仮想ファイルシステム (VFS) | **完了** |
| **Phase 2: 3Dメッシュパース** | Gamebryo 2.6 NIF コアノード、ジオメトリ、マテリアル完全解析 | **完了** |
| **Phase 3: レンダリング** | wgpu、法線マップ、スペキュラ、半透明ソート、地形スプラット | **完了** |
| **Phase 4: ESM セル配置** | CELL、REFR、STAT、LAND、LIGHT レコードによる空間配置再現 | **完了** |
| **Phase 5: コリジョン・物理** | Havok コリジョン変換、Rapier3D 統合、リアルタイム KCC 移動 | **完了** |
| **Phase 6-A/B: スキニング・アニメ** | NiSkinData スキニング、KF 補間、マルチパーツアクター結合 | **完了** |
| **Phase 6-C: セル内アクター配置 (次期)** | 室内・屋外セル内での全 NPC (ACHR) 自動組み立てとアイドル動作 | **計画中** |
| **Phase 6-D: GPU スキニング** | 頂点シェーダー内でのボーンパレット参照による描画最適化 | 予定 |
| **Phase 7: インタラクション** | ドアテレポート (XTEL)、コンテナ、アクティベーター作動 | 予定 |
| **Phase 8: キャラクター & カメラ** | プレイヤーアクター、三人称/一人称モデル統合、ステートマシン | 予定 |
| **Phase 9: スクリプト VM & 会話** | SCPT バイトコード実行仮想マシン、ダイアログ (DIAL/INFO) UI | 予定 |

---

## 注意事項・免責事項

- 本ソフトウェアはオープンソースのゲームエンジン再実装であり、Fallout 3 のゲームアセット（テクスチャ、メッシュ、サウンド、マスターファイル等）は一切含まれていません。動作にはユーザー自身が所有する正規のゲームデータが必要です。
- 本プロジェクトは Bethesda Softworks または ZeniMax Media と提携、承認、支援されたものではありません。
- すべての商標および登録商標は、それぞれの所有者に帰属します。
