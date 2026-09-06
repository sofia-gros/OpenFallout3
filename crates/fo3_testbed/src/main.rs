//! # fo3_testbed
//!
//! Fallout 3 アセット（NIF / BSA）検証用 CLI ツール。
//!
//! - NIF パース検証: `fo3_testbed nif <path/to/file.nif>`
//! - BSA ファイル一覧表示: `fo3_testbed bsa-list <path/to/archive.bsa>`
//! - BSA ファイル抽出検証: `fo3_testbed bsa-extract <path/to/archive.bsa> <relative/path>`

use std::env;
use std::fs::File;
use std::io::{BufReader, Cursor};
use fo3_bsa::BsaArchive;
use fo3_nif::NifHeader;

fn print_usage() {
    println!("使用法:");
    println!("  cargo run -p fo3_testbed -- nif <path/to/mesh.nif>");
    println!("  cargo run -p fo3_testbed -- bsa-list <path/to/archive.bsa>");
    println!("  cargo run -p fo3_testbed -- bsa-extract <path/to/archive.bsa> <relative/path>");
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
        _ => {
            // 後方互換: 直接 nif パスが渡された場合
            if args[1].ends_with(".nif") {
                test_nif(&args[1])?;
            } else {
                print_usage();
            }
        }
    }

    Ok(())
}
