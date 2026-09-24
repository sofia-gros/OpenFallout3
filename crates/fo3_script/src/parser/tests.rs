//! GECK スクリプト構文解析テストモジュール。

#[cfg(test)]
mod tests {
    use super::super::{Expr, Parser, Statement};

    #[test]
    fn test_hex_parsing() {
        let mut p = Parser::new("setstage 0x00014E89 10");
        let stmts = p.parse_statements().expect("parse should succeed");
        assert_eq!(stmts.len(), 1);
    }

    #[test]
    fn test_hex_parsing2() {
        let mut p = Parser::new("setstage 0x00014E89 10\n");
        let stmts = p.parse_statements().expect("parse should succeed");
        assert_eq!(stmts.len(), 1);
    }

    #[test]
    fn test_geck_function_with_args_in_if() {
        let mut p = Parser::new("if GetIsID NPC == 1\nsetstage CG00 10\nendif\n");
        let stmts = p.parse_statements().expect("parse should succeed");
        assert_eq!(stmts.len(), 1);
        if let Statement::If { condition, then_block, .. } = &stmts[0] {
            assert_eq!(then_block.len(), 1);
            match condition {
                Expr::BinaryOp { left, .. } => {
                    if let Expr::FunctionCall { function, args, .. } = left.as_ref() {
                        assert_eq!(function, "GetIsID");
                        assert_eq!(args.len(), 1);
                    } else {
                        panic!("Expected FunctionCall");
                    }
                }
                _ => panic!("Expected BinaryOp"),
            }
        } else {
            panic!("Expected If statement");
        }
    }

    #[test]
    fn test_dot_call_in_if() {
        let mut p = Parser::new("if player.GetDistance Dad < 500\nreturn\nendif\n");
        let stmts = p.parse_statements().expect("parse should succeed");
        assert_eq!(stmts.len(), 1);
    }

    #[test]
    fn test_dot_property_in_if() {
        let mut p = Parser::new("if CG03.HitSnake == 2\nreturn\nendif\n");
        let stmts = p.parse_statements().expect("parse should succeed");
        assert_eq!(stmts.len(), 1);
    }
}
