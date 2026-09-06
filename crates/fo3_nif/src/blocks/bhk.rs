//! # Havok コリジョンブロック定義
//!
//! Bethesda による Gamebryo 2.6 / Havok 物理統合ブロックをパースします。
//!
//! 参照元:
//! - `references/nifxml/nif.xml:L2284` (`HavokFilter`)
//! - `references/nifxml/nif.xml:L2301` (`hkSubPartData`)
//! - `references/nifxml/nif.xml:L2765` (`bhkWorldObject`)
//! - `references/nifxml/nif.xml:L2803` (`bhkEntity`)
//! - `references/nifxml/nif.xml:L2808` (`bhkRigidBodyCInfo550_660`)
//! - `references/nifxml/nif.xml:L2931` (`bhkRigidBody`)
//! - `references/nifxml/nif.xml:L3066` (`bhkSphereShape`)
//! - `references/nifxml/nif.xml:L3079` (`bhkCapsuleShape`)
//! - `references/nifxml/nif.xml:L3088` (`bhkBoxShape`)
//! - `references/nifxml/nif.xml:L3134` (`bhkBvTreeShape`)
//! - `references/nifxml/nif.xml:L3140` (`hkpMoppCode`)
//! - `references/nifxml/nif.xml:L3153` (`bhkMoppBvTreeShape`)
//! - `references/nifxml/nif.xml:L3193` (`bhkPackedNiTriStripsShape`)
//! - `references/nifxml/nif.xml:L3370` (`NiCollisionObject`)
//! - `references/nifxml/nif.xml:L3403` (`bhkNiCollisionObject`)
//! - `references/nifxml/nif.xml:L3420` (`bhkCollisionObject`)
//! - `references/nifxml/nif.xml:L3958` (`hkPackedNiTriStripsData`)

use std::io::{self, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::types::Vector3;

/// IEEE-754 半精度浮動小数点数 (16-bit float) を 単精度 (f32) に変換する。
///
/// 参照元: `references/nifskope/lib/half.cpp:L403`
pub fn half_to_f32(h: u16) -> f32 {
    let s = ((h >> 15) & 0x0001) as u32;
    let e = ((h >> 10) & 0x001f) as u32;
    let m = (h & 0x03ff) as u32;

    if e == 0 {
        if m == 0 {
            f32::from_bits(s << 31)
        } else {
            // 非正規化数 (Subnormal)
            let mut m_shift = m;
            let mut shift_count = 0;
            while (m_shift & 0x0400) == 0 {
                m_shift <<= 1;
                shift_count += 1;
            }
            m_shift &= 0x03ff;
            let exp = (127 - 15 + 1 - shift_count) as u32;
            f32::from_bits((s << 31) | (exp << 23) | (m_shift << 13))
        }
    } else if e == 31 {
        // 無限大または NaN
        f32::from_bits((s << 31) | 0x7f800000 | (m << 13))
    } else {
        // 通常の正規化数
        let exp = (e + (127 - 15)) as u32;
        f32::from_bits((s << 31) | (exp << 23) | (m << 13))
    }
}

/// Havok コリジョンオブジェクト（ルートアタッチ）。
///
/// 参照元: `references/nifxml/nif.xml:L3370, L3403, L3420` (`bhkCollisionObject`)
#[derive(Clone, Debug, PartialEq)]
pub struct BhkCollisionObject {
    /// アタッチされている `NiAVObject` へのブロックインデックス (Ptr)
    pub target: i32,
    /// コリジョンフラグ (`bhkCOFlags`: FO3 デフォルトは 0x0001)
    pub flags: u16,
    /// 剛体 (`bhkWorldObject` / `bhkRigidBody`) へのブロックインデックス (Ref)
    pub body: i32,
}

impl BhkCollisionObject {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let target = reader.read_i32::<LittleEndian>()?;
        let flags = reader.read_u16::<LittleEndian>()?;
        let body = reader.read_i32::<LittleEndian>()?;
        Ok(BhkCollisionObject { target, flags, body })
    }
}

/// Havok ワールドオブジェクト共通基底フィールド。
///
/// 参照元: `references/nifxml/nif.xml:L2765` (`bhkWorldObject`)
#[derive(Clone, Debug, PartialEq)]
pub struct BhkWorldObjectCommon {
    /// 形状オブジェクトへの参照 (Ref)
    pub shape: i32,
    /// コリジョンレイヤー (`Fallout3Layer`)
    pub havok_filter_layer: u8,
    /// フィルターフラグ
    pub havok_filter_flags: u8,
    /// フィルターグループ番号
    pub havok_filter_group: u16,
    /// ブロードフェーズ種別 (通常 1 = Entity)
    pub broad_phase_type: u8,
    /// プロパティデータ
    pub prop_data: u32,
    /// プロパティサイズ
    pub prop_size: u32,
    /// プロパティ容量とフラグ
    pub prop_capacity_and_flags: u32,
}

impl BhkWorldObjectCommon {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let shape = reader.read_i32::<LittleEndian>()?;
        let havok_filter_layer = reader.read_u8()?;
        let havok_filter_flags = reader.read_u8()?;
        let havok_filter_group = reader.read_u16::<LittleEndian>()?;

        let mut unused01 = [0u8; 4];
        reader.read_exact(&mut unused01)?;

        let broad_phase_type = reader.read_u8()?;

        let mut unused02 = [0u8; 3];
        reader.read_exact(&mut unused02)?;

        let prop_data = reader.read_u32::<LittleEndian>()?;
        let prop_size = reader.read_u32::<LittleEndian>()?;
        let prop_capacity_and_flags = reader.read_u32::<LittleEndian>()?;

        Ok(BhkWorldObjectCommon {
            shape,
            havok_filter_layer,
            havok_filter_flags,
            havok_filter_group,
            broad_phase_type,
            prop_data,
            prop_size,
            prop_capacity_and_flags,
        })
    }
}

/// Havok 剛体オブジェクト（Fallout 3: 236 バイト）。
///
/// 参照元:
/// - `references/nifxml/nif.xml:L2803` (`bhkEntity`)
/// - `references/nifxml/nif.xml:L2808` (`bhkRigidBodyCInfo550_660`)
/// - `references/nifxml/nif.xml:L2931` (`bhkRigidBody`)
#[derive(Clone, Debug, PartialEq)]
pub struct BhkRigidBody {
    /// ワールドオブジェクト基底
    pub world_obj: BhkWorldObjectCommon,
    /// 衝突レスポンス種別 (`hkResponseType`)
    pub collision_response: u8,
    /// コンタクトコールバック遅延フレーム
    pub process_contact_callback_delay: u16,

    // --- bhkRigidBodyCInfo550_660 ---
    /// 平行移動ベクトル (X, Y, Z, W)
    pub translation: [f32; 4],
    /// 回転クォータニオン (X, Y, Z, W)
    pub rotation: [f32; 4],
    /// 線形速度
    pub linear_velocity: [f32; 4],
    /// 角速度
    pub angular_velocity: [f32; 4],
    /// 慣性テンソル 4x3 行列 (hkMatrix3: 12 floats)
    pub inertia_tensor: [f32; 12],
    /// 重心
    pub center_of_mass: [f32; 4],
    /// 質量 (kg)。0.0 は不動 (Static)
    pub mass: f32,
    /// 線形減衰
    pub linear_damping: f32,
    /// 角減衰
    pub angular_damping: f32,
    /// 摩擦係数
    pub friction: f32,
    /// 反発係数
    pub restitution: f32,
    /// 最大線形速度
    pub max_linear_velocity: f32,
    /// 最大角速度
    pub max_angular_velocity: f32,
    /// 許容貫通深度
    pub penetration_depth: f32,
    /// モーションシステム種別
    pub motion_system: u8,
    /// スリープ非アクティブ化種別
    pub deactivator_type: u8,
    /// ソルバー非アクティブ化種別
    pub solver_deactivation: u8,
    /// クオリティ種別
    pub quality_type: u8,

    // --- bhkRigidBody 直下フィールド ---
    /// 拘束リストへの参照
    pub constraints: Vec<i32>,
    /// ボディフラグ
    pub body_flags: u32,
}

impl BhkRigidBody {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let world_obj = BhkWorldObjectCommon::read(reader)?;

        // bhkEntity
        let collision_response = reader.read_u8()?;
        let _unused_entity = reader.read_u8()?;
        let process_contact_callback_delay = reader.read_u16::<LittleEndian>()?;

        // bhkRigidBodyCInfo550_660
        let mut unused01 = [0u8; 4];
        reader.read_exact(&mut unused01)?;

        let _cinfo_layer = reader.read_u8()?;
        let _cinfo_flags = reader.read_u8()?;
        let _cinfo_group = reader.read_u16::<LittleEndian>()?;

        let mut unused02 = [0u8; 4];
        reader.read_exact(&mut unused02)?;

        let _cinfo_resp = reader.read_u8()?;
        let _cinfo_unused03 = reader.read_u8()?;
        let _cinfo_delay = reader.read_u16::<LittleEndian>()?;

        let mut unused04 = [0u8; 4];
        reader.read_exact(&mut unused04)?;

        let mut translation = [0.0f32; 4];
        for v in &mut translation {
            *v = reader.read_f32::<LittleEndian>()?;
        }

        let mut rotation = [0.0f32; 4];
        for v in &mut rotation {
            *v = reader.read_f32::<LittleEndian>()?;
        }

        let mut linear_velocity = [0.0f32; 4];
        for v in &mut linear_velocity {
            *v = reader.read_f32::<LittleEndian>()?;
        }

        let mut angular_velocity = [0.0f32; 4];
        for v in &mut angular_velocity {
            *v = reader.read_f32::<LittleEndian>()?;
        }

        let mut inertia_tensor = [0.0f32; 12];
        for v in &mut inertia_tensor {
            *v = reader.read_f32::<LittleEndian>()?;
        }

        let mut center_of_mass = [0.0f32; 4];
        for v in &mut center_of_mass {
            *v = reader.read_f32::<LittleEndian>()?;
        }

        let mass = reader.read_f32::<LittleEndian>()?;
        let linear_damping = reader.read_f32::<LittleEndian>()?;
        let angular_damping = reader.read_f32::<LittleEndian>()?;
        let friction = reader.read_f32::<LittleEndian>()?;
        let restitution = reader.read_f32::<LittleEndian>()?;
        let max_linear_velocity = reader.read_f32::<LittleEndian>()?;
        let max_angular_velocity = reader.read_f32::<LittleEndian>()?;
        let penetration_depth = reader.read_f32::<LittleEndian>()?;

        let motion_system = reader.read_u8()?;
        let deactivator_type = reader.read_u8()?;
        let solver_deactivation = reader.read_u8()?;
        let quality_type = reader.read_u8()?;

        let mut unused05 = [0u8; 12];
        reader.read_exact(&mut unused05)?;

        // 直下フィールド
        let num_constraints = reader.read_u32::<LittleEndian>()? as usize;
        let mut constraints = Vec::with_capacity(num_constraints);
        for _ in 0..num_constraints {
            constraints.push(reader.read_i32::<LittleEndian>()?);
        }

        let body_flags = reader.read_u32::<LittleEndian>()?;

        Ok(BhkRigidBody {
            world_obj,
            collision_response,
            process_contact_callback_delay,
            translation,
            rotation,
            linear_velocity,
            angular_velocity,
            inertia_tensor,
            center_of_mass,
            mass,
            linear_damping,
            angular_damping,
            friction,
            restitution,
            max_linear_velocity,
            max_angular_velocity,
            penetration_depth,
            motion_system,
            deactivator_type,
            solver_deactivation,
            quality_type,
            constraints,
            body_flags,
        })
    }
}

/// MOPP バウンディングボリュームツリー形状ブロック。
///
/// 参照元: `references/nifxml/nif.xml:L3134, L3140, L3153` (`bhkMoppBvTreeShape`)
#[derive(Clone, Debug, PartialEq)]
pub struct BhkMoppBvTreeShape {
    /// 内包される形状 (`bhkPackedNiTriStripsShape`) への参照 (Ref)
    pub shape: i32,
    /// スケール
    pub scale: f32,
    /// MOPP 座標系原点および量子化スケール (X, Y, Z, W)
    pub mopp_offset: [f32; 4],
    /// MOPP コードツリーバイト列
    pub mopp_data: Vec<u8>,
}

impl BhkMoppBvTreeShape {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let shape = reader.read_i32::<LittleEndian>()?;

        let mut unused01 = [0u8; 12];
        reader.read_exact(&mut unused01)?;

        let scale = reader.read_f32::<LittleEndian>()?;

        // hkpMoppCode
        let data_size = reader.read_u32::<LittleEndian>()? as usize;
        let mut mopp_offset = [0.0f32; 4];
        for v in &mut mopp_offset {
            *v = reader.read_f32::<LittleEndian>()?;
        }

        let mut mopp_data = vec![0u8; data_size];
        reader.read_exact(&mut mopp_data)?;

        Ok(BhkMoppBvTreeShape {
            shape,
            scale,
            mopp_offset,
            mopp_data,
        })
    }
}

/// パックドポリゴンストリップ形状ブロック。
///
/// 参照元: `references/nifxml/nif.xml:L3193` (`bhkPackedNiTriStripsShape`)
#[derive(Clone, Debug, PartialEq)]
pub struct BhkPackedNiTriStripsShape {
    /// ユーザーデータ (通常 0)
    pub user_data: u32,
    /// コリジョン球半径 (通常 0.1)
    pub radius: f32,
    /// スケールベクトル (通常 1.0, 1.0, 1.0, 0.0)
    pub scale: [f32; 4],
    /// 半径コピー (通常 0.1)
    pub radius_copy: f32,
    /// スケールコピー
    pub scale_copy: [f32; 4],
    /// データブロック (`hkPackedNiTriStripsData`) への参照 (Ref)
    pub data: i32,
}

impl BhkPackedNiTriStripsShape {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let user_data = reader.read_u32::<LittleEndian>()?;

        let mut unused01 = [0u8; 4];
        reader.read_exact(&mut unused01)?;

        let radius = reader.read_f32::<LittleEndian>()?;

        let mut unused02 = [0u8; 4];
        reader.read_exact(&mut unused02)?;

        let mut scale = [0.0f32; 4];
        for v in &mut scale {
            *v = reader.read_f32::<LittleEndian>()?;
        }

        let radius_copy = reader.read_f32::<LittleEndian>()?;

        let mut scale_copy = [0.0f32; 4];
        for v in &mut scale_copy {
            *v = reader.read_f32::<LittleEndian>()?;
        }

        let data = reader.read_i32::<LittleEndian>()?;

        Ok(BhkPackedNiTriStripsShape {
            user_data,
            radius,
            scale,
            radius_copy,
            scale_copy,
            data,
        })
    }
}

/// パックドコリジョンメッシュのサブパート情報。
///
/// 参照元: `references/nifxml/nif.xml:L2301` (`hkSubPartData`)
#[derive(Clone, Debug, PartialEq)]
pub struct HkSubPartData {
    /// コリジョンレイヤー (`Fallout3Layer`)
    pub havok_filter_layer: u8,
    /// フィルターフラグ
    pub havok_filter_flags: u8,
    /// フィルターグループ
    pub havok_filter_group: u16,
    /// このサブパートに属する頂点数
    pub num_vertices: u32,
    /// ハボックマテリアル (`Fallout3HavokMaterial`)
    pub material: u32,
}

impl HkSubPartData {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let havok_filter_layer = reader.read_u8()?;
        let havok_filter_flags = reader.read_u8()?;
        let havok_filter_group = reader.read_u16::<LittleEndian>()?;
        let num_vertices = reader.read_u32::<LittleEndian>()?;
        let material = reader.read_u32::<LittleEndian>()?;

        Ok(HkSubPartData {
            havok_filter_layer,
            havok_filter_flags,
            havok_filter_group,
            num_vertices,
            material,
        })
    }
}

/// コリジョンメッシュ三角形データ。
///
/// 参照元: `references/nifxml/nif.xml:L2246` (`TriangleData`)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HkTriangleData {
    /// 頂点インデックス 3 つ
    pub triangle: [u16; 3],
    /// トライアングル溶接情報 (`bhkWeldInfo`)
    pub welding_info: u16,
}

impl HkTriangleData {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let v0 = reader.read_u16::<LittleEndian>()?;
        let v1 = reader.read_u16::<LittleEndian>()?;
        let v2 = reader.read_u16::<LittleEndian>()?;
        let welding_info = reader.read_u16::<LittleEndian>()?;
        Ok(HkTriangleData {
            triangle: [v0, v1, v2],
            welding_info,
        })
    }
}

/// パックドポリゴンストリップ実データブロック。
///
/// 参照元: `references/nifxml/nif.xml:L3958` (`hkPackedNiTriStripsData`)
#[derive(Clone, Debug, PartialEq)]
pub struct HkPackedNiTriStripsData {
    /// 三角形リスト
    pub triangles: Vec<HkTriangleData>,
    /// 圧縮フラグ（1 の場合 16bit half-precision で格納されていた）
    pub is_compressed: bool,
    /// 3次元座標頂点リスト (f32 にデコード済み)
    pub vertices: Vec<[f32; 3]>,
    /// サブシェイプ情報
    pub sub_shapes: Vec<HkSubPartData>,
}

impl HkPackedNiTriStripsData {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let num_triangles = reader.read_u32::<LittleEndian>()? as usize;
        let mut triangles = Vec::with_capacity(num_triangles);
        for _ in 0..num_triangles {
            triangles.push(HkTriangleData::read(reader)?);
        }

        let num_vertices = reader.read_u32::<LittleEndian>()? as usize;
        let compressed_byte = reader.read_u8()?;
        let is_compressed = compressed_byte != 0;

        let mut vertices = Vec::with_capacity(num_vertices);
        if !is_compressed {
            for _ in 0..num_vertices {
                let x = reader.read_f32::<LittleEndian>()?;
                let y = reader.read_f32::<LittleEndian>()?;
                let z = reader.read_f32::<LittleEndian>()?;
                vertices.push([x, y, z]);
            }
        } else {
            // 16-bit half-precision 浮動小数点数を展開
            for _ in 0..num_vertices {
                let hx = reader.read_u16::<LittleEndian>()?;
                let hy = reader.read_u16::<LittleEndian>()?;
                let hz = reader.read_u16::<LittleEndian>()?;
                vertices.push([
                    half_to_f32(hx),
                    half_to_f32(hy),
                    half_to_f32(hz),
                ]);
            }
        }

        let num_sub_shapes = reader.read_u16::<LittleEndian>()? as usize;
        let mut sub_shapes = Vec::with_capacity(num_sub_shapes);
        for _ in 0..num_sub_shapes {
            sub_shapes.push(HkSubPartData::read(reader)?);
        }

        Ok(HkPackedNiTriStripsData {
            triangles,
            is_compressed,
            vertices,
            sub_shapes,
        })
    }
}

/// ボックスコリジョン形状。
///
/// 参照元: `references/nifxml/nif.xml:L3088` (`bhkBoxShape`)
#[derive(Clone, Debug, PartialEq)]
pub struct BhkBoxShape {
    /// マテリアル (`Fallout3HavokMaterial`)
    pub material: u32,
    /// シェル半径 (通常 0.05)
    pub radius: f32,
    /// 半径ハーフエクステント (Half Extents: 半分の幅・高さ・奥行き)
    pub dimensions: Vector3,
}

impl BhkBoxShape {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let material = reader.read_u32::<LittleEndian>()?;
        let radius = reader.read_f32::<LittleEndian>()?;

        let mut unused01 = [0u8; 8];
        reader.read_exact(&mut unused01)?;

        let dimensions = Vector3::read(reader)?;
        let _unused_float = reader.read_f32::<LittleEndian>()?;

        Ok(BhkBoxShape {
            material,
            radius,
            dimensions,
        })
    }
}

/// 球コリジョン形状。
///
/// 参照元: `references/nifxml/nif.xml:L3066` (`bhkSphereShape`)
#[derive(Clone, Debug, PartialEq)]
pub struct BhkSphereShape {
    /// マテリアル (`Fallout3HavokMaterial`)
    pub material: u32,
    /// 半径
    pub radius: f32,
}

impl BhkSphereShape {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let material = reader.read_u32::<LittleEndian>()?;
        let radius = reader.read_f32::<LittleEndian>()?;
        Ok(BhkSphereShape { material, radius })
    }
}

/// カプセルコリジョン形状。
///
/// 参照元: `references/nifxml/nif.xml:L3079` (`bhkCapsuleShape`)
#[derive(Clone, Debug, PartialEq)]
pub struct BhkCapsuleShape {
    /// マテリアル (`Fallout3HavokMaterial`)
    pub material: u32,
    /// シェル半径
    pub radius: f32,
    /// 第1の端点座標
    pub first_point: Vector3,
    /// 第1の端点半径
    pub radius1: f32,
    /// 第2の端点座標
    pub second_point: Vector3,
    /// 第2の端点半径
    pub radius2: f32,
}

impl BhkCapsuleShape {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let material = reader.read_u32::<LittleEndian>()?;
        let radius = reader.read_f32::<LittleEndian>()?;

        let mut unused01 = [0u8; 8];
        reader.read_exact(&mut unused01)?;

        let first_point = Vector3::read(reader)?;
        let radius1 = reader.read_f32::<LittleEndian>()?;
        let second_point = Vector3::read(reader)?;
        let radius2 = reader.read_f32::<LittleEndian>()?;

        Ok(BhkCapsuleShape {
            material,
            radius,
            first_point,
            radius1,
            second_point,
            radius2,
        })
    }
}

/// 凸ポリゴン頂点コリジョン形状ブロック。
///
/// 参照元: `references/nifxml/nif.xml:L3096` (`bhkConvexVerticesShape`)
#[derive(Clone, Debug, PartialEq)]
pub struct BhkConvexVerticesShape {
    /// マテリアル (`Fallout3HavokMaterial`)
    pub material: u32,
    /// シェル半径 (通常 0.05)
    pub radius: f32,
    /// 頂点プロパティ
    pub prop_vertices: [u32; 3],
    /// 法線プロパティ
    pub prop_normals: [u32; 3],
    /// 凸ポリゴン頂点リスト (X, Y, Z, W)
    pub vertices: Vec<[f32; 4]>,
    /// 分離平面法線リスト (Nx, Ny, Nz, Distance)
    pub normals: Vec<[f32; 4]>,
}

impl BhkConvexVerticesShape {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let material = reader.read_u32::<LittleEndian>()?;
        let radius = reader.read_f32::<LittleEndian>()?;

        let prop_vertices = [
            reader.read_u32::<LittleEndian>()?,
            reader.read_u32::<LittleEndian>()?,
            reader.read_u32::<LittleEndian>()?,
        ];
        let prop_normals = [
            reader.read_u32::<LittleEndian>()?,
            reader.read_u32::<LittleEndian>()?,
            reader.read_u32::<LittleEndian>()?,
        ];

        let num_vertices = reader.read_u32::<LittleEndian>()? as usize;
        let mut vertices = Vec::with_capacity(num_vertices);
        for _ in 0..num_vertices {
            let x = reader.read_f32::<LittleEndian>()?;
            let y = reader.read_f32::<LittleEndian>()?;
            let z = reader.read_f32::<LittleEndian>()?;
            let w = reader.read_f32::<LittleEndian>()?;
            vertices.push([x, y, z, w]);
        }

        let num_normals = reader.read_u32::<LittleEndian>()? as usize;
        let mut normals = Vec::with_capacity(num_normals);
        for _ in 0..num_normals {
            let x = reader.read_f32::<LittleEndian>()?;
            let y = reader.read_f32::<LittleEndian>()?;
            let z = reader.read_f32::<LittleEndian>()?;
            let w = reader.read_f32::<LittleEndian>()?;
            normals.push([x, y, z, w]);
        }

        Ok(BhkConvexVerticesShape {
            material,
            radius,
            prop_vertices,
            prop_normals,
            vertices,
            normals,
        })
    }
}

/// 形状リストブロック（複数の bhkShape の集合）。
///
/// 参照元: `references/nifxml/nif.xml:L3165` (`bhkListShape`)
#[derive(Clone, Debug, PartialEq)]
pub struct BhkListShape {
    /// 内包する子形状へのブロックインデックスリスト
    pub sub_shapes: Vec<i32>,
    /// マテリアル
    pub material: u32,
    /// 子形状プロパティ
    pub child_shape_prop: [u32; 3],
    /// 子フィルタープロパティ
    pub child_filter_prop: [u32; 3],
    /// フィルターリスト
    pub filters: Vec<[u8; 4]>,
}

impl BhkListShape {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let num_sub_shapes = reader.read_u32::<LittleEndian>()? as usize;
        let mut sub_shapes = Vec::with_capacity(num_sub_shapes);
        for _ in 0..num_sub_shapes {
            sub_shapes.push(reader.read_i32::<LittleEndian>()?);
        }

        let material = reader.read_u32::<LittleEndian>()?;
        let child_shape_prop = [
            reader.read_u32::<LittleEndian>()?,
            reader.read_u32::<LittleEndian>()?,
            reader.read_u32::<LittleEndian>()?,
        ];
        let child_filter_prop = [
            reader.read_u32::<LittleEndian>()?,
            reader.read_u32::<LittleEndian>()?,
            reader.read_u32::<LittleEndian>()?,
        ];

        let num_filters = reader.read_u32::<LittleEndian>()? as usize;
        let mut filters = Vec::with_capacity(num_filters);
        for _ in 0..num_filters {
            let mut filter = [0u8; 4];
            reader.read_exact(&mut filter)?;
            filters.push(filter);
        }

        Ok(BhkListShape {
            sub_shapes,
            material,
            child_shape_prop,
            child_filter_prop,
            filters,
        })
    }
}

/// スケルトン用ブレンドコリジョンオブジェクト。
///
/// 参照元: `references/nifxml/nif.xml:L3424` (`bhkBlendCollisionObject`)
#[derive(Clone, Debug, PartialEq)]
pub struct BhkBlendCollisionObject {
    /// コリジョンオブジェクト基底
    pub col: BhkCollisionObject,
    /// 階層ゲイン
    pub heir_gain: f32,
    /// 速度ゲイン
    pub vel_gain: f32,
}

impl BhkBlendCollisionObject {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let col = BhkCollisionObject::read(reader)?;
        let heir_gain = reader.read_f32::<LittleEndian>()?;
        let vel_gain = reader.read_f32::<LittleEndian>()?;
        Ok(BhkBlendCollisionObject {
            col,
            heir_gain,
            vel_gain,
        })
    }
}

/// 局所変換行列付き凸形状ブロック。
///
/// 参照元: `references/nifxml/nif.xml:L3107` (`bhkConvexTransformShape`)
#[derive(Clone, Debug, PartialEq)]
pub struct BhkConvexTransformShape {
    /// 内包される凸形状 (`bhkConvexShape`) への参照 (Ref)
    pub shape: i32,
    /// マテリアル (`Fallout3HavokMaterial`)
    pub material: u32,
    /// コリジョンシェル半径 (通常 0.05)
    pub radius: f32,
    /// 局所変換 4x4 行列 (`Matrix44`: 16 floats, 列優先/行優先)
    pub transform: [f32; 16],
}

impl BhkConvexTransformShape {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let shape = reader.read_i32::<LittleEndian>()?;
        let material = reader.read_u32::<LittleEndian>()?;
        let radius = reader.read_f32::<LittleEndian>()?;

        let mut unused01 = [0u8; 8];
        reader.read_exact(&mut unused01)?;

        let mut transform = [0.0f32; 16];
        for v in &mut transform {
            *v = reader.read_f32::<LittleEndian>()?;
        }

        Ok(BhkConvexTransformShape {
            shape,
            material,
            radius,
            transform,
        })
    }
}

/// 凸形状リストブロック（複数の凸形状の集合）。
///
/// 参照元: `references/nifxml/nif.xml:L6835` (`bhkConvexListShape`)
#[derive(Clone, Debug, PartialEq)]
pub struct BhkConvexListShape {
    /// 内包する子凸形状へのブロックインデックスリスト (Ref)
    pub sub_shapes: Vec<i32>,
    /// マテリアル (`Fallout3HavokMaterial`)
    pub material: u32,
    /// 半径
    pub radius: f32,
    /// 未知 uint
    pub unknown_int1: u32,
    /// 未知 float
    pub unknown_float1: f32,
    /// 子形状プロパティ (`bhkWorldObjCInfoProperty`)
    pub child_shape_prop: [u32; 3],
    /// キャッシュ AABB 使用フラグ
    pub use_cached_aabb: bool,
    /// 最近接点最小距離
    pub closest_point_min_distance: f32,
}

impl BhkConvexListShape {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let num_sub_shapes = reader.read_u32::<LittleEndian>()? as usize;
        let mut sub_shapes = Vec::with_capacity(num_sub_shapes);
        for _ in 0..num_sub_shapes {
            sub_shapes.push(reader.read_i32::<LittleEndian>()?);
        }

        let material = reader.read_u32::<LittleEndian>()?;
        let radius = reader.read_f32::<LittleEndian>()?;
        let unknown_int1 = reader.read_u32::<LittleEndian>()?;
        let unknown_float1 = reader.read_f32::<LittleEndian>()?;

        let child_shape_prop = [
            reader.read_u32::<LittleEndian>()?,
            reader.read_u32::<LittleEndian>()?,
            reader.read_u32::<LittleEndian>()?,
        ];

        let use_cached_aabb = reader.read_u8()? != 0;
        let closest_point_min_distance = reader.read_f32::<LittleEndian>()?;

        Ok(BhkConvexListShape {
            sub_shapes,
            material,
            radius,
            unknown_int1,
            unknown_float1,
            child_shape_prop,
            use_cached_aabb,
            closest_point_min_distance,
        })
    }
}

/// Fallout 3 Havok マテリアル種別。
///
/// 参照元: `references/nifxml/nif.xml:L444` (`Fallout3HavokMaterial`)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Fallout3HavokMaterial {
    Stone = 0,
    Cloth = 1,
    Dirt = 2,
    Glass = 3,
    Grass = 4,
    Metal = 5,
    Organic = 6,
    Skin = 7,
    Water = 8,
    Wood = 9,
    HeavyStone = 10,
    HeavyMetal = 11,
    HeavyWood = 12,
    Chain = 13,
    Bottlecap = 14,
    Elevator = 15,
    HollowMetal = 16,
    SheetMetal = 17,
    Sand = 18,
    BrokenConcrete = 19,
    VehicleBody = 20,
    VehiclePartSolid = 21,
    VehiclePartHollow = 22,
    Barrel = 23,
    Bottle = 24,
    SodaCan = 25,
    Pistol = 26,
    Rifle = 27,
    ShoppingCart = 28,
    Lunchbox = 29,
    BabyRattle = 30,
    RubberBall = 31,
    StonePlatform = 32,
    ClothPlatform = 33,
    DirtPlatform = 34,
    GlassPlatform = 35,
    MetalPlatform = 36,
    SandPlatform = 37,
    WoodPlatform = 38,
    Unknown(u32),
}

impl From<u32> for Fallout3HavokMaterial {
    fn from(val: u32) -> Self {
        match val {
            0 => Self::Stone,
            1 => Self::Cloth,
            2 => Self::Dirt,
            3 => Self::Glass,
            4 => Self::Grass,
            5 => Self::Metal,
            6 => Self::Organic,
            7 => Self::Skin,
            8 => Self::Water,
            9 => Self::Wood,
            10 => Self::HeavyStone,
            11 => Self::HeavyMetal,
            12 => Self::HeavyWood,
            13 => Self::Chain,
            14 => Self::Bottlecap,
            15 => Self::Elevator,
            16 => Self::HollowMetal,
            17 => Self::SheetMetal,
            18 => Self::Sand,
            19 => Self::BrokenConcrete,
            20 => Self::VehicleBody,
            21 => Self::VehiclePartSolid,
            22 => Self::VehiclePartHollow,
            23 => Self::Barrel,
            24 => Self::Bottle,
            25 => Self::SodaCan,
            26 => Self::Pistol,
            27 => Self::Rifle,
            28 => Self::ShoppingCart,
            29 => Self::Lunchbox,
            30 => Self::BabyRattle,
            31 => Self::RubberBall,
            32 => Self::StonePlatform,
            33 => Self::ClothPlatform,
            34 => Self::DirtPlatform,
            35 => Self::GlassPlatform,
            36 => Self::MetalPlatform,
            37 => Self::SandPlatform,
            38 => Self::WoodPlatform,
            other => Self::Unknown(other),
        }
    }
}

/// Fallout 3 Havok コリジョンレイヤー種別。
///
/// 参照元: `references/nifxml/nif.xml:L719` (`Fallout3Layer`)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Fallout3Layer {
    Unidentified = 0,
    Static = 1,
    AnimStatic = 2,
    Transparent = 3,
    Clutter = 4,
    Weapon = 5,
    Projectile = 6,
    Spell = 7,
    Biped = 8,
    Trees = 9,
    Props = 10,
    Water = 11,
    Trigger = 12,
    Terrain = 13,
    Trap = 14,
    NonCollidable = 15,
    CloudTrap = 16,
    Ground = 17,
    Portal = 18,
    DebrisSmall = 19,
    DebrisLarge = 20,
    AcousticSpace = 21,
    ActorZone = 22,
    ProjectileZone = 23,
    GasTrap = 24,
    ShellCasing = 25,
    TransparentSmall = 26,
    InvisibleWall = 27,
    TransparentSmallAnim = 28,
    DeadBip = 29,
    CharController = 30,
    AvoidBox = 31,
    CollisionBox = 32,
    CameraSphere = 33,
    DoorDetection = 34,
    CameraPick = 35,
    ItemPick = 36,
    LineOfSight = 37,
    PathPick = 38,
    CustomPick1 = 39,
    CustomPick2 = 40,
    SpellExplosion = 41,
    DroppingPick = 42,
    Null = 43,
    Unknown(u8),
}

impl From<u8> for Fallout3Layer {
    fn from(val: u8) -> Self {
        match val {
            0 => Self::Unidentified,
            1 => Self::Static,
            2 => Self::AnimStatic,
            3 => Self::Transparent,
            4 => Self::Clutter,
            5 => Self::Weapon,
            6 => Self::Projectile,
            7 => Self::Spell,
            8 => Self::Biped,
            9 => Self::Trees,
            10 => Self::Props,
            11 => Self::Water,
            12 => Self::Trigger,
            13 => Self::Terrain,
            14 => Self::Trap,
            15 => Self::NonCollidable,
            16 => Self::CloudTrap,
            17 => Self::Ground,
            18 => Self::Portal,
            19 => Self::DebrisSmall,
            20 => Self::DebrisLarge,
            21 => Self::AcousticSpace,
            22 => Self::ActorZone,
            23 => Self::ProjectileZone,
            24 => Self::GasTrap,
            25 => Self::ShellCasing,
            26 => Self::TransparentSmall,
            27 => Self::InvisibleWall,
            28 => Self::TransparentSmallAnim,
            29 => Self::DeadBip,
            30 => Self::CharController,
            31 => Self::AvoidBox,
            32 => Self::CollisionBox,
            33 => Self::CameraSphere,
            34 => Self::DoorDetection,
            35 => Self::CameraPick,
            36 => Self::ItemPick,
            37 => Self::LineOfSight,
            38 => Self::PathPick,
            39 => Self::CustomPick1,
            40 => Self::CustomPick2,
            41 => Self::SpellExplosion,
            42 => Self::DroppingPick,
            43 => Self::Null,
            other => Self::Unknown(other),
        }
    }
}

/// ローカル 4x4 変換行列を付与する Havok コリジョン形状ラッパー。
///
/// 参照元: `references/nifxml/nif.xml:L3029` (`bhkTransformShape`)
#[derive(Clone, Debug, PartialEq)]
pub struct BhkTransformShape {
    /// 変換対象の子シェイプへのブロックインデックス (Ref template="bhkShape")
    pub shape: i32,
    /// シェイプのマテリアル種別
    pub material: Fallout3HavokMaterial,
    /// コリジョン球半径 (通常 0.1)
    pub radius: f32,
    /// 予約・未使用領域 (8 バイト)
    pub unused_01: [u8; 8],
    /// 4x4 変換行列 (16要素 float)
    pub transform: [f32; 16],
}

impl BhkTransformShape {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let shape = reader.read_i32::<LittleEndian>()?;
        let mat_val = reader.read_u32::<LittleEndian>()?;
        let material = Fallout3HavokMaterial::from(mat_val);
        let radius = reader.read_f32::<LittleEndian>()?;
        let mut unused_01 = [0u8; 8];
        reader.read_exact(&mut unused_01)?;
        let mut transform = [0.0f32; 16];
        for val in &mut transform {
            *val = reader.read_f32::<LittleEndian>()?;
        }

        Ok(BhkTransformShape {
            shape,
            material,
            radius,
            unused_01,
            transform,
        })
    }
}

/// NiTriStripsData をジオメトリデータとして参照する静的メッシュコリジョン形状。
///
/// 参照元: `references/nifxml/nif.xml:L3207` (`bhkNiTriStripsShape`)
#[derive(Clone, Debug, PartialEq)]
pub struct BhkNiTriStripsShape {
    /// シェイプのマテリアル種別
    pub material: Fallout3HavokMaterial,
    /// コリジョン球半径 (通常 0.1)
    pub radius: f32,
    /// 予約・未使用領域 (20 バイト)
    pub unused_01: [u8; 20],
    /// 拡張バッファ設定 (デフォルト 1)
    pub grow_by: u32,
    /// スケールベクトル (XYZW)
    pub scale: [f32; 4],
    /// NiTriStripsData へのブロックインデックス列
    pub strips_data: Vec<i32>,
    /// HavokFilter 配列
    pub filters: Vec<u32>,
}

impl BhkNiTriStripsShape {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mat_val = reader.read_u32::<LittleEndian>()?;
        let material = Fallout3HavokMaterial::from(mat_val);
        let radius = reader.read_f32::<LittleEndian>()?;
        let mut unused_01 = [0u8; 20];
        reader.read_exact(&mut unused_01)?;
        let grow_by = reader.read_u32::<LittleEndian>()?;

        let mut scale = [0.0f32; 4];
        for val in &mut scale {
            *val = reader.read_f32::<LittleEndian>()?;
        }

        let num_strips_data = reader.read_u32::<LittleEndian>()? as usize;
        let mut strips_data = Vec::with_capacity(num_strips_data);
        for _ in 0..num_strips_data {
            strips_data.push(reader.read_i32::<LittleEndian>()?);
        }

        let num_filters = reader.read_u32::<LittleEndian>()? as usize;
        let mut filters = Vec::with_capacity(num_filters);
        for _ in 0..num_filters {
            filters.push(reader.read_u32::<LittleEndian>()?);
        }

        Ok(BhkNiTriStripsShape {
            material,
            radius,
            unused_01,
            grow_by,
            scale,
            strips_data,
            filters,
        })
    }
}

/// トリガーゾーンやイベント領域用の任意形状ファントムオブジェクト。
///
/// 参照元: `references/nifxml/nif.xml:L2790` (`bhkSimpleShapePhantom`)
#[derive(Clone, Debug, PartialEq)]
pub struct BhkSimpleShapePhantom {
    /// ワールドオブジェクト共通ヘッダー (shape, フィルター等)
    pub common: BhkWorldObjectCommon,
    /// 予約・未使用領域 (8 バイト)
    pub unused_01: [u8; 8],
    /// 4x4 変換行列 (16要素 float)
    pub transform: [f32; 16],
}

impl BhkSimpleShapePhantom {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let common = BhkWorldObjectCommon::read(reader)?;
        let mut unused_01 = [0u8; 8];
        reader.read_exact(&mut unused_01)?;
        let mut transform = [0.0f32; 16];
        for val in &mut transform {
            *val = reader.read_f32::<LittleEndian>()?;
        }

        Ok(BhkSimpleShapePhantom {
            common,
            unused_01,
            transform,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_half_to_f32() {
        assert_eq!(half_to_f32(0x0000), 0.0);
        assert_eq!(half_to_f32(0x8000), -0.0);
        assert_eq!(half_to_f32(0x3C00), 1.0);
        assert_eq!(half_to_f32(0xBC00), -1.0);
        assert_eq!(half_to_f32(0x4000), 2.0);
        assert_eq!(half_to_f32(0x3800), 0.5);
    }
}
