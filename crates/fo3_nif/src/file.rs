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
                "BSShaderNoLightingProperty" => match BSShaderNoLightingProperty::read(&mut cursor) {
                    Ok(prop) => NifBlock::BSShaderNoLightingProperty(prop),
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
                "NiStencilProperty" => match NiStencilProperty::read(&mut cursor) {
                    Ok(sten) => NifBlock::NiStencilProperty(sten),
                    Err(_) => NifBlock::Unknown {
                        type_name: block_type_name.clone(),
                        data: block_bytes,
                    },
                },
                "BSXFlags" => match BSXFlags::read(&mut cursor) {
                    Ok(flags) => NifBlock::BSXFlags(flags),
                    Err(_) => NifBlock::Unknown {
                        type_name: block_type_name.clone(),
                        data: block_bytes,
                    },
                },
                "NiStringExtraData" => match NiStringExtraData::read(&mut cursor) {
                    Ok(extra) => NifBlock::NiStringExtraData(extra),
                    Err(_) => NifBlock::Unknown {
                        type_name: block_type_name.clone(),
                        data: block_bytes,
                    },
                },
                "NiIntegerExtraData" => match NiIntegerExtraData::read(&mut cursor) {
                    Ok(extra) => NifBlock::NiIntegerExtraData(extra),
                    Err(_) => NifBlock::Unknown {
                        type_name: block_type_name.clone(),
                        data: block_bytes,
                    },
                },
                "NiFloatExtraData" => match NiFloatExtraData::read(&mut cursor) {
                    Ok(extra) => NifBlock::NiFloatExtraData(extra),
                    Err(_) => NifBlock::Unknown {
                        type_name: block_type_name.clone(),
                        data: block_bytes,
                    },
                },
                "BSBound" => match BSBound::read(&mut cursor) {
                    Ok(bound) => NifBlock::BSBound(bound),
                    Err(_) => NifBlock::Unknown {
                        type_name: block_type_name.clone(),
                        data: block_bytes,
                    },
                },
                "bhkCollisionObject" => match BhkCollisionObject::read(&mut cursor) {
                    Ok(obj) => NifBlock::BhkCollisionObject(obj),
                    Err(e) => {
                        eprintln!("[WARN] bhkCollisionObjectパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "bhkSPCollisionObject" => match BhkCollisionObject::read(&mut cursor) {
                    Ok(obj) => NifBlock::BhkSPCollisionObject(obj),
                    Err(e) => {
                        eprintln!("[WARN] bhkSPCollisionObjectパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "bhkRigidBody" => match BhkRigidBody::read(&mut cursor) {
                    Ok(body) => NifBlock::BhkRigidBody(body),
                    Err(e) => {
                        eprintln!("[WARN] bhkRigidBodyパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "bhkRigidBodyT" => match BhkRigidBody::read(&mut cursor) {
                    Ok(body) => NifBlock::BhkRigidBodyT(body),
                    Err(e) => {
                        eprintln!("[WARN] bhkRigidBodyTパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "bhkMoppBvTreeShape" => match BhkMoppBvTreeShape::read(&mut cursor) {
                    Ok(shape) => NifBlock::BhkMoppBvTreeShape(shape),
                    Err(e) => {
                        eprintln!("[WARN] bhkMoppBvTreeShapeパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "bhkPackedNiTriStripsShape" => match BhkPackedNiTriStripsShape::read(&mut cursor) {
                    Ok(shape) => NifBlock::BhkPackedNiTriStripsShape(shape),
                    Err(e) => {
                        eprintln!("[WARN] bhkPackedNiTriStripsShapeパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "bhkNiTriStripsShape" => match BhkNiTriStripsShape::read(&mut cursor) {
                    Ok(shape) => NifBlock::BhkNiTriStripsShape(shape),
                    Err(e) => {
                        eprintln!("[WARN] bhkNiTriStripsShapeパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "hkPackedNiTriStripsData" => match HkPackedNiTriStripsData::read(&mut cursor) {
                    Ok(data) => NifBlock::HkPackedNiTriStripsData(data),
                    Err(e) => {
                        eprintln!("[WARN] hkPackedNiTriStripsDataパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "bhkBoxShape" => match BhkBoxShape::read(&mut cursor) {
                    Ok(shape) => NifBlock::BhkBoxShape(shape),
                    Err(e) => {
                        eprintln!("[WARN] bhkBoxShapeパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "bhkSphereShape" => match BhkSphereShape::read(&mut cursor) {
                    Ok(shape) => NifBlock::BhkSphereShape(shape),
                    Err(e) => {
                        eprintln!("[WARN] bhkSphereShapeパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "bhkCapsuleShape" => match BhkCapsuleShape::read(&mut cursor) {
                    Ok(shape) => NifBlock::BhkCapsuleShape(shape),
                    Err(e) => {
                        eprintln!("[WARN] bhkCapsuleShapeパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "bhkConvexVerticesShape" => match BhkConvexVerticesShape::read(&mut cursor) {
                    Ok(shape) => NifBlock::BhkConvexVerticesShape(shape),
                    Err(e) => {
                        eprintln!("[WARN] bhkConvexVerticesShapeパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "bhkConvexTransformShape" => match BhkConvexTransformShape::read(&mut cursor) {
                    Ok(shape) => NifBlock::BhkConvexTransformShape(shape),
                    Err(e) => {
                        eprintln!("[WARN] bhkConvexTransformShapeパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "bhkTransformShape" => match BhkTransformShape::read(&mut cursor) {
                    Ok(shape) => NifBlock::BhkTransformShape(shape),
                    Err(e) => {
                        eprintln!("[WARN] bhkTransformShapeパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "bhkConvexListShape" => match BhkConvexListShape::read(&mut cursor) {
                    Ok(shape) => NifBlock::BhkConvexListShape(shape),
                    Err(e) => {
                        eprintln!("[WARN] bhkConvexListShapeパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "bhkListShape" => match BhkListShape::read(&mut cursor) {
                    Ok(shape) => NifBlock::BhkListShape(shape),
                    Err(e) => {
                        eprintln!("[WARN] bhkListShapeパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "bhkSimpleShapePhantom" => match BhkSimpleShapePhantom::read(&mut cursor) {
                    Ok(phantom) => NifBlock::BhkSimpleShapePhantom(phantom),
                    Err(e) => {
                        eprintln!("[WARN] bhkSimpleShapePhantomパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "bhkBlendCollisionObject" => match BhkBlendCollisionObject::read(&mut cursor) {
                    Ok(obj) => NifBlock::BhkBlendCollisionObject(obj),
                    Err(e) => {
                        eprintln!("[WARN] bhkBlendCollisionObjectパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "NiSkinData" => match NiSkinData::read(&mut cursor) {
                    Ok(data) => NifBlock::NiSkinData(data),
                    Err(e) => {
                        eprintln!("[WARN] NiSkinDataパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "NiSkinInstance" => match NiSkinInstance::read(&mut cursor) {

                    Ok(inst) => NifBlock::NiSkinInstance(inst),
                    Err(e) => {
                        eprintln!("[WARN] NiSkinInstanceパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "NiSkinPartition" => match NiSkinPartition::read(&mut cursor) {
                    Ok(part) => NifBlock::NiSkinPartition(part),
                    Err(e) => {
                        eprintln!("[WARN] NiSkinPartitionパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
                },
                "BSDismemberSkinInstance" => match BSDismemberSkinInstance::read(&mut cursor) {
                    Ok(inst) => NifBlock::BSDismemberSkinInstance(inst),
                    Err(e) => {
                        eprintln!("[WARN] BSDismemberSkinInstanceパース失敗: {:?}", e);
                        NifBlock::Unknown {
                            type_name: block_type_name.clone(),
                            data: block_bytes,
                        }
                    }
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
