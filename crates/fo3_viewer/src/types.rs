//! ビューアーの共有データ構造およびユーティリティ関数。
//!
//! 参照元: Gamebryo 2.6 キャラクタパーツ合成, `references/openmw/components/esm4/loadlvli.cpp`

use std::collections::HashMap;
use winit::window::Window;

/// カメラの動作モード。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraMode {
    /// Fallout 3 実機標準プレイヤーカメラ (1人称 / 3人称 / 物理KCC / しゃがみ等)
    Standard,
    /// デバッグ用フリーオービットカメラ (F12 で切替、全体俯瞰・回転周回)
    FreeOrbit,
}

/// ビューアーの表示対象。
#[derive(Clone, Debug)]
pub enum ViewerTarget {
    Mesh(String),
    Cell(String),
    World(String, Option<(i32, i32)>),
    /// 単一スキンメッシュ + KF アニメーション再生モード
    Anim {
        nif_path: String,
        kf_path: String,
    },
    /// 人型アクター（全身パーツ合成: 頭部、胴体/衣装、右手、左手）+ KF アニメーション再生モード
    Actor {
        outfit_or_naked: String,
        kf_path: String,
    },
    /// 実機完全準拠のニューゲームモード (CG00: Vault 101 Infirmary 〜 CG04)
    NewGame {
        intro_movie: Option<String>,
        start_quest: u32,
        start_stage: u32,
    },
}

/// ウィンドウタイトルを設定する。
pub fn update_window_title(window: &Window, target: &ViewerTarget) {
    let title = match target {
        ViewerTarget::Mesh(path) => format!("OpenFallout3 - Mesh: {}", path),
        ViewerTarget::Cell(edid) => format!("OpenFallout3 - Cell: {}", edid),
        ViewerTarget::World(edid, _) => format!("OpenFallout3 - World: {}", edid),
        ViewerTarget::Anim { nif_path, kf_path } => {
            format!("OpenFallout3 - Anim: {} + {}", nif_path, kf_path)
        }
        ViewerTarget::Actor {
            outfit_or_naked,
            kf_path,
        } => {
            format!("OpenFallout3 - Actor: {} + {}", outfit_or_naked, kf_path)
        }
        ViewerTarget::NewGame { .. } => "OpenFallout3 - New Game (CG00: Vault 101)".to_string(),
    };
    window.set_title(&title);
}

/// 操作ガイドを標準出力に表示する。
pub fn print_controls_guide() {
    let guide = crate::input::InputManager::new().get_guide_text();
    println!("{}", guide);
}

/// エディタ用配置マーカー（矢印、ボックス）や光線エフェクトメッシュ（真っ白な板になる）かどうかを判定。
pub fn is_editor_marker_or_effect(edid: &str, model: &str) -> bool {
    let lower_model = model.to_ascii_lowercase();
    let lower_edid = edid.to_ascii_lowercase();

    // エディタ専用マーカー (矢印 MarkerXHeading、不可視ドアマーカー等)
    if lower_model.contains("marker") || lower_edid.contains("marker") {
        return true;
    }
    // 環境光線・グローエフェクト (不透明ジオメトリ描画では真っ白な板として現れてしまうもの)
    if lower_model.contains("lightbeam")
        || lower_model.contains("glow")
        || lower_model.contains("ray")
        || lower_model.starts_with("effects\\")
        || lower_model.starts_with("effects/")
    {
        return true;
    }

    false
}

/// グール種族 (GhoulRace / GhoulGlowingOneRace) であるかを判定する。
/// 参照元: `references/openmw/components/esm4/loadrace.hpp`, Fallout 3 `Race` 定義
pub fn is_ghoul_race(race: fo3_esm::FormId) -> bool {
    race.0 == 0x00003B3E || race.0 == 0x000638EF
}

/// 人型アクターのパーツ NIF 相対パス一覧を取得する。
///
/// 頭部 (人間/グール別)、目 (左右)、歯 (上下)、舌、頭部装備 (帽子/ヘルメット等)、髪型 (指定時)、胴体/衣装、手 (手袋または男女別素手)、武器を過不足なく構成する。
/// 参照元: Gamebryo 2.6 キャラクタパーツ合成, `knowledge/actor_and_skin_mesh.md` (セクション 4.6, 4.7, 4.13)
pub fn get_actor_part_paths(
    is_female: bool,
    race: fo3_esm::FormId,
    body_path: &str,
    head_gear_path: Option<&str>,
    hand_gear_path: Option<&str>,
    weapon_path: Option<&str>,
    hair_path: Option<&str>,
    hide_hair: bool,
) -> Vec<String> {
    let (default_right_hand, default_left_hand) = if is_female {
        (
            "meshes\\characters\\_male\\femalerighthand.nif",
            "meshes\\characters\\_male\\femalelefthand.nif",
        )
    } else {
        (
            "meshes\\characters\\_male\\righthand.nif",
            "meshes\\characters\\_male\\lefthand.nif",
        )
    };
    let head_nif = if is_ghoul_race(race) {
        "meshes\\characters\\head\\headghoul.nif"
    } else {
        "meshes\\characters\\head\\headhuman.nif"
    };

    let mut parts = vec![
        head_nif.to_string(),
        "meshes\\characters\\head\\eyelright.nif".to_string(),
        "meshes\\characters\\head\\eyelleft.nif".to_string(),
        "meshes\\characters\\head\\teethlower.nif".to_string(),
        "meshes\\characters\\head\\teethupper.nif".to_string(),
        "meshes\\characters\\head\\tongue.nif".to_string(),
        body_path.to_string(),
    ];

    if let Some(hg) = head_gear_path {
        parts.push(hg.to_string());
    }

    if let Some(hg) = hand_gear_path {
        parts.push(hg.to_string());
    } else {
        parts.push(default_right_hand.to_string());
        parts.push(default_left_hand.to_string());
    }

    if let Some(wp) = weapon_path {
        parts.push(wp.to_string());
    }

    if !hide_hair {
        if let Some(hair) = hair_path {
            parts.push(hair.to_string());
        }
    }

    parts
}

/// レベルドアイテム (LVLI) を再帰的に展開し、含まれる具象アイテム FormID リストを収集する。
///
/// 循環参照を防ぐため訪問済みセット (visited) を使用し、キューによる幅優先探索 (BFS) で
/// ネストされたすべてのレベルドリストを末端の具象アイテム（防具、武器、弾薬等）まで完全に展開する。
/// 参照元: `references/openmw/components/esm4/loadlvli.cpp:57`, `inventory.hpp:38`
pub fn resolve_candidate_items(
    initial_items: &[fo3_esm::FormId],
    lvli_map: &HashMap<fo3_esm::FormId, fo3_esm::LvliRecord>,
) -> Vec<fo3_esm::FormId> {
    let mut result = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut queue: std::collections::VecDeque<fo3_esm::FormId> =
        initial_items.iter().copied().collect();

    while let Some(item_id) = queue.pop_front() {
        if !visited.insert(item_id) {
            continue;
        }
        if let Some(lvli) = lvli_map.get(&item_id) {
            for entry in &lvli.entries {
                queue.push_back(entry.item);
            }
        } else {
            result.push(item_id);
        }
    }
    result
}
