//! # fo3_script
//!
//! Fallout 3 (Gamebryo 2.6) スクリプト VM & 条件式評価エンジン。
//!
//! - `opcodes`: スクリプト命令セット & 関数テーブル
//! - `conditions`: `CTDA` 会話/スクリプト条件評価エンジン
//! - `vm`: スクリプト仮想マシンカーネル (状態管理、Result Script 実行)

pub mod conditions;
pub mod event;
pub mod opcodes;
pub mod parser;
pub mod quest;
pub mod vm;

pub use conditions::{evaluate_conditions, evaluate_single_condition, ConditionContext};
pub use event::{EventDispatcher, GameEvent, ScriptInstanceContext};
pub use opcodes::{functions, opcodes as opcode_constants};
pub use quest::QuestManager;
pub use vm::{ScriptError, ScriptVm};
