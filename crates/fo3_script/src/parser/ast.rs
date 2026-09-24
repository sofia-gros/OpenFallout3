//! GECK スクリプト AST 構造体および演算子定義。

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f32),
    String(String),
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
