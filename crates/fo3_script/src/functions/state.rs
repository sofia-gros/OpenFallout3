//! # オブジェクト・ワールド状態関連スクリプト関数
//! 参照元: references/openmw/components/esm4/script.hpp

use crate::parser::Expr;
use crate::vm::{ScriptError, ScriptVm};
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
        // Enable
        "enable" => {
            if let Some(t) = subject {
                vm.disabled_objects.remove(&t);
                vm.enabled_objects.insert(t);
            }
            Ok(Some(0.0))
        }
        // Disable
        "disable" => {
            if let Some(t) = subject {
                vm.enabled_objects.remove(&t);
                vm.disabled_objects.insert(t);
            }
            Ok(Some(0.0))
        }
        // FUN_GetDisabled = 35
        "getdisabled" => {
            if let Some(t) = subject {
                let disabled = vm.disabled_objects.contains(&t);
                return Ok(Some(if disabled { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // Unlock
        "unlock" => {
            if let Some(t) = subject {
                vm.unlocked_objects.insert(t);
            }
            Ok(Some(0.0))
        }
        // Lock
        "lock" => {
            if let Some(t) = subject {
                vm.unlocked_objects.remove(&t);
            }
            Ok(Some(0.0))
        }
        // FUN_GetLocked = 5
        "getlocked" => {
            if let Some(t) = subject {
                let locked = !vm.is_unlocked(t);
                return Ok(Some(if locked { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // FUN_GetLockLevel = 65
        "getlocklevel" => {
            // デフォルト 0 (Very Easy / 鍵なし)
            Ok(Some(0.0))
        }
        // FUN_GetOpenState = 157 (1=Open, 2=Opening, 3=Closed, 4=Closing)
        "getopenstate" => {
            if let Some(t) = subject {
                if vm.is_unlocked(t) {
                    return Ok(Some(1.0)); // Open
                }
            }
            Ok(Some(3.0)) // Closed
        }
        // SetOpenState [0=Close, 1=Open]
        "setopenstate" => {
            if !args.is_empty() {
                let state = vm.eval_ast_expr(&args[0], subject)? as u32;
                if let Some(t) = subject {
                    if state == 1 {
                        vm.unlocked_objects.insert(t);
                    } else {
                        vm.unlocked_objects.remove(&t);
                    }
                }
            }
            Ok(Some(0.0))
        }
        // FUN_GetScale = 24
        "getscale" => Ok(Some(1.0)),
        // SetScale [Scale]
        "setscale" => {
            if !args.is_empty() {
                let _scale = vm.eval_ast_expr(&args[0], subject)?;
            }
            Ok(Some(0.0))
        }
        // FUN_GetDestroyed = 203
        "getdestroyed" => Ok(Some(0.0)),
        // SetDestroyed [1 or 0]
        "setdestroyed" => Ok(Some(0.0)),
        // PlaySound [SoundEDID]
        "playsound" => {
            if !args.is_empty() {
                let s_id = if let Expr::Variable(ref name) = args[0] {
                    name.clone()
                } else {
                    format!("{:08X}", get_form_id(&args[0]).unwrap_or(fo3_esm::FormId(0)).0)
                };
                println!("[Script] PlaySound: {:?}", s_id);
                vm.sound_queue.push(s_id);
            }
            Ok(Some(0.0))
        }
        // ShowMessage [MessageEDID]
        "showmessage" => {
            if !args.is_empty() {
                if let Expr::Variable(ref name) = args[0] {
                    vm.show_messages.push(name.clone());
                } else if let Ok(fid) = get_form_id(&args[0]) {
                    vm.show_messages.push(format!("{:08X}", fid.0));
                }
            }
            Ok(Some(0.0))
        }
        // FUN_GetMenuMode = 36
        "getmenumode" => {
            // 0 = 通常ゲームプレイモード, 1 = メニューモード
            Ok(Some(0.0))
        }
        // Activate [ActionRef] [DoDefault]
        // 参照元: GECK Wiki `Activate`, references/openmw/components/esm4/script.hpp
        // オブジェクトのアクティベートを実行する
        "activate" => {
            if !args.is_empty() {
                let _ = get_form_id(&args[0]);
            }
            Ok(Some(1.0))
        }
        // FUN_IsActionRef / IsActionRef [ActorRef]
        // 参照元: GECK Wiki `IsActionRef`
        // イベント発火元が指定されたアクター/参照と一致するか判定する
        "isactionref" => {
            if !args.is_empty() {
                let expected = get_form_id(&args[0]).unwrap_or(FormId(0x14));
                // デフォルトではプレイヤー (0x14) によるアクションを想定
                let is_player = expected == FormId(0x14);
                return Ok(Some(if is_player { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // FUN_GetDestructionStage = 471
        // 参照元: references/openmw/components/esm4/script.hpp:280
        // オブジェクトの現在の破壊段階を取得する (0 = 未破壊)
        "getdestructablestate" | "getdestructionstage" => Ok(Some(0.0)),
        // SetDestructionStage [Stage]
        // 参照元: GECK Wiki `SetDestructionStage`
        // オブジェクトの破壊段階を設定する
        "setdestructablestate" | "setdestructionstage" => {
            if !args.is_empty() {
                let _ = vm.eval_ast_expr(&args[0], subject)?;
            }
            Ok(Some(0.0))
        }
        // DamageObject [DamageAmount]
        // 参照元: GECK Wiki `DamageObject`
        // オブジェクトの耐久力にダメージを与える
        "damageobject" => {
            if !args.is_empty() {
                let _ = vm.eval_ast_expr(&args[0], subject)?;
            }
            Ok(Some(0.0))
        }
        // ResetHealth
        // 参照元: GECK Wiki `ResetHealth`
        // オブジェクト/アクターのヘルスを最大値にリセットする
        "resethealth" => Ok(Some(0.0)),
        // FUN_IsOwner = 278
        // 参照元: references/openmw/components/esm4/script.hpp:166
        // オブジェクトの所有者が指定されたアクターまたはファクションか判定する
        "isowner" => {
            if !args.is_empty() {
                let _ = get_form_id(&args[0]);
            }
            Ok(Some(0.0))
        }
        // FUN_IsCellOwner = 280
        // 参照元: references/openmw/components/esm4/script.hpp:167
        // 現在のセルの所有者が指定されたアクターまたはファクションか判定する
        "iscellowner" => {
            if !args.is_empty() {
                let _ = get_form_id(&args[0]);
            }
            Ok(Some(0.0))
        }
        // FUN_IsInInterior = 300
        // 参照元: references/openmw/components/esm4/script.hpp:174
        // 現在のセルが室内（インテリア）かどうか判定する (1.0 = 室内, 0.0 = 屋外)
        "isininterior" => Ok(Some(0.0)),
        // FUN_GetInWorldspace = 310
        // 参照元: references/openmw/components/esm4/script.hpp:178
        // オブジェクトが指定されたワールドスペース内に存在するか判定する
        "getinworldspace" => {
            if !args.is_empty() {
                let _ = get_form_id(&args[0]);
            }
            Ok(Some(0.0))
        }
        // FUN_GetInCell = 67
        // 参照元: references/openmw/components/esm4/script.hpp:109
        // オブジェクトが指定されたセル内に存在するか判定する
        "getincell" => {
            if !args.is_empty() {
                let _ = get_form_id(&args[0]);
            }
            Ok(Some(0.0))
        }
        // FUN_GetIsInSameCell = 32
        // 参照元: references/openmw/components/esm4/script.hpp:83
        // 対象オブジェクトと同一のセル内に存在するか判定する
        "getisinsamecell" => {
            if !args.is_empty() {
                let _ = get_form_id(&args[0]);
            }
            Ok(Some(1.0))
        }
        // FUN_GetInZone = 446
        // 参照元: references/openmw/components/esm4/script.hpp:269
        // 指定されたエンカウントゾーン内に存在するか判定する
        "getinzone" => {
            if !args.is_empty() {
                let _ = get_form_id(&args[0]);
            }
            Ok(Some(0.0))
        }
        // FUN_GetDefaultOpen = 215
        // 参照元: references/openmw/components/esm4/script.hpp:161
        // ドアなどのオブジェクトがデフォルトで開いた状態かを判定する
        "getdefaultopen" => Ok(Some(0.0)),
        // SetDefaultOpen [0/1]
        // 参照元: GECK Wiki `SetDefaultOpen`
        // ドアなどのオブジェクトのデフォルト開閉状態を設定する
        "setdefaultopen" => {
            if !args.is_empty() {
                let _ = vm.eval_ast_expr(&args[0], subject)?;
            }
            Ok(Some(0.0))
        }
        // FUN_IsRaining = 62
        // 参照元: references/openmw/components/esm4/script.hpp:104
        // 現在雨が降っているかどうか判定する
        "israining" => Ok(Some(0.0)),
        // FUN_IsSnowing = 75
        // 参照元: references/openmw/components/esm4/script.hpp:117
        // 現在雪が降っているかどうか判定する
        "issnowing" => Ok(Some(0.0)),
        // FUN_IsCloudy = 267
        // 参照元: references/openmw/components/esm4/script.hpp:164
        // 現在の天候が曇りかどうか判定する
        "iscloudy" => Ok(Some(0.0)),
        // FUN_IsPleasant = 266
        // 参照元: references/openmw/components/esm4/script.hpp:163
        // 現在の天候が快晴・心地よい天気かどうか判定する
        "ispleasant" => Ok(Some(1.0)),
        // FUN_GetCurrentWeatherPercent = 148
        // 参照元: references/openmw/components/esm4/script.hpp:148
        // 現在の天候への遷移率 (0.0〜1.0) を取得する
        "getcurrentweatherpercent" => Ok(Some(1.0)),
        // FUN_GetIsCurrentWeather = 149
        // 参照元: references/openmw/components/esm4/script.hpp:149
        // 現在の天候が指定された天候IDと一致するか判定する
        "getiscurrentweather" => {
            if !args.is_empty() {
                let _ = get_form_id(&args[0]);
            }
            Ok(Some(1.0))
        }
        // FUN_GetIsObjectType = 433
        // 参照元: references/openmw/components/esm4/script.hpp:265
        // オブジェクトのタイプが指定種別と一致するか判定する
        "getisobjecttype" => {
            if !args.is_empty() {
                let _ = vm.eval_ast_expr(&args[0], subject)?;
            }
            Ok(Some(1.0))
        }
        // FUN_GetIsReference = 136
        // 参照元: references/openmw/components/esm4/script.hpp:136
        // オブジェクトの参照IDが指定されたRefと一致するか判定する
        "getisreference" => {
            if !args.is_empty() {
                let target_ref = get_form_id(&args[0])?;
                if let Some(s) = subject {
                    return Ok(Some(if s == target_ref { 1.0 } else { 0.0 }));
                }
            }
            Ok(Some(0.0))
        }
        // FUN_HasLoaded3D = 558
        // 参照元: references/openmw/components/esm4/script.hpp:298
        // オブジェクトの3Dメッシュがメモリ上にロードされているか判定する
        "hasloaded3d" => Ok(Some(1.0)),
        // FUN_GetIsLockBroken = 522
        // 参照元: references/openmw/components/esm4/script.hpp:292
        // ドアやコンテナの鍵が破壊されているか判定する
        "getislockbroken" => Ok(Some(0.0)),
        // FUN_GetUnconscious = 242
        // 参照元: references/openmw/components/esm4/script.hpp:242
        // 対象が気絶状態にあるか判定する
        "getunconscious" => Ok(Some(0.0)),
        // SetUnconscious [0/1]
        // 参照元: GECK Wiki `SetUnconscious`
        // 対象の気絶状態を設定する
        "setunconscious" => {
            if !args.is_empty() {
                let _ = vm.eval_ast_expr(&args[0], subject)?;
            }
            Ok(Some(0.0))
        }
        // FUN_GetRestrained = 244
        // 参照元: references/openmw/components/esm4/script.hpp:244
        // 対象が拘束状態にあるか判定する
        "getrestrained" => Ok(Some(0.0)),
        // SetRestrained [0/1]
        // 参照元: GECK Wiki `SetRestrained`
        // 対象の拘束状態を設定する
        "setrestrained" => {
            if !args.is_empty() {
                let _ = vm.eval_ast_expr(&args[0], subject)?;
            }
            Ok(Some(0.0))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_opcodes() {
        let mut vm = ScriptVm::new();
        let player = FormId(0x14);
        let obj = FormId(0x1000);

        // enable / disable / getdisabled
        assert_eq!(
            execute("getdisabled", &[], Some(obj), &mut vm).unwrap(),
            Some(0.0)
        );
        execute("disable", &[], Some(obj), &mut vm).unwrap();
        assert_eq!(
            execute("getdisabled", &[], Some(obj), &mut vm).unwrap(),
            Some(1.0)
        );
        execute("enable", &[], Some(obj), &mut vm).unwrap();
        assert_eq!(
            execute("getdisabled", &[], Some(obj), &mut vm).unwrap(),
            Some(0.0)
        );

        // lock / unlock / getlocked
        assert_eq!(
            execute("getlocked", &[], Some(obj), &mut vm).unwrap(),
            Some(1.0)
        );
        execute("unlock", &[], Some(obj), &mut vm).unwrap();
        assert_eq!(
            execute("getlocked", &[], Some(obj), &mut vm).unwrap(),
            Some(0.0)
        );

        // activate / isactionref
        assert_eq!(
            execute("activate", &[], Some(obj), &mut vm).unwrap(),
            Some(1.0)
        );
        assert_eq!(
            execute(
                "isactionref",
                &[Expr::Number(player.0 as f32)],
                Some(obj),
                &mut vm
            )
            .unwrap(),
            Some(1.0)
        );
        assert_eq!(
            execute(
                "isactionref",
                &[Expr::Number(0x9999 as f32)],
                Some(obj),
                &mut vm
            )
            .unwrap(),
            Some(0.0)
        );

        // destruction / health
        assert_eq!(
            execute("getdestructablestate", &[], Some(obj), &mut vm).unwrap(),
            Some(0.0)
        );
        assert_eq!(
            execute("damageobject", &[Expr::Number(50.0)], Some(obj), &mut vm).unwrap(),
            Some(0.0)
        );
        assert_eq!(
            execute("resethealth", &[], Some(obj), &mut vm).unwrap(),
            Some(0.0)
        );

        // world / cell / weather
        assert_eq!(
            execute("isininterior", &[], Some(obj), &mut vm).unwrap(),
            Some(0.0)
        );
        assert_eq!(
            execute(
                "getisinsamecell",
                &[Expr::Number(player.0 as f32)],
                Some(obj),
                &mut vm
            )
            .unwrap(),
            Some(1.0)
        );
        assert_eq!(
            execute("ispleasant", &[], None, &mut vm).unwrap(),
            Some(1.0)
        );
        assert_eq!(execute("israining", &[], None, &mut vm).unwrap(), Some(0.0));

        // references / load
        assert_eq!(
            execute(
                "getisreference",
                &[Expr::Number(obj.0 as f32)],
                Some(obj),
                &mut vm
            )
            .unwrap(),
            Some(1.0)
        );
        assert_eq!(
            execute(
                "getisreference",
                &[Expr::Number(player.0 as f32)],
                Some(obj),
                &mut vm
            )
            .unwrap(),
            Some(0.0)
        );
        assert_eq!(
            execute("hasloaded3d", &[], Some(obj), &mut vm).unwrap(),
            Some(1.0)
        );
        assert_eq!(
            execute("getunconscious", &[], Some(obj), &mut vm).unwrap(),
            Some(0.0)
        );
    }
}
