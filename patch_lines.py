import sys
content = open('crates/fo3_script/src/event.rs', 'r', encoding='utf-8').read()
lines = content.split('\n')
new_lines = lines[:59] + [
    '    },',
    '    /// アニメーション終了イベント (カスタム)',
    '    /// 参照元: eferences/openmw/apps/openmw/mwlua/engineevents.hpp:63',
    '    OnAnimationEnd {',
    '        /// 対象アクター',
    '        actor: FormId,',
    '    },',
    '}',
] + lines[75:]
open('crates/fo3_script/src/event.rs', 'w', encoding='utf-8').write('\n'.join(new_lines))
