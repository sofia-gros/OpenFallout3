//! # AI・派閥・敵対・犯罪・同行者関連スクリプト関数
//! 参照元: references/openmw/components/esm4/script.hpp:61, 63, 71, 73, 122, 454, 455

use crate::parser::Expr;
use crate::vm::{ScriptVm, ScriptError};
use fo3_esm::FormId;

pub fn execute(
    cmd: &str,
    args: &[Expr],
    subject: Option<FormId>,
    vm: &mut ScriptVm,
) -> Result<Option<f32>, ScriptError> {
    let target = subject.unwrap_or(FormId(0x14));

    let get_form_id = |expr: &Expr| -> Result<FormId, ScriptError> {
        match expr {
            Expr::Number(n) => Ok(FormId(*n as u32)),
            Expr::Variable(v) => vm.resolve_form_id(v),
            _ => Err(ScriptError::InvalidArguments("Expected FormId".to_string())),
        }
    };

    match cmd {
        // SetPlayerTeammate [0/1]
        "setplayerteammate" => {
            if !args.is_empty() {
                let flag = vm.eval_ast_expr(&args[0], subject)? != 0.0;
                let key = format!("{:08X}.teammate", target.0);
                vm.locals.insert(key, if flag { 1.0 } else { 0.0 });
                println!("[Script] SetPlayerTeammate: Target {:?}, Teammate: {}", target, flag);
            }
            Ok(Some(0.0))
        }
        // FUN_GetPlayerTeammate = 454
        "getplayerteammate" => {
            let key = format!("{:08X}.teammate", target.0);
            let is_tm = vm.locals.get(&key).copied().unwrap_or(0.0) != 0.0;
            Ok(Some(if is_tm { 1.0 } else { 0.0 }))
        }
        // FUN_GetPlayerTeammateCount = 455
        "getplayerteammatecount" => {
            // 現在の仲間数を集計
            let count = vm.locals.iter().filter(|(k, &v)| k.ends_with(".teammate") && v != 0.0).count();
            Ok(Some(count as f32))
        }
        // FUN_GetInFaction = 71
        // GetInFaction [FactionFormID]
        "getinfaction" => {
            if !args.is_empty() {
                let faction_id = get_form_id(&args[0])?;
                let key = format!("{:08X}.faction.{:08X}", target.0, faction_id.0);
                let in_f = vm.locals.get(&key).copied().unwrap_or(0.0) != 0.0;
                return Ok(Some(if in_f { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // FUN_GetFactionRank = 73
        // GetFactionRank [FactionFormID]
        "getfactionrank" => {
            if !args.is_empty() {
                let faction_id = get_form_id(&args[0])?;
                let key = format!("{:08X}.factionrank.{:08X}", target.0, faction_id.0);
                let rank = vm.locals.get(&key).copied().unwrap_or(-1.0);
                return Ok(Some(rank));
            }
            Ok(Some(-1.0))
        }
        // SetFactionRank [FactionFormID] [Rank]
        "setfactionrank" => {
            if args.len() >= 2 {
                let faction_id = get_form_id(&args[0])?;
                let rank = vm.eval_ast_expr(&args[1], subject)?;
                let key_f = format!("{:08X}.faction.{:08X}", target.0, faction_id.0);
                let key_r = format!("{:08X}.factionrank.{:08X}", target.0, faction_id.0);
                vm.locals.insert(key_f, 1.0);
                vm.locals.insert(key_r, rank);
            }
            Ok(Some(0.0))
        }
        // ModFactionRank [FactionFormID] [Delta]
        "modfactionrank" => {
            if args.len() >= 2 {
                let faction_id = get_form_id(&args[0])?;
                let delta = vm.eval_ast_expr(&args[1], subject)?;
                let key_r = format!("{:08X}.factionrank.{:08X}", target.0, faction_id.0);
                let cur = vm.locals.get(&key_r).copied().unwrap_or(0.0);
                vm.locals.insert(key_r, cur + delta);
            }
            Ok(Some(0.0))
        }
        // SetEnemy [Faction1] [Faction2] [1=Enemy, 0=Neutral]
        "setenemy" => {
            if args.len() >= 2 {
                let f1 = get_form_id(&args[0])?;
                let f2 = get_form_id(&args[1])?;
                println!("[Script] SetEnemy: {:?} vs {:?}", f1, f2);
            }
            Ok(Some(0.0))
        }
        // SetAlly [Faction1] [Faction2]
        "setally" => {
            if args.len() >= 2 {
                let f1 = get_form_id(&args[0])?;
                let f2 = get_form_id(&args[1])?;
                println!("[Script] SetAlly: {:?} with {:?}", f1, f2);
            }
            Ok(Some(0.0))
        }
        // FUN_GetAlarmed = 61
        "getalarmed" => {
            Ok(Some(0.0))
        }
        // FUN_GetAttacked = 63
        "getattacked" => {
            Ok(Some(0.0))
        }
        // FUN_GetCrime = 122
        "getcrime" => {
            let gold = vm.globals.get("crimegold").copied().unwrap_or(0.0);
            Ok(Some(if gold > 0.0 { 1.0 } else { 0.0 }))
        }
        // SetCrimeGold [Amount]
        "setcrimegold" => {
            if !args.is_empty() {
                let gold = vm.eval_ast_expr(&args[0], subject)?;
                vm.globals.insert("crimegold".to_string(), gold);
            }
            Ok(Some(0.0))
        }
        // GetCrimeGold
        "getcrimegold" => {
            let gold = vm.globals.get("crimegold").copied().unwrap_or(0.0);
            Ok(Some(gold))
        }
        // PayFine [Faction] [0=GoToJail, 1=PayFine]
        "payfine" => {
            vm.globals.insert("crimegold".to_string(), 0.0);
            println!("[Script] PayFine executed");
            Ok(Some(0.0))
        }
        // StartCombat [EnemyRef]
        // 参照元: GECK: StartCombat
        "startcombat" => {
            if !args.is_empty() {
                let enemy = get_form_id(&args[0])?;
                vm.locals.insert(format!("{:08X}.incombat", target.0), 1.0);
                vm.locals.insert(format!("{:08X}.combattarget", target.0), enemy.0 as f32);
                vm.locals.insert(format!("{:08X}.incombat", enemy.0), 1.0);
                vm.locals.insert(format!("{:08X}.combattarget", enemy.0), target.0 as f32);
                println!("[Script] StartCombat: Target {:?} vs {:?}", target, enemy);
            }
            Ok(Some(0.0))
        }
        // StopCombat
        // 参照元: GECK: StopCombat
        "stopcombat" => {
            vm.locals.insert(format!("{:08X}.incombat", target.0), 0.0);
            vm.locals.insert(format!("{:08X}.combattarget", target.0), 0.0);
            println!("[Script] StopCombat: Target {:?}", target);
            Ok(Some(0.0))
        }
        // StopCombatAtOnce
        // 参照元: GECK: StopCombatAtOnce
        "stopcombatatonce" => {
            vm.locals.insert(format!("{:08X}.incombat", target.0), 0.0);
            vm.locals.insert(format!("{:08X}.combattarget", target.0), 0.0);
            println!("[Script] StopCombatAtOnce: Target {:?}", target);
            Ok(Some(0.0))
        }
        // GetInCombat / IsInCombat
        // 参照元: references/openmw/components/esm4/script.hpp:219 (FUN_IsInCombat = 289)
        "getincombat" | "isincombat" => {
            let key = format!("{:08X}.incombat", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // GetCombatTarget
        // 参照元: GECK: GetCombatTarget
        "getcombattarget" => {
            let key = format!("{:08X}.combattarget", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // IsCombatTarget [TargetRef]
        // 参照元: references/openmw/components/esm4/script.hpp:291 (FUN_IsCombatTarget = 515)
        "iscombattarget" => {
            if !args.is_empty() {
                let other_ref = get_form_id(&args[0])?;
                let key = format!("{:08X}.combattarget", target.0);
                let ct = vm.locals.get(&key).copied().unwrap_or(0.0);
                return Ok(Some(if ct == other_ref.0 as f32 { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // SetAlert [0/1]
        // 参照元: GECK: SetAlert
        "setalert" => {
            if !args.is_empty() {
                let flag = vm.eval_ast_expr(&args[0], subject)?;
                vm.locals.insert(format!("{:08X}.alert", target.0), flag);
                println!("[Script] SetAlert: Target {:?}, Flag {}", target, flag);
            }
            Ok(Some(0.0))
        }
        // GetAlert / GetIsAlerted
        // 参照元: references/openmw/components/esm4/script.hpp:124 (FUN_GetIsAlerted = 91)
        "getalert" | "getisalerted" => {
            let key = format!("{:08X}.alert", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // EvaluatePackage / EVP
        // 参照元: GECK: EvaluatePackage
        "evaluatepackage" | "evp" => {
            vm.locals.insert(format!("{:08X}.evp_requested", target.0), 1.0);
            println!("[Script] EvaluatePackage: Target {:?}", target);
            Ok(Some(0.0))
        }
        // AddScriptPackage [PackageRef]
        // 参照元: GECK: AddScriptPackage
        "addscriptpackage" => {
            if !args.is_empty() {
                let pkg = get_form_id(&args[0])?;
                vm.locals.insert(format!("{:08X}.scriptpackage", target.0), pkg.0 as f32);
                println!("[Script] AddScriptPackage: Target {:?}, Package {:?}", target, pkg);
            }
            Ok(Some(0.0))
        }
        // RemoveScriptPackage [PackageRef]
        // 参照元: GECK: RemoveScriptPackage
        "removescriptpackage" => {
            vm.locals.insert(format!("{:08X}.scriptpackage", target.0), 0.0);
            println!("[Script] RemoveScriptPackage: Target {:?}", target);
            Ok(Some(0.0))
        }
        // GetCurrentPackage / GetCurrentAIPackage
        // 参照元: references/openmw/components/esm4/script.hpp:134 (FUN_GetCurrentAIPackage = 110)
        "getcurrentpackage" | "getcurrentaipackage" => {
            let key = format!("{:08X}.currentpackage", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // GetIsCurrentPackage [PackageRef]
        // 参照元: references/openmw/components/esm4/script.hpp:168 (FUN_GetIsCurrentPackage = 161)
        "getiscurrentpackage" => {
            if !args.is_empty() {
                let expected_pkg = get_form_id(&args[0])?;
                let key = format!("{:08X}.currentpackage", target.0);
                let cur = vm.locals.get(&key).copied().unwrap_or(0.0);
                return Ok(Some(if cur == expected_pkg.0 as f32 { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // GetCurrentAIProcedure
        // 参照元: references/openmw/components/esm4/script.hpp:155 (FUN_GetCurrentAIProcedure = 143)
        "getcurrentaiprocedure" => {
            let key = format!("{:08X}.currentprocedure", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // StartPursuing [TargetRef]
        // 参照元: GECK: StartPursuing
        "startpursuing" => {
            if !args.is_empty() {
                let other_ref = get_form_id(&args[0])?;
                vm.locals.insert(format!("{:08X}.pursuing", target.0), other_ref.0 as f32);
                println!("[Script] StartPursuing: Target {:?} pursuing {:?}", target, other_ref);
            }
            Ok(Some(0.0))
        }
        // StopPursuing
        // 参照元: GECK: StopPursuing
        "stoppursuing" => {
            vm.locals.insert(format!("{:08X}.pursuing", target.0), 0.0);
            println!("[Script] StopPursuing: Target {:?}", target);
            Ok(Some(0.0))
        }
        // FUN_GetShouldAttack = 66
        // 参照元: references/openmw/components/esm4/script.hpp:108 (FUN_GetShouldAttack = 66)
        "getshouldattack" => {
            let incombat = vm.locals.get(&format!("{:08X}.incombat", target.0)).copied().unwrap_or(0.0);
            let alert = vm.locals.get(&format!("{:08X}.alert", target.0)).copied().unwrap_or(0.0);
            Ok(Some(if incombat != 0.0 || alert != 0.0 { 1.0 } else { 0.0 }))
        }
        // FUN_GetThreatRatio = 478
        // 参照元: references/openmw/components/esm4/script.hpp:282 (FUN_GetThreatRatio = 478)
        "getthreatratio" => {
            if !args.is_empty() {
                let other_ref = get_form_id(&args[0])?;
                let key = format!("{:08X}.threatratio.{:08X}", target.0, other_ref.0);
                let val = vm.locals.get(&key).copied().unwrap_or(1.0);
                return Ok(Some(val));
            }
            Ok(Some(1.0))
        }
        // FUN_GetFactionCombatReaction = 411
        // 参照元: references/openmw/components/esm4/script.hpp:256 (FUN_GetFactionCombatReaction = 411)
        "getfactioncombatreaction" => {
            Ok(Some(0.0))
        }
        // FUN_GetGroupMemberCount = 416
        // 参照元: references/openmw/components/esm4/script.hpp:258 (FUN_GetGroupMemberCount = 416)
        "getgroupmembercount" => {
            Ok(Some(1.0))
        }
        // FUN_GetGroupTargetCount = 417
        // 参照元: references/openmw/components/esm4/script.hpp:259 (FUN_GetGroupTargetCount = 417)
        "getgrouptargetcount" => {
            let incombat = vm.locals.get(&format!("{:08X}.incombat", target.0)).copied().unwrap_or(0.0);
            Ok(Some(if incombat != 0.0 { 1.0 } else { 0.0 }))
        }
        // FUN_GetDetected = 45
        // 参照元: references/openmw/components/esm4/script.hpp:92 (FUN_GetDetected = 45)
        "getdetected" => {
            if !args.is_empty() {
                let other_ref = get_form_id(&args[0])?;
                let key = format!("{:08X}.detected.{:08X}", target.0, other_ref.0);
                let val = vm.locals.get(&key).copied().unwrap_or(1.0);
                return Ok(Some(val));
            }
            Ok(Some(1.0))
        }
        // FUN_GetDetectionLevel = 180
        // 参照元: references/openmw/components/esm4/script.hpp:175 (FUN_GetDetectionLevel = 180)
        "getdetectionlevel" => {
            if !args.is_empty() {
                let other_ref = get_form_id(&args[0])?;
                let key = format!("{:08X}.detectionlevel.{:08X}", target.0, other_ref.0);
                let val = vm.locals.get(&key).copied().unwrap_or(3.0);
                return Ok(Some(val));
            }
            Ok(Some(3.0))
        }
        // SetUnconscious [0/1]
        // 参照元: GECK: SetUnconscious
        "setunconscious" => {
            if !args.is_empty() {
                let flag = vm.eval_ast_expr(&args[0], subject)?;
                vm.locals.insert(format!("{:08X}.unconscious", target.0), flag);
                println!("[Script] SetUnconscious: Target {:?}, Flag {}", target, flag);
            }
            Ok(Some(0.0))
        }
        // FUN_GetUnconscious = 242
        // 参照元: references/openmw/components/esm4/script.hpp:198 (FUN_GetUnconscious = 242)
        "getunconscious" => {
            let key = format!("{:08X}.unconscious", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // SetRestrained [0/1]
        // 参照元: GECK: SetRestrained
        "setrestrained" => {
            if !args.is_empty() {
                let flag = vm.eval_ast_expr(&args[0], subject)?;
                vm.locals.insert(format!("{:08X}.restrained", target.0), flag);
                println!("[Script] SetRestrained: Target {:?}, Flag {}", target, flag);
            }
            Ok(Some(0.0))
        }
        // FUN_GetRestrained = 244
        // 参照元: references/openmw/components/esm4/script.hpp:199 (FUN_GetRestrained = 244)
        "getrestrained" => {
            let key = format!("{:08X}.restrained", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // ResetAI
        // 参照元: GECK: ResetAI
        "resetai" => {
            vm.locals.insert(format!("{:08X}.incombat", target.0), 0.0);
            vm.locals.insert(format!("{:08X}.combattarget", target.0), 0.0);
            vm.locals.insert(format!("{:08X}.alert", target.0), 0.0);
            vm.locals.insert(format!("{:08X}.pursuing", target.0), 0.0);
            println!("[Script] ResetAI: Target {:?}", target);
            Ok(Some(0.0))
        }
        // SetIgnoreFriendlyHits [0/1]
        // 参照元: GECK: SetIgnoreFriendlyHits
        "setignorefriendlyhits" => {
            if !args.is_empty() {
                let flag = vm.eval_ast_expr(&args[0], subject)?;
                vm.locals.insert(format!("{:08X}.ignorefriendlyhits", target.0), flag);
            }
            Ok(Some(0.0))
        }
        // FUN_GetIgnoreFriendlyHits = 338
        // 参照元: references/openmw/components/esm4/script.hpp:234 (FUN_GetIgnoreFriendlyHits = 338)
        "getignorefriendlyhits" => {
            let key = format!("{:08X}.ignorefriendlyhits", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_combat_opcodes_extended() {
        let mut vm = ScriptVm::new();
        let actor1 = FormId(0x1001);
        let actor2 = FormId(0x1002);
        let pkg_id = FormId(0x3001);

        // Combat
        assert_eq!(execute("getincombat", &[], Some(actor1), &mut vm).unwrap(), Some(0.0));
        execute("startcombat", &[Expr::Number(actor2.0 as f32)], Some(actor1), &mut vm).unwrap();
        assert_eq!(execute("getincombat", &[], Some(actor1), &mut vm).unwrap(), Some(1.0));
        assert_eq!(execute("getcombattarget", &[], Some(actor1), &mut vm).unwrap(), Some(actor2.0 as f32));
        assert_eq!(execute("iscombattarget", &[Expr::Number(actor2.0 as f32)], Some(actor1), &mut vm).unwrap(), Some(1.0));

        execute("stopcombatatonce", &[], Some(actor1), &mut vm).unwrap();
        assert_eq!(execute("getincombat", &[], Some(actor1), &mut vm).unwrap(), Some(0.0));

        // Alert
        execute("setalert", &[Expr::Number(1.0)], Some(actor1), &mut vm).unwrap();
        assert_eq!(execute("getalert", &[], Some(actor1), &mut vm).unwrap(), Some(1.0));

        // Package
        execute("addscriptpackage", &[Expr::Number(pkg_id.0 as f32)], Some(actor1), &mut vm).unwrap();
        execute("evaluatepackage", &[], Some(actor1), &mut vm).unwrap();
        assert_eq!(vm.locals.get(&format!("{:08X}.evp_requested", actor1.0)).copied(), Some(1.0));

        // Pursuing
        execute("startpursuing", &[Expr::Number(actor2.0 as f32)], Some(actor1), &mut vm).unwrap();
        assert_eq!(vm.locals.get(&format!("{:08X}.pursuing", actor1.0)).copied(), Some(actor2.0 as f32));
        execute("stoppursuing", &[], Some(actor1), &mut vm).unwrap();
        assert_eq!(vm.locals.get(&format!("{:08X}.pursuing", actor1.0)).copied(), Some(0.0));

        // Unconscious & Restrained
        execute("setunconscious", &[Expr::Number(1.0)], Some(actor1), &mut vm).unwrap();
        assert_eq!(execute("getunconscious", &[], Some(actor1), &mut vm).unwrap(), Some(1.0));
        execute("setrestrained", &[Expr::Number(1.0)], Some(actor1), &mut vm).unwrap();
        assert_eq!(execute("getrestrained", &[], Some(actor1), &mut vm).unwrap(), Some(1.0));
    }
}
