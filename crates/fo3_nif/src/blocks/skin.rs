//! # NIF スキン・ボーンブロック定義
//!
//! 参照元:
//! - `references/nifxml/nif.xml:L5076` (`NiSkinInstance`)
//! - `references/nifxml/nif.xml:L5093` (`NiSkinPartition`)
//! - `references/nifxml/nif.xml:L2143` (`SkinPartition`)

use std::io::{self, Read};
use byteorder::{LittleEndian, ReadBytesExt};

/// スキンインスタンス（メッシュとスケルトンボーンの紐付け）。
///
/// 参照元: `references/nifxml/nif.xml:L5076` (`NiSkinInstance`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiSkinInstance {
    /// スキンデータ (`NiSkinData`) への参照 (Ref)
    pub data: i32,
    /// スキンパーティション (`NiSkinPartition`) への参照 (Ref)
    pub skin_partition: i32,
    /// アーマチュア・スケルトンのルートノード (`NiNode`) への参照 (Ptr)
    pub skeleton_root: i32,
    /// 影響を与えるボーンノード (`NiNode`) への参照リスト
    pub bones: Vec<i32>,
}

impl NiSkinInstance {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let data = reader.read_i32::<LittleEndian>()?;
        let skin_partition = reader.read_i32::<LittleEndian>()?;
        let skeleton_root = reader.read_i32::<LittleEndian>()?;
        let num_bones = reader.read_u32::<LittleEndian>()? as usize;

        let mut bones = Vec::with_capacity(num_bones);
        for _ in 0..num_bones {
            bones.push(reader.read_i32::<LittleEndian>()?);
        }

        Ok(NiSkinInstance {
            data,
            skin_partition,
            skeleton_root,
            bones,
        })
    }
}

/// ハードウェアスキニング用サブメッシュ分割パーティション。
///
/// 参照元: `references/nifxml/nif.xml:L2143` (`SkinPartition`)
#[derive(Clone, Debug, PartialEq)]
pub struct SkinPartition {
    pub num_vertices: u16,
    pub num_triangles: u16,
    pub num_bones: u16,
    pub num_strips: u16,
    pub num_weights_per_vertex: u16,
    /// このサブメッシュに影響するボーン（`NiSkinInstance.bones` のインデックス）
    pub bones: Vec<u16>,
    /// 元の `NiTriShapeData` 頂点インデックスへのマップ
    pub vertex_map: Vec<u16>,
    /// 頂点ごとのボーン重み（最大 4 つ、合計 1.0）
    pub vertex_weights: Vec<[f32; 4]>,
    /// 三角形インデックスリスト
    pub triangles: Vec<[u16; 3]>,
    /// 頂点ごとのボーンインデックス（上記 `bones` 配列のインデックス）
    pub bone_indices: Vec<[u8; 4]>,
}

impl SkinPartition {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let num_vertices = reader.read_u16::<LittleEndian>()?;
        let num_triangles = reader.read_u16::<LittleEndian>()?;
        let num_bones = reader.read_u16::<LittleEndian>()?;
        let num_strips = reader.read_u16::<LittleEndian>()?;
        let num_weights_per_vertex = reader.read_u16::<LittleEndian>()?;

        let mut bones = Vec::with_capacity(num_bones as usize);
        for _ in 0..num_bones {
            bones.push(reader.read_u16::<LittleEndian>()?);
        }

        let has_vertex_map = reader.read_u8()? != 0;
        let mut vertex_map = Vec::with_capacity(if has_vertex_map { num_vertices as usize } else { 0 });
        if has_vertex_map {
            for _ in 0..num_vertices {
                vertex_map.push(reader.read_u16::<LittleEndian>()?);
            }
        }

        let has_vertex_weights = reader.read_u8()? != 0;
        let mut vertex_weights = Vec::with_capacity(if has_vertex_weights { num_vertices as usize } else { 0 });
        if has_vertex_weights {
            for _ in 0..num_vertices {
                let mut weights = [0.0f32; 4];
                for w in 0..num_weights_per_vertex as usize {
                    let val = reader.read_f32::<LittleEndian>()?;
                    if w < 4 {
                        weights[w] = val;
                    }
                }
                vertex_weights.push(weights);
            }
        }

        let mut strip_lengths = Vec::with_capacity(num_strips as usize);
        for _ in 0..num_strips {
            strip_lengths.push(reader.read_u16::<LittleEndian>()?);
        }

        let has_faces = reader.read_u8()? != 0;
        let mut triangles = Vec::new();
        if has_faces {
            if num_strips == 0 {
                triangles.reserve(num_triangles as usize);
                for _ in 0..num_triangles {
                    let v0 = reader.read_u16::<LittleEndian>()?;
                    let v1 = reader.read_u16::<LittleEndian>()?;
                    let v2 = reader.read_u16::<LittleEndian>()?;
                    triangles.push([v0, v1, v2]);
                }
            } else {
                // Strips
                for &len in &strip_lengths {
                    for _ in 0..len {
                        let _ = reader.read_u16::<LittleEndian>()?;
                    }
                }
            }
        }

        let has_bone_indices = reader.read_u8()? != 0;
        let mut bone_indices = Vec::with_capacity(if has_bone_indices { num_vertices as usize } else { 0 });
        if has_bone_indices {
            for _ in 0..num_vertices {
                let mut indices = [0u8; 4];
                for i in 0..num_weights_per_vertex as usize {
                    let b = reader.read_u8()?;
                    if i < 4 {
                        indices[i] = b;
                    }
                }
                bone_indices.push(indices);
            }
        }

        Ok(SkinPartition {
            num_vertices,
            num_triangles,
            num_bones,
            num_strips,
            num_weights_per_vertex,
            bones,
            vertex_map,
            vertex_weights,
            triangles,
            bone_indices,
        })
    }
}

/// ハードウェアスキニング用パーティションブロック。
///
/// 参照元: `references/nifxml/nif.xml:L5093` (`NiSkinPartition`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiSkinPartition {
    pub partitions: Vec<SkinPartition>,
}

impl NiSkinPartition {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let num_partitions = reader.read_u32::<LittleEndian>()? as usize;
        let mut partitions = Vec::with_capacity(num_partitions);
        for _ in 0..num_partitions {
            partitions.push(SkinPartition::read(reader)?);
        }

        Ok(NiSkinPartition { partitions })
    }
}
