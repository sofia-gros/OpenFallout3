//! # インタラクション & オブジェクト操作アクション
//!
//! ドア遷移、コンテナ開閉、アイテム拾得、NPC 会話開始、ターミナル起動アクション、
//! テレポート移動要求、AI パッケージ適用、アニメーション再生要求のディスパッチを行う。
//!
//! 参照元:
//! - Gamebryo 2.6 セル遷移 & `references/openmw/components/esm4/loadrefr.cpp:103-127`
//! - `references/openmw/components/esm4/loaddial.hpp`, `loadinfo.hpp`
//! - `menus/dialog/dialog_menu.xml`, `menus/terminal/terminal_menu.xml`

mod door;
mod interact;
mod package;
mod playgroup;
mod teleport;

pub use interact::perform_interact;
pub use package::process_package_requests;
pub use playgroup::process_playgroup_requests;
pub use teleport::process_teleport_requests;

pub(crate) use package::{resolve_idle_kf_from_pack, resolve_pack_for_actor};




