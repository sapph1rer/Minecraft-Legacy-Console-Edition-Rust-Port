use mc_storage::{RegionFileCache, SaveFile};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&a[1]).unwrap();
    let prefix = a.get(2).cloned().unwrap_or_default();
    let split = a.get(3).map(|s| s == "split").unwrap_or(false);
    let mut s = SaveFile::open(&bytes).unwrap();
    let mut c = RegionFileCache::new();
    let (mut minx, mut maxx, mut minz, mut maxz, mut n) = (999, -999, 999, -999, 0);
    for x in -64..64 { for z in -64..64 {
        if let Ok(Some(_)) = c.read_chunk(&mut s, &prefix, x, z, split) {
            n += 1; minx = minx.min(x); maxx = maxx.max(x); minz = minz.min(z); maxz = maxz.max(z);
        }
    }}
    println!("{prefix} split={split}: {n} chunks x[{minx},{maxx}] z[{minz},{maxz}]");
}
