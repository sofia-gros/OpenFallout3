import sys
content = open('crates/fo3_viewer/tests/new_game_simulation_test.rs', 'r', encoding='utf-8').read()

# First, remove all existing digest_pending
content = content.replace('digest_pending(&mut vm);\n', '')
content = content.replace('    let mut digest_pending = |vm: &mut fo3_script::ScriptVm| {\n        while let Some((_quest_id, _stage, script)) = vm.pending_stage_scripts.pop_front() {\n            if let Err(e) = vm.execute_result_script(&script, None) {\n                println!("[TEST ERR] {}", e);\n            }\n        }\n    };', '')

# Redefine digest_pending at the top of the test
patch_def = r'''fn digest_pending(vm: &mut fo3_script::ScriptVm) {
    while let Some((_quest_id, _stage, script)) = vm.pending_stage_scripts.pop_front() {
        if let Err(e) = vm.execute_result_script(&script, None) {
            println!("[TEST ERR] {}", e);
        }
    }
}

#[test]
fn test_new_game_cg00_progression_simulation() {'''

content = content.replace('#[test]\nfn test_new_game_cg00_progression_simulation() {', patch_def)

# Replace all dispatcher.process_queue with it + digest
content = content.replace('dispatcher.process_queue(&mut vm).unwrap();', 'dispatcher.process_queue(&mut vm).unwrap();\n    digest_pending(&mut vm);')

# Also for vm.set_stage(cg00_id, 0);
content = content.replace('vm.set_stage(cg00_id, 0);', 'vm.set_stage(cg00_id, 0);\n    digest_pending(&mut vm);')

open('crates/fo3_viewer/tests/new_game_simulation_test.rs', 'w', encoding='utf-8').write(content)
