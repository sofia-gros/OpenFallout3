//! GECK スクリプト字句解析モジュール (Lexer)。

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
