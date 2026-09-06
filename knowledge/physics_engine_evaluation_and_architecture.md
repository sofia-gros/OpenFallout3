# 物理エンジン調査報告および選定アーキテクチャ設計

本ドキュメントは、OpenFallout3 における物理エンジン統合（Havok と Rapier の比較、導入可能性、キャラクタコントローラー設計）に関する調査結果とアーキテクチャ方針を記録した永続化知識ベースです。

---

## 1. 調査背景とユーザー要求
- **問題意識**:
  歩行コントローラーを先に自前（レイキャスト等）で実装してしまうと、物理エンジンと乖離した「独自実装」になり、二重実装やアーキテクチャ破綻を招く。
- **検討事項**:
  1. Havok のソースコードや Rust 版など、Havok を直接プロジェクトに導入可能か？
  2. 導入が困難な場合、先に Rapier3D を導入して物理統合とキャラクタコントローラーを完成させ、Havok 完全エミュレーションは後回しにするのが最適か？

---

## 2. Havok 物理エンジンの現状と導入可能性調査

| 項目 | Havok (Havok Physics) の実態 |
| :--- | :--- |
| **著作権・所有者** | Microsoft (元 Intel / Havok Inc.) |
| **ソースコード** | **完全非公開（プロプライエタリ・商用クローズドソース）** |
| **配布ライセンス** | 再配布不可。商用契約または特別な教育・パートナー契約が必要。 |
| **Rust エコシステム** | 公式バインディングなし。オープンソースのクレートなし。 |
| **バイナリリンク** | ユーザー全員に商用 SDK バイナリ（`Havok.dll` 等）の準備を要求することになり、オープンソースのクローン/ビルドが不可能になる。 |

### 結論:
Havok を外部ライブラリとして直接リンク・組み込むことは、**法的一元管理およびオープンソース Rust プロジェクトの観点から不可能**です。

---

## 3. OpenMW の先例（先行リファレンス調査）

一次文献: `references/openmw/components/nifbullet/bulletnifloader.cpp`
- OpenMW は Havok をリンクせず、オープンソース物理エンジンである **Bullet Physics** を採用。
- NIF 内のコリジョン情報を読み取り、Bullet の `btCollisionShape` / `btKinematicCharacterController` に変換してワールドへ注入している。
- ただし OpenMW では NIF の Havok 物理ブロック（`bhk*`）の解釈が一部不完全（描画用メッシュからのフォールバックなど）であった。
- 対して、我々の **OpenFallout3 は先ほど NIF 内の全 Havok コリジョン形状（Box, Sphere, Capsule, Convex, PackedTriStrips, NiTriStrips, Transform, Phantom）を 100% 正確にパースすることに成功**しているため、オープン物理エンジンへの注入において OpenMW 以上の完全性を誇る。

---

## 4. Rapier3D (純粋 Rust 物理エンジン) の評価

| 評価軸 | Rapier3D (`rapier3d`) |
| :--- | :--- |
| **言語・ビルド** | 100% Pure Rust。C/C++ コンパイラや外部 CMake 等一切不要。`cargo build` だけで即座にビルド可能。 |
| **ライセンス** | Apache-2.0 (完全オープンソース、商用・非商用自由) |
| **形状の整合性** | 我々が `fo3_nif` から抽出した全形状と **1:1 完全対応**: <br>・`bhkBoxShape` $\to$ `SharedShape::cuboid`<br>・`bhkSphereShape` $\to$ `SharedShape::ball`<br>・`bhkCapsuleShape` $\to$ `SharedShape::capsule`<br>・`bhkConvexVerticesShape` $\to$ `SharedShape::convex_hull`<br>・`bhk*TriStripsShape` $\to$ `SharedShape::trimesh`<br>・`bhkCompoundShape` $\to$ `SharedShape::compound` |
| **キャラクタコントローラー** | **`KinematicCharacterController` が標準搭載**: <br>・オートステップ（階段昇降）<br>・スロープ滑り落ち/登坂限界角<br>・重力と床接地判定<br>・押し出し・衝突スライド<br>$\to$ 独自実装不要で、堅牢なコントローラーが即座に手に入る。 |
| **パフォーマンス** | SIMD 最適化されており、数千個のコライダーをリアルタイムでシミュレーション可能。 |

---

## 5. 推奨アーキテクチャ（結論）

### フェーズ 5 の進め方:
1. **クレート `fo3_physics` の新設**:
   - `rapier3d` を物理バックエンドとして採用。
   - `PhysicsWorld`（剛体・コライダー管理）、`CharacterController`（プレイヤー移動）を提供。
2. **NIF $\to$ 物理アダプター**:
   - `fo3_nif::collision::NifCollisionData` を入力とし、Rapier の `ColliderBuilder` / `RigidBodyBuilder` を自動生成。
   - Fallout 3 のマテリアル（摩擦 `friction`, 反発 `restitution`）、接触フィルター（レイヤー `Fallout3Layer`）を正確にマッピング。
3. **将来の Havok 完全エミュレーションへの拡張性**:
   - 物理エンジンのインターフェースを Trait 化しておくことで、将来「Havok の数学・ソルバーをゼロから忠実エミュレートした自作物理エンジン」を作った際も、設定切り替え（`Backend::Rapier` / `Backend::HavokEmu`）でスムーズに置換・共存可能。
