import sys
content = open('crates/fo3_script/src/vm.rs', 'r', encoding='utf-8').read()

patch = r'''                        if let Some(q_id) = target_q_id {
                            println!("[DEBUG] TARGET Q_ID FOUND FOR {}: {:?}", prefix, q_id);
                            self.quest_manager.set_quest_variable(q_id, sub, val);
                        } else {
                            println!("[DEBUG] TARGET Q_ID NOT FOUND FOR {}", prefix);
                            self.locals.insert(format!("{}.{}", prefix, sub), val);
                            self.locals.insert(sub.to_string(), val);
                        }'''

content = content.replace(
'''                        if let Some(q_id) = target_q_id {
                            self.quest_manager.set_quest_variable(q_id, sub, val);
                        } else {
                            self.locals.insert(format!("{}.{}", prefix, sub), val);
                            self.locals.insert(sub.to_string(), val);
                        }''', patch)

open('crates/fo3_script/src/vm.rs', 'w', encoding='utf-8').write(content)
