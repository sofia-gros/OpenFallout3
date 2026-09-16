use std::fs;
fn main() {
    let mut content = fs::read_to_string("crates/fo3_script/src/vm.rs").unwrap();
    let old_globals = "    pub globals: HashMap<String, f32>,\r\n    /// グローバル変数 (`GLOB`) マップ (EDID -> f64)\r\n    pub globals: HashMap<String, f64>,";
    content = content.replace(old_globals, "    pub globals: HashMap<String, f64>,");
    
    // Also fix any other f32 issues
    content = content.replace("b as f32).unwrap_or(-1.0);", "b as f64).unwrap_or(-1.0);");
    fs::write("crates/fo3_script/src/vm.rs", content).unwrap();
}
