//! NBT chunk -> region file -> save blob -> disk bytes -> back again.

use mc_nbt::{nbt_io, CompoundTag, ListTag};
use mc_storage::{RegionFileCache, SaveFile};

fn make_chunk(x: i32, z: i32) -> CompoundTag {
    let mut level = CompoundTag::new();
    level.put_int("xPos", x);
    level.put_int("zPos", z);
    level.put_long("LastUpdate", 123_456_789);
    // 16*16*128 block ids, mostly stone with a few features: compresses well via RLE
    let blocks: Vec<u8> = (0..32768).map(|i| if i % 128 < 64 { 1 } else { (i % 7) as u8 }).collect();
    level.put_byte_array("Blocks", blocks);
    let mut ents = ListTag::new();
    let mut pig = CompoundTag::new();
    pig.put_string("id", "Pig");
    pig.put_float("Health", 10.0);
    ents.push(pig).unwrap();
    level.put_list("Entities", ents);
    let mut root = CompoundTag::new();
    root.put_compound("Level", level);
    root
}

#[test]
fn nbt_chunks_survive_the_whole_stack() {
    let mut save = SaveFile::new();
    let mut cache = RegionFileCache::new();

    // level.dat lives next to the regions
    let mut level_dat = CompoundTag::new();
    level_dat.put_long("RandomSeed", -4962768465676381896);
    let dat = save.create_file("level.dat").unwrap();
    save.write_file(dat, &nbt_io::to_bytes(&level_dat).unwrap()).unwrap();

    let coords = [(0, 0), (1, 0), (-1, -1), (40, 17)];
    for &(x, z) in &coords {
        let bytes = nbt_io::to_bytes(&make_chunk(x, z)).unwrap();
        cache.write_chunk(&mut save, "", x, z, false, &bytes).unwrap();
    }

    let disk = save.to_disk_bytes();
    let mut save2 = SaveFile::open(&disk).unwrap();
    let mut cache2 = RegionFileCache::new();

    let dat2 = save2.find_file("level.dat").unwrap();
    let back = nbt_io::from_bytes(save2.read_all(dat2).unwrap()).unwrap();
    assert_eq!(back.get_long("RandomSeed"), -4962768465676381896);

    for &(x, z) in &coords {
        let raw = cache2.read_chunk(&mut save2, "", x, z, false).unwrap().expect("chunk present");
        let tag = nbt_io::from_bytes(&raw).unwrap();
        assert_eq!(tag, make_chunk(x, z));
        let level = tag.get_compound("Level").unwrap();
        assert_eq!((level.get_int("xPos"), level.get_int("zPos")), (x, z));
    }
    assert!(cache2.read_chunk(&mut save2, "", 5, 5, false).unwrap().is_none());
}
