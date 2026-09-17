//! # プレイヤー制御・カメラ・UI関連スクリプト関数
//! 参照元: references/openmw/components/esm4/script.hpp:98
//! 参照元: GECK DisablePlayerControls, EnablePlayerControls, SetInCharGen

use crate::parser::Expr;
use crate::vm::{ScriptVm, ScriptError};
use fo3_esm::FormId;

pub fn execute(
    cmd: &str,
    args: &[Expr],
    _subject: Option<FormId>,
    vm: &mut ScriptVm,
) -> Result<Option<f32>, ScriptError> {
    match cmd {
        // DisablePlayerControls [Movement] [Pipboy] [Fighting] [POV] [Looking] [Sneaking]
        "disableplayercontrols" => {
            vm.player_controls_enabled = false;
            // 引数フラグをパース
            if !args.is_empty() {
                if let Ok(v) = vm.eval_ast_expr(&args[0], None) {
                    vm.player_controls.movement = v == 0.0;
                }
            } else {
                vm.player_controls.movement = false;
                vm.player_controls.pipboy = false;
                vm.player_controls.fight = false;
                vm.player_controls.pov = false;
                vm.player_controls.looking = false;
                // vm.player_controls.sneaking
            }
            println!("[Script] DisablePlayerControls applied");
            Ok(Some(0.0))
        }
        // EnablePlayerControls
        "enableplayercontrols" => {
            vm.player_controls_enabled = true;
            vm.player_controls.movement = true;
            vm.player_controls.pipboy = true;
            vm.player_controls.fight = true;
            vm.player_controls.pov = true;
            vm.player_controls.looking = true;
            // vm.player_controls.sneaking
            println!("[Script] EnablePlayerControls applied");
            Ok(Some(0.0))
        }
        // FUN_GetPlayerControlsDisabled = 98
        "getplayercontrolsdisabled" => {
            let disabled = !vm.player_controls_enabled;
            Ok(Some(if disabled { 1.0 } else { 0.0 }))
        }
        // SetInCharGen [0/1]
        "setinchargen" => {
            if !args.is_empty() {
                let flag = vm.eval_ast_expr(&args[0], None)? != 0.0;
                vm.in_chargen = flag;
            } else {
                vm.in_chargen = true;
            }
            Ok(Some(0.0))
        }
        // GetButtonPressed
        "getbuttonpressed" => {
            let btn = vm.last_button_pressed.take().unwrap_or(-1) as f32;
            Ok(Some(btn))
        }
        // ShowRaceMenu
        "showracemenu" => {
            vm.chargen_events.push("ShowRaceMenu".to_string());
            vm.chargen_menu_active = true;
            Ok(Some(0.0))
        }
        // ShowBarterMenu
        "showbartermenu" => {
            vm.show_messages.push("[BarterMenu]".to_string());
            Ok(Some(0.0))
        }
        // ShowRepairMenu
        "showrepairmenu" => {
            vm.show_messages.push("[RepairMenu]".to_string());
            Ok(Some(0.0))
        }
        // ShowWarning [Text]
        "showwarning" => {
            if !args.is_empty() {
                if let Expr::Variable(ref s) = args[0] {
                    vm.show_messages.push(format!("[Warning] {}", s));
                }
            }
            Ok(Some(0.0))
        }
        // FUN_IsPCSleeping = 175
        // 参照元: references/openmw/components/esm4/script.hpp:108
        // プレイヤーが睡眠中かどうか判定する (1.0 = 睡眠中, 0.0 = 起床中)
        "ispcsleeping" => {
            let sleeping = vm.locals.get("player_sleeping").copied().unwrap_or(0.0) != 0.0;
            Ok(Some(if sleeping { 1.0 } else { 0.0 }))
        }
        // DisableVanityMode
        // 参照元: GECK Wiki `DisableVanityMode`
        // 放置時の三人称自動カメラ回転（バニティモード）を無効化する
        "disablevanitymode" => {
            vm.player_controls.cam_switch = false;
            println!("[Script] DisableVanityMode applied");
            Ok(Some(0.0))
        }
        // EnableVanityMode
        // 参照元: GECK Wiki `EnableVanityMode`
        // バニティモードを再度有効化する
        "enablevanitymode" => {
            vm.player_controls.cam_switch = true;
            println!("[Script] EnableVanityMode applied");
            Ok(Some(0.0))
        }
        // FUN_IsPC1stPerson = 392
        // 参照元: references/openmw/components/esm4/script.hpp:248
        // プレイヤーが一人称視点かどうか判定する (1.0 = 一人称, 0.0 = 三人称)
        "ispc1stperson" => {
            let first_person = vm.locals.get("player_1st_person").copied().unwrap_or(1.0);
            Ok(Some(first_person))
        }
        // ForceFirstPerson
        // 参照元: GECK Wiki `ForceFirstPerson`
        // カメラを一人称視点に強制切り替えする
        "forcefirstperson" => {
            vm.locals.insert("player_1st_person".into(), 1.0);
            println!("[Script] ForceFirstPerson applied");
            Ok(Some(0.0))
        }
        // ForceThirdPerson
        // 参照元: GECK Wiki `ForceThirdPerson`
        // カメラを三人称視点に強制切り替えする
        "forcethirdperson" => {
            vm.locals.insert("player_1st_person".into(), 0.0);
            println!("[Script] ForceThirdPerson applied");
            Ok(Some(0.0))
        }
        // ToggleCamera
        // 参照元: GECK Wiki `ToggleCamera`
        // 一人称視点と三人称視点をトグル切り替えする
        "togglecamera" => {
            let cur = vm.locals.get("player_1st_person").copied().unwrap_or(1.0);
            let next = if cur == 1.0 { 0.0 } else { 1.0 };
            vm.locals.insert("player_1st_person".into(), next);
            println!("[Script] ToggleCamera applied: 1stPerson = {}", next);
            Ok(Some(0.0))
        }
        // TFC / ToggleFreeCamera
        // 参照元: GECK Wiki `TFC`
        // フリーカメラモードのトグル切り替え
        "tfc" | "con_tfc" => {
            let cur = vm.locals.get("free_camera_enabled").copied().unwrap_or(0.0);
            let next = if cur == 1.0 { 0.0 } else { 1.0 };
            vm.locals.insert("free_camera_enabled".into(), next);
            println!("[Script] TFC applied: FreeCamera = {}", next);
            Ok(Some(0.0))
        }
        // FUN_GetVATSMode = 224
        // 参照元: references/openmw/components/esm4/script.hpp:160
        // 現在のVATSモード状態を取得する (0 = なし, 1 = モード中など)
        "getvatsmode" => {
            let vats = vm.locals.get("vats_mode").copied().unwrap_or(0.0);
            Ok(Some(vats))
        }
        // FUN_GetPCMiscStat = 312
        // 参照元: references/openmw/components/esm4/script.hpp:179
        // プレイヤーの各種実績・統計値を取得する
        "getpcmiscstat" => {
            if !args.is_empty() {
                let stat_val = vm.eval_ast_expr(&args[0], _subject)?;
                let key = format!("pcmiscstat_{}", stat_val as u32);
                let val = vm.locals.get(&key).copied().unwrap_or(0.0);
                return Ok(Some(val));
            }
            Ok(Some(0.0))
        }
        // ShowNameMenu
        // 参照元: GECK Wiki `ShowNameMenu`
        // プレイヤーの名前入力メニューを表示する
        "shownamemenu" => {
            vm.chargen_events.push("shownamemenu".into());
            vm.chargen_menu_active = true;
            println!("[Script] ShowNameMenu requested");
            Ok(Some(0.0))
        }
        // ShowClassMenu
        // 参照元: GECK Wiki `ShowClassMenu`
        // クラス選択メニューを表示する
        "showclassmenu" => {
            vm.chargen_events.push("showclassmenu".into());
            vm.chargen_menu_active = true;
            println!("[Script] ShowClassMenu requested");
            Ok(Some(0.0))
        }
        // ShowSpecialBookMenu
        // 参照元: GECK Wiki `ShowSpecialBookMenu`
        // 赤ん坊の絵本（SPECIALステータス配分）メニューを表示する
        "showspecialbookmenu" => {
            vm.chargen_events.push("showspecialbookmenu".into());
            vm.chargen_menu_active = true;
            println!("[Script] ShowSpecialBookMenu requested");
            Ok(Some(0.0))
        }
        // FUN_WhichServiceMenu = 323
        // 参照元: references/openmw/components/esm4/script.hpp:184
        // 現在開かれているサービスメニューの種類を取得する (0 = なし)
        "whichservicemenu" => {
            Ok(Some(0.0))
        }
        // FUN_IsPlayerGrabbedRef = 464
        // 参照元: references/openmw/components/esm4/script.hpp:276
        // オブジェクトが現在プレイヤーによって掴まれている (Zキー把持) か判定する
        "isplayergrabbedref" => {
            Ok(Some(0.0))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_opcodes() {
        let mut vm = ScriptVm::new();

        // disable / enable controls
        execute("disableplayercontrols", &[], None, &mut vm).unwrap();
        assert_eq!(execute("getplayercontrolsdisabled", &[], None, &mut vm).unwrap(), Some(1.0));
        execute("enableplayercontrols", &[], None, &mut vm).unwrap();
        assert_eq!(execute("getplayercontrolsdisabled", &[], None, &mut vm).unwrap(), Some(0.0));

        // vanity mode
        execute("disablevanitymode", &[], None, &mut vm).unwrap();
        assert!(!vm.player_controls.cam_switch);
        execute("enablevanitymode", &[], None, &mut vm).unwrap();
        assert!(vm.player_controls.cam_switch);

        // 1st / 3rd person camera
        assert_eq!(execute("ispc1stperson", &[], None, &mut vm).unwrap(), Some(1.0));
        execute("forcethirdperson", &[], None, &mut vm).unwrap();
        assert_eq!(execute("ispc1stperson", &[], None, &mut vm).unwrap(), Some(0.0));
        execute("forcefirstperson", &[], None, &mut vm).unwrap();
        assert_eq!(execute("ispc1stperson", &[], None, &mut vm).unwrap(), Some(1.0));
        execute("togglecamera", &[], None, &mut vm).unwrap();
        assert_eq!(execute("ispc1stperson", &[], None, &mut vm).unwrap(), Some(0.0));

        // free camera (tfc)
        execute("tfc", &[], None, &mut vm).unwrap();
        assert_eq!(vm.locals.get("free_camera_enabled"), Some(&1.0));

        // chargen menus
        execute("shownamemenu", &[], None, &mut vm).unwrap();
        assert!(vm.chargen_events.contains(&"shownamemenu".to_string()));
        execute("showclassmenu", &[], None, &mut vm).unwrap();
        assert!(vm.chargen_events.contains(&"showclassmenu".to_string()));
        execute("showspecialbookmenu", &[], None, &mut vm).unwrap();
        assert!(vm.chargen_events.contains(&"showspecialbookmenu".to_string()));

        // sleeping / vats / menus
        assert_eq!(execute("ispcsleeping", &[], None, &mut vm).unwrap(), Some(0.0));
        assert_eq!(execute("getvatsmode", &[], None, &mut vm).unwrap(), Some(0.0));
        assert_eq!(execute("whichservicemenu", &[], None, &mut vm).unwrap(), Some(0.0));
        assert_eq!(execute("isplayergrabbedref", &[], None, &mut vm).unwrap(), Some(0.0));
    }
}

