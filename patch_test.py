import sys
content = open('crates/fo3_viewer/tests/new_game_simulation_test.rs', 'r', encoding='utf-8').read()

content = content.replace('''    for (_, q) in quest_map {
        vm.quest_manager.register_quest(q);
    }''', '''    for (_, q) in quest_map.clone() {
        vm.edid_map.insert(q.editor_id.to_ascii_uppercase(), q.form_id);
        vm.quest_manager.register_quest(q);
    }''')

open('crates/fo3_viewer/tests/new_game_simulation_test.rs', 'w', encoding='utf-8').write(content)
