use mc_nbt::nbt_io;
use mc_storage::SaveFile;
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let bytes = std::fs::read(p).unwrap();
    let mut s = SaveFile::open(&bytes).unwrap();
    println!("platform {:?} version {} orig {}", s.platform(), s.save_version(), s.original_save_version());
    for e in s.header().entries() {
        println!("{:40} size {:>9} region={}", e.name, e.file_size(), e.is_region_file());
    }
    if let Some(h) = s.find_file("level.dat") {
        let raw = s.read_all(h).unwrap().to_vec();
        match nbt_io::from_bytes(&raw) {
            Ok(t) => println!("{:#?}", t),
            Err(e) => println!("level.dat parse error {e:?}; first bytes {:02x?}", &raw[..raw.len().min(32)]),
        }
    }
}
