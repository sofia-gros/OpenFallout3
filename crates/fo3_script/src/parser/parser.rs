//! GECK スクリプト構文解析モジュール (Parser)。

use super::ast::{BinaryOperator, Expr, Statement};
use super::lexer::{Lexer, Token};

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
}

impl<'a> Parser<'a> {
    pub fn parse_single_expr(source: &str) -> Result<Expr, String> {
        let mut parser = Parser::new(source);
        parser.parse_expression(0)
    }
    pub fn new(input: &'a str) -> Self {
        let mut lexer = Lexer::new(input);
        let current_token = lexer.next_token();
        Self {
            lexer,
            current_token,
        }
    }

    fn advance(&mut self) {
        self.current_token = self.lexer.next_token();
    }

    pub fn parse_statements(&mut self) -> Result<Vec<Statement>, String> {
        let mut stmts = Vec::new();
        while self.current_token != Token::EOF {
            if self.current_token == Token::Newline {
                self.advance();
                continue;
            }
            let stmt = self.parse_statement()?;
            stmts.push(stmt);
        }
        Ok(stmts)
    }
    fn parse_statement(&mut self) -> Result<Statement, String> {
        while self.current_token == Token::Newline {
            self.advance();
        }
        match &self.current_token {
            Token::Keyword(kw) => {
                match kw.as_str() {
                    "set" => self.parse_set(),
                    "if" => self.parse_if(),
                    "return" => {
                        self.advance();
                        Ok(Statement::Return)
                    }
                    "activate" => {
                        self.advance();
                        Ok(Statement::Activate)
                    }
                    _ => {
                        // Could be a command that starts with a keyword? Or error.
                        // Wait, SetStage, AddItem etc are not keywords in Lexer, they are Identifiers.
                        let cmd = kw.clone();
                        self.advance();
                        self.parse_call(None, cmd)
                    }
                }
            }
            Token::Identifier(_) => {
                let mut subject = None;
                let mut cmd_name = match self.current_token.clone() {
                    Token::Identifier(s) => s,
                    _ => unreachable!(),
                };
                self.advance();

                if let Token::Dot = self.current_token {
                    self.advance(); // consume dot
                    subject = Some(cmd_name);
                    cmd_name = match self.current_token.clone() {
                        Token::Identifier(s) => s,
                        Token::Keyword(kw) => kw,
                        _ => return Err("Expected command after dot".to_string()),
                    };
                    self.advance();
                }
                self.parse_call(subject, cmd_name)
            }
            _ => {
                let tok = self.current_token.clone();
                self.advance();
                Err(format!("Unexpected token {:?} at statement start", tok))
            }
        }
    }

    fn parse_set(&mut self) -> Result<Statement, String> {
        self.advance(); // consume "set"

        let target = match self.current_token.clone() {
            Token::Identifier(s) => {
                self.advance();
                // Check if there is a dot (e.g. set CG00.timer to ...)
                if let Token::Dot = self.current_token {
                    self.advance();
                    if let Token::Identifier(sub) = self.current_token.clone() {
                        self.advance();
                        format!("{}.{}", s, sub)
                    } else {
                        return Err("Expected identifier after dot in set".to_string());
                    }
                } else {
                    s
                }
            }
            _ => return Err("Expected identifier in set".to_string()),
        };

        match &self.current_token {
            Token::Keyword(kw) if kw == "to" => self.advance(),
            _ => return Err("Expected 'to' in set".to_string()),
        }

        let expr = self.parse_expression(0)?;
        Ok(Statement::Set { target, expr })
    }

    fn skip_newlines(&mut self) {
        while self.current_token == Token::Newline {
            self.advance();
        }
    }

    fn parse_if(&mut self) -> Result<Statement, String> {
        self.advance(); // consume "if"
        let condition = self.parse_expression(0)?;

        let mut then_block = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.current_token, Token::Keyword(ref k) if k == "elseif" || k == "else" || k == "endif" || k == "end")
                || self.current_token == Token::EOF
            {
                break;
            }
            then_block.push(self.parse_statement()?);
        }

        let mut else_ifs = Vec::new();
        loop {
            self.skip_newlines();
            if let Token::Keyword(ref k) = self.current_token {
                if k == "elseif" {
                    self.advance(); // consume elseif
                    let ei_cond = self.parse_expression(0)?;
                    let mut ei_block = Vec::new();
                    loop {
                        self.skip_newlines();
                        if matches!(self.current_token, Token::Keyword(ref k2) if k2 == "elseif" || k2 == "else" || k2 == "endif" || k2 == "end")
                            || self.current_token == Token::EOF
                        {
                            break;
                        }
                        ei_block.push(self.parse_statement()?);
                    }
                    else_ifs.push((ei_cond, ei_block));
                    continue;
                }
            }
            break;
        }

        let mut else_block = None;
        self.skip_newlines();
        if let Token::Keyword(ref k) = self.current_token {
            if k == "else" {
                self.advance(); // consume else
                let mut e_block = Vec::new();
                loop {
                    self.skip_newlines();
                    if matches!(self.current_token, Token::Keyword(ref k2) if k2 == "endif" || k2 == "end")
                        || self.current_token == Token::EOF
                    {
                        break;
                    }
                    e_block.push(self.parse_statement()?);
                }
                else_block = Some(e_block);
            }
        }

        self.skip_newlines();
        if let Token::Keyword(ref k) = self.current_token {
            if k == "endif" || k == "end" {
                self.advance(); // consume endif/end
            } else {
                return Err(format!("Expected endif, got {:?}", self.current_token));
            }
        } else {
            return Err(format!("Expected endif, got {:?}", self.current_token));
        }

        Ok(Statement::If {
            condition,
            then_block,
            else_ifs,
            else_block,
        })
    }
    fn parse_call(
        &mut self,
        subject: Option<String>,
        command: String,
    ) -> Result<Statement, String> {
        let mut args = Vec::new();
        while self.current_token != Token::Newline && self.current_token != Token::EOF {
            if self.current_token == Token::Comma {
                self.advance();
                continue;
            }
            args.push(self.parse_expression(0)?);
        }
        if self.current_token == Token::Newline {
            self.advance();
        }
        Ok(Statement::Call {
            subject,
            command,
            args,
        })
    }

    fn parse_expression(&mut self, precedence: u8) -> Result<Expr, String> {
        let mut left = match self.current_token.clone() {
            Token::Number(n) => {
                self.advance();
                Expr::Number(n)
            }
            Token::StringLiteral(s) => {
                self.advance();
                Expr::String(s)
            }
            Token::Identifier(mut name) => {
                self.advance();
                let mut subject = None;
                if self.current_token == Token::Dot {
                    self.advance();
                    let member = match &self.current_token {
                        Token::Identifier(m) => m.clone(),
                        Token::Keyword(kw) => kw.clone(),
                        _ => return Err("Expected identifier after '.'".to_string()),
                    };
                    self.advance();
                    subject = Some(name);
                    name = member;
                }

                let is_func = subject.is_some() || is_geck_function(&name);
                if is_func {
                    let mut args = Vec::new();
                    while !is_expr_boundary(&self.current_token) {
                        let arg = match self.current_token.clone() {
                            Token::Number(n) => {
                                self.advance();
                                Expr::Number(n)
                            }
                            Token::StringLiteral(s) => {
                                self.advance();
                                Expr::String(s)
                            }
                            Token::Identifier(arg_id) => {
                                self.advance();
                                if self.current_token == Token::Dot {
                                    self.advance();
                                    if let Token::Identifier(sub) = self.current_token.clone() {
                                        self.advance();
                                        Expr::Variable(format!("{}.{}", arg_id, sub))
                                    } else {
                                        Expr::Variable(arg_id)
                                    }
                                } else {
                                    Expr::Variable(arg_id)
                                }
                            }
                            _ => break,
                        };
                        args.push(arg);
                    }
                    Expr::FunctionCall {
                        subject,
                        function: name,
                        args,
                    }
                } else if let Some(sub) = subject {
                    Expr::Variable(format!("{}.{}", sub, name))
                } else {
                    Expr::Variable(name)
                }
            }
            Token::LParen => {
                self.advance();
                let expr = self.parse_expression(0)?;
                if matches!(self.current_token, Token::RParen) {
                    self.advance();
                    expr
                } else {
                    return Err("Expected ')'".to_string());
                }
            }
            Token::Operator(ref op) if op == "-" => {
                self.advance();
                let right = self.parse_expression(70)?; // 単項マイナス
                Expr::BinaryOp {
                    op: BinaryOperator::Sub,
                    left: Box::new(Expr::Number(0.0)),
                    right: Box::new(right),
                }
            }
            tok => return Err(format!("Unexpected token in expression: {:?}", tok)),
        };

        loop {
            let op = match &self.current_token {
                Token::Operator(op_str) => match op_str.as_str() {
                    "+" => BinaryOperator::Add,
                    "-" => BinaryOperator::Sub,
                    "*" => BinaryOperator::Mul,
                    "/" => BinaryOperator::Div,
                    "==" => BinaryOperator::Eq,
                    "!=" => BinaryOperator::Neq,
                    "<" => BinaryOperator::Lt,
                    ">" => BinaryOperator::Gt,
                    "<=" => BinaryOperator::Lte,
                    ">=" => BinaryOperator::Gte,
                    "&&" => BinaryOperator::And,
                    "||" => BinaryOperator::Or,
                    _ => break,
                },
                _ => break,
            };

            let op_prec = match op {
                BinaryOperator::Or => 10,
                BinaryOperator::And => 20,
                BinaryOperator::Eq
                | BinaryOperator::Neq
                | BinaryOperator::Lt
                | BinaryOperator::Gt
                | BinaryOperator::Lte
                | BinaryOperator::Gte => 30,
                BinaryOperator::Add | BinaryOperator::Sub => 50,
                BinaryOperator::Mul | BinaryOperator::Div => 60,
            };

            if op_prec <= precedence {
                break;
            }

            self.advance(); // Consume operator
            let right = self.parse_expression(op_prec)?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }
}

/// トークンが式の二項演算子であるかを判定
fn is_binary_op(tok: &Token) -> bool {
    matches!(
        tok,
        Token::Operator(op) if matches!(
            op.as_str(),
            "+" | "-" | "*" | "/" | "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||"
        )
    )
}

/// 式の引数収集の終端境界であるかを判定
fn is_expr_boundary(tok: &Token) -> bool {
    matches!(
        tok,
        Token::Newline | Token::EOF | Token::RParen | Token::Comma | Token::Keyword(_)
    ) || is_binary_op(tok)
}

/// GECK組み込み条件・値取得関数であるかを判定
fn is_geck_function(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("get")
        || lower.starts_with("is")
        || lower.starts_with("has")
        || matches!(
            lower.as_str(),
            "menucomplete" | "playerteammate" | "say" | "startquest" | "stopquest"
        )
}
