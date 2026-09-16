import sys
content = open('crates/fo3_viewer/tests/new_game_simulation_test.rs', 'r', encoding='utf-8').read()

patch_1 = r'''    // 父親の台詞終了 (15秒経過) -> ResultScript set CG00DadREF.doTalk to 0; set CG00MomREF.doTalk to 1 が実行され、
    // 即座に自律トピックトリガーにより母親の台詞 (CG00MomSpeech, INFO 0x0005EDD8) が開始される
    sound_engine.update(15.0, &mut vm, &master, &mut vfs);
    sound_engine.update(0.016, &mut vm, &master, &mut vfs);
    assert!((!sound_engine.active_subtitles.is_empty()), "父親終了後に母親の台詞が自律開始されること");'''

content = content.replace('    // 父親の台詞終了 (15秒経過) -> ResultScript set CG00DadREF.doTalk to 0; set CG00MomREF.doTalk to 1 が実行され、\n    // 即座に自律トピックトリガーにより母親の台詞 (CG00MomSpeech, INFO 0x0005EDD8) が開始される\n    sound_engine.update(15.0, &mut vm, &master, &mut vfs);\n    assert!((!sound_engine.active_subtitles.is_empty()), "父親終了後に母親の台詞が自律開始されること");', patch_1)

patch_2 = r'''    // 母親の台詞終了 (15秒経過) -> ResultScript set CG00DadREF.doTalk to 1 が実行され、
    // 即座に自律トピックトリガーにより父親の台詞が開始される
    sound_engine.update(15.0, &mut vm, &master, &mut vfs);
    sound_engine.update(0.016, &mut vm, &master, &mut vfs);
    assert!((!sound_engine.active_subtitles.is_empty()), "母親終了後に父親の台詞が自律開始されること");'''

content = content.replace('    // 母親の台詞終了 (15秒経過) -> ResultScript set CG00DadREF.doTalk to 1 が実行され、\n    // 即座に自律トピックトリガーにより父親の台詞が開始される\n    sound_engine.update(15.0, &mut vm, &master, &mut vfs);\n    assert!((!sound_engine.active_subtitles.is_empty()), "母親終了後に父親の台詞が自律開始されること");', patch_2)

open('crates/fo3_viewer/tests/new_game_simulation_test.rs', 'w', encoding='utf-8').write(content)
