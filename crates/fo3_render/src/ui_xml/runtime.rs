//! # Bethesda Menu XML ランタイム・レンダリングエンジン
//!
//! パースされた MenuNode ツリーを画面解像度および実行時状態変数に従って評価し、
//! 実機 DDS テクスチャおよび BitmapFont グリフによる描画バッチを生成する。
//!
//! 参照元:
//! - `menus/dialog/dialog_menu.xml`
//! - `menus/prefabs/top_bracket.xml`, `bottom_bracket.xml`

use crate::ui::{colors as ui_colors, BitmapFont, TextBatch};
use crate::ui_xml::ast::{ExprOp, MenuNode, NodeType, TraitSource, TraitValue};
use crate::ui_xml::atlas::TextureAtlas;
use std::collections::HashMap;

/// 計算済みノードのレイアウト情報。
#[derive(Clone, Debug, Default)]
pub struct ComputedLayout {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub visible: bool,
    pub alpha: f32,
}

/// Menu XML 実行ランタイム。
#[derive(Clone, Debug)]
pub struct MenuRuntime {
    /// ルートメニューノード
    pub root: MenuNode,
    /// 外部状態変数 (例: `_ShowingText` -> 1.0, `_DialogVisible` -> 1.0)
    pub io_traits: HashMap<String, f32>,
    /// 文字列変数 (例: "DM_SpeakerNameLabel" -> "NOVA")
    pub text_overrides: HashMap<String, String>,
    /// テクスチャアトラス設定
    pub atlas: Option<TextureAtlas>,
}

impl MenuRuntime {
    /// 新しいランタイムを初期化する。
    pub fn new(root: MenuNode) -> Self {
        let mut io_traits = HashMap::new();
        io_traits.insert("_showingtext".to_string(), 1.0);
        io_traits.insert("_dialogvisible".to_string(), 1.0);
        io_traits.insert("_showsubtitles".to_string(), 1.0);
        io_traits.insert("_minlistheight".to_string(), 110.0);

        Self {
            root,
            io_traits,
            text_overrides: HashMap::new(),
            atlas: None,
        }
    }

    /// 外部状態フラグを設定する。
    pub fn set_io_flag(&mut self, name: &str, val: bool) {
        self.io_traits
            .insert(name.to_lowercase(), if val { 1.0 } else { 0.0 });
    }

    /// 外部状態数値を設定する。
    pub fn set_io_num(&mut self, name: &str, val: f32) {
        self.io_traits.insert(name.to_lowercase(), val);
    }

    /// テキスト表示文字列を設定する。
    pub fn set_text(&mut self, node_name: &str, text: &str) {
        self.text_overrides
            .insert(node_name.to_lowercase(), text.to_string());
    }

    /// 画面解像度に合わせて全ノードのレイアウトとプロパティを解決し、TextBatch にレンダリングする。
    pub fn render_to_batch(
        &self,
        batch: &mut TextBatch,
        font_main: &BitmapFont,
        font_large: &BitmapFont,
        screen_w: f32,
        screen_h: f32,
    ) {
        let mut computed = HashMap::new();
        self.evaluate_node(&self.root, None, screen_w, screen_h, &mut computed);
        self.render_node(&self.root, &computed, batch, font_main, font_large);
    }

    /// 再帰的にノードの座標とサイズを解決する。
    fn evaluate_node(
        &self,
        node: &MenuNode,
        parent: Option<&MenuNode>,
        screen_w: f32,
        screen_h: f32,
        computed: &mut HashMap<String, ComputedLayout>,
    ) {
        // 幅・高さの評価
        let width = self
            .eval_trait_num(node, "width", parent, screen_w, screen_h, computed)
            .unwrap_or(0.0);
        let height = self
            .eval_trait_num(node, "height", parent, screen_w, screen_h, computed)
            .unwrap_or(0.0);

        // 一時登録 (自己参照 me() 用)
        computed.insert(
            node.name.to_lowercase(),
            ComputedLayout {
                x: 0.0,
                y: 0.0,
                width,
                height,
                visible: true,
                alpha: 1.0,
            },
        );

        // X・Y 座標の評価
        let x = self
            .eval_trait_num(node, "x", parent, screen_w, screen_h, computed)
            .unwrap_or(0.0);
        let y = self
            .eval_trait_num(node, "y", parent, screen_w, screen_h, computed)
            .unwrap_or(0.0);

        // 可視性 (visible)
        let visible = self
            .eval_trait_num(node, "visible", parent, screen_w, screen_h, computed)
            .map(|v| v > 0.5)
            .unwrap_or(true);

        // アルファ値
        let alpha = self
            .eval_trait_num(node, "alpha", parent, screen_w, screen_h, computed)
            .map(|a| (a / 255.0).clamp(0.0, 1.0))
            .unwrap_or(1.0);

        computed.insert(
            node.name.to_lowercase(),
            ComputedLayout {
                x,
                y,
                width,
                height,
                visible,
                alpha,
            },
        );

        for child in &node.children {
            self.evaluate_node(child, Some(node), screen_w, screen_h, computed);
        }
    }

    /// 単一 Trait を数値として評価する。
    fn eval_trait_num(
        &self,
        node: &MenuNode,
        trait_name: &str,
        parent: Option<&MenuNode>,
        screen_w: f32,
        screen_h: f32,
        computed: &HashMap<String, ComputedLayout>,
    ) -> Option<f32> {
        let trait_key = trait_name.to_lowercase();
        if let Some(val) = node.traits.get(&trait_key) {
            match val {
                TraitValue::Number(n) => Some(*n),
                TraitValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
                TraitValue::Expression(ops) => {
                    let mut current = 0.0;
                    for op in ops {
                        self.apply_expr_op(
                            op,
                            &mut current,
                            node,
                            parent,
                            screen_w,
                            screen_h,
                            computed,
                        );
                    }
                    Some(current)
                }
                _ => None,
            }
        } else {
            None
        }
    }

    /// 演算ステップを適用する。
    fn apply_expr_op(
        &self,
        op: &ExprOp,
        current: &mut f32,
        node: &MenuNode,
        parent: Option<&MenuNode>,
        screen_w: f32,
        screen_h: f32,
        computed: &HashMap<String, ComputedLayout>,
    ) {
        match op {
            ExprOp::Const(val) => *current = *val,
            ExprOp::CopyTrait { source, trait_name } => {
                *current = self.resolve_trait_val(
                    source, trait_name, node, parent, screen_w, screen_h, computed,
                );
            }
            ExprOp::Add(inner) => {
                let mut v = 0.0;
                self.apply_expr_op(inner, &mut v, node, parent, screen_w, screen_h, computed);
                *current += v;
            }
            ExprOp::Sub(inner) => {
                let mut v = 0.0;
                self.apply_expr_op(inner, &mut v, node, parent, screen_w, screen_h, computed);
                *current -= v;
            }
            ExprOp::Mul(inner) => {
                let mut v = 0.0;
                self.apply_expr_op(inner, &mut v, node, parent, screen_w, screen_h, computed);
                *current *= v;
            }
            ExprOp::Div(inner) => {
                let mut v = 1.0;
                self.apply_expr_op(inner, &mut v, node, parent, screen_w, screen_h, computed);
                if v != 0.0 {
                    *current /= v;
                }
            }
            ExprOp::Min(inner) => {
                let mut v = 0.0;
                self.apply_expr_op(inner, &mut v, node, parent, screen_w, screen_h, computed);
                *current = current.min(v);
            }
            ExprOp::Max(inner) => {
                let mut v = 0.0;
                self.apply_expr_op(inner, &mut v, node, parent, screen_w, screen_h, computed);
                *current = current.max(v);
            }
            ExprOp::And(inner) => {
                let mut v = 0.0;
                self.apply_expr_op(inner, &mut v, node, parent, screen_w, screen_h, computed);
                *current = if *current > 0.5 && v > 0.5 { 1.0 } else { 0.0 };
            }
            ExprOp::Or(inner) => {
                let mut v = 0.0;
                self.apply_expr_op(inner, &mut v, node, parent, screen_w, screen_h, computed);
                *current = if *current > 0.5 || v > 0.5 { 1.0 } else { 0.0 };
            }
            ExprOp::Not(inner) => {
                let mut v = 0.0;
                self.apply_expr_op(inner, &mut v, node, parent, screen_w, screen_h, computed);
                *current = if v <= 0.5 { 1.0 } else { 0.0 };
            }
            ExprOp::OnlyIf { condition, operand } => {
                let mut cond_val = 0.0;
                self.apply_expr_op(
                    condition,
                    &mut cond_val,
                    node,
                    parent,
                    screen_w,
                    screen_h,
                    computed,
                );
                if cond_val > 0.5 {
                    if let Some(opnd) = operand {
                        let mut v = 0.0;
                        self.apply_expr_op(
                            opnd, &mut v, node, parent, screen_w, screen_h, computed,
                        );
                        *current += v;
                    }
                } else {
                    *current = 0.0;
                }
            }
            ExprOp::OnlyIfNot { condition, operand } => {
                let mut cond_val = 0.0;
                self.apply_expr_op(
                    condition,
                    &mut cond_val,
                    node,
                    parent,
                    screen_w,
                    screen_h,
                    computed,
                );
                if cond_val <= 0.5 {
                    if let Some(opnd) = operand {
                        let mut v = 0.0;
                        self.apply_expr_op(
                            opnd, &mut v, node, parent, screen_w, screen_h, computed,
                        );
                        *current += v;
                    }
                } else {
                    *current = 0.0;
                }
            }
        }
    }

    /// ソース指定から Trait 値を解決する。
    fn resolve_trait_val(
        &self,
        src: &TraitSource,
        trait_name: &str,
        node: &MenuNode,
        parent: Option<&MenuNode>,
        screen_w: f32,
        screen_h: f32,
        computed: &HashMap<String, ComputedLayout>,
    ) -> f32 {
        let tname = trait_name.to_lowercase();
        match src {
            TraitSource::Screen => match tname.as_str() {
                "width" => screen_w,
                "height" => screen_h,
                "cropx" => 0.0,
                "cropy" => 0.0,
                _ => 0.0,
            },
            TraitSource::Me => {
                if let Some(layout) = computed.get(&node.name.to_lowercase()) {
                    match tname.as_str() {
                        "width" => layout.width,
                        "height" => layout.height,
                        "x" => layout.x,
                        "y" => layout.y,
                        _ => 0.0,
                    }
                } else {
                    0.0
                }
            }
            TraitSource::Parent => {
                if let Some(p) = parent {
                    if let Some(layout) = computed.get(&p.name.to_lowercase()) {
                        match tname.as_str() {
                            "width" => layout.width,
                            "height" => layout.height,
                            "x" => layout.x,
                            "y" => layout.y,
                            _ => 0.0,
                        }
                    } else {
                        0.0
                    }
                } else {
                    0.0
                }
            }
            TraitSource::Sibling(sib_name) | TraitSource::Named(sib_name) => {
                if let Some(layout) = computed.get(&sib_name.to_lowercase()) {
                    match tname.as_str() {
                        "width" => layout.width,
                        "height" => layout.height,
                        "x" => layout.x,
                        "y" => layout.y,
                        _ => 0.0,
                    }
                } else {
                    0.0
                }
            }
            TraitSource::Io => self.io_traits.get(&tname).copied().unwrap_or(0.0),
            TraitSource::Globals => match tname.as_str() {
                "_line_thickness" => 2.0,
                "_background_fill_alpha" => 200.0,
                _ => 0.0,
            },
        }
    }

    /// 再帰的に描画バッチへノードを送出する。
    fn render_node(
        &self,
        node: &MenuNode,
        computed: &HashMap<String, ComputedLayout>,
        batch: &mut TextBatch,
        font_main: &BitmapFont,
        font_large: &BitmapFont,
    ) {
        if let Some(layout) = computed.get(&node.name.to_lowercase()) {
            if !layout.visible {
                return; // 非表示なら子要素含めスキップ
            }

            match node.node_type {
                NodeType::Image => {
                    let color = [
                        crate::ui_colors::PIPBOY_GREEN[0],
                        crate::ui_colors::PIPBOY_GREEN[1],
                        crate::ui_colors::PIPBOY_GREEN[2],
                        layout.alpha,
                    ];
                    let filename = node.traits.get("filename").and_then(|v| match v {
                        TraitValue::String(s) => Some(s.as_str()),
                        _ => None,
                    });

                    let mut rendered_from_atlas = false;
                    if let Some(atlas) = &self.atlas {
                        if let Some(file) = filename {
                            if let Some((atlas_tex, sub_tex)) = atlas.lookup(file) {
                                batch.set_texture(crate::ui::font::UiTexture::Image(atlas_tex.to_string()));
                                batch.add_textured_rect(
                                    layout.x,
                                    layout.y,
                                    layout.width.max(1.0),
                                    layout.height.max(1.0),
                                    [sub_tex.u_min, sub_tex.v_min, sub_tex.u_max, sub_tex.v_max],
                                    color,
                                );
                                rendered_from_atlas = true;
                            }
                        }
                    }

                    if !rendered_from_atlas {
                        if let Some(file) = filename {
                            if !file.is_empty() && !file.contains("solid") {
                                batch.set_texture(crate::ui::font::UiTexture::Image(file.to_string()));
                                batch.add_textured_rect(
                                    layout.x,
                                    layout.y,
                                    layout.width.max(1.0),
                                    layout.height.max(1.0),
                                    [0.0, 0.0, 1.0, 1.0],
                                    color,
                                );
                                rendered_from_atlas = true;
                            }
                        }
                    }

                    if !rendered_from_atlas {
                        if node.name.eq_ignore_ascii_case("DM_TextBackground") {
                            batch.add_rect(
                                layout.x,
                                layout.y,
                                layout.width,
                                layout.height,
                                [0.0, 0.0, 0.0, 0.85],
                            );
                        } else {
                            batch.add_rect(
                                layout.x,
                                layout.y,
                                layout.width.max(2.0),
                                layout.height.max(2.0),
                                color,
                            );
                        }
                    }
                }
                NodeType::Text => {
                    let text = self
                        .text_overrides
                        .get(&node.name.to_lowercase())
                        .cloned()
                        .or_else(|| {
                            node.traits.get("string").and_then(|v| match v {
                                TraitValue::String(s) => Some(s.clone()),
                                _ => None,
                            })
                        })
                        .unwrap_or_default();

                    if !text.is_empty() {
                        let font_id = node
                            .traits
                            .get("font")
                            .and_then(|v| match v {
                                TraitValue::Number(n) => Some(*n as u32),
                                _ => None,
                            })
                            .unwrap_or(6);

                        let font = if font_id >= 7 { font_large } else { font_main };
                        let color = if font_id >= 7 {
                            ui_colors::HIGHLIGHT_WHITE
                        } else {
                            ui_colors::PIPBOY_GREEN
                        };
                        batch.add_text(font, &text, layout.x, layout.y, 1.0, color);
                    }
                }
                NodeType::Rect | NodeType::HotRect | NodeType::Menu | NodeType::Template => {}
            }
        }

        for child in &node.children {
            self.render_node(child, computed, batch, font_main, font_large);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_xml::parser::MenuXmlParser;

    #[test]
    fn test_runtime_evaluate_center_and_width() {
        let xml = r#"
        <menu name="DialogMenu">
            <_ShowingText> &true; </_ShowingText>
            <_DialogVisible> &true; </_DialogVisible>
            <rect name="DM_TextBackground">
                <width> 1080 </width>
                <x>
                    <copy src="screen()" trait="width"/>
                    <sub src="me()" trait="width"/>
                    <div> 2 </div>
                </x>
                <y>
                    <copy src="screen()" trait="height"/>
                    <mul> 0.8 </mul>
                </y>
            </rect>
        </menu>
        "#;

        let parser = MenuXmlParser::new(None);
        let root = parser.parse(xml).expect("parse xml");
        let runtime = MenuRuntime::new(root);

        let screen_w = 1920.0;
        let screen_h = 1080.0;
        let mut computed = HashMap::new();
        runtime.evaluate_node(&runtime.root, None, screen_w, screen_h, &mut computed);

        let bg = computed
            .get("dm_textbackground")
            .expect("dm_textbackground layout");
        assert_eq!(bg.width, 1080.0);
        // x = (1920 - 1080) / 2 = 840 / 2 = 420
        assert_eq!(bg.x, 420.0);
        // y = 1080 * 0.8 = 864
        assert_eq!(bg.y, 864.0);
    }
}
