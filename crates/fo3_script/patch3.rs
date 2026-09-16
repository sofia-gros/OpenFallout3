use std::fs;
fn main() {
    let mut content = fs::read_to_string("crates/fo3_script/src/vm.rs").unwrap();
    let old_eval_cond = r#"    pub fn eval_condition_str(&self, expr: &str) -> bool {
        // 先頭・末尾のカッコや空白を削除
        let clean = expr.trim().trim_matches(|c| c == '(' || c == ')').trim();
        if clean.is_empty() {
            return true;
        }

        // 演算子の検出 (優先順位: ==, !=, <=, >= が先)
        let op_opt = if let Some(pos) = clean.find("==") {
            Some((pos, 2, "=="))
        } else if let Some(pos) = clean.find("!=") {
            Some((pos, 2, "!="))
        } else if let Some(pos) = clean.find("<=") {
            Some((pos, 2, "<="))
        } else if let Some(pos) = clean.find(">=") {
            Some((pos, 2, ">="))
        } else if let Some(pos) = clean.find('<') {
            Some((pos, 1, "<"))
        } else if let Some(pos) = clean.find('>') {
            Some((pos, 1, ">"))
        } else {
            None
        };

        if let Some((pos, len, op)) = op_opt {
            let left_str = clean[..pos].trim();
            let right_str = clean[pos + len..].trim();
            let left_val = self.eval_expr(left_str);
            let right_val = self.eval_expr(right_str);
            match op {
                "==" => left_val == right_val,
                "!=" => left_val != right_val,
                "<"  => left_val < right_val,
                ">"  => left_val > right_val,
                "<=" => left_val <= right_val,
                ">=" => left_val >= right_val,
                _ => false,
            }
        } else {
            let val = self.eval_expr(clean);
            val != 0.0
        }
    }"#;
    let new_eval_cond = r#"    pub fn eval_condition_str(&self, expr: &str) -> bool {
        let clean = expr.trim();
        if clean.is_empty() {
            return true;
        }
        self.eval_expr(clean) != 0.0
    }"#;
    
    // Fallback if formatting doesn't match
    let start_idx = content.find("pub fn eval_condition_str(&self, expr: &str) -> bool {").unwrap();
    let end_idx = content[start_idx..].find("    }\n\n    ///").unwrap() + start_idx + 5;
    
    content.replace_range(start_idx..end_idx, new_eval_cond);
    fs::write("crates/fo3_script/src/vm.rs", content).unwrap();
}
