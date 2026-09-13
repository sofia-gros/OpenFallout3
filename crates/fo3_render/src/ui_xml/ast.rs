//! # Bethesda Menu XML 抽象構文木 (AST) 定義モジュール
//!
//! Fallout 3 の UI 定義 XML (`dialog_menu.xml`, `hud_main_menu.xml` 等) の
//! 要素ノード、プロパティ (Trait)、および動的評価演算式を表現する。
//!
//! 参照元:
//! - `menus/dialog/dialog_menu.xml`
//! - `menus/prefabs/top_bracket.xml`, `bottom_bracket.xml`

use std::collections::HashMap;

/// Menu XML の要素ノード種別。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeType {
    /// ルートメニュー (`<menu>`)
    Menu,
    /// 汎用矩形コンテナ (`<rect>`)
    Rect,
    /// 画像・テクスチャ表示 (`<image>`)
    Image,
    /// テキスト文字列表示 (`<text>`)
    Text,
    /// マウス・キー入力受付矩形 (`<hotrect>`)
    HotRect,
    /// リストアイテム動的生成テンプレート (`<template>`)
    Template,
}

/// プロパティ (Trait) 参照先コンテキスト。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraitSource {
    /// 画面全体 (`screen()`)
    Screen,
    /// 自身 (`me()`)
    Me,
    /// 親ノード (`parent()`)
    Parent,
    /// 兄弟ノード (`sibling(name)`)
    Sibling(String),
    /// 入出力/外部状態 (`io()`)
    Io,
    /// グローバル設定 (`globals()`)
    Globals,
    /// 名前指定ノード直接参照 (例: `src="DM_TextBackground"`)
    Named(String),
}

/// 動的評価演算式の単一ステップ。
#[derive(Clone, Debug, PartialEq)]
pub enum ExprOp {
    /// 定数値の代入 (`<copy> 123 </copy>`)
    Const(f32),
    /// 他ノード Trait の取得 (`<copy src="..." trait="..."/>`)
    CopyTrait { source: TraitSource, trait_name: String },
    /// 加算 (`<add> ... </add>`)
    Add(Box<ExprOp>),
    /// 減算 (`<sub> ... </sub>`)
    Sub(Box<ExprOp>),
    /// 乗算 (`<mul> ... </mul>`)
    Mul(Box<ExprOp>),
    /// 除算 (`<div> ... </div>`)
    Div(Box<ExprOp>),
    /// 最小値 (`<min> ... </min>`)
    Min(Box<ExprOp>),
    /// 最大値 (`<max> ... </max>`)
    Max(Box<ExprOp>),
    /// 論理積 (`<and> ... </and>`)
    And(Box<ExprOp>),
    /// 論理和 (`<or> ... </or>`)
    Or(Box<ExprOp>),
    /// 論理否定 (`<not src="..." trait="..."/>`)
    Not(Box<ExprOp>),
    /// 条件成立時のみ加算 (`<onlyif src="..." trait="..."/>`)
    OnlyIf { condition: Box<ExprOp>, operand: Option<Box<ExprOp>> },
    /// 条件不成立時のみ加算 (`<onlyifnot src="..." trait="..."/>`)
    OnlyIfNot { condition: Box<ExprOp>, operand: Option<Box<ExprOp>> },
}

/// プロパティ値 (Trait)。
#[derive(Clone, Debug, PartialEq)]
pub enum TraitValue {
    /// 即値の浮動小数点数値
    Number(f32),
    /// 即値の文字列
    String(String),
    /// 即値の真偽値
    Bool(bool),
    /// 動的計算式 (順次評価ステップ群)
    Expression(Vec<ExprOp>),
}

/// 単一の Menu XML 要素ノード。
#[derive(Clone, Debug)]
pub struct MenuNode {
    /// ノード名 (例: "DialogMenu", "DM_TextBackground", "DM_SpeakerText")
    pub name: String,
    /// 要素種別
    pub node_type: NodeType,
    /// 包含するプロパティ群 (x, y, width, height, visible, font, string 等)
    pub traits: HashMap<String, TraitValue>,
    /// 子要素ノード群
    pub children: Vec<MenuNode>,
}

impl MenuNode {
    /// 新しい要素ノードを生成する。
    pub fn new(name: &str, node_type: NodeType) -> Self {
        Self {
            name: name.to_string(),
            node_type,
            traits: HashMap::new(),
            children: Vec::new(),
        }
    }

    /// 数値プロパティを設定する。
    pub fn set_num(&mut self, trait_name: &str, val: f32) {
        self.traits.insert(trait_name.to_lowercase(), TraitValue::Number(val));
    }

    /// 文字列プロパティを設定する。
    pub fn set_str(&mut self, trait_name: &str, val: &str) {
        self.traits.insert(trait_name.to_lowercase(), TraitValue::String(val.to_string()));
    }

    /// 真偽値プロパティを設定する。
    pub fn set_bool(&mut self, trait_name: &str, val: bool) {
        self.traits.insert(trait_name.to_lowercase(), TraitValue::Bool(val));
    }

    /// 動的計算式を設定する。
    pub fn set_expr(&mut self, trait_name: &str, ops: Vec<ExprOp>) {
        self.traits.insert(trait_name.to_lowercase(), TraitValue::Expression(ops));
    }
}
