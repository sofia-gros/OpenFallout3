//! # NIF ジオメトリ & メッシュデータブロック
//!
//! 参照元:
//! - `references/nifxml/nif.xml:L3853` (`NiGeometry`)
//! - `references/nifxml/nif.xml:L3872` (`NiTriBasedGeom`)
//! - `references/nifxml/nif.xml:L5278` (`NiTriShape`)
//! - `references/nifxml/nif.xml:L3876` (`NiGeometryData`)
//! - `references/nifxml/nif.xml:L5282` (`NiTriShapeData`)
//! - `references/nifxml/nif.xml:L5292` (`NiTriStrips`)
//! - `references/nifxml/nif.xml:L5296` (`NiTriStripsData`)

use std::io::{self, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::blocks::node::NiAVObject;
use crate::types::{BoundingSphere, Color4, TexCoord, Triangle, Vector3};

/// マテリアル関連のメタデータ。
///
/// 参照元: `references/nifxml/nif.xml:L3840` (`MaterialData`)
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialData {
    pub material_names: Vec<u32>,
    pub material_extra_data: Vec<i32>,
    pub active_material: i32,
    pub material_needs_update: bool,
}

impl MaterialData {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let num_materials = reader.read_u32::<LittleEndian>()? as usize;
        let mut material_names = Vec::with_capacity(num_materials);
        for _ in 0..num_materials {
            material_names.push(reader.read_u32::<LittleEndian>()?);
        }

        let mut material_extra_data = Vec::with_capacity(num_materials);
        for _ in 0..num_materials {
            material_extra_data.push(reader.read_i32::<LittleEndian>()?);
        }

        let active_material = reader.read_i32::<LittleEndian>()?;
        let material_needs_update = reader.read_u8()? != 0;

        Ok(MaterialData {
            material_names,
            material_extra_data,
            active_material,
            material_needs_update,
        })
    }
}

/// メッシュやパーティクル等の可視ジオメトリ基底。
///
/// 参照元: `references/nifxml/nif.xml:L3853` (`NiGeometry`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiGeometry {
    pub av: NiAVObject,
    /// ジオメトリデータ（NiTriShapeData / NiTriStripsData）への参照
    pub data: i32,
    /// スキンインスタンス（ボーンスキニング情報）への参照 (-1 は None)
    pub skin_instance: i32,
    /// マテリアルメタデータ
    pub material_data: MaterialData,
}

impl NiGeometry {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let av = NiAVObject::read(reader)?;
        let data = reader.read_i32::<LittleEndian>()?;
        let skin_instance = reader.read_i32::<LittleEndian>()?;
        let material_data = MaterialData::read(reader)?;

        Ok(NiGeometry {
            av,
            data,
            skin_instance,
            material_data,
        })
    }
}

/// 単一インデックス三角形メッシュ。
///
/// 参照元: `references/nifxml/nif.xml:L5278` (`NiTriShape`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiTriShape {
    pub geom: NiGeometry,
}

impl NiTriShape {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let geom = NiGeometry::read(reader)?;
        Ok(NiTriShape { geom })
    }
}

/// トライアングルストリップルメッシュ。
///
/// 参照元: `references/nifxml/nif.xml:L5292` (`NiTriStrips`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiTriStrips {
    pub geom: NiGeometry,
}

impl NiTriStrips {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let geom = NiGeometry::read(reader)?;
        Ok(NiTriStrips { geom })
    }
}

/// 頂点データ基底。
///
/// 参照元: `references/nifxml/nif.xml:L3876` (`NiGeometryData`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiGeometryDataCommon {
    pub num_vertices: u16,
    pub vertices: Vec<Vector3>,
    pub bs_data_flags: u16,
    pub normals: Vec<Vector3>,
    pub tangents: Vec<Vector3>,
    pub bitangents: Vec<Vector3>,
    pub bounding_sphere: BoundingSphere,
    pub vertex_colors: Vec<Color4>,
    pub uv_sets: Vec<Vec<TexCoord>>,
}

impl NiGeometryDataCommon {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let _group_id = reader.read_i32::<LittleEndian>()?;
        let num_vertices = reader.read_u16::<LittleEndian>()?;
        let _keep_flags = reader.read_u8()?;
        let _compress_flags = reader.read_u8()?;
        let has_vertices = reader.read_u8()? != 0;

        let mut vertices = Vec::new();
        if has_vertices {
            vertices.reserve(num_vertices as usize);
            for _ in 0..num_vertices {
                vertices.push(Vector3::read(reader)?);
            }
        }

        let bs_data_flags = reader.read_u16::<LittleEndian>()?;
        let has_normals = reader.read_u8()? != 0;
        let mut normals = Vec::new();
        let mut tangents = Vec::new();
        let mut bitangents = Vec::new();

        if has_normals {
            normals.reserve(num_vertices as usize);
            for _ in 0..num_vertices {
                normals.push(Vector3::read(reader)?);
            }

            // bs_data_flags & 4096 != 0 の場合、Tangents と Bitangents が存在する
            if (bs_data_flags & 4096) != 0 {
                tangents.reserve(num_vertices as usize);
                for _ in 0..num_vertices {
                    tangents.push(Vector3::read(reader)?);
                }

                bitangents.reserve(num_vertices as usize);
                for _ in 0..num_vertices {
                    bitangents.push(Vector3::read(reader)?);
                }
            }
        }

        let bounding_sphere = BoundingSphere::read(reader)?;

        let has_vertex_colors = reader.read_u8()? != 0;
        let mut vertex_colors = Vec::new();
        if has_vertex_colors {
            vertex_colors.reserve(num_vertices as usize);
            for _ in 0..num_vertices {
                vertex_colors.push(Color4::read(reader)?);
            }
        }

        // UVセット数: bs_data_flags & 1 (Fallout 3 では通常 1)
        let num_uv_sets = (bs_data_flags & 1) as usize;
        let mut uv_sets = Vec::with_capacity(num_uv_sets);
        for _ in 0..num_uv_sets {
            let mut uvs = Vec::with_capacity(num_vertices as usize);
            for _ in 0..num_vertices {
                uvs.push(TexCoord::read(reader)?);
            }
            uv_sets.push(uvs);
        }

        let _consistency_flags = reader.read_u16::<LittleEndian>()?;
        let _additional_data = reader.read_i32::<LittleEndian>()?;

        Ok(NiGeometryDataCommon {
            num_vertices,
            vertices,
            bs_data_flags,
            normals,
            tangents,
            bitangents,
            bounding_sphere,
            vertex_colors,
            uv_sets,
        })
    }
}

/// 単一インデックス三角形メッシュの頂点・ポリゴンデータ。
///
/// 参照元: `references/nifxml/nif.xml:L5282` (`NiTriShapeData`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiTriShapeData {
    pub common: NiGeometryDataCommon,
    pub num_triangles: u16,
    pub triangles: Vec<Triangle>,
    pub match_groups: Vec<Vec<u16>>,
}

impl NiTriShapeData {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let common = NiGeometryDataCommon::read(reader)?;
        let num_triangles = reader.read_u16::<LittleEndian>()?;

        let _num_triangle_points = reader.read_u32::<LittleEndian>()?;
        let has_triangles = reader.read_u8()? != 0;

        let mut triangles = Vec::new();
        if has_triangles {
            triangles.reserve(num_triangles as usize);
            for _ in 0..num_triangles {
                triangles.push(Triangle::read(reader)?);
            }
        }

        let num_match_groups = reader.read_u16::<LittleEndian>()? as usize;
        let mut match_groups = Vec::with_capacity(num_match_groups);
        for _ in 0..num_match_groups {
            let count = reader.read_u16::<LittleEndian>()? as usize;
            let mut group = Vec::with_capacity(count);
            for _ in 0..count {
                group.push(reader.read_u16::<LittleEndian>()?);
            }
            match_groups.push(group);
        }

        Ok(NiTriShapeData {
            common,
            num_triangles,
            triangles,
            match_groups,
        })
    }
}

/// トライアングルストリップルメッシュの頂点・ポリゴンデータ。
///
/// 参照元: `references/nifxml/nif.xml:L5296` (`NiTriStripsData`), `references/openmw/components/nif/data.cpp:L188-202`
#[derive(Clone, Debug, PartialEq)]
pub struct NiTriStripsData {
    pub common: NiGeometryDataCommon,
    pub num_triangles: u16,
    pub num_strips: u16,
    pub strip_lengths: Vec<u16>,
    pub strips: Vec<Vec<u16>>,
}

impl NiTriStripsData {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let common = NiGeometryDataCommon::read(reader)?;
        let num_triangles = reader.read_u16::<LittleEndian>()?;
        let num_strips = reader.read_u16::<LittleEndian>()?;
        let mut strip_lengths = Vec::with_capacity(num_strips as usize);
        for _ in 0..num_strips {
            strip_lengths.push(reader.read_u16::<LittleEndian>()?);
        }

        let has_points = reader.read_u8()? != 0;
        let mut strips = Vec::with_capacity(num_strips as usize);
        if has_points {
            for &len in &strip_lengths {
                let mut strip = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    strip.push(reader.read_u16::<LittleEndian>()?);
                }
                strips.push(strip);
            }
        }

        Ok(NiTriStripsData {
            common,
            num_triangles,
            num_strips,
            strip_lengths,
            strips,
        })
    }
}
