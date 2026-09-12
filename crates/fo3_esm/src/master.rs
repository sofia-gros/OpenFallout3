//! ESM マスター定義キャッシュ
//!
//! 参照元: Gamebryo 2.6 `NiStream`, Bethesda ESM/BSA 構造, `knowledge/gamebryo_resource_management_and_caching.md`
//!
//! `Fallout3.esm` に含まれる静的マスターレコード（3Dモデル定義、アクター、防具、衣装、髪型、レベルドアイテム、光源）
//! をゲーム起動時に一度だけ走査・パースし、メモリ上に常駐させるための構造体を提供します。

use std::collections::HashMap;
use std::io::{self, Read, Seek};
use std::path::Path;

use crate::records::light::LightRecord;
use crate::records::lvli::LvliRecord;
use crate::records::npc::NpcRecord;
use crate::records::otft::OtftRecord;
use crate::records::hair::HairRecord;
use crate::records::armo::ArmorRecord;
use crate::reader::{BaseObjectInfo, EsmReader};
use crate::types::FormId;

/// ESM ファイル全体の静的マスター定義をキャッシュする構造体。
///
/// セル間遷移（テレポート）のたびに 1GB 超の ESM 全体を再スキャンすることを防ぎ、
/// 数値 FormID からの高速な O(1) 逆引きを実現します。
#[derive(Clone, Debug, Default)]
pub struct EsmMasterContext {
    /// FormID から 3D モデル情報 (STAT, DOOR, CONT, ACTI, FURN, etc.) へのマップ
    pub model_map: HashMap<FormId, BaseObjectInfo>,
    /// FormID から NPC 定義レコードへのマップ
    pub npc_map: HashMap<FormId, NpcRecord>,
    /// FormID から防具レコードへのマップ
    pub armor_map: HashMap<FormId, ArmorRecord>,
    /// FormID から衣装レコードへのマップ
    pub outfit_map: HashMap<FormId, OtftRecord>,
    /// FormID から髪型レコードへのマップ
    pub hair_map: HashMap<FormId, HairRecord>,
    /// FormID からレベルドアイテムへのマップ
    pub lvli_map: HashMap<FormId, LvliRecord>,
    /// FormID から光源定義レコードへのマップ
    pub light_map: HashMap<FormId, LightRecord>,
}

impl EsmMasterContext {
    /// 新しい空のマスターコンテキストを生成する。
    pub fn new() -> Self {
        Self::default()
    }

    /// リーダーから全マスターレコードを一括走査・構築する。
    ///
    /// 参照元: `references/openmw/components/esm4/`
    pub fn load_from_reader<R: Read + Seek>(reader: &mut EsmReader<R>) -> io::Result<Self> {
        let model_map = reader.read_all_models_map()?;
        let (npc_map, armor_map, outfit_map, hair_map, lvli_map) =
            reader.read_npc_and_armor_map().unwrap_or_default();
        let light_map = reader.read_light_map().unwrap_or_default();

        Ok(Self {
            model_map,
            npc_map,
            armor_map,
            outfit_map,
            hair_map,
            lvli_map,
            light_map,
        })
    }

    /// 指定された ESM ファイルパスからマスターコンテキストを読み込む。
    pub fn open_and_load(path: &Path) -> io::Result<Self> {
        let mut reader = EsmReader::open(path)?;
        Self::load_from_reader(&mut reader)
    }

    /// 登録されているマスターレコードの総数を返す。
    pub fn total_records(&self) -> usize {
        self.model_map.len()
            + self.npc_map.len()
            + self.armor_map.len()
            + self.outfit_map.len()
            + self.hair_map.len()
            + self.lvli_map.len()
            + self.light_map.len()
    }
}
