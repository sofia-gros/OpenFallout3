use super::*;
use fo3_vfs::VfsManager;


    #[test]
    fn test_locomotion_state_transitions() {
        let mut vfs = VfsManager::new();
        let mut sm = LocomotionStateMachine::new(&mut vfs);

        // 初期ステートは Idle
        assert_eq!(sm.current_state, LocomotionState::Idle);

        // 前進歩行 (Wキー、Shiftなし、しゃがみなし)
        sm.update_state(true, false, false, false, false, false, false, true);
        assert_eq!(sm.current_state, LocomotionState::WalkForward);

        // 前進走行 (Wキー + Shift、しゃがみなし)
        sm.update_state(true, false, false, false, true, false, false, true);
        assert_eq!(sm.current_state, LocomotionState::RunForward);

        // しゃがみ待機 (Ctrlキー、移動なし)
        sm.update_state(false, false, false, false, false, false, true, true);
        assert_eq!(sm.current_state, LocomotionState::SneakIdle);

        // しゃがみ前進歩行 (Wキー + Ctrl)
        sm.update_state(true, false, false, false, false, false, true, true);
        assert_eq!(sm.current_state, LocomotionState::SneakWalkForward);

        // ジャンプ (Space)
        sm.update_state(true, false, false, false, true, true, false, true);
        assert_eq!(sm.current_state, LocomotionState::JumpStart);

        // 空中滞空
        sm.state_elapsed = 0.5;
        sm.update_state(true, false, false, false, true, false, false, false);
        assert_eq!(sm.current_state, LocomotionState::JumpLoop);

        // 着地してキー入力なし -> Idle
        sm.update_state(false, false, false, false, false, false, false, true);
        assert_eq!(sm.current_state, LocomotionState::Idle);
    }
