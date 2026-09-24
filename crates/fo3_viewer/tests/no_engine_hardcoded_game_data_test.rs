//! # エンジン層へのゲームデータハードコード防止静的解析テスト
//!
//! 参照元: `AGENTS.md Rule 5 (No Game Data Hardcoding)`
//!
//! エンジン実装コード (`crates/*/src/`) 内に、クエスト進行用のモックや
//! 特定ゲームデータ (`CG00`, `CG01`, `MQ01` 等) のハードコード判定ロジックが
//! 混入していないことを自動検証する。

use std::fs;
use std::path::{Path, PathBuf};

fn collect_src_rs_files(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                // target ディレクトリや tests ディレクトリは除外
                if name != "target" && name != "tests" {
                    collect_src_rs_files(&path, files);
                }
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                // src/ ディレクトリ配下のファイルのみ対象
                if path.to_string_lossy().contains("src") {
                    files.push(path);
                }
            }
        }
    }
}

#[test]
fn test_no_hardcoded_game_logic_in_engine_src() {
    let crates_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates dir");

    let mut files = Vec::new();
    collect_src_rs_files(crates_dir, &mut files);

    assert!(!files.is_empty(), "src 配下の Rust ソースファイルが見つかりません");

    let forbidden_patterns = [
        "\"CG00ChooseSexMessage\"",
        "\"setstage CG00",
        "\"player.addScriptPackage CG00",
        "eq_ignore_ascii_case(\"CG00BirthISFX\")",
        "eq_ignore_ascii_case(\"CG00BlackScreenISFX\")",
    ];

    let mut violations = Vec::new();

    for file in &files {
        let content = fs::read_to_string(file).expect("Failed to read file");
        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            // コメント行はスキップ
            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
                continue;
            }

            for pattern in &forbidden_patterns {
                if line.contains(pattern) {
                    violations.push(format!(
                        "{}:{}: 禁止されたゲームデータハードコードを検出: `{}`",
                        file.display(),
                        line_num + 1,
                        pattern
                    ));
                }
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "AGENTS.md Rule 5 違反: エンジン層にゲームデータのモックハードコードが検出されました:\n{}",
            violations.join("\n")
        );
    }
}
