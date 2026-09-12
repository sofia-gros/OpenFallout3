# Gamebryo 2.6 & Fallout 3 リソース管理・キャッシュアーキテクチャ

## 1. 概要と基本理念 (Overview & Philosophy)

Gamebryo 2.6 (Fallout 3 / Oblivion) において、ゲーム世界は広大であり、何万ものオブジェクトやテクスチャ、セル間遷移が頻繁に発生します。
実機 Fallout 3 では、セルを移動するたびにアーカイブ（BSA）のインデックスを再構築したり、一度読み込んだ 3D メッシュ（NIF）やテクスチャ（DDS）をゼロから再デコード・再アップロードすることはありません。

本ドキュメントでは、Gamebryo 2.6 および OpenMW `ResourceSystem` の一次文献に基づき、OpenFallout3 におけるリソース永続化・多層キャッシュの仕様を定義します。

---

## 2. 一次文献と根拠 (Primary References)

1. **Gamebryo 2.6 Core Architecture**:
   - `NiStream`: NIF ファイルの逆シリアライズおよび共有オブジェクトグラフの復元。
   - `NiObject::Clone` / `NiCloningProcess`: 共有ジオメトリデータ（`NiTriShapeData`）やテクスチャプロパティ（`NiTexturingProperty`）を参照共有したまま、インスタンス固有のノード（`NiNode`, `NiTransform`）のみを高速複製する機構。
   - `NiSourceTexture`: ファイルパス（正規化文字列）をキーとしたテクスチャキャッシュ。同一テクスチャは GPU メモリ上に 1 つのみ確保され、全マテリアルで共有される。
2. **OpenMW Architecture**:
   - `references/openmw/components/resource/resourcesystem.hpp`: VFS を中心とした統合リソースマネージャ。
   - `references/openmw/components/resource/niffilemanager.hpp`: `NifFileManager` によるパース済み `NIFFilePtr` のスレッドセーフなキャッシング。
   - `references/openmw/components/resource/imagemanager.hpp`: テクスチャイメージ・GPU オブジェクトのキャッシング。
3. **Bethesda ESM/BSA Architecture**:
   - `Fallout3.esm`: ゲーム起動時にマスターレコード群（EDID/FormID インデックス、STAT, DOOR, CONT, NPC_, ARMO 等の定義）をメモリに常駐。
   - `BSA (Bethesda Softworks Archive)`: 起動時にディレクトリツリーとハッシュテーブルを一度だけパースし、ファイルハンドルを開いたままシークロード。

---

## 3. 4層キャッシュ階層 (4-Tier Cache Hierarchy)

```
+-------------------------------------------------------------------------+
| Layer 1: Persistent VFS & Archive Handles (fo3_vfs::VfsManager)         |
|   - 全 BSA のヘッダー・ディレクトリエントリ・ハッシュテーブル常駐       |
|   - 開放済みファイルハンドルの維持                                      |
+-------------------------------------------------------------------------+
                                   |
                                   v
+-------------------------------------------------------------------------+
| Layer 2: Master ESM Definition Cache (EsmCache / MasterContext)         |
|   - 3Dモデル定義 (model_map: FormId -> BaseObjectInfo)                  |
|   - アクター・防具・衣装・髪型・LVLI 定義マップ                         |
|   - 光源定義マップ (light_map)                                          |
+-------------------------------------------------------------------------+
                                   |
                                   v
+-------------------------------------------------------------------------+
| Layer 3: Shared NIF AST Cache (NifCache: Path -> Arc<NifFile>)          |
|   - パース済み NIF 構造体のメモリプール                                 |
|   - 複数セル・複数 REFR 間でのゼロコスト共有                            |
+-------------------------------------------------------------------------+
                                   |
                                   v
+-------------------------------------------------------------------------+
| Layer 4: GPU Texture Cache (TextureCache: Path -> Arc<GpuTexture>)      |
|   - VRAM アップロード済みテクスチャ (Diffuse, Normal, Glow) の共有      |
|   - 同一テクスチャの重複 GPU アロケーション完全排除                     |
+-------------------------------------------------------------------------+
```

---

## 4. 各レイヤーの実装仕様

### 4.1 Layer 1: 永続 VFS (`VfsManager`)
- **問題点**: 従来の `load_scene` では、セル遷移のたびに `VfsManager::new()` を実行し、全 BSA を毎回リオープンして全ファイルハッシュを再計算していた。
- **改善仕様**:
  - `ViewerState`（またはアプリケーションコンテキスト）が単一の `VfsManager` インスタンスを所有。
  - セル切り替え時は既存の `&VfsManager` を `load_scene` に渡し、BSA の再オープンをゼロにする。

### 4.2 Layer 2: マスター ESM 定義キャッシュ (`MasterContext`)
- **問題点**: 1.3GB の `Fallout3.esm` をセル遷移のたびに再スキャンし、8,823件のモデルや1,647件のNPCを再パースしていた。
- **改善仕様**:
  - 起動時に以下のマスター辞書を一度だけ構築し、メモリ上に常駐:
    - `model_map: HashMap<FormId, BaseObjectInfo>`
    - `npc_map: HashMap<FormId, NpcRecord>`
    - `armor_map: HashMap<FormId, ArmoRecord>`
    - `outfit_map: HashMap<FormId, OtftRecord>`
    - `hair_map: HashMap<FormId, HairRecord>`
    - `lvli_map: HashMap<FormId, LvliRecord>`
    - `light_map: HashMap<FormId, LightRecord>`
  - セルロード時は、対象セルのセルレコード・REFR・LAND のみを取得する（`read_cell_refrs_and_land`）。

### 4.3 Layer 3: NIF AST キャッシュ (`NifCache`)
- **問題点**: メッシュごとに BSA から解凍・バイナリパースを毎回実行していた。
- **改善仕様**:
  - `HashMap<String, Arc<fo3_nif::NifFile>>` を永続化。
  - パスを小文字・バックスラッシュ正規化（例: `meshes\architecture\megaton\metalscrapdoor01.nif`）。
  - キャッシュにヒットした場合は `Arc::clone` のみで即座に返却（解凍・パース時間 0ms）。

### 4.4 Layer 4: GPU テクスチャキャッシュ (`TextureCache`)
- **問題点**: 同一の `MetalScrap01.dds` などのテクスチャが REFR ごと、およびセル切り替えごとに毎回 GPU に再生成されていた。
- **改善仕様**:
  - `HashMap<String, Arc<GpuTexture>>` を `RenderContext` または `ViewerState` で永続管理。
  - キャッシュヒット時は既存の `Arc<GpuTexture>` の `view` と `sampler` をそのまま `BindGroup` に設定。
  - 重複アップロード・VRAM 浪費・セル切り替え時のスパイクを解消。

---

## 5. Gamebryo 2.6 クローニング適合性 (`NiObject::Clone`)

Gamebryo 2.6 では、ジオメトリデータ（`NiTriShapeData`）は不変（Immutable）の共有リソースであり、インスタンスごとに以下の固有データのみが生成されます:
1. `NiTransform`（ワールド変換行列: 平行移動、回転、スケール）
2. ユニフォームバッファ（wgpu の `model_bind_group`）
3. バウンディングボックス・スフィア (`bound.transform(world_transform)`)

本キャッシュ設計は、Gamebryo 2.6 のデータモデルと完全に一致します。
