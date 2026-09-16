//! # fo3_script
//!
//! Fallout 3 (Gamebryo 2.6) スクリプト VM & 条件式評価エンジン。
//!
//! - `opcodes`: スクリプト命令セット & 関数テーブル
//! - `conditions`: `CTDA` 会話/スクリプト条件評価エンジン
//! - `vm`: スクリプト仮想マシンカーネル (状態管理、Result Script 実行)

pub mod opcodes;
pub mod conditions;
pub mod vm;
pub mod parser;
pub mod event;
pub mod quest;

pub use opcodes::{functions, opcodes as opcode_constants};
pub use conditions::{evaluate_conditions, evaluate_single_condition, ConditionContext};
pub use vm::{ScriptError, ScriptVm};
pub use event::{EventDispatcher, GameEvent, ScriptInstanceContext};
pub use quest::QuestManager;
