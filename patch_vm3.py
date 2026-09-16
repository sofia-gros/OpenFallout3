import sys
content = open('crates/fo3_script/src/vm.rs', 'r', encoding='utf-8').read()
content = content.replace('pub paused_for_movie: bool,', 'pub paused_for_movie: bool,\n    pub chargen_menu_active: bool,')
content = content.replace('paused_for_movie: false,', 'paused_for_movie: false,\n            chargen_menu_active: false,')
open('crates/fo3_script/src/vm.rs', 'w', encoding='utf-8').write(content)
