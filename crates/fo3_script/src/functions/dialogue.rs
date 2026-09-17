//! # 会話・対話・音声関連スクリプト関数
//! 参照元: references/openmw/components/esm4/script.hpp:50, 123, 141, 172, 435, 436

use crate::parser::Expr;
use crate::vm::{ScriptError, ScriptVm};
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
        // Say [TopicID] [TargetRef]
        "say" => {
            if !args.is_empty() {
                let topic_str = match &args[0] {
                    Expr::Variable(v) => v.clone(),
                    Expr::Number(n) => format!("{:08X}", *n as u32),
                    _ => "GenericTopic".to_string(),
                };
                vm.say_queue.push((Some(target), topic_str.clone()));
                println!("[Script] Say: Speaker {:?}, Topic {}", target, topic_str);
            }
            Ok(Some(0.0))
        }
        // StopLookAt
        "stoplookat" => {
            println!("[Script] StopLookAt: {:?}", target);
            Ok(Some(0.0))
        }
        // FUN_GetTalkedToPC = 50
        "gettalkedtopc" => {
            let key = format!("{:08X}.talkedtopc", target.0);
            let talked = vm.locals.get(&key).copied().unwrap_or(0.0) != 0.0;
            Ok(Some(if talked { 1.0 } else { 0.0 }))
        }
        // FUN_GetTalkedToPCParam = 172
        // GetTalkedToPCParam [TargetActorRef]
        "gettalkedtopcparam" => {
            if !args.is_empty() {
                let other_actor = get_form_id(&args[0])?;
                let key = format!("{:08X}.talkedtopc", other_actor.0);
                let talked = vm.locals.get(&key).copied().unwrap_or(0.0) != 0.0;
                return Ok(Some(if talked { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // FUN_IsGreetingPlayer = 123
        "isgreetingplayer" => Ok(Some(0.0)),
        // FUN_IsTalking = 141
        "istalking" => {
            let talking = !vm.say_queue.is_empty();
            Ok(Some(if talking { 1.0 } else { 0.0 }))
        }
        // FUN_GetDialogueEmotion = 435 (0=Neutral, 1=Anger, 2=Disgust, 3=Fear, 4=Happy, 5=Sad, 6=Surprise)
        "getdialogueemotion" => {
            Ok(Some(0.0)) // Neutral
        }
        // FUN_GetDialogueEmotionValue = 436 (0..100)
        "getdialogueemotionvalue" => Ok(Some(50.0)),
        // 参照元: GECK: SayTo <TargetActorRef> <TopicID>
        // 対象のアクターに向けて指定トピックのセリフを発話する
        "sayto" => {
            if args.len() >= 2 {
                let listener = get_form_id(&args[0])?;
                let topic_str = match &args[1] {
                    Expr::Variable(v) => v.clone(),
                    Expr::Number(n) => format!("{:08X}", *n as u32),
                    _ => "GenericTopic".to_string(),
                };
                vm.say_queue.push((Some(target), topic_str.clone()));
                println!(
                    "[Script] SayTo: Speaker {:?}, Listener {:?}, Topic {}",
                    target, listener, topic_str
                );
            }
            Ok(Some(0.0))
        }
        // 参照元: GECK: SayDone / GetSayDone
        // セリフの発話が終了しているかどうかを判定 (完了なら 1.0, 発話中なら 0.0)
        "saydone" | "getsaydone" => {
            let done = vm.say_queue.is_empty();
            Ok(Some(if done { 1.0 } else { 0.0 }))
        }
        // 参照元: references/openmw/components/esm4/script.hpp:262 (FUN_GetIsVoiceType = 427)
        // GetIsVoiceType [VoiceTypeID]
        // アクターのボイスタイプが指定されたものと一致するか判定
        "getisvoicetype" => {
            if !args.is_empty() {
                let vt_id = get_form_id(&args[0])?;
                let key = format!("{:08X}.voicetype", target.0);
                let current_vt = vm.locals.get(&key).copied().unwrap_or(0.0) as u32;
                return Ok(Some(if current_vt == vt_id.0 { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // 参照元: GECK: AddTopic <TopicID>
        // 会話トピックをプレイヤーの対話リストに追加する
        "addtopic" => {
            if !args.is_empty() {
                let topic_str = match &args[0] {
                    Expr::Variable(v) => v.clone(),
                    Expr::Number(n) => format!("{:08X}", *n as u32),
                    _ => "GenericTopic".to_string(),
                };
                vm.quest_manager.add_topic(&topic_str);
                println!("[Script] AddTopic: {}", topic_str);
            }
            Ok(Some(0.0))
        }
        // 参照元: GECK: RemoveTopic <TopicID>
        // 会話トピックをプレイヤーの対話リストから削除する
        "removetopic" => {
            if !args.is_empty() {
                let topic_str = match &args[0] {
                    Expr::Variable(v) => v.clone(),
                    Expr::Number(n) => format!("{:08X}", *n as u32),
                    _ => "GenericTopic".to_string(),
                };
                vm.quest_manager.remove_topic(&topic_str);
                println!("[Script] RemoveTopic: {}", topic_str);
            }
            Ok(Some(0.0))
        }
        // 参照元: GECK: GetInVoiceList <VoiceListID>
        // アクターのボイスタイプが指定のボイスリストに含まれているか判定
        "getinvoicelist" => {
            if !args.is_empty() {
                let list_id = get_form_id(&args[0])?;
                let key = format!("{:08X}.invoicelist.{:08X}", target.0, list_id.0);
                let in_list = vm.locals.get(&key).copied().unwrap_or(0.0) != 0.0;
                return Ok(Some(if in_list { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // 参照元: GECK: StartConversation <TargetActorRef> [TopicID]
        // 指定したアクターとの会話を開始する
        "startconversation" => {
            if !args.is_empty() {
                let other_actor = get_form_id(&args[0])?;
                let key = format!("{:08X}.in_conversation", target.0);
                vm.locals.insert(key, 1.0);
                println!(
                    "[Script] StartConversation: Speaker {:?}, Target {:?}",
                    target, other_actor
                );
            }
            Ok(Some(0.0))
        }
        // 参照元: GECK: ForceGreeting
        // 対象アクターからプレイヤーへ強制的に会話を開始する
        "forcegreeting" => {
            let key = format!("{:08X}.forcegreeting", target.0);
            vm.locals.insert(key, 1.0);
            println!("[Script] ForceGreeting: Actor {:?}", target);
            Ok(Some(0.0))
        }
        // 参照元: GECK: Goodbye
        // 会話ウィンドウを閉じて終了するフラグを設定する
        "goodbye" => {
            vm.locals.insert("dialogue.goodbye".to_string(), 1.0);
            println!("[Script] Goodbye executed");
            Ok(Some(0.0))
        }
        // 参照元: references/openmw/components/esm4/script.hpp:130 (FUN_IsFacingUp = 106)
        // アクターが倒れて上を向いているかどうか判定 (1.0 or 0.0)
        "isfacingup" => {
            let key = format!("{:08X}.isfacingup", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // 参照元: references/openmw/components/esm4/script.hpp:244 (FUN_IsTalkingActivatorActor = 370)
        // 対象アクターが Talking Activator の話者として発話中か判定
        "istalkingactivatoractor" => {
            let key = format!("{:08X}.istalkingactivator", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // 参照元: references/openmw/components/esm4/script.hpp:264 (FUN_IsActorTalkingThroughActivator = 430)
        // アクターが Talking Activator 経由で話しているか判定
        "isactortalkingthroughactivator" => {
            let key = format!("{:08X}.talkingthroughactivator", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // 参照元: GECK: ShowSubtitle / ShowSubtitles [1 or 0]
        // 会話字幕の表示を制御する
        "showsubtitle" | "showsubtitles" => {
            let enable = if !args.is_empty() {
                vm.eval_ast_expr(&args[0], subject)? != 0.0
            } else {
                true
            };
            vm.locals
                .insert("showsubtitles".to_string(), if enable { 1.0 } else { 0.0 });
            println!("[Script] ShowSubtitles: {}", enable);
            Ok(Some(0.0))
        }
        // 参照元: references/openmw/components/esm4/script.hpp:143 (FUN_HasBeenEaten = 127)
        // 対象アクターが食屍能力等で食べられたか判定 (1.0 or 0.0)
        "hasbeeneaten" => {
            let key = format!("{:08X}.hasbeeneaten", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // 参照元: GECK: LookAt <TargetRef>
        // 指定対象に視線を向ける
        "lookat" => {
            if !args.is_empty() {
                let other_ref = get_form_id(&args[0])?;
                vm.locals
                    .insert(format!("{:08X}.lookat", target.0), other_ref.0 as f32);
                println!(
                    "[Script] LookAt: Target {:?} -> Other {:?}",
                    target, other_ref
                );
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
    fn test_dialogue_opcodes_extended() {
        let mut vm = ScriptVm::new();
        let actor = FormId(0x00020001);
        let player = FormId(0x00000014);

        // saydone (キューが空なら 1.0)
        assert_eq!(
            execute("saydone", &[], Some(actor), &mut vm).unwrap(),
            Some(1.0)
        );

        // sayto
        execute(
            "sayto",
            &[
                Expr::Number(player.0 as f32),
                Expr::Variable("TopicGreeting".into()),
            ],
            Some(actor),
            &mut vm,
        )
        .unwrap();
        assert_eq!(vm.say_queue.len(), 1);
        assert_eq!(
            execute("saydone", &[], Some(actor), &mut vm).unwrap(),
            Some(0.0)
        );
        assert_eq!(
            execute("istalking", &[], Some(actor), &mut vm).unwrap(),
            Some(1.0)
        );

        // addtopic / removetopic
        execute(
            "addtopic",
            &[Expr::Variable("TopicRumors".into())],
            None,
            &mut vm,
        )
        .unwrap();
        assert!(vm.quest_manager.has_topic("TopicRumors"));
        execute(
            "removetopic",
            &[Expr::Variable("TopicRumors".into())],
            None,
            &mut vm,
        )
        .unwrap();
        assert!(!vm.quest_manager.has_topic("TopicRumors"));

        // getisvoicetype
        let vt = FormId(0x00030001);
        assert_eq!(
            execute(
                "getisvoicetype",
                &[Expr::Number(vt.0 as f32)],
                Some(actor),
                &mut vm
            )
            .unwrap(),
            Some(0.0)
        );
        vm.locals
            .insert(format!("{:08X}.voicetype", actor.0), vt.0 as f32);
        assert_eq!(
            execute(
                "getisvoicetype",
                &[Expr::Number(vt.0 as f32)],
                Some(actor),
                &mut vm
            )
            .unwrap(),
            Some(1.0)
        );

        // startconversation
        execute(
            "startconversation",
            &[Expr::Number(player.0 as f32)],
            Some(actor),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            vm.locals.get(&format!("{:08X}.in_conversation", actor.0)),
            Some(&1.0)
        );

        // forcegreeting
        execute("forcegreeting", &[], Some(actor), &mut vm).unwrap();
        assert_eq!(
            vm.locals.get(&format!("{:08X}.forcegreeting", actor.0)),
            Some(&1.0)
        );

        // goodbye
        execute("goodbye", &[], None, &mut vm).unwrap();
        assert_eq!(vm.locals.get("dialogue.goodbye"), Some(&1.0));

        // showsubtitles
        execute("showsubtitles", &[Expr::Number(1.0)], None, &mut vm).unwrap();
        assert_eq!(vm.locals.get("showsubtitles"), Some(&1.0));

        // lookat / stoplookat
        let other = FormId(0x00040001);
        execute(
            "lookat",
            &[Expr::Number(other.0 as f32)],
            Some(actor),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            vm.locals.get(&format!("{:08X}.lookat", actor.0)),
            Some(&(other.0 as f32))
        );
        execute("stoplookat", &[], Some(actor), &mut vm).unwrap();
    }
}
