# OpenMW の歴史的実装順序に学ぶ Fallout 3 エンジン開発ロードマップ

## 現在の開発進捗状況 (2026年9月現在)

| フェーズ | クレート | 状態 | 詳細 |
| :--- | :--- | :--- | :--- |
| **Phase 1: ファイル基盤** | `fo3_bsa`, `fo3_vfs` | **100% 完了** | BSA v104 完全解凍、ルーズファイル優先透過読み込み |
| **Phase 2: 3Dメッシュパース** | `fo3_nif` | **100% 完了** | NiNode, BSFadeNode, NiTriShape/Strips, BSShaderPPLightingProperty, BSShaderTextureSet, NiMaterialProperty, NiAlphaProperty |
| **Phase 3: レンダリング** | `fo3_render` | **100% 完了** | wgpu, DDS デコード, 法線マッピング, Blinn-Phong スペキュラ, Glow Map 自己発光, 半透明ソート描画, 地形スプラット |
| **Phase 4: ESM 配置・データ構造** | `fo3_esm` | **100% 完了** | STAT, SCOL, DOOR, ACTI, FURN, CONT, LIGH, CELL, WRLD, LAND, REFR 拡張 (XTEL テレポートドア, XLOC 施錠, XESP 親連動, XMRK/TNAM マーカー, XOWN 所有権, XCNT スタック), ACHR/ACRE 配置アクター |
| **Phase 4-3: 室内・屋外探索** | `fo3_viewer` | **達成済み** | Vault 101 / Megaton / Wasteland を光源・環境光・マテリアル付きでフリーカメラ探索可能 |
| **Phase 5: コリジョンと基本物理** | `fo3_nif` (bhk), `fo3_physics` | **着手予定** | Havok 物理 / bhkRigidBody / bhkMoppBvTreeShape パース、接地・歩行コントローラー |
| **Phase 6: スケルトンとアニメーション** | `fo3_gamebryo_anim` | 未着手 | NiSkinInstance, NiSkinData, ボーンスキニング, KF キーフレーム再生 |
| **Phase 7: エクステリア動的ストリーミング** | `fo3_world` | 未着手 | $3 \times 3$ セルグリッド動的ロード/アンロード |
| **Phase 8: スクリプト・ゲームプレイ** | `fo3_gameplay` | 未着手 | SCPT スクリプト仮想マシン, Pip-Boy UI, 会話 |

---

## 1. OpenMW が辿った実装の軌跡（歴史的経緯の分析）

OpenMW（0.1.0 〜 1.0）の CHANGELOG およびアーキテクチャ進化ログを分析すると、極めて合理的かつ安全な開発順序が浮かび上がります。

```mermaid
graph TD
    A["Phase 1: CLIツールとパーサー<br>(esmtool, bsatool, niftest)"] --> B["Phase 2: 仮想ファイルシステム (VFS)<br>(BSAアーカイブ + ルーズファイルの透過アクセス)"]
    B --> C["Phase 3: 単体メッシュビューアー<br>(NIFパース, 頂点/法線/UV, DDSテクスチャ描画)"]
    C --> D["Phase 4: 単一インテリアセルの静的配置<br>(CELL/REFR読込, 静的オブジェクトの空間配置)"]
    D --> E["Phase 5: カメラと基本物理<br>(フリーカメラ → コリジョン判定と歩行)"]
    E --> F["Phase 6: スケルトンとアニメーション<br>(NiSkinInstance, ボーンスキニング, アイドル再生)"]
    F --> G["Phase 7: エクステリアセルと地形<br>(LANDレコード, $3 \times 3$ セルグリッド動的ストリーミング)"]
    G --> H["Phase 8: ゲームプレイとGUI<br>(インベントリ, 会話, スクリプトVM, VATS)"]
```

### なぜこの順序なのか？（失敗を避けるための鉄則）
1. **なぜ最初は CLI なのか？**:
   ウィンドウ生成や GPU パイプラインを初期段階で持ち込むと、パースエラーなのかシェーダーエラーなのかの切り分けが困難になりトークンと時間を浪費する。
   まずは CLI で純粋にバイナリが 1 バイトの余りもなく正確に読めることをテスト駆動（TDD）で固める。
2. **なぜ VFS が描画の前に必須なのか？**:
   NIF 内のテクスチャ参照（`textures\clutter\streetlights\lamppost01.dds` 等）は、ディスク上のルーズファイルにある場合と、`Fallout - Textures.bsa` 内に圧縮されている場合がある。VFS がないと、メッシュのテクスチャを貼るたびに個別コードが必要になり破綻する。
3. **なぜ「単体メッシュ」→「インテリアセル」なのか？**:
   いきなり広大なメガトンや屋外ワールド（WRLD）を読み込もうとすると、セルストリーミング、ハイトマップ地形（LAND）、オクルージョン、LOD が絡み合いバグが爆発する。
   まず単体 NIF を完璧に描画し、次に「閉じた室内空間（Vault 101 やメガトンの民家などの単一 CELL）」を静的に配置することで、最小限のスコープで世界を構築できる。

---

## 2. Fallout 3 (Gamebryo 2.6) 向け最終開発順序

OpenMW の先人の知恵を Fallout 3 の仕様（Gamebryo 2.6, NIF v20.2.0.7, BSA v104, ESM v0.94）に適用した最終開発順序です。

---

### ステップ 1: アセット基盤と VFS (Virtual File System)
- **1-1. BSA (v104) デシリアライザ (`fo3_bsa`)**:
  - `Data/Fallout - Meshes.bsa`, `Fallout - Textures.bsa` のヘッダー、ディレクトリレコード、ファイルレコードのパース。
  - zlib 圧縮ブロックの解凍とオンデマンド読み出し。
- **1-2. VFS (`fo3_vfs`)**:
  - ルーズファイル（ディスク優先）と BSA アーカイブを統合し、`meshes/...` や `textures/...` のパスから直接ストリームを取得する透過的ファイルリゾルバ。

---

### ステップ 2: NIF (v20.2.0.7) ジオメトリパース完全制覇
- **2-1. コアジオメトリブロックの実装 (`fo3_nif`)**:
  - `NiNode`, `BSFadeNode`
  - `NiTriShape`, `NiTriShapeData`, `NiTriStrips`, `NiTriStripsData`
  - 頂点座標（Vertices）、法線（Normals）、UV座標（TexCoords）、三角形インデックス（Triangles）。
- **2-2. マテリアル・テクスチャブロックの実装**:
  - `BSShaderPPLightingProperty`
  - `BSShaderTextureSet` (ディフューズ、ノーマルマップ等のパス取得)
  - `NiMaterialProperty`, `NiAlphaProperty`
- **2-3. CLI 検証 (`fo3_testbed`)**:
  - 実ゲームの `lamppost01ONOFF.nif` や武器・クラッターの NIF を全ブロック解析し、ジオメトリデータとテクスチャパスが 100% 正常に抽出できることを自動テスト。

---

### ステップ 3: 単体メッシュビューアー (`fo3_viewer`)
- **3-1. wgpu レンダリングパイプライン (`fo3_render_wgpu`)**:
  - ウィンドウ生成（`winit`）と WebGPU コンテキストの初期化。
  - `wgpu` の頂点バッファ・インデックスバッファのセットアップ。
- **3-2. テクスチャローダー**:
  - DDS デコーダ（DXT1, DXT3, DXT5, 非圧縮）を実装し、VFS 経由でテクスチャを GPU へアップロード。
- **3-3. シェーダー実装**:
  - `BSShaderPPLightingProperty` を模したフォン/ブラインフォンシェーダー（ディフューズ + 法線マップ + 簡易スペキュラ）。
- **3-4. 単体メッシュビューアー完成**:
  - CLI 引数で指定した単体 NIF（例: ピストルや標識）をウィンドウに表示し、マウスドラッグで 360度 観察できる。

---

### ステップ 4: ESM パーサーとインテリアセル配置 (`fo3_esm` & `fo3_world`)
- **4-1. ESM/ESP レコードパーサー (`fo3_esm`)**:
  - `Fallout3.esm` のグループ構造（Top Group, Subgroups）とレコードヘッダー（FormID, Flags）のパース。
  - `STAT` (Static Object) レコード: 表示用 NIF モデルパスの取得。
  - `CELL` (Interior Cell) レコード: 室内セル（例: `Vault101a`）の抽出。
  - `REFR` (Reference) レコード: 配置オブジェクトの FormID、ワールド座標（X, Y, Z）、回転（P, Y, R）、スケール。
- **4-2. アリーナ型シーングラフへの展開 (`fo3_gamebryo_scene`)**:
  - 室内セルに含まれる全 `REFR` をインスタンス化し、`NiNode` ツリーを構築。
  - `UpdateDownwardPass` により、ワールド行列を一括計算。
- **4-3. 室内ウォークスルー**:
  - フリーカメラ（WASD + マウス）で、Vault 101 やメガトンの室内セルを自由に飛び回れるようにする。

---

### ステップ 5: キャラクター移動と基本物理（Havok / コリジョン）
- **5-1. NIF 内のコリジョンメッシュ抽出**:
  - `bhkRigidBody`, `bhkBoxShape`, `bhkMoppBvTreeShape`。
- **5-2. 簡易プレイヤーコントローラー**:
  - プレイヤーの足元レイキャスト、接地判定、重力落下、壁衝突判定。
  - 地面に立って室内を歩行可能にする。

---

### ステップ 6: スケルトンとアニメーション
- **6-1. ボーン階層とスキニング (`fo3_gamebryo_anim`)**:
  - `NiSkinInstance`, `NiSkinData`, `NiSkinPartition` のパース。
  - GPU 頂点シェーダーでのボーンマトリクススキニング。
- **6-2. キーフレームアニメーション (`NiTimeController`)**:
  - KF ファイルまたは NIF 内の `NiTransformInterpolator` によるボーン回転・移動のアニメーション再生。

---

### ステップ 7: エクステリアセルと地形（Landscape / Terrain）
- **7-1. `LAND` レコードの解析**:
  - ハイトマップ（頂点高さデータ）、頂点法線、テクスチャ頂点カラー。
- **7-2. 地形レンダリング**:
  - グリッドメッシュの生成とテクスチャブレンド（スプラッティング）。
- **7-3. セルグリッド動的ストリーミング**:
  - プレイヤーの移動に伴う周囲 $3 \times 3$ セルの動的ロード・アンロード。

---

### ステップ 8: ゲームプレイ・スクリプト・GUI
- **8-1. スクリプトエンジン (`SCPT`)**:
  - Fallout 3 のスクリプトバイトコード仮想マシンの実装。
- **8-2. UI システム**:
  - HUD（コンパス、HP/APバー）、ピップボーイ（Pip-Boy 3000）、会話ウィンドウ。
