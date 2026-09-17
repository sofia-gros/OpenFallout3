//! # アクター・NPC関連スクリプト関数
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
    let get_form_id = |expr: &Expr| -> Result<FormId, ScriptError> {
        match expr {
            Expr::Number(n) => Ok(FormId(*n as u32)),
            Expr::Variable(v) => vm.resolve_form_id(v),
            _ => Err(ScriptError::InvalidArguments("Expected FormId".to_string())),
        }
    };

    match cmd {
        // FUN_GetIsSex = 70 (0 = Male, 1 = Female)
        "getissex" => {
            if !args.is_empty() {
                let target_sex = vm.eval_ast_expr(&args[0], subject)? as u32;
                // デフォルト 0 (Male)
                return Ok(Some(if target_sex == 0 { 1.0 } else { 0.0 }));
            }
            Ok(Some(1.0))
        }
        // FUN_GetIsID = 72
        "getisid" => {
            if !args.is_empty() {
                let base_id = get_form_id(&args[0])?;
                if let Some(s) = subject {
                    return Ok(Some(if s == base_id { 1.0 } else { 0.0 }));
                }
            }
            Ok(Some(0.0))
        }
        // FUN_GetDead = 46
        "getdead" => {
            Ok(Some(0.0))
        }
        // Kill
        "kill" => {
            let target = subject.unwrap_or(FormId(0x14));
            println!("[Script] Kill: Target {:?}", target);
            Ok(Some(0.0))
        }
        // Resurrect
        "resurrect" => {
            let target = subject.unwrap_or(FormId(0x14));
            println!("[Script] Resurrect: Target {:?}", target);
            Ok(Some(0.0))
        }

        // FUN_GetDistance = 1
        "getdistance" => {
            if !args.is_empty() {
                let _target_ref = get_form_id(&args[0])?;
                // デフォルトで 500.0 単位の距離を返す
                return Ok(Some(500.0));
            }
            Ok(Some(0.0))
        }
        // FUN_GetLevel = 80
        "getlevel" => {
            Ok(Some(1.0))
        }
        // FUN_GetIsRace = 69
        "getisrace" => {
            if !args.is_empty() {
                let race_id = get_form_id(&args[0])?;
                if let Some(s) = subject {
                    return Ok(Some(if s == race_id { 1.0 } else { 0.0 }));
                }
            }
            Ok(Some(0.0))
        }
        // FUN_GetIsClass = 68
        "getisclass" => {
            if !args.is_empty() {
                let class_id = get_form_id(&args[0])?;
                if let Some(s) = subject {
                    return Ok(Some(if s == class_id { 1.0 } else { 0.0 }));
                }
            }
            Ok(Some(0.0))
        }
        // StartCombat [TargetRef]
        "startcombat" => {
            if !args.is_empty() {
                let enemy = get_form_id(&args[0])?;
                let caller = subject.unwrap_or(FormId(0x14));
                println!("[Script] StartCombat: Caller {:?} vs {:?}", caller, enemy);
            }
            Ok(Some(0.0))
        }
        // StopCombat
        "stopcombat" => {
            let caller = subject.unwrap_or(FormId(0x14));
            println!("[Script] StopCombat: Caller {:?}", caller);
            Ok(Some(0.0))
        }
        // FUN_IsInCombat = 289
        "isincombat" => {
            Ok(Some(0.0))
        }
        // EvaluatePackage / EVP
        "evp" | "evaluatepackage" => {
            let caller = subject.unwrap_or(FormId(0x14));
            println!("[Script] EvaluatePackage: Caller {:?}", caller);
            Ok(Some(0.0))
        }
        // ResetAI
        "resetai" => {
            let caller = subject.unwrap_or(FormId(0x14));
            println!("[Script] ResetAI: Caller {:?}", caller);
            Ok(Some(0.0))
        }
        // FUN_GetHeadingAngle = 99
        "getheadingangle" => {
            Ok(Some(0.0))
        }
        // LookAt [TargetRef]
        "lookat" => {
            if !args.is_empty() {
                let target_ref = get_form_id(&args[0])?;
                let caller = subject.unwrap_or(FormId(0x14));
                println!("[Script] LookAt: Caller {:?} looking at {:?}", caller, target_ref);
            }
            Ok(Some(0.0))
        }
        // SetGhost [0/1]
        // 対象のゴースト（無敵・物理すり抜け）フラグを設定
        "setghost" => {
            let caller = subject.unwrap_or(FormId(0x14));
            let flag = if !args.is_empty() {
                vm.eval_ast_expr(&args[0], subject)?
            } else {
                1.0
            };
            let key = format!("{:08X}.isghost", caller.0);
            vm.locals.insert(key, flag);
            println!("[Script] SetGhost: Target {:?}, Ghost: {}", caller, flag);
            Ok(Some(0.0))
        }
        // FUN_GetIsGhost = 237
        // ゴースト状態判定
        "getisghost" | "getghost" => {
            let caller = subject.unwrap_or(FormId(0x14));
            let key = format!("{:08X}.isghost", caller.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_IsEssential = 354
        // 不死属性（Essential）判定
        "isessential" => {
            let caller = subject.unwrap_or(FormId(0x14));
            let key = format!("{:08X}.essential", caller.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // SetEssential [BaseId] [0/1]
        // ベースアクターの不死属性を設定
        "setessential" => {
            if !args.is_empty() {
                let base_id = get_form_id(&args[0])?;
                let flag = if args.len() >= 2 {
                    vm.eval_ast_expr(&args[1], subject)?
                } else {
                    1.0
                };
                let key = format!("{:08X}.essential", base_id.0);
                vm.locals.insert(key, flag);
                println!("[Script] SetEssential: Base {:?}, Essential: {}", base_id, flag);
            }
            Ok(Some(0.0))
        }
        // FUN_IsActor = 353
        // アクター（NPC/クリーチャー）判定
        "isactor" => {
            Ok(Some(1.0))
        }
        // FUN_IsChild = 365
        // 子供判定
        "ischild" => {
            let caller = subject.unwrap_or(FormId(0x14));
            let key = format!("{:08X}.ischild", caller.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_GetPlayerTeammate = 454
        // プレイヤーのチームメイト判定 (エイリアス)
        "isplayerteammate" => {
            let caller = subject.unwrap_or(FormId(0x14));
            let key = format!("{:08X}.teammate", caller.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_GetDisposition = 76
        // 対象に対する好感度を取得 (0〜100, デフォルト 50)
        "getdisposition" => {
            if !args.is_empty() {
                let _target_ref = get_form_id(&args[0])?;
            }
            let caller = subject.unwrap_or(FormId(0x14));
            let key = format!("{:08X}.disposition", caller.0);
            let val = vm.locals.get(&key).copied().unwrap_or(50.0);
            Ok(Some(val))
        }
        // SetDisposition [TargetRef] [Value]
        // 好感度を設定
        "setdisposition" => {
            if args.len() >= 2 {
                let target_ref = get_form_id(&args[0])?;
                let val = vm.eval_ast_expr(&args[1], subject)?;
                let key = format!("{:08X}.disposition.{:08X}", subject.unwrap_or(FormId(0x14)).0, target_ref.0);
                vm.locals.insert(key, val);
            }
            Ok(Some(0.0))
        }
        // ModDisposition [TargetRef] [Delta]
        // 好感度を加算
        "moddisposition" => {
            if args.len() >= 2 {
                let target_ref = get_form_id(&args[0])?;
                let delta = vm.eval_ast_expr(&args[1], subject)?;
                let key = format!("{:08X}.disposition.{:08X}", subject.unwrap_or(FormId(0x14)).0, target_ref.0);
                let cur = vm.locals.get(&key).copied().unwrap_or(50.0);
                vm.locals.insert(key, cur + delta);
            }
            Ok(Some(0.0))
        }

        // FUN_GetSleeping = 49
        // 睡眠状態判定 (0: 起きている, 1: 睡眠中, 2: 睡眠中かつ目覚め不能)
        "getsleeping" => {
            let caller = subject.unwrap_or(FormId(0x14));
            let key = format!("{:08X}.sleeping", caller.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_IsSneaking = 286
        // スニーク中判定
        "issneaking" => {
            let caller = subject.unwrap_or(FormId(0x14));
            let key = format!("{:08X}.sneaking", caller.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_IsRunning = 287
        // 走行中判定
        "isrunning" => {
            let caller = subject.unwrap_or(FormId(0x14));
            let key = format!("{:08X}.running", caller.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_IsMoving = 25
        // 移動中判定
        "ismoving" => {
            let caller = subject.unwrap_or(FormId(0x14));
            let key = format!("{:08X}.moving", caller.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_IsTurning = 26
        // 旋回中判定
        "isturning" => {
            Ok(Some(0.0))
        }
        // FUN_IsWeaponOut = 101
        // 武器を構えているか判定
        "isweaponout" => {
            let caller = subject.unwrap_or(FormId(0x14));
            let key = format!("{:08X}.weaponout", caller.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // SetWeaponOut [0/1]
        // 武器の構え状態を設定
        "setweaponout" => {
            let caller = subject.unwrap_or(FormId(0x14));
            let flag = if !args.is_empty() {
                vm.eval_ast_expr(&args[0], subject)?
            } else {
                1.0
            };
            let key = format!("{:08X}.weaponout", caller.0);
            vm.locals.insert(key, flag);
            println!("[Script] SetWeaponOut: Target {:?}, Out: {}", caller, flag);
            Ok(Some(0.0))
        }
        // FUN_GetKnockedState = 107
        // ノックダウン/転倒状態コード (0: 通常, 1: 転倒中, 2: 仰向け, 3: 起き上がり中)
        "getknockedstate" => {
            let caller = subject.unwrap_or(FormId(0x14));
            let key = format!("{:08X}.knockedstate", caller.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_GetWeaponAnimType = 108
        // 武器アニメーション種別 (0: 素手, 1: 片手剣, 2: 両手剣, 3: 弓, 4: 片手銃, 5: 両手ライフル, 6: 重火器等)
        "getweaponanimtype" => {
            Ok(Some(0.0))
        }
        // FUN_IsGuard = 125
        // 衛兵NPC判定
        "isguard" => {
            let caller = subject.unwrap_or(FormId(0x14));
            let key = format!("{:08X}.isguard", caller.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_GetWalkSpeed = 142
        // 歩行速度取得 (デフォルト 100.0)
        "getwalkspeed" => {
            Ok(Some(100.0))
        }
        // FUN_GetIsCreature = 64
        // クリーチャー判定
        "getiscreature" => {
            let caller = subject.unwrap_or(FormId(0x14));
            let key = format!("{:08X}.iscreature", caller.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_GetIsCreatureType = 438
        // クリーチャー種別判定
        "getiscreaturetype" => {
            Ok(Some(0.0))
        }
        // FUN_IsActorEvil = 313
        // 悪人アクター判定
        "isactorevil" => {
            Ok(Some(0.0))
        }
        // FUN_IsActorAVictim = 314
        // 被害者アクター判定
        "isactoravictim" => {
            Ok(Some(0.0))
        }
        // FUN_GetIsPlayableRace = 254
        // プレイ可能種族判定
        "getisplayable" | "getisplayablerace" => {
            Ok(Some(1.0))
        }
        // FUN_GetIsVoiceType = 427
        // 音声タイプ一致判定
        "getisvoicetype" => {
            if !args.is_empty() {
                let voice_id = get_form_id(&args[0])?;
                let caller = subject.unwrap_or(FormId(0x14));
                let key = format!("{:08X}.voicetype", caller.0);
                if let Some(&v) = vm.locals.get(&key) {
                    return Ok(Some(if v as u32 == voice_id.0 { 1.0 } else { 0.0 }));
                }
            }
            Ok(Some(0.0))
        }
        // FUN_GetLineOfSight = 27
        // 視線が通っているか判定 (Line of Sight)
        "getlineofsight" | "getlos" => {
            if !args.is_empty() {
                let _target_ref = get_form_id(&args[0])?;
            }
            Ok(Some(1.0))
        }
        // FUN_GetDetected = 45
        // 対象を探知しているか判定
        "getdetected" => {
            if !args.is_empty() {
                let _target_ref = get_form_id(&args[0])?;
            }
            Ok(Some(0.0))
        }
        // FUN_GetDetectionLevel = 180
        // 探知レベル取得 (0: 未検知, 1: 警戒, 2: 捜索, 3: 発見)
        "getdetectionlevel" => {
            if !args.is_empty() {
                let _target_ref = get_form_id(&args[0])?;
            }
            Ok(Some(0.0))
        }
        // FUN_GetShouldAttack = 66
        // 攻撃すべきか判定
        "getshouldattack" => {
            Ok(Some(0.0))
        }
        // Dismount
        // 乗騎から降りる
        "dismount" => {
            let caller = subject.unwrap_or(FormId(0x14));
            println!("[Script] Dismount: Target {:?}", caller);
            Ok(Some(0.0))
        }
        // FUN_IsRidingHorse = 327
        // 乗馬中判定
        "isridinghorse" => {
            Ok(Some(0.0))
        }
        // FUN_GetActorsInHigh = 557
        // 高優先度処理アクター数取得
        "getactorsinhigh" => {
            Ok(Some(1.0))
        }
        // FUN_GetIgnoreFriendlyHits = 338
        // 味方誤射無視フラグ取得
        "getignorefriendlyhits" => {
            let caller = subject.unwrap_or(FormId(0x14));
            let key = format!("{:08X}.ignorefriendlyhits", caller.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // SetIgnoreFriendlyHits [0/1]
        // 味方誤射無視フラグ設定
        "setignorefriendlyhits" => {
            let caller = subject.unwrap_or(FormId(0x14));
            let flag = if !args.is_empty() {
                vm.eval_ast_expr(&args[0], subject)?
            } else {
                1.0
            };
            let key = format!("{:08X}.ignorefriendlyhits", caller.0);
            vm.locals.insert(key, flag);
            Ok(Some(0.0))
        }
        // FUN_IsPlayersLastRiddenHorse = 339
        // プレイヤーが最後に乗った馬か判定
        "isplayerslastriddenhorse" => {
            Ok(Some(0.0))
        }
        // FUN_GetTimeDead = 361
        // 死亡経過時間を取得
        "gettimedead" => {
            let caller = subject.unwrap_or(FormId(0x14));
            let key = format!("{:08X}.timedead", caller.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_GetCauseofDeath = 397
        // 死因コードを取得
        "getcauseofdeath" => {
            Ok(Some(0.0))
        }
        // FUN_IsLimbGone = 398
        // 指定部位の欠損判定
        "islimbgone" => {
            Ok(Some(0.0))
        }
        // FUN_HasFriendDisposition = 403
        // 友好関係判定
        "hasfrienddisposition" => {
            Ok(Some(1.0))
        }
        // FUN_IsKiller = 409
        // 対象を殺害したアクターか判定
        "iskiller" => {
            Ok(Some(0.0))
        }
        // FUN_IsKillerObject = 410
        // 対象を殺害したオブジェクトか判定
        "iskillerobject" => {
            Ok(Some(0.0))
        }
        // FUN_GetKillingBlowLimb = 496
        // とどめを刺した部位コードを取得
        "getkillingblowlimb" => {
            Ok(Some(0.0))
        }
        // FUN_GetConcussed = 489
        // 脳震盪状態判定
        "getconcussed" => {
            Ok(Some(0.0))
        }
        // FUN_IsCombatTarget = 515
        // 戦闘対象判定
        "iscombattarget" => {
            Ok(Some(0.0))
        }
        // FUN_IsInCriticalStage = 531
        // クリティカル分解/溶解状態判定
        "isincriticalstage" => {
            Ok(Some(0.0))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// アクター関連オペコードの実行テスト
    #[test]
    fn test_actor_opcodes_extended() {
        let mut vm = ScriptVm::new();
        let actor = FormId(0x1000);

        // ゴースト属性のテスト
        assert_eq!(execute("getisghost", &[], Some(actor), &mut vm).unwrap(), Some(0.0));
        execute("setghost", &[Expr::Number(1.0)], Some(actor), &mut vm).unwrap();
        assert_eq!(execute("getisghost", &[], Some(actor), &mut vm).unwrap(), Some(1.0));
        assert_eq!(execute("getghost", &[], Some(actor), &mut vm).unwrap(), Some(1.0));

        // 不死（Essential）判定のテスト
        assert_eq!(execute("isessential", &[], Some(actor), &mut vm).unwrap(), Some(0.0));
        execute("setessential", &[Expr::Number(actor.0 as f32), Expr::Number(1.0)], None, &mut vm).unwrap();
        assert_eq!(execute("isessential", &[], Some(actor), &mut vm).unwrap(), Some(1.0));

        // アクター、子供、チームメイト判定
        assert_eq!(execute("isactor", &[], Some(actor), &mut vm).unwrap(), Some(1.0));
        assert_eq!(execute("ischild", &[], Some(actor), &mut vm).unwrap(), Some(0.0));
        assert_eq!(execute("isplayerteammate", &[], Some(actor), &mut vm).unwrap(), Some(0.0));

        // 好感度（Disposition）のテスト
        assert_eq!(execute("getdisposition", &[], Some(actor), &mut vm).unwrap(), Some(50.0));
        let target_npc = FormId(0x2000);
        execute("setdisposition", &[Expr::Number(target_npc.0 as f32), Expr::Number(75.0)], Some(actor), &mut vm).unwrap();
        execute("moddisposition", &[Expr::Number(target_npc.0 as f32), Expr::Number(10.0)], Some(actor), &mut vm).unwrap();

        // 状態判定（睡眠、スニーク、走行、移動、武器構え）
        assert_eq!(execute("getsleeping", &[], Some(actor), &mut vm).unwrap(), Some(0.0));
        assert_eq!(execute("issneaking", &[], Some(actor), &mut vm).unwrap(), Some(0.0));
        assert_eq!(execute("isrunning", &[], Some(actor), &mut vm).unwrap(), Some(0.0));
        assert_eq!(execute("ismoving", &[], Some(actor), &mut vm).unwrap(), Some(0.0));
        assert_eq!(execute("isweaponout", &[], Some(actor), &mut vm).unwrap(), Some(0.0));
        execute("setweaponout", &[Expr::Number(1.0)], Some(actor), &mut vm).unwrap();
        assert_eq!(execute("isweaponout", &[], Some(actor), &mut vm).unwrap(), Some(1.0));

        // 誤射無視フラグ
        assert_eq!(execute("getignorefriendlyhits", &[], Some(actor), &mut vm).unwrap(), Some(0.0));
        execute("setignorefriendlyhits", &[Expr::Number(1.0)], Some(actor), &mut vm).unwrap();
        assert_eq!(execute("getignorefriendlyhits", &[], Some(actor), &mut vm).unwrap(), Some(1.0));

        // その他の判定
        assert_eq!(execute("getknockedstate", &[], Some(actor), &mut vm).unwrap(), Some(0.0));
        assert_eq!(execute("isguard", &[], Some(actor), &mut vm).unwrap(), Some(0.0));
        assert_eq!(execute("getisplayable", &[], Some(actor), &mut vm).unwrap(), Some(1.0));
        assert_eq!(execute("getactorsinhigh", &[], Some(actor), &mut vm).unwrap(), Some(1.0));
        assert_eq!(execute("hasfrienddisposition", &[], Some(actor), &mut vm).unwrap(), Some(1.0));
        assert_eq!(execute("iscombattarget", &[], Some(actor), &mut vm).unwrap(), Some(0.0));
    }
}
