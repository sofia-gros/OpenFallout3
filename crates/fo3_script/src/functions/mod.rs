pub mod actor;
pub mod ai_combat;
pub mod control;
pub mod dialogue;
pub mod item;
pub mod magic;
pub mod math;
pub mod movement;
pub mod quest;
pub mod state;
pub mod stats;

use crate::parser::Expr;
use crate::vm::{ScriptError, ScriptVm};
use fo3_esm::FormId;

/// スクリプト組み込み関数およびコマンドの総合ディスパッチャ。
/// 参照元: eferences/openmw/components/esm4/script.hpp
pub fn dispatch(
    cmd: &str,
    args: &[Expr],
    subject_id: Option<FormId>,
    vm: &mut ScriptVm,
) -> Result<Option<f32>, ScriptError> {
    let lower_cmd = cmd.to_ascii_lowercase();

    if let Some(res) = actor::execute(&lower_cmd, args, subject_id, vm)? {
        return Ok(Some(res));
    }
    if let Some(res) = item::execute(&lower_cmd, args, subject_id, vm)? {
        return Ok(Some(res));
    }
    if let Some(res) = quest::execute(&lower_cmd, args, subject_id, vm)? {
        return Ok(Some(res));
    }
    if let Some(res) = state::execute(&lower_cmd, args, subject_id, vm)? {
        return Ok(Some(res));
    }
    if let Some(res) = math::execute(&lower_cmd, args, subject_id, vm)? {
        return Ok(Some(res));
    }
    if let Some(res) = stats::execute(&lower_cmd, args, subject_id, vm)? {
        return Ok(Some(res));
    }
    if let Some(res) = magic::execute(&lower_cmd, args, subject_id, vm)? {
        return Ok(Some(res));
    }
    if let Some(res) = control::execute(&lower_cmd, args, subject_id, vm)? {
        return Ok(Some(res));
    }
    if let Some(res) = movement::execute(&lower_cmd, args, subject_id, vm)? {
        return Ok(Some(res));
    }
    if let Some(res) = dialogue::execute(&lower_cmd, args, subject_id, vm)? {
        return Ok(Some(res));
    }
    if let Some(res) = ai_combat::execute(&lower_cmd, args, subject_id, vm)? {
        return Ok(Some(res));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opcode_dispatch_all_11_modules() {
        let mut vm = ScriptVm::new();
        let player = FormId(0x14);
        let quest_id = FormId(0x1000);
        let item_id = FormId(0x0000000F); // Caps
        let door_id = FormId(0x2000);
        let perk_id = FormId(0x3000);

        // 1. Math
        assert_eq!(dispatch("iswin32", &[], None, &mut vm).unwrap(), Some(1.0));
        assert_eq!(dispatch("isps3", &[], None, &mut vm).unwrap(), Some(0.0));
        assert_eq!(
            dispatch("getsecondspassed", &[], None, &mut vm).unwrap(),
            Some(1.0 / 60.0)
        );
        dispatch(
            "setglobalvalue",
            &[Expr::Variable("myglob".into()), Expr::Number(42.0)],
            None,
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            dispatch(
                "getglobalvalue",
                &[Expr::Variable("myglob".into())],
                None,
                &mut vm
            )
            .unwrap(),
            Some(42.0)
        );

        // 2. Item
        dispatch(
            "additem",
            &[Expr::Number(item_id.0 as f32), Expr::Number(100.0)],
            Some(player),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            dispatch(
                "getitemcount",
                &[Expr::Number(item_id.0 as f32)],
                Some(player),
                &mut vm
            )
            .unwrap(),
            Some(100.0)
        );
        assert_eq!(
            dispatch("getgold", &[], Some(player), &mut vm).unwrap(),
            Some(100.0)
        );

        // 3. Quest
        dispatch(
            "setstage",
            &[Expr::Number(quest_id.0 as f32), Expr::Number(10.0)],
            None,
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            dispatch(
                "getstage",
                &[Expr::Number(quest_id.0 as f32)],
                None,
                &mut vm
            )
            .unwrap(),
            Some(10.0)
        );
        assert_eq!(
            dispatch(
                "getstagedone",
                &[Expr::Number(quest_id.0 as f32), Expr::Number(10.0)],
                None,
                &mut vm
            )
            .unwrap(),
            Some(1.0)
        );

        // 4. State
        dispatch("disable", &[], Some(door_id), &mut vm).unwrap();
        assert_eq!(
            dispatch("getdisabled", &[], Some(door_id), &mut vm).unwrap(),
            Some(1.0)
        );
        dispatch("enable", &[], Some(door_id), &mut vm).unwrap();
        assert_eq!(
            dispatch("getdisabled", &[], Some(door_id), &mut vm).unwrap(),
            Some(0.0)
        );

        // 5. Actor
        assert_eq!(
            dispatch("getissex", &[Expr::Number(0.0)], Some(player), &mut vm).unwrap(),
            Some(1.0)
        );
        assert_eq!(
            dispatch(
                "getdistance",
                &[Expr::Number(door_id.0 as f32)],
                Some(player),
                &mut vm
            )
            .unwrap(),
            Some(500.0)
        );

        // 6. Stats (GetAV, SetAV, ModAV, HealthPercentage)
        dispatch(
            "setav",
            &[Expr::Variable("strength".into()), Expr::Number(8.0)],
            Some(player),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            dispatch(
                "getav",
                &[Expr::Variable("strength".into())],
                Some(player),
                &mut vm
            )
            .unwrap(),
            Some(8.0)
        );
        dispatch(
            "modav",
            &[Expr::Variable("strength".into()), Expr::Number(2.0)],
            Some(player),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            dispatch(
                "getav",
                &[Expr::Variable("strength".into())],
                Some(player),
                &mut vm
            )
            .unwrap(),
            Some(10.0)
        );
        assert_eq!(
            dispatch("gethealthpercentage", &[], Some(player), &mut vm).unwrap(),
            Some(1.0)
        );

        // 7. Magic (AddPerk, HasPerk, RemovePerk)
        assert_eq!(
            dispatch(
                "hasperk",
                &[Expr::Number(perk_id.0 as f32)],
                Some(player),
                &mut vm
            )
            .unwrap(),
            Some(0.0)
        );
        dispatch(
            "addperk",
            &[Expr::Number(perk_id.0 as f32)],
            Some(player),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            dispatch(
                "hasperk",
                &[Expr::Number(perk_id.0 as f32)],
                Some(player),
                &mut vm
            )
            .unwrap(),
            Some(1.0)
        );
        dispatch(
            "removeperk",
            &[Expr::Number(perk_id.0 as f32)],
            Some(player),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            dispatch(
                "hasperk",
                &[Expr::Number(perk_id.0 as f32)],
                Some(player),
                &mut vm
            )
            .unwrap(),
            Some(0.0)
        );

        // 8. Control (DisablePlayerControls, EnablePlayerControls)
        dispatch("disableplayercontrols", &[], None, &mut vm).unwrap();
        assert_eq!(
            dispatch("getplayercontrolsdisabled", &[], None, &mut vm).unwrap(),
            Some(1.0)
        );
        dispatch("enableplayercontrols", &[], None, &mut vm).unwrap();
        assert_eq!(
            dispatch("getplayercontrolsdisabled", &[], None, &mut vm).unwrap(),
            Some(0.0)
        );

        // 9. Movement (SetPos, GetPos, SetAngle, GetAngle)
        dispatch(
            "setpos",
            &[Expr::Variable("x".into()), Expr::Number(1234.5)],
            Some(player),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            dispatch(
                "getpos",
                &[Expr::Variable("x".into())],
                Some(player),
                &mut vm
            )
            .unwrap(),
            Some(1234.5)
        );
        dispatch(
            "setangle",
            &[Expr::Variable("z".into()), Expr::Number(90.0)],
            Some(player),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            dispatch(
                "getangle",
                &[Expr::Variable("z".into())],
                Some(player),
                &mut vm
            )
            .unwrap(),
            Some(90.0)
        );

        // 10. Dialogue (Say, IsTalking)
        dispatch(
            "say",
            &[Expr::Variable("TestTopic".into())],
            Some(player),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            dispatch("istalking", &[], Some(player), &mut vm).unwrap(),
            Some(1.0)
        );

        // 11. AI Combat (SetPlayerTeammate, GetPlayerTeammate, SetFactionRank, GetFactionRank, CrimeGold)
        dispatch(
            "setplayerteammate",
            &[Expr::Number(1.0)],
            Some(door_id),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            dispatch("getplayerteammate", &[], Some(door_id), &mut vm).unwrap(),
            Some(1.0)
        );
        let fac_id = FormId(0x4000);
        dispatch(
            "setfactionrank",
            &[Expr::Number(fac_id.0 as f32), Expr::Number(3.0)],
            Some(player),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            dispatch(
                "getfactionrank",
                &[Expr::Number(fac_id.0 as f32)],
                Some(player),
                &mut vm
            )
            .unwrap(),
            Some(3.0)
        );
        dispatch("setcrimegold", &[Expr::Number(500.0)], None, &mut vm).unwrap();
        assert_eq!(
            dispatch("getcrimegold", &[], None, &mut vm).unwrap(),
            Some(500.0)
        );
        dispatch("payfine", &[], None, &mut vm).unwrap();
        assert_eq!(
            dispatch("getcrimegold", &[], None, &mut vm).unwrap(),
            Some(0.0)
        );
    }
}
