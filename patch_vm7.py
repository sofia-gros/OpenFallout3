import sys
content = open('crates/fo3_script/src/vm.rs', 'r', encoding='utf-8').read()

old_getstage = r'''                } else if func_lower == "getstage" {
                    let q_id = args.get(0).and_then(|a| match a {
                        Expr::Variable(v) => {
                            let lower = v.to_ascii_lowercase();
                            self.edid_map.get(&lower).copied()
                                .or_else(|| self.edid_map.get(&v.to_ascii_uppercase()).copied())
                        }
                        _ => None
                    });
                    let res = if let Some(id) = q_id {
                        self.quest_stages.get(&id).copied().unwrap_or(0) as f64
                    } else {
                        0.0
                    };
                    println!("[DEBUG AST] getstage q_id={:?} -> {}", q_id, res);
                    res
                }'''

new_getstage = r'''                } else if func_lower == "getstage" {
                    let q_id = args.get(0).and_then(|a| match a {
                        Expr::Variable(v) => self.resolve_form_id(v).ok(),
                        _ => None
                    });
                    let res = if let Some(id) = q_id {
                        self.quest_stages.get(&id).copied().unwrap_or(0) as f64
                    } else {
                        0.0
                    };
                    println!("[DEBUG AST] getstage q_id={:?} -> {}", q_id, res);
                    res
                }'''

content = content.replace(old_getstage, new_getstage)
open('crates/fo3_script/src/vm.rs', 'w', encoding='utf-8').write(content)
