import sys
content = open('crates/fo3_viewer/src/loader.rs', 'r', encoding='utf-8').read()

patch = '''
                                    .and_then(|pkg| {
                                        let mut kf_paths = crate::action::resolve_idle_kf_from_pack(master_context, pkg);
                                        // Fallback to mtidle.kf if package has no animations
                                        if kf_paths.is_empty() {
                                            kf_paths.push("meshes\\\\characters\\\\_male\\\\idleanims\\\\mtidle.kf".to_string());
                                        }
                                        for path in &kf_paths {
'''
content = content.replace('''
                                    .and_then(|pkg| {
                                        let kf_paths = crate::action::resolve_idle_kf_from_pack(master_context, pkg);
                                        for path in &kf_paths {
''', patch)

patch2 = '''
                                    let result = match resolved {
                                        Some(pair) => (Some(pair.0), Some(pair.1)),
                                        None => {
                                            // どのパッケージも条件を満たさない（またはアニメーション解決に失敗した）場合も mtidle.kf にフォールバック
                                            if let Ok(bytes) = vfs.read("meshes\\\\characters\\\\_male\\\\idleanims\\\\mtidle.kf") {
                                                let mut cursor = std::io::Cursor::new(bytes);
                                                if let Ok(kf) = fo3_nif::NifFile::read(&mut cursor) {
                                                    if let Some(clip) = fo3_render::animation::AnimationClip::from_kf(&kf) {
                                                        (Some(std::sync::Arc::new(kf)), Some(std::sync::Arc::new(clip)))
                                                    } else {
                                                        (None, None)
                                                    }
                                                } else {
                                                    (None, None)
                                                }
                                            } else {
                                                (None, None)
                                            }
                                        }
                                    };
'''
content = content.replace('''
                                    let result = match resolved {
                                        Some(pair) => (Some(pair.0), Some(pair.1)),
                                        None => (None, None),
                                    };
''', patch2)

open('crates/fo3_viewer/src/loader.rs', 'w', encoding='utf-8').write(content)
