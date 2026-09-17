//! # GECK スクリプト (SCTX) パーサー
//!
//! ソーステキスト (SCTX) をパースし、抽象構文木 (AST) またはより強固な
//! 実行可能表現に変換する。
//!
//! 参照元: GECK Wiki "Scripting"

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f32),
    Variable(String),
    FunctionCall {
        subject: Option<String>,
        function: String,
        args: Vec<Expr>,
    },
    BinaryOp {
        op: BinaryOperator,
        left: Box<Expr>,
        right: Box<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryOperator {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Set {
        target: String,
        expr: Expr,
    },
    If {
        condition: Expr,
        then_block: Vec<Statement>,
        else_ifs: Vec<(Expr, Vec<Statement>)>,
        else_block: Option<Vec<Statement>>,
    },
    Call {
        subject: Option<String>,
        command: String,
        args: Vec<Expr>,
    },
    Return,
    Activate,
}
use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Keyword(String),
    Identifier(String),
    Number(f32),
    StringLiteral(String),
    Operator(String),
    LParen,
    RParen,
    Comma,
    Dot,
    Newline,
    EOF,
}

pub struct Lexer<'a> {
    input: Peekable<Chars<'a>>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input: input.chars().peekable(),
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        while let Some(&c) = self.input.peek() {
            if c.is_whitespace() && c != '\n' && c != '\r' {
                self.input.next();
            } else if c == ';' {
                while let Some(&ch) = self.input.peek() {
                    if ch == '\n' {
                        break;
                    }
                    self.input.next();
                }
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace_and_comments();
        if let Some(&c) = self.input.peek() {
            if c == '\n' || c == '\r' {
                self.input.next();
                if c == '\r' {
                    if let Some(&'\n') = self.input.peek() {
                        self.input.next();
                    }
                }
                return Token::Newline;
            }
            if c.is_alphabetic() || c == '_' {
                return self.read_identifier_or_keyword();
            } else if c.is_ascii_digit() || c == '-' || c == '.' {
                // Could be number or dot
                if c == '.' {
                    let mut temp = self.input.clone();
                    temp.next();
                    if let Some(&next_c) = temp.peek() {
                        if !next_c.is_ascii_digit() {
                            self.input.next();
                            return Token::Dot;
                        }
                    }
                }
                return self.read_number();
            } else if c == '"' {
                return self.read_string();
            } else {
                return self.read_operator_or_punct();
            }
        }
        Token::EOF
    }

    fn read_identifier_or_keyword(&mut self) -> Token {
        let mut s = String::new();
        while let Some(&c) = self.input.peek() {
            if c.is_alphanumeric() || c == '_' {
                s.push(c);
                self.input.next();
            } else {
                break;
            }
        }
        let lower = s.to_ascii_lowercase();
        match lower.as_str() {
            "begin" | "end" | "if" | "else" | "elseif" | "endif" | "set" | "to" | "return"
            | "activate" => Token::Keyword(lower),
            _ => Token::Identifier(s),
        }
    }

    fn read_number(&mut self) -> Token {
        let mut s = String::new();
        if let Some(&'-') = self.input.peek() {
            s.push('-');
            self.input.next();
        }

        // Handle hex: 0x or 0X
        if let Some(&'0') = self.input.peek() {
            s.push('0');
            self.input.next();
            if let Some(&'x') | Some(&'X') = self.input.peek() {
                s.push('x');
                self.input.next();
                while let Some(&c) = self.input.peek() {
                    if c.is_ascii_hexdigit() {
                        s.push(c);
                        self.input.next();
                    } else {
                        break;
                    }
                }
                // We parse hex into f32 for now, or just return an Identifier since we don't have Hex Token
                // But typically hex forms are FormIDs. In Expr we can parse f32 from hex.
                if let Ok(num) = u32::from_str_radix(&s[2..], 16) {
                    return Token::Number(num as f32);
                } else {
                    return Token::Identifier(s);
                }
            }
        }

        while let Some(&c) = self.input.peek() {
            if c.is_ascii_digit() || c == '.' {
                s.push(c);
                self.input.next();
            } else {
                break;
            }
        }
        if s == "-" || s == "." {
            return Token::Operator(s);
        }
        if let Ok(num) = s.parse::<f32>() {
            Token::Number(num)
        } else {
            Token::Identifier(s)
        }
    }
    fn read_string(&mut self) -> Token {
        self.input.next(); // skip "
        let mut s = String::new();
        while let Some(&c) = self.input.peek() {
            if c == '"' {
                self.input.next();
                break;
            }
            s.push(c);
            self.input.next();
        }
        Token::StringLiteral(s)
    }

    fn read_operator_or_punct(&mut self) -> Token {
        let c = self.input.next().unwrap();
        match c {
            '(' => Token::LParen,
            ')' => Token::RParen,
            ',' => Token::Comma,
            '.' => Token::Dot,
            '=' | '!' | '<' | '>' | '&' | '|' | '+' | '-' | '*' | '/' => {
                let mut op = c.to_string();
                if let Some(&next_c) = self.input.peek() {
                    if (c == '=' && next_c == '=')
                        || (c == '!' && next_c == '=')
                        || (c == '<' && next_c == '=')
                        || (c == '>' && next_c == '=')
                        || (c == '&' && next_c == '&')
                        || (c == '|' && next_c == '|')
                    {
                        op.push(next_c);
                        self.input.next();
                    }
                }
                Token::Operator(op)
            }
            _ => Token::Operator(c.to_string()),
        }
    }
}
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

    fn peek(&self) -> &Token {
        &self.current_token
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
            Token::Identifier(name) => {
                self.advance();
                let lower = name.to_ascii_lowercase();
                if lower == "getsecondspassed"
                    || lower == "getbuttonpressed"
                    || lower == "getinchargen"
                {
                    Expr::FunctionCall {
                        subject: None,
                        function: name,
                        args: vec![],
                    }
                } else if lower == "getstage" {
                    let mut args = vec![];
                    if let Token::Identifier(ref arg) = self.current_token {
                        args.push(Expr::Variable(arg.clone()));
                        self.advance();
                    }
                    Expr::FunctionCall {
                        subject: None,
                        function: name,
                        args,
                    }
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
                let right = self.parse_expression(70)?; // Unary minus
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
                    _ => break,
                },
                _ => break,
            };

            let op_prec = match op {
                BinaryOperator::Eq
                | BinaryOperator::Neq
                | BinaryOperator::Lt
                | BinaryOperator::Gt
                | BinaryOperator::Lte
                | BinaryOperator::Gte => 30,
                BinaryOperator::Add | BinaryOperator::Sub => 50,
                BinaryOperator::Mul | BinaryOperator::Div => 60,
                _ => 0,
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
#[test]
fn test_hex_parsing() {
    let mut p = crate::parser::Parser::new("setstage 0x00014E89 10");
    println!("{:#?}", p.parse_statements());
}
#[test]
fn test_hex_parsing2() {
    let mut p = crate::parser::Parser::new("setstage 0x00014E89 10\n");
    println!("{:#?}", p.parse_statements());
}
