//! ESM からのターゲットセル・近傍セル・ワールドスペースのセル探索モジュール。
//!
//! 参照元: Gamebryo 2.6 セルグラフ走査, `references/openmw/components/esm4/loadcell.hpp`

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use fo3_esm::{CellRecord, EsmReader, LandRecord, RefrRecord};

use crate::types::ViewerTarget;

/// ターゲット指定に基づき ESM から対象セル群および近傍セル・LAND を探索・抽出する。
pub fn find_cells_for_target(
    data_dir: &str,
    target: &ViewerTarget,
) -> (EsmReader<BufReader<File>>, Vec<(CellRecord, Vec<RefrRecord>, Option<LandRecord>)>) {
    let esm_path = Path::new(data_dir).join("Fallout3.esm");
    let mut esm_reader = EsmReader::open(&esm_path).expect("Failed to open Fallout3.esm");

    let cells: Vec<(CellRecord, Vec<RefrRecord>, Option<LandRecord>)> = match target {
        ViewerTarget::NewGame { .. } => {
            println!("ESM からニューゲーム初期セル \"Vault101d\" を検索中...");
            esm_reader
                .find_cell_and_neighbors("Vault101d", 0)
                .expect("find Vault101d")
                .unwrap_or_default()
        }
        ViewerTarget::Cell(cell_edid) => {
            println!("ESM からセル \"{}\" および近傍セルを検索中...", cell_edid);
            esm_reader
                .find_cell_and_neighbors(cell_edid, 1)
                .expect("Failed to find cell and neighbors")
                .unwrap_or_else(|| panic!("セル \"{}\" が見つかりませんでした", cell_edid))
        }
        ViewerTarget::World(world_edid, grid_opt) => {
            println!("ESM からワールド \"{}\" を検索中...", world_edid);
            let (world_rec, group_start, group_end) = esm_reader
                .find_world_by_edid(world_edid)
                .expect("Failed to search world")
                .unwrap_or_else(|| {
                    panic!("ワールドスペース \"{}\" が見つかりませんでした", world_edid)
                });
            println!(
                "ワールドスペース発見: \"{}\" (FormID: 0x{:08X}, 表示名: {:?})",
                world_rec.edid, world_rec.form_id.0, world_rec.full_name
            );
            let (resolved_center, cells) = esm_reader
                .read_cells_in_world_region(group_start, group_end, *grid_opt, 1)
                .expect("Failed to read world cells");
            println!(
                "走査結果: 解決中心グリッド: {:?}, 取得セル数: {}",
                resolved_center,
                cells.len()
            );
            if cells.is_empty() {
                panic!(
                    "ワールド \"{}\" 内に指定グリッドのセルが見つかりませんでした",
                    world_edid
                );
            }
            cells
        }
        ViewerTarget::WorldGrids(world_edid, grids) => {
            let (_, group_start, group_end) = esm_reader
                .find_world_by_edid(world_edid)
                .expect("Failed to search world")
                .unwrap();
            let mut all_cells = Vec::new();
            for g in grids {
                if let Ok((_, mut cells)) = esm_reader.read_cells_in_world_region(
                    group_start,
                    group_end,
                    Some(*g),
                    0,
                ) {
                    all_cells.append(&mut cells);
                }
            }
            all_cells
        }
        _ => unreachable!(),
    };

    println!("ロード対象セル数: {}", cells.len());
    (esm_reader, cells)
}
