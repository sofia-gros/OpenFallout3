import sys
content = open('crates/fo3_script/src/event.rs', 'r', encoding='utf-8').read()

import re
# Add OnAnimationEnd if not present
if 'OnAnimationEnd' not in content:
    content = re.sub(r'killer: FormId,\s*\n\s*},', 'killer: FormId,\n    },\n    /// アニメーション終了イベント (カスタム)\n    /// 参照元: eferences/openmw/apps/openmw/mwlua/engineevents.hpp:63\n    OnAnimationEnd {\n        /// 対象アクター\n        actor: FormId,\n    },', content, count=1)

open('crates/fo3_script/src/event.rs', 'w', encoding='utf-8').write(content)

quest_content = open('crates/fo3_script/src/quest.rs', 'r', encoding='utf-8').read()
quest_content = quest_content.replace('quest_variables: HashMap<FormId, HashMap<String, f64>>', 'pub quest_variables: HashMap<FormId, HashMap<String, f64>>')
open('crates/fo3_script/src/quest.rs', 'w', encoding='utf-8').write(quest_content)

vm_content = open('crates/fo3_script/src/vm.rs', 'r', encoding='utf-8').read()
vm_content = vm_content.replace('quest.variables.insert(sub.to_string(), val);', 'self.quest_manager.set_quest_variable(q_id, &sub, val);')
vm_content = vm_content.replace('quest.variables.insert(lower_var.clone(), val);', 'self.quest_manager.set_quest_variable(id, &lower_var, val);')
open('crates/fo3_script/src/vm.rs', 'w', encoding='utf-8').write(vm_content)
