import sys
content = open('crates/fo3_script/src/vm.rs', 'r', encoding='utf-8').read()
content = content.replace('let cmd = cmd.as_str();\n        let parts = cmd_parts;\n            "setstage" => {', 'let cmd = cmd.as_str();\n        let parts = cmd_parts;\n        match cmd {\n            "setstage" => {')
open('crates/fo3_script/src/vm.rs', 'w', encoding='utf-8').write(content)
