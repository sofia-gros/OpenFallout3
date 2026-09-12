//! # 実 ESM (Fallout3.esm) 検証統合テスト
//!
//! Vault 101 セルおよび配置オブジェクト群の完全パース検証。
//! 参照元: `references/openmw/components/esm4/loadcell.hpp`, `loadrefr.hpp`

use std::path::Path;
use fo3_esm::{EsmReader, REC_ACHR, REC_REFR};

#[test]
fn test_real_vault101_refr_and_doors() {
    let esm_path = Path::new(r"A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data\Fallout3.esm");
    if !esm_path.exists() {
        eprintln!("Fallout3.esm が見つからないためスキップします: {:?}", esm_path);
        return;
    }

    let mut reader = EsmReader::open(esm_path).expect("Failed to open Fallout3.esm");

    // Vault101a セルを検索
    let (cell, refrs, land) = reader
        .find_cell_by_edid("Vault101a")
        .expect("Failed to search cell")
        .expect("Vault101a が見つかりませんでした");

    assert_eq!(cell.edid, "Vault101a");
    assert!(cell.is_interior());
    assert!(land.is_none());
    assert!(!refrs.is_empty(), "Vault101a の配置参照が 0 件です");

    println!("Vault101a 配置オブジェクト総数: {} 件", refrs.len());

    let mut teleport_doors = Vec::new();
    let mut locked_objects = Vec::new();
    let mut actors = Vec::new();
    let mut standard_refr_count = 0;

    for refr in &refrs {
        if refr.record_type == REC_REFR {
            standard_refr_count += 1;
        }
        if let Some(teleport) = &refr.teleport {
            teleport_doors.push((refr, teleport));
        }
        if let Some(lock) = &refr.lock {
            locked_objects.push((refr, lock));
        }
        if refr.record_type == REC_ACHR {
            actors.push(refr);
        }
    }
    assert!(standard_refr_count > 0, "通常 REFR レコードが存在しません");

    println!("テレポートドア発見件数: {} 件", teleport_doors.len());
    for (refr, tele) in &teleport_doors {
        println!(
            "  - ドア FormID 0x{:08X} (EDID: {:?}) -> 行き先ドア 0x{:08X}, 出現位置 {:?}",
            refr.form_id.0, refr.edid, tele.dest_door.0, tele.dest_pos
        );
    }

    println!("施錠オブジェクト発見件数: {} 件", locked_objects.len());
    for (refr, lock) in &locked_objects {
        println!(
            "  - 施錠 FormID 0x{:08X} (EDID: {:?}): レベル {}, キー {:?}",
            refr.form_id.0, refr.edid, lock.lock_level, lock.key
        );
    }

    println!("初期配置アクター (ACHR) 発見件数: {} 件", actors.len());
    for actor in &actors {
        println!(
            "  - アクター FormID 0x{:08X} (EDID: {:?}, Base: 0x{:08X})",
            actor.form_id.0, actor.edid, actor.base_object.0
        );
    }

    // Vault101a には隣接エリアへのテレポートドアが存在すること
    assert!(!teleport_doors.is_empty(), "テレポートドアが 1 件も見つかりませんでした");

    // テレポート先の dest_door FormID が有効であること
    for (_, tele) in &teleport_doors {
        assert_ne!(tele.dest_door.0, 0, "テレポート先のドア FormID が 0 です");
    }
}

#[test]
fn test_real_megaton_saloon_npcs() {
    let esm_path = Path::new(r"A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data\Fallout3.esm");
    if !esm_path.exists() {
        return;
    }

    let mut reader = EsmReader::open(esm_path).expect("Failed to open Fallout3.esm");
    let (npc_map, armor_map, outfit_map, hair_map, lvli_map) = reader
        .read_npc_and_armor_map()
        .expect("Failed to read maps");

    println!("読み込み完了: NPC {} 件, Armor {} 件, Outfit {} 件, Hair {} 件, LVLI {} 件",
        npc_map.len(), armor_map.len(), outfit_map.len(), hair_map.len(), lvli_map.len()
    );

    assert!(!lvli_map.is_empty(), "LVLI レコードが1件以上パースされていること");

    let (_cell, refrs, _) = reader
        .find_cell_by_edid("MegatonMoriartysSaloon")
        .expect("Failed to search cell")
        .expect("MegatonMoriartysSaloon が見つかりませんでした");

    println!("MegatonMoriartysSaloon 内 REFR 数: {}", refrs.len());
    for refr in &refrs {
        if let Some(npc) = npc_map.get(&refr.base_object) {
            println!("--- NPC発見: EDID: {}, Name: {:?}, Female: {}, Pos: {:?}", npc.edid, npc.full_name, npc.is_female, refr.position);
            println!("    Race: 0x{:08X}, WNAM: {:?}, DOFT: {:?}, HNAM: {:?}", npc.race.0, npc.default_armor, npc.default_outfit, npc.hair);
            if let Some(doft_id) = npc.default_outfit {
                if let Some(otft) = outfit_map.get(&doft_id) {
                    println!("    DOFT Outfit: EDID: {}, Inventory: {:?}", otft.edid, otft.inventory);
                    for &arm_id in &otft.inventory {
                        if let Some(arm) = armor_map.get(&arm_id) {
                            println!("      Armor: EDID: {}, Male: '{}', Female: '{}'", arm.edid, arm.male_model, arm.female_model);
                        } else {
                            println!("      Armor FormID 0x{:08X} が armor_map に未登録！", arm_id.0);
                        }
                    }
                } else {
                    println!("    DOFT FormID 0x{:08X} が outfit_map に未登録！", doft_id.0);
                }
            }
            if let Some(wnam_id) = npc.default_armor {
                if let Some(arm) = armor_map.get(&wnam_id) {
                    println!("    WNAM Armor: EDID: {}, Male: '{}', Female: '{}'", arm.edid, arm.male_model, arm.female_model);
                } else {
                    println!("    WNAM FormID 0x{:08X} が armor_map に未登録！", wnam_id.0);
                }
            }
            if let Some(hair_id) = npc.hair {
                if let Some(hair) = hair_map.get(&hair_id) {
                    println!("    Hair: EDID: {}, Model: '{}'", hair.edid, hair.model);
                } else {
                    println!("    Hair FormID 0x{:08X} が hair_map に未登録！", hair_id.0);
                }
            }
            println!("    所持品数 (CNTO): {}", npc.inventory.len());
            for item in &npc.inventory {
                if let Some(arm) = armor_map.get(&item.item) {
                    println!("      ★防具所持: FormID 0x{:08X}, EDID: {}, Male: '{}', Female: '{}'", item.item.0, arm.edid, arm.male_model, arm.female_model);
                } else {
                    println!("      アイテム FormID 0x{:08X} (count: {})", item.item.0, item.count);
                }
            }
        }
    }
}

/// レベルドアイテム (LVLI) の再帰展開により、Vault101 警備員や Lucas Simms の
/// ヘルメット・帽子・服・武器が欠損なく解決されることを検証する。
/// 参照元: `references/openmw/components/esm4/loadlvli.cpp:57`, `inventory.hpp:38`
#[test]
fn test_real_lvli_resolution() {
    let esm_path = "A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\\Fallout3.esm";
    if !std::path::Path::new(esm_path).exists() {
        eprintln!("Fallout3.esm が見つからないためテストをスキップします");
        return;
    }

    let mut reader = EsmReader::open(esm_path).expect("Failed to open Fallout3.esm");
    let model_map = reader.read_all_models_map().expect("Failed to read models");
    let (npc_map, armor_map, _outfit_map, _hair_map, lvli_map) = reader
        .read_npc_and_armor_map()
        .expect("Failed to read maps");

    // BFS 再帰展開ヘルパー
    fn resolve(items: &[fo3_esm::FormId], lvlis: &std::collections::HashMap<fo3_esm::FormId, fo3_esm::LvliRecord>) -> Vec<fo3_esm::FormId> {
        let mut res = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut q: std::collections::VecDeque<fo3_esm::FormId> = items.iter().copied().collect();
        while let Some(id) = q.pop_front() {
            if !visited.insert(id) { continue; }
            if let Some(lvli) = lvlis.get(&id) {
                for e in &lvli.entries {
                    q.push_back(e.item);
                }
            } else {
                res.push(id);
            }
        }
        res
    }

    // 1. Vault 101 警備員 (Officer Wolfe: 0x00035DA9)
    let wolfe = npc_map.get(&fo3_esm::FormId(0x00035DA9)).expect("Officer Wolfe が存在すること");
    let wolfe_raw: Vec<fo3_esm::FormId> = wolfe.inventory.iter().map(|i| i.item).collect();
    let wolfe_resolved = resolve(&wolfe_raw, &lvli_map);

    let has_helmet = wolfe_resolved.iter().any(|id| {
        armor_map.get(id).map(|a| a.is_head()).unwrap_or(false)
    });
    let has_armor = wolfe_resolved.iter().any(|id| {
        armor_map.get(id).map(|a| a.is_upper_body()).unwrap_or(false)
    });
    let has_weapon = wolfe_resolved.iter().any(|id| {
        model_map.get(id).map(|b| b.record_type == fo3_esm::types::REC_WEAP).unwrap_or(false)
    });

    assert!(has_helmet, "Officer Wolfe が警備ヘルメット (頭部防具) を解決できること");
    assert!(has_armor, "Officer Wolfe が警備服 (胴体防具) を解決できること");
    assert!(has_weapon, "Officer Wolfe が警棒等の武器を解決できること");

    // 2. Lucas Simms (0x00000A60)
    let simms = npc_map.get(&fo3_esm::FormId(0x00000A60)).expect("Lucas Simms が存在すること");
    let simms_raw: Vec<fo3_esm::FormId> = simms.inventory.iter().map(|i| i.item).collect();
    let simms_resolved = resolve(&simms_raw, &lvli_map);

    let simms_has_hat = simms_resolved.iter().any(|id| {
        armor_map.get(id).map(|a| a.is_head() && a.shows_hat()).unwrap_or(false)
    });
    let simms_has_duster = simms_resolved.iter().any(|id| {
        armor_map.get(id).map(|a| a.is_upper_body()).unwrap_or(false)
    });
    let simms_has_rifle = simms_resolved.iter().any(|id| {
        model_map.get(id).map(|b| b.record_type == fo3_esm::types::REC_WEAP).unwrap_or(false)
    });

    assert!(simms_has_hat, "Lucas Simms が保安官帽子を解決できること");
    assert!(simms_has_duster, "Lucas Simms がダスターコートを解決できること");
    assert!(simms_has_rifle, "Lucas Simms が中国軍アサルトライフル (武器) を解決できること");
}


