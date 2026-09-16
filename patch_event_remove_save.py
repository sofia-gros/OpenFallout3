import sys
content = open('crates/fo3_script/src/event.rs', 'r', encoding='utf-8').read()
content = content.replace('vm.quest_manager.quest_variables.insert(q_id, vm.locals.clone());', '// removed')
open('crates/fo3_script/src/event.rs', 'w', encoding='utf-8').write(content)
