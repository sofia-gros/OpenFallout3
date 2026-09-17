//! # トランスフォーム・位置・セル・空間関連スクリプト関数
//! 参照元: references/openmw/components/esm4/script.hpp:6, 8, 10, 11, 32, 67, 310

use crate::parser::Expr;
use crate::vm::{ScriptVm, ScriptError};
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
        // FUN_GetPos = 6
        // GetPos [X/Y/Z]
        "getpos" => {
            if !args.is_empty() {
                let axis = match &args[0] {
                    Expr::Variable(v) => v.to_ascii_lowercase(),
                    _ => "x".to_string(),
                };
                let key = format!("{:08X}.pos.{}", target.0, axis);
                let pos = vm.locals.get(&key).copied().unwrap_or(0.0);
                return Ok(Some(pos));
            }
            Ok(Some(0.0))
        }
        // SetPos [X/Y/Z] [Value]
        "setpos" => {
            if args.len() >= 2 {
                let axis = match &args[0] {
                    Expr::Variable(v) => v.to_ascii_lowercase(),
                    _ => "x".to_string(),
                };
                let val = vm.eval_ast_expr(&args[1], subject)?;
                let key = format!("{:08X}.pos.{}", target.0, axis);
                vm.locals.insert(key, val);
            }
            Ok(Some(0.0))
        }
        // FUN_GetAngle = 8
        // GetAngle [X/Y/Z]
        "getangle" => {
            if !args.is_empty() {
                let axis = match &args[0] {
                    Expr::Variable(v) => v.to_ascii_lowercase(),
                    _ => "z".to_string(),
                };
                let key = format!("{:08X}.angle.{}", target.0, axis);
                let angle = vm.locals.get(&key).copied().unwrap_or(0.0);
                return Ok(Some(angle));
            }
            Ok(Some(0.0))
        }
        // SetAngle [X/Y/Z] [Value]
        "setangle" => {
            if args.len() >= 2 {
                let axis = match &args[0] {
                    Expr::Variable(v) => v.to_ascii_lowercase(),
                    _ => "z".to_string(),
                };
                let val = vm.eval_ast_expr(&args[1], subject)?;
                let key = format!("{:08X}.angle.{}", target.0, axis);
                vm.locals.insert(key, val);
            }
            Ok(Some(0.0))
        }
        // FUN_GetStartingPos = 10
        "getstartingpos" => {
            if !args.is_empty() {
                let axis = match &args[0] {
                    Expr::Variable(v) => v.to_ascii_lowercase(),
                    _ => "x".to_string(),
                };
                let key = format!("{:08X}.startpos.{}", target.0, axis);
                let pos = vm.locals.get(&key).copied().unwrap_or(0.0);
                return Ok(Some(pos));
            }
            Ok(Some(0.0))
        }
        // FUN_GetStartingAngle = 11
        "getstartingangle" => {
            if !args.is_empty() {
                let axis = match &args[0] {
                    Expr::Variable(v) => v.to_ascii_lowercase(),
                    _ => "z".to_string(),
                };
                let key = format!("{:08X}.startangle.{}", target.0, axis);
                let angle = vm.locals.get(&key).copied().unwrap_or(0.0);
                return Ok(Some(angle));
            }
            Ok(Some(0.0))
        }
        // FUN_GetInCell = 67
        // GetInCell [CellFormID/EDID]
        "getincell" => {
            if !args.is_empty() {
                let cell_id = get_form_id(&args[0])?;
                let key = format!("{:08X}.cell", target.0);
                let cur_cell = vm.locals.get(&key).copied().map(|c| FormId(c as u32));
                return Ok(Some(if cur_cell == Some(cell_id) { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // FUN_GetIsInSameCell = 32
        // GetIsInSameCell [TargetRef]
        "getisinsamecell" => {
            if !args.is_empty() {
                let other_ref = get_form_id(&args[0])?;
                let key1 = format!("{:08X}.cell", target.0);
                let key2 = format!("{:08X}.cell", other_ref.0);
                let cell1 = vm.locals.get(&key1).copied().unwrap_or(0.0);
                let cell2 = vm.locals.get(&key2).copied().unwrap_or(0.0);
                return Ok(Some(if cell1 == cell2 { 1.0 } else { 0.0 }));
            }
            Ok(Some(1.0))
        }
        // FUN_GetInWorldspace = 310
        // GetInWorldspace [WorldspaceID]
        "getinworldspace" => {
            if !args.is_empty() {
                let ws_id = get_form_id(&args[0])?;
                let key = format!("{:08X}.worldspace", target.0);
                let cur_ws = vm.locals.get(&key).copied().map(|w| FormId(w as u32));
                return Ok(Some(if cur_ws == Some(ws_id) { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // PositionCell [X] [Y] [Z] [AngleZ] [CellID]
        "positioncell" => {
            if args.len() >= 5 {
                let x = vm.eval_ast_expr(&args[0], subject)?;
                let y = vm.eval_ast_expr(&args[1], subject)?;
                let z = vm.eval_ast_expr(&args[2], subject)?;
                let az = vm.eval_ast_expr(&args[3], subject)?;
                let cell_id = get_form_id(&args[4])?;
                vm.locals.insert(format!("{:08X}.pos.x", target.0), x);
                vm.locals.insert(format!("{:08X}.pos.y", target.0), y);
                vm.locals.insert(format!("{:08X}.pos.z", target.0), z);
                vm.locals.insert(format!("{:08X}.angle.z", target.0), az);
                vm.locals.insert(format!("{:08X}.cell", target.0), cell_id.0 as f32);
                println!("[Script] PositionCell: Target {:?} -> ({}, {}, {}) Cell {:?}", target, x, y, z, cell_id);
            }
            Ok(Some(0.0))
        }
        // MoveTo [TargetRef] [OffsetX] [OffsetY] [OffsetZ]
        // 参照元: GECK: MoveTo
        "moveto" => {
            if !args.is_empty() {
                let target_ref = get_form_id(&args[0])?;
                let offset_x = if args.len() > 1 { vm.eval_ast_expr(&args[1], subject)? } else { 0.0 };
                let offset_y = if args.len() > 2 { vm.eval_ast_expr(&args[2], subject)? } else { 0.0 };
                let offset_z = if args.len() > 3 { vm.eval_ast_expr(&args[3], subject)? } else { 0.0 };

                let px = vm.locals.get(&format!("{:08X}.pos.x", target_ref.0)).copied().unwrap_or(0.0) + offset_x;
                let py = vm.locals.get(&format!("{:08X}.pos.y", target_ref.0)).copied().unwrap_or(0.0) + offset_y;
                let pz = vm.locals.get(&format!("{:08X}.pos.z", target_ref.0)).copied().unwrap_or(0.0) + offset_z;

                vm.locals.insert(format!("{:08X}.pos.x", target.0), px);
                vm.locals.insert(format!("{:08X}.pos.y", target.0), py);
                vm.locals.insert(format!("{:08X}.pos.z", target.0), pz);

                if let Some(&cell) = vm.locals.get(&format!("{:08X}.cell", target_ref.0)) {
                    vm.locals.insert(format!("{:08X}.cell", target.0), cell);
                }
                if let Some(&ws) = vm.locals.get(&format!("{:08X}.worldspace", target_ref.0)) {
                    vm.locals.insert(format!("{:08X}.worldspace", target.0), ws);
                }

                println!(
                    "[Script] MoveTo: Target {:?} moved to {:?} at ({}, {}, {})",
                    target, target_ref, px, py, pz
                );
            }
            Ok(Some(0.0))
        }
        // MoveToMarker [MarkerRef]
        // 参照元: GECK: MoveToMarker
        "movetomarker" => {
            if !args.is_empty() {
                let marker_ref = get_form_id(&args[0])?;
                let px = vm.locals.get(&format!("{:08X}.pos.x", marker_ref.0)).copied().unwrap_or(0.0);
                let py = vm.locals.get(&format!("{:08X}.pos.y", marker_ref.0)).copied().unwrap_or(0.0);
                let pz = vm.locals.get(&format!("{:08X}.pos.z", marker_ref.0)).copied().unwrap_or(0.0);

                vm.locals.insert(format!("{:08X}.pos.x", target.0), px);
                vm.locals.insert(format!("{:08X}.pos.y", target.0), py);
                vm.locals.insert(format!("{:08X}.pos.z", target.0), pz);

                if let Some(&cell) = vm.locals.get(&format!("{:08X}.cell", marker_ref.0)) {
                    vm.locals.insert(format!("{:08X}.cell", target.0), cell);
                }
                if let Some(&ws) = vm.locals.get(&format!("{:08X}.worldspace", marker_ref.0)) {
                    vm.locals.insert(format!("{:08X}.worldspace", target.0), ws);
                }

                println!("[Script] MoveToMarker: Target {:?} -> Marker {:?}", target, marker_ref);
            }
            Ok(Some(0.0))
        }
        // FUN_GetDistance = 1
        // GetDistance [TargetRef]
        // 参照元: references/openmw/components/esm4/script.hpp:70 (FUN_GetDistance = 1)
        "getdistance" => {
            if !args.is_empty() {
                let other_ref = get_form_id(&args[0])?;
                let x1 = vm.locals.get(&format!("{:08X}.pos.x", target.0)).copied().unwrap_or(0.0);
                let y1 = vm.locals.get(&format!("{:08X}.pos.y", target.0)).copied().unwrap_or(0.0);
                let z1 = vm.locals.get(&format!("{:08X}.pos.z", target.0)).copied().unwrap_or(0.0);

                let x2 = vm.locals.get(&format!("{:08X}.pos.x", other_ref.0)).copied().unwrap_or(0.0);
                let y2 = vm.locals.get(&format!("{:08X}.pos.y", other_ref.0)).copied().unwrap_or(0.0);
                let z2 = vm.locals.get(&format!("{:08X}.pos.z", other_ref.0)).copied().unwrap_or(0.0);

                let dx = x1 - x2;
                let dy = y1 - y2;
                let dz = z1 - z2;
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                return Ok(Some(dist));
            }
            Ok(Some(0.0))
        }
        // FUN_GetHeadingAngle = 99
        // GetHeadingAngle [TargetRef]
        // 参照元: references/openmw/components/esm4/script.hpp:126 (FUN_GetHeadingAngle = 99)
        "getheadingangle" => {
            if !args.is_empty() {
                let other_ref = get_form_id(&args[0])?;
                let x1 = vm.locals.get(&format!("{:08X}.pos.x", target.0)).copied().unwrap_or(0.0);
                let y1 = vm.locals.get(&format!("{:08X}.pos.y", target.0)).copied().unwrap_or(0.0);

                let x2 = vm.locals.get(&format!("{:08X}.pos.x", other_ref.0)).copied().unwrap_or(0.0);
                let y2 = vm.locals.get(&format!("{:08X}.pos.y", other_ref.0)).copied().unwrap_or(0.0);

                let dx = x2 - x1;
                let dy = y2 - y1;
                let target_angle_deg = dx.atan2(dy).to_degrees();

                let my_angle_deg = vm.locals.get(&format!("{:08X}.angle.z", target.0)).copied().unwrap_or(0.0);
                let mut diff = target_angle_deg - my_angle_deg;
                while diff > 180.0 {
                    diff -= 360.0;
                }
                while diff < -180.0 {
                    diff += 360.0;
                }
                return Ok(Some(diff));
            }
            Ok(Some(0.0))
        }
        // Rotate [Axis] [Speed]
        // 参照元: GECK: Rotate
        "rotate" => {
            if args.len() >= 2 {
                let axis = match &args[0] {
                    Expr::Variable(v) => v.to_ascii_lowercase(),
                    _ => "z".to_string(),
                };
                let speed = vm.eval_ast_expr(&args[1], subject)?;
                vm.locals.insert(format!("{:08X}.rotatespeed.{}", target.0, axis), speed);
                println!("[Script] Rotate: Target {:?}, Axis {}, Speed {}", target, axis, speed);
            }
            Ok(Some(0.0))
        }
        // FastTravel [MarkerRef]
        // 参照元: GECK: FastTravel
        "fasttravel" => {
            if !args.is_empty() {
                let marker_ref = get_form_id(&args[0])?;
                let player = FormId(0x14);
                let px = vm.locals.get(&format!("{:08X}.pos.x", marker_ref.0)).copied().unwrap_or(0.0);
                let py = vm.locals.get(&format!("{:08X}.pos.y", marker_ref.0)).copied().unwrap_or(0.0);
                let pz = vm.locals.get(&format!("{:08X}.pos.z", marker_ref.0)).copied().unwrap_or(0.0);

                vm.locals.insert(format!("{:08X}.pos.x", player.0), px);
                vm.locals.insert(format!("{:08X}.pos.y", player.0), py);
                vm.locals.insert(format!("{:08X}.pos.z", player.0), pz);

                if let Some(&cell) = vm.locals.get(&format!("{:08X}.cell", marker_ref.0)) {
                    vm.locals.insert(format!("{:08X}.cell", player.0), cell);
                }
                println!("[Script] FastTravel to Marker {:?}", marker_ref);
            }
            Ok(Some(0.0))
        }
        // FUN_GetInCellParam = 230
        // GetInCellParam [CellID] [TargetRef]
        // 参照元: references/openmw/components/esm4/script.hpp:195 (FUN_GetInCellParam = 230)
        "getincellparam" => {
            if args.len() >= 2 {
                let cell_id = get_form_id(&args[0])?;
                let other_ref = get_form_id(&args[1])?;
                let key = format!("{:08X}.cell", other_ref.0);
                let cur_cell = vm.locals.get(&key).copied().map(|c| FormId(c as u32));
                return Ok(Some(if cur_cell == Some(cell_id) { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // FUN_GetInZone = 446
        // GetInZone [ZoneID]
        // 参照元: references/openmw/components/esm4/script.hpp:270 (FUN_GetInZone = 446)
        "getinzone" => {
            if !args.is_empty() {
                let zone_id = get_form_id(&args[0])?;
                let key = format!("{:08X}.zone", target.0);
                let cur_zone = vm.locals.get(&key).copied().map(|z| FormId(z as u32));
                return Ok(Some(if cur_zone == Some(zone_id) { 1.0 } else { 0.0 }));
            }
            Ok(Some(0.0))
        }
        // FUN_IsInInterior = 300
        // 参照元: references/openmw/components/esm4/script.hpp:220 (FUN_IsInInterior = 300)
        "isininterior" => {
            let key = format!("{:08X}.isinterior", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_IsMoving = 25
        // 参照元: references/openmw/components/esm4/script.hpp:80 (FUN_IsMoving = 25)
        "ismoving" => {
            let key = format!("{:08X}.ismoving", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_IsTurning = 26
        // 参照元: references/openmw/components/esm4/script.hpp:81 (FUN_IsTurning = 26)
        "isturning" => {
            let key = format!("{:08X}.isturning", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_IsRunning = 287
        // 参照元: references/openmw/components/esm4/script.hpp:217 (FUN_IsRunning = 287)
        "isrunning" => {
            let key = format!("{:08X}.isrunning", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_IsSneaking = 286
        // 参照元: references/openmw/components/esm4/script.hpp:216 (FUN_IsSneaking = 286)
        "issneaking" => {
            let key = format!("{:08X}.issneaking", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_GetLineOfSight = 27
        // GetLineOfSight [TargetRef]
        // 参照元: references/openmw/components/esm4/script.hpp:82 (FUN_GetLineOfSight = 27)
        "getlineofsight" => {
            if !args.is_empty() {
                let other_ref = get_form_id(&args[0])?;
                let c1 = vm.locals.get(&format!("{:08X}.cell", target.0)).copied().unwrap_or(0.0);
                let c2 = vm.locals.get(&format!("{:08X}.cell", other_ref.0)).copied().unwrap_or(0.0);
                return Ok(Some(if c1 == c2 { 1.0 } else { 0.0 }));
            }
            Ok(Some(1.0))
        }
        // FUN_GetWalkSpeed = 142
        // 参照元: references/openmw/components/esm4/script.hpp:154 (FUN_GetWalkSpeed = 142)
        "getwalkspeed" => {
            let key = format!("{:08X}.walkspeed", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(100.0);
            Ok(Some(val))
        }
        // FUN_IsWaterObject = 304
        // 参照元: references/openmw/components/esm4/script.hpp:221 (FUN_IsWaterObject = 304)
        "iswaterobject" => {
            let key = format!("{:08X}.iswaterobject", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_IsInDangerousWater = 332
        // 参照元: references/openmw/components/esm4/script.hpp:233 (FUN_IsInDangerousWater = 332)
        "isindangerouswater" => {
            let key = format!("{:08X}.isindangerouswater", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_IsPlayerMovingIntoNewSpace = 358
        // 参照元: references/openmw/components/esm4/script.hpp:238 (FUN_IsPlayerMovingIntoNewSpace = 358)
        "isplayermovingintonewspace" => {
            let val = vm.locals.get("player_moving_into_new_space").copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // FUN_GetScale = 24
        // 参照元: references/openmw/components/esm4/script.hpp:79 (FUN_GetScale = 24)
        "getscale" => {
            let key = format!("{:08X}.scale", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(1.0);
            Ok(Some(val))
        }
        // SetScale [Scale]
        // 参照元: GECK: SetScale
        "setscale" => {
            if !args.is_empty() {
                let scale = vm.eval_ast_expr(&args[0], subject)?;
                vm.locals.insert(format!("{:08X}.scale", target.0), scale);
                println!("[Script] SetScale: Target {:?}, Scale {}", target, scale);
            }
            Ok(Some(0.0))
        }
        // ModScale [Delta]
        // 参照元: GECK: ModScale
        "modscale" => {
            if !args.is_empty() {
                let delta = vm.eval_ast_expr(&args[0], subject)?;
                let key = format!("{:08X}.scale", target.0);
                let cur = vm.locals.get(&key).copied().unwrap_or(1.0);
                vm.locals.insert(key, cur + delta);
            }
            Ok(Some(0.0))
        }
        // GetGroundHeight
        // 参照元: GECK: GetGroundHeight
        "getgroundheight" => {
            let key = format!("{:08X}.pos.z", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // Look / LookAt [TargetRef]
        // 参照元: GECK: Look, LookAt
        "look" | "lookat" => {
            if !args.is_empty() {
                let other_ref = get_form_id(&args[0])?;
                vm.locals.insert(format!("{:08X}.lookat", target.0), other_ref.0 as f32);
                println!("[Script] LookAt: Target {:?} looking at {:?}", target, other_ref);
            }
            Ok(Some(0.0))
        }
        // StopLook / StopLookAt
        // 参照元: GECK: StopLook, StopLookAt
        "stoplook" | "stoplookat" => {
            vm.locals.insert(format!("{:08X}.lookat", target.0), 0.0);
            println!("[Script] StopLookAt: Target {:?}", target);
            Ok(Some(0.0))
        }
        // FUN_IsFacingUp = 106
        // 参照元: references/openmw/components/esm4/script.hpp:130 (FUN_IsFacingUp = 106)
        "isfacingup" => {
            let key = format!("{:08X}.isfacingup", target.0);
            let val = vm.locals.get(&key).copied().unwrap_or(0.0);
            Ok(Some(val))
        }
        // ResetFallPosition
        // 参照元: GECK: ResetFallPosition
        "resetfallposition" => {
            println!("[Script] ResetFallPosition: Target {:?}", target);
            Ok(Some(0.0))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_movement_opcodes_extended() {
        let mut vm = ScriptVm::new();
        let npc1 = FormId(0x1001);
        let marker = FormId(0x2001);

        // Position & Angle
        execute("setpos", &[Expr::Variable("x".into()), Expr::Number(100.0)], Some(npc1), &mut vm).unwrap();
        execute("setpos", &[Expr::Variable("y".into()), Expr::Number(200.0)], Some(npc1), &mut vm).unwrap();
        execute("setpos", &[Expr::Variable("z".into()), Expr::Number(50.0)], Some(npc1), &mut vm).unwrap();
        execute("setangle", &[Expr::Variable("z".into()), Expr::Number(90.0)], Some(npc1), &mut vm).unwrap();

        assert_eq!(execute("getpos", &[Expr::Variable("x".into())], Some(npc1), &mut vm).unwrap(), Some(100.0));
        assert_eq!(execute("getangle", &[Expr::Variable("z".into())], Some(npc1), &mut vm).unwrap(), Some(90.0));

        // MoveTo
        execute("setpos", &[Expr::Variable("x".into()), Expr::Number(500.0)], Some(marker), &mut vm).unwrap();
        execute("setpos", &[Expr::Variable("y".into()), Expr::Number(200.0)], Some(marker), &mut vm).unwrap();
        execute("setpos", &[Expr::Variable("z".into()), Expr::Number(50.0)], Some(marker), &mut vm).unwrap();
        execute("moveto", &[Expr::Number(marker.0 as f32)], Some(npc1), &mut vm).unwrap();
        assert_eq!(execute("getpos", &[Expr::Variable("x".into())], Some(npc1), &mut vm).unwrap(), Some(500.0));

        // Distance
        let dist = execute("getdistance", &[Expr::Number(marker.0 as f32)], Some(npc1), &mut vm).unwrap().unwrap();
        assert!(dist < 0.001);

        // Scale
        execute("setscale", &[Expr::Number(1.5)], Some(npc1), &mut vm).unwrap();
        assert_eq!(execute("getscale", &[], Some(npc1), &mut vm).unwrap(), Some(1.5));
        execute("modscale", &[Expr::Number(0.5)], Some(npc1), &mut vm).unwrap();
        assert_eq!(execute("getscale", &[], Some(npc1), &mut vm).unwrap(), Some(2.0));
    }
}
