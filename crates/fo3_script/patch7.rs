use std::fs;
fn main() {
    let mut content = fs::read_to_string("crates/fo3_script/src/vm.rs").unwrap();
    let old = "    pub fn eval_ast_expr(&self, expr: &crate::parser::Expr, self_id: Option<FormId>) -> f64 {\n        use crate::parser::{Expr, BinaryOperator};\n        match expr {";
    let new = "    pub fn eval_ast_expr(&self, expr: &crate::parser::Expr, self_id: Option<FormId>) -> f64 {\n        use crate::parser::{Expr, BinaryOperator};\n        let res = match expr {";
    content = content.replace(old, new);
    
    let old_end = "        }\n    }";
    let new_end = "        };\n        println!(\"[VM] eval_ast_expr({:?}) = {}\", expr, res);\n        res\n    }";
    
    // need to find the specific `        }\n    }` at the end of eval_ast_expr
    let start_idx = content.find("pub fn eval_ast_expr").unwrap();
    let end_idx = content[start_idx..].find("        }\n    }").unwrap() + start_idx;
    content.replace_range(end_idx..end_idx + "        }\n    }".len(), new_end);
    
    fs::write("crates/fo3_script/src/vm.rs", content).unwrap();
}
