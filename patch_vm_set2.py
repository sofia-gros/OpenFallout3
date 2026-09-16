import sys
content = open('crates/fo3_script/src/vm.rs', 'r', encoding='utf-8').read()

patch = r'''                        if let Some(id) = self_id {
                            if let Some(quest) = self.quest_manager.quests.get_mut(&id) {
                                println!("[DEBUG] Setting quest {:?} variable {} to {}", id, lower_var, val);
                                quest.variables.insert(lower_var.clone(), val);
                            } else {
                                println!("[DEBUG] set error: quest {:?} not found", id);
                            }
                        }
                        self.locals.insert(lower_var, val);'''

content = content.replace(
'''                        if let Some(id) = self_id {
                            if let Some(quest) = self.quest_manager.quests.get_mut(&id) {
                                quest.variables.insert(lower_var.clone(), val);
                            }
                        }
                        self.locals.insert(lower_var, val);''', patch)

open('crates/fo3_script/src/vm.rs', 'w', encoding='utf-8').write(content)
