use super::*;
use fo3_esm::FormId;
use fo3_script::ScriptVm;

    #[test]
    fn test_dialog_two_phase_navigation_and_selection() {
        let choices = vec![
            DialogChoice {
                prompt: "父親を探している。".to_string(),
                response: "ジェームズのことか？".to_string(),
                is_goodbye: false,
                result_script: Some("player.additem 0x0000000F 10".to_string()),
                info_form_id: Some(FormId(0x00012345)),
            },
            DialogChoice {
                prompt: "メガトンについて教えてくれ。".to_string(),
                response: "不発弾の周りにできた街さ。".to_string(),
                is_goodbye: false,
                result_script: None,
                info_form_id: None,
            },
        ];

        let mut dialog = DialogState::new("Colin Moriarty", "何か用か？", choices.clone());
        // 初期状態はセリフ表示フェーズ
        assert_eq!(dialog.phase, DialogPhase::ShowingSpeech);
        assert_eq!(dialog.choices.len(), 3); // Goodbye 自動付加で3個
        assert_eq!(dialog.selected_index, 0);

        // 1回目の advance でトピック一覧へ遷移
        let closed = dialog.advance();
        assert!(!closed);
        assert_eq!(dialog.phase, DialogPhase::ShowingTopics);

        // トピック選択: 下移動
        dialog.select_down();
        assert_eq!(dialog.selected_index, 1);
        dialog.select_down();
        assert_eq!(dialog.selected_index, 2); // Goodbye
        dialog.select_down();
        assert_eq!(dialog.selected_index, 0); // ループ

        // トピック決定: advance_with_vm でセリフ表示へ遷移
        let mut vm = ScriptVm::new();
        let closed = dialog.advance_with_vm(&mut vm);
        assert!(!closed);
        assert_eq!(dialog.phase, DialogPhase::ShowingSpeech);
        assert_eq!(dialog.current_speech, "ジェームズのことか？");

        // もう一度 advance でトピック一覧へ戻る
        dialog.advance();
        assert_eq!(dialog.phase, DialogPhase::ShowingTopics);

        // Goodbye 選択肢 (index 2) を選ぶ
        dialog.selected_index = 2;
        let closed = dialog.advance_with_vm(&mut vm);
        assert!(!closed);
        assert_eq!(dialog.phase, DialogPhase::ShowingSpeech);
        assert_eq!(dialog.current_speech, "またな。");
        assert!(dialog.is_goodbye_pending);

        // Goodbye セリフ表示後に advance を呼ぶと終了
        let closed = dialog.advance();
        assert!(closed);
        assert!(dialog.is_closed);
    }
