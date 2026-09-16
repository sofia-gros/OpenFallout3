import sys
content = open('crates/fo3_viewer/src/audio.rs', 'r', encoding='utf-8').read()

patch = r'''        if self.active_subtitles.is_empty()
            && vm.say_queue.is_empty()
            && !line_ended_this_frame
            && !vm.chargen_menu_active
        {
            let dad_talking = vm.locals.get("cg00dadref.dotalk").copied().unwrap_or(0.0) == 1.0;
            let mom_talking = vm.locals.get("cg00momref.dotalk").copied().unwrap_or(0.0) == 1.0;
            let drli_talking = vm.locals.get("cg00doctorliref.dotalk").copied().unwrap_or(0.0) == 1.0;
            println!("[SoundEngine] Autonomous check! dad={}, mom={}, drli={}", dad_talking, mom_talking, drli_talking);'''

content = content.replace('        if self.active_subtitles.is_empty()\n            && vm.say_queue.is_empty()\n            && !line_ended_this_frame\n            && !vm.chargen_menu_active\n        {\n            let dad_talking = vm.locals.get("cg00dadref.dotalk").copied().unwrap_or(0.0) == 1.0;\n            let mom_talking = vm.locals.get("cg00momref.dotalk").copied().unwrap_or(0.0) == 1.0;\n            let drli_talking = vm.locals.get("cg00doctorliref.dotalk").copied().unwrap_or(0.0) == 1.0;', patch)

open('crates/fo3_viewer/src/audio.rs', 'w', encoding='utf-8').write(content)
