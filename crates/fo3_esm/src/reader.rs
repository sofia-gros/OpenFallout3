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
    ArmorRecord, CellRecord, HairRecord, LandRecord, LightRecord, LtexRecord, NpcRecord, OtftRecord,
    RefrRecord, StatRecord, Tes4Header, TextureSetRecord, WorldRecord,
};
use crate::subrecord::{parse_subrecords, Subrecord};
use crate::types::{
    FormId, FourCC, REC_ACHR, REC_ACRE, REC_ACTI, REC_ALCH, REC_AMMO, REC_ARMO, REC_BOOK, REC_CELL,
    REC_CONT, REC_DOOR, REC_FURN, REC_HAIR, REC_KEYM, REC_LAND, REC_LIGH, REC_LTEX, REC_MISC,
    REC_MSTT, REC_NPC_, REC_OTFT, REC_REFR, REC_SCOL, REC_STAT, REC_TERM, REC_TES4, REC_TXST,
    REC_WEAP, REC_WRLD, SUB_EDID, SUB_MODL,
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
    reader: R,
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
    )> {
        let mut npcs = HashMap::new();
        let mut armors = HashMap::new();
        let mut outfits = HashMap::new();
        let mut hairs = HashMap::new();

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

        Ok((npcs, armors, outfits, hairs))
    }

    /// 指定された EDID を持つ CELL レコード、その子 REFR レコード群、および地形 LAND レコード（存在する場合）を検索・取得する。
    ///
    /// 内部セル（トップレベル CELL グループ）および外部セル（WRLD グループ配下）の双方を走査する。
    pub fn find_cell_by_edid(&mut self, target_edid: &str) -> io::Result<Option<(CellRecord, Vec<RefrRecord>, Option<LandRecord>)>> {
        let start_pos = 24 + self.header_record.data_size as u64;
        self.reader.seek(SeekFrom::Start(start_pos))?;

        // 1. トップレベルの CELL グループ内を走査
        let mut wrld_group_start = 0u64;
        let mut wrld_group_size = 0u64;

        while let Some(entry) = self.read_next_entry()? {
            match entry {
                EsmEntry::Group(group) => {
                    let rtype = group.target_record_type();
                    let group_end = self.reader.stream_position()? + (group.group_size as u64 - GroupHeader::SIZE as u64);
                    if rtype == Some(REC_CELL) {
                        if let Some(res) = self.search_cell_in_stream(group_end, target_edid)? {
                            return Ok(Some(res));
                        }
                    } else if rtype == Some(REC_WRLD) {
                        wrld_group_start = self.reader.stream_position()?;
                        wrld_group_size = group.group_size as u64 - GroupHeader::SIZE as u64;
                        self.skip(wrld_group_size)?;
                    } else {
                        let rem = group.group_size as u64 - GroupHeader::SIZE as u64;
                        self.skip(rem)?;
                    }
                }
                EsmEntry::Record(rec, _) => {
                    self.skip(rec.data_size as u64)?;
                }
            }
        }

        // 2. CELL グループで見つからなかった場合、WRLD グループ内（外部セル）を走査
        if wrld_group_start > 0 {
            self.reader.seek(SeekFrom::Start(wrld_group_start))?;
            let wrld_end = wrld_group_start + wrld_group_size;
            return self.search_cell_in_stream(wrld_end, target_edid);
        }

        Ok(None)
    }

    fn search_cell_in_stream(&mut self, group_end: u64, target_edid: &str) -> io::Result<Option<(CellRecord, Vec<RefrRecord>, Option<LandRecord>)>> {
        let mut found_cell: Option<CellRecord> = None;
        let mut refrs = Vec::new();
        let mut land: Option<LandRecord> = None;

        while self.reader.stream_position()? < group_end {
            let entry = match self.read_next_entry()? {
                Some(e) => e,
                None => break,
            };

            match entry {
                EsmEntry::Group(group) => {
                    let group_size = group.group_size as u64 - GroupHeader::SIZE as u64;
                    let inner_end = self.reader.stream_position()? + group_size;

                    if let Some(ref cell) = found_cell {
                        let cell_id = cell.form_id.0;
                        let group_label_id = u32::from_le_bytes(group.label);
                        // 対象セルの子グループ (CellChildren=6, Persistent=8, Temporary=9, VisibleDistant=10)
                        // 参照元: references/openmw/components/esm4/loadgrup.hpp:L134-149
                        if (group.group_type == 6 || group.group_type == 8 || group.group_type == 9 || group.group_type == 10)
                            && group_label_id == cell_id
                        {
                            self.collect_children_in_group(inner_end, &mut refrs, &mut land)?;
                            continue;
                        } else {
                            // 対象セルの子グループ群を抜けたので検索終了
                            return Ok(Some((cell.clone(), refrs, land)));
                        }
                    } else {
                        // セル未発見時、セルレコードを含み得ない子グループ (6, 7, 8, 9, 10) は高速スキップ
                        if group.group_type == 6 || group.group_type == 7 || group.group_type == 8 || group.group_type == 9 || group.group_type == 10 {
                            self.skip(group_size)?;
                            continue;
                        }

                        // まだセルが見つかっていない場合、再帰的に探索
                        if let Some(result) = self.search_cell_in_stream(inner_end, target_edid)? {
                            return Ok(Some(result));
                        }
                    }
                }
                EsmEntry::Record(header, subrecords) => {
                    if header.type_id == REC_CELL {
                        if let Some(cell) = found_cell.take() {
                            return Ok(Some((cell, refrs, land)));
                        }

                        let cell = CellRecord::from_record(&header, &subrecords)?;
                        if cell.edid.eq_ignore_ascii_case(target_edid) {
                            found_cell = Some(cell);
                        }
                    }
                }
            }
        }

        if let Some(cell) = found_cell {
            Ok(Some((cell, refrs, land)))
        } else {
            Ok(None)
        }
    }

    fn collect_children_in_group(
        &mut self,
        group_end: u64,
        refrs: &mut Vec<RefrRecord>,
        land: &mut Option<LandRecord>,
    ) -> io::Result<()> {
        while self.reader.stream_position()? < group_end {
            let entry = match self.read_next_entry()? {
                Some(e) => e,
                None => break,
            };
            match entry {
                EsmEntry::Group(group) => {
                    let inner_end = self.reader.stream_position()? + (group.group_size as u64 - GroupHeader::SIZE as u64);
                    self.collect_children_in_group(inner_end, refrs, land)?;
                }
                EsmEntry::Record(header, subrecords) => {
                    if header.type_id == REC_REFR || header.type_id == REC_ACHR || header.type_id == REC_ACRE {
                        refrs.push(RefrRecord::from_record(&header, &subrecords)?);
                    } else if header.type_id == REC_LAND {
                        if land.is_none() {
                            *land = Some(LandRecord::parse(&header, &subrecords)?);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// 指定された EDID を持つ WRLD (World Space) レコードを検索し、
    /// (WorldRecord, 子グループ開始位置, 子グループ終了位置) を返す。
    pub fn find_world_by_edid(&mut self, target_edid: &str) -> io::Result<Option<(WorldRecord, u64, u64)>> {
        let start_pos = 24 + self.header_record.data_size as u64;
        self.reader.seek(SeekFrom::Start(start_pos))?;

        while let Some(entry) = self.read_next_entry()? {
            match entry {
                EsmEntry::Group(group) => {
                    let rtype = group.target_record_type();
                    let group_end = self.reader.stream_position()? + (group.group_size as u64 - GroupHeader::SIZE as u64);
                    if rtype == Some(REC_WRLD) {
                        while self.reader.stream_position()? < group_end {
                            if let Some(inner) = self.read_next_entry()? {
                                match inner {
                                    EsmEntry::Record(rec_hdr, subs) => {
                                        if rec_hdr.type_id == REC_WRLD {
                                            let world = WorldRecord::from_subrecords(rec_hdr.form_id, &subs);
                                            if world.edid.eq_ignore_ascii_case(target_edid) {
                                                let next_pos = self.reader.stream_position()?;
                                                if let Some(next_entry) = self.read_next_entry()? {
                                                    if let EsmEntry::Group(child_grp) = next_entry {
                                                        let child_start = self.reader.stream_position()?;
                                                        let child_end = child_start + (child_grp.group_size as u64 - GroupHeader::SIZE as u64);
                                                        return Ok(Some((world, child_start, child_end)));
                                                    }
                                                }
                                                self.reader.seek(SeekFrom::Start(next_pos))?;
                                                return Ok(Some((world, 0, 0)));
                                            }
                                        }
                                    }
                                    EsmEntry::Group(g) => {
                                        let skip = g.group_size as u64 - GroupHeader::SIZE as u64;
                                        self.skip(skip)?;
                                    }
                                }
                            }
                        }
                    } else {
                        let rem = group.group_size as u64 - GroupHeader::SIZE as u64;
                        self.skip(rem)?;
                    }
                }
                EsmEntry::Record(rec, _) => {
                    self.skip(rec.data_size as u64)?;
                }
            }
        }
        Ok(None)
    }

    /// ワールド空間ストリーム内を走査し、指定グリッド範囲内にある全セル（CELL, REFR群, LAND）を抽出する。
    /// `center_grid` が None の場合、独立ワールド空間なら全景（全セル）、広大ワールドなら最密集セル周辺を自動選択する。
    /// 参照元: references/openmw/components/esm4/loadgrup.hpp:L134-149
    pub fn read_cells_in_world_region(
        &mut self,
        group_start: u64,
        group_end: u64,
        center_grid: Option<(i32, i32)>,
        radius: i32,
    ) -> io::Result<(Option<(i32, i32)>, Vec<(CellRecord, Vec<RefrRecord>, Option<LandRecord>)>)> {
        self.reader.seek(SeekFrom::Start(group_start))?;
        let mut cell_order = Vec::new();
        let mut cell_map: HashMap<FormId, (CellRecord, Vec<RefrRecord>, Option<LandRecord>)> = HashMap::new();

        self.collect_region_cells_recursive(
            group_end,
            center_grid,
            radius,
            &mut cell_order,
            &mut cell_map,
        )?;

        // 有効セル（REFR または LAND または EDID を保持するセル）のみを抽出
        let mut all_valid_cells: Vec<(CellRecord, Vec<RefrRecord>, Option<LandRecord>)> = cell_order
            .into_iter()
            .filter_map(|id| cell_map.remove(&id))
            .filter(|(cell, refrs, land)| !refrs.is_empty() || land.is_some() || !cell.edid.is_empty())
            .collect();

        if let Some((cx, cy)) = center_grid {
            // グリッド指定がある場合: 指定グリッドの周囲 radius マスに厳密フィルタ
            all_valid_cells.retain(|(cell, _, _)| {
                if let Some((gx, gy)) = cell.grid {
                    (gx - cx).abs() <= radius && (gy - cy).abs() <= radius
                } else {
                    true
                }
            });
            Ok((Some((cx, cy)), all_valid_cells))
        } else {
            // グリッド未指定の場合:
            // 独立ワールド空間 (MegatonWorld 等: 有効セル数 25 件以下) なら全景をそのまま一括返却
            if all_valid_cells.len() <= 25 {
                let resolved_center = all_valid_cells
                    .iter()
                    .max_by_key(|(_, r, _)| r.len())
                    .and_then(|(c, _, _)| c.grid);
                Ok((resolved_center, all_valid_cells))
            } else {
                // 巨大荒野ワールド空間 (Wasteland 等): 最も配置物 (REFR) が多いセルを中心として radius マスを抽出
                let best_center = all_valid_cells
                    .iter()
                    .max_by_key(|(_, r, _)| r.len())
                    .and_then(|(c, _, _)| c.grid)
                    .unwrap_or((0, 0));
                all_valid_cells.retain(|(cell, _, _)| {
                    if let Some((gx, gy)) = cell.grid {
                        (gx - best_center.0).abs() <= radius && (gy - best_center.1).abs() <= radius
                    } else {
                        false
                    }
                });
                Ok((Some(best_center), all_valid_cells))
            }
        }
    }

    fn collect_region_cells_recursive(
        &mut self,
        end_pos: u64,
        center: Option<(i32, i32)>,
        radius: i32,
        cell_order: &mut Vec<FormId>,
        cell_map: &mut HashMap<FormId, (CellRecord, Vec<RefrRecord>, Option<LandRecord>)>,
    ) -> io::Result<()> {
        while self.reader.stream_position()? < end_pos {
            let entry = match self.read_next_entry()? {
                Some(e) => e,
                None => break,
            };

            match entry {
                EsmEntry::Group(group) => {
                    let group_size = group.group_size as u64 - GroupHeader::SIZE as u64;
                    let inner_end = self.reader.stream_position()? + group_size;
                    let group_label_id = FormId(u32::from_le_bytes(group.label));

                    // セル子グループ (CellChildren=6, Persistent=8, Temporary=9, VisibleDistant=10)
                    // 参照元: references/openmw/components/esm4/loadgrup.hpp:L134-149
                    if group.group_type == 6 || group.group_type == 8 || group.group_type == 9 || group.group_type == 10 {
                        if let Some((_, ref mut refrs, ref mut land)) = cell_map.get_mut(&group_label_id) {
                            self.collect_children_in_group(inner_end, refrs, land)?;
                        } else {
                            // 対象外のセルグループはスキップ
                            self.skip(group_size)?;
                        }
                        continue;
                    }

                    // コンテナグループ（Type 1: World Children, Type 4: Exterior Cell, Type 5: Sub-Cell 等）は再帰探索
                    self.collect_region_cells_recursive(
                        inner_end,
                        center,
                        radius,
                        cell_order,
                        cell_map,
                    )?;
                }
                EsmEntry::Record(header, subrecords) => {
                    if header.type_id == REC_CELL {
                        let cell = CellRecord::from_record(&header, &subrecords)?;
                        let in_range = if let Some((cx, cy)) = center {
                            if let Some((gx, gy)) = cell.grid {
                                (gx - cx).abs() <= radius && (gy - cy).abs() <= radius
                            } else {
                                true
                            }
                        } else {
                            // 中心未指定時は候補として全セルを登録
                            true
                        };

                        if in_range {
                            if !cell_map.contains_key(&cell.form_id) {
                                cell_order.push(cell.form_id);
                                cell_map.insert(cell.form_id, (cell, Vec::new(), None));
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// セル名からセルを検索し、内部セルなら単一セル、外部セルなら周囲 `radius` マス（3x3等）の全セルを返す。
    pub fn find_cell_and_neighbors(
        &mut self,
        target_edid: &str,
        radius: i32,
    ) -> io::Result<Option<Vec<(CellRecord, Vec<RefrRecord>, Option<LandRecord>)>>> {
        let initial = match self.find_cell_by_edid(target_edid)? {
            Some(res) => res,
            None => return Ok(None),
        };

        let (grid_x, grid_y) = match initial.0.grid {
            Some(g) if radius > 0 => g,
            _ => return Ok(Some(vec![initial])),
        };

        // 外部セルの場合、WRLD グループから周囲のセルを収集
        let start_pos = 24 + self.header_record.data_size as u64;
        self.reader.seek(SeekFrom::Start(start_pos))?;

        while let Some(entry) = self.read_next_entry()? {
            if let EsmEntry::Group(group) = entry {
                if group.target_record_type() == Some(REC_WRLD) {
                    let group_start = self.reader.stream_position()?;
                    let group_end = group_start + (group.group_size as u64 - GroupHeader::SIZE as u64);
                    let (_, cells) = self.read_cells_in_world_region(group_start, group_end, Some((grid_x, grid_y)), radius)?;
                    if !cells.is_empty() {
                        return Ok(Some(cells));
                    }
                } else {
                    self.skip(group.group_size as u64 - GroupHeader::SIZE as u64)?;
                }
            }
        }

        Ok(Some(vec![initial]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;
    use crate::header::SubrecordHeader;
    use crate::types::{FourCC, SUB_EDID};

    #[test]
    fn test_record_header_size() {
        assert_eq!(RecordHeader::SIZE, 24);
        assert_eq!(GroupHeader::SIZE, 24);
        assert_eq!(SubrecordHeader::SIZE, 6);
    }

    #[test]
    fn test_parse_subrecords_with_xxxx() {
        // [XXXX][size=4][real_size=8] + [EDID][size=0][data=8 bytes]
        let mut buf = Vec::new();
        // XXXX header (6 bytes): type="XXXX", data_size=4
        buf.extend_from_slice(b"XXXX");
        buf.extend_from_slice(&4u16.to_le_bytes());
        // XXXX data (4 bytes): real size 8
        buf.extend_from_slice(&8u32.to_le_bytes());

        // EDID header (6 bytes): type="EDID", data_size=0 (ダミー)
        buf.extend_from_slice(b"EDID");
        buf.extend_from_slice(&0u16.to_le_bytes());
        // EDID data (8 bytes): "TestEdid"
        buf.extend_from_slice(b"TestEdid");

        let total_size = buf.len();
        let mut cursor = Cursor::new(buf);
        let subrecords = parse_subrecords(&mut cursor, total_size).unwrap();

        assert_eq!(subrecords.len(), 1);
        assert_eq!(subrecords[0].type_id, SUB_EDID);
        assert_eq!(subrecords[0].as_string(), "TestEdid");
    }

    #[test]
    fn test_tes4_roundtrip_reading() {
        let mut buf = Vec::new();

        // 1. TES4 Record Header (24 bytes)
        // HEDR (6 + 12 = 18 bytes) + CNAM (6 + 6 = 12 bytes) = 30 bytes
        let data_size = 30u32;
        buf.extend_from_slice(b"TES4");
        buf.extend_from_slice(&data_size.to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes()); // flags: ESM
        buf.extend_from_slice(&0u32.to_le_bytes()); // FormId
        buf.extend_from_slice(&0u32.to_le_bytes()); // vc_info
        buf.extend_from_slice(&15u16.to_le_bytes()); // form_version
        buf.extend_from_slice(&0u16.to_le_bytes()); // vc_info2

        // 2. HEDR subrecord (18 bytes)
        buf.extend_from_slice(b"HEDR");
        buf.extend_from_slice(&12u16.to_le_bytes());
        buf.extend_from_slice(&0.94f32.to_le_bytes()); // version
        buf.extend_from_slice(&100i32.to_le_bytes()); // num_records
        buf.extend_from_slice(&0x800u32.to_le_bytes()); // next_object_id

        // 3. CNAM subrecord (12 bytes)
        buf.extend_from_slice(b"CNAM");
        buf.extend_from_slice(&6u16.to_le_bytes());
        buf.extend_from_slice(b"Bethesda\0".get(..6).unwrap());

        let cursor = Cursor::new(buf);
        let reader = EsmReader::new(cursor).unwrap();

        assert_eq!(reader.header.version, 0.94);
        assert_eq!(reader.header.num_records, 100);
        assert_eq!(reader.header.next_object_id, 0x800);
        assert_eq!(reader.header.author, "Bethes");
    }

    #[test]
    fn test_compressed_record_reading() {
        // 非圧縮データ: EDID subrecord (6 + 5 = 11 bytes): "Hello\0"
        let mut uncompressed_data = Vec::new();
        uncompressed_data.extend_from_slice(b"EDID");
        uncompressed_data.extend_from_slice(&6u16.to_le_bytes());
        uncompressed_data.extend_from_slice(b"Hello\0");

        let uncompressed_size = uncompressed_data.len() as u32;

        // zlib 圧縮
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&uncompressed_data).unwrap();
        let compressed_bytes = encoder.finish().unwrap();

        // TES4 ダミーヘッダー + 圧縮レコード
        let mut file_buf = Vec::new();
        // TES4 Header (24 bytes) + 空データ
        file_buf.extend_from_slice(b"TES4");
        file_buf.extend_from_slice(&0u32.to_le_bytes());
        file_buf.extend_from_slice(&1u32.to_le_bytes());
        file_buf.extend_from_slice(&0u32.to_le_bytes());
        file_buf.extend_from_slice(&0u32.to_le_bytes());
        file_buf.extend_from_slice(&15u16.to_le_bytes());
        file_buf.extend_from_slice(&0u16.to_le_bytes());

        // 圧縮レコード (STAT)
        let record_data_size = (4 + compressed_bytes.len()) as u32;
        file_buf.extend_from_slice(b"STAT");
        file_buf.extend_from_slice(&record_data_size.to_le_bytes());
        file_buf.extend_from_slice(&0x00040000u32.to_le_bytes()); // compressed flag
        file_buf.extend_from_slice(&0x1234u32.to_le_bytes());
        file_buf.extend_from_slice(&0u32.to_le_bytes());
        file_buf.extend_from_slice(&15u16.to_le_bytes());
        file_buf.extend_from_slice(&0u16.to_le_bytes());

        // レコード本体: 先頭4バイトに uncompressed_size + 圧縮データ
        file_buf.extend_from_slice(&uncompressed_size.to_le_bytes());
        file_buf.extend_from_slice(&compressed_bytes);

        let cursor = Cursor::new(file_buf);
        let mut reader = EsmReader::new(cursor).unwrap();

        let entry = reader.read_next_entry().unwrap().unwrap();
        if let EsmEntry::Record(header, subrecords) = entry {
            assert_eq!(header.type_id, FourCC(*b"STAT"));
            assert_eq!(subrecords.len(), 1);
            assert_eq!(subrecords[0].type_id, SUB_EDID);
            assert_eq!(subrecords[0].as_string(), "Hello");
        } else {
            panic!("Expected record");
        }
    }

    #[test]
    fn test_cell_and_refr_parsing() {
        use crate::types::{SUB_DATA, SUB_EDID, SUB_NAME, SUB_XSCL};

        // 1. CELL レコードのパーステスト
        let cell_header = RecordHeader {
            type_id: REC_CELL,
            data_size: 0,
            flags: 0,
            form_id: FormId(0x00012345),
            vc_info: 0,
            form_version: 15,
            vc_info2: 0,
        };
        let cell_subs = vec![
            Subrecord {
                type_id: SUB_EDID,
                data: b"TestCell01\0".to_vec(),
            },
            Subrecord {
                type_id: SUB_DATA,
                data: vec![0x01, 0x00], // Interior flag
            },
        ];
        let cell = CellRecord::from_record(&cell_header, &cell_subs).unwrap();
        assert_eq!(cell.form_id, FormId(0x00012345));
        assert_eq!(cell.edid, "TestCell01");
        assert!(cell.is_interior());

        // 2. REFR レコードのパーステスト
        let refr_header = RecordHeader {
            type_id: REC_REFR,
            data_size: 0,
            flags: 0,
            form_id: FormId(0x0006789A),
            vc_info: 0,
            form_version: 15,
            vc_info2: 0,
        };
        let mut data_bytes = Vec::new();
        // pos: [100.0, 200.0, 300.0]
        data_bytes.extend_from_slice(&100.0f32.to_le_bytes());
        data_bytes.extend_from_slice(&200.0f32.to_le_bytes());
        data_bytes.extend_from_slice(&300.0f32.to_le_bytes());
        // rot: [0.1, 0.2, 0.3]
        data_bytes.extend_from_slice(&0.1f32.to_le_bytes());
        data_bytes.extend_from_slice(&0.2f32.to_le_bytes());
        data_bytes.extend_from_slice(&0.3f32.to_le_bytes());

        let refr_subs = vec![
            Subrecord {
                type_id: SUB_EDID,
                data: b"TestRefr01\0".to_vec(),
            },
            Subrecord {
                type_id: SUB_NAME,
                data: 0x000ABCDEu32.to_le_bytes().to_vec(),
            },
            Subrecord {
                type_id: SUB_DATA,
                data: data_bytes,
            },
            Subrecord {
                type_id: SUB_XSCL,
                data: 1.5f32.to_le_bytes().to_vec(),
            },
        ];
        let refr = RefrRecord::from_record(&refr_header, &refr_subs).unwrap();
        assert_eq!(refr.form_id, FormId(0x0006789A));
        assert_eq!(refr.base_object, FormId(0x000ABCDE));
        assert_eq!(refr.position, [100.0, 200.0, 300.0]);
        assert_eq!(refr.rotation, [0.1, 0.2, 0.3]);
        assert_eq!(refr.scale, 1.5);
    }

    #[test]
    fn test_land_record_parsing_and_heights() {
        use crate::types::{SUB_DATA, SUB_VHGT};
        use crate::records::land::{LAND_NUM_VERTS, LAND_VERTS_PER_SIDE};

        let land_header = RecordHeader {
            type_id: REC_LAND,
            data_size: 0,
            flags: 0,
            form_id: FormId(0x00099999),
            vc_info: 0,
            form_version: 15,
            vc_info2: 0,
        };

        let mut vhgt_data = Vec::new();
        // height_offset = 100.0
        vhgt_data.extend_from_slice(&100.0f32.to_le_bytes());
        // 1089 個の勾配差分: すべて 1 (i8)
        let grad = vec![1i8; LAND_NUM_VERTS];
        for b in grad {
            vhgt_data.push(b as u8);
        }
        // unknown 3 bytes
        vhgt_data.extend_from_slice(&[0, 0, 0]);

        let land_subs = vec![
            Subrecord {
                type_id: SUB_DATA,
                data: 1u32.to_le_bytes().to_vec(),
            },
            Subrecord {
                type_id: SUB_VHGT,
                data: vhgt_data,
            },
        ];

        let land = LandRecord::parse(&land_header, &land_subs).unwrap();
        assert_eq!(land.form_id, FormId(0x00099999));
        assert_eq!(land.height_offset, 100.0);
        assert_eq!(land.gradient_data.len(), LAND_NUM_VERTS);

        let heights = land.compute_heights();
        // y=0, x=0: row_offset = 100 + 1 = 101, height = 101 * 8 = 808
        assert_eq!(heights[0], 808.0);
        // y=0, x=1: col_offset = 101 + 1 = 102, height = 102 * 8 = 816
        assert_eq!(heights[1], 816.0);
        // y=1, x=0: row_offset = 101 + 1 = 102, height = 102 * 8 = 816
        assert_eq!(heights[LAND_VERTS_PER_SIDE], 816.0);
    }

    /// REFR 拡張サブレコード (XTEL, XLOC, XESP, XMRK, TNAM, XOWN, XRNK, XCNT) のパース検証。
    #[test]
    fn test_refr_extended_subrecords() {
        use crate::types::{
            SUB_TNAM, SUB_XCNT, SUB_XESP, SUB_XLOC, SUB_XMRK, SUB_XOWN, SUB_XRNK, SUB_XTEL,
        };
        use crate::records::refr::{EnableParent, LockData, TeleportDoor};

        let refr_header = RecordHeader {
            type_id: REC_REFR,
            data_size: 0,
            flags: 0,
            form_id: FormId(0x00011111),
            vc_info: 0,
            form_version: 15,
            vc_info2: 0,
        };

        // XTEL (32 bytes): dest_door(0x00022222), pos(10.0, 20.0, 30.0), rot(0.5, 0.6, 0.7), flags(1)
        let mut xtel_data = Vec::new();
        xtel_data.extend_from_slice(&0x00022222u32.to_le_bytes());
        xtel_data.extend_from_slice(&10.0f32.to_le_bytes());
        xtel_data.extend_from_slice(&20.0f32.to_le_bytes());
        xtel_data.extend_from_slice(&30.0f32.to_le_bytes());
        xtel_data.extend_from_slice(&0.5f32.to_le_bytes());
        xtel_data.extend_from_slice(&0.6f32.to_le_bytes());
        xtel_data.extend_from_slice(&0.7f32.to_le_bytes());
        xtel_data.extend_from_slice(&1u32.to_le_bytes());

        // XLOC (12 bytes): lock_level(50), pad[3], key(0x00033333), flags(0)
        let mut xloc_data = vec![50u8, 0, 0, 0];
        xloc_data.extend_from_slice(&0x00033333u32.to_le_bytes());
        xloc_data.extend_from_slice(&0u32.to_le_bytes());

        // XESP (8 bytes): parent(0x00044444), flags(0x01 = Inversed)
        let mut xesp_data = Vec::new();
        xesp_data.extend_from_slice(&0x00044444u32.to_le_bytes());
        xesp_data.extend_from_slice(&1u32.to_le_bytes());

        let subs = vec![
            Subrecord {
                type_id: SUB_EDID,
                data: b"VaultExitDoor\0".to_vec(),
            },
            Subrecord {
                type_id: SUB_XTEL,
                data: xtel_data,
            },
            Subrecord {
                type_id: SUB_XLOC,
                data: xloc_data,
            },
            Subrecord {
                type_id: SUB_XESP,
                data: xesp_data,
            },
            Subrecord {
                type_id: SUB_XMRK,
                data: Vec::new(),
            },
            Subrecord {
                type_id: SUB_TNAM,
                data: 5u16.to_le_bytes().to_vec(),
            },
            Subrecord {
                type_id: SUB_XOWN,
                data: 0x00055555u32.to_le_bytes().to_vec(),
            },
            Subrecord {
                type_id: SUB_XRNK,
                data: 2i32.to_le_bytes().to_vec(),
            },
            Subrecord {
                type_id: SUB_XCNT,
                data: 50i32.to_le_bytes().to_vec(),
            },
        ];

        let refr = RefrRecord::from_record(&refr_header, &subs).unwrap();
        assert_eq!(refr.form_id, FormId(0x00011111));
        assert_eq!(refr.edid, "VaultExitDoor");
        assert_eq!(
            refr.teleport,
            Some(TeleportDoor {
                dest_door: FormId(0x00022222),
                dest_pos: [10.0, 20.0, 30.0],
                dest_rot: [0.5, 0.6, 0.7],
                flags: 1,
            })
        );
        assert_eq!(
            refr.lock,
            Some(LockData {
                lock_level: 50,
                key: Some(FormId(0x00033333)),
                flags: 0,
            })
        );
        assert_eq!(
            refr.enable_parent,
            Some(EnableParent {
                parent: FormId(0x00044444),
                flags: 1,
            })
        );
        assert!(refr.is_map_marker);
        assert_eq!(refr.map_marker_type, Some(5));
        assert_eq!(refr.owner, Some(FormId(0x00055555)));
        assert_eq!(refr.faction_rank, Some(2));
        assert_eq!(refr.count, 50);
    }
}
