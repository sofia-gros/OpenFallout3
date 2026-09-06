//! # fo3_testbed
//!
//! Fallout 3 アセット（NIF / BSA / VFS）検証用 CLI ツール。
//!
//! - NIF パース検証: `fo3_testbed nif <path/to/file.nif>`
//! - BSA ファイル一覧表示: `fo3_testbed bsa-list <path/to/archive.bsa>`
//! - BSA ファイル抽出検証: `fo3_testbed bsa-extract <path/to/archive.bsa> <relative/path>`
//! - VFS 統合読み込み検証: `fo3_testbed vfs-test <data_dir> <relative/path>`

use std::env;
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::Path;
use fo3_bsa::BsaArchive;
use fo3_nif::NifHeader;
use fo3_vfs::VfsManager;

fn print_usage() {
    println!("使用法:");
    println!("  cargo run -p fo3_testbed -- nif <path/to/mesh.nif>");
    println!("  cargo run -p fo3_testbed -- bsa-list <path/to/archive.bsa>");
    println!("  cargo run -p fo3_testbed -- bsa-extract <path/to/archive.bsa> <relative/path>");
    println!("  cargo run -p fo3_testbed -- vfs-test <data_dir> <relative/path>");
}

fn test_nif(nif_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== NIF パース検証: {} ===", nif_path);

    let file = File::open(nif_path)?;
    let mut reader = BufReader::new(file);

    let header = NifHeader::read(&mut reader)?;

    println!("ヘッダー文字列: {}", header.header_string.trim());
    println!("NIF バージョン: {:#X}", header.version);
    println!("ユーザーバージョン: {}", header.user_version);
    println!("BS バージョン: {}", header.bs_header.bs_version);
    println!("著者情報: {}", header.bs_header.author.value);
    println!("エクスポートスクリプト: {}", header.bs_header.export_script.value);
    println!("ブロック総数: {}", header.num_blocks);
    println!("使用ブロック型数: {}", header.block_types.len());
    println!("ブロック型一覧:");
    for (i, btype) in header.block_types.iter().enumerate() {
        println!("  [{}] {}", i, btype);
    }
    println!("グローバル文字列数: {}", header.strings.len());

    println!("\nヘッダーパース成功！");
    Ok(())
}

fn test_bsa_list(bsa_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== BSA アーカイブ検証: {} ===", bsa_path);

    let archive = BsaArchive::open(bsa_path)?;
    let header = archive.header();

    println!("バージョン: {}", header.version);
    println!("フラグ: {:#X}", header.flags);
    println!("ディレクトリ数: {}", header.folder_count);
    println!("ファイル総数: {}", header.file_count);

    let files = archive.list_files();
    println!("展開ファイル数: {}", files.len());

    println!("\n先頭 10 件のファイル:");
    for (i, f) in files.iter().take(10).enumerate() {
        println!("  [{}] {}", i, f);
    }

    if files.len() > 10 {
        println!("...\n末尾 5 件のファイル:");
        for (i, f) in files.iter().skip(files.len() - 5).enumerate() {
            println!("  [{}] {}", files.len() - 5 + i, f);
        }
    }

    println!("\nBSA ファイル一覧取得成功！");
    Ok(())
}

fn test_bsa_extract(bsa_path: &str, relative_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== BSA ファイル抽出検証: {} ({}) ===", bsa_path, relative_path);

    let mut archive = BsaArchive::open(bsa_path)?;
    let data = archive.extract_file(relative_path)?;

    println!("抽出成功: {} バイト解凍展開完了", data.len());

    // NIF ファイルの場合は NifHeader でパース検証
    if relative_path.to_ascii_lowercase().ends_with(".nif") {
        println!("\n--- 抽出された NIF データの整合性検証 ---");
        let mut cursor = Cursor::new(&data);
        let header = NifHeader::read(&mut cursor)?;
        println!("NIF バージョン: {:#X}", header.version);
        println!("ブロック総数: {}", header.num_blocks);
        println!("使用ブロック型一覧:");
        for (i, btype) in header.block_types.iter().enumerate() {
            println!("  [{}] {}", i, btype);
        }
        println!("NIF ヘッダーパース検証成功！");
    }

    Ok(())
}

fn test_vfs(data_dir: &str, relative_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== VFS 統合読み込み検証 ===");
    println!("Data ディレクトリ: {}", data_dir);
    println!("ターゲット相対パス: {}", relative_path);

    let mut vfs = VfsManager::new();
    let data_path = Path::new(data_dir);

    // 1. ルーズファイルルートを登録
    vfs.add_loose_root(data_path);
    println!("ルーズファイルルート登録: OK");

    // 2. BSA アーカイブを自動探索してマウント
    let bsa_names = [
        "Fallout - Meshes.bsa",
        "Fallout - Textures.bsa",
        "Fallout - Misc.bsa",
    ];

    for bsa_name in &bsa_names {
        let bsa_file = data_path.join(bsa_name);
        if bsa_file.exists() {
            let archive = BsaArchive::open(&bsa_file)?;
            println!("BSA マウント成功: {} (ファイル数: {})", bsa_name, archive.list_files().len());
            vfs.add_bsa(archive);
        }
    }

    // 3. ファイル存在判定
    let exists = vfs.exists(relative_path);
    println!("VFS exists 判定: {}", exists);

    // 4. 透過読み込み
    let data = vfs.read(relative_path)?;
    println!("VFS 読み込み成功: {} バイト取得完了", data.len());

    // NIF ファイルの場合はパース検証
    if relative_path.to_ascii_lowercase().ends_with(".nif") {
        println!("\n--- 読み込まれた NIF データの整合性検証 ---");
        let mut cursor = Cursor::new(&data);
        let header = NifHeader::read(&mut cursor)?;
        println!("NIF バージョン: {:#X}", header.version);
        println!("ブロック総数: {}", header.num_blocks);
        println!("NIF ヘッダーパース検証成功！");
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "nif" => {
            if args.len() < 3 {
                print_usage();
                return Ok(());
            }
            test_nif(&args[2])?;
        }
        "bsa-list" => {
            if args.len() < 3 {
                print_usage();
                return Ok(());
            }
            test_bsa_list(&args[2])?;
        }
        "bsa-extract" => {
            if args.len() < 4 {
                print_usage();
                return Ok(());
            }
            test_bsa_extract(&args[2], &args[3])?;
        }
        "vfs-test" => {
            if args.len() < 4 {
                print_usage();
                return Ok(());
            }
            test_vfs(&args[2], &args[3])?;
        }
        _ => {
            if args[1].ends_with(".nif") {
                test_nif(&args[1])?;
            } else {
                print_usage();
            }
        }
    }

    Ok(())
}
