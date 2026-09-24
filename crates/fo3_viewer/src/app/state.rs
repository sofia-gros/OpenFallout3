//! ビューアーの状態管理・セルスクリプト登録・UIバインド (`ViewerState`)。
//!
//! 参照元: Gamebryo 2.6 セル遷移 & `references/openmw/components/esm4/loadrefr.cpp`

use fo3_esm::FormId;
use fo3_render::RenderContext;
use std::path::Path;
use winit::dpi::PhysicalSize;

use crate::ui::ViewerMode;
use super::ViewerState;

impl ViewerState {
    /// セル内の配置オブジェクトにアタッチされたスクリプトを抽出し、イベントディスパッチャーへ登録する。
    /// `cell_edid`: 現在ロード中のセルの EditorID (例: "Vault101Infirmary", "MegatonSaloon")
    /// 参照元: `AGENTS.md Rule 1` — ハードコード禁止 / `knowledge/new_game_and_quest_engine_architecture.md`
    pub fn setup_scripts_for_cell(&mut self, cell_edid: &str) {
        // 0. クエスト EditorID -> FormID マップを VM へ登録
        for (edid, form_id) in &self.master_context.quest_edid_map {
            self.vm.edid_map.insert(edid.clone(), *form_id);
        }

        // メッセージ (MESG) と サウンド (SOUN) のマップを登録
        for (edid, form_id) in &self.master_context.mesg_edid_map {
            self.vm.edid_map.insert(edid.clone(), *form_id);
        }
        for (edid, form_id) in &self.master_context.soun_edid_map {
            self.vm.edid_map.insert(edid.clone(), *form_id);
        }

        // パッケージ (PACK) とアイドル (IDLE) の EDID も VM で解決できるようにする
        for (form_id, pack) in &self.master_context.pack_map {
            if let Some(edid) = &pack.editor_id {
                self.vm.edid_map.insert(edid.to_ascii_uppercase(), *form_id);
            }
        }
        for (form_id, idle) in &self.master_context.idle_map {
            if let Some(edid) = &idle.editor_id {
                self.vm.edid_map.insert(edid.to_ascii_uppercase(), *form_id);
            }
        }
        // 0.1 セル内の配置参照 (REFR / ACHR) の EditorID およびスクリプトを登録
        // 参照元: `references/openmw/components/esm4/loadrefr.cpp` — REFR スクリプトアタッチ
        let esm_path = Path::new(&self.data_dir).join("Fallout3.esm");
        if let Ok(mut reader) = fo3_esm::EsmReader::open(&esm_path) {
            if let Ok(Some(cells)) = reader.find_cell_and_neighbors(cell_edid, 0) {
                for (_, refrs, _) in cells {
                    for refr in refrs {
                        if !refr.edid.is_empty() {
                            self.vm
                                .edid_map
                                .insert(refr.edid.to_ascii_uppercase(), refr.form_id);
                            self.vm
                                .edid_map
                                .insert(refr.edid.clone(), refr.form_id);
                        }
                        if let Some(scpt_id) = refr.script {
                            self.dispatcher.attach_script(refr.form_id, scpt_id);
                            self.vm.attached_scripts.insert(refr.form_id, scpt_id);
                        }
                    }
                }
            }
        }

        // 1. master_context に存在する全スクリプトを dispatcher および VM に登録
        for scpt in self.master_context.script_map.values() {
            self.dispatcher.register_script(scpt.clone());
            self.vm.scripts.insert(scpt.form_id, scpt.clone());
        }

        // 2. セル内の配置オブジェクト (interactables / actors) のスクリプトをアタッチ
        for obj in &self.interactables {
            let obj_id = FormId(obj.form_id);
            let base_id = FormId(obj.base_id);

            if let Some(base_info) = self.master_context.model_map.get(&base_id) {
                if let Some(script_id) = base_info.script {
                    self.dispatcher.attach_script(obj_id, script_id);
                    self.vm.attached_scripts.insert(obj_id, script_id);
                }
            } else if let Some(npc) = self.master_context.npc_map.get(&base_id) {
                if let Some(script_id) = npc.script {
                    self.dispatcher.attach_script(obj_id, script_id);
                    self.vm.attached_scripts.insert(obj_id, script_id);
                }
            }
        }

        // シーン内のアクターに対してもベーススクリプトをアタッチ
        for actor in &self.scene.actors {
            let obj_id = fo3_esm::FormId(actor.form_id);
            let base_id = fo3_esm::FormId(actor.base_id);

            if let Some(npc) = self.master_context.npc_map.get(&base_id) {
                if let Some(script_id) = npc.script {
                    self.dispatcher.attach_script(obj_id, script_id);
                    self.vm.attached_scripts.insert(obj_id, script_id);
                }
            }
        }
    }

    /// フォーカス中オブジェクトに対するインタラクトアクション (`E` キー) を実行。
    /// ドア (`XTEL`) の場合は遷移先セルを解決してロードしテレポートを行う。
    /// 参照元: Gamebryo 2.6 セル遷移 & `references/openmw/components/esm4/loadrefr.cpp:103-127`
    pub fn interact_or_teleport(&mut self) {
        crate::action::perform_interact(self);
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_view =
                RenderContext::create_depth_texture(&self.device, new_size.width, new_size.height);
            self.controller.camera.aspect = new_size.width as f32 / new_size.height as f32;
        }
    }

    pub fn bind_ui_to_refr(&mut self, form_id: u32) {
        self.unbind_ui();
        if let Some(binding) = self.refr_bindings.get(&form_id) {
            for &mesh_idx in &binding.static_mesh_indices {
                if let Some(mesh) = self.scene.meshes.get_mut(mesh_idx) {
                    let name = mesh.name.to_lowercase();
                    if name.contains("screen")
                        || name.contains("projector")
                        || name.contains("terminal")
                        || name.contains("monitor")
                    {
                        println!(
                            "RTT をメッシュ {} (Index: {}) にバインドします",
                            mesh.name, mesh_idx
                        );

                        let rtt_bind_group =
                            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                label: Some("RTT Texture Bind Group"),
                                layout: &self.context.texture_bind_group_layout,
                                entries: &[
                                    wgpu::BindGroupEntry {
                                        binding: 0,
                                        resource: wgpu::BindingResource::TextureView(
                                            &self.ui_rtt.view,
                                        ),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 1,
                                        resource: wgpu::BindingResource::Sampler(
                                            &self.ui_rtt.sampler,
                                        ),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 2,
                                        resource: wgpu::BindingResource::TextureView(
                                            &self.ui_rtt.view,
                                        ),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 3,
                                        resource: wgpu::BindingResource::Sampler(
                                            &self.ui_rtt.sampler,
                                        ),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 4,
                                        resource: wgpu::BindingResource::TextureView(
                                            &self.ui_rtt.view,
                                        ),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 5,
                                        resource: wgpu::BindingResource::Sampler(
                                            &self.ui_rtt.sampler,
                                        ),
                                    },
                                ],
                            });

                        let old_bind_group =
                            std::mem::replace(&mut mesh.texture_bind_group, rtt_bind_group);
                        self.active_rtt_bindings.push((mesh_idx, old_bind_group));
                    }
                }
            }
        }
    }

    pub fn unbind_ui(&mut self) {
        for (mesh_idx, old_bind_group) in self.active_rtt_bindings.drain(..) {
            if let Some(mesh) = self.scene.meshes.get_mut(mesh_idx) {
                mesh.texture_bind_group = old_bind_group;
                println!(
                    "メッシュ (Index: {}) の元のテクスチャを復元しました",
                    mesh_idx
                );
            }
        }
    }

    pub fn set_viewer_mode(&mut self, mode: ViewerMode, form_id: Option<u32>) {
        self.mode = mode;
        if self.mode.is_ui_active() {
            if let Some(fid) = form_id {
                self.bind_ui_to_refr(fid);
            }
        } else {
            self.unbind_ui();
        }
    }
}
