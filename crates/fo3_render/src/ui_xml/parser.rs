//! # Bethesda Menu XML パーサーモジュール
//!
//! Fallout 3 の UI XML 定義 (`dialog_menu.xml` 等) を解析し、AST ツリーを構築する。
//! Prefab (`top_bracket.xml`, `bottom_bracket.xml`) の自動インクルードに対応。
//!
//! 参照元:
//! - `menus/dialog/dialog_menu.xml`
//! - `menus/prefabs/top_bracket.xml`, `bottom_bracket.xml`

use crate::ui_xml::ast::{ExprOp, MenuNode, NodeType, TraitSource, TraitValue};

/// Bethesda XML パーサー。
pub struct MenuXmlParser<'a> {
    /// Prefab インクルード解決関数 (ファイル名 -> XML文字列)
    pub prefab_loader: Option<&'a dyn Fn(&str) -> Option<String>>,
}

impl<'a> MenuXmlParser<'a> {
    /// 新しいパーサーインスタンスを生成する。
    pub fn new(prefab_loader: Option<&'a dyn Fn(&str) -> Option<String>>) -> Self {
        Self { prefab_loader }
    }

    /// XML 文字列からルート `MenuNode` をパースする。
    pub fn parse(&self, xml_text: &str) -> Result<MenuNode, String> {
        let clean_xml = remove_comments_and_normalize(xml_text);
        let mut tokens = tokenize(&clean_xml);
        let mut root_node = None;

        while let Some(tok) = tokens.first() {
            if tok.is_open_tag() {
                let tag_name = tok.tag_name().to_lowercase();
                if matches!(
                    tag_name.as_str(),
                    "menu" | "rect" | "image" | "text" | "hotrect" | "template"
                ) {
                    let node = self.parse_node(&mut tokens)?;
                    root_node = Some(node);
                    break;
                }
            }
            tokens.remove(0);
        }

        root_node.ok_or_else(|| "No valid root menu element found".to_string())
    }

    /// ノード階層を再帰的にパースする。
    fn parse_node(&self, tokens: &mut Vec<XmlToken>) -> Result<MenuNode, String> {
        if tokens.is_empty() {
            return Err("Unexpected EOF while parsing node".to_string());
        }

        let open_tok = tokens.remove(0);
        let tag_name = open_tok.tag_name().to_lowercase();
        let name_attr = open_tok
            .get_attr("name")
            .unwrap_or_else(|| tag_name.clone());

        let node_type = match tag_name.as_str() {
            "menu" => NodeType::Menu,
            "rect" => NodeType::Rect,
            "image" => NodeType::Image,
            "text" => NodeType::Text,
            "hotrect" => NodeType::HotRect,
            "template" => NodeType::Template,
            _ => NodeType::Rect,
        };

        let mut node = MenuNode::new(&name_attr, node_type);

        while !tokens.is_empty() {
            let tok = &tokens[0];

            if tok.is_close_tag() && tok.tag_name().eq_ignore_ascii_case(&tag_name) {
                tokens.remove(0); // 終了タグを消費
                break;
            }

            if tok.is_open_tag() {
                let child_tag = tok.tag_name().to_lowercase();

                // インクルードタグ `<include src="..."/>`
                if child_tag == "include" {
                    let inc_tok = tokens.remove(0);
                    if let Some(src) = inc_tok.get_attr("src") {
                        if let Some(loader) = self.prefab_loader {
                            if let Some(prefab_xml) = loader(&src) {
                                // Prefab XML をパースして直下の子要素を展開
                                if let Ok(prefab_node) = self.parse(&prefab_xml) {
                                    // Prefab の traits をマージ
                                    for (k, v) in prefab_node.traits {
                                        node.traits.insert(k, v);
                                    }
                                    // Prefab の子ノードを自ノードの子に追加
                                    for child in prefab_node.children {
                                        node.children.push(child);
                                    }
                                }
                            }
                        }
                    }
                    continue;
                }

                // 子ノード (rect, image, text, hotrect, template)
                if matches!(
                    child_tag.as_str(),
                    "rect" | "image" | "text" | "hotrect" | "template"
                ) {
                    let child_node = self.parse_node(tokens)?;
                    node.children.push(child_node);
                    continue;
                }

                // Trait プロパティ (x, y, width, height, visible, font, string, filename, _ShowingText 等)
                let trait_name = child_tag.clone();
                let trait_val = self.parse_trait_value(tokens, &trait_name)?;
                node.traits.insert(trait_name, trait_val);
                continue;
            }

            tokens.remove(0);
        }

        Ok(node)
    }

    /// Trait の値または演算式をパースする。
    fn parse_trait_value(
        &self,
        tokens: &mut Vec<XmlToken>,
        trait_name: &str,
    ) -> Result<TraitValue, String> {
        let open_tok = tokens.remove(0);
        if open_tok.is_self_closing() {
            return Ok(TraitValue::String(String::new()));
        }

        let mut ops = Vec::new();
        let mut raw_text = String::new();

        while !tokens.is_empty() {
            let tok = &tokens[0];

            if tok.is_close_tag() && tok.tag_name().eq_ignore_ascii_case(trait_name) {
                tokens.remove(0); // 終了タグを消費
                break;
            }

            if tok.is_text() {
                let txt = tok.text().trim();
                if !txt.is_empty() {
                    raw_text.push_str(txt);
                }
                tokens.remove(0);
                continue;
            }

            if tok.is_open_tag() {
                let op = self.parse_expression_op(tokens)?;
                ops.push(op);
                continue;
            }

            tokens.remove(0);
        }

        if !ops.is_empty() {
            Ok(TraitValue::Expression(ops))
        } else {
            // 文字列または数値・真偽値の判定
            let trimmed = raw_text.trim();
            if let Ok(num) = trimmed.parse::<f32>() {
                Ok(TraitValue::Number(num))
            } else if trimmed.eq_ignore_ascii_case("true") || trimmed == "1" {
                Ok(TraitValue::Bool(true))
            } else if trimmed.eq_ignore_ascii_case("false") || trimmed == "0" {
                Ok(TraitValue::Bool(false))
            } else {
                Ok(TraitValue::String(trimmed.to_string()))
            }
        }
    }

    /// 動的演算式ステップをパースする。
    fn parse_expression_op(&self, tokens: &mut Vec<XmlToken>) -> Result<ExprOp, String> {
        let open_tok = tokens.remove(0);
        let tag = open_tok.tag_name().to_lowercase();
        let src_str = open_tok.get_attr("src");
        let trait_str = open_tok.get_attr("trait").unwrap_or_default();

        let source = src_str.map(|s| match s.to_lowercase().as_str() {
            "screen()" => TraitSource::Screen,
            "me()" => TraitSource::Me,
            "parent()" => TraitSource::Parent,
            "io()" => TraitSource::Io,
            "globals()" => TraitSource::Globals,
            other => {
                if other.starts_with("sibling(") && other.ends_with(')') {
                    let inner = &other[8..other.len() - 1];
                    TraitSource::Sibling(inner.to_string())
                } else {
                    TraitSource::Named(other.to_string())
                }
            }
        });

        // 単一タグで trait 指定がある場合 (例: `<copy src="screen()" trait="width"/>`)
        if let (Some(src), false) = (source.clone(), trait_str.is_empty()) {
            match tag.as_str() {
                "copy" => {
                    return Ok(ExprOp::CopyTrait {
                        source: src,
                        trait_name: trait_str,
                    })
                }
                "add" => {
                    return Ok(ExprOp::Add(Box::new(ExprOp::CopyTrait {
                        source: src,
                        trait_name: trait_str,
                    })))
                }
                "sub" => {
                    return Ok(ExprOp::Sub(Box::new(ExprOp::CopyTrait {
                        source: src,
                        trait_name: trait_str,
                    })))
                }
                "mul" => {
                    return Ok(ExprOp::Mul(Box::new(ExprOp::CopyTrait {
                        source: src,
                        trait_name: trait_str,
                    })))
                }
                "div" => {
                    return Ok(ExprOp::Div(Box::new(ExprOp::CopyTrait {
                        source: src,
                        trait_name: trait_str,
                    })))
                }
                "onlyif" => {
                    return Ok(ExprOp::OnlyIf {
                        condition: Box::new(ExprOp::CopyTrait {
                            source: src,
                            trait_name: trait_str,
                        }),
                        operand: None,
                    })
                }
                "onlyifnot" => {
                    return Ok(ExprOp::OnlyIfNot {
                        condition: Box::new(ExprOp::CopyTrait {
                            source: src,
                            trait_name: trait_str,
                        }),
                        operand: None,
                    })
                }
                _ => {}
            }
        }

        // 子要素またはテキストを持つ演算子タグ (例: `<div> 2 </div>`, `<add> <copy .../> </add>`)
        let mut inner_ops = Vec::new();
        let mut inner_text = String::new();

        if !open_tok.is_self_closing() {
            while !tokens.is_empty() {
                let tok = &tokens[0];
                if tok.is_close_tag() && tok.tag_name().eq_ignore_ascii_case(&tag) {
                    tokens.remove(0);
                    break;
                }
                if tok.is_text() {
                    inner_text.push_str(tok.text().trim());
                    tokens.remove(0);
                    continue;
                }
                if tok.is_open_tag() {
                    let op = self.parse_expression_op(tokens)?;
                    inner_ops.push(op);
                    continue;
                }
                tokens.remove(0);
            }
        }

        let operand = if let Some(op) = inner_ops.into_iter().next() {
            Box::new(op)
        } else if let Ok(val) = inner_text.parse::<f32>() {
            Box::new(ExprOp::Const(val))
        } else if let (Some(src), false) = (source, trait_str.is_empty()) {
            Box::new(ExprOp::CopyTrait {
                source: src,
                trait_name: trait_str,
            })
        } else {
            Box::new(ExprOp::Const(0.0))
        };

        match tag.as_str() {
            "copy" => Ok(*operand),
            "add" => Ok(ExprOp::Add(operand)),
            "sub" => Ok(ExprOp::Sub(operand)),
            "mul" => Ok(ExprOp::Mul(operand)),
            "div" => Ok(ExprOp::Div(operand)),
            "min" => Ok(ExprOp::Min(operand)),
            "max" => Ok(ExprOp::Max(operand)),
            "and" => Ok(ExprOp::And(operand)),
            "or" => Ok(ExprOp::Or(operand)),
            "not" => Ok(ExprOp::Not(operand)),
            _ => Ok(*operand),
        }
    }
}

/// XML コメントを除去し、Bethesda 特有の実体参照を数値に正規化する。
fn remove_comments_and_normalize(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut in_comment = false;
    let chars: Vec<char> = xml.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if !in_comment
            && i + 3 < chars.len()
            && chars[i] == '<'
            && chars[i + 1] == '!'
            && chars[i + 2] == '-'
            && chars[i + 3] == '-'
        {
            in_comment = true;
            i += 4;
            continue;
        }
        if in_comment
            && i + 2 < chars.len()
            && chars[i] == '-'
            && chars[i + 1] == '-'
            && chars[i + 2] == '>'
        {
            in_comment = false;
            i += 3;
            continue;
        }
        if !in_comment {
            out.push(chars[i]);
        }
        i += 1;
    }

    // Bethesda 実体参照の正規化
    out.replace("&true;", "1")
        .replace("&false;", "0")
        .replace("&right;", "2")
        .replace("&center;", "1")
        .replace("&left;", "0")
        .replace("&scale;", "1")
        .replace("&hudmain;", "1")
        .replace("&no_click_past;", "1")
        .replace("&nosound;", "0")
        .replace("&DialogMenu;", "1")
}

/// 単純な XML トークン。
#[derive(Clone, Debug)]
enum XmlToken {
    OpenTag {
        name: String,
        attrs: Vec<(String, String)>,
        self_closing: bool,
    },
    CloseTag {
        name: String,
    },
    Text(String),
}

impl XmlToken {
    fn is_open_tag(&self) -> bool {
        matches!(self, XmlToken::OpenTag { .. })
    }
    fn is_close_tag(&self) -> bool {
        matches!(self, XmlToken::CloseTag { .. })
    }
    fn is_self_closing(&self) -> bool {
        match self {
            XmlToken::OpenTag { self_closing, .. } => *self_closing,
            _ => false,
        }
    }
    fn tag_name(&self) -> &str {
        match self {
            XmlToken::OpenTag { name, .. } => name.as_str(),
            XmlToken::CloseTag { name } => name.as_str(),
            _ => "",
        }
    }
    fn get_attr(&self, attr_name: &str) -> Option<String> {
        match self {
            XmlToken::OpenTag { attrs, .. } => attrs
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(attr_name))
                .map(|(_, v)| v.clone()),
            _ => None,
        }
    }
    fn is_text(&self) -> bool {
        matches!(self, XmlToken::Text(_))
    }
    fn text(&self) -> &str {
        match self {
            XmlToken::Text(s) => s.as_str(),
            _ => "",
        }
    }
}

/// 簡易 XML 字句解析器。
fn tokenize(text: &str) -> Vec<XmlToken> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();

    while let Some(&c) = chars.peek() {
        if c == '<' {
            chars.next();
            let mut tag_str = String::new();
            while let Some(&tc) = chars.peek() {
                if tc == '>' {
                    chars.next();
                    break;
                }
                tag_str.push(tc);
                chars.next();
            }

            let trimmed = tag_str.trim();
            if trimmed.starts_with('/') {
                let name = trimmed[1..].trim().to_string();
                tokens.push(XmlToken::CloseTag { name });
            } else {
                let self_closing = trimmed.ends_with('/');
                let tag_body = if self_closing {
                    trimmed[..trimmed.len() - 1].trim()
                } else {
                    trimmed
                };
                let mut parts = tag_body.split_whitespace();
                let name = parts.next().unwrap_or("").to_string();

                let mut attrs = Vec::new();
                for part in parts {
                    if let Some(eq_idx) = part.find('=') {
                        let k = part[..eq_idx].trim().to_string();
                        let v = part[eq_idx + 1..]
                            .trim()
                            .trim_matches('"')
                            .trim_matches('\'')
                            .to_string();
                        attrs.push((k, v));
                    }
                }

                tokens.push(XmlToken::OpenTag {
                    name,
                    attrs,
                    self_closing,
                });
            }
        } else {
            let mut text_buf = String::new();
            while let Some(&tc) = chars.peek() {
                if tc == '<' {
                    break;
                }
                text_buf.push(tc);
                chars.next();
            }
            let trimmed = text_buf.trim();
            if !trimmed.is_empty() {
                tokens.push(XmlToken::Text(trimmed.to_string()));
            }
        }
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_menu_xml() {
        let xml = r#"
        <menu name="TestMenu">
            <_ShowingText> &true; </_ShowingText>
            <rect name="Box">
                <width> 500 </width>
                <height> 200 </height>
                <x>
                    <copy src="screen()" trait="width"/>
                    <sub> 500 </sub>
                    <div> 2 </div>
                </x>
                <text name="Title">
                    <font> 7 </font>
                    <string> Hello Fallout 3 </string>
                </text>
            </rect>
        </menu>
        "#;

        let parser = MenuXmlParser::new(None);
        let root = parser.parse(xml).expect("parse xml");
        assert_eq!(root.name, "TestMenu");
        assert_eq!(root.children.len(), 1);

        let box_node = &root.children[0];
        assert_eq!(box_node.name, "Box");
        assert_eq!(box_node.children.len(), 1);

        let title_node = &box_node.children[0];
        assert_eq!(title_node.name, "Title");
        assert_eq!(
            title_node.traits.get("string"),
            Some(&TraitValue::String("Hello Fallout 3".to_string()))
        );
    }
}
