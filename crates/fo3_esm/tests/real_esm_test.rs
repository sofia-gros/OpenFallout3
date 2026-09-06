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
