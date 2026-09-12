//! # ESM / ESP リーダー
//!
//! ESM ファイルの逐次走査、グループトラバース、zlib 圧縮レコードの展開を担当。
//! 参照元: `references/openmw/components/esm4/reader.cpp`

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::Path;
use flate2::read::ZlibDecoder;
use byteorder::{LittleEndian, ReadBytesExt};

use crate::header::{GroupHeader, RecordHeader};
use crate::records::{
    ArmorRecord, HairRecord, LightRecord, LtexRecord, LvliRecord, NpcRecord, OtftRecord,
    StatRecord, Tes4Header, TextureSetRecord,
};
use crate::subrecord::{parse_subrecords, Subrecord};
use crate::types::{
    FormId, FourCC, REC_ACTI, REC_ALCH, REC_AMMO, REC_ARMO, REC_BOOK,
    REC_CONT, REC_DOOR, REC_FURN, REC_HAIR, REC_KEYM, REC_LIGH, REC_LTEX, REC_LVLI, REC_MISC,
    REC_MSTT, REC_NPC_, REC_OTFT, REC_SCOL, REC_STAT, REC_TERM, REC_TES4, REC_TXST,
    REC_WEAP, SUB_EDID, SUB_MODL,
};

/// 配置元ベースオブジェクトのメタ情報（モデルパス、エディタID、レコード型）。
#[derive(Clone, Debug, PartialEq)]
pub struct BaseObjectInfo {
    pub form_id: FormId,
    pub edid: String,
    pub model: String,
    pub record_type: FourCC,
}

/// ESM ファイルのエントリ（レコードまたはグループ）。
#[derive(Debug)]
pub enum EsmEntry {
    Record(RecordHeader, Vec<Subrecord>),
    Group(GroupHeader),
}

/// ESM ファイル読み込みリーダー。
pub struct EsmReader<R> {
    pub(crate) reader: R,
    pub header: Tes4Header,
    pub header_record: RecordHeader,
}

impl EsmReader<BufReader<File>> {
    /// ファイルパスを指定して ESM ファイルを開く。
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Self::new(reader)
    }
}

impl<R: Read + Seek> EsmReader<R> {
    pub fn new(mut reader: R) -> io::Result<Self> {
        // 先頭の TES4 レコードヘッダーを読み込み
        let header_record = RecordHeader::read(&mut reader)?;
        if header_record.type_id != REC_TES4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected TES4 header, found {:?}", header_record.type_id),
            ));
        }

        // サブレコードをパース
        let subrecords = parse_subrecords(&mut reader, header_record.data_size as usize)?;
        let header = Tes4Header::from_subrecords(&subrecords)?;

        Ok(EsmReader {
            reader,
            header,
            header_record,
        })
    }

    /// 現在のファイル位置にあるヘッダーがレコードかグループかを判定して読み出す。
    /// ファイル終端に達した場合は `None` を返す。
    pub fn read_next_entry(&mut self) -> io::Result<Option<EsmEntry>> {
        let mut sig = [0u8; 4];
        match self.reader.read_exact(&mut sig) {
            Ok(_) => {}
            Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        }

        // 4バイト戻してヘッダー全体をパース
        self.reader.seek(SeekFrom::Current(-4))?;

        if sig == *b"GRUP" {
            let group = GroupHeader::read(&mut self.reader)?;
            Ok(Some(EsmEntry::Group(group)))
        } else {
            let record = RecordHeader::read(&mut self.reader)?;
            let subrecords = self.read_record_data(&record)?;
            Ok(Some(EsmEntry::Record(record, subrecords)))
        }
    }

    /// レコードデータを読み込み（圧縮されている場合は zlib 展開）。
    pub fn read_record_data(&mut self, record: &RecordHeader) -> io::Result<Vec<Subrecord>> {
        if record.is_compressed() {
            let uncompressed_size = self.reader.read_u32::<LittleEndian>()? as usize;
            let compressed_size = (record.data_size - 4) as usize;

            let mut compressed = vec![0u8; compressed_size];
            self.reader.read_exact(&mut compressed)?;

            let mut decoder = ZlibDecoder::new(Cursor::new(compressed));
            let mut decompressed = Vec::with_capacity(uncompressed_size);
            decoder.read_to_end(&mut decompressed)?;

            let mut cursor = Cursor::new(decompressed);
            parse_subrecords(&mut cursor, uncompressed_size)
        } else {
            parse_subrecords(&mut self.reader, record.data_size as usize)
        }
    }

    /// 現在の読み込みストリーム位置を取得する。
    pub fn stream_position(&mut self) -> io::Result<u64> {
        self.reader.stream_position()
    }

    /// 指定した位置にシークする。
    pub fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.reader.seek(pos)
    }

    /// 現在位置から指定バイト数スキップする。
    pub fn skip(&mut self, bytes: u64) -> io::Result<()> {
        self.reader.seek(SeekFrom::Current(bytes as i64))?;
        Ok(())
    }

    /// トップレベルグループを一覧走査する。
    pub fn list_top_groups(&mut self) -> io::Result<Vec<GroupHeader>> {
        // TES4 レコードの直後 (offset: 24 + data_size) にシーク
        let start_pos = 24 + self.header_record.data_size as u64;
        self.reader.seek(SeekFrom::Start(start_pos))?;

        let mut groups = Vec::new();
        while let Some(entry) = self.read_next_entry()? {
            match entry {
                EsmEntry::Group(group) => {
                    groups.push(group);
                    // グループ全体のサイズ（ヘッダーの24バイト含む）からヘッダー分をスキップして次のグループへ
                    let remaining = group.group_size as u64 - GroupHeader::SIZE as u64;
                    self.skip(remaining)?;
                }
                EsmEntry::Record(rec, _) => {
                    // グループ外のレコードがあればスキップ
                    self.skip(rec.data_size as u64)?;
                }
            }
        }

        Ok(groups)
    }

    /// STAT グループを探索し、指定件数（または全件）の StatRecord を読み出す。
    pub fn read_stat_records(&mut self, limit: Option<usize>) -> io::Result<Vec<StatRecord>> {
        let start_pos = 24 + self.header_record.data_size as u64;
        self.reader.seek(SeekFrom::Start(start_pos))?;

        let mut stats = Vec::new();

        while let Some(entry) = self.read_next_entry()? {
            match entry {
                EsmEntry::Group(group) => {
                    if group.target_record_type() == Some(REC_STAT) {
                        // STAT グループ内に進入
                        let group_end = self.reader.stream_position()? + (group.group_size as u64 - GroupHeader::SIZE as u64);
                        while self.reader.stream_position()? < group_end {
                            if let Some(inner) = self.read_next_entry()? {
                                if let EsmEntry::Record(header, subrecords) = inner {
                                    if header.type_id == REC_STAT {
                                        stats.push(StatRecord::from_record(&header, &subrecords)?);
                                        if let Some(lim) = limit {
                                            if stats.len() >= lim {
                                                return Ok(stats);
                                            }
                                        }
                                    }
                                }
                            } else {
                                break;
                            }
                        }
                        return Ok(stats);
                    } else {
                        // 目的以外のグループはスキップ
                        let remaining = group.group_size as u64 - GroupHeader::SIZE as u64;
                        self.skip(remaining)?;
                    }
                }
                EsmEntry::Record(rec, _) => {
                    self.skip(rec.data_size as u64)?;
                }
            }
        }

        Ok(stats)
    }

    /// 全 STAT レコードを FormId をキーとする HashMap として読み出す。
    pub fn read_stat_map(&mut self) -> io::Result<HashMap<FormId, StatRecord>> {
        let stats = self.read_stat_records(None)?;
        let mut map = HashMap::with_capacity(stats.len());
        for stat in stats {
            map.insert(stat.form_id, stat);
        }
        Ok(map)
    }

    /// LIGH グループ内の全 LightRecord を走査して取得する。
    pub fn read_light_records(&mut self, limit: Option<usize>) -> io::Result<Vec<LightRecord>> {
        let start_pos = 24 + self.header_record.data_size as u64;
        self.reader.seek(SeekFrom::Start(start_pos))?;

        let mut lights = Vec::new();

        while let Some(entry) = self.read_next_entry()? {
            match entry {
                EsmEntry::Group(group) => {
                    if group.target_record_type() == Some(REC_LIGH) {
                        let group_end = self.reader.stream_position()? + (group.group_size as u64 - GroupHeader::SIZE as u64);
                        while self.reader.stream_position()? < group_end {
                            if let Some(inner) = self.read_next_entry()? {
                                if let EsmEntry::Record(header, subrecords) = inner {
                                    if header.type_id == REC_LIGH {
                                        lights.push(LightRecord::from_record(&header, &subrecords)?);
                                        if let Some(lim) = limit {
                                            if lights.len() >= lim {
                                                return Ok(lights);
                                            }
                                        }
                                    }
                                }
                            } else {
                                break;
                            }
                        }
                        return Ok(lights);
                    } else {
                        let remaining = group.group_size as u64 - GroupHeader::SIZE as u64;
                        self.skip(remaining)?;
                    }
                }
                EsmEntry::Record(rec, _) => {
                    self.skip(rec.data_size as u64)?;
                }
            }
        }

        Ok(lights)
    }

    /// 全 LIGH レコードを FormId をキーとする HashMap として読み出す。
    pub fn read_light_map(&mut self) -> io::Result<HashMap<FormId, LightRecord>> {
        let lights = self.read_light_records(None)?;
        let mut map = HashMap::with_capacity(lights.len());
        for light in lights {
            map.insert(light.form_id, light);
        }
        Ok(map)
    }

    /// LTEX グループ内の全 LtexRecord を走査して取得する。
    /// 参照元: `references/openmw/components/esm4/loadltex.hpp`, `loadltex.cpp`
    pub fn read_ltex_records(&mut self, limit: Option<usize>) -> io::Result<Vec<LtexRecord>> {
        let start_pos = 24 + self.header_record.data_size as u64;
        self.reader.seek(SeekFrom::Start(start_pos))?;

        let mut textures = Vec::new();

        while let Some(entry) = self.read_next_entry()? {
            match entry {
                EsmEntry::Group(group) => {
                    if group.target_record_type() == Some(REC_LTEX) {
                        let group_end = self.reader.stream_position()? + (group.group_size as u64 - GroupHeader::SIZE as u64);
                        while self.reader.stream_position()? < group_end {
                            if let Some(inner) = self.read_next_entry()? {
                                if let EsmEntry::Record(header, subrecords) = inner {
                                    if header.type_id == REC_LTEX {
                                        textures.push(LtexRecord::read(&header, &subrecords)?);
                                        if let Some(lim) = limit {
                                            if textures.len() >= lim {
                                                return Ok(textures);
                                            }
                                        }
                                    }
                                }
                            } else {
                                break;
                            }
                        }
                        return Ok(textures);
                    } else {
                        let remaining = group.group_size as u64 - GroupHeader::SIZE as u64;
                        self.skip(remaining)?;
                    }
                }
                EsmEntry::Record(rec, _) => {
                    self.skip(rec.data_size as u64)?;
                }
            }
        }

        Ok(textures)
    }

    /// 全 LTEX レコードを FormId をキーとする HashMap として読み出す。
    pub fn read_ltex_map(&mut self) -> io::Result<HashMap<FormId, LtexRecord>> {
        let textures = self.read_ltex_records(None)?;
        let mut map = HashMap::with_capacity(textures.len());
        for tex in textures {
            map.insert(tex.form_id, tex);
        }
        Ok(map)
    }

    /// TXST グループ内の全 TextureSetRecord を走査して取得する。
    /// 参照元: `references/openmw/components/esm4/loadtxst.hpp`, `loadtxst.cpp`
    pub fn read_txst_records(&mut self, limit: Option<usize>) -> io::Result<Vec<TextureSetRecord>> {
        let start_pos = 24 + self.header_record.data_size as u64;
        self.reader.seek(SeekFrom::Start(start_pos))?;

        let mut textures = Vec::new();

        while let Some(entry) = self.read_next_entry()? {
            match entry {
                EsmEntry::Group(group) => {
                    if group.target_record_type() == Some(REC_TXST) {
                        let group_end = self.reader.stream_position()? + (group.group_size as u64 - GroupHeader::SIZE as u64);
                        while self.reader.stream_position()? < group_end {
                            if let Some(inner) = self.read_next_entry()? {
                                if let EsmEntry::Record(header, subrecords) = inner {
                                    if header.type_id == REC_TXST {
                                        textures.push(TextureSetRecord::read(&header, &subrecords)?);
                                        if let Some(lim) = limit {
                                            if textures.len() >= lim {
                                                return Ok(textures);
                                            }
                                        }
                                    }
                                }
                            } else {
                                break;
                            }
                        }
                        return Ok(textures);
                    } else {
                        let remaining = group.group_size as u64 - GroupHeader::SIZE as u64;
                        self.skip(remaining)?;
                    }
                }
                EsmEntry::Record(rec, _) => {
                    self.skip(rec.data_size as u64)?;
                }
            }
        }

        Ok(textures)
    }

    /// 全 TXST レコードを FormId をキーとする HashMap として読み出す。
    pub fn read_txst_map(&mut self) -> io::Result<HashMap<FormId, TextureSetRecord>> {
        let textures = self.read_txst_records(None)?;
        let mut map = HashMap::with_capacity(textures.len());
        for tex in textures {
            map.insert(tex.form_id, tex);
        }
        Ok(map)
    }

    /// LTEX と TXST を一括走査し、LTEX の FormId から (diffuse, normal_map) のファイルパスを取得できるマップを構築する。
    pub fn read_landscape_texture_map(&mut self) -> io::Result<HashMap<FormId, (String, String)>> {
        let ltex_map = self.read_ltex_map()?;
        let txst_map = self.read_txst_map()?;

        let mut result = HashMap::with_capacity(ltex_map.len());
        for (ltex_id, ltex) in ltex_map {
            if let Some(txst) = txst_map.get(&ltex.texture_set) {
                result.insert(ltex_id, (txst.diffuse.clone(), txst.normal_map.clone()));
            } else if !ltex.texture_path.is_empty() {
                let norm = ltex.texture_path.replace(".dds", "_n.dds");
                result.insert(ltex_id, (ltex.texture_path.clone(), norm));
            }
        }

        Ok(result)
    }

    /// STAT, SCOL, DOOR, ACTI, FURN, CONT, MSTT, TERM など
    /// 3D モデル (MODL) を保持するすべての基本レコードを一括走査してマップを構築する。
    pub fn read_all_models_map(&mut self) -> io::Result<HashMap<FormId, BaseObjectInfo>> {
        let start_pos = 24 + self.header_record.data_size as u64;
        self.reader.seek(SeekFrom::Start(start_pos))?;

        let target_types = [
            REC_STAT, REC_SCOL, REC_DOOR, REC_ACTI,
            REC_FURN, REC_CONT, REC_MSTT, REC_TERM,
            REC_LIGH, REC_MISC, REC_BOOK, REC_ALCH,
            REC_KEYM, REC_WEAP, REC_AMMO, REC_ARMO,
        ];

        let mut map = HashMap::new();

        while let Some(entry) = self.read_next_entry()? {
            match entry {
                EsmEntry::Group(group) => {
                    if let Some(rtype) = group.target_record_type() {
                        if target_types.contains(&rtype) {
                            let group_end = self.reader.stream_position()? + (group.group_size as u64 - GroupHeader::SIZE as u64);
                            while self.reader.stream_position()? < group_end {
                                if let Some(inner) = self.read_next_entry()? {
                                    if let EsmEntry::Record(header, subrecords) = inner {
                                        let mut edid = String::new();
                                        let mut model = String::new();
                                        for sub in &subrecords {
                                            if sub.type_id == SUB_EDID {
                                                edid = sub.as_string();
                                            } else if sub.type_id == SUB_MODL {
                                                model = sub.as_string();
                                            }
                                        }
                                        if !model.is_empty() {
                                            map.insert(header.form_id, BaseObjectInfo {
                                                form_id: header.form_id,
                                                edid,
                                                model,
                                                record_type: header.type_id,
                                            });
                                        }
                                    }
                                } else {
                                    break;
                                }
                            }
                            continue;
                        }
                    }
                    let remaining = group.group_size as u64 - GroupHeader::SIZE as u64;
                    self.skip(remaining)?;
                }
                EsmEntry::Record(rec, _) => {
                    self.skip(rec.data_size as u64)?;
                }
            }
        }

        Ok(map)
    }

    /// ESM 内の NPC_ (NPC定義)、ARMO (防具定義)、OTFT (衣装定義)、HAIR (髪型定義) を一括収集する。
    pub fn read_npc_and_armor_map(
        &mut self,
    ) -> io::Result<(
        HashMap<FormId, NpcRecord>,
        HashMap<FormId, ArmorRecord>,
        HashMap<FormId, OtftRecord>,
        HashMap<FormId, HairRecord>,
        HashMap<FormId, LvliRecord>,
    )> {
        let mut npcs = HashMap::new();
        let mut armors = HashMap::new();
        let mut outfits = HashMap::new();
        let mut hairs = HashMap::new();
        let mut lvlis = HashMap::new();

        let start_pos = 24 + self.header_record.data_size as u64;
        self.reader.seek(SeekFrom::Start(start_pos))?;

        while let Some(entry) = self.read_next_entry()? {
            match entry {
                EsmEntry::Group(group) => {
                    let rtype = group.target_record_type();
                    if rtype == Some(REC_NPC_)
                        || rtype == Some(REC_ARMO)
                        || rtype == Some(REC_OTFT)
                        || rtype == Some(REC_HAIR)
                        || rtype == Some(REC_LVLI)
                    {
                        let group_end = self.reader.stream_position()? + (group.group_size as u64 - GroupHeader::SIZE as u64);
                        while self.reader.stream_position()? < group_end {
                            if let Some(inner) = self.read_next_entry()? {
                                match inner {
                                    EsmEntry::Record(header, subs) => {
                                        if header.type_id == REC_NPC_ {
                                            if let Ok(npc) = NpcRecord::from_record(&header, &subs) {
                                                npcs.insert(header.form_id, npc);
                                            }
                                        } else if header.type_id == REC_ARMO {
                                            if let Ok(armor) = ArmorRecord::from_record(&header, &subs) {
                                                armors.insert(header.form_id, armor);
                                            }
                                        } else if header.type_id == REC_OTFT {
                                            if let Ok(otft) = OtftRecord::from_record(&header, &subs) {
                                                outfits.insert(header.form_id, otft);
                                            }
                                        } else if header.type_id == REC_HAIR {
                                            if let Ok(hair) = HairRecord::from_record(&header, &subs) {
                                                hairs.insert(header.form_id, hair);
                                            }
                                        } else if header.type_id == REC_LVLI {
                                            if let Ok(lvli) = LvliRecord::from_record(&header, &subs) {
                                                lvlis.insert(header.form_id, lvli);
                                            }
                                        }
                                    }
                                    EsmEntry::Group(child_group) => {
                                        let skip = child_group.group_size as u64 - GroupHeader::SIZE as u64;
                                        self.skip(skip)?;
                                    }
                                }
                            } else {
                                break;
                            }
                        }
                        continue;
                    }
                    let remaining = group.group_size as u64 - GroupHeader::SIZE as u64;
                    self.skip(remaining)?;
                }
                EsmEntry::Record(rec, _) => {
                    self.skip(rec.data_size as u64)?;
                }
            }
        }

        Ok((npcs, armors, outfits, hairs, lvlis))
    }
}
