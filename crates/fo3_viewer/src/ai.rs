use std::collections::HashMap;
use fo3_esm::types::FormId;
use fo3_esm::EsmMasterContext;
use fo3_script::ScriptVm;

pub struct ActorAiState {
    pub form_id: FormId,
    pub base_form_id: FormId,
    /// NPCのベースパッケージ群
    pub base_packages: Vec<FormId>,
    /// スクリプトで追加されたパッケージ群
    pub script_packages: Vec<FormId>,
    /// 現在アクティブなパッケージ
    pub current_package: Option<FormId>,
}

#[derive(Default)]
pub struct AiManager {
    pub actors: HashMap<FormId, ActorAiState>,
}

impl AiManager {
    pub fn new() -> Self {
        Self {
            actors: HashMap::new(),
        }
    }

    /// アクターをAI管理下に登録する
    pub fn register_actor(&mut self, form_id: FormId, base_form_id: FormId, master: &EsmMasterContext) {
        let mut base_packages = Vec::new();
        if let Some(npc) = master.npc_map.get(&base_form_id) {
            base_packages.clone_from(&npc.ai_packages);
        }
        self.actors.insert(form_id, ActorAiState {
            form_id,
            base_form_id,
            base_packages,
            script_packages: Vec::new(),
            current_package: None,
        });
    }

    /// 毎フレーム呼ばれ、全アクターのAIパッケージの条件を評価する
    pub fn update(&mut self, vm: &mut ScriptVm, master: &EsmMasterContext) {
        for state in self.actors.values_mut() {
            let mut active_pack = None;

            // スクリプトパッケージを優先して評価
            for pkg_id in state.script_packages.iter().chain(state.base_packages.iter()) {
                if let Some(pack) = master.pack_map.get(pkg_id) {
                    if pack.conditions.is_empty() {
                        active_pack = Some(*pkg_id);
                        break;
                    }

                    let cond_ctx = fo3_script::ConditionContext {
                        speaker: Some(state.form_id),
                        target: None,
                        speaker_pos: glam::Vec3::ZERO,
                        player_pos: glam::Vec3::ZERO,
                        quest_stages: vm.quest_stages.clone(),
                        quest_stage_history: std::collections::HashMap::new(),
                        inventory: vm.inventory.clone(),
                        is_female: vm.player_is_female,
                    };

                    // 全条件が true か評価
                    if fo3_script::evaluate_conditions(&pack.conditions, &cond_ctx) {
                        active_pack = Some(*pkg_id);
                        break; // 条件に合致する最初のパッケージを選択
                    }
                }
            }

            // アクティブなパッケージが切り替わった場合
            if state.current_package != active_pack {
                state.current_package = active_pack;

                if let Some(pack_id) = active_pack {
                    if let Some(pack) = master.pack_map.get(&pack_id) {
                        if let Some(topic_id) = pack.topic_id {
                            // Topic FormID から EDID を逆引き
                            if let Some((dial, _)) = master.topic_map.values().find(|(d, _)| d.form_id == topic_id) {
                                println!("[AiManager] NPC 0x{:08X} のパッケージ 0x{:08X} が発火。Topic: {}", state.form_id.0, pack_id.0, dial.edid);
                                vm.say_queue.push((Some(state.form_id), dial.edid.clone()));
                            }
                        }
                    }
                }
            }
        }
    }
}
