use std::fs;
fn main() {
    let mut content = fs::read_to_string("crates/fo3_script/src/vm.rs").unwrap();
    
    let old_struct_end = "    pub pending_stage_scripts: std::collections::VecDeque<(FormId, u16, String)>,\n}";
    let new_struct_end = "    pub pending_stage_scripts: std::collections::VecDeque<(FormId, u16, String)>,\n    pub playgroup_queue: Vec<(FormId, String)>,\n    pub chargen_menu_active: bool,\n    pub paused_for_movie: bool,\n}";
    
    let old_struct_end_crlf = "    pub pending_stage_scripts: std::collections::VecDeque<(FormId, u16, String)>,\r\n}";
    let new_struct_end_crlf = "    pub pending_stage_scripts: std::collections::VecDeque<(FormId, u16, String)>,\r\n    pub playgroup_queue: Vec<(FormId, String)>,\r\n    pub chargen_menu_active: bool,\r\n    pub paused_for_movie: bool,\r\n}";
    
    content = content.replace(old_struct_end, new_struct_end);
    content = content.replace(old_struct_end_crlf, new_struct_end_crlf);
    
    let old_default_end = "            pending_stage_scripts: std::collections::VecDeque::new(),\n        }\n    }";
    let new_default_end = "            pending_stage_scripts: std::collections::VecDeque::new(),\n            playgroup_queue: Vec::new(),\n            chargen_menu_active: false,\n            paused_for_movie: false,\n        }\n    }";
    
    let old_default_end_crlf = "            pending_stage_scripts: std::collections::VecDeque::new(),\r\n        }\r\n    }";
    let new_default_end_crlf = "            pending_stage_scripts: std::collections::VecDeque::new(),\r\n            playgroup_queue: Vec::new(),\r\n            chargen_menu_active: false,\r\n            paused_for_movie: false,\r\n        }\r\n    }";

    content = content.replace(old_default_end, new_default_end);
    content = content.replace(old_default_end_crlf, new_default_end_crlf);

    fs::write("crates/fo3_script/src/vm.rs", content).unwrap();
}
