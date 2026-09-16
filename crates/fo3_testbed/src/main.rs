use fo3_esm::EsmReader;
use fo3_esm::types::FormId;
use byteorder::{LittleEndian, ByteOrder};

fn main() {
    let mut reader = EsmReader::open("A:/SteamLibrary/steamapps/common/Fallout 3 goty/Data/Fallout3.esm").unwrap();
    let mut pack_count = 0;
    let mut tnam_count = 0;
    let mut inam_count = 0;
    
    // Quick manual parse just to count
    while let Ok(rec) = reader.read_record() {
        if rec.type_id.0 == *b"PACK" {
            pack_count += 1;
            let subrecords = fo3_esm::parser::parse_subrecords(&rec.data).unwrap_or_default();
            for sub in subrecords {
                if sub.type_id.0 == *b"TNAM" {
                    tnam_count += 1;
                    if sub.data.len() == 4 {
                        println!("TNAM FormID: {:08X}", LittleEndian::read_u32(&sub.data));
                    } else {
                        println!("TNAM abnormal length: {}", sub.data.len());
                    }
                }
            }
        }
    }
    println!("Total PACK: {}, TNAMs found: {}", pack_count, tnam_count);
}
