//! # GECK スクリプトパーサー
//!
//! 字句解析 (Lexer) および構文解析 (Parser) を行い、AST を生成する。

mod ast;
mod lexer;
mod parser;

pub use ast::{BinaryOperator, Expr, Statement};
pub use lexer::{Lexer, Token};
pub use parser::Parser;

#[cfg(test)]
mod tests;
