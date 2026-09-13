//! # Bethesda Menu XML UI サブシステム
//!
//! Fallout 3 の宣言的 Menu XML (`dialog_menu.xml`, `hud_main_menu.xml` 等) を
//! 完全パースし、動的 trait 評価、アトラステクスチャマッピング、および画面描画を行う。
//!
//! 参照元:
//! - `menus/dialog/dialog_menu.xml`
//! - `menus/prefabs/top_bracket.xml`, `bottom_bracket.xml`
//! - `textures/interface/interfaceshared.tai`

pub mod ast;
pub mod atlas;
pub mod parser;
pub mod runtime;

pub use ast::{ExprOp, MenuNode, NodeType, TraitSource, TraitValue};
pub use atlas::{AtlasSubTexture, TextureAtlas};
pub use parser::MenuXmlParser;
pub use runtime::{ComputedLayout, MenuRuntime};
