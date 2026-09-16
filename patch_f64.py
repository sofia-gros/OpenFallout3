import sys
content = open('crates/fo3_script/src/event.rs', 'r', encoding='utf-8').read()

content = content.replace('HashMap<String, f32>', 'HashMap<String, f64>')

open('crates/fo3_script/src/event.rs', 'w', encoding='utf-8').write(content)
