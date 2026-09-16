use std::fs;

fn main() {
    let mut content = fs::read_to_string("crates/fo3_viewer/tests/new_game_simulation_test.rs").unwrap();
    let old = "// 1フレーム目: timer = 0.01 - 0.016 = -0.006\n    dispatcher.push_event(GameEvent::GameMode);";
    let new = "// 1フレーム目: timer = 0.01 - 0.016 = -0.006\n    vm.delta_time = 0.016;\n    dispatcher.push_event(GameEvent::GameMode);";
    content = content.replace(old, new);

    let old_crlf = "// 1フレーム目: timer = 0.01 - 0.016 = -0.006\r\n    dispatcher.push_event(GameEvent::GameMode);";
    let new_crlf = "// 1フレーム目: timer = 0.01 - 0.016 = -0.006\r\n    vm.delta_time = 0.016;\r\n    dispatcher.push_event(GameEvent::GameMode);";
    content = content.replace(old_crlf, new_crlf);

    let old2 = "// 2フレーム目: timer <= 0 のため if getstage CG00 == 5 -> setstage CG00 6\n    dispatcher.push_event(GameEvent::GameMode);";
    let new2 = "// 2フレーム目: timer <= 0 のため if getstage CG00 == 5 -> setstage CG00 6\n    vm.delta_time = 0.016;\n    dispatcher.push_event(GameEvent::GameMode);";
    content = content.replace(old2, new2);

    let old2_crlf = "// 2フレーム目: timer <= 0 のため if getstage CG00 == 5 -> setstage CG00 6\r\n    dispatcher.push_event(GameEvent::GameMode);";
    let new2_crlf = "// 2フレーム目: timer <= 0 のため if getstage CG00 == 5 -> setstage CG00 6\r\n    vm.delta_time = 0.016;\r\n    dispatcher.push_event(GameEvent::GameMode);";
    content = content.replace(old2_crlf, new2_crlf);

    fs::write("crates/fo3_viewer/tests/new_game_simulation_test.rs", content).unwrap();
}
