use std::fs;
let mut code = fs::read_to_string("crates/fo3_script/src/quest.rs").unwrap();
code = code.replace(
    "pub fn set_quest_variable(&mut self, quest: FormId, name: &str, value: f64) {",
    "pub fn set_quest_variable(&mut self, quest: FormId, name: &str, value: f64) { println!("DEBUG: set_quest_variable 0x{:08X}.{} = {}", quest.0, name, value);"
);
code = code.replace(
    "pub fn get_quest_variable(&self, quest: FormId, name: &str) -> Option<f64> {",
    "pub fn get_quest_variable(&self, quest: FormId, name: &str) -> Option<f64> { let res = self.quest_variables.get(&quest).and_then(|vars| vars.get(&name.to_ascii_lowercase()).copied()); println!("DEBUG: get_quest_variable 0x{:08X}.{} -> {:?}", quest.0, name, res); return res; "
);
fs::write("crates/fo3_script/src/quest.rs", code).unwrap();
