# Bink ムービーのゲームウィンドウ内再生パイプライン (BinkVideo)

Fallout 3 の Bink ムービー (`Video/*.bik`) は、Gamebryo 2.6 では**ゲームウィンドウのサーフェスに直接描画**される。
本プロジェクトでも「別ウィンドウ / 独立した全画面ウィンドウ」を一切生成せず、
ゲーム自体のウィンドウ設定（全画面かウィンドウか）に従って再生する。

## 従来実装の問題点 (修正前)

- `action.rs::play_bink_video()` が `ffplay -fs` を**別子プロセス**として `status()` でブロッキング実行していた。
- その結果、ムービーだけの独立ウィンドウが生成され、ゲームウィンドウの全画面/ウィンドウ設定に追従しない。

## 修正後アーキテクチャ

```
 ffmpeg 映像プロセス ──(rawvideo RGBA, 実時間ペース)──> BinkPlayer
   (bink_player.rs)                                         │ frame_buffer
                                                             ▼
                                        wgpu::Texture (Rgba8UnormSrgb, COPY_DST|TEXTURE_BINDING)
                                                             │ write_texture (queue)
                                                             ▼
                        create_video_bind_group (hud.rs) ──> バインドグループ
                                                             ▼
                        render_video_frame (hud.rs) ──> ゲームウィンドウ全面矩形描画

 ffmpeg 音声プロセス ──(s16le PCM, 実時間ペース)──> AudioPipeReader スレッド
   (bink_player.rs)                                        │ f32 サンプル列
                                                             ▼
                                                        PcmStreamSource
                                                             │ rodio Source::next()
                                                             ▼
                                                        rodio::Sink ──> OS オーディオ出力
```

### 1. `crates/fo3_viewer/src/bink_player.rs` — BinkPlayer

- `BinkPlayer::open(file_path, win_w, win_h, device, audio_handle)`:
  - **映像**: ffmpeg を子プロセス起動: `ffmpeg -re -loglevel quiet -i <file> -f rawvideo -pix_fmt rgba -vf scale=win_w:win_h pipe:1`
    - **`-re` 必須**: 入力をネイティブフレームレートで読み取り、rawvideo パイプ出力を実時間ペースにする。
      省略するとデコード速度で全フレームが即時出力され、ムービーが早送りになる。
  - **音声** (`audio_handle` がある場合): 別プロセス `ffmpeg -re -loglevel quiet -i <file> -vn -ac 2 -ar 44100 -f s16le pipe:1`
    を起動し、`AudioPipeReader` スレッドが stdout から s16le サンプルを f32 に変換して共有 `VecDeque` に積む。
    `PcmStreamSource` (rodio Source 実装) が sink からサンプルを消費する。
  - wgpu テクスチャ (`Rgba8UnormSrgb`, `TEXTURE_BINDING | COPY_DST`) とテクスチャビューを確保。
- `advance_frame(queue)`: stdout から 1 フレーム分 (`win_w * win_h * 4` バイト) をブロッキング読み取り、
  `queue.write_texture` でテクスチャへ転送。EOF で `finished = true` を返す。
- `Drop`: 音声プロセス → 音声スレッド (`join`) → 映像プロセスの順に `kill()` し、確実に終了する。
  Windows では `Child::drop` だけでは子プロセスが残るため、明示的な `kill()` が必須。

### 2. 音声パイプライン詳細

- **なぜ s16le で WAV ではないか**: rodio 0.20 の WAV デコーダ (hound 3.5.1) はストリーミング
  WAV の data チャンク長 `0xFFFFFFFF` を不正 (サンプルサイズの倍数でない) として拒否する。
 生 PCM (s16le) はヘッダ不要で、パッファ不要のストリーミングが可能。
- **同期**: 映像・音声ともに `-re` で実時間デコードするため、バッファは最小限で保たれ、
  動画と音声の同期が保たれる。
- **無音対応**: 音声トラックが存在しない `.bik` ファイルでは、ffmpeg 音声プロセスが即終了し、
  `PcmStreamSource` は直ちに `None` を返して sink が消滅する。

### 3. ムービーのスキップ (Esc / Space)

- `window_input.rs` で `state.bink_player.is_some()` 時に Escape / Space キー入力を検知し、
  `state.bink_player = None` を設定する。`BinkPlayer::Drop` が映像・音声プロセスを確実に終了する。
- Activate (E) の `bink_player.is_none()` ゲートとは独立して処理される。
  Escape はゲーム終了ではなくムービースキップに割り当てられ、ゲーム復帰する。

### 4. `crates/fo3_viewer/src/hud.rs` — 動画描画用 HUD エクステンション

- `bind_group_layout` を `HudRenderer` に保持し、外部テクスチャ用バインドグループを生成可能にする。
- `create_video_bind_group(device, texture_view)`: 動画テクスチャ + リニアサンプラー + uniform を
  HUD パイプラインのレイアウト (binding 0: uniform, 1: t_diffuse, 2: sampler) に結合。
- `render_video_frame(rpass, queue, video_bind_group, screen_w, screen_h)`:
  ウィンドウ全面の左右三角形 2 枚 (6 頂点) を描画。カラー uniform は白 (1,1,1,1) で
  テクスチャ色をそのまま出力する。

### 5. `crates/fo3_viewer/src/app.rs` — ViewerState 統合

- フィールド: `bink_player: Option<BinkPlayer>`, `bink_video_bind_group: Option<wgpu::BindGroup>`
- `update()` 冒頭 (section 0):
  1. `vm.play_bink_queue` を消化 → 相対パスは `data_dir/Video/` に解決
  2. `BinkPlayer::open(…, sound_engine.output_handle())` で映像+音声を開始
  3. `create_video_bind_group` でバインドグループをキャッシュ
  4. `advance_frame` で次フレーム取得。終了時は `bink_player` / `bink_video_bind_group` を `None` に
- **ムービー再生中はゲームワールドのシミュレーションを停止** (`if bink_player.is_some() { return }`)。
  旧 `play_bink_video` (ブロッキング ffplay) と同等の「動画再生中はゲーム進行を停止」を維持。
- `render()`: 3D シーン描画の後、スクリーンフェードの下に `render_video_frame` で全面描画。

### 6. オープニングムービー (Fallout INTRO Vsk.bik)

- `ViewerState::new()` の NewGame パスで直接 ffplay を起動するのをやめ、
  `state.vm.play_bink_queue` に絶対パスを積む。ゲームウィンドウ生成後の `update()` 内で
  インウィンドウ再生が開始される。

## 参照元

- Gamebryo 2.6 `BinkVideo` — ゲームウィンドウ内描画仕様
- Fallout 3 実機 `playBink "1 year later.bik"` — `Data/Video/` フォルダ基準
- ffmpeg `-re` の実時間入力読み取り仕様
- rodio 0.20 WAV デコーダ (hound 3.5.1) の data chunk 長検証 — ストリーミング不可の確認

## 実装上の注意点 (実機検証で判明)

### 1. HUD シェーダーとは別に動画専用パイプラインが必要

- HUD フラグメントシェーダーは Gamebryo UI の「アルファ焼き込み」仕様で
  **RGB を常に `u_hud.color.rgb` (白) 固定**で出力する。
- ffmpeg の `rgba` 出力は全ピクセル alpha=255 のため、このシェーダーで動画を描くと
  **画面全体が真っ白 (1,1,1)** になる。
- `HudRenderer` に `video_pipeline` を追加し、`fs_video_main` で
  `vec4(tex.rgb * u_hud.color.rgb, 1.0)` を出力する専用シェーダーで描画する。
- `render_video_frame()` は `video_pipeline` を使用する (bind_group_layout は共有)。

### 2. wgpu `SurfaceError::Outdated` の復帰処理

- `RedrawRequested` ハンドラで `Outdated` / `Lost` を受けたら、`window.inner_size()` の
  現在実サイズで `surface.configure` を再実行し `request_redraw` する。
- 放置するとスワップチェーンが古いままとなり、DX12 では毎フレーム失敗し続ける。

### 3. ムービー再生中の入力ゲート

- `update()` はゲートされるが、winit の `WindowEvent::KeyboardInput` は
  `update()` の外で処理される。Activate (E) 経路はゲームワールドを直接変更するため、
  `state.bink_player.is_none()` でガードする (再生中はワールド操作を禁止)。
- **スキップ (Esc / Space)** は `bink_player.is_some()` のとき最優先で処理され、
  `state.bink_player = None` で即座に Drop に流れる。

### 4. 音声ソースの無音 / EOF 遷移

- `PcmStreamSource::next()` はバッファ空で未 EOF の場合 `Some(0.0)` (無音) を返す。
  ffmpeg プロセスが起動から数ミリ秒のデコード遅延を持つため、最初数フレームは無音が混ざる。
- ffmpeg 終了後 (EOF) は `None` を返し、rodio sink が自動的に再生を停止する。