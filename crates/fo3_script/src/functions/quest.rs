//! # クエスト関連スクリプト関数
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
        // SetStage [QuestID] [Stage]
        "setstage" => {
            if args.len() >= 2 {
                let q_id = get_form_id(&args[0])?;
                let stage = vm.eval_ast_expr(&args[1], subject)? as u32;
                vm.set_stage(q_id, stage);
            }
            Ok(Some(0.0))
        }
        // FUN_GetStage = 58
        // GetStage [QuestID]
        "getstage" => {
            let q_id = if !args.is_empty() {
                get_form_id(&args[0])?
            } else if let Some(fid) = subject {
                fid
            } else {
                return Ok(Some(0.0));
            };
            let stage = vm.get_stage(q_id) as f32;
            Ok(Some(stage))
        }
        // FUN_GetStageDone = 59
        // GetStageDone [QuestID] [Stage]
        "getstagedone" => {
            if args.len() >= 2 {
                let q_id = get_form_id(&args[0])?;
                let stage = vm.eval_ast_expr(&args[1], subject)? as u32;
                let done = vm.quest_manager.get_stage_done(q_id, stage as u16);
                return Ok(Some(if done { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // SetObjectiveDisplayed [QuestID] [ObjectiveIndex] [Displayed: 0 or 1]
        "setobjectivedisplayed" => {
            if args.len() >= 3 {
                let q_id = get_form_id(&args[0])?;
                let index = vm.eval_ast_expr(&args[1], subject)? as u32;
                let displayed = vm.eval_ast_expr(&args[2], subject)? != 0.0;
                vm.quest_manager
                    .set_objective_displayed(q_id, index, displayed);
            }
            Ok(Some(0.0))
        }
        // FUN_GetObjectiveDisplayed = 421
        // GetObjectiveDisplayed [QuestID] [ObjectiveIndex]
        "getobjectivedisplayed" => {
            if args.len() >= 2 {
                let q_id = get_form_id(&args[0])?;
                let index = vm.eval_ast_expr(&args[1], subject)? as u32;
                let displayed = vm.quest_manager.is_objective_displayed(q_id, index);
                return Ok(Some(if displayed { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // SetObjectiveCompleted [QuestID] [ObjectiveIndex] [Completed: 0 or 1]
        "setobjectivecompleted" => {
            if args.len() >= 3 {
                let q_id = get_form_id(&args[0])?;
                let index = vm.eval_ast_expr(&args[1], subject)? as u32;
                let completed = vm.eval_ast_expr(&args[2], subject)? != 0.0;
                vm.quest_manager
                    .set_objective_completed(q_id, index, completed);
            }
            Ok(Some(0.0))
        }
        // FUN_GetObjectiveCompleted = 420
        // GetObjectiveCompleted [QuestID] [ObjectiveIndex]
        "getobjectivecompleted" => {
            if args.len() >= 2 {
                let q_id = get_form_id(&args[0])?;
                let index = vm.eval_ast_expr(&args[1], subject)? as u32;
                let completed = vm.quest_manager.is_objective_completed(q_id, index);
                return Ok(Some(if completed { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // FUN_GetQuestRunning = 56
        // GetQuestRunning [QuestID]
        "getquestrunning" => {
            let q_id = if !args.is_empty() {
                get_form_id(&args[0])?
            } else if let Some(fid) = subject {
                fid
            } else {
                return Ok(Some(0.0));
            };
            let stage = vm.get_stage(q_id);
            // ステージが 0 より大きく、完了フラグが立っていない場合は実行中
            Ok(Some(
                if stage > 0 && !vm.quest_manager.is_quest_completed(q_id) {
                    1.0
                } else {
                    0.0
                },
            ))
        }
        // StartQuest [QuestID]
        "startquest" => {
            if !args.is_empty() {
                let q_id = get_form_id(&args[0])?;
                if vm.get_stage(q_id) == 0 {
                    vm.set_stage(q_id, 10);
                }
            }
            Ok(Some(0.0))
        }
        // StopQuest [QuestID]
        "stopquest" => {
            if !args.is_empty() {
                let q_id = get_form_id(&args[0])?;
                vm.quest_manager.complete_quest(q_id);
            }
            Ok(Some(0.0))
        }
        // CompleteQuest [QuestID]
        "completequest" => {
            if !args.is_empty() {
                let q_id = get_form_id(&args[0])?;
                vm.quest_manager.complete_quest(q_id);
            }
            Ok(Some(0.0))
        }
        // 参照元: GECK: ResetQuest <QuestID>
        // クエストの進行状況、目標、履歴を初期化する
        "resetquest" => {
            if !args.is_empty() {
                let q_id = get_form_id(&args[0])?;
                vm.quest_stages.remove(&q_id);
                vm.quest_manager.reset_quest(q_id);
                println!("[Script] ResetQuest: {:?}", q_id);
            }
            Ok(Some(0.0))
        }
        // 参照元: references/openmw/components/esm4/script.hpp:305 (FUN_GetQuestCompleted = 546)
        // GetQuestCompleted [QuestID]
        // クエストが完了済みかどうかを判定 (完了なら 1.0, 未完了なら 0.0)
        "getquestcompleted" => {
            let q_id = if !args.is_empty() {
                get_form_id(&args[0])?
            } else if let Some(fid) = subject {
                fid
            } else {
                return Ok(Some(0.0));
            };
            let completed = vm.quest_manager.is_quest_completed(q_id);
            Ok(Some(if completed { 1.0 } else { 0.0 }))
        }
        // 参照元: references/openmw/components/esm4/script.hpp:120 (FUN_GetQuestVariable = 79)
        // GetQuestVariable [QuestID] [VarName]
        // 指定クエストに紐づくローカル変数の値を取得する
        "getquestvariable" => {
            if args.len() >= 2 {
                let q_id = get_form_id(&args[0])?;
                let var_name = match &args[1] {
                    Expr::Variable(v) => v.as_str(),
                    _ => "",
                };
                let val = vm
                    .quest_manager
                    .get_quest_variable(q_id, var_name)
                    .unwrap_or(0.0) as f32;
                return Ok(Some(val));
            }
            Ok(Some(0.0))
        }
        // 参照元: GECK: SetObjectiveFailed <QuestID> <ObjectiveIndex> [Failed: 0 or 1]
        // クエスト目標の失敗状態を設定する
        "setobjectivefailed" => {
            if args.len() >= 2 {
                let q_id = get_form_id(&args[0])?;
                let index = vm.eval_ast_expr(&args[1], subject)? as u32;
                let failed = if args.len() >= 3 {
                    vm.eval_ast_expr(&args[2], subject)? != 0.0
                } else {
                    true
                };
                vm.quest_manager.set_objective_failed(q_id, index, failed);
            }
            Ok(Some(0.0))
        }
        // 参照元: GECK: GetObjectiveFailed <QuestID> <ObjectiveIndex>
        // クエスト目標が失敗状態かどうか判定 (失敗なら 1.0, 否なら 0.0)
        "getobjectivefailed" => {
            if args.len() >= 2 {
                let q_id = get_form_id(&args[0])?;
                let index = vm.eval_ast_expr(&args[1], subject)? as u32;
                let failed = vm.quest_manager.is_objective_failed(q_id, index);
                return Ok(Some(if failed { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // 参照元: GECK: CompleteAllObjectives <QuestID>
        // クエストの全目標を一括完了する
        "completeallobjectives" => {
            if !args.is_empty() {
                let q_id = get_form_id(&args[0])?;
                vm.quest_manager.complete_all_objectives(q_id);
            }
            Ok(Some(0.0))
        }
        // 参照元: GECK: FailAllObjectives <QuestID>
        // クエストの全目標を一括失敗にする
        "failallobjectives" => {
            if !args.is_empty() {
                let q_id = get_form_id(&args[0])?;
                vm.quest_manager.fail_all_objectives(q_id);
            }
            Ok(Some(0.0))
        }
        // 参照元: GECK: SetCurrentQuest <QuestID>
        // 現在アクティブな追跡クエストを設定する
        "setcurrentquest" => {
            if !args.is_empty() {
                let q_id = get_form_id(&args[0])?;
                vm.quest_manager.set_current_quest(q_id);
            }
            Ok(Some(0.0))
        }
        // 参照元: GECK: GetCurrentQuest
        // 現在アクティブな追跡クエストの FormID を返す
        "getcurrentquest" => {
            let current = vm
                .quest_manager
                .get_current_quest()
                .map(|q| q.0 as f32)
                .unwrap_or(0.0);
            Ok(Some(current))
        }
        // 参照元: GECK: SetQuestDelay <QuestID> <DelayFloat>
        // クエストスクリプトの更新間隔（秒）を設定する
        "setquestdelay" => {
            if args.len() >= 2 {
                let q_id = get_form_id(&args[0])?;
                let delay = vm.eval_ast_expr(&args[1], subject)?;
                vm.quest_manager.set_quest_delay(q_id, delay);
            }
            Ok(Some(0.0))
        }
        // 参照元: GECK: GetQuestDelay <QuestID>
        // クエストスクリプトの更新間隔（秒）を取得する (未設定時は 5.0)
        "getquestdelay" => {
            if !args.is_empty() {
                let q_id = get_form_id(&args[0])?;
                let delay = vm.quest_manager.get_quest_delay(q_id);
                return Ok(Some(delay));
            }
            Ok(Some(5.0))
        }
        // 参照元: GECK: SetQuestObject <FormID> [IsQuest: 0 or 1]
        // アイテムのクエスト属性（ドロップ不可フラグ）を設定する
        "setquestobject" => {
            if !args.is_empty() {
                let item_id = get_form_id(&args[0])?;
                let is_quest = if args.len() >= 2 {
                    vm.eval_ast_expr(&args[1], subject)? != 0.0
                } else {
                    true
                };
                vm.quest_manager.set_quest_item(item_id, is_quest);
            }
            Ok(Some(0.0))
        }
        // 参照元: GECK: IsQuestObject <FormID>
        // アイテムがクエスト属性を持つか判定 (1.0 or 0.0)
        "isquestobject" => {
            let item_id = if !args.is_empty() {
                get_form_id(&args[0])?
            } else if let Some(fid) = subject {
                fid
            } else {
                return Ok(Some(0.0));
            };
            let is_q = vm.quest_manager.is_quest_item(item_id);
            Ok(Some(if is_q { 1.0 } else { 0.0 }))
        }
        // 参照元: GECK: ShowQuestStages (SQS) [QuestID]
        // コンソール向け: クエストステージのログ出力
        "showqueststages" | "sqs" => {
            if !args.is_empty() {
                let q_id = get_form_id(&args[0])?;
                let current = vm.quest_manager.get_stage(q_id);
                println!(
                    "[Script] ShowQuestStages: Quest {:?}, Current Stage {}",
                    q_id, current
                );
            }
            Ok(Some(0.0))
        }
        // 参照元: GECK: ShowQuestVars (SQV) [QuestID]
        // コンソール向け: クエスト変数のログ出力
        "showquestvars" | "sqv" => {
            if !args.is_empty() {
                let q_id = get_form_id(&args[0])?;
                println!("[Script] ShowQuestVars: Quest {:?}", q_id);
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
    fn test_quest_opcodes_extended() {
        let mut vm = ScriptVm::new();
        let quest_id = FormId(0x00014E89);

        // startquest / getquestrunning
        execute(
            "startquest",
            &[Expr::Number(quest_id.0 as f32)],
            None,
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute(
                "getquestrunning",
                &[Expr::Number(quest_id.0 as f32)],
                None,
                &mut vm
            )
            .unwrap(),
            Some(1.0)
        );

        // setstage / getstage / getstagedone
        execute(
            "setstage",
            &[Expr::Number(quest_id.0 as f32), Expr::Number(20.0)],
            None,
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute(
                "getstage",
                &[Expr::Number(quest_id.0 as f32)],
                None,
                &mut vm
            )
            .unwrap(),
            Some(20.0)
        );
        assert_eq!(
            execute(
                "getstagedone",
                &[Expr::Number(quest_id.0 as f32), Expr::Number(20.0)],
                None,
                &mut vm
            )
            .unwrap(),
            Some(1.0)
        );

        // objectives: set / get / failed
        execute(
            "setobjectivedisplayed",
            &[
                Expr::Number(quest_id.0 as f32),
                Expr::Number(1.0),
                Expr::Number(1.0),
            ],
            None,
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute(
                "getobjectivedisplayed",
                &[Expr::Number(quest_id.0 as f32), Expr::Number(1.0)],
                None,
                &mut vm
            )
            .unwrap(),
            Some(1.0)
        );
        execute(
            "setobjectivecompleted",
            &[
                Expr::Number(quest_id.0 as f32),
                Expr::Number(1.0),
                Expr::Number(1.0),
            ],
            None,
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute(
                "getobjectivecompleted",
                &[Expr::Number(quest_id.0 as f32), Expr::Number(1.0)],
                None,
                &mut vm
            )
            .unwrap(),
            Some(1.0)
        );
        execute(
            "setobjectivefailed",
            &[
                Expr::Number(quest_id.0 as f32),
                Expr::Number(1.0),
                Expr::Number(1.0),
            ],
            None,
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute(
                "getobjectivefailed",
                &[Expr::Number(quest_id.0 as f32), Expr::Number(1.0)],
                None,
                &mut vm
            )
            .unwrap(),
            Some(1.0)
        );

        // current quest
        execute(
            "setcurrentquest",
            &[Expr::Number(quest_id.0 as f32)],
            None,
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute("getcurrentquest", &[], None, &mut vm).unwrap(),
            Some(quest_id.0 as f32)
        );

        // quest delay
        execute(
            "setquestdelay",
            &[Expr::Number(quest_id.0 as f32), Expr::Number(2.5)],
            None,
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute(
                "getquestdelay",
                &[Expr::Number(quest_id.0 as f32)],
                None,
                &mut vm
            )
            .unwrap(),
            Some(2.5)
        );

        // quest object
        let item_id = FormId(0x00020000);
        assert_eq!(
            execute(
                "isquestobject",
                &[Expr::Number(item_id.0 as f32)],
                None,
                &mut vm
            )
            .unwrap(),
            Some(0.0)
        );
        execute(
            "setquestobject",
            &[Expr::Number(item_id.0 as f32), Expr::Number(1.0)],
            None,
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute(
                "isquestobject",
                &[Expr::Number(item_id.0 as f32)],
                None,
                &mut vm
            )
            .unwrap(),
            Some(1.0)
        );

        // completequest / getquestcompleted
        assert_eq!(
            execute(
                "getquestcompleted",
                &[Expr::Number(quest_id.0 as f32)],
                None,
                &mut vm
            )
            .unwrap(),
            Some(0.0)
        );
        execute(
            "completequest",
            &[Expr::Number(quest_id.0 as f32)],
            None,
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute(
                "getquestcompleted",
                &[Expr::Number(quest_id.0 as f32)],
                None,
                &mut vm
            )
            .unwrap(),
            Some(1.0)
        );

        // resetquest
        execute(
            "resetquest",
            &[Expr::Number(quest_id.0 as f32)],
            None,
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute(
                "getstage",
                &[Expr::Number(quest_id.0 as f32)],
                None,
                &mut vm
            )
            .unwrap(),
            Some(0.0)
        );
        assert_eq!(
            execute(
                "getquestcompleted",
                &[Expr::Number(quest_id.0 as f32)],
                None,
                &mut vm
            )
            .unwrap(),
            Some(0.0)
        );
    }
}
