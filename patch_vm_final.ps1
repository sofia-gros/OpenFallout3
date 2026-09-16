$content = Get-Content crates/fo3_script/src/vm.rs -Raw

# 1. Add fields to ScriptVm
$patch_fields = "    pub teleport_requests: Vec<(Option<FormId>, String)>,`n    pub chargen_menu_active: bool,`n    pub playgroup_queue: Vec<(FormId, String)>,"
$content = $content.Replace("    pub teleport_requests: Vec<(Option<FormId>, String)>,", $patch_fields)

# 2. Add defaults
$patch_defaults = "            teleport_requests: Vec::new(),`n            chargen_menu_active: false,`n            playgroup_queue: Vec::new(),"
$content = $content.Replace("            teleport_requests: Vec::new(),", $patch_defaults)

# 3. Replace self_id with effective_self_id in vm.rs execute_statement
$content = $content.Replace('self.add_script_package_requests.push((self_id, pkg_id));', 'self.add_script_package_requests.push((effective_self_id, pkg_id));')
$content = $content.Replace('self.teleport_requests.push((self_id, target_marker));', 'self.teleport_requests.push((effective_self_id, target_marker));')
$content = $content.Replace('println!("[Script] MoveTo 要求: subject={:?}, target={}", self_id, target_marker);', 'println!("[Script] MoveTo 要求: subject={:?}, target={}", effective_self_id, target_marker);')
$content = $content.Replace('println!("[Script] EvaluatePackage (AI パッケージ再評価要求): subject={:?}", self_id);', 'println!("[Script] EvaluatePackage (AI パッケージ再評価要求): subject={:?}", effective_self_id);')
$content = $content.Replace('self.evaluate_package_requests.push(self_id);', 'self.evaluate_package_requests.push(effective_self_id);')
$content = $content.Replace('self.say_queue.push((self_id, topic));', 'self.say_queue.push((effective_self_id, topic));')
$content = $content.Replace('println!("[Script] Say (台詞発言要求): topic={:?}, speaker={:?}", topic, self_id);', 'println!("[Script] Say (台詞発言要求): topic={:?}, speaker={:?}", topic, effective_self_id);')
$content = $content.Replace('println!("[Script] SayTo (台詞発言要求): topic={:?}, speaker={:?}, target_str={:?}", topic, self_id, parts[1]);', 'println!("[Script] SayTo (台詞発言要求): topic={:?}, speaker={:?}, target_str={:?}", topic, effective_self_id, parts[1]);')
$content = $content.Replace('println!("[Script] SayTo (台詞発言要求): topic={:?}, speaker={:?}", topic, self_id);', 'println!("[Script] SayTo (台詞発言要求): topic={:?}", topic, effective_self_id);')

# 4. Add match cmd {
$patch_match = "        let cmd = cmd.as_str();`n        let parts = cmd_parts;`n        match cmd {"
$content = $content.Replace("        let cmd = cmd.as_str();`r`n        let parts = cmd_parts;", $patch_match)

# 5. Fix set command
$patch_set = @"
                    // 1. Check if global
                    let global_key = self.globals.keys()
                        .find(|k| k.eq_ignore_ascii_case(var_name))
                        .cloned();
                    if let Some(k) = global_key {
                        self.globals.insert(k, val);
                    } else if let Some((prefix, sub)) = lower_var.split_once('.') {
                        // 2. Cross reference set (e.g. CG00.timer)
                        let mut target_q_id = None;
                        if let Some(q_id) = self.edid_map.get(prefix).or_else(|| self.edid_map.get(&prefix.to_ascii_uppercase())) {
                            if self.quest_manager.quests.contains_key(q_id) {
                                target_q_id = Some(*q_id);
                            }
                        }
                        if let Some(q_id) = target_q_id {
                            println!("[DEBUG] TARGET Q_ID FOUND FOR {}: {:?}", prefix, q_id);
                            self.quest_manager.set_quest_variable(q_id, sub, val);
                        } else {
                            println!("[DEBUG] TARGET Q_ID NOT FOUND FOR {}", prefix);
                            self.locals.insert(format!("{}.{}", prefix, sub), val);
                            self.locals.insert(sub.to_string(), val);
                        }
                    } else {
                        // 3. Local variable set
                        if let Some(q_id) = self_id {
                            if self.quest_manager.quests.contains_key(&q_id) {
                                self.quest_manager.set_quest_variable(q_id, &lower_var, val);
                            }
                        }
                        self.locals.insert(lower_var, val);
                    }
"@

$old_set = @"
                    // 変数解決 (case-insensitive 検索)
                    let global_key = self.globals.keys()
                        .find(|k| k.eq_ignore_ascii_case(var_name))
                        .cloned();
                    if let Some(k) = global_key {
                        self.globals.insert(k, val);
                    } else if let Some((prefix, sub)) = lower_var.split_once('.') {
                        // ドット表記: "cg00.timer" は locals["cg00.timer"] と locals["timer"] の両方に格納
                        // 理由: GECK での変数解決の曖昧さ (QuestEditorID.VarName 対策)
                        self.locals.insert(format!("{}.{}", prefix, sub), val);
                        self.locals.insert(sub.to_string(), val);
                    } else {
                        // 単体名: そのままローカル変数 (cg00. のコンテキストを想定)
                        self.locals.insert(lower_var, val);
                    }
"@

$old_set2 = @"
                    // グローバル変数 (case-insensitive で検索)
                    let global_key = self.globals.keys()
                        .find(|k| k.eq_ignore_ascii_case(var_name))
                        .cloned();
                    if let Some(k) = global_key {
                        self.globals.insert(k, val);
                    } else if let Some((prefix, sub)) = lower_var.split_once('.') {
                        // ドット付き変数: "cg00.timer" → locals["cg00.timer"] と locals["timer"] 両方へ格納
                        // 参照元: GECK スクリプト変数命名規約 (QuestEditorID.VarName 形式)
                        self.locals.insert(format!("{}.{}", prefix, sub), val);
                        self.locals.insert(sub.to_string(), val);
                    } else {
                        // ベア変数: そのままローカルテーブルへ (cg00. 自動付与は廃止)
                        self.locals.insert(lower_var, val);
                    }
"@

$content = $content.Replace($old_set, $patch_set)
$content = $content.Replace($old_set2, $patch_set)

Set-Content crates/fo3_script/src/vm.rs -Value $content -NoNewline
