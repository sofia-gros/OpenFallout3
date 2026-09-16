import sys
content = open('crates/fo3_viewer/src/audio.rs', 'r', encoding='utf-8').read()

patch = r'''        println!("[SoundEngine DEBUG] active={}, say_queue={}, line_ended={}, menu={}",
            self.active_subtitles.is_empty(),
            vm.say_queue.is_empty(),
            line_ended_this_frame,
            vm.chargen_menu_active);
        if self.active_subtitles.is_empty()
            && vm.say_queue.is_empty()
            && !line_ended_this_frame
            && !vm.chargen_menu_active
        {'''

content = content.replace('        if self.active_subtitles.is_empty()\n            && vm.say_queue.is_empty()\n            && !line_ended_this_frame\n            && !vm.chargen_menu_active\n        {', patch)

open('crates/fo3_viewer/src/audio.rs', 'w', encoding='utf-8').write(content)
