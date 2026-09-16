import sys
content = open('crates/fo3_script/src/event.rs', 'r', encoding='utf-8').read()

patch = r'''                        if let Some(blocks) = self.parsed_blocks.get(&script_id) {
                            for block in blocks {
                                if block.event_type == ScriptEventType::GameMode {
                                    println!("[EventDispatcher] calling execute_block for quest {:?}", q_id);
                                    let _ = vm.execute_block(&block.lines, Some(q_id));
                                }
                            }
                        } else {
                            println!("[EventDispatcher] quest {:?} missing script {:?}", q_id, script_id);
                        }'''

content = content.replace('                        if let Some(blocks) = self.parsed_blocks.get(&script_id) {\n                            for block in blocks {\n                                if block.event_type == ScriptEventType::GameMode {\n                                    let _ = vm.execute_block(&block.lines, Some(q_id));\n                                }\n                            }\n                        }', patch)

open('crates/fo3_script/src/event.rs', 'w', encoding='utf-8').write(content)
