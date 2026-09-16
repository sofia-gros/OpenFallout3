import sys
content = open('crates/fo3_viewer/tests/new_game_simulation_test.rs', 'r', encoding='utf-8').read()

patch = r'''    // Stage 38 の 1フレーム後 -> Stage 40 (名前入力)
    for _ in 0..5 {
        vm.delta_time = 1.5;
        dispatcher.push_event(GameEvent::GameMode);
        dispatcher.process_queue(&mut vm).unwrap();
        digest_pending(&mut vm);
    }'''

content = content.replace('    // Stage 38 の 1フレーム後 -> Stage 40 (名前入力)\n    vm.delta_time = 1.5;\n    dispatcher.push_event(GameEvent::GameMode);\n    dispatcher.process_queue(&mut vm).unwrap();\n    digest_pending(&mut vm);\n    dispatcher.push_event(GameEvent::GameMode);\n    dispatcher.process_queue(&mut vm).unwrap();\n    digest_pending(&mut vm);', patch)

patch_2 = r'''    // Stage 18 の 1フレーム後 -> Stage 20 (性別選択メッセージ)
    for _ in 0..5 {
        vm.delta_time = 1.5;
        dispatcher.push_event(GameEvent::GameMode);
        dispatcher.process_queue(&mut vm).unwrap();
        digest_pending(&mut vm);
    }'''

content = content.replace('    // Stage 18 の 1フレーム後 -> Stage 20 (性別選択メッセージ)\n    vm.delta_time = 1.5;\n    dispatcher.push_event(GameEvent::GameMode);\n    dispatcher.process_queue(&mut vm).unwrap();\n    digest_pending(&mut vm);\n    dispatcher.push_event(GameEvent::GameMode);\n    dispatcher.process_queue(&mut vm).unwrap();\n    digest_pending(&mut vm);', patch_2)

open('crates/fo3_viewer/tests/new_game_simulation_test.rs', 'w', encoding='utf-8').write(content)
