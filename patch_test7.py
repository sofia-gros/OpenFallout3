import sys
content = open('crates/fo3_viewer/tests/new_game_simulation_test.rs', 'r', encoding='utf-8').read()

patch_1 = r'''    sound_engine.update(15.0, &mut vm, &master, &mut vfs);
    sound_engine.update(0.016, &mut vm, &master, &mut vfs);
    assert!((!sound_engine.active_subtitles.is_empty()), "父親終了後に母親の台詞が自律開始されること");'''

content = content.replace('    sound_engine.update(15.0, &mut vm, &master, &mut vfs);\n    assert!((!sound_engine.active_subtitles.is_empty()), "父親終了後に母親の台詞が自律開始されること");', patch_1)

patch_2 = r'''    sound_engine.update(15.0, &mut vm, &master, &mut vfs);
    sound_engine.update(0.016, &mut vm, &master, &mut vfs);
    assert!((!sound_engine.active_subtitles.is_empty()), "母親終了後に父親の台詞が自律開始されること");'''

content = content.replace('    sound_engine.update(15.0, &mut vm, &master, &mut vfs);\n    assert!((!sound_engine.active_subtitles.is_empty()), "母親終了後に父親の台詞が自律開始されること");', patch_2)

open('crates/fo3_viewer/tests/new_game_simulation_test.rs', 'w', encoding='utf-8').write(content)
