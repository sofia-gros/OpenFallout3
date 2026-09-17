//! # 初期化ヘルパーモジュール
//!
//! 仮想ファイルシステム (VFS) の初期化および全 BSA アーカイブの常駐インデックス構築、
//! およびマスター ESM (`Fallout3.esm`) の静的定義一括ロードを担当する。
//!
//! 参照元: Gamebryo 2.6 アーカイブマネージャ, `references/openmw/components/resource/resourcesystem.hpp`

use fo3_esm::EsmMasterContext;
use fo3_vfs::VfsManager;
use std::path::Path;

/// ゲーム起動時に仮想ファイルシステム (VFS) を初期化し、全 BSA アーカイブのインデックスを常駐させる。
pub fn initialize_vfs(data_dir: &str) -> VfsManager {
    let mut vfs = VfsManager::new();
    let data_p = Path::new(data_dir);
    vfs.add_loose_root(data_p);

    let bsa_names = [
        "Fallout - Meshes.bsa",
        "Fallout - Textures.bsa",
        "Fallout - Misc.bsa",
        "Fallout - Sound.bsa",
        "Fallout - Voices.bsa",
    ];
    for bsa_name in &bsa_names {
        let bsa_file = data_p.join(bsa_name);
        if bsa_file.exists() {
            if let Ok(archive) = fo3_bsa::BsaArchive::open(&bsa_file) {
                vfs.add_bsa(archive);
            }
        }
    }
    vfs
}

/// マスター ESM (`Fallout3.esm`) から全静的定義 (3Dモデル、アクター、防具、光源) を一括ロードして常駐させる。
pub fn initialize_master_context(data_dir: &str) -> EsmMasterContext {
    let esm_path = Path::new(data_dir).join("Fallout3.esm");
    if esm_path.exists() {
        println!(
            "マスター ESM \"{:?}\" から静的定義を一括ロード中...",
            esm_path
        );
        match EsmMasterContext::open_and_load(&esm_path) {
            Ok(ctx) => {
                println!(
                    "マスター静的定義常駐完了: モデル={}, NPC={}, 防具={}, 光源={}, クエスト={}, AIパッケージ={}",
                    ctx.model_map.len(),
                    ctx.npc_map.len(),
                    ctx.armor_map.len(),
                    ctx.light_map.len(),
                    ctx.quest_map.len(),
                    ctx.pack_map.len(),
                );
                ctx
            }
            Err(e) => {
                eprintln!("マスター ESM の読み込みに失敗しました: {:?}", e);
                EsmMasterContext::new()
            }
        }
    } else {
        println!(
            "マスター ESM が指定ディレクトリに見つかりません: {:?}",
            esm_path
        );
        EsmMasterContext::new()
    }
}
