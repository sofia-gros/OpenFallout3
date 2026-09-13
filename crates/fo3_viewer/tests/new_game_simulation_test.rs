//! 実機ニューゲームシミュレーションテスト (CG00: 出産・Vault 101 Infirmary シーケンス)
//!
//! 参照元: `knowledge/phase11_ai_package_and_quest_progression.md`,
//! `knowledge/new_game_and_quest_engine_architecture.md`, `Fallout3.esm:CG00`

use fo3_esm::types::FormId;
use fo3_esm::EsmReader;
use fo3_script::{EventDispatcher, GameEvent, ScriptVm};
use std::path::Path;

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
}
