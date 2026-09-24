use fo3_esm::types::FormId;
use fo3_esm::EsmMasterContext;
use fo3_script::ScriptVm;
use std::collections::HashMap;

pub struct ActorAiState {
    pub form_id: FormId,
    pub base_form_id: FormId,
    /// NPCのベースパッケージ群
    pub base_packages: Vec<FormId>,
    /// スクリプトで追加されたパッケージ群
    pub script_packages: Vec<FormId>,
    /// 現在アクティブなパッケージ
    pub current_package: Option<FormId>,
    /// 算出された移動経路
    pub current_path: Option<fo3_navigation::NavPath>,
    /// 移動経路の現在の目標インデックス
    pub path_target_index: usize,
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
    pub fn register_actor(
        &mut self,
        form_id: FormId,
        base_form_id: FormId,
        master: &EsmMasterContext,
    ) {
        let mut base_packages = Vec::new();
        if let Some(npc) = master.npc_map.get(&base_form_id) {
            base_packages.clone_from(&npc.ai_packages);
        }
        self.actors.insert(
            form_id,
            ActorAiState {
                form_id,
                base_form_id,
                base_packages,
                script_packages: Vec::new(),
                current_package: None,
                current_path: None,
                path_target_index: 0,
            },
        );
    }

    /// 毎フレーム呼ばれ、全アクターのAIパッケージの条件を評価する
    pub fn update(
        &mut self,
        vm: &mut ScriptVm,
        master: &EsmMasterContext,
        nav_graph: &fo3_navigation::NavGraph,
        actor_positions: &HashMap<FormId, glam::Vec3>,
    ) {
        for state in self.actors.values_mut() {
            let mut active_pack = None;

            // 優先度順に条件評価
            for pkg_id in state
                .script_packages
                .iter()
                .chain(state.base_packages.iter())
            {
                if let Some(pack) = master.pack_map.get(pkg_id) {
                    if pack.conditions.is_empty() {
                        active_pack = Some(*pkg_id);
                        break;
                    }

                    // スクリプト変数の逆引き (VMの string key から FormID へ)
                    let mut script_vars = std::collections::HashMap::new();
                    for (k, v) in &vm.globals {
                        if let Some((prefix, _sub)) = k.split_once('.') {
                            if let Some(&fid) = vm.edid_map.get(&prefix.to_ascii_uppercase()) {
                                // 変数インデックスが不明なため、とりあえず全て index 0 として扱う（CG00用フォールバック）
                                script_vars.insert((fid, 0), *v);
                            }
                        }
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
                        script_vars,
                    };

                    // 条件が true のものを選択
                    if fo3_script::evaluate_conditions(&pack.conditions, &cond_ctx, vm) {
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
                        // 目的地座標を取得してNavPathを算出
                        if let Some(loc) = &pack.location {
                            if let Some(target_fid) = loc.form_id {
                                if let (Some(&start_pos), Some(&end_pos)) = (
                                    actor_positions.get(&state.form_id),
                                    actor_positions.get(&target_fid),
                                ) {
                                    // 簡易的に最も近いNavMeshポリゴンを探索 (本来は空間分割・レイキャスト)
                                    let mut best_start = None;
                                    let mut best_start_dist = f32::MAX;
                                    let mut best_end = None;
                                    let mut best_end_dist = f32::MAX;

                                    for (&mesh_id, nodes) in &nav_graph.nodes {
                                        for node in nodes {
                                            let s_dist = node.center.distance(start_pos);
                                            if s_dist < best_start_dist {
                                                best_start_dist = s_dist;
                                                best_start = Some((mesh_id, node.triangle_idx));
                                            }
                                            let e_dist = node.center.distance(end_pos);
                                            if e_dist < best_end_dist {
                                                best_end_dist = e_dist;
                                                best_end = Some((mesh_id, node.triangle_idx));
                                            }
                                        }
                                    }

                                    if let (Some((sm, st)), Some((em, et))) = (best_start, best_end)
                                    {
                                        state.current_path = fo3_navigation::astar::find_path(
                                            nav_graph, sm, st, em, et,
                                        );
                                        state.path_target_index = 0;
                                        if let Some(path) = &state.current_path {
                                            println!("[AiManager] NPC 0x{:08X} の経路を算出しました (Waypoints: {})", state.form_id.0, path.points.len());
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(topic_id) = pack.topic_id {
                            // Topic FormID から EDID を逆引き
                            if let Some((dial, _)) = master
                                .topic_map
                                .values()
                                .find(|(d, _)| d.form_id == topic_id)
                            {
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
