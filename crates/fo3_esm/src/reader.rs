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
use crate::records::{CellRecord, RefrRecord, StatRecord, Tes4Header};
use crate::subrecord::{parse_subrecords, Subrecord};
use crate::types::{
    FormId, FourCC, REC_ACTI, REC_CELL, REC_CONT, REC_DOOR, REC_FURN, REC_MSTT, REC_REFR,
    REC_SCOL, REC_STAT, REC_TERM, REC_TES4, SUB_EDID, SUB_MODL,
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

    /// STAT, SCOL, DOOR, ACTI, FURN, CONT, MSTT, TERM など
    /// 3D モデル (MODL) を保持するすべての基本レコードを一括走査してマップを構築する。
    pub fn read_all_models_map(&mut self) -> io::Result<HashMap<FormId, BaseObjectInfo>> {
        let start_pos = 24 + self.header_record.data_size as u64;
        self.reader.seek(SeekFrom::Start(start_pos))?;

        let target_types = [
            REC_STAT, REC_SCOL, REC_DOOR, REC_ACTI,
            REC_FURN, REC_CONT, REC_MSTT, REC_TERM,
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

    /// 指定された EDID を持つ CELL レコードとその子 REFR レコード群を検索・取得する。
    pub fn find_cell_by_edid(&mut self, target_edid: &str) -> io::Result<Option<(CellRecord, Vec<RefrRecord>)>> {
        let start_pos = 24 + self.header_record.data_size as u64;
        self.reader.seek(SeekFrom::Start(start_pos))?;

        // 1. トップレベルの CELL グループを見つける
        let mut cell_group_end = 0u64;
        while let Some(entry) = self.read_next_entry()? {
            match entry {
                EsmEntry::Group(group) => {
                    if group.target_record_type() == Some(REC_CELL) {
                        cell_group_end = self.reader.stream_position()? + (group.group_size as u64 - GroupHeader::SIZE as u64);
                        break;
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

        if cell_group_end == 0 {
            return Ok(None);
        }

        // 2. CELL グループ内を再帰走査して target_edid に一致する CELL とその REFR を探す
        self.search_cell_in_stream(cell_group_end, target_edid)
    }

    fn search_cell_in_stream(&mut self, group_end: u64, target_edid: &str) -> io::Result<Option<(CellRecord, Vec<RefrRecord>)>> {
        let mut found_cell: Option<CellRecord> = None;
        let mut refrs = Vec::new();

        while self.reader.stream_position()? < group_end {
            let entry = match self.read_next_entry()? {
                Some(e) => e,
                None => break,
            };

            match entry {
                EsmEntry::Group(group) => {
                    let inner_end = self.reader.stream_position()? + (group.group_size as u64 - GroupHeader::SIZE as u64);
                    if let Some(ref cell) = found_cell {
                        let cell_id = cell.form_id.0;
                        let group_label_id = u32::from_le_bytes(group.label);
                        // 対象セルの子グループ (CellChildren=6, Persistent=8, Temporary=9)
                        if (group.group_type == 6 || group.group_type == 8 || group.group_type == 9) && group_label_id == cell_id {
                            self.collect_refrs_in_group(inner_end, &mut refrs)?;
                        } else {
                            // 子グループを抜けたので終了
                            return Ok(Some((cell.clone(), refrs)));
                        }
                    } else {
                        // まだセルが見つかっていない場合、再帰的に探索
                        if let Some(result) = self.search_cell_in_stream(inner_end, target_edid)? {
                            return Ok(Some(result));
                        }
                    }
                }
                EsmEntry::Record(header, subrecords) => {
                    if header.type_id == REC_CELL {
                        if let Some(cell) = found_cell.take() {
                            return Ok(Some((cell, refrs)));
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
            Ok(Some((cell, refrs)))
        } else {
            Ok(None)
        }
    }

    fn collect_refrs_in_group(&mut self, group_end: u64, refrs: &mut Vec<RefrRecord>) -> io::Result<()> {
        while self.reader.stream_position()? < group_end {
            let entry = match self.read_next_entry()? {
                Some(e) => e,
                None => break,
            };
            match entry {
                EsmEntry::Group(group) => {
                    let inner_end = self.reader.stream_position()? + (group.group_size as u64 - GroupHeader::SIZE as u64);
                    self.collect_refrs_in_group(inner_end, refrs)?;
                }
                EsmEntry::Record(header, subrecords) => {
                    if header.type_id == REC_REFR {
                        refrs.push(RefrRecord::from_record(&header, &subrecords)?);
                    }
                }
            }
        }
        Ok(())
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
}
