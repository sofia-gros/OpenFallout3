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
                    if let Some(id) = q_id {
                        self.quest_stages.get(&id).copied().unwrap_or(0) as f64
                    } else {
                        0.0
                    }
                }'''

new_getstage = r'''                } else if func_lower == "getstage" {
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

content = content.replace(old_getstage, new_getstage)

# ALSO print parse_single_expr errors
old_parse = r'''    pub fn eval_expr(&self, expr: &str) -> f64 {
        match crate::parser::Parser::parse_single_expr(expr) {
            Ok(ast) => self.eval_ast_expr(&ast, None),
            Err(_) => self.resolve_value(expr.trim())
        }
    }'''

new_parse = r'''    pub fn eval_expr(&self, expr: &str) -> f64 {
        match crate::parser::Parser::parse_single_expr(expr) {
            Ok(ast) => {
                let res = self.eval_ast_expr(&ast, None);
                println!("[DEBUG EVAL] ast ok expr='{}' res={}", expr, res);
                res
            },
            Err(e) => {
                let res = self.resolve_value(expr.trim());
                println!("[DEBUG EVAL] ast err expr='{}' e='{}' res={}", expr, e, res);
                res
            }
        }
    }'''

content = content.replace(old_parse, new_parse)

open('crates/fo3_script/src/vm.rs', 'w', encoding='utf-8').write(content)
