import sys
content = open('crates/fo3_script/src/event.rs', 'r', encoding='utf-8').read()

lines = content.split('\n')
start = -1
end = -1
for i, line in enumerate(lines):
    if 'GameEvent::MenuMode' in line or 'MenuMode {' in line:
        pass
    if '/// アクターの死亡' in line:
        if start == -1:
            start = i
    if 'pub struct ScriptInstanceContext' in line:
        end = i - 3

if start != -1 and end != -1:
    new_lines = lines[:start] + [
        '    /// アクターの死亡',
        '    /// 参照元: GECK Wiki Begin OnDeath [KillerRef]',
        '    OnDeath {',
        '        /// 死亡したアクターの FormID',
        '        actor: FormId,',
        '        /// 殺害者の FormID (自殺・環境死の場合は FormId(0))',
        '        killer: FormId,',
        '    },',
        '    /// アニメーション終了イベント (カスタム)',
        '    /// 参照元: eferences/openmw/apps/openmw/mwlua/engineevents.hpp:63',
        '    OnAnimationEnd {',
        '        /// 対象アクター',
        '        actor: FormId,',
        '    },',
        '}'
    ] + lines[end+1:]
    open('crates/fo3_script/src/event.rs', 'w', encoding='utf-8').write('\n'.join(new_lines))
    print("Fixed!")
else:
    print("Not found start or end")

