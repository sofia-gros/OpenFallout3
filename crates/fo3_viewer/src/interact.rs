//! # オブジェクト検知 & インタラクトモジュール
//!
//! プレイヤーの視線（クロスヘア）正面にある実オブジェクト（ドア、NPC、コンテナ、アイテム等）を
//! バウンディング球レイキャスト判定で検出する。
//!
//! 参照元:
//! - Gamebryo 2.6 `NiPick` / `NiBound::RayIntersect`
//! - `references/openmw/components/esm4/loadrefr.hpp`
//! - `knowledge/actor_and_skin_mesh.md`

use glam::Vec3;
use fo3_esm::records::refr::{LockData, TeleportDoor};

/// インタラクト対象の種別。
#[derive(Clone, Debug, PartialEq)]
pub enum InteractableKind {
    /// ドア（`DOOR` / `REFR`）。テレポート遷移情報または施錠データ、開閉状態を持つ。
    Door {
        teleport: Option<TeleportDoor>,
        lock: Option<LockData>,
        is_open: bool,
    },
    /// アクター（`ACHR` / `NPC_` / `CREA`）。
    Actor {
        form_id: u32,
        is_dead: bool,
    },
    /// コンテナ（`CONT`）。
    Container {
        form_id: u32,
        lock: Option<LockData>,
        is_open: bool,
    },
    /// 入手可能アイテム（`WEAP`, `ARMO`, `ALCH`, `BOOK`, `MISC`）。
    Item {
        form_id: u32,
    },
    /// アクティベーター（`ACTI`）。スイッチ等。
    Activator {
        form_id: u32,
    },
    /// ターミナル（`TERM`）。
    Terminal {
        form_id: u32,
        lock: Option<LockData>,
    },
}

/// インタラクト可能な 3D 配置オブジェクト。
#[derive(Clone, Debug)]
pub struct InteractableObject {
    /// 配置参照の FormID (`REFR` または `ACHR`)
    pub form_id: u32,
    /// エディタ ID (`EDID`)
    pub edid: String,
    /// ゲーム内表示名 (`FULL` サブレコード)
    pub name: String,
    /// ワールド空間の中心座標 (足元ピボット)
    pub position: Vec3,
    /// 垂直方向の高さ (アクター: ~125, ドア: ~140, コンテナ: ~60, アイテム: 0~20)
    pub height: f32,
    /// バウンディング球・円柱の半径
    pub radius: f32,
    /// オブジェクト種別
    pub kind: InteractableKind,
}

impl InteractableObject {
    /// 表示用のアクションテキストを取得する。
    /// 参照元: Fallout 3 実機 HUD 表示フォーマット
    pub fn prompt_text(&self) -> String {
        let label = if self.name.is_empty() {
            if self.edid.is_empty() {
                "オブジェクト".to_string()
            } else {
                self.edid.clone()
            }
        } else {
            self.name.clone()
        };

        match &self.kind {
            InteractableKind::Door { lock, teleport, is_open } => {
                if let Some(l) = lock {
                    let level_str = match l.lock_level {
                        0..=24 => "非常に簡単",
                        25..=49 => "簡単",
                        50..=74 => "普通",
                        75..=99 => "困難",
                        100..=254 => "非常に困難",
                        _ => "要キー",
                    };
                    format!("[E] 解錠 ({}) - {}", level_str, label)
                } else if teleport.is_some() {
                    format!("[E] 開く - {}", label)
                } else if *is_open {
                    format!("[E] 閉じる - {}", label)
                } else {
                    format!("[E] 開く - {}", label)
                }
            }
            InteractableKind::Actor { is_dead, .. } => {
                if *is_dead {
                    format!("[E] 探る - {}", label)
                } else {
                    format!("[E] 話す - {}", label)
                }
            }
            InteractableKind::Container { lock, is_open, .. } => {
                if let Some(l) = lock {
                    let level_str = match l.lock_level {
                        0..=24 => "非常に簡単",
                        25..=49 => "簡単",
                        50..=74 => "普通",
                        75..=99 => "困難",
                        100..=254 => "非常に困難",
                        _ => "要キー",
                    };
                    format!("[E] 解錠 ({}) - {}", level_str, label)
                } else if *is_open {
                    format!("[E] 閉じる - {}", label)
                } else {
                    format!("[E] 探る - {}", label)
                }
            }
            InteractableKind::Item { .. } => format!("[E] 取る - {}", label),
            InteractableKind::Activator { .. } => format!("[E] 作動 - {}", label),
            InteractableKind::Terminal { .. } => format!("[E] アクセス - {}", label),
        }
    }
}

/// Fallout 3 GMST: 実機デフォルトのアクティベート到達距離 (約 2.5m = 180 単位)
/// 参照元: Fallout 3 GMST `fActivatePickLength`
pub const F_ACTIVATE_PICK_LENGTH: f32 = 180.0;

/// 物理レイキャスト結果に基づき、クロスヘアが捉えている最前面のインタラクティブオブジェクトを検出する。
///
/// 参照元: Gamebryo 2.6 `NiPick::PickObjects`, Fallout 3 実機クロスヘア判定, OpenMW `World::getFocusObject`
///
/// 壁などの障害物が手前にある場合（`user_data == 0` または他オブジェクト）、
/// レイがそこで停止するため、遮蔽された背後のオブジェクトは正しく無視される。
pub fn find_focused_by_raycast<'a>(
    ray_hit: Option<fo3_physics::InteractionRayHit>,
    interactables: &'a [InteractableObject],
) -> Option<&'a InteractableObject> {
    let hit = ray_hit?;
    if hit.user_data == 0 {
        // 静的壁・天井・床などの遮蔽物にヒットしたため、ターゲットなし
        return None;
    }
    let target_form_id = hit.user_data as u32;
    interactables.iter().find(|obj| obj.form_id == target_form_id)
}

/// プレイヤーの視線レイとオブジェクト群のバウンディング球との交差判定を行い、
/// 視線中央（最前面）かつインタラクト最大距離内にあるオブジェクトを検出する（フォールバック用）。
///
/// 参照元: Gamebryo 2.6 `NiPick::PickObjects`, Fallout 3 実機アクティベーション判定
pub fn find_focused_interactable<'a>(
    camera_pos: Vec3,
    camera_dir: Vec3,
    interactables: &'a [InteractableObject],
    max_distance: f32,
) -> Option<&'a InteractableObject> {
    let mut best_candidate: Option<(&'a InteractableObject, f32)> = None;
    let dir = camera_dir.normalize_or_zero();
    if dir == Vec3::ZERO {
        return None;
    }

    for obj in interactables {
        let height = obj.height.max(0.0);
        let effective_radius = obj.radius.max(35.0);

        // 垂直線分 S(u) = P + u * (0, 0, 1) (0 <= u <= height) と Ray R(t) = C + t * dir (t > 0)
        let w0 = camera_pos - obj.position; // C - P
        let d_z = dir.z;
        let det = dir.x * dir.x + dir.y * dir.y; // 1.0 - d_z * d_z

        let u_clamped = if height <= 0.0 {
            0.0
        } else if det < 1e-6 {
            // 視線がほぼ真上または真下を向いている場合
            (camera_pos.z - obj.position.z).clamp(0.0, height)
        } else {
            let w0_dot_dir = w0.dot(dir);
            let w0_z = w0.z;
            // 2直線間の最短距離を与える u パラメータ: (w0_z - d_z * (w0 . dir)) / det
            let u = (w0_z - d_z * w0_dot_dir) / det;
            u.clamp(0.0, height)
        };

        // クランプされた垂直線分上の点 S_clamped
        let s_clamped = obj.position + Vec3::new(0.0, 0.0, u_clamped);
        let v = s_clamped - camera_pos;
        let t_proj = v.dot(dir);

        // カメラの後ろ、または最大到達距離を超える場合は除外
        if t_proj <= 0.0 || t_proj > max_distance {
            continue;
        }

        // Ray上の最近接点 R(t_proj) と S_clamped の幾何学的距離
        let closest_on_ray = camera_pos + dir * t_proj;
        let dist_sq = (closest_on_ray - s_clamped).length_squared();

        if dist_sq <= effective_radius * effective_radius {
            if let Some((_, best_t)) = best_candidate {
                if t_proj < best_t {
                    best_candidate = Some((obj, t_proj));
                }
            } else {
                best_candidate = Some((obj, t_proj));
            }
        }
    }

    best_candidate.map(|(obj, _)| obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_focused_interactable() {
        let cam_pos = Vec3::new(0.0, 0.0, 0.0);
        let cam_dir = Vec3::new(0.0, 1.0, 0.0);

        let door = InteractableObject {
            form_id: 0x0001,
            edid: "TestDoor".to_string(),
            name: "メガトン入口".to_string(),
            position: Vec3::new(0.0, 100.0, 0.0),
            height: 140.0,
            radius: 50.0,
            kind: InteractableKind::Door {
                teleport: Some(TeleportDoor {
                    dest_door: fo3_esm::FormId(0x0002),
                    dest_pos: [10.0, 20.0, 30.0],
                    dest_rot: [0.0, 0.0, 0.0],
                    flags: 0,
                }),
                lock: None,
                is_open: false,
            },
        };

        let far_door = InteractableObject {
            form_id: 0x0003,
            edid: "FarDoor".to_string(),
            name: "遠くのドア".to_string(),
            position: Vec3::new(0.0, 500.0, 0.0),
            height: 140.0,
            radius: 50.0,
            kind: InteractableKind::Door {
                teleport: None,
                lock: None,
                is_open: false,
            },
        };

        let side_item = InteractableObject {
            form_id: 0x0004,
            edid: "SideItem".to_string(),
            name: "横のボトルキャップ".to_string(),
            position: Vec3::new(100.0, 50.0, 0.0),
            height: 10.0,
            radius: 20.0,
            kind: InteractableKind::Item { form_id: 0x0004 },
        };

        let list = vec![door.clone(), far_door, side_item];

        // 1. 正面のドアがヒットする
        let hit = find_focused_interactable(cam_pos, cam_dir, &list, 200.0);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().name, "メガトン入口");
        assert_eq!(hit.unwrap().prompt_text(), "[E] 開く - メガトン入口");

        // 2. 距離制限（50.0ユニット未満）では遠すぎて当たらない
        let hit_short = find_focused_interactable(cam_pos, cam_dir, &list, 50.0);
        assert!(hit_short.is_none());

        // 3. 視線が反対方向の場合はヒットしない
        let hit_back = find_focused_interactable(cam_pos, -cam_dir, &list, 200.0);
        assert!(hit_back.is_none());

        // 4. アクターの顔・胸の高さを見上げた／水平に見下ろした時の判定テスト
        // 足元が Z = -130 にある NPC に対して、プレイヤーの目線 Z = 0 から水平に見ている場合
        let npc = InteractableObject {
            form_id: 0x0005,
            edid: "ColinMoriarty".to_string(),
            name: "コリン・モリアティ".to_string(),
            position: Vec3::new(0.0, 100.0, -130.0), // 足元
            height: 125.0,                         // 頭頂 Z = -5.0 付近
            radius: 40.0,
            kind: InteractableKind::Actor {
                form_id: 0x0005,
                is_dead: false,
            },
        };
        let npc_list = vec![npc];
        let hit_npc = find_focused_interactable(cam_pos, cam_dir, &npc_list, 200.0);
        assert!(hit_npc.is_some(), "アイレベルから水平に見ている NPC がヒットすること");
        assert_eq!(hit_npc.unwrap().name, "コリン・モリアティ");
    }

    /// 物理レイキャストによるターゲット特定および遮蔽の検証テスト。
    #[test]
    fn test_find_focused_by_raycast() {
        let door = InteractableObject {
            form_id: 0x0001572F,
            edid: "ShackDoor".to_string(),
            name: "モリアティ酒場のドア".to_string(),
            position: Vec3::new(100.0, 0.0, 0.0),
            height: 140.0,
            radius: 50.0,
            kind: InteractableKind::Door {
                teleport: None,
                lock: None,
                is_open: false,
            },
        };
        let list = vec![door];

        // 1. レイがドアコライダーにヒット (user_data == 0x0001572F)
        let hit_door = Some(fo3_physics::InteractionRayHit {
            point: Vec3::new(100.0, 0.0, 0.0),
            normal: -Vec3::X,
            distance: 100.0,
            user_data: 0x0001572F,
        });
        let res = find_focused_by_raycast(hit_door, &list);
        assert!(res.is_some());
        assert_eq!(res.unwrap().name, "モリアティ酒場のドア");

        // 2. 手前の壁にヒット (user_data == 0)
        let hit_wall = Some(fo3_physics::InteractionRayHit {
            point: Vec3::new(50.0, 0.0, 0.0),
            normal: -Vec3::X,
            distance: 50.0,
            user_data: 0,
        });
        let res_wall = find_focused_by_raycast(hit_wall, &list);
        assert!(res_wall.is_none(), "手前の壁で遮蔽された場合は None になること");

        // 3. 何もヒットしなかった場合 (None)
        assert!(find_focused_by_raycast(None, &list).is_none());
    }
}
