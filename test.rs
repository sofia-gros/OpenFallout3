use fo3_esm::FormId;

fn test_f64() {
    let f: f64 = 0x000290A7 as f64;
    println!("f64 as u32 = 0x{:08X}", f as u32);
}
test_f64();
