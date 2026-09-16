use std::fs;

fn main() {
    let vm_path = "crates/fo3_script/src/vm.rs";
    let event_path = "crates/fo3_script/src/event.rs";

    let mut vm_content = fs::read_to_string(vm_path).unwrap();
    vm_content = vm_content.replace("pub globals: HashMap<String, f32>", "pub globals: HashMap<String, f64>");
    vm_content = vm_content.replace("pub locals: HashMap<String, f32>", "pub locals: HashMap<String, f64>");
    vm_content = vm_content.replace("pub fn eval_expr(&self, expr: &str) -> f32", "pub fn eval_expr(&self, expr: &str) -> f64");
    vm_content = vm_content.replace("pub fn resolve_value(&self, token: &str) -> f32", "pub fn resolve_value(&self, token: &str) -> f64");
    vm_content = vm_content.replace("return v;", "return v;"); // keep
    // Change f32 to f64 inside eval methods
    vm_content = vm_content.replace("parse::<f32>()", "parse::<f64>()");
    vm_content = vm_content.replace("f32>", "f64>");
    vm_content = vm_content.replace(" 1e-4", " 1e-6"); // precision
    
    // In resolve_value
    vm_content = vm_content.replace("map(|b| b as f32)", "map(|b| b as f64)");
    vm_content = vm_content.replace("self.get_stage(qid) as f32", "self.get_stage(qid) as f64");
    
    fs::write(vm_path, vm_content).unwrap();

    let mut event_content = fs::read_to_string(event_path).unwrap();
    event_content = event_content.replace("local_vars: HashMap<String, f32>", "local_vars: HashMap<String, f64>");
    fs::write(event_path, event_content).unwrap();
}
