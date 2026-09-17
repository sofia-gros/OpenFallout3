use crate::parser::{BinaryOperator, Expr, Statement};
use crate::vm::{ScriptError, ScriptVm};
use fo3_esm::FormId;

#[derive(Default)]
pub struct ExecutionResult {
    pub returned: bool,
    pub activated: bool,
}

impl ExecutionResult {
    pub fn merge(mut self, other: ExecutionResult) -> Self {
        self.returned |= other.returned;
        self.activated |= other.activated;
        self
    }
}

impl ScriptVm {
    pub fn eval_ast_expr(&self, expr: &Expr, self_id: Option<FormId>) -> Result<f32, ScriptError> {
        match expr {
            Expr::Number(val) => Ok(*val),
            Expr::Variable(name) => Ok(self.resolve_value(name, self_id)),
            Expr::FunctionCall {
                subject,
                function,
                args,
            } => {
                let mut cmd = function.clone();
                if let Some(sub) = subject {
                    cmd = format!("{}.{}", sub, function);
                }
                // For now fallback to string resolving for functions like GetStage
                for arg in args {
                    if let Expr::Variable(v) = arg {
                        cmd = format!("{} {}", cmd, v);
                    } else {
                        let arg_val = self.eval_ast_expr(arg, self_id)?;
                        cmd = format!("{} {}", cmd, arg_val);
                    }
                }
                Ok(self.resolve_value(&cmd, self_id))
            }
            Expr::BinaryOp { op, left, right } => {
                let l = self.eval_ast_expr(left, self_id)?;
                let r = self.eval_ast_expr(right, self_id)?;
                match op {
                    BinaryOperator::Add => Ok(l + r),
                    BinaryOperator::Sub => Ok(l - r),
                    BinaryOperator::Mul => Ok(l * r),
                    BinaryOperator::Div => {
                        if r == 0.0 {
                            Ok(0.0)
                        } else {
                            Ok(l / r)
                        }
                    }
                    BinaryOperator::Eq => Ok(if (l - r).abs() < 1e-4 { 1.0 } else { 0.0 }),
                    BinaryOperator::Neq => Ok(if (l - r).abs() >= 1e-4 { 1.0 } else { 0.0 }),
                    BinaryOperator::Lt => Ok(if l < r { 1.0 } else { 0.0 }),
                    BinaryOperator::Gt => Ok(if l > r { 1.0 } else { 0.0 }),
                    BinaryOperator::Lte => Ok(if l <= r { 1.0 } else { 0.0 }),
                    BinaryOperator::Gte => Ok(if l >= r { 1.0 } else { 0.0 }),
                    BinaryOperator::And => Ok(if l != 0.0 && r != 0.0 { 1.0 } else { 0.0 }),
                    BinaryOperator::Or => Ok(if l != 0.0 || r != 0.0 { 1.0 } else { 0.0 }),
                }
            }
        }
    }

    pub fn resolve_variable_name(&self, name: &str, subject: Option<FormId>) -> String {
        let lname = name.to_lowercase();
        if let Some(idx) = lname.find('.') {
            let prefix = &lname[..idx];
            let suffix = &lname[idx + 1..];
            if let Ok(id) = self.resolve_form_id(prefix) {
                if Some(id) == subject {
                    return suffix.to_string();
                }
                return format!("{:08X}.{}", id.0, suffix);
            }
        }
        lname
    }

    pub fn execute_ast_statement(
        &mut self,
        stmt: &Statement,
        self_id: Option<FormId>,
    ) -> Result<ExecutionResult, ScriptError> {
        let mut result = ExecutionResult::default();
        match stmt {
            Statement::Set { target, expr } => {
                let val = self.eval_ast_expr(expr, self_id)?;
                let resolved_target = self.resolve_variable_name(target, self_id);
                self.locals.insert(resolved_target.clone(), val);

                // If the subject is a quest, persist the variable immediately
                if let Some(q_id) = self_id {
                    if self.quest_manager.quests.contains_key(&q_id) {
                        self.quest_manager
                            .set_quest_variable(q_id, &resolved_target, val as f64);
                    }
                }
            }
            Statement::If {
                condition,
                then_block,
                else_ifs,
                else_block,
            } => {
                let cond_val = self.eval_ast_expr(condition, self_id)?;
                if cond_val != 0.0 {
                    for s in then_block {
                        result = result.merge(self.execute_ast_statement(s, self_id)?);
                        if result.returned {
                            return Ok(result);
                        }
                    }
                    return Ok(result);
                }

                for (ei_cond, ei_block) in else_ifs {
                    let ei_val = self.eval_ast_expr(ei_cond, self_id)?;
                    if ei_val != 0.0 {
                        for s in ei_block {
                            result = result.merge(self.execute_ast_statement(s, self_id)?);
                            if result.returned {
                                return Ok(result);
                            }
                        }
                        return Ok(result);
                    }
                }

                if let Some(eblock) = else_block {
                    for s in eblock {
                        result = result.merge(self.execute_ast_statement(s, self_id)?);
                        if result.returned {
                            return Ok(result);
                        }
                    }
                }
            }
            Statement::Call {
                subject,
                command,
                args,
            } => {
                let lower_cmd = command.to_ascii_lowercase();
                if lower_cmd == "activate" {
                    result.activated = true;
                } else if let Some(_) = crate::functions::dispatch(
                    &lower_cmd,
                    args,
                    subject
                        .as_ref()
                        .map(|s| self.resolve_form_id(s).ok())
                        .flatten()
                        .or(self_id),
                    self,
                )? {
                    // Handled by function dispatcher
                } else {
                    // Fallback to old string execution for unimplemented functions (e.g. some obscure ones)
                    let mut cmd_str = command.clone();
                    if let Some(sub) = subject {
                        cmd_str = format!("{}.{}", sub, command);
                    }
                    for arg in args {
                        if let Expr::Variable(v) = arg {
                            cmd_str = format!("{} {}", cmd_str, v);
                        } else {
                            let arg_val = self.eval_ast_expr(arg, self_id)?;
                            cmd_str = format!("{} {}", cmd_str, arg_val);
                        }
                    }
                    self.execute_statement(&cmd_str, self_id)?;
                }
            }
            Statement::Return => {
                result.returned = true;
            }
            Statement::Activate => {
                result.activated = true;
            }
        }
        Ok(result)
    }
}
