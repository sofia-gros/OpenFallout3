import sys
content = open('crates/fo3_script/src/vm.rs', 'r', encoding='utf-8').read()

patch = r'''                    let global_key = self.globals.keys()
                        .find(|k| k.eq_ignore_ascii_case(var_name))
                        .cloned();
                    if let Some(k) = global_key {
                        println!("[DEBUG] Setting GLOBAL {} to {}", k, val);
                        self.globals.insert(k, val);
                    }'''

content = content.replace(
'''                    let global_key = self.globals.keys()
                        .find(|k| k.eq_ignore_ascii_case(var_name))
                        .cloned();
                    if let Some(k) = global_key {
                        self.globals.insert(k, val);
                    }''', patch)

open('crates/fo3_script/src/vm.rs', 'w', encoding='utf-8').write(content)
