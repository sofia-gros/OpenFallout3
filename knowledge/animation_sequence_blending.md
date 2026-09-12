# Gamebryo 2.6 アニメーションシーケンス合成 & リップシンク・フェイシャル仕様

本ドキュメントは、Gamebryo 2.6 および Fallout 3 における複数アニメーションシーケンス（ボディ、フェイシャル、リップシンク）のブレンド・合成仕様と数学モデルを記録する。

---

## 1. クラス構造とアーキテクチャ

参照元:
- `references/nifxml/nif.xml:L4195` (`NiControllerManager`)
- `references/nifxml/nif.xml:L4214` (`NiControllerSequence`)
- Gamebryo 2.6 `NiControllerManager.h`, `NiControllerSequence.h`

```
NiControllerManager (ルートノードにアタッチ)
 ├── NiControllerSequence (Track 0: Base Body, e.g. Idle, Priority: 0, Weight: 1.0)
 ├── NiControllerSequence (Track 1: Locomotion, e.g. Walk, Priority: 0, Weight: 0.0..1.0)
 ├── NiControllerSequence (Track 2: Facial Expression, e.g. Smile/Blink, Priority: 10, Weight: 1.0)
 └── NiControllerSequence (Track 3: Lip Sync, e.g. Phonemes, Priority: 20, Weight: 1.0)
```

- **`Priority` (i32)**: アニメーション階層の優先度。上位優先度トラックが制御するボーンは、下位優先度トラックのボーン変換を上書きする（例: リップシンクの口の動きは、歩行アニメーションの頭部傾きを保持しつつ、口・顎ボーンのみを上書き制御する）。
- **`Weight` (f32, 0.0 .. 1.0)**: 同一優先度内での合成比率。クロスフェード時に使用。
- **`ControlledBlocks`**: 各シーケンスが制御する対象ボーンノード一覧。フェイシャル・リップシンクは頭部・顔ボーンのみを含み、体ボーンを含まないため、自動的に局所レイヤー合成（Additive/Masking）が成立する。

---

## 2. ボーンブレンドの数学モデル

参照元:
- Gamebryo 2.6 `NiTransformInterpolator::Interpolate`
- `references/openmw/components/nifosg/nifloader.cpp`

### 2.1 2つのボーン変換のブレンド
2つのボーンローカル変換 $\mathcal{T}_A = (\mathbf{T}_A, \mathbf{R}_A, S_A)$ と $\mathcal{T}_B = (\mathbf{T}_B, \mathbf{R}_B, S_B)$、合成比率 $\alpha \in [0, 1]$：

1. **平行移動 (Translation)**:
   $$
   \mathbf{T}(\alpha) = (1 - \alpha)\mathbf{T}_A + \alpha \mathbf{T}_B
   $$
2. **回転四元数 (Rotation)**:
   $$
   \mathbf{R}(\alpha) = \text{SLERP}(\mathbf{R}_A, \mathbf{R}_B, \alpha) = \frac{\sin((1 - \alpha)\theta)}{\sin\theta}\mathbf{R}_A + \frac{\sin(\alpha\theta)}{\sin\theta}\mathbf{R}_B
   $$
   （ここで $\cos\theta = \mathbf{R}_A \cdot \mathbf{R}_B$、必要に応じて最短弧となるよう内積が負の場合は符号反転）。
3. **スケール (Scale)**:
   $$
   S(\alpha) = (1 - \alpha)S_A + \alpha S_B
   $$

### 2.2 優先度（Priority）とマルチトラック評価アルゴリズム
1. アクティブな全トラック $k = 1, \dots, N$ を評価し、各トラックのローカルボーン姿勢 $\mathcal{P}_k$ をサンプリングする。
2. ボーン名ごとに、制御しているトラック群を抽出する。
3. そのボーンを制御するトラックの中で、**最大優先度** $\text{Prio}_{\max}$ を持つトラック群のみを対象とする。
4. 対象トラック群のウェイト $w_k$ を正規化し、加重ブレンドを適用する:
   $$
   \tilde{w}_k = \frac{w_k}{\sum_{j} w_j}
   $$
5. 最終的なボーン姿勢 $\mathcal{P}_{\text{final}}$ を確定し、スケルトンの前方運動学 (FK: `recompute_bone_world_maps_with_pose`) に入力する。
