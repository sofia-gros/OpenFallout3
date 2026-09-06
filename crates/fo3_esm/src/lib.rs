//! # fo3_esm
//!
//! Fallout 3 (Gamebryo 2.6) 向け ESM / ESP マスター・プラグインパーサー基盤。
//!
//! - `types`: 4文字シグネチャ (`FourCC`)、`FormId`、境界ボックス (`ObjectBounds`)
//! - `header`: 24 バイトレコードヘッダー (`RecordHeader`)、グループヘッダー (`GroupHeader`)
//! - `subrecord`: サブレコード (`Subrecord`)、`XXXX` 巨大サイズ対応パーサー
//! - `records`: 各種レコード実装 (`Tes4Header`, `StatRecord`, `CellRecord`, `RefrRecord`)
//! - `reader`: ESM ファイル走査・zlib 解凍リーダー (`EsmReader`)

pub mod types;
pub mod header;
pub mod subrecord;
pub mod records;
pub mod reader;

pub use types::{FormId, FourCC, ObjectBounds, REC_LTEX, REC_TXST};
pub use header::{GroupHeader, RecordHeader, SubrecordHeader};
pub use subrecord::Subrecord;
pub use records::{
    CellLighting, CellRecord, LandRecord, LandTextureLayer, LightRecord, LtexRecord, RefrRecord, StatRecord, Tes4Header,
    TextureSetRecord, LAND_HEIGHT_SCALE, LAND_NUM_VERTS, LAND_REAL_SIZE, LAND_VERTS_PER_SIDE,
};
pub use reader::{BaseObjectInfo, EsmEntry, EsmReader};

