import sys
import re

# mod.rs
mod_content = open('crates/fo3_nif/src/blocks/mod.rs', 'r', encoding='utf-8').read()
if 'NiTransformController,' not in mod_content:
    mod_content = mod_content.replace('NiTransformData,', 'NiTransformData, NiTransformController,')
    mod_content = mod_content.replace('NiTransformData(NiTransformData),', 'NiTransformData(NiTransformData),\n    NiTransformController(NiTransformController),')
open('crates/fo3_nif/src/blocks/mod.rs', 'w', encoding='utf-8').write(mod_content)

# file.rs
file_content = open('crates/fo3_nif/src/file.rs', 'r', encoding='utf-8').read()
if 'NiTransformController::read' not in file_content:
    patch = '''                "NiTransformController" => match NiTransformController::read(&mut cursor) {
                    Ok(ctrl) => NifBlock::NiTransformController(ctrl),
                    Err(e) => {
                        eprintln!("[WARN] NiTransformControllerパース失敗: {:?}", e);
                        NifBlock::Unknown
                    }
                },
                "NiTransformData"'''
    file_content = file_content.replace('"NiTransformData"', patch, 1)
open('crates/fo3_nif/src/file.rs', 'w', encoding='utf-8').write(file_content)
