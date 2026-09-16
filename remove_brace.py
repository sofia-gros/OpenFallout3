lines = open('crates/fo3_script/src/vm.rs', 'r', encoding='utf-8').read().splitlines()
if lines[-1].strip() == '}':
    lines.pop()
open('crates/fo3_script/src/vm.rs', 'w', encoding='utf-8').write('\n'.join(lines))
