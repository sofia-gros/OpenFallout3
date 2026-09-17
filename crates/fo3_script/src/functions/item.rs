//! # アイテム・インベントリ関連スクリプト関数
//! 参照元: references/openmw/components/esm4/script.hpp

use crate::parser::Expr;
use crate::vm::{ScriptVm, ScriptError};
use fo3_esm::FormId;

pub fn execute(
    cmd: &str,
    args: &[Expr],
    subject: Option<FormId>,
    vm: &mut ScriptVm,
) -> Result<Option<f32>, ScriptError> {
    let get_form_id = |expr: &Expr| -> Result<FormId, ScriptError> {
        match expr {
            Expr::Number(n) => Ok(FormId(*n as u32)),
            Expr::Variable(v) => vm.resolve_form_id(v),
            _ => Err(ScriptError::InvalidArguments("Expected FormId".to_string())),
        }
    };

    match cmd {
        // AddItem [ItemID] [Count]
        "additem" => {
            let target = subject.unwrap_or(FormId(0x14));
            if args.len() >= 2 {
                let item_id = get_form_id(&args[0])?;
                let count = vm.eval_ast_expr(&args[1], subject)? as u32;
                if target == FormId(0x14) {
                    vm.add_item(item_id, count);
                }
            }
            Ok(Some(0.0))
        }
        // RemoveItem [ItemID] [Count]
        "removeitem" => {
            let target = subject.unwrap_or(FormId(0x14));
            if args.len() >= 2 {
                let item_id = get_form_id(&args[0])?;
                let count = vm.eval_ast_expr(&args[1], subject)? as u32;
                if target == FormId(0x14) {
                    let current = vm.get_item_count(item_id);
                    if current > count {
                        vm.inventory.insert(item_id, current - count);
                    } else {
                        vm.inventory.remove(&item_id);
                    }
                }
            }
            Ok(Some(0.0))
        }
        // FUN_GetItemCount = 47
        // GetItemCount [ItemID]
        "getitemcount" => {
            let target = subject.unwrap_or(FormId(0x14));
            if !args.is_empty() {
                let item_id = get_form_id(&args[0])?;
                if target == FormId(0x14) {
                    return Ok(Some(vm.get_item_count(item_id) as f32));
                }
            }
            Ok(Some(0.0))
        }
        // FUN_GetGold = 48
        // GetGold (Fallout 3 では 0x0000000F キャップ)
        "getgold" => {
            let target = subject.unwrap_or(FormId(0x14));
            if target == FormId(0x14) {
                let caps_id = FormId(0x0000000F);
                return Ok(Some(vm.get_item_count(caps_id) as f32));
            }
            Ok(Some(0.0))
        }
        // EquipItem [ItemID]
        "equipitem" => {
            if !args.is_empty() {
                let item_id = get_form_id(&args[0])?;
                let target = subject.unwrap_or(FormId(0x14));
                // 将来的にインベントリ/アクター装備マネージャーへ通知
                println!("[Script] EquipItem: Target {:?}, Item {:?}", target, item_id);
            }
            Ok(Some(0.0))
        }
        // UnequipItem [ItemID]
        "unequipitem" => {
            if !args.is_empty() {
                let item_id = get_form_id(&args[0])?;
                let target = subject.unwrap_or(FormId(0x14));
                println!("[Script] UnequipItem: Target {:?}, Item {:?}", target, item_id);
            }
            Ok(Some(0.0))
        }
        // FUN_GetEquipped = 182
        // GetEquipped [ItemID]
        // 参照元: references/openmw/components/esm4/script.hpp:176
        "getequipped" => {
            if !args.is_empty() {
                let item_id = get_form_id(&args[0])?;
                let _target = subject.unwrap_or(FormId(0x14));
                // インベントリに所持している場合は仮で装備中判定または 0
                let count = vm.get_item_count(item_id);
                return Ok(Some(if count > 0 { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // Drop [ItemID] [Count]
        "drop" => {
            let target = subject.unwrap_or(FormId(0x14));
            if args.len() >= 2 {
                let item_id = get_form_id(&args[0])?;
                let count = vm.eval_ast_expr(&args[1], subject)? as u32;
                if target == FormId(0x14) {
                    let current = vm.get_item_count(item_id);
                    if current > count {
                        vm.inventory.insert(item_id, current - count);
                    } else {
                        vm.inventory.remove(&item_id);
                    }
                }
            }
            Ok(Some(0.0))
        }
        // RemoveAllItems
        "removeallitems" => {
            let _target = subject.unwrap_or(FormId(0x14));
            vm.inventory.clear();
            Ok(Some(0.0))
        }
        // AddItemHealthPercent [ItemID] [Count] [HealthPercent]
        "additemhealthpercent" => {
            let target = subject.unwrap_or(FormId(0x14));
            if args.len() >= 3 {
                let item_id = get_form_id(&args[0])?;
                let count = vm.eval_ast_expr(&args[1], subject)? as u32;
                let health_percent = vm.eval_ast_expr(&args[2], subject)?;
                if target == FormId(0x14) {
                    vm.add_item(item_id, count);
                    println!("[Script] AddItemHealthPercent: Target {:?}, Item {:?}, Count {}, HealthPercent {}", target, item_id, count, health_percent);
                }
            }
            Ok(Some(0.0))
        }
        // DropMe
        // 実行中アイテム自身をインベントリからドロップ
        "dropme" => {
            println!("[Script] DropMe executed for subject {:?}", subject);
            Ok(Some(0.0))
        }
        // FUN_GetIsUsedItem = 246
        // GetIsUsedItem [ItemID]
        // 参照元: references/openmw/components/esm4/script.hpp:200
        "getisuseditem" => {
            if !args.is_empty() {
                let _item_id = get_form_id(&args[0])?;
                // フォールバック: 未使用判定 0.0
                return Ok(Some(0.0));
            }
            Ok(Some(0.0))
        }
        // FUN_GetIsUsedItemType = 247
        // GetIsUsedItemType [ItemType]
        // 参照元: references/openmw/components/esm4/script.hpp:201
        "getisuseditemtype" => {
            if !args.is_empty() {
                let _item_type = vm.eval_ast_expr(&args[0], subject)? as u32;
                return Ok(Some(0.0));
            }
            Ok(Some(0.0))
        }
        // FUN_GetUsedItemLevel = 258
        // GetUsedItemLevel
        // 参照元: references/openmw/components/esm4/script.hpp:204
        "getuseditemlevel" => {
            // フォールバック: レベル1.0
            Ok(Some(1.0))
        }
        // FUN_GetUsedItemActivate = 259
        // GetUsedItemActivate
        // 参照元: references/openmw/components/esm4/script.hpp:205
        "getuseditemactivate" => {
            Ok(Some(0.0))
        }
        // FUN_GetIsUsedItemEquipType = 480
        // GetIsUsedItemEquipType [EquipType]
        // 参照元: references/openmw/components/esm4/script.hpp:283
        "getisuseditemequiptype" => {
            if !args.is_empty() {
                let _equip_type = vm.eval_ast_expr(&args[0], subject)? as u32;
                return Ok(Some(0.0));
            }
            Ok(Some(0.0))
        }
        // FUN_GetWeaponHealthPerc = 500
        // GetWeaponHealthPerc
        // 参照元: references/openmw/components/esm4/script.hpp:288
        "getweaponhealthperc" => {
            // 装備中武器の耐久度割合 (100%健全フォールバック)
            Ok(Some(100.0))
        }
        // SetWeaponHealthPerc [Percent]
        "setweaponhealthperc" => {
            if !args.is_empty() {
                let health_perc = vm.eval_ast_expr(&args[0], subject)?;
                let target = subject.unwrap_or(FormId(0x14));
                println!("[Script] SetWeaponHealthPerc: Target {:?}, Perc {}", target, health_perc);
            }
            Ok(Some(0.0))
        }
        // FUN_IsWeaponInList = 399
        // IsWeaponInList [FormListID]
        // 参照元: references/openmw/components/esm4/script.hpp:251
        "isweaponinlist" => {
            if !args.is_empty() {
                let _list_id = get_form_id(&args[0])?;
                return Ok(Some(0.0));
            }
            Ok(Some(0.0))
        }
        // FUN_IsWeaponOut = 101
        // IsWeaponOut
        // 参照元: references/openmw/components/esm4/script.hpp:127
        "isweaponout" => {
            // 武器を構えているか (0.0 = 納刀, 1.0 = 抜刀)
            Ok(Some(0.0))
        }
        // FUN_GetWeaponAnimType = 108
        // GetWeaponAnimType
        // 参照元: references/openmw/components/esm4/script.hpp:132
        "getweaponanimtype" => {
            // 武器アニメーションタイプ (0 = HandToHand, 1 = OneHandMelee 等)
            Ok(Some(0.0))
        }
        // FUN_IsWeaponSkillType = 109
        // IsWeaponSkillType [SkillID]
        // 参照元: references/openmw/components/esm4/script.hpp:133
        "isweaponskilltype" => {
            if !args.is_empty() {
                let _skill_id = vm.eval_ast_expr(&args[0], subject)? as u32;
                return Ok(Some(0.0));
            }
            Ok(Some(0.0))
        }
        // FUN_IsInList = 372
        // IsInList [FormListID]
        // 参照元: references/openmw/components/esm4/script.hpp:245
        "isinlist" => {
            if !args.is_empty() {
                let _list_id = get_form_id(&args[0])?;
                return Ok(Some(0.0));
            }
            Ok(Some(0.0))
        }
        // FUN_GetHasNote = 382
        // GetHasNote [NoteID]
        // 参照元: references/openmw/components/esm4/script.hpp:246
        "gethasnote" => {
            if !args.is_empty() {
                let note_id = get_form_id(&args[0])?;
                let target = subject.unwrap_or(FormId(0x14));
                if target == FormId(0x14) {
                    let count = vm.get_item_count(note_id);
                    return Ok(Some(if count > 0 { 1.0 } else { 0.0 }));
                }
            }
            Ok(Some(0.0))
        }
        // AddNote [NoteID]
        "addnote" => {
            if !args.is_empty() {
                let note_id = get_form_id(&args[0])?;
                let target = subject.unwrap_or(FormId(0x14));
                if target == FormId(0x14) {
                    vm.add_item(note_id, 1);
                }
                println!("[Script] AddNote: Target {:?}, Note {:?}", target, note_id);
            }
            Ok(Some(0.0))
        }
        // RemoveNote [NoteID]
        "removenote" => {
            if !args.is_empty() {
                let note_id = get_form_id(&args[0])?;
                let target = subject.unwrap_or(FormId(0x14));
                if target == FormId(0x14) {
                    vm.inventory.remove(&note_id);
                }
                println!("[Script] RemoveNote: Target {:?}, Note {:?}", target, note_id);
            }
            Ok(Some(0.0))
        }
        // FUN_GetAmountSoldStolen = 190
        // GetAmountSoldStolen
        // 参照元: references/openmw/components/esm4/script.hpp:178
        "getamountsoldstolen" => {
            Ok(Some(0.0))
        }
        // FUN_GetBarterGold = 264
        // GetBarterGold
        // 参照元: references/openmw/components/esm4/script.hpp:206
        "getbartergold" => {
            // 商人の所持キャップ数フォールバック
            Ok(Some(500.0))
        }
        // FUN_GetClothingValue = 41
        // GetClothingValue
        // 参照元: references/openmw/components/esm4/script.hpp:88
        "getclothingvalue" => {
            Ok(Some(10.0))
        }
        // FUN_GetArmorRating = 81
        // GetArmorRating
        // 参照元: references/openmw/components/esm4/script.hpp:122
        "getarmorrating" => {
            Ok(Some(0.0))
        }
        // FUN_GetArmorRatingUpperBody = 274
        // GetArmorRatingUpperBody
        // 参照元: references/openmw/components/esm4/script.hpp:210
        "getarmorratingupperbody" => {
            Ok(Some(0.0))
        }
        // ShowRepairMenu
        "showrepairmenu" => {
            let target = subject.unwrap_or(FormId(0x14));
            println!("[Script] ShowRepairMenu: Target {:?}", target);
            Ok(Some(0.0))
        }
        // ShowBarterMenu
        "showbartermenu" => {
            let target = subject.unwrap_or(FormId(0x14));
            println!("[Script] ShowBarterMenu: Target {:?}", target);
            Ok(Some(0.0))
        }
        // DamageObject [DamageAmount]
        "damageobject" => {
            if !args.is_empty() {
                let damage = vm.eval_ast_expr(&args[0], subject)?;
                println!("[Script] DamageObject: Subject {:?}, Damage {}", subject, damage);
            }
            Ok(Some(0.0))
        }
        // DuplicateAllItems [TargetRef]
        "duplicateallitems" => {
            if !args.is_empty() {
                let dest_id = get_form_id(&args[0])?;
                let src_id = subject.unwrap_or(FormId(0x14));
                println!("[Script] DuplicateAllItems: From {:?} To {:?}", src_id, dest_id);
            }
            Ok(Some(0.0))
        }
        // GetInv
        "getinv" => {
            let target = subject.unwrap_or(FormId(0x14));
            println!("[Script] GetInv: Target {:?}, ItemCount: {}", target, vm.inventory.len());
            for (item_id, count) in &vm.inventory {
                println!("  Item {:08X}: Count {}", item_id.0, count);
            }
            Ok(Some(0.0))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_opcodes() {
        let mut vm = ScriptVm::new();
        let player = FormId(0x14);
        let stimpak = FormId(0x00015169);
        let caps = FormId(0x0000000F);

        // AddItem & GetItemCount
        execute("additem", &[Expr::Number(stimpak.0 as f32), Expr::Number(5.0)], Some(player), &mut vm).unwrap();
        assert_eq!(execute("getitemcount", &[Expr::Number(stimpak.0 as f32)], Some(player), &mut vm).unwrap(), Some(5.0));

        // AddItemHealthPercent
        let pistol = FormId(0x0000434F);
        execute("additemhealthpercent", &[Expr::Number(pistol.0 as f32), Expr::Number(1.0), Expr::Number(85.0)], Some(player), &mut vm).unwrap();
        assert_eq!(execute("getitemcount", &[Expr::Number(pistol.0 as f32)], Some(player), &mut vm).unwrap(), Some(1.0));

        // GetGold
        execute("additem", &[Expr::Number(caps.0 as f32), Expr::Number(150.0)], Some(player), &mut vm).unwrap();
        assert_eq!(execute("getgold", &[], Some(player), &mut vm).unwrap(), Some(150.0));

        // EquipItem & GetEquipped
        execute("equipitem", &[Expr::Number(pistol.0 as f32)], Some(player), &mut vm).unwrap();
        assert_eq!(execute("getequipped", &[Expr::Number(pistol.0 as f32)], Some(player), &mut vm).unwrap(), Some(1.0));

        // RemoveItem
        execute("removeitem", &[Expr::Number(stimpak.0 as f32), Expr::Number(2.0)], Some(player), &mut vm).unwrap();
        assert_eq!(execute("getitemcount", &[Expr::Number(stimpak.0 as f32)], Some(player), &mut vm).unwrap(), Some(3.0));

        // AddNote & GetHasNote & RemoveNote
        let note = FormId(0x0002A123);
        assert_eq!(execute("gethasnote", &[Expr::Number(note.0 as f32)], Some(player), &mut vm).unwrap(), Some(0.0));
        execute("addnote", &[Expr::Number(note.0 as f32)], Some(player), &mut vm).unwrap();
        assert_eq!(execute("gethasnote", &[Expr::Number(note.0 as f32)], Some(player), &mut vm).unwrap(), Some(1.0));
        execute("removenote", &[Expr::Number(note.0 as f32)], Some(player), &mut vm).unwrap();
        assert_eq!(execute("gethasnote", &[Expr::Number(note.0 as f32)], Some(player), &mut vm).unwrap(), Some(0.0));

        // Conditions & Getters
        assert_eq!(execute("getweaponhealthperc", &[], Some(player), &mut vm).unwrap(), Some(100.0));
        assert_eq!(execute("getbartergold", &[], Some(player), &mut vm).unwrap(), Some(500.0));
        assert_eq!(execute("getuseditemlevel", &[], Some(player), &mut vm).unwrap(), Some(1.0));

        // Drop & RemoveAllItems
        execute("drop", &[Expr::Number(stimpak.0 as f32), Expr::Number(1.0)], Some(player), &mut vm).unwrap();
        assert_eq!(execute("getitemcount", &[Expr::Number(stimpak.0 as f32)], Some(player), &mut vm).unwrap(), Some(2.0));
        execute("dropme", &[], Some(stimpak), &mut vm).unwrap();
        execute("removeallitems", &[], Some(player), &mut vm).unwrap();
        assert_eq!(execute("getitemcount", &[Expr::Number(stimpak.0 as f32)], Some(player), &mut vm).unwrap(), Some(0.0));
    }
}

