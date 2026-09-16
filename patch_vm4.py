import sys
content = open('crates/fo3_script/src/vm.rs', 'r', encoding='utf-8').read()
content = content.replace('let cmd = cmd.as_str();\n        let parts = cmd_parts;', 'let cmd = cmd.as_str();\n        println!("[EXEC] cmd: {}, line: {}", cmd, line);\n        let parts = cmd_parts;')
open('crates/fo3_script/src/vm.rs', 'w', encoding='utf-8').write(content)
