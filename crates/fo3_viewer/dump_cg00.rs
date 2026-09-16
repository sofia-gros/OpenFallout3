use fo3_esm::EsmMasterContext;
use fo3_vfs::VfsManager;
use fo3_esm::types::FormId;

fn main() {
    let mut vfs = VfsManager::new();
    let mut ctx = EsmMasterContext::new();
    ctx.load_master("A:/Project/OpenFallout3/data/Fallout3.esm", &mut vfs).unwrap();
    let script = ctx.scripts.get(&FormId(0x0001F388)).unwrap();
    println!("{}", script.source);
}
