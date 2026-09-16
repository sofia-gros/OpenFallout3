import sys
content = open('crates/fo3_viewer/tests/new_game_simulation_test.rs', 'r', encoding='utf-8').read()

content = content.replace('    vm.locals.insert("cg00.button".to_string(), 0.0);\n', '    vm.locals.insert("cg00.button".to_string(), 0.0);\n    vm.chargen_menu_active = false;\n')

open('crates/fo3_viewer/tests/new_game_simulation_test.rs', 'w', encoding='utf-8').write(content)
