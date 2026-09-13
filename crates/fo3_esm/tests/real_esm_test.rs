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
            println!("    Race: 0x{:08X}, WNAM: {:?}, DOFT: {:?}, HNAM: {:?}, HairColor: {:?}", npc.race.0, npc.default_armor, npc.default_outfit, npc.hair, npc.hair_color);
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

#[test]
fn test_real_qust_inspection() {
    let esm_path = "A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\\Fallout3.esm";
    if !std::path::Path::new(esm_path).exists() {
        eprintln!("Fallout3.esm が見つからないためテストをスキップします");
        return;
    }

    let mut reader = EsmReader::open(esm_path).expect("Failed to open Fallout3.esm");
    let mut qust_count = 0;
    while let Some(entry) = reader.read_next_entry().expect("read entry") {
        if let fo3_esm::reader::EsmEntry::Record(record, subrecords) = entry {
            if record.type_id == fo3_esm::types::REC_QUST {
                qust_count += 1;
                let mut edid = String::new();
                let mut full = String::new();
                let mut sub_tags = Vec::new();
                for sub in &subrecords {
                    sub_tags.push(std::str::from_utf8(&sub.type_id.0).unwrap_or("????").to_string());
                    if sub.type_id == fo3_esm::types::SUB_EDID {
                        edid = String::from_utf8_lossy(&sub.data).trim_end_matches('\0').to_string();
                    } else if sub.type_id == fo3_esm::types::SUB_FULL {
                        full = String::from_utf8_lossy(&sub.data).trim_end_matches('\0').to_string();
                    }
                }
                if qust_count <= 5 || edid == "MQ01" || edid == "MS11" {
                    println!(
                        "QUST [0x{:08X}] EDID: \"{}\", FULL: \"{}\", Subs: {:?}",
                        record.form_id.0, edid, full, sub_tags
                    );
                    for sub in &subrecords {
                        let stag = std::str::from_utf8(&sub.type_id.0).unwrap_or("????");
                        if matches!(stag, "INDX" | "QSDT" | "DATA" | "SCRI" | "SCHR" | "QOBJ" | "QSTA" | "NNAM") {
                            println!("   Sub {}: len={}, hex={:02X?}", stag, sub.data.len(), &sub.data[..sub.data.len().min(16)]);
                        }
                    }
                }
            }
        }
    }
    println!("Total QUST records found: {}", qust_count);
    assert!(qust_count > 0, "QUST レコードが ESM から取得できること");
}

#[test]
fn test_real_pack_inspection() {
    let esm_path = "A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\\Fallout3.esm";
    if !std::path::Path::new(esm_path).exists() {
        eprintln!("Fallout3.esm が見つからないためテストをスキップします");
        return;
    }

    let mut reader = EsmReader::open(esm_path).expect("Failed to open Fallout3.esm");
    let pack_map = reader.read_all_packages_map().expect("read_all_packages_map");
    println!("Total PACK records found: {}", pack_map.len());
    assert!(pack_map.len() > 100, "PACK レコードが多数取得できること (実機は1000件以上)");

    let mut cg00_packs = 0;
    for (form_id, pack) in &pack_map {
        if let Some(edid) = &pack.editor_id {
            if edid.starts_with("CG00") {
                cg00_packs += 1;
                println!(
                    "CG00 PACK [0x{:08X}] EDID: \"{}\", Type: {:?}, Flags: 0x{:08X}",
                    form_id.0, edid, pack.pack_type, pack.flags
                );
            }
        }
    }
    println!("Found {} CG00 PACK records", cg00_packs);
    assert!(cg00_packs > 0, "CG00 関連の AI パッケージが存在すること");
}

#[test]
fn test_real_cg00_cell_and_markers() {
    let esm_path = "A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\\Fallout3.esm";
    if !std::path::Path::new(esm_path).exists() {
        return;
    }
    let mut reader = EsmReader::open(esm_path).expect("Failed to open Fallout3.esm");
    // 全 REFR から EDID が CG00 で始まるものを探索
    let mut cg00_refrs = Vec::new();
    while let Some(entry) = reader.read_next_entry().expect("read entry") {
        if let fo3_esm::reader::EsmEntry::Record(record, subs) = entry {
            if record.type_id == fo3_esm::types::REC_REFR || record.type_id == fo3_esm::types::REC_ACHR {
                for sub in &subs {
                    if sub.type_id == fo3_esm::types::SUB_EDID {
                        let edid = sub.as_string();
                        if edid.starts_with("CG00") {
                            cg00_refrs.push((record.form_id, edid, record.type_id));
                        }
                    }
                }
            }
        }
    }
    println!("Found {} CG00 REFR/ACHR records:", cg00_refrs.len());
    for (fid, edid, rec_type) in &cg00_refrs {
        let tag = std::str::from_utf8(&rec_type.0).unwrap_or("????");
        println!("  [{}] 0x{:08X}: \"{}\"", tag, fid.0, edid);
    }

    let marker_id = fo3_esm::types::FormId(0x00039562);
    // cell_map または reader で marker_id の所属セルを逆引き
    // EsmReader::find_cell_for_refr または CELL レコード走査
    let marker_ids: std::collections::HashSet<fo3_esm::types::FormId> = cg00_refrs.iter().map(|(id, _, _)| *id).collect();
    let mut reader2 = EsmReader::open(esm_path).expect("reopen");
    while let Some(entry) = reader2.read_next_entry().expect("read") {
        if let fo3_esm::reader::EsmEntry::Record(record, subs) = entry {
            if marker_ids.contains(&record.form_id) {
                let edid = cg00_refrs.iter().find(|(id, _, _)| *id == record.form_id).map(|(_, e, _)| e.as_str()).unwrap_or("");
                if let Ok(refr) = fo3_esm::records::refr::RefrRecord::from_record(&record, &subs) {
                    println!("★ CG00 REFR \"{}\" (0x{:08X}): Pos={:?}, Rot={:?}", edid, record.form_id.0, refr.position, refr.rotation);
                }
                println!("  Subs for \"{}\" (0x{:08X}):", edid, record.form_id.0);
                for s in &subs {
                    let tag = std::str::from_utf8(&s.type_id.0).unwrap_or("????");
                    if s.data.len() == 4 {
                        let fid = u32::from_le_bytes(s.data.as_slice().try_into().unwrap());
                        println!("    [{}] len=4, FormId=0x{:08X}", tag, fid);
                    } else if s.data.len() < 30 {
                        println!("    [{}] len={}, str=\"{}\"", tag, s.data.len(), s.as_string());
                    } else {
                        println!("    [{}] len={}", tag, s.data.len());
                    }
                }
            }
        }
    }
}



