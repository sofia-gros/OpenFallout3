use std::fs;

fn main() {
    let app_path = "crates/fo3_viewer/src/app.rs";
    let mut content = fs::read_to_string(app_path).unwrap();

    // 1. Find the old dispatcher block and remove it
    let old_dispatcher_str = r#"
        // Q[XNvgCxg̖t[fBXpb` (GameMode [v)
        self.vm.delta_time = dt;
        self.dispatcher.push_event(fo3_script::GameEvent::GameMode);
        let _ = self.dispatcher.process_queue(&mut self.vm);
"#;
    // We will just replace it with empty if found. Wait, regex is safer if whitespace differs.
    // Instead, I'll use regex.
}
