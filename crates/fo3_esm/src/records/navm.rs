//! # NAVM レコード (NavMesh)
//!
//! NPCの自律移動のための経路探索用NavMeshレコード。
//! Fallout 3 の仕様に従い、複数のサブレコード (NVVX, NVTR 等) から構成されます。
//! 参照元: `references/bevyout/src/vsa/openmw_esm4/navmesh.rs`

use crate::header::RecordHeader;
use crate::subrecord::Subrecord;
use crate::types::{FormId, FourCC, SUB_DATA};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{self, Cursor};

pub const SUB_NVER: FourCC = FourCC(*b"NVER");
pub const SUB_NVVX: FourCC = FourCC(*b"NVVX");
pub const SUB_NVTR: FourCC = FourCC(*b"NVTR");
pub const SUB_NVCA: FourCC = FourCC(*b"NVCA");
pub const SUB_NVDP: FourCC = FourCC(*b"NVDP");
pub const SUB_NVEX: FourCC = FourCC(*b"NVEX");
pub const SUB_NVGD: FourCC = FourCC(*b"NVGD");

/// NavMeshのポリゴン（三角形）定義 (`NVTR`)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavMeshTriangle {
    /// ポリゴンを構成する3つの頂点インデックス (NVVXのインデックス)
    pub vertices: [i16; 3],
    /// 隣接する3つのエッジインデックス (同NavMesh内のポリゴンインデックス、-1なら境界や外部接続)
    pub edges: [i16; 3],
    /// ウォーカビリティフラグ等 (水やドアなど)
    pub flags: u32,
}

impl NavMeshTriangle {
    pub const EDGE0_EXTERNAL: u32 = 0x0000_0001;
    pub const EDGE1_EXTERNAL: u32 = 0x0000_0002;
    pub const EDGE2_EXTERNAL: u32 = 0x0000_0004;
    pub const PREFERRED_PATHING: u32 = 0x0000_0040;
    pub const WATER: u32 = 0x0000_0200;
    pub const CONTAINS_DOOR: u32 = 0x0000_0400;
}

/// NavMesh上のドア情報 (`NVDP`)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavMeshDoor {
    pub door_ref: FormId,
    pub triangle: u16,
}

/// 外部NavMeshとの接続情報 (`NVEX`)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavMeshExternalConnection {
    pub target_navmesh: FormId,
    pub triangle: u16,
}

/// NavMesh グリッドヘッダ (`NVGD`)
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NavMeshGrid {
    pub divisor: u32,
    pub max_x_dist: f32,
    pub max_y_dist: f32,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

/// NavMeshの頂点・ポリゴン・各種接続情報を格納するレコード
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NavMeshRecord {
    pub form_id: FormId,
    pub flags: u32,
    pub version: Option<u32>,
    pub cell_form_id: Option<FormId>,
    /// 頂点座標 (`NVVX`)
    pub vertices: Vec<[f32; 3]>,
    /// 三角形ポリゴン (`NVTR`)
    pub triangles: Vec<NavMeshTriangle>,
    /// カバー用三角形 (`NVCA`)
    pub cover_triangles: Vec<i16>,
    /// ドア接続 (`NVDP`)
    pub doors: Vec<NavMeshDoor>,
    /// 外部接続 (`NVEX`)
    pub external_connections: Vec<NavMeshExternalConnection>,
    /// グリッドバウンディング情報 (`NVGD`)
    pub grid: Option<NavMeshGrid>,
}

impl NavMeshRecord {
    /// サブレコード列から NavMeshRecord を生成する。
    pub fn from_record(header: &RecordHeader, subrecords: &[Subrecord]) -> io::Result<Self> {
        let mut navm = NavMeshRecord {
            form_id: header.form_id,
            flags: header.flags,
            ..Default::default()
        };

        for sub in subrecords {
            let mut cursor = Cursor::new(&sub.data);
            match sub.type_id {
                SUB_NVER => {
                    if sub.data.len() >= 4 {
                        navm.version = Some(cursor.read_u32::<LittleEndian>()?);
                    }
                }
                SUB_DATA => {
                    if sub.data.len() >= 24 {
                        let cell = cursor.read_u32::<LittleEndian>()?;
                        if cell != 0 {
                            navm.cell_form_id = Some(FormId(cell));
                        }
                    }
                }
                SUB_NVVX => {
                    let count = sub.data.len() / 12;
                    for _ in 0..count {
                        navm.vertices.push([
                            cursor.read_f32::<LittleEndian>()?,
                            cursor.read_f32::<LittleEndian>()?,
                            cursor.read_f32::<LittleEndian>()?,
                        ]);
                    }
                }
                SUB_NVTR => {
                    let count = sub.data.len() / 16;
                    for _ in 0..count {
                        navm.triangles.push(NavMeshTriangle {
                            vertices: [
                                cursor.read_i16::<LittleEndian>()?,
                                cursor.read_i16::<LittleEndian>()?,
                                cursor.read_i16::<LittleEndian>()?,
                            ],
                            edges: [
                                cursor.read_i16::<LittleEndian>()?,
                                cursor.read_i16::<LittleEndian>()?,
                                cursor.read_i16::<LittleEndian>()?,
                            ],
                            flags: cursor.read_u32::<LittleEndian>()?,
                        });
                    }
                }
                SUB_NVCA => {
                    let count = sub.data.len() / 2;
                    for _ in 0..count {
                        navm.cover_triangles
                            .push(cursor.read_i16::<LittleEndian>()?);
                    }
                }
                SUB_NVDP => {
                    let count = sub.data.len() / 8;
                    for _ in 0..count {
                        let door_id = cursor.read_u32::<LittleEndian>()?;
                        let triangle = cursor.read_u16::<LittleEndian>()?;
                        let _padding = cursor.read_u16::<LittleEndian>()?;
                        navm.doors.push(NavMeshDoor {
                            door_ref: FormId(door_id),
                            triangle,
                        });
                    }
                }
                SUB_NVEX => {
                    let count = sub.data.len() / 10;
                    for _ in 0..count {
                        let _unknown = cursor.read_u32::<LittleEndian>()?;
                        let target = cursor.read_u32::<LittleEndian>()?;
                        let triangle = cursor.read_u16::<LittleEndian>()?;
                        navm.external_connections.push(NavMeshExternalConnection {
                            target_navmesh: FormId(target),
                            triangle,
                        });
                    }
                }
                SUB_NVGD => {
                    if sub.data.len() >= 36 {
                        navm.grid = Some(NavMeshGrid {
                            divisor: cursor.read_u32::<LittleEndian>()?,
                            max_x_dist: cursor.read_f32::<LittleEndian>()?,
                            max_y_dist: cursor.read_f32::<LittleEndian>()?,
                            min: [
                                cursor.read_f32::<LittleEndian>()?,
                                cursor.read_f32::<LittleEndian>()?,
                                cursor.read_f32::<LittleEndian>()?,
                            ],
                            max: [
                                cursor.read_f32::<LittleEndian>()?,
                                cursor.read_f32::<LittleEndian>()?,
                                cursor.read_f32::<LittleEndian>()?,
                            ],
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(navm)
    }
}
