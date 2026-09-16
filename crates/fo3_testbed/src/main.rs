use fo3_esm::EsmMasterContext;
use fo3_vfs::VfsManager;
use fo3_esm::types::FormId;
use fo3_script::vm::ScriptVm;
use fo3_script::event::{GameEvent, ScriptEventDispatcher};

fn main() {
    let mut vfs = VfsManager::new();
    let mut ctx = EsmMasterContext::new();
    ctx.load_master("A:/SteamLibrary/steamapps/common/Fallout 3 goty/Data/Fallout3.esm", &mut vfs).unwrap();
    
    let mut vm = ScriptVm::new();
    for (_, q) in ctx.quest_map.clone() {
        vm.edid_map.insert(q.editor_id.to_ascii_uppercase(), q.form_id);
        vm.quest_manager.register_quest(q);
    }
    for (id, scpt) in &ctx.script_map {
        vm.scripts.insert(*id, scpt.clone());
        if !scpt.edid.is_empty() {
            vm.edid_map.insert(scpt.edid.to_ascii_uppercase(), *id);
        }
    }
    let mut dispatcher = ScriptEventDispatcher::new();
    dispatcher.build_indexes(&vm.scripts);

    // Set to Stage 6
    let cg00_id = FormId(0x0001F388);
    vm.set_stage(cg00_id, 6);
    
    // Simulate GameMode for a while
    for _ in 0..1000 {
        vm.delta_time = 0.016;
        dispatcher.push_event(GameEvent::GameMode);
        dispatcher.process_queue(&mut vm).unwrap();
        if vm.get_stage(cg00_id) >= 8 {
            println!("Transitioned to stage {}", vm.get_stage(cg00_id));
            break;
        }
    }
    println!("Final stage: {}, Timer: {:?}", vm.get_stage(cg00_id), vm.quest_manager.get_quest_variable(cg00_id, "timer"));
}
