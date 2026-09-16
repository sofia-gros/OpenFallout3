import sys
content = open('crates/fo3_script/src/event.rs', 'r', encoding='utf-8').read()

patch = r'''                      let active_quests: Vec<(FormId, FormId)> = vm
                          .quest_manager
                          .quests
                          .iter()
                          .filter_map(|(&q_id, quest)| {
                              if vm.quest_manager.current_stages.contains_key(&q_id) {
                                  quest.script_form_id.map(|s_id| (q_id, s_id))
                              } else {
                                  None
                              }
                          })
                          .collect();

                      println!("[DEBUG EVENT] GameMode active quests count: {}", active_quests.len());

                      for (q_id, script_id) in active_quests {
                          if let Some(blocks) = self.parsed_blocks.get(&script_id) {
                              let gm_blocks: Vec<ScriptBlock> = blocks
                                  .iter()
                                  .filter(|b| b.event_type == ScriptEventType::GameMode)
                                  .cloned()
                                  .collect();
                              println!("[DEBUG EVENT] gm_blocks for {:?}: {}", q_id, gm_blocks.len());
                              if !gm_blocks.is_empty() {
                                  vm.locals.clear();
                                  if let Some(q_vars) = vm.quest_manager.quest_variables.get(&q_id) {
                                      vm.locals = q_vars.clone();
                                  }
                                  for block in gm_blocks {
                                      let _ = vm.execute_block(&block.lines, Some(q_id));
                                  }
                                  // removed
                              }
                          } else {
                              println!("[DEBUG EVENT] No parsed blocks for {:?}", script_id);
                          }
                      }'''

content = content.replace(
'''                      let active_quests: Vec<(FormId, FormId)> = vm
                          .quest_manager
                          .quests
                          .iter()
                          .filter_map(|(&q_id, quest)| {
                              if vm.quest_manager.current_stages.contains_key(&q_id) {
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
                                  vm.locals.clear();
                                  if let Some(q_vars) = vm.quest_manager.quest_variables.get(&q_id) {
                                      vm.locals = q_vars.clone();
                                  }
                                  for block in gm_blocks {
                                      let _ = vm.execute_block(&block.lines, Some(q_id));
                                  }
                                  // removed
                              }
                          }
                      }''', patch)

open('crates/fo3_script/src/event.rs', 'w', encoding='utf-8').write(content)
