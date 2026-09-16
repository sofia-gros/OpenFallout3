import sys
content = open('crates/fo3_script/src/vm.rs', 'r', encoding='utf-8').read()
content = content.replace('let mut if_stack: Vec<(bool, bool)> = Vec::new();', 'let mut if_stack: Vec<(bool, bool)> = Vec::new();\n        println!("[EXEC_BLOCK] START BLOCK");')
content = content.replace('let parts: Vec<&str> = trimmed.split_whitespace().collect();', 'println!("[EXEC_BLOCK] line: {}", trimmed);\n            let parts: Vec<&str> = trimmed.split_whitespace().collect();')
open('crates/fo3_script/src/vm.rs', 'w', encoding='utf-8').write(content)
