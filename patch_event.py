import sys
content = open('crates/fo3_script/src/event.rs', 'r', encoding='utf-8').read()

patch = r'''                    let active_quests: Vec<(FormId, FormId)> = vm
                        .quest_manager
                        .quests
                        .iter()
                        .filter_map(|(&q_id, quest)| {
                            if quest.active {
                                quest.script_id.map(|s| (q_id, s))
                            } else {
                                None
                            }
                        })
                        .collect();
                    println!("[EventDispatcher] GameMode: active_quests.len() = {}", active_quests.len());
                    for (q_id, script_id) in active_quests {'''

content = content.replace('                    let active_quests: Vec<(FormId, FormId)> = vm\n                        .quest_manager\n                        .quests\n                        .iter()\n                        .filter_map(|(&q_id, quest)| {\n                            if quest.active {\n                                quest.script_id.map(|s| (q_id, s))\n                            } else {\n                                None\n                            }\n                        })\n                        .collect();\n                    for (q_id, script_id) in active_quests {', patch)

open('crates/fo3_script/src/event.rs', 'w', encoding='utf-8').write(content)
