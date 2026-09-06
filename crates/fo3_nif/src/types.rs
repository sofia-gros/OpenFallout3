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
