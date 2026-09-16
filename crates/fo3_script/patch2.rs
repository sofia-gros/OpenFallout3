use std::fs;
fn main() {
    let mut content = fs::read_to_string("crates/fo3_script/src/vm.rs").unwrap();
    let old_eval_expr = r#"    pub fn eval_expr(&self, expr: &str) -> f64 {
        let clean = expr.trim();
        if let Some((left, right)) = clean.split_once(" - ") {
            return self.eval_expr(left) - self.eval_expr(right);
        }
        if let Some((left, right)) = clean.split_once(" + ") {
            return self.eval_expr(left) + self.eval_expr(right);
        }
        self.resolve_value(clean)
    }"#;
    let new_eval_expr = r#"    pub fn eval_expr(&self, expr: &str) -> f64 {
        if let Ok(ast) = crate::parser::Parser::parse_single_expr(expr) {
            self.eval_ast_expr(&ast, None)
        } else {
            self.resolve_value(expr.trim()) // Fallback
        }
    }

    pub fn eval_ast_expr(&self, expr: &crate::parser::Expr, self_id: Option<FormId>) -> f64 {
        use crate::parser::{Expr, BinaryOperator};
        match expr {
            Expr::Number(n) => *n as f64,
            Expr::Variable(v) => self.resolve_value(v),
            Expr::FunctionCall { subject, function, args } => {
                let func_lower = function.to_ascii_lowercase();
                if func_lower == "getsecondspassed" {
                    self.delta_time as f64
                } else if func_lower == "getinchargen" {
                    if self.in_chargen { 1.0 } else { 0.0 }
                } else if func_lower == "getstage" {
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
                } else {
                    0.0
                }
            }
            Expr::BinaryOp { op, left, right } => {
                let l = self.eval_ast_expr(left, self_id);
                let r = self.eval_ast_expr(right, self_id);
                match op {
                    BinaryOperator::Add => l + r,
                    BinaryOperator::Sub => l - r,
                    BinaryOperator::Mul => l * r,
                    BinaryOperator::Div => if r != 0.0 { l / r } else { 0.0 },
                    BinaryOperator::Eq => if l == r { 1.0 } else { 0.0 },
                    BinaryOperator::Neq => if l != r { 1.0 } else { 0.0 },
                    BinaryOperator::Lt => if l < r { 1.0 } else { 0.0 },
                    BinaryOperator::Gt => if l > r { 1.0 } else { 0.0 },
                    BinaryOperator::Lte => if l <= r { 1.0 } else { 0.0 },
                    BinaryOperator::Gte => if l >= r { 1.0 } else { 0.0 },
                    BinaryOperator::And => if l != 0.0 && r != 0.0 { 1.0 } else { 0.0 },
                    BinaryOperator::Or => if l != 0.0 || r != 0.0 { 1.0 } else { 0.0 },
                }
            }
        }
    }"#;
    content = content.replace(old_eval_expr, new_eval_expr);
    fs::write("crates/fo3_script/src/vm.rs", content).unwrap();
}
