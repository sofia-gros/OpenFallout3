using System;
using System.IO;
using System.Text.RegularExpressions;

string text = File.ReadAllText("crates/fo3_script/src/vm.rs");

text = text.Replace("pub fn eval_condition_str(&self, expr: &str) -> bool", "pub fn eval_condition_str(&self, expr: &str, self_id: Option<FormId>) -> bool");
text = text.Replace("let left_val = self.eval_expr(left_str);", "let left_val = self.eval_expr(left_str, self_id);");
text = text.Replace("let right_val = self.eval_expr(right_str);", "let right_val = self.eval_expr(right_str, self_id);");
text = text.Replace("self.eval_expr(clean).abs() > 1e-4", "self.eval_expr(clean, self_id).abs() > 1e-4");

text = text.Replace("pub fn eval_expr(&self, expr: &str) -> f32", "pub fn eval_expr(&self, expr: &str, self_id: Option<FormId>) -> f32");
text = text.Replace("return self.eval_expr(left) - self.eval_expr(right);", "return self.eval_expr(left, self_id) - self.eval_expr(right, self_id);");
text = text.Replace("return self.eval_expr(left) + self.eval_expr(right);", "return self.eval_expr(left, self_id) + self.eval_expr(right, self_id);");
text = text.Replace("self.resolve_value(clean)", "self.resolve_value(clean, self_id)");

text = text.Replace("pub fn resolve_value(&self, token: &str) -> f32", "pub fn resolve_value(&self, token: &str, self_id: Option<FormId>) -> f32");

text = text.Replace("let val = self.eval_expr(&expr);", "let val = self.eval_expr(&expr, self_id);");
text = text.Replace("let cond = self.eval_condition_str(cond_str);", "let cond = self.eval_condition_str(cond_str, self_id);");

File.WriteAllText("crates/fo3_script/src/vm.rs", text);
