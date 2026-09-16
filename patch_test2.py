import sys
content = open('crates/fo3_viewer/tests/new_game_simulation_test.rs', 'r', encoding='utf-8').read()

content = content.replace('''    let timer_val = vm.locals.get("cg00.timer").or_else(|| vm.locals.get("timer")).or_else(|| vm.globals.get("CG00.timer"));''', '''    let timer_val = vm.quest_manager.get_quest_variable(cg00_id, "timer");''')

open('crates/fo3_viewer/tests/new_game_simulation_test.rs', 'w', encoding='utf-8').write(content)
