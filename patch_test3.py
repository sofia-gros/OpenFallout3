import sys
content = open('crates/fo3_viewer/tests/new_game_simulation_test.rs', 'r', encoding='utf-8').read()

content = content.replace('''    let stage_name_prompt = vm.get_stage(cg00_id);
    println!("名前決定ステージ: {}", stage_name_prompt);
    assert_eq!(stage_name_prompt, 38, "名前入力準備 (Stage 38) に進行すること");

    // Stage 38 でタイマーを回して Stage 40 (GetPlayerName) へ
    for _ in 0..2 {''', '''    digest_pending(&mut vm);
    let stage_name_prompt = vm.get_stage(cg00_id);
    println!("名前決定ステージ: {}", stage_name_prompt);
    assert_eq!(stage_name_prompt, 38, "名前入力準備 (Stage 38) に進行すること");

    // Stage 38 でタイマーを回して Stage 40 (GetPlayerName) へ
    for _ in 0..2 {''')

open('crates/fo3_viewer/tests/new_game_simulation_test.rs', 'w', encoding='utf-8').write(content)
