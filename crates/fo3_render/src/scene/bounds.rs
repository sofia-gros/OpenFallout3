use fo3_nif::{NifBlock, NifFile};
use glam::Vec3;

/// 単一 NIF のバウンディング中心と球半径を計算する。
/// 参照元: Gamebryo 2.6 `NiBound`, references/nifxml/nif.xml:L173
pub fn calculate_scene_bounds(nif: &NifFile) -> (Vec3, f32) {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut found = false;

    for block in &nif.blocks {
        match block {
            NifBlock::NiTriShapeData(d) => {
                for v in &d.common.vertices {
                    let p = Vec3::new(v.x, v.y, v.z);
                    min = min.min(p);
                    max = max.max(p);
                    found = true;
                }
            }
            NifBlock::NiTriStripsData(d) => {
                for v in &d.common.vertices {
                    let p = Vec3::new(v.x, v.y, v.z);
                    min = min.min(p);
                    max = max.max(p);
                    found = true;
                }
            }
            _ => {}
        }
    }

    if !found {
        return (Vec3::ZERO, 50.0);
    }

    let center = (min + max) * 0.5;
    let radius = (max - min).length() * 0.5;
    (center, radius.max(10.0))
}

/// 複数パーツ NIF のメッシュ頂点群を包含するバウンディング中心と半径を計算する。
/// 参照元: Gamebryo 2.6 `NiBound::Merge`, references/nifxml/nif.xml:L173
pub fn calculate_scene_bounds_from_parts(parts: &[&NifFile]) -> (Vec3, f32) {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut found = false;

    for nif in parts {
        for block in &nif.blocks {
            match block {
                NifBlock::NiTriShapeData(d) => {
                    for v in &d.common.vertices {
                        let p = Vec3::new(v.x, v.y, v.z);
                        min = min.min(p);
                        max = max.max(p);
                        found = true;
                    }
                }
                NifBlock::NiTriStripsData(d) => {
                    for v in &d.common.vertices {
                        let p = Vec3::new(v.x, v.y, v.z);
                        min = min.min(p);
                        max = max.max(p);
                        found = true;
                    }
                }
                _ => {}
            }
        }
    }

    if !found {
        return (Vec3::ZERO, 50.0);
    }

    let center = (min + max) * 0.5;
    let radius = (max - min).length() * 0.5;
    (center, radius.max(10.0))
}
