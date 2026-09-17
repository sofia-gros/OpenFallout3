//! # 数値・環境・ユーティリティ関数
//! 参照元: references/openmw/components/esm4/script.hpp

use crate::parser::Expr;
use crate::vm::{ScriptVm, ScriptError};
use fo3_esm::FormId;

pub fn execute(
    cmd: &str,
    args: &[Expr],
    subject: Option<FormId>,
    vm: &mut ScriptVm,
) -> Result<Option<f32>, ScriptError> {
    match cmd {
        // FUN_GetSecondsPassed = 12
        "getsecondspassed" => {
            // スクリプトフレーム間の経過秒数 (標準 1/60 秒)
            Ok(Some(1.0 / 60.0))
        }
        // FUN_GetCurrentTime = 18
        "getcurrenttime" => {
            let hour = vm.globals.get("gamehour").copied().unwrap_or(12.0);
            Ok(Some(hour))
        }
        // FUN_GetRandomPercent = 77
        "getrandompercent" => {
            // 0〜99 の疑似乱数
            let sec = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u32)
                .unwrap_or(42);
            let percent = (sec % 100) as f32;
            Ok(Some(percent))
        }
        // FUN_GetGlobalValue = 74
        "getglobalvalue" => {
            if !args.is_empty() {
                if let Expr::Variable(ref name) = args[0] {
                    let val = vm.resolve_value(name, subject);
                    return Ok(Some(val));
                }
            }
            Ok(Some(0.0))
        }
        // SetGlobalValue
        "setglobalvalue" => {
            if args.len() >= 2 {
                if let Expr::Variable(ref name) = args[0] {
                    let val = vm.eval_ast_expr(&args[1], subject)?;
                    vm.globals.insert(name.to_ascii_lowercase(), val);
                }
            }
            Ok(Some(0.0))
        }
        // FUN_GetDayofWeek = 170
        "getdayofweek" => {
            let days = vm.globals.get("gamedayspassed").copied().unwrap_or(1.0) as u32;
            let dow = (days % 7) as f32;
            Ok(Some(dow))
        }
        // FUN_IsWin32 = 524
        "iswin32" => Ok(Some(1.0)),
        // FUN_IsPS3 = 523
        "isps3" => Ok(Some(0.0)),
        // FUN_IsXBox = 309
        "isxbox" => Ok(Some(0.0)),
        // abs [x]
        // 参照元: GECK Wiki `abs`
        // 数値の絶対値を算出する
        "abs" => {
            if !args.is_empty() {
                let val = vm.eval_ast_expr(&args[0], subject)?;
                return Ok(Some(val.abs()));
            }
            Ok(Some(0.0))
        }
        // sin [degrees]
        // 参照元: GECK Wiki `sin`
        // 正弦関数 (引数は度数法)
        "sin" => {
            if !args.is_empty() {
                let deg = vm.eval_ast_expr(&args[0], subject)?;
                return Ok(Some(deg.to_radians().sin()));
            }
            Ok(Some(0.0))
        }
        // cos [degrees]
        // 参照元: GECK Wiki `cos`
        // 余弦関数 (引数は度数法)
        "cos" => {
            if !args.is_empty() {
                let deg = vm.eval_ast_expr(&args[0], subject)?;
                return Ok(Some(deg.to_radians().cos()));
            }
            Ok(Some(1.0))
        }
        // tan [degrees]
        // 参照元: GECK Wiki `tan`
        // 正接関数 (引数は度数法)
        "tan" => {
            if !args.is_empty() {
                let deg = vm.eval_ast_expr(&args[0], subject)?;
                return Ok(Some(deg.to_radians().tan()));
            }
            Ok(Some(0.0))
        }
        // asin [x]
        // 参照元: GECK Wiki `asin`
        // 逆正弦関数 (結果は度数法 -90〜90)
        "asin" => {
            if !args.is_empty() {
                let val = vm.eval_ast_expr(&args[0], subject)?;
                let clamped = val.clamp(-1.0, 1.0);
                return Ok(Some(clamped.asin().to_degrees()));
            }
            Ok(Some(0.0))
        }
        // acos [x]
        // 参照元: GECK Wiki `acos`
        // 逆余弦関数 (結果は度数法 0〜180)
        "acos" => {
            if !args.is_empty() {
                let val = vm.eval_ast_expr(&args[0], subject)?;
                let clamped = val.clamp(-1.0, 1.0);
                return Ok(Some(clamped.acos().to_degrees()));
            }
            Ok(Some(0.0))
        }
        // atan [x]
        // 参照元: GECK Wiki `atan`
        // 逆正接関数 (結果は度数法 -90〜90)
        "atan" => {
            if !args.is_empty() {
                let val = vm.eval_ast_expr(&args[0], subject)?;
                return Ok(Some(val.atan().to_degrees()));
            }
            Ok(Some(0.0))
        }
        // sqrt [x]
        // 参照元: GECK Wiki `sqrt`
        // 平方根を算出する
        "sqrt" => {
            if !args.is_empty() {
                let val = vm.eval_ast_expr(&args[0], subject)?;
                return Ok(Some(if val < 0.0 { 0.0 } else { val.sqrt() }));
            }
            Ok(Some(0.0))
        }
        // floor [x]
        // 参照元: GECK Wiki `floor`
        // 指定数値以下の最大の整数を算出する
        "floor" => {
            if !args.is_empty() {
                let val = vm.eval_ast_expr(&args[0], subject)?;
                return Ok(Some(val.floor()));
            }
            Ok(Some(0.0))
        }
        // ceil [x]
        // 参照元: GECK Wiki `ceil`
        // 指定数値以上の最小の整数を算出する
        "ceil" => {
            if !args.is_empty() {
                let val = vm.eval_ast_expr(&args[0], subject)?;
                return Ok(Some(val.ceil()));
            }
            Ok(Some(0.0))
        }
        // round [x]
        // 参照元: GECK Wiki `round`
        // 最も近い整数に四捨五入する
        "round" => {
            if !args.is_empty() {
                let val = vm.eval_ast_expr(&args[0], subject)?;
                return Ok(Some(val.round()));
            }
            Ok(Some(0.0))
        }
        // log / ln [x]
        // 参照元: GECK Wiki `log`
        // 自然対数を算出する
        "log" | "ln" => {
            if !args.is_empty() {
                let val = vm.eval_ast_expr(&args[0], subject)?;
                return Ok(Some(if val <= 0.0 { 0.0 } else { val.ln() }));
            }
            Ok(Some(0.0))
        }
        // exp [x]
        // 参照元: GECK Wiki `exp`
        // ネイピア数の累乗 (e^x) を算出する
        "exp" => {
            if !args.is_empty() {
                let val = vm.eval_ast_expr(&args[0], subject)?;
                return Ok(Some(val.exp()));
            }
            Ok(Some(1.0))
        }
        // FUN_GetDaysPassed / GetDaysPassed
        // 参照元: GECK Wiki `GetDaysPassed`, references/openmw/components/esm4/script.hpp
        // ゲーム開始からの経過日数を取得する
        "getdayspassed" => {
            let days = vm.globals.get("gamedayspassed").copied().unwrap_or(1.0);
            Ok(Some(days))
        }
        // FUN_IsTimePassing = 265
        // 参照元: references/openmw/components/esm4/script.hpp:162
        // 睡眠・待機・ファストトラベル等の時間経過処理中か判定する
        "istimepassing" => {
            Ok(Some(0.0))
        }
        // GetTimeSinceActivated
        // 参照元: GECK Wiki `GetTimeSinceActivated`
        // 直近のアクティベートからの経過秒数を取得する
        "gettimesinceactivated" => {
            Ok(Some(0.0))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_math_opcodes() {
        let mut vm = ScriptVm::new();

        // abs
        assert_eq!(execute("abs", &[Expr::Number(-42.5)], None, &mut vm).unwrap(), Some(42.5));
        assert_eq!(execute("abs", &[Expr::Number(10.0)], None, &mut vm).unwrap(), Some(10.0));

        // sin / cos / tan
        let sin_val = execute("sin", &[Expr::Number(90.0)], None, &mut vm).unwrap().unwrap();
        assert!((sin_val - 1.0).abs() < 1e-5);
        let cos_val = execute("cos", &[Expr::Number(0.0)], None, &mut vm).unwrap().unwrap();
        assert!((cos_val - 1.0).abs() < 1e-5);
        let tan_val = execute("tan", &[Expr::Number(45.0)], None, &mut vm).unwrap().unwrap();
        assert!((tan_val - 1.0).abs() < 1e-4);

        // asin / acos / atan
        let asin_val = execute("asin", &[Expr::Number(1.0)], None, &mut vm).unwrap().unwrap();
        assert!((asin_val - 90.0).abs() < 1e-4);
        let acos_val = execute("acos", &[Expr::Number(1.0)], None, &mut vm).unwrap().unwrap();
        assert!(acos_val.abs() < 1e-4);
        let atan_val = execute("atan", &[Expr::Number(1.0)], None, &mut vm).unwrap().unwrap();
        assert!((atan_val - 45.0).abs() < 1e-4);

        // sqrt / floor / ceil / round
        assert_eq!(execute("sqrt", &[Expr::Number(16.0)], None, &mut vm).unwrap(), Some(4.0));
        assert_eq!(execute("floor", &[Expr::Number(3.7)], None, &mut vm).unwrap(), Some(3.0));
        assert_eq!(execute("ceil", &[Expr::Number(3.2)], None, &mut vm).unwrap(), Some(4.0));
        assert_eq!(execute("round", &[Expr::Number(3.5)], None, &mut vm).unwrap(), Some(4.0));

        // exp / log
        let exp_val = execute("exp", &[Expr::Number(1.0)], None, &mut vm).unwrap().unwrap();
        assert!((exp_val - std::f32::consts::E).abs() < 1e-4);
        let log_val = execute("log", &[Expr::Number(std::f32::consts::E)], None, &mut vm).unwrap().unwrap();
        assert!((log_val - 1.0).abs() < 1e-4);

        // days passed / time passing
        vm.globals.insert("gamedayspassed".into(), 5.0);
        assert_eq!(execute("getdayspassed", &[], None, &mut vm).unwrap(), Some(5.0));
        assert_eq!(execute("istimepassing", &[], None, &mut vm).unwrap(), Some(0.0));
    }
}
