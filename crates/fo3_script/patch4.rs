use std::fs;
fn main() {
    let mut content = fs::read_to_string("crates/fo3_script/src/parser.rs").unwrap();
    let old_parse_expr = r#"    fn parse_expression(&mut self, _precedence: u8) -> Result<Expr, String> {
        // stub
        Ok(Expr::Number(0.0))
    }"#;
    let new_parse_expr = r#"    fn parse_expression(&mut self, precedence: u8) -> Result<Expr, String> {
        let mut left = match self.next_token() {
            Some(Token::Number(n)) => Expr::Number(n),
            Some(Token::Identifier(name)) => {
                let lower = name.to_ascii_lowercase();
                if lower == "getsecondspassed" || lower == "getbuttonpressed" || lower == "getinchargen" {
                    Expr::FunctionCall { subject: None, function: name, args: vec![] }
                } else if lower == "getstage" || lower == "getstage" {
                    let mut args = vec![];
                    if let Some(Token::Identifier(arg)) = self.peek().cloned() {
                        self.next_token();
                        args.push(Expr::Variable(arg));
                    }
                    Expr::FunctionCall { subject: None, function: name, args }
                } else {
                    Expr::Variable(name)
                }
            },
            Some(Token::Symbol('(')) => {
                let expr = self.parse_expression(0)?;
                if let Some(Token::Symbol(')')) = self.next_token() {
                    expr
                } else {
                    return Err("Expected ')'".to_string());
                }
            },
            Some(Token::Symbol('-')) => {
                let right = self.parse_expression(70)?; // Unary minus
                Expr::BinaryOp { op: BinaryOperator::Sub, left: Box::new(Expr::Number(0.0)), right: Box::new(right) }
            },
            Some(tok) => return Err(format!("Unexpected token in expression: {:?}", tok)),
            None => return Err("Unexpected EOF in expression".to_string()),
        };

        loop {
            let op = match self.peek() {
                Some(Token::Symbol('+')) => BinaryOperator::Add,
                Some(Token::Symbol('-')) => BinaryOperator::Sub,
                Some(Token::Symbol('*')) => BinaryOperator::Mul,
                Some(Token::Symbol('/')) => BinaryOperator::Div,
                Some(Token::Symbol('=')) => {
                    self.next_token(); // consume '='
                    if let Some(Token::Symbol('=')) = self.peek() {
                        BinaryOperator::Eq
                    } else {
                        return Err("Expected '=='".to_string());
                    }
                },
                Some(Token::Symbol('!')) => {
                    self.next_token();
                    if let Some(Token::Symbol('=')) = self.peek() {
                        BinaryOperator::Neq
                    } else {
                        return Err("Expected '!='".to_string());
                    }
                },
                Some(Token::Symbol('<')) => {
                    self.next_token();
                    if let Some(Token::Symbol('=')) = self.peek() {
                        BinaryOperator::Lte
                    } else {
                        self.push_back(Token::Symbol('<'));
                        BinaryOperator::Lt
                    }
                },
                Some(Token::Symbol('>')) => {
                    self.next_token();
                    if let Some(Token::Symbol('=')) = self.peek() {
                        BinaryOperator::Gte
                    } else {
                        self.push_back(Token::Symbol('>'));
                        BinaryOperator::Gt
                    }
                },
                _ => break,
            };

            let op_prec = match op {
                BinaryOperator::Eq | BinaryOperator::Neq | BinaryOperator::Lt | BinaryOperator::Gt | BinaryOperator::Lte | BinaryOperator::Gte => 30,
                BinaryOperator::Add | BinaryOperator::Sub => 50,
                BinaryOperator::Mul | BinaryOperator::Div => 60,
                _ => 0,
            };

            if op_prec <= precedence {
                break;
            }

            self.next_token(); // Consume operator
            let right = self.parse_expression(op_prec)?;
            left = Expr::BinaryOp { op, left: Box::new(left), right: Box::new(right) };
        }

        Ok(left)
    }"#;
    content = content.replace(old_parse_expr, new_parse_expr);
    fs::write("crates/fo3_script/src/parser.rs", content).unwrap();
}
