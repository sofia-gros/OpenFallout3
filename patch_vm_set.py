import sys
content = open('crates/fo3_script/src/vm.rs', 'r', encoding='utf-8').read()

patch = r'''            "set" => {
                // set [var] to [expr...]
                if parts.len() >= 4 && parts[2].eq_ignore_ascii_case("to") {
                    let var_name = parts[1];
                    let expr = parts[3..].join(" ");
                    let val = self.eval_expr(&expr);
                    let lower_var = var_name.to_ascii_lowercase();
                    // 1. Check Global
                    let global_key = self.globals.keys()
                        .find(|k| k.eq_ignore_ascii_case(var_name))
                        .cloned();
                    if let Some(k) = global_key {
                        self.globals.insert(k, val);
                    } else if let Some((prefix, sub)) = lower_var.split_once('.') {
                        // 2. Cross-reference set (e.g. CG00.timer)
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
                        // 3. Local set
                        if let Some(id) = self_id {
                            if let Some(quest) = self.quest_manager.quests.get_mut(&id) {
                                quest.variables.insert(lower_var.clone(), val);
                            }
                        }
                        self.locals.insert(lower_var, val);
                    }
                }
            }'''

content = content.replace(
'''            "set" => {
                // set [var] to [expr...]
                if parts.len() >= 4 && parts[2].eq_ignore_ascii_case("to") {
                    let var_name = parts[1];
                    let expr = parts[3..].join(" ");
                    let val = self.eval_expr(&expr);
                    let lower_var = var_name.to_ascii_lowercase();
                    // 変数スコープ (case-insensitive で検索)
                    let global_key = self.globals.keys()
                        .find(|k| k.eq_ignore_ascii_case(var_name))
                        .cloned();
                    if let Some(k) = global_key {
                        self.globals.insert(k, val);
                    } else if let Some((prefix, sub)) = lower_var.split_once('.') {
                        // クロスリファレンス: "cg00.timer" は locals["cg00.timer"] と locals["timer"] 両方に書き込む
                        // 理由: GECK でのエイリアス解決不備 (QuestEditorID.VarName 対策)
                        self.locals.insert(format!("{}.{}", prefix, sub), val);
                        self.locals.insert(sub.to_string(), val);
                    } else {
                        // 通常ローカル: プレフィックスなし (cg00. スコープ内等)
                        self.locals.insert(lower_var, val);
                    }
                }
            }''', patch)

open('crates/fo3_script/src/vm.rs', 'w', encoding='utf-8').write(content)
