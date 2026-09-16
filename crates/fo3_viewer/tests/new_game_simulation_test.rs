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
#[path = "../src/chargen_menu.rs"]
mod chargen_menu;

fn digest_pending(vm: &mut fo3_script::ScriptVm) {
    while let Some((_quest_id, _stage, script)) = vm.pending_stage_scripts.pop_front() {
        if let Err(e) = vm.execute_result_script(&script, None) {
            println!("[TEST ERR] {}", e);
        }
    }
}

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
    let (dialogue_map, info_map) = reader.read_all_dialogues_map().expect("Failed to read dialogues");

    println!("マスター定義ロード完了:");
    println!("  - クエスト: {} 件", quest_map.len());
    println!("  - スクリプト: {} 件", script_map.len());
    println!("  - AI パッケージ: {} 件", pack_map.len());
    println!("  - 会話トピック: {} 件", dialogue_map.len());

    // 2. ScriptVm の初期化とマスター同期
    let mut vm = ScriptVm::new();
    for (_, q) in quest_map.clone() {
        vm.edid_map.insert(q.editor_id.to_ascii_uppercase(), q.form_id);
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
    digest_pending(&mut vm);


    
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
    let timer_val = vm.quest_manager.get_quest_variable(cg00_id, "timer");
    println!("CG00.timer: {:?}", timer_val);
    assert!(
        timer_val.is_some(),
        "Stage 5 の `set CG00.timer to .01` によりタイマー変数が記録されていること"
    );

    // 6. フレーム毎 GameMode イベントディスパッチ (タイマー減算とステージ進行)
    // 1フレーム目: timer = 0.01 - 0.016 = -0.006
    vm.delta_time = 0.016;
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();
    digest_pending(&mut vm);
    
    // 2フレーム目: timer <= 0 のため if getstage CG00 == 5 -> setstage CG00 6
    vm.delta_time = 0.016;
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();
    digest_pending(&mut vm);
    
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
    digest_pending(&mut vm);

    // 判定フレーム
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();
    digest_pending(&mut vm);

    let stage_after_timer = vm.get_stage(cg00_id);
    println!("タイマー満了後の CG00 ステージ: {}", stage_after_timer);
    assert_eq!(stage_after_timer, 8);

    // 8. Stage 8 の 2 秒タイマー経過シミュレーション (父親の台詞開始フェーズ)
    println!("Stage 8 の 2秒タイマー経過シミュレーション...");
    vm.delta_time = 2.5;
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();
    digest_pending(&mut vm);

    // 判定フレーム
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();
    digest_pending(&mut vm);

    let stage_after_timer2 = vm.get_stage(cg00_id);
    println!("Stage 8 満了後の CG00 ステージ: {}", stage_after_timer2);
    assert_eq!(stage_after_timer2, 9);

    // 9. Stage 9 の 7.5 秒タイマー経過シミュレーション (父親の観察・台詞フェーズ)
    println!("Stage 9 の 7.5秒タイマー経過シミュレーション...");
    vm.delta_time = 8.0;
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();
    digest_pending(&mut vm);

    // 判定フレーム
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();
    digest_pending(&mut vm);

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

    // 11. SoundEngine による台詞進行 (実機 ESM トピックと条件式による完全駆動)
    let mut sound_engine = audio::SoundEngine::new();
    let mut vfs = fo3_vfs::VfsManager::new();
    let mut master = fo3_esm::EsmMasterContext::new();
    master.topic_map = dialogue_map;
    master.info_map = info_map;

    // Stage 10: 父親の最初の台詞を発火 (doTalk == 1)
    sound_engine.update(0.016, &mut vm, &master, &mut vfs);
    assert!(
        (!sound_engine.active_subtitles.is_empty()),
        "Stage 10 で父親の台詞字幕が開始されていること"
    );
    let sub = sound_engine.active_subtitles.values().next().map(|(s, _)| s).unwrap();
    println!("表示中の台詞字幕: FormID=0x{:08X}, Speaker={}", sub.form_id.0, sub.speaker);
    assert_eq!(sub.form_id, FormId(0x0001F387));

    // 台詞終了 (6.0秒経過) -> ResultScript `setstage CG00 18` の自動実行
    sound_engine.update(6.0, &mut vm, &master, &mut vfs);
    let stage_after_speech = vm.get_stage(cg00_id);
    println!("台詞終了後の CG00 ステージ: {}", stage_after_speech);
    assert_eq!(stage_after_speech, 18, "父親の台詞完了により Stage 18 へ自動遷移すること");

    // Stage 18 の 1フレーム後 -> Stage 20 (性別選択メッセージ)
    for _ in 0..1 {
        vm.delta_time = 1.5;
        dispatcher.push_event(GameEvent::GameMode);
        dispatcher.process_queue(&mut vm).unwrap();
        digest_pending(&mut vm);
    }

    let stage_choose_sex = vm.get_stage(cg00_id);
    println!("性別選択フェーズの CG00 ステージ: {}", stage_choose_sex);
    assert_eq!(stage_choose_sex, 20, "タイマー満了で Stage 20 (性別選択) へ進むこと");

    // 性別選択: 1 (Female) を選択
    vm.player_is_female = true;
    vm.set_button_pressed(1);
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();
    digest_pending(&mut vm);

    // 実機 CG00SCRIPT: chooseSex == 1 により GetButtonPressed が処理され、
    // timer <= 0 で if getStage CG00 == 20 -> setstage CG00 22 が実行される
    vm.delta_time = 1.5;
    dispatcher.push_event(GameEvent::GameMode);
    dispatcher.process_queue(&mut vm).unwrap();
    digest_pending(&mut vm);

    let stage_after_select = vm.get_stage(cg00_id);
    println!("性別選択後の CG00 ステージ: {}", stage_after_select);
    assert_eq!(stage_after_select, 22, "性別選択により Stage 22 へ進むこと");

    // 12. Stage 22: 父親の性別認知台詞 (INFO 0x0001F385)
    sound_engine.update(0.016, &mut vm, &master, &mut vfs);
    assert!((!sound_engine.active_subtitles.is_empty()), "父親の台詞が開始されること");
    let dad_sub = sound_engine.active_subtitles.values().next().map(|(s, _)| s).unwrap();
    println!("父親の台詞1: FormID=0x{:08X}", dad_sub.form_id.0);
    assert_eq!(dad_sub.form_id, FormId(0x0001F385));

    // 父親の台詞終了 (15秒経過) -> ResultScript `set CG00DadREF.doTalk to 0; set CG00MomREF.doTalk to 1` が実行され、
    // 即座に自律トピックトリガーにより母親の台詞 (CG00MomSpeech, INFO 0x0005EDD8) が開始される
    sound_engine.update(15.0, &mut vm, &master, &mut vfs);
    sound_engine.update(0.016, &mut vm, &master, &mut vfs);
    assert!((!sound_engine.active_subtitles.is_empty()), "父親終了後に母親の台詞が自律開始されること");
    let mom_sub = sound_engine.active_subtitles.values().next().map(|(s, _)| s).unwrap();
    println!("母親の台詞: FormID=0x{:08X}", mom_sub.form_id.0);
    assert_eq!(mom_sub.form_id, FormId(0x0005EDD8));

    // 母親の台詞終了 (15秒経過) -> ResultScript `set CG00DadREF.doTalk to 1` が実行され、
    // 即座に自律トピックトリガーにより父親の台詞が開始される
    sound_engine.update(15.0, &mut vm, &master, &mut vfs);
    sound_engine.update(0.016, &mut vm, &master, &mut vfs);
    assert!((!sound_engine.active_subtitles.is_empty()), "母親終了後に父親の台詞が自律開始されること");

    // 父親が名前を促す Stage 38 に到達するまで台詞を自動進行
    for _ in 0..5 {
        if vm.get_stage(cg00_id) >= 38 {
            break;
        }
        sound_engine.update(0.016, &mut vm, &master, &mut vfs);
        if let Some(sub) = sound_engine.active_subtitles.values().next().map(|(s, _)| s) {
            println!("父親の台詞: FormID=0x{:08X}", sub.form_id.0);
        }
        sound_engine.update(15.0, &mut vm, &master, &mut vfs);
    }

    let stage_name_prompt = vm.get_stage(cg00_id);
    println!("名前入力準備ステージ: {}", stage_name_prompt);
    assert_eq!(stage_name_prompt, 38, "名前入力準備 (Stage 38) に進行すること");

    digest_pending(&mut vm);

    // Stage 38 でタイマーを回して Stage 40 (GetPlayerName) へ
    for _ in 0..2 {
        vm.delta_time = 1.5;
        dispatcher.push_event(GameEvent::GameMode);
        dispatcher.process_queue(&mut vm).unwrap();
        digest_pending(&mut vm);
    }

    let stage_name = vm.get_stage(cg00_id);
    println!("名前決定ステージ: {}", stage_name);
    assert_eq!(stage_name, 40, "タイマー満了で Stage 40 (GetPlayerName) へ進むこと");

    // 16. ChargenMenu による名前入力処理
    let mut chargen_menu = chargen_menu::ChargenMenu::new();
    chargen_menu.poll_events(&mut vm);
    assert!(chargen_menu.is_active(), "GetPlayerName により ChargenMenu がアクティブになること");

    // Enter キーで名前決定 -> Stage 42 へ自動進行
    chargen_menu.handle_key(winit::keyboard::KeyCode::Enter, &mut vm);
    let stage_after_name = vm.get_stage(cg00_id);
    println!("名前決定後のステージ: {}", stage_after_name);
    assert_eq!(stage_after_name, 42, "名前決定により Stage 42 へ進むこと");
}


