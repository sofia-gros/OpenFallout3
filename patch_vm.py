import sys
import re

content = open('crates/fo3_script/src/vm.rs', 'r', encoding='utf-8').read()

# Replace self_id with effective_self_id inside the specific match blocks
def repl_fn(m):
    return m.group(0).replace('self_id', 'effective_self_id')

content = re.sub(r'"moveto" => \{.*?\}', repl_fn, content, flags=re.DOTALL)
content = re.sub(r'"evaluatepackage" \| "evp" => \{.*?\}', repl_fn, content, flags=re.DOTALL)
content = re.sub(r'"say" => \{.*?\}', repl_fn, content, flags=re.DOTALL)
content = re.sub(r'"sayto" => \{.*?\}', repl_fn, content, flags=re.DOTALL)
content = re.sub(r'"addscriptpackage" => \{.*?\}', repl_fn, content, flags=re.DOTALL)

open('crates/fo3_script/src/vm.rs', 'w', encoding='utf-8').write(content)
