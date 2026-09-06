//! # NIF ファイル全体パーサー (NifFile)
//!
//! ヘッダーおよび全ブロックを安全にデシリアライズします。
//! 各ブロックは `block_sizes` に基づいて厳密に区切られてパースされるため、
//! 未対応ブロックが存在しても後続ブロックがずれることなく安全に読み切れます。

use std::io::{BufRead, Cursor};
use crate::blocks::*;
use crate::header::{NifError, NifHeader};

/// 完全な NIF ファイル表現構造体。
#[derive(Clone, Debug)]
pub struct NifFile {
    /// NIF ヘッダー
    pub header: NifHeader,
    /// ファイル内に含まれる全ブロックのリスト
    pub blocks: Vec<NifBlock>,
}

impl NifFile {
    /// バッファリーダーから NIF ファイル全体を読み込む。
    pub fn read<R: BufRead>(reader: &mut R) -> Result<Self, NifError> {
        let header = NifHeader::read(reader)?;
        let num_blocks = header.num_blocks as usize;
        let mut blocks = Vec::with_capacity(num_blocks);

        for i in 0..num_blocks {
            let block_type_idx = header.block_type_indices[i] as usize;
            let block_type_name = &header.block_types[block_type_idx];
            let block_size = header.block_sizes[i] as usize;

            // 各ブロックの生バイト列を厳密に block_size 分だけ読み込む
            let mut block_bytes = vec![0u8; block_size];
            reader.read_exact(&mut block_bytes)?;

            let mut cursor = Cursor::new(&block_bytes);

            let block = match block_type_name.as_str() {
                "NiNode" => match NiNode::read(&mut cursor) {
                    Ok(node) => NifBlock::NiNode(node),
                    Err(_) => NifBlock::Unknown {
                        type_name: block_type_name.clone(),
                        data: block_bytes,
                    },
                },
                "BSFadeNode" => match BSFadeNode::read(&mut cursor) {
                    Ok(node) => NifBlock::BSFadeNode(node),
                    Err(_) => NifBlock::Unknown {
                        type_name: block_type_name.clone(),
                        data: block_bytes,
                    },
                },
                "NiTriShape" => match NiTriShape::read(&mut cursor) {
                    Ok(shape) => NifBlock::NiTriShape(shape),
                    Err(_) => NifBlock::Unknown {
                        type_name: block_type_name.clone(),
                        data: block_bytes,
                    },
                },
                "NiTriShapeData" => match NiTriShapeData::read(&mut cursor) {
                    Ok(data) => NifBlock::NiTriShapeData(data),
                    Err(_) => NifBlock::Unknown {
                        type_name: block_type_name.clone(),
                        data: block_bytes,
                    },
                },
                "NiTriStrips" => match NiTriStrips::read(&mut cursor) {
                    Ok(strips) => NifBlock::NiTriStrips(strips),
                    Err(_) => NifBlock::Unknown {
                        type_name: block_type_name.clone(),
                        data: block_bytes,
                    },
                },
                "NiTriStripsData" => match NiTriStripsData::read(&mut cursor) {
                    Ok(data) => NifBlock::NiTriStripsData(data),
                    Err(e) => {
                        eprintln!("[DEBUG] NiTriStripsDataパースエラー: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "BSShaderPPLightingProperty" => match BSShaderPPLightingProperty::read(&mut cursor) {
                    Ok(prop) => NifBlock::BSShaderPPLightingProperty(prop),
                    Err(_) => NifBlock::Unknown {
                        type_name: block_type_name.clone(),
                        data: block_bytes,
                    },
                },
                "BSShaderTextureSet" => match BSShaderTextureSet::read(&mut cursor) {
                    Ok(set) => NifBlock::BSShaderTextureSet(set),
                    Err(_) => NifBlock::Unknown {
                        type_name: block_type_name.clone(),
                        data: block_bytes,
                    },
                },
                "NiMaterialProperty" => match NiMaterialProperty::read(&mut cursor) {
                    Ok(mat) => NifBlock::NiMaterialProperty(mat),
                    Err(_) => NifBlock::Unknown {
                        type_name: block_type_name.clone(),
                        data: block_bytes,
                    },
                },
                "NiAlphaProperty" => match NiAlphaProperty::read(&mut cursor) {
                    Ok(alpha) => NifBlock::NiAlphaProperty(alpha),
                    Err(_) => NifBlock::Unknown {
                        type_name: block_type_name.clone(),
                        data: block_bytes,
                    },
                },
                _ => NifBlock::Unknown {
                    type_name: block_type_name.clone(),
                    data: block_bytes,
                },
            };

            blocks.push(block);
        }

        Ok(NifFile { header, blocks })
    }

    /// インデックス番号から文字列プール内の文字列を取得する。
    pub fn get_string(&self, index: u32) -> Option<&str> {
        self.header.strings.get(index as usize).map(|s| s.as_str())
    }
}
