//! # プレイヤーアクター & ロコモーションステートマシン
//!
//! Fallout 3 (Gamebryo 2.6) の実機仕様に準拠したプレイヤーアクター。
//! 三人称全身モデルおよび一人称腕モデルのパーツ合成、実機 KF 駆動ロコモーションアニメーション、
//! 視点モードに応じたカリングを制御する。
//!
//! 参照元:
//! - `Fallout3.esm`: `0x00000007` (`PlayerRef`), `0x00000014` (`Player`)
//! - 実機アーカイブ: `Fallout - Meshes.bsa` (`characters\_male\locomotion\*.kf`, `characters\_1stperson\skeleton.nif`)
//! - Gamebryo 2.6 `NiControllerSequence` & マルチパーツアクター仕様

mod actor;
mod locomotion;

#[cfg(test)]
mod tests;

pub use actor::{build_player_actor, PlayerActor};
pub use locomotion::{LocomotionState, LocomotionStateMachine};
