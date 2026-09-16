import sys
content = open('crates/fo3_script/src/event.rs', 'r', encoding='utf-8').read()

# 1. f32 -> f64
content = content.replace('HashMap<String, f32>', 'HashMap<String, f64>')

# 2. Add OnAnimationEnd to GameEvent
event_patch = '''    /// アクターの死亡
    /// 参照元: GECK Wiki Begin OnDeath [KillerRef]
    OnDeath {
        /// 死亡したアクターの FormID
        actor: FormId,
        /// 殺害者の FormID (自殺・環境死の場合は FormId(0))
        killer: FormId,
    },
    /// アニメーション終了イベント (カスタム)
    /// 参照元: eferences/openmw/apps/openmw/mwlua/engineevents.hpp:63
    OnAnimationEnd {
        /// 対象アクター
        actor: FormId,
    },
}'''
content = content.replace('''    /// アクターの死亡
    /// 参照元: GECK Wiki Begin OnDeath [KillerRef]
    OnDeath {
        /// 死亡したアクターの FormID
        actor: FormId,
        /// 殺害者の FormID (自殺・環境死の場合は FormId(0))
        killer: FormId,
    },
}''', event_patch)

# 3. Add OnAnimationEnd match arm
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
content = content.replace('''            }
        }
        Ok(())''', match_patch)

# 4. Quest Variables loading logic in GameMode
quest_loop_orig = '''                    for (q_id, script_id) in active_quests {
                        if let Some(blocks) = self.parsed_blocks.get(&script_id) {
                            for block in blocks {
                                if block.event_type == ScriptEventType::GameMode {
                                    let _ = vm.execute_block(&block.lines, Some(q_id));
                                }
                            }
                        }
                    }'''
quest_loop_new = '''                    for (q_id, script_id) in active_quests {
                        if let Some(blocks) = self.parsed_blocks.get(&script_id) {
                            let gm_blocks: Vec<ScriptBlock> = blocks.iter()
                                .filter(|b| b.event_type == ScriptEventType::GameMode)
                                .cloned().collect();
                            if !gm_blocks.is_empty() {
                                vm.locals.clear();
                                if let Some(q_vars) = vm.quest_manager.quest_variables.get(&q_id) {
                                    vm.locals = q_vars.clone();
                                }
                                for block in gm_blocks {
                                    let _ = vm.execute_block(&block.lines, Some(q_id));
                                }
                                vm.quest_manager.quest_variables.insert(q_id, vm.locals.clone());
                            }
                        }
                    }'''
content = content.replace(quest_loop_orig, quest_loop_new)

open('crates/fo3_script/src/event.rs', 'w', encoding='utf-8').write(content)
