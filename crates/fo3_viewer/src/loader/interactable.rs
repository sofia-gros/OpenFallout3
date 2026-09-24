//! セル内配置参照 (REFR) からインタラクト可能オブジェクト (`InteractableObject`) を抽出・判定するモジュール。
//!
//! 参照元: Fallout 3 インタラクション仕様, `references/nifskope/`

use fo3_esm::{BaseObjectInfo, RefrRecord};

use crate::interact::{InteractableKind, InteractableObject};

/// 配置参照とベースオブジェクト情報から、インタラクト可能な 3D オブジェクトを解決する。
pub fn try_resolve_interactable(
    refr: &RefrRecord,
    obj_info: &BaseObjectInfo,
    pos: glam::Vec3,
) -> Option<InteractableObject> {
    let kind_opt = if refr.teleport.is_some() || obj_info.record_type == fo3_esm::types::REC_DOOR {
        Some((
            InteractableKind::Door {
                teleport: refr.teleport.clone(),
                lock: refr.lock,
                is_open: false,
            },
            140.0,
            50.0,
        ))
    } else if obj_info.record_type == fo3_esm::types::REC_CONT {
        Some((
            InteractableKind::Container {
                form_id: refr.base_object.0,
                lock: refr.lock,
                is_open: false,
            },
            60.0,
            45.0,
        ))
    } else if obj_info.record_type == fo3_esm::types::REC_ALCH
        || obj_info.record_type == fo3_esm::types::REC_WEAP
        || obj_info.record_type == fo3_esm::types::REC_ARMO
        || obj_info.record_type == fo3_esm::types::REC_BOOK
        || obj_info.record_type == fo3_esm::types::REC_MISC
    {
        Some((
            InteractableKind::Item {
                form_id: refr.base_object.0,
            },
            20.0,
            35.0,
        ))
    } else if obj_info.record_type == fo3_esm::types::REC_TERM {
        Some((
            InteractableKind::Terminal {
                form_id: refr.base_object.0,
                lock: refr.lock,
            },
            80.0,
            40.0,
        ))
    } else if obj_info.record_type == fo3_esm::types::REC_ACTI {
        Some((
            InteractableKind::Activator {
                form_id: refr.base_object.0,
            },
            100.0,
            45.0,
        ))
    } else {
        None
    };

    let (kind, height, radius) = kind_opt?;
    let name = if !refr.edid.is_empty() {
        refr.edid.clone()
    } else {
        obj_info.edid.clone()
    };

    Some(InteractableObject {
        form_id: refr.form_id.0,
        base_id: refr.base_object.0,
        edid: refr.edid.clone(),
        name,
        position: pos,
        height,
        radius,
        kind,
    })
}
