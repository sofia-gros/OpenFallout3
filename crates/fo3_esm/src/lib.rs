//! # fo3_esm
//!
//! Fallout 3 (Gamebryo 2.6) 向け ESM / ESP マスター・プラグインパーサー基盤。
//!
//! - `types`: 4文字シグネチャ (`FourCC`)、`FormId`、境界ボックス (`ObjectBounds`)
//! - `header`: 24 バイトレコードヘッダー (`RecordHeader`)、グループヘッダー (`GroupHeader`)
//! - `subrecord`: サブレコード (`Subrecord`)、`XXXX` 巨大サイズ対応パーサー
//! - `records`: 各種レコード実装 (`Tes4Header`, `StatRecord`, `CellRecord`, `RefrRecord`)
//! - `reader`: ESM ファイル走査・zlib 解凍リーダー (`EsmReader`)

pub mod header;
pub mod master;
pub mod query;
pub mod reader;
pub mod records;
pub mod subrecord;
pub mod types;

pub use header::{GroupHeader, RecordHeader, SubrecordHeader};
pub use master::EsmMasterContext;
pub use reader::{BaseObjectInfo, EsmEntry, EsmReader};
pub use records::{
    info_flags, ActiRecord, AlchRecord, AmmoRecord, ArmorRecord, BookRecord, CellLighting,
    CellRecord, ContRecord, DialRecord, DoorRecord, EnableParent, FurnRecord, GlobRecord,
    GmstRecord, GmstValue, HairRecord, IdleRecord, InfoRecord, InventoryItem, KeymRecord,
    LandRecord, LandTextureLayer, LightRecord, LockData, LtexRecord, LvliRecord, LvloEntry,
    MiscRecord, NpcRecord, OtftRecord, PackIdleCollection, PackRecord, QuestObjective, QuestRecord,
    QuestStage, RefrRecord, ScptRecord, ScriptHeader, ScriptLocalVar, ScriptType, StatRecord,
    TargetCondition, TeleportDoor, TermMenuItem, TermRecord, Tes4Header, TextureSetRecord,
    WeapRecord, WorldRecord, LAND_HEIGHT_SCALE, LAND_NUM_VERTS, LAND_REAL_SIZE,
    LAND_VERTS_PER_SIDE,
};
pub use subrecord::Subrecord;
pub use types::{
    FormId, FourCC, ObjectBounds, REC_ACHR, REC_ACRE, REC_DIAL, REC_GLOB, REC_INFO, REC_LTEX,
    REC_LVLI, REC_PACK, REC_REFR, REC_SCPT, REC_TERM, REC_TXST,
};

#[cfg(test)]
mod reader_tests;
