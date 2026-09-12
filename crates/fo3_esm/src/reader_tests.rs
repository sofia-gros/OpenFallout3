#[cfg(test)]
mod tests {
    use crate::*;
    use crate::records::*;
    use crate::types::*;
    use crate::subrecord::parse_subrecords;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;
    use std::io::Cursor;

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
