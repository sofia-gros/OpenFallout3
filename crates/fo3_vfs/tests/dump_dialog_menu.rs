#[test]
fn test_dump_dialog_menu() {
    let data_dir = "A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data";
    if !std::path::Path::new(data_dir).exists() {
        eprintln!("Data dir not found");
        return;
    }

    let bsa_path = format!("{}\\Fallout - Misc.bsa", data_dir);
    let mut bsa = fo3_bsa::BsaArchive::open(&bsa_path).expect("open bsa");
    let content = bsa.extract_file("menus\\dialog\\dialog_menu.xml").expect("extract dialog_menu.xml");
    let text = String::from_utf8_lossy(&content);
    println!("=== dialog_menu.xml (len = {}) ===", text.len());
    println!("{}", text);
}

#[test]
fn test_dump_prefabs() {
    let data_dir = "A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data";
    if !std::path::Path::new(data_dir).exists() {
        return;
    }
    let bsa_path = format!("{}\\Fallout - Misc.bsa", data_dir);
    let mut bsa = fo3_bsa::BsaArchive::open(&bsa_path).expect("open bsa");
    for path in [
        "menus\\prefabs\\top_bracket.xml",
        "menus\\prefabs\\bottom_bracket.xml",
        "menus\\prefabs\\list_box.xml",
        "menus\\prefabs\\list_box_template.xml",
        "menus\\prefabs\\box.xml",
    ] {
        if let Ok(bytes) = bsa.extract_file(path) {
            println!("=== {} (len = {}) ===", path, bytes.len());
            println!("{}", String::from_utf8_lossy(&bytes));
        } else {
            // menus\prefabs\ 以外も検索
            println!("Not found: {}", path);
        }
    }
}

#[test]
fn test_dump_tai() {
    let data_dir = "A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data";
    if !std::path::Path::new(data_dir).exists() {
        return;
    }
    let bsa_path = format!("{}\\Fallout - Misc.bsa", data_dir);
    let mut bsa = fo3_bsa::BsaArchive::open(&bsa_path).expect("open bsa");
    if let Ok(bytes) = bsa.extract_file("Interface\\InterfaceShared.tai") {
        println!("=== InterfaceShared.tai (len = {}) ===", bytes.len());
        println!("{}", String::from_utf8_lossy(&bytes[..bytes.len().min(500)]));
    } else {
        println!("Not found: Interface\\InterfaceShared.tai");
    }
}

#[test]
fn test_find_tai() {
    let data_dir = "A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data";
    for entry in std::fs::read_dir(data_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().map(|e| e == "bsa").unwrap_or(false) {
            if let Ok(mut bsa) = fo3_bsa::BsaArchive::open(&path) {
                for f in bsa.list_files() {
                    if f.ends_with(".tai") || f.contains("solid_black") {
                        println!("Found {} in {:?}", f, path.file_name().unwrap());
                    }
                }
            }
        }
    }
}


#[test]
fn test_read_tai_content() {
    let data_dir = "A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data";
    let bsa_path = format!("{}\\Fallout - Textures.bsa", data_dir);
    let mut bsa = fo3_bsa::BsaArchive::open(&bsa_path).expect("open bsa");
    if let Ok(bytes) = bsa.extract_file("textures\\interface\\interfaceshared.tai") {
        println!("=== interfaceshared.tai (len = {}) ===", bytes.len());
        println!("{}", String::from_utf8_lossy(&bytes[..bytes.len().min(1000)]));
    }
}

#[test]
fn test_check_tai_entries() {
    let data_dir = "A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data";
    let bsa_path = format!("{}\\Fallout - Textures.bsa", data_dir);
    let mut bsa = fo3_bsa::BsaArchive::open(&bsa_path).expect("open bsa");
    if let Ok(bytes) = bsa.extract_file("textures\\interface\\interfaceshared.tai") {
        let text = String::from_utf8_lossy(&bytes);
        for line in text.lines() {
            if line.contains("solid.dds") || line.contains("fade_to_bottom") || line.contains("fade_to_top") {
                println!("TAI entry: {}", line);
            }
        }
    }
}

#[test]
fn test_find_cg00_kf() {
    let data_dir = "A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data";
    let meshes_bsa_path = format!("{}\\Fallout - Meshes.bsa", data_dir);
    if let Ok(bsa) = fo3_bsa::BsaArchive::open(&meshes_bsa_path) {
        let kf_files: Vec<_> = bsa.list_files().iter().filter(|f| f.to_ascii_lowercase().contains("cg00") && f.ends_with(".kf")).cloned().collect();
        println!("=== Found {} CG00 KF files ===", kf_files.len());
        for f in kf_files {
            println!("  KF: {}", f);
        }
    }
}

