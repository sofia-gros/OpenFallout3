//! # NIF 基本バイナリデータ型
//!
//! 参照元:
//! - `references/nifxml/nif.xml:L173` (`NiBound`)
//! - `references/nifxml/nif.xml:L2022` (`TexCoord`)
//! - `references/nifxml/nif.xml:L2043` (`Triangle`)

use std::io::{self, Read};
use byteorder::{LittleEndian, ReadBytesExt};

/// 3次元浮動小数点ベクトル。
///
/// 参照元: `references/nifxml/nif.xml` (`Vector3`)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3 {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let x = reader.read_f32::<LittleEndian>()?;
        let y = reader.read_f32::<LittleEndian>()?;
        let z = reader.read_f32::<LittleEndian>()?;
        Ok(Vector3 { x, y, z })
    }
}

/// 3x3 回転行列。
///
/// 参照元: `references/nifxml/nif.xml` (`Matrix33`)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix33 {
    pub m: [[f32; 3]; 3],
}

impl Matrix33 {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut m = [[0.0f32; 3]; 3];
        for row in &mut m {
            for val in row {
                *val = reader.read_f32::<LittleEndian>()?;
            }
        }
        Ok(Matrix33 { m })
    }
}

/// 4要素カラー (RGBA)。
///
/// 参照元: `references/nifxml/nif.xml` (`Color4`)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color4 {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color4 {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let r = reader.read_f32::<LittleEndian>()?;
        let g = reader.read_f32::<LittleEndian>()?;
        let b = reader.read_f32::<LittleEndian>()?;
        let a = reader.read_f32::<LittleEndian>()?;
        Ok(Color4 { r, g, b, a })
    }
}

/// 3要素カラー (RGB)。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color3 {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Color3 {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let r = reader.read_f32::<LittleEndian>()?;
        let g = reader.read_f32::<LittleEndian>()?;
        let b = reader.read_f32::<LittleEndian>()?;
        Ok(Color3 { r, g, b })
    }
}

/// 2次元テクスチャUV座標。
///
/// 参照元: `references/nifxml/nif.xml:L2022` (`TexCoord`)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexCoord {
    pub u: f32,
    pub v: f32,
}

impl TexCoord {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let u = reader.read_f32::<LittleEndian>()?;
        let v = reader.read_f32::<LittleEndian>()?;
        Ok(TexCoord { u, v })
    }
}

/// 三角形ポリゴンインデックス（3つの頂点番号）。
///
/// 参照元: `references/nifxml/nif.xml` (`Triangle`)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Triangle {
    pub v1: u16,
    pub v2: u16,
    pub v3: u16,
}

impl Triangle {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let v1 = reader.read_u16::<LittleEndian>()?;
        let v2 = reader.read_u16::<LittleEndian>()?;
        let v3 = reader.read_u16::<LittleEndian>()?;
        Ok(Triangle { v1, v2, v3 })
    }
}

/// バウンディングスフィア（中心座標 + 半径）。
///
/// 参照元: `references/nifxml/nif.xml:L173` (`NiBound`)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundingSphere {
    pub center: Vector3,
    pub radius: f32,
}

impl BoundingSphere {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let center = Vector3::read(reader)?;
        let radius = reader.read_f32::<LittleEndian>()?;
        Ok(BoundingSphere { center, radius })
    }
}

/// 長さ付き文字列 (len: u32 + bytes) を読み込む。
pub fn read_sized_string<R: Read>(reader: &mut R) -> io::Result<String> {
    let len = reader.read_u32::<LittleEndian>()? as usize;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).to_string())
}

/// 4要素クォータニオン (w, x, y, z)。
///
/// 参照元: `references/nifxml/nif.xml:L1787` (`Quaternion`)
/// 注意: Gamebryo / NIF のメモリ配置は `(w, x, y, z)` の順序である。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quaternion {
    pub w: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Default for Quaternion {
    fn default() -> Self {
        Quaternion { w: 1.0, x: 0.0, y: 0.0, z: 0.0 }
    }
}

impl Quaternion {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let w = reader.read_f32::<LittleEndian>()?;
        let x = reader.read_f32::<LittleEndian>()?;
        let y = reader.read_f32::<LittleEndian>()?;
        let z = reader.read_f32::<LittleEndian>()?;
        Ok(Quaternion { w, x, y, z })
    }

    /// glam::Quat に変換する。
    pub fn to_glam(&self) -> glam::Quat {
        glam::Quat::from_xyzw(self.x, self.y, self.z, self.w)
    }

    /// glam::Quat から作成する。
    pub fn from_glam(q: glam::Quat) -> Self {
        Quaternion { w: q.w, x: q.x, y: q.y, z: q.z }
    }
}

/// クォータニオン回転を含むローカルトランスフォーム。
///
/// 参照元: `references/nifxml/nif.xml:L2199` (`NiQuatTransform`)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NiQuatTransform {
    pub translation: Vector3,
    pub rotation: Quaternion,
    pub scale: f32,
}

impl Default for NiQuatTransform {
    fn default() -> Self {
        NiQuatTransform {
            translation: Vector3 { x: 0.0, y: 0.0, z: 0.0 },
            rotation: Quaternion::default(),
            scale: 1.0,
        }
    }
}

impl NiQuatTransform {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let translation = Vector3::read(reader)?;
        let rotation = Quaternion::read(reader)?;
        let scale = reader.read_f32::<LittleEndian>()?;
        Ok(NiQuatTransform { translation, rotation, scale })
    }
}

/// アニメーション補間補間方式 (KeyType)。
///
/// 参照元: `references/nifxml/nif.xml:L399` (`KeyType`)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum KeyType {
    Linear = 1,
    Quadratic = 2,
    Tbc = 3,
    XyzRotation = 4,
    Const = 5,
    Unknown(u32),
}

impl KeyType {
    pub fn from_u32(val: u32) -> Self {
        match val {
            1 => KeyType::Linear,
            2 => KeyType::Quadratic,
            3 => KeyType::Tbc,
            4 => KeyType::XyzRotation,
            5 => KeyType::Const,
            other => KeyType::Unknown(other),
        }
    }
}

/// TBC (Tension, Bias, Continuity) パラメータ。
///
/// 参照元: `references/nifxml/nif.xml:L1991` (`TBC`)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tbc {
    pub t: f32,
    pub b: f32,
    pub c: f32,
}

impl Tbc {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let t = reader.read_f32::<LittleEndian>()?;
        let b = reader.read_f32::<LittleEndian>()?;
        let c = reader.read_f32::<LittleEndian>()?;
        Ok(Tbc { t, b, c })
    }
}

/// 一般キーフレームデータ要素 (Key<T>)。
///
/// 参照元: `references/nifxml/nif.xml:L1998` (`Key`)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key<T> {
    pub time: f32,
    pub value: T,
    pub forward: Option<T>,
    pub backward: Option<T>,
    pub tbc: Option<Tbc>,
}

/// クォータニオンキーフレーム要素 (QuatKey)。
///
/// 参照元: `references/nifxml/nif.xml:L2014` (`QuatKey`)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuatKey {
    pub time: f32,
    pub value: Quaternion,
    pub tbc: Option<Tbc>,
}

impl QuatKey {
    pub fn read<R: Read>(reader: &mut R, key_type: KeyType) -> io::Result<Self> {
        let time = reader.read_f32::<LittleEndian>()?;
        let value = Quaternion::read(reader)?;
        let tbc = if key_type == KeyType::Tbc {
            Some(Tbc::read(reader)?)
        } else {
            None
        };
        Ok(QuatKey { time, value, tbc })
    }
}

/// 補間タイプ付きキーグループ。
///
/// 参照元: `references/nifxml/nif.xml:L2007` (`KeyGroup`)
#[derive(Clone, Debug, PartialEq)]
pub struct KeyGroup<T> {
    pub interpolation: KeyType,
    pub keys: Vec<Key<T>>,
}

impl<T> Default for KeyGroup<T> {
    fn default() -> Self {
        KeyGroup {
            interpolation: KeyType::Linear,
            keys: Vec::new(),
        }
    }
}

impl KeyGroup<f32> {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let num_keys = reader.read_u32::<LittleEndian>()? as usize;
        if num_keys == 0 {
            return Ok(KeyGroup::default());
        }
        let interpolation = KeyType::from_u32(reader.read_u32::<LittleEndian>()?);
        let mut keys = Vec::with_capacity(num_keys);
        for _ in 0..num_keys {
            let time = reader.read_f32::<LittleEndian>()?;
            let value = reader.read_f32::<LittleEndian>()?;
            let (forward, backward) = if interpolation == KeyType::Quadratic {
                (Some(reader.read_f32::<LittleEndian>()?), Some(reader.read_f32::<LittleEndian>()?))
            } else {
                (None, None)
            };
            let tbc = if interpolation == KeyType::Tbc {
                Some(Tbc::read(reader)?)
            } else {
                None
            };
            keys.push(Key { time, value, forward, backward, tbc });
        }
        Ok(KeyGroup { interpolation, keys })
    }
}

impl KeyGroup<Vector3> {
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let num_keys = reader.read_u32::<LittleEndian>()? as usize;
        if num_keys == 0 {
            return Ok(KeyGroup::default());
        }
        let interpolation = KeyType::from_u32(reader.read_u32::<LittleEndian>()?);
        let mut keys = Vec::with_capacity(num_keys);
        for _ in 0..num_keys {
            let time = reader.read_f32::<LittleEndian>()?;
            let value = Vector3::read(reader)?;
            let (forward, backward) = if interpolation == KeyType::Quadratic {
                (Some(Vector3::read(reader)?), Some(Vector3::read(reader)?))
            } else {
                (None, None)
            };
            let tbc = if interpolation == KeyType::Tbc {
                Some(Tbc::read(reader)?)
            } else {
                None
            };
            keys.push(Key { time, value, forward, backward, tbc });
        }
        Ok(KeyGroup { interpolation, keys })
    }
}

