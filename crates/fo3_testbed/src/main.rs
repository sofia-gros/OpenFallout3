//! # fo3_testbed
//!
//! Fallout 3 NIF パース検証用 CLI ツール。
//!
//! 指定した NIF ファイルのヘッダーおよびブロック情報を読み込み、
//! 構造の整合性をコンソールに出力します。

use std::env;
use std::fs::File;
use std::io::BufReader;
use fo3_nif::NifHeader;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("使用法: cargo run -p fo3_testbed -- <path/to/mesh.nif>");
        return Ok(());
    }

    let nif_path = &args[1];
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
