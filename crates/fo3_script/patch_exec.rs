use std::fs;

fn main() {
    let mut content = fs::read_to_string("crates/fo3_script/src/vm.rs").unwrap();
    
    // In execute_block
    let old_if_say = r#"            if first == "say" {
                if parts.len() > 1 {
                    let topic = parts[1].to_string();
                    self.say_queue.push((self_id, topic));
                }
                continue;
            }"#;
            
    let new_if_playgroup = r#"            if first == "say" {
                if parts.len() > 1 {
                    let topic = parts[1].to_string();
                    self.say_queue.push((self_id, topic));
                }
                continue;
            }

            if first == "playgroup" || first == "playanim" {
                if parts.len() > 1 {
                    let anim_name = parts[1].to_string();
                    if let Some(target) = self_id {
                        self.playgroup_queue.push((target, anim_name));
                    }
                }
                continue;
            }"#;
            
    content = content.replace(old_if_say, new_if_playgroup);
    fs::write("crates/fo3_script/src/vm.rs", content).unwrap();
}
