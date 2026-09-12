//! # プレイヤー所持品・インベントリ管理モジュール
//!
//! Fallout 3 (Gamebryo 2.6) のプレイヤー所持品 (`TES4` / `TES3` 準拠の `CNTO` コンテナ/インベントリ構造) を管理する。
//!
//! 参照元:
//! - `references/openmw/components/esm4/loadcnto.hpp`
//! - `references/openmw/components/esm4/loadrefr.cpp:103-127`
//! - `knowledge/gamebryo_resource_management_and_caching.md`

use std::collections::HashMap;
use fo3_esm::FormId;

/// 所持品内の単一アイテムエントリ。
#[derive(Clone, Debug, PartialEq)]
pub struct InventoryEntry {
    /// アイテムのベースオブジェクト FormID
    pub form_id: FormId,
    /// 所持数量
    pub count: u32,
    /// アイテム表示名（HUD 表示・インベントリ一覧用）
    pub name: String,
}

/// プレイヤーのインベントリ。
#[derive(Clone, Debug, Default)]
pub struct PlayerInventory {
    /// FormID をキーとする所持品マップ
    items: HashMap<FormId, InventoryEntry>,
}

#[allow(dead_code)]
impl PlayerInventory {
    /// 新しい空のインベントリを生成する。
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    /// アイテムをインベントリに追加する。
    ///
    /// 既存の同一 FormID アイテムが存在する場合は数量を加算し、存在しない場合は新規エントリを生成する。
    /// 追加後の総数量を返す。
    pub fn add_item(&mut self, form_id: FormId, count: u32, name: &str) -> u32 {
        let entry = self.items.entry(form_id).or_insert_with(|| InventoryEntry {
            form_id,
            count: 0,
            name: name.to_string(),
        });
        entry.count = entry.count.saturating_add(count);
        entry.count
    }

    /// アイテムをインベントリから減算・消費する。
    ///
    /// 数量が 0 に達した場合はエントリを削除する。実際に消費できた数量を返す。
    pub fn remove_item(&mut self, form_id: FormId, count: u32) -> u32 {
        if let Some(entry) = self.items.get_mut(&form_id) {
            if entry.count <= count {
                let actual = entry.count;
                self.items.remove(&form_id);
                actual
            } else {
                entry.count -= count;
                count
            }
        } else {
            0
        }
    }

    /// 指定された FormID のアイテムを所持しているか判定する。
    pub fn has_item(&self, form_id: FormId) -> bool {
        self.items.contains_key(&form_id)
    }

    /// 指定された FormID の所持数量を取得する（所持していない場合は 0）。
    pub fn get_count(&self, form_id: FormId) -> u32 {
        self.items.get(&form_id).map(|e| e.count).unwrap_or(0)
    }

    /// 所持品一覧のイテレータを返す。
    pub fn iter(&self) -> impl Iterator<Item = &InventoryEntry> {
        self.items.values()
    }

    /// 所持品の種類数（エントリ数）を返す。
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// インベントリが空か判定する。
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inventory_add_and_remove() {
        let mut inv = PlayerInventory::new();
        let cap_id = FormId(0x0000000F); // ボトルキャップ

        assert_eq!(inv.add_item(cap_id, 50, "キャップ"), 50);
        assert_eq!(inv.add_item(cap_id, 25, "キャップ"), 75);
        assert_eq!(inv.get_count(cap_id), 75);
        assert!(inv.has_item(cap_id));

        assert_eq!(inv.remove_item(cap_id, 30), 30);
        assert_eq!(inv.get_count(cap_id), 45);

        assert_eq!(inv.remove_item(cap_id, 100), 45);
        assert_eq!(inv.get_count(cap_id), 0);
        assert!(!inv.has_item(cap_id));
    }
}
