use std::fs;
fn main() {
    let content = fs::read_to_string("crates/fo3_script/src/vm.rs").unwrap();
    let lines: Vec<&str> = content.lines().collect();
    let mut depth = 0;
    for (i, line) in lines.iter().enumerate() {
        if i < 255 || i > 565 { continue; }
        for c in line.chars() {
            if c == '{' { depth += 1; }
            if c == '}' { depth -= 1; }
        }
        println!("{:3}: {:2} {}", i + 1, depth, line);
    }
}
