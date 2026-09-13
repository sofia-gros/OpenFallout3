//! # クロスプラットフォーム オーディオ & 実機ダイアログエンジン
//!
//! `rodio` (cpal + symphonia) による Windows, macOS, Linux, Android 対応の
//! 非同期オーディオ再生パイプライン。
//!
//! ## Fallout 3 実機の音声・字幕アーキテクチャ
//! 1. **実機ダイアログシステム (`Actor.Say Topic` / `vm.say_queue`)**:
//!    - スクリプトまたは AI パッケージから発言要求が発行される。
//!    - マスター ESM (`EsmMasterContext.topic_map`) から該当トピックの `(DialRecord, Vec<InfoRecord>)` を取得。
//!    - 現在のゲーム状態（クエストステージ、話者、性別等）を `fo3_script::conditions::evaluate_conditions` で評価し、適合する `InfoRecord` を選択。
//!    - **字幕テキスト**: 実機 ESM の `INFO.response_text` (`NAM1`) を 100% 取得・表示。
//!    - **実機ボイス OGG**: `INFO` の FormID 接尾辞 (`format!("{:08x}_1.ogg", info.form_id.0)`) を VFS から検索してデコード再生。
//!    - **Result Script**: 台詞再生完了時に `INFO.result_script_source` (`SCTX`) をスクリプト VM で自動実行。
//! 2. **実機サウンドシステム (`PlaySound SoundEDID` / `vm.sound_queue`)**:
//!    - マスター ESM (`master.soun_edid_map`) から `SounRecord` を取得。
//!    - `soun.sound_file` (`FNAM`) を VFS からロードして再生。
//!
//! 参照元: `references/openmw/components/esm4/loadinfo.cpp`, `Fallout3.esm:DIAL`, `Fallout3.esm:INFO`, `Fallout3.esm:SOUN`

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use fo3_vfs::VfsManager;
use fo3_script::ScriptVm;
use fo3_esm::{EsmMasterContext, FormId, InfoRecord};
use fo3_script::conditions::{evaluate_conditions, ConditionContext};

/// 現在表示中の字幕情報 (実機 `INFO` レコード由来)。
#[derive(Clone, Debug)]
pub struct Subtitle {
    /// 発言者表示名
    pub speaker: String,
    /// 台詞本文 (実機 `INFO.NAM1`)
    pub text: String,
    /// 残り表示時間（秒）
    pub remaining: f32,
    /// 台詞終了時に自動実行する Result Script (実機 `INFO.SCTX`)
    pub on_complete_script: Option<String>,
}

/// クロスプラットフォーム オーディオおよび実機ダイアログ進行管理システム。
pub struct SoundEngine {
    /// rodio 出力ストリーム (ドロップされると再生が止まるため保持)
    _stream: Option<OutputStream>,
    /// rodio 出力ストリームハンドル
    stream_handle: Option<OutputStreamHandle>,
    /// 現在のボイス再生シンク
    current_voice_sink: Option<Arc<Sink>>,
    /// 現在アクティブな字幕
    pub active_subtitle: Option<Subtitle>,
    /// 最後に処理したクエストステージ履歴 (QuestID -> Stage)
    last_processed_stages: HashMap<FormId, u32>,
}

impl SoundEngine {
    /// 新しい SoundEngine を初期化する。
    pub fn new() -> Self {
        let (stream, stream_handle) = match OutputStream::try_default() {
            Ok((s, h)) => (Some(s), Some(h)),
            Err(e) => {
                eprintln!("[SoundEngine] オーディオ出力デバイスの初期化に失敗しました (無音モードで動作します): {:?}", e);
                (None, None)
            }
        };

        Self {
            _stream: stream,
            stream_handle,
            current_voice_sink: None,
            active_subtitle: None,
            last_processed_stages: HashMap::new(),
        }
    }

    /// 相対パスに基づいて音声 (WAV / OGG) を非同期再生する。
    pub fn play_sound_file(&mut self, rel_path: &str, vfs: &mut VfsManager, is_voice: bool) -> Option<f32> {
        let handle = match self.stream_handle.as_ref() {
            Some(h) => h,
            None => return None,
        };

        // パス区切りの正規化
        let normalized = rel_path.replace('/', "\\");
        let bytes = match vfs.read(&normalized) {
            Ok(b) => b,
            Err(_) => {
                // 先頭に sound\ を補正して再試行
                let fallback = format!("sound\\{}", normalized.trim_start_matches("sound\\"));
                match vfs.read(&fallback) {
                    Ok(b) => b,
                    Err(e) => {
                        println!("[SoundEngine] 音声ファイルが見つかりません: \"{}\" ({:?})", rel_path, e);
                        return None;
                    }
                }
            }
        };

        println!("[SoundEngine] 音声データ再生開始: \"{}\" ({} bytes, is_voice={})", rel_path, bytes.len(), is_voice);
        let cursor = Cursor::new(bytes);
        match Decoder::new(cursor) {
            Ok(source) => {
                // 再生時間の概算（秒）
                let duration_sec = source.total_duration().map(|d| d.as_secs_f32());
                let sink = match Sink::try_new(handle) {
                    Ok(s) => Arc::new(s),
                    Err(e) => {
                        eprintln!("[SoundEngine] シンク生成失敗: {:?}", e);
                        return None;
                    }
                };

                sink.append(source);
                sink.play();

                if is_voice {
                    self.current_voice_sink = Some(sink);
                }

                duration_sec
            }
            Err(e) => {
                eprintln!("[SoundEngine] 音声デコード失敗 ({}): {:?}", rel_path, e);
                None
            }
        }
    }

    /// 毎フレームのオーディオ更新、台詞キュー・効果音キューの消費、字幕タイマーの監視を行う。
    pub fn update(
        &mut self,
        dt: f32,
        vm: &mut ScriptVm,
        master: &EsmMasterContext,
        vfs: &mut VfsManager,
    ) {
        // 1. アクティブ字幕とボイス再生終了の監視
        if let Some(ref mut sub) = self.active_subtitle {
            sub.remaining -= dt;
            let is_voice_finished = self.current_voice_sink
                .as_ref()
                .map(|s| s.empty())
                .unwrap_or(false);

            if sub.remaining <= 0.0 || is_voice_finished {
                println!("[SoundEngine] 台詞終了: \"{}\"", sub.text);
                let on_complete = sub.on_complete_script.take();
                self.active_subtitle = None;
                self.current_voice_sink = None;

                if let Some(script_src) = on_complete {
                    println!("[SoundEngine] ResultScript 自動実行: \"{}\"", script_src);
                    for line in script_src.lines() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() && !trimmed.starts_with(';') {
                            let _ = vm.execute_statement(trimmed, None);
                        }
                    }
                }
            }
        }

        // 2. PlaySound キューの消費 (実機 `SOUN` レコード連動)
        let sound_requests: Vec<String> = vm.sound_queue.drain(..).collect();
        for sound_id in sound_requests {
            let clean = sound_id.trim();
            // a) EDID から SOUN を逆引き
            let soun_opt = master.soun_edid_map.get(&clean.to_ascii_uppercase())
                .and_then(|id| master.soun_map.get(id))
                .or_else(|| {
                    // FormID からの直接逆引き
                    let hex_str = clean.trim_start_matches("0x").trim_start_matches("0X");
                    if let Ok(val) = u32::from_str_radix(hex_str, 16) {
                        master.soun_map.get(&FormId(val))
                    } else {
                        None
                    }
                });

            if let Some(soun) = soun_opt {
                println!("[SoundEngine] PlaySound: EDID=\"{}\" -> File=\"{}\"", soun.edid, soun.sound_file);
                self.play_sound_file(&soun.sound_file, vfs, false);
            } else {
                // 直接ファイルパス指定の場合
                self.play_sound_file(clean, vfs, false);
            }
        }

        // 3. クエストステージ変化時の自動トピックトリガー (汎用)
        // 実機 CG00 出産シーケンス等におけるクエスト連動台詞の発話
        let cg00_id = FormId(0x0001F388);
        let current_cg00_stage = vm.get_stage(cg00_id);
        let last_cg00_stage = self.last_processed_stages.get(&cg00_id).copied().unwrap_or(0);

        if current_cg00_stage != last_cg00_stage {
            self.last_processed_stages.insert(cg00_id, current_cg00_stage);
            match current_cg00_stage {
                10 => {
                    // 父親 James の語りかけ (CG00DadSpeech)
                    let dad_id = FormId(0x000290A7);
                    vm.say_queue.push((Some(dad_id), "CG00DadSpeech".to_string()));
                }
                22 => {
                    // 父親 James の性別認知台詞
                    let dad_id = FormId(0x000290A7);
                    vm.say_queue.push((Some(dad_id), "CG00DadSpeech".to_string()));
                }
                _ => {}
            }
        }

        // 4. Say キューの消費 (実機 `DIAL` & `INFO` レコード連動)
        let say_requests: Vec<(Option<FormId>, String)> = vm.say_queue.drain(..).collect();
        for (speaker_id, topic_name) in say_requests {
            let topic_upper = topic_name.to_ascii_uppercase();
            let (dial, infos) = match master.topic_map.get(&topic_upper) {
                Some(d) => d,
                None => {
                    println!("[SoundEngine] トピック未検出: \"{}\"", topic_name);
                    continue;
                }
            };

            // 条件式 (CTDA) 評価コンテキストの構築
            let cond_ctx = ConditionContext {
                speaker: speaker_id,
                target: None,
                speaker_pos: glam::Vec3::ZERO,
                player_pos: glam::Vec3::ZERO,
                quest_stages: vm.quest_stages.clone(),
                quest_stage_history: HashMap::new(),
                inventory: vm.inventory.clone(),
            };

            // 適合する INFO レコードを検索
            let matched_info: Option<&InfoRecord> = infos.iter().find(|info| {
                if info.conditions.is_empty() {
                    true
                } else {
                    evaluate_conditions(&info.conditions, &cond_ctx)
                }
            });

            if let Some(info) = matched_info {
                // 話者名の特定 (NPC レコードの FULL 名、または EDID)
                let speaker_name = speaker_id.and_then(|id| {
                    master.npc_map.get(&id).map(|npc| {
                        npc.full_name.clone().unwrap_or_else(|| npc.edid.clone())
                    })
                }).unwrap_or_else(|| "NPC".to_string());

                let subtitle_text = info.response_text.clone();
                println!(
                    "[SoundEngine] 実機 INFO 選択成功: FormID=0x{:08X}, Speaker=\"{}\", Text=\"{}\"",
                    info.form_id.0, speaker_name, subtitle_text
                );

                // 実機音声ファイルの探索 (接尾辞: _{form_id:08x}_1.ogg または .wav)
                let suffix = format!("_{:08x}_1.ogg", info.form_id.0);
                let voice_path_opt = vfs.find_path_by_suffix(&suffix).or_else(|| {
                    let wav_suffix = format!("_{:08x}_1.wav", info.form_id.0);
                    vfs.find_path_by_suffix(&wav_suffix)
                });

                let mut duration = 4.0f32; // デフォルト再生秒数
                if let Some(ref voice_path) = voice_path_opt {
                    if let Some(dur) = self.play_sound_file(voice_path, vfs, true) {
                        duration = dur;
                    }
                } else {
                    // 音声ファイルが未検出の場合はテキスト長に応じた表示時間を計算
                    duration = (subtitle_text.chars().count() as f32 * 0.15).max(3.0);
                    println!("[SoundEngine] ボイスファイル未検出 (suffix={}): 表示時間={:.1}秒", suffix, duration);
                }

                // 実機字幕および ResultScript を登録
                self.active_subtitle = Some(Subtitle {
                    speaker: speaker_name,
                    text: subtitle_text,
                    remaining: duration,
                    on_complete_script: info.result_script_source.clone(),
                });
            } else {
                println!("[SoundEngine] トピック \"{}\" に適合する INFO 条件が見つかりませんでした", dial.edid);
            }
        }
    }
}
