import sys
content = open('crates/fo3_viewer/src/app.rs', 'r', encoding='utf-8').read()
content = content.replace('self.vm.delta_time = dt;', 'self.vm.delta_time = dt as f64;')
open('crates/fo3_viewer/src/app.rs', 'w', encoding='utf-8').write(content)
