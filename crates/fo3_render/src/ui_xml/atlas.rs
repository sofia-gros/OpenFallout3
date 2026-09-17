//! # TAI (Texture Atlas Index) パーサーモジュール
//!
//! Fallout 3 の UI テクスチャアトラス設定ファイル (`InterfaceShared.tai` 等) をパースし、
//! 各パーツテクスチャのアトラス画像名および正規化 UV 座標矩形を提供する。
//!
//! 参照元:
//! - `textures/interface/interfaceshared.tai`
//! - `menus/prefabs/top_bracket.xml`, `bottom_bracket.xml`

use std::collections::HashMap;

/// アトラス内での 2D テクスチャ UV 座標矩形。
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AtlasSubTexture {
    /// 最小 U 座標 (左)
    pub u_min: f32,
    /// 最小 V 座標 (上)
    pub v_min: f32,
    /// 最大 U 座標 (右)
    pub u_max: f32,
    /// 最大 V 座標 (下)
    pub v_max: f32,
}

/// テクスチャアトラスインデックス。
#[derive(Clone, Debug, Default)]
pub struct TextureAtlas {
    /// テクスチャ小ファイル名 (小文字、例: "solid.dds") -> (アトラスdds名, UV矩形)
    pub entries: HashMap<String, (String, AtlasSubTexture)>,
}

impl TextureAtlas {
    /// TAI テキストデータからアトラスインデックスをパースする。
    pub fn parse(text: &str) -> Self {
        let mut entries = HashMap::new();

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // フォーマット:
            // <filename> <atlas filename>, <atlas idx>, <atlas type>, <woffset>, <hoffset>, <depth offset>, <width>, <height>
            // 例: solid.dds InterfaceShared0.dds, 0, 2D, 0.801025, 0.816650, 0.000000, 0.007324, 0.007324
            let parts: Vec<&str> = trimmed
                .split('\t')
                .filter(|s| !s.trim().is_empty())
                .collect();
            if parts.len() < 2 {
                // タブではなく連続空白の場合もフォールバック
                let tokens: Vec<&str> = trimmed.split_whitespace().collect();
                if tokens.len() >= 8 {
                    let filename = tokens[0].trim().to_lowercase();
                    let atlas_file = tokens[1].trim_end_matches(',').to_lowercase();
                    let woffset: f32 = tokens[4].trim_end_matches(',').parse().unwrap_or(0.0);
                    let hoffset: f32 = tokens[5].trim_end_matches(',').parse().unwrap_or(0.0);
                    let width: f32 = tokens[7].trim_end_matches(',').parse().unwrap_or(0.0);
                    let height: f32 = tokens
                        .get(8)
                        .map(|s| s.trim_end_matches(',').parse().unwrap_or(0.0))
                        .unwrap_or(0.0);

                    entries.insert(
                        filename,
                        (
                            atlas_file,
                            AtlasSubTexture {
                                u_min: woffset,
                                v_min: hoffset,
                                u_max: woffset + width,
                                v_max: hoffset + height,
                            },
                        ),
                    );
                }
                continue;
            }

            let filename = parts[0].trim().to_lowercase();
            let right_side = parts[1].trim();
            let fields: Vec<&str> = right_side.split(',').map(|s| s.trim()).collect();
            if fields.len() >= 8 {
                let atlas_file = fields[0].to_lowercase();
                let woffset: f32 = fields[3].parse().unwrap_or(0.0);
                let hoffset: f32 = fields[4].parse().unwrap_or(0.0);
                let width: f32 = fields[6].parse().unwrap_or(0.0);
                let height: f32 = fields[7].parse().unwrap_or(0.0);

                entries.insert(
                    filename,
                    (
                        atlas_file,
                        AtlasSubTexture {
                            u_min: woffset,
                            v_min: hoffset,
                            u_max: woffset + width,
                            v_max: hoffset + height,
                        },
                    ),
                );
            }
        }

        Self { entries }
    }

    /// 指定テクスチャ名の UV 座標およびアトラスファイル名を取得する。
    pub fn lookup(&self, name: &str) -> Option<(&str, AtlasSubTexture)> {
        let clean = name.trim().to_lowercase();
        // パス付きの場合は末尾のファイル名のみで照会
        let base_name = std::path::Path::new(&clean)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&clean);

        self.entries
            .get(base_name)
            .map(|(atlas, uv)| (atlas.as_str(), *uv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tai_line() {
        let tai_sample = r#"
# InterfaceShared.tai
fade_to_top.dds		InterfaceShared0.dds, 0, 2D, 0.754150, 0.812988, 0.000000, 0.007324, 0.030273
solid.dds		InterfaceShared0.dds, 0, 2D, 0.801025, 0.816650, 0.000000, 0.007324, 0.007324
"#;
        let atlas = TextureAtlas::parse(tai_sample);
        let solid = atlas.lookup("solid.dds").expect("solid.dds exists");
        assert_eq!(solid.0, "interfaceshared0.dds");
        assert!((solid.1.u_min - 0.801025).abs() < 1e-5);
        assert!((solid.1.v_min - 0.816650).abs() < 1e-5);
        assert!((solid.1.u_max - (0.801025 + 0.007324)).abs() < 1e-5);
    }
}
