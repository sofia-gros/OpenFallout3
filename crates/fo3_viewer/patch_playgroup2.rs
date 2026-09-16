use std::fs;

fn main() {
    let mut content = fs::read_to_string("crates/fo3_viewer/src/action.rs").unwrap();
    content = content.replace("fo3_nif::parse_nif(&buf)", "fo3_nif::NifFile::read(&mut std::io::Cursor::new(&buf))");
    fs::write("crates/fo3_viewer/src/action.rs", content).unwrap();
}
