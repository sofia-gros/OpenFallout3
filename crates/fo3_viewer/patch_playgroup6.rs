use std::fs;

fn main() {
    let mut app_content = fs::read_to_string("crates/fo3_viewer/src/app.rs").unwrap();
    let old = "        crate::action::process_teleport_requests(self);\n\n        // 0.2 スクリプトの AI パッケージ・アニメーション要求 (AddScriptPackage / evp) の処理\n        crate::action::process_package_requests(self);";
    let new = "        crate::action::process_teleport_requests(self);\n\n        // 0.2 スクリプトの AI パッケージ・アニメーション要求 (AddScriptPackage / evp) の処理\n        crate::action::process_package_requests(self);\n        crate::action::process_playgroup_requests(self);";
    
    if app_content.contains("process_package_requests(self);") {
        app_content = app_content.replace(old, new);
        let old2 = "        crate::action::process_teleport_requests(self);\r\n\r\n        // 0.2 スクリプトの AI パッケージ・アニメーション要求 (AddScriptPackage / evp) の処理\r\n        crate::action::process_package_requests(self);";
        let new2 = "        crate::action::process_teleport_requests(self);\r\n\r\n        // 0.2 スクリプトの AI パッケージ・アニメーション要求 (AddScriptPackage / evp) の処理\r\n        crate::action::process_package_requests(self);\r\n        crate::action::process_playgroup_requests(self);";
        app_content = app_content.replace(old2, new2);
        fs::write("crates/fo3_viewer/src/app.rs", app_content).unwrap();
    }
}
