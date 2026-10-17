//! Writer for the console chunk format (`OldChunkStorage::save(LevelChunk*, Level*, DataOutputStream*)`),
//! the inverse of [`crate::chunk_io::parse_chunk`]. The compressed tile / sparse light encodings are
//! valid encodings that the game's readers accept; they are not guaranteed to be byte-identical with 4J's encoder
//! (palette order may differ), but they decode to identical data.

use crate::chunk::LevelChunk;
use crate::chunk_io::SAVE_FILE_VERSION_CHUNK_INHABITED_TIME;
use crate::tile_entity::{ItemStack, TileEntity};
use mc_nbt::{CompoundTag, ListTag};
use std::collections::HashMap;

#[inline]
fn tile_index(block: usize, tile: usize) -> usize {
    (((block & 0x180) << 6) | ((block & 0x060) << 4) | ((block & 0x01f) << 2)) | (((tile & 0x30) << 7) | ((tile & 0x0c) << 5) | (tile & 0x03))
}

/// `CompressedTileStorage::write`: 32768 tiles in `x << 11 | z << 7 | y` order.
fn write_tiles(out: &mut Vec<u8>, tiles: &[u8]) {
    let mut index = [0u16; 512];
    let mut data: Vec<u8> = Vec::new();
    for i in 0..512usize {
        let vals: Vec<u8> = (0..64).map(|j| tiles[tile_index(i, j)]).collect();
        let mut palette: Vec<u8> = Vec::new();
        for &v in &vals {
            if !palette.contains(&v) {
                palette.push(v);
                if palette.len() > 16 {
                    break;
                }
            }
        }
        if palette.len() == 1 {
            index[i] = ((palette[0] as u16) << 8) | 0b111;
            continue;
        }
        let off = data.len();
        debug_assert!(off % 2 == 0 && off <= 0x7ffe);
        if palette.len() > 16 {
            data.extend_from_slice(&vals);
            index[i] = ((off as u16) << 1) | 3;
        } else {
            let ty = match palette.len() {
                0..=2 => 0usize,
                3..=4 => 1,
                _ => 2,
            };
            let bits = 1usize << ty;
            let count = 1usize << bits;
            let indexshift = 3 - ty;
            let indexmask_bits = 7 >> ty;
            let indexmask_bytes = 62 >> indexshift;
            let mut types = vec![0u8; count];
            types[..palette.len()].copy_from_slice(&palette);
            let mut packed = vec![0u8; 64 * bits / 8];
            for (j, &v) in vals.iter().enumerate() {
                let pi = palette.iter().position(|&p| p == v).unwrap() as u8;
                let bi = (j >> indexshift) & indexmask_bytes;
                packed[bi] |= pi << ((j & indexmask_bits) * bits);
            }
            data.extend_from_slice(&types);
            data.extend_from_slice(&packed);
            index[i] = ((off as u16) << 1) | ty as u16;
        }
    }
    out.extend_from_slice(&((1024 + data.len()) as i32).to_be_bytes());
    for v in index {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&data);
}

/// `SparseLightStorage::write` (`allow_all15`) / `SparseDataStorage::write` from Java-order nibbles.
fn write_sparse(out: &mut Vec<u8>, nibbles: &[u8], allow_all15: bool) {
    let mut planes = [0u8; 128];
    let mut uniq: Vec<Vec<u8>> = Vec::new();
    let mut seen: HashMap<Vec<u8>, usize> = HashMap::new();
    for y in 0..128usize {
        let shift = 4 * (y & 1);
        let base = y >> 1;
        let mut plane = vec![0u8; 128];
        for k in 0..128 {
            let lo = (nibbles[base + (2 * k) * 64] >> shift) & 15;
            let hi = (nibbles[base + (2 * k + 1) * 64] >> shift) & 15;
            plane[k] = lo | (hi << 4);
        }
        if plane.iter().all(|&b| b == 0) {
            planes[y] = 128;
        } else if allow_all15 && plane.iter().all(|&b| b == 0xff) {
            planes[y] = 129;
        } else {
            let n = uniq.len();
            let id = *seen.entry(plane.clone()).or_insert_with(|| {
                uniq.push(plane);
                n
            });
            planes[y] = id as u8;
        }
    }
    out.extend_from_slice(&(uniq.len() as i32).to_be_bytes());
    out.extend_from_slice(&planes);
    for p in uniq {
        out.extend_from_slice(&p);
    }
}

pub fn item_tag(slot: Option<usize>, it: &ItemStack) -> CompoundTag {
    let mut t = CompoundTag::new();
    if let Some(s) = slot {
        t.put_byte("Slot", s as i8);
    }
    t.put_short("id", it.id as i16);
    t.put_byte("Count", it.count as i8);
    t.put_short("Damage", it.damage as i16);
    if !it.enchantments.is_empty() {
        let mut list = ListTag::new();
        for &(id, lvl) in &it.enchantments {
            let mut e = CompoundTag::new();
            e.put_short("id", id as i16);
            e.put_short("lvl", lvl as i16);
            list.push(e).unwrap();
        }
        let mut tag = CompoundTag::new();
        tag.put_list("StoredEnchantments", list);
        t.put_compound("tag", tag);
    }
    t
}

fn items_list(items: &[Option<ItemStack>]) -> ListTag {
    let mut l = ListTag::new();
    for (i, it) in items.iter().enumerate() {
        if let Some(it) = it {
            l.push(item_tag(Some(i), it)).unwrap();
        }
    }
    l
}

/// `TileEntity::save` + the subclass part.
pub fn tile_entity_tag(pos: (i32, i32, i32), te: &TileEntity) -> CompoundTag {
    let mut t = CompoundTag::new();
    let id = match te {
        TileEntity::Chest { .. } => "Chest",
        TileEntity::Spawner { .. } => "MobSpawner",
        TileEntity::Dispenser { .. } => "Trap",
        TileEntity::Sign { .. } => "Sign",
    };
    t.put_string("id", id);
    t.put_int("x", pos.0);
    t.put_int("y", pos.1);
    t.put_int("z", pos.2);
    match te {
        TileEntity::Chest { items } => {
            t.put_list("Items", items_list(items));
            t.put_boolean("bonus", false);
        }
        TileEntity::Dispenser { items } => t.put_list("Items", items_list(items)),
        TileEntity::Sign { lines } => {
            for (i, l) in lines.iter().enumerate() {
                t.put_string(&format!("Text{}", i + 1), l);
            }
        }
        TileEntity::Spawner { entity_id } => {
            // `BaseMobSpawner::save` with the constructor defaults
            t.put_string("EntityId", entity_id);
            t.put_short("Delay", 20);
            t.put_short("MinSpawnDelay", 200);
            t.put_short("MaxSpawnDelay", 800);
            t.put_short("SpawnCount", 4);
            t.put_short("MaxNearbyEntities", 6);
            t.put_short("RequiredPlayerRange", 16);
            t.put_short("SpawnRange", 4);
        }
    }
    t
}

fn read_item(t: &CompoundTag) -> ItemStack {
    let mut enchantments = Vec::new();
    if let Some(tag) = t.get_compound("tag") {
        if let Some(l) = tag.get_list("StoredEnchantments") {
            for e in l.iter() {
                if let mc_nbt::Tag::Compound(c) = e {
                    enchantments.push((c.get_short("id") as u8, c.get_short("lvl") as u8));
                }
            }
        }
    }
    ItemStack { id: t.get_short("id") as i32, count: t.get_byte("Count") as i32, damage: t.get_short("Damage") as i32, enchantments }
}

fn read_items(t: &CompoundTag, size: usize) -> Vec<Option<ItemStack>> {
    let mut items = vec![None; size];
    if let Some(l) = t.get_list("Items") {
        for e in l.iter() {
            if let mc_nbt::Tag::Compound(c) = e {
                let slot = c.get_byte("Slot") as usize;
                if slot < size {
                    items[slot] = Some(read_item(c));
                }
            }
        }
    }
    items
}

/// The tile entities stored in a chunk record's trailing NBT (`LevelChunk::load` → `TileEntity::loadStatic`).
pub fn read_tile_entities(tail: &[u8]) -> Vec<((i32, i32, i32), TileEntity)> {
    let Ok(root) = mc_nbt::nbt_io::from_bytes(tail) else { return Vec::new() };
    let mut out = Vec::new();
    let Some(list) = root.get_list("TileEntities") else { return out };
    for e in list.iter() {
        let mc_nbt::Tag::Compound(c) = e else { continue };
        let pos = (c.get_int("x"), c.get_int("y"), c.get_int("z"));
        let te = match c.get_string("id").as_str() {
            "Chest" => TileEntity::Chest { items: read_items(c, crate::tile_entity::CHEST_SIZE) },
            "Trap" => TileEntity::Dispenser { items: read_items(c, crate::tile_entity::DISPENSER_SIZE) },
            "Sign" => TileEntity::Sign { lines: [c.get_string("Text1"), c.get_string("Text2"), c.get_string("Text3"), c.get_string("Text4")] },
            "MobSpawner" => TileEntity::Spawner { entity_id: c.get_string("EntityId") },
            _ => continue,
        };
        out.push((pos, te));
    }
    out
}

/// A scheduled tick of this chunk: tile id, position and delay relative to the level time.
pub struct SavedTick {
    pub tile: i32,
    pub pos: (i32, i32, i32),
    pub delay: i32,
}

/// Serialise one chunk record (not yet region-compressed).
pub fn write_chunk(
    chunk: &LevelChunk,
    game_time: i64,
    tile_entities: &[((i32, i32, i32), &TileEntity)],
    ticks: &[SavedTick],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(64 * 1024);
    out.extend_from_slice(&SAVE_FILE_VERSION_CHUNK_INHABITED_TIME.to_be_bytes());
    out.extend_from_slice(&chunk.x.to_be_bytes());
    out.extend_from_slice(&chunk.z.to_be_bytes());
    out.extend_from_slice(&game_time.to_be_bytes());
    out.extend_from_slice(&chunk.inhabited_time.to_be_bytes());
    write_tiles(&mut out, &chunk.lower_blocks());
    write_tiles(&mut out, &chunk.upper_blocks());
    let (dl, du) = chunk.data_bytes();
    write_sparse(&mut out, &dl, false);
    write_sparse(&mut out, &du, false);
    let (sl, su) = chunk.sky_light_bytes();
    write_sparse(&mut out, &sl, true);
    write_sparse(&mut out, &su, true);
    let (bl, bu) = chunk.block_light_bytes();
    write_sparse(&mut out, &bl, true);
    write_sparse(&mut out, &bu, true);
    out.extend_from_slice(&chunk.heightmap);
    out.extend_from_slice(&chunk.terrain_populated.to_be_bytes());
    out.extend_from_slice(&chunk.biomes);

    let mut root = CompoundTag::new();
    let mut tes = ListTag::new();
    for (pos, te) in tile_entities {
        tes.push(tile_entity_tag(*pos, te)).unwrap();
    }
    root.put_list("TileEntities", tes);
    if !ticks.is_empty() {
        let mut tl = ListTag::new();
        for t in ticks {
            let mut c = CompoundTag::new();
            c.put_int("i", t.tile);
            c.put_int("x", t.pos.0);
            c.put_int("y", t.pos.1);
            c.put_int("z", t.pos.2);
            c.put_int("t", t.delay);
            tl.push(c).unwrap();
        }
        root.put_list("TileTicks", tl);
    }
    mc_nbt::nbt_io::write(&root, &mut out).expect("nbt write to Vec");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk_io::parse_chunk;
    use mc_world::tile_ids as t;

    fn noisy_chunk() -> LevelChunk {
        let mut blocks = vec![0u8; 32768];
        let mut s = 12345u32;
        for x in 0..16usize {
            for z in 0..16usize {
                for y in 0..70usize {
                    s = s.wrapping_mul(1664525).wrapping_add(1013904223);
                    blocks[(x << 11) | (z << 7) | y] = match (s >> 24) % 7 {
                        0 => t::STONE,
                        1 => t::DIRT,
                        2 | 3 => t::STONE,
                        4 => (s >> 16) as u8, // many distinct ids -> raw 8-bit blocks
                        _ => 0,
                    };
                }
            }
        }
        let mut c = LevelChunk::from_blocks(3, -2, &blocks);
        c.recalc_heightmap(false);
        c.set_data_raw(4, 10, 4, 9);
        c.set_data_raw(5, 11, 4, 3);
        c.inhabited_time = 77;
        c.terrain_populated = 0x7fe;
        c.biomes[9] = 4;
        c
    }

    #[test]
    fn write_then_parse_roundtrips() {
        let c = noisy_chunk();
        let chest = TileEntity::new_chest();
        let bytes = write_chunk(&c, 4321, &[((50, 20, -30), &chest)], &[SavedTick { tile: 8, pos: (1, 2, 3), delay: 5 }]);
        let p = parse_chunk(&bytes).unwrap();
        assert_eq!((p.chunk.x, p.chunk.z, p.game_time, p.chunk.inhabited_time), (3, -2, 4321, 77));
        assert_eq!(p.chunk.lower_blocks(), c.lower_blocks());
        assert_eq!(p.chunk.upper_blocks(), c.upper_blocks());
        assert_eq!(p.chunk.data_bytes(), c.data_bytes());
        assert_eq!(p.chunk.sky_light_bytes(), c.sky_light_bytes());
        assert_eq!(p.chunk.block_light_bytes(), c.block_light_bytes());
        assert_eq!(p.chunk.heightmap, c.heightmap);
        assert_eq!(p.chunk.biomes, c.biomes);
        assert_eq!(p.chunk.terrain_populated, 0x7fe);
        let tail = mc_nbt::nbt_io::from_bytes(&p.nbt_tail).unwrap();
        assert_eq!(tail.get_list("TileEntities").unwrap().len(), 1);
        assert_eq!(tail.get_list("TileTicks").unwrap().len(), 1);
    }

    #[test]
    fn items_and_spawner_tags() {
        let mut items = vec![None; 27];
        items[3] = Some(ItemStack { id: 403, count: 1, damage: 0, enchantments: vec![(16, 2)] });
        let tag = tile_entity_tag((1, 2, 3), &TileEntity::Chest { items });
        let l = tag.get_list("Items").unwrap();
        assert_eq!(l.len(), 1);
        let sp = tile_entity_tag((0, 0, 0), &TileEntity::Spawner { entity_id: "Blaze".into() });
        assert_eq!(sp.get_string("EntityId"), "Blaze");
        assert_eq!(sp.get_short("MaxSpawnDelay"), 800);
    }

    #[test]
    fn tile_entities_roundtrip_through_a_chunk_record() {
        let mut items = vec![None; 27];
        items[3] = Some(ItemStack { id: 264, count: 5, damage: 0, enchantments: vec![] });
        items[26] = Some(ItemStack { id: 403, count: 1, damage: 0, enchantments: vec![(16, 2)] });
        let chunk = LevelChunk::empty(0, 0);
        let te = TileEntity::Chest { items };
        let sp = TileEntity::Spawner { entity_id: "Zombie".into() };
        let bytes = write_chunk(&chunk, 0, &[((1, 2, 3), &te), ((4, 5, 6), &sp)], &[]);
        let p = crate::chunk_io::parse_chunk(&bytes).unwrap();
        let back = read_tile_entities(&p.nbt_tail);
        assert_eq!(back, vec![((1, 2, 3), te), ((4, 5, 6), sp)]);
    }
}
