//! # 魔法・Perk・スペル・効果関連スクリプト関数
//! 参照元: references/openmw/components/esm4/script.hpp:214, 223, 449

use crate::parser::Expr;
use crate::vm::{ScriptError, ScriptVm};
use fo3_esm::FormId;

pub fn execute(
    cmd: &str,
    args: &[Expr],
    subject: Option<FormId>,
    vm: &mut ScriptVm,
) -> Result<Option<f32>, ScriptError> {
    let target = subject.unwrap_or(FormId(0x14));

    let get_form_id = |expr: &Expr| -> Result<FormId, ScriptError> {
        match expr {
            Expr::Number(n) => Ok(FormId(*n as u32)),
            Expr::Variable(v) => vm.resolve_form_id(v),
            _ => Err(ScriptError::InvalidArguments("Expected FormId".to_string())),
        }
    };

    match cmd {
        // FUN_HasPerk = 449
        // HasPerk [PerkFormID]
        "hasperk" => {
            if !args.is_empty() {
                let perk_id = get_form_id(&args[0])?;
                let key = format!("{:08X}.perk.{:08X}", target.0, perk_id.0);
                let has = vm.locals.get(&key).copied().unwrap_or(0.0) != 0.0;
                return Ok(Some(if has { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // AddPerk [PerkFormID]
        "addperk" => {
            if !args.is_empty() {
                let perk_id = get_form_id(&args[0])?;
                let key = format!("{:08X}.perk.{:08X}", target.0, perk_id.0);
                vm.locals.insert(key, 1.0);
                println!("[Script] AddPerk: Target {:?}, Perk {:?}", target, perk_id);
            }
            Ok(Some(0.0))
        }
        // RemovePerk [PerkFormID]
        "removeperk" => {
            if !args.is_empty() {
                let perk_id = get_form_id(&args[0])?;
                let key = format!("{:08X}.perk.{:08X}", target.0, perk_id.0);
                vm.locals.remove(&key);
                println!(
                    "[Script] RemovePerk: Target {:?}, Perk {:?}",
                    target, perk_id
                );
            }
            Ok(Some(0.0))
        }
        // AddSpell [SpellFormID]
        "addspell" => {
            if !args.is_empty() {
                let spell_id = get_form_id(&args[0])?;
                let key = format!("{:08X}.spell.{:08X}", target.0, spell_id.0);
                vm.locals.insert(key, 1.0);
                println!(
                    "[Script] AddSpell: Target {:?}, Spell {:?}",
                    target, spell_id
                );
            }
            Ok(Some(0.0))
        }
        // RemoveSpell [SpellFormID]
        "removespell" => {
            if !args.is_empty() {
                let spell_id = get_form_id(&args[0])?;
                let key = format!("{:08X}.spell.{:08X}", target.0, spell_id.0);
                vm.locals.remove(&key);
                println!(
                    "[Script] RemoveSpell: Target {:?}, Spell {:?}",
                    target, spell_id
                );
            }
            Ok(Some(0.0))
        }
        // FUN_IsSpellTarget = 223
        // IsSpellTarget [SpellFormID]
        "isspelltarget" => {
            if !args.is_empty() {
                let spell_id = get_form_id(&args[0])?;
                let key = format!("{:08X}.spell.{:08X}", target.0, spell_id.0);
                let has = vm.locals.get(&key).copied().unwrap_or(0.0) != 0.0;
                return Ok(Some(if has { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // FUN_HasMagicEffect = 214
        // HasMagicEffect [EffectArchType/Code]
        // 参照元: references/openmw/components/esm4/script.hpp:185
        "hasmagiceffect" => {
            if !args.is_empty() {
                let _effect_id = get_form_id(&args[0])?;
                // フォールバック: 魔法効果未適用 0.0
                return Ok(Some(0.0));
            }
            Ok(Some(0.0))
        }
        // Cast [SpellID] [TargetRef]
        // 参照元: GECK command: Cast
        "cast" => {
            if !args.is_empty() {
                let spell_id = get_form_id(&args[0])?;
                let cast_target = if args.len() >= 2 {
                    get_form_id(&args[1]).ok()
                } else {
                    None
                };
                let caster = subject.unwrap_or(FormId(0x14));
                println!(
                    "[Script] Cast: Caster {:?}, Spell {:?}, Target {:?}",
                    caster, spell_id, cast_target
                );
            }
            Ok(Some(0.0))
        }
        // Dispel [SpellID]
        // 参照元: GECK command: Dispel
        "dispel" => {
            if !args.is_empty() {
                let spell_id = get_form_id(&args[0])?;
                let key = format!("{:08X}.spell.{:08X}", target.0, spell_id.0);
                vm.locals.remove(&key);
                println!("[Script] Dispel: Target {:?}, Spell {:?}", target, spell_id);
            }
            Ok(Some(0.0))
        }
        // DispelAllSpells
        // 参照元: GECK command: DispelAllSpells
        "dispelallspells" | "removespells" => {
            let prefix = format!("{:08X}.spell.", target.0);
            let keys_to_remove: Vec<String> = vm
                .locals
                .keys()
                .filter(|k| k.starts_with(&prefix))
                .cloned()
                .collect();
            for k in keys_to_remove {
                vm.locals.remove(&k);
            }
            println!("[Script] DispelAllSpells: Target {:?}", target);
            Ok(Some(0.0))
        }
        // FUN_GetSpellUsageNum = 555
        // GetSpellUsageNum [SpellID]
        // 参照元: references/openmw/components/esm4/script.hpp:307
        "getspellusage" | "getspellusagenum" => {
            if !args.is_empty() {
                let _spell_id = get_form_id(&args[0])?;
                return Ok(Some(0.0));
            }
            Ok(Some(0.0))
        }
        // FUN_GetRadiationLevel = 503
        // GetRadiationLevel
        // 参照元: references/openmw/components/esm4/script.hpp:289
        "getradiationlevel" => {
            let key = format!("{:08X}.rads", target.0);
            let rads = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(rads))
        }
        // FUN_GetDisease = 39
        // GetDisease
        // 参照元: references/openmw/components/esm4/script.hpp:86
        "getdisease" => {
            let key = format!("{:08X}.disease", target.0);
            let disease = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(disease))
        }
        // FUN_GetVampire = 40
        // GetVampire
        // 参照元: references/openmw/components/esm4/script.hpp:87
        "getvampire" => {
            let key = format!("{:08X}.vampire", target.0);
            let vampire = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(vampire))
        }
        // FUN_IsInCombat = 289
        // IsInCombat
        // 参照元: references/openmw/components/esm4/script.hpp:219
        "isincombat" => {
            let key = format!("{:08X}.in_combat", target.0);
            let in_combat = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(in_combat))
        }
        // FUN_GetIsAlignment = 474
        // GetIsAlignment [AlignmentIndex] (0 = Good, 1 = Neutral, 2 = Evil)
        // 参照元: references/openmw/components/esm4/script.hpp:281
        "getisalignment" => {
            if !args.is_empty() {
                let query_align = vm.eval_ast_expr(&args[0], subject)? as u32;
                let key = format!("{:08X}.alignment", target.0);
                let current_align = vm.locals.get(&key).copied().unwrap_or(1.0) as u32; // デフォルト中立
                return Ok(Some(if query_align == current_align {
                    1.0
                } else {
                    0.0
                }));
            }
            Ok(Some(0.0))
        }
        // FUN_GetConcussed = 489
        // GetConcussed
        // 参照元: references/openmw/components/esm4/script.hpp:284
        "getconcussed" => {
            let key = format!("{:08X}.concussed", target.0);
            let concussed = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(concussed))
        }
        // FUN_IsInCriticalStage = 531
        // IsInCriticalStage
        // 参照元: references/openmw/components/esm4/script.hpp:303
        "isincriticalstage" => {
            let key = format!("{:08X}.critical_stage", target.0);
            let stage = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(stage))
        }
        // FUN_HasLoaded3D = 558
        // HasLoaded3D
        // 参照元: references/openmw/components/esm4/script.hpp:309
        "hasloaded3d" => {
            // エンジン上で3Dモデルがロードされているか (スタブ: 常に 1.0)
            Ok(Some(1.0))
        }
        // ApplyImageSpaceModifier / imod [ImodID]
        // 参照元: GECK command: ApplyImageSpaceModifier
        "applyimagespacemodifier" | "imod" => {
            if !args.is_empty() {
                let imod_name = match &args[0] {
                    Expr::Variable(v) => v.clone(),
                    Expr::Number(n) => format!("{:08X}", *n as u32),
                    _ => "DefaultImod".to_string(),
                };
                vm.active_imods.push(imod_name.clone());
                println!("[Script] ApplyImageSpaceModifier: {:?}", imod_name);
            }
            Ok(Some(0.0))
        }
        // RemoveImageSpaceModifier / rimod [ImodID]
        // 参照元: GECK command: RemoveImageSpaceModifier
        "removeimagespacemodifier" | "rimod" => {
            if !args.is_empty() {
                let imod_name = match &args[0] {
                    Expr::Variable(v) => v.clone(),
                    Expr::Number(n) => format!("{:08X}", *n as u32),
                    _ => "".to_string(),
                };
                vm.active_imods.retain(|m| m != &imod_name);
                println!("[Script] RemoveImageSpaceModifier: {:?}", imod_name);
            }
            Ok(Some(0.0))
        }
        // FUN_HasFlames = 154
        // HasFlames
        // 参照元: references/openmw/components/esm4/script.hpp:164
        "hasflames" => {
            let key = format!("{:08X}.has_flames", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_CanHaveFlames = 153
        // CanHaveFlames
        // 参照元: references/openmw/components/esm4/script.hpp:163
        "canhaveflames" => {
            // アクター/オブジェクトが炎上エフェクトを纏えるか
            Ok(Some(1.0))
        }
        // AddPerkRank [PerkFormID] [Rank]
        "addperkrank" => {
            if !args.is_empty() {
                let perk_id = get_form_id(&args[0])?;
                let rank = if args.len() >= 2 {
                    vm.eval_ast_expr(&args[1], subject)?
                } else {
                    1.0
                };
                let key = format!("{:08X}.perk.{:08X}", target.0, perk_id.0);
                vm.locals.insert(key, rank);
                println!(
                    "[Script] AddPerkRank: Target {:?}, Perk {:?}, Rank {}",
                    target, perk_id, rank
                );
            }
            Ok(Some(0.0))
        }
        // GetPerkRank [PerkFormID]
        "getperkrank" => {
            if !args.is_empty() {
                let perk_id = get_form_id(&args[0])?;
                let key = format!("{:08X}.perk.{:08X}", target.0, perk_id.0);
                let rank = vm.locals.get(&key).copied().unwrap_or(0.0);
                return Ok(Some(rank));
            }
            Ok(Some(0.0))
        }
        // PlayShader [ShaderID]
        "playshader" => {
            if !args.is_empty() {
                let shader_id = get_form_id(&args[0])?;
                println!(
                    "[Script] PlayShader: Target {:?}, Shader {:?}",
                    target, shader_id
                );
            }
            Ok(Some(0.0))
        }
        // RemoveShader [ShaderID]
        "removeshader" => {
            if !args.is_empty() {
                let shader_id = get_form_id(&args[0])?;
                println!(
                    "[Script] RemoveShader: Target {:?}, Shader {:?}",
                    target, shader_id
                );
            }
            Ok(Some(0.0))
        }
        // TriggerHitShader [ShaderID]
        "triggerhitshader" => {
            if !args.is_empty() {
                let shader_id = get_form_id(&args[0])?;
                println!(
                    "[Script] TriggerHitShader: Target {:?}, Shader {:?}",
                    target, shader_id
                );
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
    fn test_magic_opcodes() {
        let mut vm = ScriptVm::new();
        let player = FormId(0x14);
        let spell_id = FormId(0x00012345);
        let perk_id = FormId(0x00094EBF);

        // Spell
        assert_eq!(
            execute(
                "isspelltarget",
                &[Expr::Number(spell_id.0 as f32)],
                Some(player),
                &mut vm
            )
            .unwrap(),
            Some(0.0)
        );
        execute(
            "addspell",
            &[Expr::Number(spell_id.0 as f32)],
            Some(player),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute(
                "isspelltarget",
                &[Expr::Number(spell_id.0 as f32)],
                Some(player),
                &mut vm
            )
            .unwrap(),
            Some(1.0)
        );

        // Cast & Dispel
        execute(
            "cast",
            &[
                Expr::Number(spell_id.0 as f32),
                Expr::Number(player.0 as f32),
            ],
            Some(player),
            &mut vm,
        )
        .unwrap();
        execute(
            "dispel",
            &[Expr::Number(spell_id.0 as f32)],
            Some(player),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute(
                "isspelltarget",
                &[Expr::Number(spell_id.0 as f32)],
                Some(player),
                &mut vm
            )
            .unwrap(),
            Some(0.0)
        );

        // DispelAllSpells
        execute(
            "addspell",
            &[Expr::Number(spell_id.0 as f32)],
            Some(player),
            &mut vm,
        )
        .unwrap();
        execute("dispelallspells", &[], Some(player), &mut vm).unwrap();
        assert_eq!(
            execute(
                "isspelltarget",
                &[Expr::Number(spell_id.0 as f32)],
                Some(player),
                &mut vm
            )
            .unwrap(),
            Some(0.0)
        );

        // Perk & PerkRank
        assert_eq!(
            execute(
                "hasperk",
                &[Expr::Number(perk_id.0 as f32)],
                Some(player),
                &mut vm
            )
            .unwrap(),
            Some(0.0)
        );
        execute(
            "addperk",
            &[Expr::Number(perk_id.0 as f32)],
            Some(player),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute(
                "hasperk",
                &[Expr::Number(perk_id.0 as f32)],
                Some(player),
                &mut vm
            )
            .unwrap(),
            Some(1.0)
        );
        execute(
            "addperkrank",
            &[Expr::Number(perk_id.0 as f32), Expr::Number(3.0)],
            Some(player),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute(
                "getperkrank",
                &[Expr::Number(perk_id.0 as f32)],
                Some(player),
                &mut vm
            )
            .unwrap(),
            Some(3.0)
        );
        execute(
            "removeperk",
            &[Expr::Number(perk_id.0 as f32)],
            Some(player),
            &mut vm,
        )
        .unwrap();
        assert_eq!(
            execute(
                "hasperk",
                &[Expr::Number(perk_id.0 as f32)],
                Some(player),
                &mut vm
            )
            .unwrap(),
            Some(0.0)
        );

        // ImageSpaceModifier (imod, rimod)
        execute(
            "imod",
            &[Expr::Variable("PipboyImod".into())],
            Some(player),
            &mut vm,
        )
        .unwrap();
        assert_eq!(vm.active_imods, vec!["PipboyImod".to_string()]);
        execute(
            "rimod",
            &[Expr::Variable("PipboyImod".into())],
            Some(player),
            &mut vm,
        )
        .unwrap();
        assert!(vm.active_imods.is_empty());

        // Status & Conditions
        assert_eq!(
            execute("hasloaded3d", &[], Some(player), &mut vm).unwrap(),
            Some(1.0)
        );
        assert_eq!(
            execute("canhaveflames", &[], Some(player), &mut vm).unwrap(),
            Some(1.0)
        );
        assert_eq!(
            execute("getradiationlevel", &[], Some(player), &mut vm).unwrap(),
            Some(0.0)
        );
        assert_eq!(
            execute("getdisease", &[], Some(player), &mut vm).unwrap(),
            Some(0.0)
        );
        assert_eq!(
            execute("getvampire", &[], Some(player), &mut vm).unwrap(),
            Some(0.0)
        );
        assert_eq!(
            execute("isincombat", &[], Some(player), &mut vm).unwrap(),
            Some(0.0)
        );
    }
}
