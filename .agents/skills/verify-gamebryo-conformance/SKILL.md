---
name: verify-gamebryo-conformance
description: >-
  Verify that Rust scene graph data structures, transformation pipelines, and property cascading
  strictly conform to Gamebryo 2.6 architecture without modern engine abstractions.
---

# Verify Gamebryo Conformance Skill

このスキルは、実装された Rust コードが Gamebryo 2.6 の設計原則に厳格に準拠しているかを静的にチェック・検証するための手順です。

## チェック項目

### 1. シーングラフ階層とトランスフォーム更新順序
- [ ] 親子関係において、親ノードの更新が完了してから子ノードが更新されるか（`UpdateDownwardPass`）。
- [ ] トランスフォーム伝播の計算式が Gamebryo 2.6 と一致しているか:
  - $R_{world} = R_{parent} \times R_{local}$
  - $S_{world} = S_{parent} \times S_{local}$
  - $T_{world} = R_{parent} \times (S_{parent} \times T_{local}) + T_{parent}$
- [ ] 子のバウンディングボリューム（包含球）を親へ再帰マージしているか（`UpdateUpwardPass`）。

### 2. プロパティシステムのカスケード評価
- [ ] プロパティステート（`NiPropertyState`）が、ツリー上流の親ノードから下流の子ノードへ正しく継承されているか。
- [ ] 子ノード側で同種のプロパティが再定義されている場合、親のプロパティが適切にオーバーライドされているか。
- [ ] 独自のマテリアル構造を作らず、`NiMaterialProperty` や `BSShaderPPLightingProperty` のプロパティリストとして管理されているか。

### 3. アリーナ構造と所有権モデル
- [ ] `Rc<RefCell<T>>` などの生スマートポインタ循環を避け、`SlotMap` またはインデックスアリーナによる安全なノード追跡が行われているか。
- [ ] ノードの追加・削除・デタッチが Gamebryo の `AttachChild` / `DetachChild` のセマンティクスと整合しているか。
