import sys

content = open('crates/fo3_script/src/vm.rs', 'r', encoding='utf-8').read()

content = content.replace(
    'self.add_script_package_requests.push((self_id, pkg_id));',
    'self.add_script_package_requests.push((effective_self_id, pkg_id));'
)
content = content.replace(
    'self.teleport_requests.push((self_id, target_marker));',
    'self.teleport_requests.push((effective_self_id, target_marker));'
)
content = content.replace(
    'println!("[Script] MoveTo ??: subject={:?}, target={}", self_id, target_marker);',
    'println!("[Script] MoveTo ??: subject={:?}, target={}", effective_self_id, target_marker);'
)
content = content.replace(
    'println!("[Script] EvaluatePackage (AI ?????????): subject={:?}", self_id);',
    'println!("[Script] EvaluatePackage (AI ?????????): subject={:?}", effective_self_id);'
)
content = content.replace(
    'self.evaluate_package_requests.push(self_id);',
    'self.evaluate_package_requests.push(effective_self_id);'
)
content = content.replace(
    'self.say_queue.push((self_id, topic));',
    'self.say_queue.push((effective_self_id, topic));'
)
content = content.replace(
    'println!("[Script] Say (??????): topic={:?}, speaker={:?}", topic, self_id);',
    'println!("[Script] Say (??????): topic={:?}, speaker={:?}", topic, effective_self_id);'
)
content = content.replace(
    'println!("[Script] SayTo (??????): topic={:?}, speaker={:?}, target_str={:?}", topic, self_id, parts[1]);',
    'println!("[Script] SayTo (??????): topic={:?}, speaker={:?}, target_str={:?}", topic, effective_self_id, parts[1]);'
)
content = content.replace(
    'println!("[Script] SayTo (??????): topic={:?}, speaker={:?}", topic, self_id);',
    'println!("[Script] SayTo (??????): topic={:?}, speaker={:?}", topic, effective_self_id);'
)

open('crates/fo3_script/src/vm.rs', 'w', encoding='utf-8').write(content)
