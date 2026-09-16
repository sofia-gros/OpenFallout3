import sys
content = open('crates/fo3_script/src/vm.rs', 'r', encoding='utf-8').read()

import re

# We want to replace everything from '"set" => {' to the matching closing brace.
start_idx = content.find('            "set" => {')
if start_idx != -1:
    end_idx = content.find('            "disableplayercontrols" => {', start_idx)
    
    if end_idx != -1:
        patch = '''            "set" => {
                // set [var] to [expr...]
                if parts.len() >= 4 && parts[2].eq_ignore_ascii_case("to") {
                    let var_name = parts[1];
                    let expr = parts[3..].join(" ");
                    let val = self.eval_expr(&expr);
                    let lower_var = var_name.to_ascii_lowercase();
                    // 1. Check if global
                    let global_key = self.globals.keys()
                        .find(|k| k.eq_ignore_ascii_case(var_name))
                        .cloned();
                    if let Some(k) = global_key {
                        self.globals.insert(k, val);
                    } else if let Some((prefix, sub)) = lower_var.split_once('.') {
                        // 2. Cross reference set (e.g. CG00.timer)
                        let mut target_q_id = None;
                        if let Some(q_id) = self.edid_map.get(prefix).or_else(|| self.edid_map.get(&prefix.to_ascii_uppercase())) {
                            if self.quest_manager.quests.contains_key(q_id) {
                                target_q_id = Some(*q_id);
                            }
                        }
                        if let Some(q_id) = target_q_id {
                            if let Some(quest) = self.quest_manager.quests.get_mut(&q_id) {
                                quest.variables.insert(sub.to_string(), val);
                            }
                        } else {
                            self.locals.insert(format!("{}.{}", prefix, sub), val);
                            self.locals.insert(sub.to_string(), val);
                        }
                    } else {
                        // 3. Local set (with self_id fallback to quest)
                        let mut quest_updated = false;
                        if let Some(id) = self_id {
                            if let Some(quest) = self.quest_manager.quests.get_mut(&id) {
                                quest.variables.insert(lower_var.clone(), val);
                                quest_updated = true;
                            }
                        }
                        if !quest_updated {
                            self.locals.insert(lower_var, val);
                        }
                    }
                }
            }
'''
        new_content = content[:start_idx] + patch + content[end_idx:]
        open('crates/fo3_script/src/vm.rs', 'w', encoding='utf-8').write(new_content)
        print("Replaced!")
    else:
        print("Could not find disableplayercontrols")
else:
    print("Could not find set")
