//! # NIF ノード階層ブロック
//!
//! 参照元:
//! - `references/nifxml/nif.xml:L3359` (`NiObjectNET`)
//! - `references/nifxml/nif.xml:L3440` (`NiAVObject`)
//! - `references/nifxml/nif.xml:L4384` (`NiNode`)
//! - `references/nifxml/nif.xml:L6131` (`BSFadeNode`)

use std::io::{self, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::types::{Matrix33, Vector3};

/// 名前や追加データを持つオブジェクトの共通基底。
///
/// 参照元: `references/nifxml/nif.xml:L3359` (`NiObjectNET`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiObjectNET {
    /// オブジェクト名（グローバル文字列プールへのインデックス）
    pub name_index: u32,
    /// 追加データオブジェクトへの参照リスト
    pub extra_data_list: Vec<i32>,
    /// コントローラーオブジェクトへの参照 (-1 は None)
    pub controller: i32,
}

impl NiObjectNET {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let name_index = reader.read_u32::<LittleEndian>()?;
        let num_extra = reader.read_u32::<LittleEndian>()? as usize;
        let mut extra_data_list = Vec::with_capacity(num_extra);
        for _ in 0..num_extra {
            extra_data_list.push(reader.read_i32::<LittleEndian>()?);
        }
        let controller = reader.read_i32::<LittleEndian>()?;

        Ok(NiObjectNET {
            name_index,
            extra_data_list,
            controller,
        })
    }
}

/// シーングラフに配置可能な視覚オブジェクトの基底。
///
/// 参照元: `references/nifxml/nif.xml:L3440` (`NiAVObject`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiAVObject {
    pub net: NiObjectNET,
    /// 基本フラグ (Fallout 3 では u32)
    pub flags: u32,
    /// ローカル平行移動ベクトル
    pub translation: Vector3,
    /// ローカル回転行列
    pub rotation: Matrix33,
    /// ローカル均等スケール
    pub scale: f32,
    /// アタッチされているプロパティ（マテリアル、アルファ等）への参照リスト
    pub properties: Vec<i32>,
    /// コリジョンオブジェクトへの参照 (-1 は None)
    pub collision_object: i32,
}

impl NiAVObject {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let net = NiObjectNET::read(reader)?;
        let flags = reader.read_u32::<LittleEndian>()?;
        let translation = Vector3::read(reader)?;
        let rotation = Matrix33::read(reader)?;
        let scale = reader.read_f32::<LittleEndian>()?;

        let num_properties = reader.read_u32::<LittleEndian>()? as usize;
        let mut properties = Vec::with_capacity(num_properties);
        for _ in 0..num_properties {
            properties.push(reader.read_i32::<LittleEndian>()?);
        }

        let collision_object = reader.read_i32::<LittleEndian>()?;

        Ok(NiAVObject {
            net,
            flags,
            translation,
            rotation,
            scale,
            properties,
            collision_object,
        })
    }
}

/// 子ノードを持つ階層グループノード。
///
/// 参照元: `references/nifxml/nif.xml:L4384` (`NiNode`)
#[derive(Clone, Debug, PartialEq)]
pub struct NiNode {
    pub av: NiAVObject,
    /// 子ノードへの参照リスト
    pub children: Vec<i32>,
    /// ダイナミックエフェクト（ライト等）への参照リスト
    pub effects: Vec<i32>,
}

impl NiNode {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let av = NiAVObject::read(reader)?;

        let num_children = reader.read_u32::<LittleEndian>()? as usize;
        let mut children = Vec::with_capacity(num_children);
        for _ in 0..num_children {
            children.push(reader.read_i32::<LittleEndian>()?);
        }

        let num_effects = reader.read_u32::<LittleEndian>()? as usize;
        let mut effects = Vec::with_capacity(num_effects);
        for _ in 0..num_effects {
            effects.push(reader.read_i32::<LittleEndian>()?);
        }

        Ok(NiNode {
            av,
            children,
            effects,
        })
    }
}

/// Bethesda 固有の距離フェード制御ノード。
/// バイナリ構造は `NiNode` と完全に同一。
///
/// 参照元: `references/nifxml/nif.xml:L6131` (`BSFadeNode`)
#[derive(Clone, Debug, PartialEq)]
pub struct BSFadeNode {
    pub node: NiNode,
}

impl BSFadeNode {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let node = NiNode::read(reader)?;
        Ok(BSFadeNode { node })
    }
}
