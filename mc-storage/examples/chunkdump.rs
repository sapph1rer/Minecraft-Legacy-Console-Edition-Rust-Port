use mc_nbt::nbt_io;
use mc_storage::{RegionFileCache, SaveFile};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&a[1]).unwrap();
    let (x, z): (i32, i32) = (a[2].parse().unwrap(), a[3].parse().unwrap());
    let split = a.get(4).map(|s| s == "split").unwrap_or(false);
    let mut s = SaveFile::open(&bytes).unwrap();
    let mut c = RegionFileCache::new();
    match c.read_chunk(&mut s, "", x, z, split).unwrap() {
        None => println!("no chunk"),
        Some(raw) => {
            println!("{} raw bytes", raw.len());
            println!("{:02x?}", &raw[..64]); return;
        }
    }
}
