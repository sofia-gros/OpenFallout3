# Gamebryo 2.6 / Fallout 3 NiAlphaProperty 仕様

## 1. 概要

`NiAlphaProperty` は、Gamebryo 2.6 におけるアルファテスト（Alpha Test / Cutout）およびアルファブレンディング（Alpha Blend / Transparency）を制御するプロパティブロックである。
フェンス、金網、窓ガラス、木の葉、デカール等の透過・切り抜き描画に不可欠。

- 一次文献:
  - `references/nifskope/build/nif.xml:L1518-1528` (`AlphaFlags`)
  - `references/nifskope/build/nif.xml:L1047-1058` (`TestFunction`)
  - `references/nifskope/build/nif.xml:L1060-1073` (`AlphaFunction`)
  - `references/nifskope/src/gl/glproperty.cpp:L204-237` (`AlphaProperty::updateImpl`)
  - `references/nifxml/nif.xml:L3972`

---

## 2. バイナリデータ構造

| フィールド | 型 | バイトサイズ | 説明 |
| :--- | :--- | :--- | :--- |
| `net` | `NiObjectNET` | 可変 | オブジェクト名等の基本情報 |
| `flags` | `u16` | 2 | アルファブレンド/テスト制御ビットフラグ |
| `threshold` | `u8` | 1 | アルファテスト基準閾値 (0〜255) |

---

## 3. ビットフラグ仕様 (`AlphaFlags: u16`)

| ビット位置 | マスク | 名前 | 型 / 意味 |
| :--- | :--- | :--- | :--- |
| bit 0 | `0x0001` | **Alpha Blend** | ブレンド有効化 (1 = 有効, 0 = 無効) |
| bit 1..4 | `0x001E` | **Source Blend Mode** | 送信元ブレンド係数 (`AlphaFunction`) |
| bit 5..8 | `0x01E0` | **Destination Blend Mode** | 送信先ブレンド係数 (`AlphaFunction`) |
| bit 9 | `0x0200` | **Alpha Test** | アルファテスト有効化 (1 = 有効, 0 = 無効) |
| bit 10..12 | `0x1C00` | **Test Func** | テスト比較関数 (`TestFunction`) |
| bit 13 | `0x2000` | **No Sorter** | 1 の場合ソート無効、0 の場合カメラ距離でソート |
| bit 14 | `0x4000` | **Clone Unique** | Bethesda 特有フラグ |
| bit 15 | `0x8000` | **Editor Alpha Threshold** | 外部制御フラグ |

---

## 4. テスト比較関数 (`TestFunction`)

閾値 `ref = threshold / 255.0` と、サンプリングしたテクスチャのアルファ値 `alpha` を比較:

| 値 | 名前 | 判定条件 | 説明 |
| :--- | :--- | :--- | :--- |
| 0 | `TEST_ALWAYS` | 常に真 | テスト無効化と同等 |
| 1 | `TEST_LESS` | `alpha < ref` | 閾値未満を描画 |
| 2 | `TEST_EQUAL` | `alpha == ref` | 一致時のみ描画 |
| 3 | `TEST_LESS_EQUAL`| `alpha <= ref` | 閾値以下を描画 |
| 4 | `TEST_GREATER` | `alpha > ref` | **標準的なアルファカットアウト**（金網・木の葉等） |
| 5 | `TEST_NOT_EQUAL`| `alpha != ref` | 不一致時のみ描画 |
| 6 | `TEST_GREATER_EQUAL`| `alpha >= ref` | 閾値以上を描画 |
| 7 | `TEST_NEVER` | 常に偽 | 常に破棄 (`discard`) |

---

## 5. レンダリング実装方針 (wgpu / WGSL)

1. **アルファテスト (Alpha Test / Cutout)**:
   - フラグメントシェーダー内で、`NiAlphaProperty` の設定値に基づいてテスト関数を評価。
   - 条件を満たさないピクセルは `discard;` を実行。
   - 深度バッファ書き込み (`depth_write_enabled: true`) のまま描画可能なため、金網やフェンス、植物の陰面消去が正確に動作する。

2. **アルファブレンド (Alpha Blend / Transparency)**:
   - 半透明メッシュ（ガラス等）は、不透明メッシュ描画後に深度書き込み無効 (`depth_write_enabled: false`) かつ適切なブレンドステートで描画。
   - `alpha_sort == true` の場合はカメラからの距離の降順（奥から手前）にソートして描画。
