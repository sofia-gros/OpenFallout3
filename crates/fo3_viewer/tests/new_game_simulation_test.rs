//! 実機ニューゲームシミュレーションテスト (CG00: 出産・Vault 101 Infirmary シーケンス)
//!
//! 参照元: `knowledge/phase11_ai_package_and_quest_progression.md`,
//! `knowledge/new_game_and_quest_engine_architecture.md`, `Fallout3.esm:CG00`

use fo3_esm::types::FormId;
use fo3_esm::EsmReader;
use fo3_script::{EventDispatcher, GameEvent, ScriptVm};
use std::path::Path;

#[path = "../src/audio.rs"]
mod audio;

#[test]
fn test_new_game_cg00_progression_simulation() {
    let esm_path = "A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\\Fallout3.esm";
    if !Path::new(esm_path).exists() {
        eprintln!("Fallout3.esm が存在しないためテストをスキップします");
        return;
    }

    // 1. ESM リーダーからマスター定義（クエスト 192 件、AI パッケージ 3,266 件、スクリプト）を一括ロード
    let mut reader = EsmReader::open(esm_path).expect("Failed to open Fallout3.esm");
    let quest_map = reader.read_all_quests_map().expect("Failed to read quest map");
    let script_map = reader.read_all_scripts_map().expect("Failed to read script map");
    let pack_map = reader.read_all_packages_map().expect("Failed to read pack map");

    println!("マスター定義ロード完了:");
    println!("  - クエスト: {} 件", quest_map.len());
    println!("  - スクリプト: {} 件", script_map.len());
    println!("  - AI パッケージ: {} 件", pack_map.len());

    // 2. ScriptVm の初期化とマスター同期
    let mut vm = ScriptVm::new();
    for (_, q) in quest_map {
        vm.quest_manager.register_quest(q);
    }
    for (id, scpt) in &script_map {
        vm.scripts.insert(*id, scpt.clone());
        if !scpt.edid.is_empty() {
            vm.edid_map.insert(scpt.edid.to_ascii_uppercase(), *id);
        }
    }
    for (id, pack) in &pack_map {
        if let Some(ref edid) = pack.editor_id {
            vm.edid_map.insert(edid.to_ascii_uppercase(), *id);
        }
    }

    // 3. EventDispatcher の初期化
    let mut dispatcher = EventDispatcher::new();
    dispatcher.register_all_scripts(&script_map);

    // 4. ニューゲーム開始: CG00 (FormID: 0x0001F388) を Stage 0 でキック
    let cg00_id = FormId(0x0001F388);
    println!("ニューゲーム開始: CG00 (Stage 0) 起動...");
    vm.set_stage(cg00_id, 0);

    // 5. 連鎖実行の検証:
    // Stage 0 スクリプト: `setstage CG00 5`, `player.moveto CG00PlayerStartMarker`
    // Stage 5 スクリプト: `set CG00.timer to .01`, `set CG00.runTimer to 1`, `disableplayercontrols`, `imod CG00BlackScreenISFX`
    let stage = vm.get_stage(cg00_id);
    println!("CG00 現在ステージ: {}", stage);
    assert_eq!(stage, 5, "Stage 0 のスクリプト実行により Stage 5 に自動連鎖遷移すること");

    // テレポート要求 (player.moveto CG00PlayerStartMarker) が発行されていること
    assert!(
        vm.teleport_requests.iter().any(|(_, marker)| marker == "CG00PlayerStartMarker"),
        "Stage 0 の player.moveto CG00PlayerStartMarker によりテレポート要求が発行されていること"
    );

    // プレイヤー操作が無効化されていること
    assert!(
        !vm.player_controls_enabled,
        "Stage 5 の disableplayercontrols によりプレイヤー操作が無効化されていること"
    );

    // 暗転画面エフェクトが適用されていること
    let has_black_screen = vm
        .active_imods
        .iter()
        .any(|imod| imod.eq_ignore_ascii_case("CG00BlackScreenISFX"));
    assert!(
        has_black_screen,
        "Stage 5 の imod により CG00BlackScreenISFX が適用されていること"
    );

    // タイマー変数がセットされていること
    let timer_val = vm.locals.get("cg00.timer").or_else(|| vm.locals.get("timer")).or_else(|| vm.globals.get("CG00.timer"));
    println!("CG00.timer: {:?}", timer_val);
    assert!(
        timer_val.is_some(),
        "Stage 5 の `set CG00.timer to .01` によりタイマー変数が記録されていること"
    );

    // 6. フレーム毎 GameMode イベントディスパッチ (タイマー減算とステージ進行)
    // 1フレーム目: timer = 0.01 - 0.016 = -0.006
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();

    // 2フレーム目: timer <= 0 のため if getstage CG00 == 5 -> setstage CG00 6
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();

    let new_stage = vm.get_stage(cg00_id);
    println!("GameMode 駆動後の CG00 ステージ: {}", new_stage);
    assert_eq!(
        new_stage, 6,
        "GameMode によるタイマー進行で CG00 が自動的に Stage 6 へ進むこと"
    );

    // 7. Stage 6 の 10 秒タイマー経過シミュレーション (産声終了・目覚め・視界演出)
    println!("10秒経過シミュレーション (産声終了・視界開放)...");
    vm.delta_time = 10.5;
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();

    // 判定フレーム
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();

    let stage_after_timer = vm.get_stage(cg00_id);
    println!("タイマー満了後の CG00 ステージ: {}", stage_after_timer);
    assert_eq!(stage_after_timer, 8);

    // 8. Stage 8 の 2 秒タイマー経過シミュレーション (父親の台詞開始フェーズ)
    println!("Stage 8 の 2秒タイマー経過シミュレーション...");
    vm.delta_time = 2.5;
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();

    // 判定フレーム
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();

    let stage_after_timer2 = vm.get_stage(cg00_id);
    println!("Stage 8 満了後の CG00 ステージ: {}", stage_after_timer2);
    assert_eq!(stage_after_timer2, 9);

    // 9. Stage 9 の 7.5 秒タイマー経過シミュレーション (父親の観察・台詞フェーズ)
    println!("Stage 9 の 7.5秒タイマー経過シミュレーション...");
    vm.delta_time = 8.0;
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();

    // 判定フレーム
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();

    let stage_after_timer3 = vm.get_stage(cg00_id);
    println!("Stage 9 満了後の CG00 ステージ: {}", stage_after_timer3);
    assert_eq!(stage_after_timer3, 10, "Stage 9 満了後に自動的に Stage 10 (父親の台詞開始) へ進むこと");

    // 10. サウンド再生キューの検証 (QSTBirthStart, QSTBabyCry)
    println!("サウンド再生キュー: {:?}", vm.sound_queue);
    assert!(
        vm.sound_queue.iter().any(|s| s.eq_ignore_ascii_case("QSTBirthStart")),
        "Stage 6 で QSTBirthStart サウンドが要求されていること"
    );
    assert!(
        vm.sound_queue.iter().any(|s| s.eq_ignore_ascii_case("QSTBabyCry")),
        "Stage 8 で QSTBabyCry (産声) サウンドが要求されていること"
    );

    // 11. SoundEngine による台詞進行 (Stage 10 -> 父親の台詞 -> Stage 18 -> 性別選択 -> Stage 22)
    let mut sound_engine = audio::SoundEngine::new();
    let mut vfs = fo3_vfs::VfsManager::new();
    let mut master = fo3_esm::EsmMasterContext::new();

    // テスト用のトピックと INFO レコードを master に設定 (実機 Fallout3.esm 構造)
    let dad_info_10 = fo3_esm::InfoRecord {
        form_id: fo3_esm::FormId(0x0001F387),
        topic_id: None,
        response_text: "Let's see... Are you a boy or a girl?".to_string(),
        actor_notes: None,
        flags: 0,
        speaker_npc: Some(fo3_esm::FormId(0x000290A7)),
        conditions: vec![],
        response_data: None,
        speech_challenge: None,
        prompt_override: None,
        choices: vec![],
        result_script_source: Some("setstage CG00 18".to_string()),
        result_script_bytecode: None,
        unknown_subrecords: vec![],
    };
    let dad_dial = fo3_esm::DialRecord {
        form_id: fo3_esm::FormId(0x0001F388),
        edid: "CG00DadSpeech".to_string(),
        prompt: None,
        dial_type: 0,
        dial_flags: 0,
        priority: 50.0,
        quests: vec![],
        info_ids: vec![],
        script_id: None,
        unknown_subrecords: vec![],
    };
    master.topic_map.insert("CG00DADSPEECH".to_string(), (dad_dial, vec![dad_info_10]));

    // SoundEngine を駆動して Stage 10 の父親の台詞を発火
    sound_engine.update(0.016, &mut vm, &master, &mut vfs);
    assert!(
        sound_engine.active_subtitle.is_some(),
        "Stage 10 で父親の台詞字幕が開始されていること"
    );
    let sub = sound_engine.active_subtitle.as_ref().unwrap();
    println!("表示中の台詞字幕: \"{}\" ({})", sub.text, sub.speaker);
    assert!(sub.text.contains("Are you a boy or a girl"));

    // 台詞終了 (6.0秒経過) -> ResultScript `setstage CG00 18` の自動実行
    sound_engine.update(6.0, &mut vm, &master, &mut vfs);
    let stage_after_speech = vm.get_stage(cg00_id);
    println!("台詞終了後の CG00 ステージ: {}", stage_after_speech);
    assert_eq!(stage_after_speech, 18, "父親の台詞完了により Stage 18 へ自動遷移すること");

    // Stage 18 の 1秒タイマー経過 -> Stage 20 (性別選択メニュー表示)
    vm.delta_time = 1.5;
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();

    let stage_choose_sex = vm.get_stage(cg00_id);
    println!("性別選択フェーズの CG00 ステージ: {}", stage_choose_sex);
    assert_eq!(stage_choose_sex, 20, "タイマー満了で Stage 20 (性別選択) へ進むこと");

    // 性別選択: 実機 GetButtonPressed 仕様 (0: Male)
    vm.set_button_pressed(0);
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();

    // 実機 CG00SCRIPT: chooseSex == 1 により GetButtonPressed が処理され、
    // timer <= 0 で if getStage CG00 == 20 -> setstage CG00 22 が実行される
    vm.delta_time = 1.5;
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();

    let stage_after_select = vm.get_stage(cg00_id);
    println!("性別選択後の CG00 ステージ: {}", stage_after_select);
    assert_eq!(stage_after_select, 22, "性別選択により Stage 22 (父親・母親のリアクション) へ進むこと");
}

