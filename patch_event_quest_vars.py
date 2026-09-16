import sys
content = open('crates/fo3_script/src/event.rs', 'r', encoding='utf-8').read()

patch = r'''                    let active_quests: Vec<(FormId, FormId)> = vm
                        .quest_manager
                        .quests
                        .iter()
                        .filter_map(|(&q_id, quest)| {
                            let has_stage = vm.quest_manager.current_stages.contains_key(&q_id);
                            if has_stage {
                                quest.script_form_id.map(|s_id| (q_id, s_id))
                            } else {
                                None
                            }
                        })
                        .collect();

                    for (q_id, script_id) in active_quests {
                        if let Some(blocks) = self.parsed_blocks.get(&script_id) {
                            let gm_blocks: Vec<ScriptBlock> = blocks
                                .iter()
                                .filter(|b| b.event_type == ScriptEventType::GameMode)
                                .cloned()
                                .collect();
                            if !gm_blocks.is_empty() {
                                // Load quest variables into vm.locals
                                vm.locals.clear();
                                if let Some(q_vars) = vm.quest_manager.quest_variables.get(&q_id) {
                                    vm.locals = q_vars.clone();
                                }
                                for block in gm_blocks {
                                    let _ = vm.execute_block(&block.lines, Some(q_id));
                                }
                                // Save quest variables back
                                vm.quest_manager.quest_variables.insert(q_id, vm.locals.clone());
                            }
                        }
                    }'''

start_idx = content.find('                    let active_quests: Vec<(FormId, FormId)> = vm')
end_idx = content.find('                _ => {', start_idx)
if end_idx == -1:
    end_idx = content.find('                GameEvent::OnAdd', start_idx)

if start_idx != -1 and end_idx != -1:
    new_content = content[:start_idx] + patch + '\n' + content[end_idx:]
    open('crates/fo3_script/src/event.rs', 'w', encoding='utf-8').write(new_content)
    print("Replaced quest loop in event.rs")
else:
    print("Could not find quest loop in event.rs")
