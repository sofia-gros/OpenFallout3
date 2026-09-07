//! # NIF スキン・ボーンブロック定義
//!
//! 参照元:
//! - `references/nifxml/nif.xml:L5067` (`NiSkinData`)
//! - `references/nifxml/nif.xml:L5076` (`NiSkinInstance`)
//! - `references/nifxml/nif.xml:L5093` (`NiSkinPartition`)
//! - `references/nifxml/nif.xml:L2143` (`SkinPartition`)
//! - `references/nifxml/nif.xml:L1907` (`BoneVertData`)
//! - `references/nifxml/nif.xml:L2276` (`BoneData`)

use std::io::{self, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::types::{BoundingSphere, Matrix33, Vector3};

/// スキンデータの頂点とそのボーン影響度。
///
/// 参照元: `references/nifxml/nif.xml:L1907` (`BoneVertData`)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoneVertData {
    /// `NiTriShapeData` 内の頂点インデックス
    pub index: u16,
    /// ボーン影響度（0.0 〜 1.0）
    pub weight: f32,
}

impl BoneVertData {
    /// バイナリストリームから `BoneVertData` を読み込む。
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let index = reader.read_u16::<LittleEndian>()?;
        let weight = reader.read_f32::<LittleEndian>()?;
        Ok(BoneVertData { index, weight })
    }
}

/// スキンデータの各ボーン要素（バインドポーズ変換 + 影響頂点リスト）。
///
/// 参照元: `references/nifxml/nif.xml:L2276` (`BoneData` / `NiSkinData::BoneData`)
#[derive(Clone, Debug, PartialEq)]
pub struct BoneData {
    /// スキン座標系からこのボーン座標系へのバインドポーズ変換（逆バインドポーズ）
    pub skin_transform_translation: Vector3,
    /// スキン座標系からこのボーン座標系へのバインドポーズ回転行列
    pub skin_transform_rotation: Matrix33,
    /// スキン座標系からこのボーン座標系へのバインドポーズスケール
    pub skin_transform_scale: f32,
    /// バウンディングスフィア（AABB を包む球）
    pub bounding_sphere: BoundingSphere,
    /// このボーンに影響される頂点とその重みのリスト
    pub vertex_weights: Vec<BoneVertData>,
}

impl BoneData {
    /// バイナリストリームから `BoneData` を読み込む。
    ///
    /// `has_vertex_weights` が false の場合は頂点重みリストは空になる。
    pub fn read<R: Read>(reader: &mut R, has_vertex_weights: bool) -> io::Result<Self> {
        // NiTransform: translation (Vec3), rotation (Matrix33), scale (f32)
        // 参照元: nif.xml Skin Transform フィールド定義
        let skin_transform_translation = Vector3::read(reader)?;
        let skin_transform_rotation = Matrix33::read(reader)?;
        let skin_transform_scale = reader.read_f32::<LittleEndian>()?;

        // NiBound: center (Vec3) + radius (f32)
        let bounding_sphere = BoundingSphere::read(reader)?;

        let num_vertices = reader.read_u16::<LittleEndian>()? as usize;

        let mut vertex_weights = Vec::with_capacity(num_vertices);
        // バージョン 4.2.2.0 以降かつ has_vertex_weights=true の場合のみ読む
        // 参照元: nif.xml:L2281
        if has_vertex_weights {
            for _ in 0..num_vertices {
                vertex_weights.push(BoneVertData::read(reader)?);
            }
        }

        Ok(BoneData {
            skin_transform_translation,
            skin_transform_rotation,
            skin_transform_scale,
            bounding_sphere,
            vertex_weights,
        })
    }
}

/// スキニングデータブロック。各ボーンのバインドポーズ変換と影響頂点を格納する。
///
/// 参照元: `references/nifxml/nif.xml:L5067` (`NiSkinData`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiSkinData {
    /// ルートスキン座標系のバインドポーズ平行移動
    pub skin_transform_translation: Vector3,
    /// ルートスキン座標系のバインドポーズ回転行列
    pub skin_transform_rotation: Matrix33,
    /// ルートスキン座標系のバインドポーズスケール
    pub skin_transform_scale: f32,
    /// 各ボーンのスキンデータ（`num_bones` 個）
    pub bone_list: Vec<BoneData>,
}

impl NiSkinData {
    /// バイナリストリームから `NiSkinData` を読み込む。
    ///
    /// Fallout 3 (version 20.2.0.7, user_version 11) でのレイアウト:
    /// (NiTransform = translation: Vec3, rotation: Mat3x3, scale: f32)
    /// 1. `NiTransform skin_transform`
    /// 2. `uint num_bones`
    /// 3. `bool has_vertex_weights` (since 4.2.1.0)
    /// 4. `[num_bones] BoneData { ... }`
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let skin_transform_translation = Vector3::read(reader)?;
        let skin_transform_rotation = Matrix33::read(reader)?;
        let skin_transform_scale = reader.read_f32::<LittleEndian>()?;

        let num_bones = reader.read_u32::<LittleEndian>()? as usize;

        // has_vertex_weights (bool として u8 読み取り, since 4.2.1.0)
        // 参照元: nif.xml:L5072
        let has_vertex_weights = reader.read_u8()? != 0;

        let mut bone_list = Vec::with_capacity(num_bones);
        for _ in 0..num_bones {
            bone_list.push(BoneData::read(reader, has_vertex_weights)?);
        }

        Ok(NiSkinData {
            skin_transform_translation,
            skin_transform_rotation,
            skin_transform_scale,
            bone_list,
        })
    }
}


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

/// ディスメンバー（四肢切断）対応スキンパーツ情報。
///
/// 参照元: `references/nifxml/nif.xml:L2591` (`BodyPartList`)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyPartList {
    /// パーツフラグ（257 = デフォルト）
    pub part_flag: u16,
    /// ボディパーツ種別インデックス（0=頭部, 1=髪, ...）
    pub body_part: u16,
}

impl BodyPartList {
    /// バイナリストリームから `BodyPartList` を読み込む。
    ///
    /// 参照元: `references/nifxml/nif.xml:L2591` — size="4" (u16 + u16)
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let part_flag = reader.read_u16::<LittleEndian>()?;
        let body_part = reader.read_u16::<LittleEndian>()?;
        Ok(BodyPartList { part_flag, body_part })
    }
}

/// Bethesda 独自のスキンインスタンス（四肢切断パーツ情報付き）。
///
/// `NiSkinInstance` のサブクラス。スキニングデータへの参照構造は同一。
/// 追加フィールドとして `num_partitions` 個の `BodyPartList` を持つ。
///
/// 参照元: `references/nifxml/nif.xml:L6722` (`BSDismemberSkinInstance`)
#[derive(Clone, Debug, PartialEq)]
pub struct BSDismemberSkinInstance {
    /// 内包する `NiSkinInstance` データ（スキンデータ参照・ボーンリスト等）
    pub skin_instance: NiSkinInstance,
    /// ディスメンバーパーツリスト
    pub partitions: Vec<BodyPartList>,
}

impl BSDismemberSkinInstance {
    /// バイナリストリームから `BSDismemberSkinInstance` を読み込む。
    ///
    /// レイアウト: `NiSkinInstance` フィールド全体 + `uint num_partitions` + `[num_partitions] BodyPartList`
    ///
    /// 参照元: `references/nifxml/nif.xml:L6722` (`BSDismemberSkinInstance`)
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        // 親クラス NiSkinInstance のフィールドを先に読む
        let skin_instance = NiSkinInstance::read(reader)?;

        let num_partitions = reader.read_u32::<LittleEndian>()? as usize;
        let mut partitions = Vec::with_capacity(num_partitions);
        for _ in 0..num_partitions {
            partitions.push(BodyPartList::read(reader)?);
        }

        Ok(BSDismemberSkinInstance { skin_instance, partitions })
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
