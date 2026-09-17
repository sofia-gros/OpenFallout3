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

use fo3_esm::{EsmMasterContext, FormId, InfoRecord};
use fo3_script::conditions::{evaluate_conditions, ConditionContext};
use fo3_script::ScriptVm;
use fo3_vfs::VfsManager;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::sync::Arc;

/// 現在表示中の字幕情報 (実機 `INFO` レコード由来)。
#[derive(Clone, Debug)]
pub struct Subtitle {
    /// 発言 INFO の FormID
    pub form_id: FormId,
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
    /// Active subtitles and their optional voice sinks per actor
    pub active_subtitles: HashMap<FormId, (Subtitle, Option<Arc<Sink>>)>,
    /// Last processed quest stages (QuestID -> Stage)
    last_processed_stages: HashMap<FormId, u32>,
    /// 発話済み INFO レコードの FormID (実機 Say Once フラグ対応)
    pub spoken_infos: HashSet<FormId>,
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
            active_subtitles: HashMap::new(),
            last_processed_stages: HashMap::new(),
            spoken_infos: HashSet::new(),
        }
    }

    /// rodio 出力ストリームハンドルを複製して返す。
    ///
    /// Bink ムービー音声 (s16le パイプ → rodio) のシンク生成に使う。
    /// ストリーム本体 (`OutputStream`) は SoundEngine が保持し続ける必要がある。
    pub fn output_handle(&self) -> Option<OutputStreamHandle> {
        self.stream_handle.clone()
    }

    /// 相対パスに基づいて音声 (WAV / OGG) を非同期再生する。
    pub fn play_sound_file(
        &mut self,
        rel_path: &str,
        vfs: &mut VfsManager,
        is_voice: bool,
    ) -> Option<(Option<f32>, Arc<Sink>)> {
        let handle = match self.stream_handle.as_ref() {
            Some(h) => h,
            None => return None,
        };

        // パス区切りの正規化
        let normalized = rel_path.replace('/', "\\");
        let (bytes, resolved_path) = match vfs.read(&normalized) {
            Ok(b) => (b, normalized.clone()),
            Err(_) => {
                // 先頭に sound\ を補正して再試行
                let fallback = format!("sound\\{}", normalized.trim_start_matches("sound\\"));
                match vfs.read(&fallback) {
                    Ok(b) => (b, fallback),
                    Err(_) => {
                        // ディレクトリパスや部分プレフィックスから音声ファイルを探索
                        let candidates = [fallback.clone(), normalized.clone()];
                        let mut found = None;
                        for cand in &candidates {
                            if let Some(matched) = vfs.find_path_by_prefix(cand) {
                                if let Ok(b) = vfs.read(&matched) {
                                    found = Some((b, matched));
                                    break;
                                }
                            }
                        }
                        match found {
                            Some(res) => res,
                            None => {
                                println!(
                                    "[SoundEngine] 音声ファイルが見つかりません: \"{}\"",
                                    rel_path
                                );
                                return None;
                            }
                        }
                    }
                }
            }
        };

        println!(
            "[SoundEngine] 音声データ再生開始: \"{}\" ({} bytes, is_voice={})",
            resolved_path,
            bytes.len(),
            is_voice
        );
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
                Some((duration_sec, sink))
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
        // 台詞終了フレーム記録: ResultScript の setstage は遅延キュー (pending_stage_scripts) へ
        // 積まれ次フレームの app.update で適用されるため、終了と同一フレームで doTalk の
        // 再評価・再 push を行うと性別選択前に次の行が再生されてしまう。
        // 終了フレームは section 3 の自律トリガーを 1 フレーム休止させる。
        let mut line_ended_this_frame = false;

        // 1. Process active subtitles (per-actor)
        let mut finished_speakers = Vec::new();
        for (speaker_id, (sub, sink_opt)) in self.active_subtitles.iter_mut() {
            sub.remaining -= dt;
            let is_voice_finished = sink_opt.as_ref().map(|s| s.empty()).unwrap_or(false);

            if sub.remaining <= 0.0 || is_voice_finished {
                println!(
                    "[SoundEngine] Subtitle finished: FormID=0x{:08X}",
                    sub.form_id.0
                );
                if let Some(script_src) = sub.on_complete_script.take() {
                    println!("[SoundEngine] Running ResultScript:\n{}", script_src);
                    let _ = vm.execute_result_script(&script_src, None);
                }
                finished_speakers.push(*speaker_id);
                line_ended_this_frame = true;
            }
        }
        for speaker_id in finished_speakers {
            self.active_subtitles.remove(&speaker_id);
        }

        // 2. PlaySound キューの消費 (実機 `SOUN` レコード連動)
        let sound_requests: Vec<String> = vm.sound_queue.drain(..).collect();
        for sound_id in sound_requests {
            let clean = sound_id.trim();
            // a) EDID から SOUN を逆引き
            let soun_opt = master
                .soun_edid_map
                .get(&clean.to_ascii_uppercase())
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
                println!(
                    "[SoundEngine] PlaySound: EDID=\"{}\" -> File=\"{}\"",
                    soun.edid, soun.sound_file
                );
                let _ = self.play_sound_file(&soun.sound_file, vfs, false);
            } else {
                // 直接ファイルパス指定の場合
                let _ = self.play_sound_file(clean, vfs, false);
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
                is_female: vm.player_is_female,
            };

            // 適合する INFO レコードを検索 (未読のものを優先し、Say Once は完全除外)
            let matched_info: Option<&InfoRecord> = infos
                .iter()
                .find(|info| {
                    let is_say_once = (info.flags & 0x0004) != 0;
                    if is_say_once && self.spoken_infos.contains(&info.form_id) {
                        return false;
                    }
                    if self.spoken_infos.contains(&info.form_id) {
                        return false;
                    }
                    if info.conditions.is_empty() {
                        true
                    } else {
                        evaluate_conditions(&info.conditions, &cond_ctx)
                    }
                })
                .or_else(|| {
                    // 未読がない場合、Say Once でない候補から再探索
                    // ただし既読 INFO (spoken_infos) は厳格に除外する:
                    // doTalk はラッチ保持されるため、未読行が尽きたトピックで同じ行を
                    // 再選択し続ける無限ループを防ぎ、下位の doTalk フラグ解除処理へ到達させる。
                    infos.iter().find(|info| {
                        if self.spoken_infos.contains(&info.form_id) {
                            return false;
                        }
                        if info.conditions.is_empty() {
                            true
                        } else {
                            evaluate_conditions(&info.conditions, &cond_ctx)
                        }
                    })
                });

            if let Some(info) = matched_info {
                self.spoken_infos.insert(info.form_id);

                // 話者名の特定 (NPC レコードの FULL 名、または EDID)
                let speaker_name = speaker_id
                    .and_then(|id| {
                        master
                            .npc_map
                            .get(&id)
                            .map(|npc| npc.full_name.clone().unwrap_or_else(|| npc.edid.clone()))
                    })
                    .unwrap_or_else(|| "NPC".to_string());

                let subtitle_text = info.response_text.clone();
                println!(
                    "[SoundEngine] 実機 INFO 選択成功: FormID=0x{:08X}, Speaker=\"{}\"",
                    info.form_id.0, speaker_name
                );

                // 実機音声ファイルの探索 (接尾辞: _{form_id:08x}_1.ogg または .wav)
                let suffix = format!("_{:08x}_1.ogg", info.form_id.0);
                let voice_path_opt = vfs.find_path_by_suffix(&suffix).or_else(|| {
                    let wav_suffix = format!("_{:08x}_1.wav", info.form_id.0);
                    vfs.find_path_by_suffix(&wav_suffix)
                });

                let mut sink_opt = None;
                let mut duration = 4.0f32; // Default for text-only
                if let Some(ref voice_path) = voice_path_opt {
                    if let Some((_, s)) = self.play_sound_file(voice_path, vfs, true) {
                        duration = f32::INFINITY;
                        sink_opt = Some(s);
                    }
                } else {
                    // Fallback to text length
                    duration = (subtitle_text.chars().count() as f32 * 0.15).max(3.0);
                    println!(
                        "[SoundEngine] Voice file missing (suffix={}): duration={:.1}s",
                        suffix, duration
                    );
                }

                // Register Subtitle
                self.active_subtitles.insert(
                    speaker_id.unwrap_or(FormId(0)),
                    (
                        Subtitle {
                            form_id: info.form_id,
                            speaker: speaker_name,
                            text: subtitle_text,
                            remaining: duration,
                            on_complete_script: info.result_script_source.clone(),
                        },
                        sink_opt,
                    ),
                );
            } else {
                println!(
                    "[SoundEngine] トピック \"{}\" に適合する INFO 条件が見つかりませんでした",
                    dial.edid
                );
                // これ以上話す台詞がないため、該当アクターの doTalk フラグをクリア
                let edid_lower = dial.edid.to_ascii_lowercase();
                if edid_lower.contains("dad") {
                    vm.globals.insert("cg00dadref.dotalk".to_string(), 0.0);
                } else if edid_lower.contains("mom") {
                    vm.globals.insert("cg00momref.dotalk".to_string(), 0.0);
                } else if edid_lower.contains("doctorli") || edid_lower.contains("drli") {
                    vm.globals.insert("cg00doctorliref.dotalk".to_string(), 0.0);
                }
            }
        }
    }
}
