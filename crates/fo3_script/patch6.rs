use std::fs;
fn main() {
    let mut content = fs::read_to_string("crates/fo3_script/src/vm.rs").unwrap();
    let old = "        if let Ok(ast) = crate::parser::Parser::parse_single_expr(expr) {\n            self.eval_ast_expr(&ast, None)\n        } else {\n            self.resolve_value(expr.trim()) // Fallback\n        }";
    let new = "        match crate::parser::Parser::parse_single_expr(expr) {\n            Ok(ast) => self.eval_ast_expr(&ast, None),\n            Err(e) => {\n                println!(\"[VM] AST eval error for '{}': {}\", expr, e);\n                self.resolve_value(expr.trim())\n            }\n        }";
    content = content.replace(old, new);
    fs::write("crates/fo3_script/src/vm.rs", content).unwrap();
}
