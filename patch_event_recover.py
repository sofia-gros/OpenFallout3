import sys
content = open('crates/fo3_script/src/event.rs', 'r', encoding='utf-8').read()

# Add OnAnimationEnd enum
if 'OnAnimationEnd' not in content:
    content = content.replace('killer: FormId,\n    },', 'killer: FormId,\n    },\n    /// アニメーション終了イベント\n    OnAnimationEnd {\n        actor: FormId,\n    },')

# Add OnAnimationEnd match
match_patch = '''                GameEvent::OnAnimationEnd { actor } => {
                    if let Some(instance) = self.instances.get(&actor) {
                        let script_id = instance.script_form_id;
                        if let Some(blocks) = self.parsed_blocks.get(&script_id) {
                            let target_blocks: Vec<ScriptBlock> = blocks
                                .iter()
                                .filter(|b| b.event_type == ScriptEventType::Custom("OnAnimationEnd".to_string()))
                                .cloned()
                                .collect();
                            vm.locals = instance.local_vars.clone();
                            for block in target_blocks {
                                let _ = vm.execute_block(&block.lines, Some(actor));
                            }
                            if let Some(inst) = self.instances.get_mut(&actor) {
                                inst.local_vars = vm.locals.clone();
                            }
                        }
                    }
                }
            }
        }
        Ok(())'''
if 'GameEvent::OnAnimationEnd' not in content:
    content = content.replace('''            }
        }
        Ok(())''', match_patch)

# Fix GameMode logic
gm_patch = '''                      let active_quests: Vec<(FormId, FormId)> = vm
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
                              }
                          }
                      }'''
import re
content = re.sub(r'let active_quests: Vec<\(FormId, FormId\)>.*?// removed\n                              }\n                          }\n                      }', gm_patch, content, flags=re.DOTALL)
content = re.sub(r'let active_quests: Vec<\(FormId, FormId\)>.*?let _ = vm\.execute_block\(&block\.lines, Some\(q_id\)\);\n                                }\n                            }\n                        }\n                    }', gm_patch, content, flags=re.DOTALL)

open('crates/fo3_script/src/event.rs', 'w', encoding='utf-8').write(content)
