//! # ステータス・ActorValue (能力値) 関連スクリプト関数
//! 参照元: references/openmw/components/esm4/script.hpp:77, 277, 431, 495

use crate::parser::Expr;
use crate::vm::{ScriptError, ScriptVm};
use fo3_esm::FormId;

pub fn execute(
    cmd: &str,
    args: &[Expr],
    subject: Option<FormId>,
    vm: &mut ScriptVm,
) -> Result<Option<f32>, ScriptError> {
    let target = subject.unwrap_or(FormId(0x14)); // Player fallback

    let get_form_id = |expr: &Expr| -> Result<FormId, ScriptError> {
        match expr {
            Expr::Number(n) => Ok(FormId(*n as u32)),
            Expr::Variable(v) => vm.resolve_form_id(v),
            _ => Err(ScriptError::InvalidArguments("Expected FormId".to_string())),
        }
    };

    match cmd {
        // FUN_GetActorValue = 14
        // GetActorValue [ActorValueName/ID]
        "getactorvalue" | "getav" => {
            if !args.is_empty() {
                let av_name = match &args[0] {
                    Expr::Variable(v) => v.to_ascii_lowercase(),
                    Expr::Number(n) => format!("av_{}", *n as u32),
                    _ => "unknown".to_string(),
                };
                let key = format!("{:08X}.av.{}", target.0, av_name);
                let val = vm.locals.get(&key).copied().unwrap_or(50.0); // デフォルト標準値 50
                return Ok(Some(val));
            }
            Ok(Some(50.0))
        }
        // FUN_GetBaseActorValue = 277
        "getbaseactorvalue" | "getbaseav" => {
            if !args.is_empty() {
                let av_name = match &args[0] {
                    Expr::Variable(v) => v.to_ascii_lowercase(),
                    Expr::Number(n) => format!("av_{}", *n as u32),
                    _ => "unknown".to_string(),
                };
                let key = format!("{:08X}.baseav.{}", target.0, av_name);
                let val = vm.locals.get(&key).copied().unwrap_or(50.0);
                return Ok(Some(val));
            }
            Ok(Some(50.0))
        }
        // FUN_GetPermanentActorValue = 495
        "getpermanentactorvalue" | "getpermav" => {
            if !args.is_empty() {
                let av_name = match &args[0] {
                    Expr::Variable(v) => v.to_ascii_lowercase(),
                    Expr::Number(n) => format!("av_{}", *n as u32),
                    _ => "unknown".to_string(),
                };
                let key = format!("{:08X}.baseav.{}", target.0, av_name);
                let val = vm.locals.get(&key).copied().unwrap_or(50.0);
                return Ok(Some(val));
            }
            Ok(Some(50.0))
        }
        // SetActorValue [ActorValue] [Value]
        "setactorvalue" | "setav" => {
            if args.len() >= 2 {
                let av_name = match &args[0] {
                    Expr::Variable(v) => v.to_ascii_lowercase(),
                    Expr::Number(n) => format!("av_{}", *n as u32),
                    _ => "unknown".to_string(),
                };
                let val = vm.eval_ast_expr(&args[1], subject)?;
                let key = format!("{:08X}.av.{}", target.0, av_name);
                let base_key = format!("{:08X}.baseav.{}", target.0, av_name);
                vm.locals.insert(key, val);
                vm.locals.insert(base_key, val);
            }
            Ok(Some(0.0))
        }
        // ModActorValue [ActorValue] [Delta]
        "modactorvalue" | "modav" => {
            if args.len() >= 2 {
                let av_name = match &args[0] {
                    Expr::Variable(v) => v.to_ascii_lowercase(),
                    Expr::Number(n) => format!("av_{}", *n as u32),
                    _ => "unknown".to_string(),
                };
                let delta = vm.eval_ast_expr(&args[1], subject)?;
                let key = format!("{:08X}.av.{}", target.0, av_name);
                let current = vm.locals.get(&key).copied().unwrap_or(50.0);
                vm.locals.insert(key, current + delta);
            }
            Ok(Some(0.0))
        }
        // ForceActorValue [ActorValue] [Value]
        "forceactorvalue" | "forceav" => {
            if args.len() >= 2 {
                let av_name = match &args[0] {
                    Expr::Variable(v) => v.to_ascii_lowercase(),
                    Expr::Number(n) => format!("av_{}", *n as u32),
                    _ => "unknown".to_string(),
                };
                let val = vm.eval_ast_expr(&args[1], subject)?;
                let key = format!("{:08X}.av.{}", target.0, av_name);
                vm.locals.insert(key, val);
            }
            Ok(Some(0.0))
        }
        // DamageActorValue [ActorValue] [Damage]
        "damageactorvalue" | "damageav" => {
            if args.len() >= 2 {
                let av_name = match &args[0] {
                    Expr::Variable(v) => v.to_ascii_lowercase(),
                    Expr::Number(n) => format!("av_{}", *n as u32),
                    _ => "unknown".to_string(),
                };
                let dmg = vm.eval_ast_expr(&args[1], subject)?;
                let key = format!("{:08X}.av.{}", target.0, av_name);
                let current = vm.locals.get(&key).copied().unwrap_or(50.0);
                vm.locals.insert(key, (current - dmg).max(0.0));
            }
            Ok(Some(0.0))
        }
        // RestoreActorValue [ActorValue] [Amount]
        "restoreactorvalue" | "restoreav" => {
            if args.len() >= 2 {
                let av_name = match &args[0] {
                    Expr::Variable(v) => v.to_ascii_lowercase(),
                    Expr::Number(n) => format!("av_{}", *n as u32),
                    _ => "unknown".to_string(),
                };
                let amount = vm.eval_ast_expr(&args[1], subject)?;
                let key = format!("{:08X}.av.{}", target.0, av_name);
                let base_key = format!("{:08X}.baseav.{}", target.0, av_name);
                let current = vm.locals.get(&key).copied().unwrap_or(50.0);
                let max_val = vm.locals.get(&base_key).copied().unwrap_or(50.0);
                vm.locals.insert(key, (current + amount).min(max_val));
            }
            Ok(Some(0.0))
        }
        // FUN_GetHealthPercentage = 431
        "gethealthpercentage" => {
            let key = format!("{:08X}.av.health", target.0);
            let base_key = format!("{:08X}.baseav.health", target.0);
            let cur = vm.locals.get(&key).copied().unwrap_or(100.0);
            let max_val = vm.locals.get(&base_key).copied().unwrap_or(100.0);
            if max_val <= 0.0 {
                return Ok(Some(1.0));
            }
            Ok(Some(cur / max_val))
        }
        // FUN_GetFatiguePercentage = 128
        "getfatiguepercentage" => {
            let key = format!("{:08X}.av.fatigue", target.0);
            let base_key = format!("{:08X}.baseav.fatigue", target.0);
            let cur = vm.locals.get(&key).copied().unwrap_or(100.0);
            let max_val = vm.locals.get(&base_key).copied().unwrap_or(100.0);
            if max_val <= 0.0 {
                return Ok(Some(1.0));
            }
            Ok(Some(cur / max_val))
        }
        // FUN_GetLevel = 80
        // アクターのレベル取得
        "getlevel" => {
            let key = format!("{:08X}.level", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(1.0);
            Ok(Some(val))
        }
        // SetLevel [Level]
        // アクターのレベル設定
        "setlevel" => {
            if !args.is_empty() {
                let level = vm.eval_ast_expr(&args[0], subject)?;
                let key = format!("{:08X}.level", target.0);
                vm.locals.insert(key, level);
                println!("[Script] SetLevel: Target {:?}, Level: {}", target, level);
            }
            Ok(Some(0.0))
        }
        // ModPCLevel [Delta]
        // プレイヤーのレベルを加算
        "modpclevel" => {
            let delta = if !args.is_empty() {
                vm.eval_ast_expr(&args[0], subject)?
            } else {
                1.0
            };
            let key = format!("{:08X}.level", target.0);
            let cur = vm.locals.get(&key).copied().unwrap_or(1.0);
            vm.locals.insert(key, cur + delta);
            println!("[Script] ModPCLevel: Target {:?}, Delta: {}", target, delta);
            Ok(Some(0.0))
        }
        // AdvLevel
        // レベルを1上昇させる
        "advlevel" => {
            let key = format!("{:08X}.level", target.0);
            let cur = vm.locals.get(&key).copied().unwrap_or(1.0);
            vm.locals.insert(key, cur + 1.0);
            println!("[Script] AdvLevel: Target {:?}", target);
            Ok(Some(0.0))
        }
        // GetActorValueInfo / GetAVInfo [ActorValue]
        // 能力値の情報を取得
        "getactorvalueinfo" | "getavinfo" => {
            if !args.is_empty() {
                let av_name = match &args[0] {
                    Expr::Variable(v) => v.to_ascii_lowercase(),
                    Expr::Number(n) => format!("av_{}", *n as u32),
                    _ => "unknown".to_string(),
                };
                let key = format!("{:08X}.av.{}", target.0, av_name);
                let val = vm.locals.get(&key).copied().unwrap_or(50.0);
                return Ok(Some(val));
            }
            Ok(Some(50.0))
        }
        // DamageActorValueInfo / DamageAVInfo [ActorValue] [Delta]
        "damageactorvalueinfo" | "damageavinfo" => {
            if args.len() >= 2 {
                let av_name = match &args[0] {
                    Expr::Variable(v) => v.to_ascii_lowercase(),
                    Expr::Number(n) => format!("av_{}", *n as u32),
                    _ => "unknown".to_string(),
                };
                let delta = vm.eval_ast_expr(&args[1], subject)?;
                let key = format!("{:08X}.av.{}", target.0, av_name);
                let current = vm.locals.get(&key).copied().unwrap_or(50.0);
                vm.locals.insert(key, current - delta);
            }
            Ok(Some(0.0))
        }
        // RestoreActorValueInfo / RestoreAVInfo [ActorValue] [Delta]
        "restoreactorvalueinfo" | "restoreavinfo" => {
            if args.len() >= 2 {
                let av_name = match &args[0] {
                    Expr::Variable(v) => v.to_ascii_lowercase(),
                    Expr::Number(n) => format!("av_{}", *n as u32),
                    _ => "unknown".to_string(),
                };
                let delta = vm.eval_ast_expr(&args[1], subject)?;
                let key = format!("{:08X}.av.{}", target.0, av_name);
                let base_key = format!("{:08X}.baseav.{}", target.0, av_name);
                let current = vm.locals.get(&key).copied().unwrap_or(50.0);
                let base = vm.locals.get(&base_key).copied().unwrap_or(50.0);
                vm.locals.insert(key, (current + delta).min(base));
            }
            Ok(Some(0.0))
        }
        // FUN_GetBarterGold = 264
        // 取引所持ゴールドを取得
        "getbartergold" => {
            let key = format!("{:08X}.bartergold", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(100.0);
            Ok(Some(val))
        }
        // SetBarterGold [Amount]
        // 取引所持ゴールドを設定
        "setbartergold" => {
            if !args.is_empty() {
                let gold = vm.eval_ast_expr(&args[0], subject)?;
                let key = format!("{:08X}.bartergold", target.0);
                vm.locals.insert(key, gold);
            }
            Ok(Some(0.0))
        }
        // ModBarterGold [Delta]
        // 取引所持ゴールドを加算
        "modbartergold" => {
            if !args.is_empty() {
                let delta = vm.eval_ast_expr(&args[0], subject)?;
                let key = format!("{:08X}.bartergold", target.0);
                let cur = vm.locals.get(&key).copied().unwrap_or(100.0);
                vm.locals.insert(key, cur + delta);
            }
            Ok(Some(0.0))
        }
        // FUN_GetRadiationLevel = 503
        // 放射線被曝量 (rads) を取得
        "getradiationlevel" => {
            let key = format!("{:08X}.rads", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // DamageRadiation / DamageRad [Delta]
        // 放射線被曝量を加算
        "damagerad" | "damageradiation" => {
            if !args.is_empty() {
                let delta = vm.eval_ast_expr(&args[0], subject)?;
                let key = format!("{:08X}.rads", target.0);
                let cur = vm.locals.get(&key).copied().unwrap_or(0.0);
                vm.locals.insert(key, cur + delta);
            }
            Ok(Some(0.0))
        }
        // RestoreRadiation / RestoreRad [Delta]
        // 放射線被曝量を回復（減算）
        "restorerad" | "restoreradiation" => {
            if !args.is_empty() {
                let delta = vm.eval_ast_expr(&args[0], subject)?;
                let key = format!("{:08X}.rads", target.0);
                let cur = vm.locals.get(&key).copied().unwrap_or(0.0);
                vm.locals.insert(key, (cur - delta).max(0.0));
            }
            Ok(Some(0.0))
        }
        // FUN_GetWeaponHealthPerc = 500
        // 装備武器の耐久度割合 (0.0〜1.0) を取得
        "getweaponhealthperc" => {
            let key = format!("{:08X}.weaponhealthperc", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(1.0);
            Ok(Some(val))
        }
        // FUN_GetArmorRatingUpperBody = 274
        // 上半身防護値 (Damage Resistance) を取得
        "getarmorratingupperbody" => {
            let key = format!("{:08X}.armordr", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_GetThreatRatio = 478
        // 敵味方の脅威比率を取得
        "getthreatratio" => {
            if !args.is_empty() {
                let _target_ref = get_form_id(&args[0])?;
            }
            Ok(Some(1.0))
        }
        // FUN_GetXPForNextLevel = 533
        // 次のレベルに必要な総経験値を取得
        "getxpfornextlevel" => {
            let key = format!("{:08X}.level", target.0);
            let lvl = vm.locals.get(&key).copied().unwrap_or(1.0);
            // レベル * 1000 の簡易計算
            Ok(Some(lvl * 1000.0))
        }
        // RewardXP [Amount]
        // 経験値を付与
        "rewardxp" => {
            if !args.is_empty() {
                let amount = vm.eval_ast_expr(&args[0], subject)?;
                let key = format!("{:08X}.xp", target.0);
                let cur = vm.locals.get(&key).copied().unwrap_or(0.0);
                vm.locals.insert(key, cur + amount);
                println!("[Script] RewardXP: Target {:?}, Amount: {}", target, amount);
            }
            Ok(Some(0.0))
        }
        // FUN_GetPCMiscStat = 312
        // 各種統計値を取得
        "getpcmiscstat" => {
            let stat_idx = if !args.is_empty() {
                match &args[0] {
                    Expr::Number(n) => *n as u32,
                    _ => 0,
                }
            } else {
                0
            };
            let key = format!("miscstat.{}", stat_idx);
            let val = vm.globals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // ModPCMiscStat [StatIdx] [Delta]
        // 各種統計値を加算
        "modpcmiscstat" => {
            if args.len() >= 2 {
                let stat_idx = match &args[0] {
                    Expr::Number(n) => *n as u32,
                    _ => 0,
                };
                let delta = vm.eval_ast_expr(&args[1], subject)?;
                let key = format!("miscstat.{}", stat_idx);
                let cur = vm.globals.get(&key).copied().unwrap_or(0.0);
                vm.globals.insert(key, cur + delta);
            }
            Ok(Some(0.0))
        }
        // SetPCMiscStat [StatIdx] [Value]
        // 各種統計値を設定
        "setpcmiscstat" => {
            if args.len() >= 2 {
                let stat_idx = match &args[0] {
                    Expr::Number(n) => *n as u32,
                    _ => 0,
                };
                let val = vm.eval_ast_expr(&args[1], subject)?;
                let key = format!("miscstat.{}", stat_idx);
                vm.globals.insert(key, val);
            }
            Ok(Some(0.0))
        }

        // FUN_GetReputation = 573
        // 勢力評判を取得
        "getreputation" => {
            if !args.is_empty() {
                let faction_id = get_form_id(&args[0])?;
                let key = format!("{:08X}.rep.{:08X}", target.0, faction_id.0);
                let val = vm.locals.get(&key).copied().unwrap_or(0.0);
                return Ok(Some(val));
            }
            Ok(Some(0.0))
        }
        // FUN_GetReputationPct = 574
        // 勢力評判パーセンテージ (0〜100) を取得
        "getreputationpct" => {
            if !args.is_empty() {
                let faction_id = get_form_id(&args[0])?;
                let key = format!("{:08X}.reppct.{:08X}", target.0, faction_id.0);
                let val = vm.locals.get(&key).copied().unwrap_or(50.0);
                return Ok(Some(val));
            }
            Ok(Some(50.0))
        }
        // FUN_GetReputationThreshold = 575
        // 勢力評判しきい値判定を取得
        "getreputationthreshold" => {
            if !args.is_empty() {
                let faction_id = get_form_id(&args[0])?;
                let key = format!("{:08X}.repthresh.{:08X}", target.0, faction_id.0);
                let val = vm.locals.get(&key).copied().unwrap_or(0.0);
                return Ok(Some(val));
            }
            Ok(Some(0.0))
        }
        // SetReputation [FactionId] [Value]
        // 勢力評判を設定
        "setreputation" => {
            if args.len() >= 2 {
                let faction_id = get_form_id(&args[0])?;
                let val = vm.eval_ast_expr(&args[1], subject)?;
                let key = format!("{:08X}.rep.{:08X}", target.0, faction_id.0);
                vm.locals.insert(key, val);
            }
            Ok(Some(0.0))
        }
        // ModReputation [FactionId] [Delta]
        // 勢力評判を加算
        "modreputation" => {
            if args.len() >= 2 {
                let faction_id = get_form_id(&args[0])?;
                let delta = vm.eval_ast_expr(&args[1], subject)?;
                let key = format!("{:08X}.rep.{:08X}", target.0, faction_id.0);
                let cur = vm.locals.get(&key).copied().unwrap_or(0.0);
                vm.locals.insert(key, cur + delta);
            }
            Ok(Some(0.0))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ステータス関連オペコードの実行テスト
    #[test]
    fn test_stats_opcodes_extended() {
        let mut vm = ScriptVm::new();
        let actor = FormId(0x1000);

        // レベル操作のテスト
        assert_eq!(
            execute("getlevel", &[], Some(actor), &mut vm).unwrap(),
            Some(1.0)
        );
        execute("setlevel", &[Expr::Number(5.0)], Some(actor), &mut vm).unwrap();
        assert_eq!(
            execute("getlevel", &[], Some(actor), &mut vm).unwrap(),
            Some(5.0)
        );
        execute("modpclevel", &[Expr::Number(2.0)], Some(actor), &mut vm).unwrap();
        assert_eq!(
            execute("getlevel", &[], Some(actor), &mut vm).unwrap(),
            Some(7.0)
        );
        execute("advlevel", &[], Some(actor), &mut vm).unwrap();
        assert_eq!(
            execute("getlevel", &[], Some(actor), &mut vm).unwrap(),
            Some(8.0)
        );

        // ActorValue情報の取得・ダメージ・回復
        execute(
            "setav",
            &[Expr::Variable("strength".into()), Expr::Number(10.0)],
            Some(actor),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute(
                "getavinfo",
                &[Expr::Variable("strength".into())],
                Some(actor),
                &mut vm
            )
            .unwrap(),
            Some(10.0)
        );
        execute(
            "damageavinfo",
            &[Expr::Variable("strength".into()), Expr::Number(3.0)],
            Some(actor),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute(
                "getavinfo",
                &[Expr::Variable("strength".into())],
                Some(actor),
                &mut vm
            )
            .unwrap(),
            Some(7.0)
        );
        execute(
            "restoreavinfo",
            &[Expr::Variable("strength".into()), Expr::Number(2.0)],
            Some(actor),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute(
                "getavinfo",
                &[Expr::Variable("strength".into())],
                Some(actor),
                &mut vm
            )
            .unwrap(),
            Some(9.0)
        );

        // 取引ゴールド
        assert_eq!(
            execute("getbartergold", &[], Some(actor), &mut vm).unwrap(),
            Some(100.0)
        );
        execute(
            "setbartergold",
            &[Expr::Number(500.0)],
            Some(actor),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute("getbartergold", &[], Some(actor), &mut vm).unwrap(),
            Some(500.0)
        );
        execute(
            "modbartergold",
            &[Expr::Number(150.0)],
            Some(actor),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute("getbartergold", &[], Some(actor), &mut vm).unwrap(),
            Some(650.0)
        );

        // 放射線
        assert_eq!(
            execute("getradiationlevel", &[], Some(actor), &mut vm).unwrap(),
            Some(0.0)
        );
        execute("damagerad", &[Expr::Number(50.0)], Some(actor), &mut vm).unwrap();
        assert_eq!(
            execute("getradiationlevel", &[], Some(actor), &mut vm).unwrap(),
            Some(50.0)
        );
        execute("restorerad", &[Expr::Number(20.0)], Some(actor), &mut vm).unwrap();
        assert_eq!(
            execute("getradiationlevel", &[], Some(actor), &mut vm).unwrap(),
            Some(30.0)
        );

        // 武器耐久度、アーマーDR、脅威比率、XP
        assert_eq!(
            execute("getweaponhealthperc", &[], Some(actor), &mut vm).unwrap(),
            Some(1.0)
        );
        assert_eq!(
            execute("getarmorratingupperbody", &[], Some(actor), &mut vm).unwrap(),
            Some(0.0)
        );
        assert_eq!(
            execute("getthreatratio", &[], Some(actor), &mut vm).unwrap(),
            Some(1.0)
        );
        assert_eq!(
            execute("getxpfornextlevel", &[], Some(actor), &mut vm).unwrap(),
            Some(8000.0)
        ); // レベル8 * 1000
        execute("rewardxp", &[Expr::Number(250.0)], Some(actor), &mut vm).unwrap();

        // 統計値（MiscStat）
        assert_eq!(
            execute("getpcmiscstat", &[Expr::Number(1.0)], None, &mut vm).unwrap(),
            Some(0.0)
        );
        execute(
            "setpcmiscstat",
            &[Expr::Number(1.0), Expr::Number(10.0)],
            None,
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute("getpcmiscstat", &[Expr::Number(1.0)], None, &mut vm).unwrap(),
            Some(10.0)
        );
        execute(
            "modpcmiscstat",
            &[Expr::Number(1.0), Expr::Number(5.0)],
            None,
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute("getpcmiscstat", &[Expr::Number(1.0)], None, &mut vm).unwrap(),
            Some(15.0)
        );

        // 評判（Reputation）
        let faction = FormId(0x3000);
        assert_eq!(
            execute(
                "getreputation",
                &[Expr::Number(faction.0 as f32)],
                Some(actor),
                &mut vm
            )
            .unwrap(),
            Some(0.0)
        );
        execute(
            "setreputation",
            &[Expr::Number(faction.0 as f32), Expr::Number(80.0)],
            Some(actor),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute(
                "getreputation",
                &[Expr::Number(faction.0 as f32)],
                Some(actor),
                &mut vm
            )
            .unwrap(),
            Some(80.0)
        );
        execute(
            "modreputation",
            &[Expr::Number(faction.0 as f32), Expr::Number(10.0)],
            Some(actor),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute(
                "getreputation",
                &[Expr::Number(faction.0 as f32)],
                Some(actor),
                &mut vm
            )
            .unwrap(),
            Some(90.0)
        );
        assert_eq!(
            execute(
                "getreputationpct",
                &[Expr::Number(faction.0 as f32)],
                Some(actor),
                &mut vm
            )
            .unwrap(),
            Some(50.0)
        );
        assert_eq!(
            execute(
                "getreputationthreshold",
                &[Expr::Number(faction.0 as f32)],
                Some(actor),
                &mut vm
            )
            .unwrap(),
            Some(0.0)
        );
    }
}
