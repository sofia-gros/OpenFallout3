import sys
content = open('crates/fo3_script/src/vm.rs', 'r', encoding='utf-8').read()

old_cond = r'''    pub fn eval_condition_str(&self, expr: &str) -> bool {
        let clean = expr.trim();
        if clean.is_empty() {
            return true;
        }
        self.eval_expr(clean).abs() > 1e-4
    }'''

new_cond = r'''    pub fn eval_condition_str(&self, expr: &str) -> bool {
        let clean = expr.trim();
        if clean.is_empty() {
            return true;
        }
        let val = self.eval_expr(clean);
        let res = val.abs() > 1e-4;
        println!("[DEBUG COND] expr='{}' val={} res={}", expr, val, res);
        res
    }'''
content = content.replace(old_cond, new_cond)

old_ast = r'''            Expr::Variable(v) => self.resolve_value(v),'''
new_ast = r'''            Expr::Variable(v) => {
                let res = self.resolve_value(v);
                println!("[DEBUG AST] var='{}' resolved={}", v, res);
                res
            },'''
content = content.replace(old_ast, new_ast)

open('crates/fo3_script/src/vm.rs', 'w', encoding='utf-8').write(content)
