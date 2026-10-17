//! Reader for the console chunk format written by `OldChunkStorage::save(LevelChunk*, Level*, DataOutputStream*)`
//! (save file version >= `SAVE_FILE_VERSION_COMPRESSED_CHUNK_STORAGE`). All integers are big-endian,
//! the compressed tile index table is little-endian (as written on a little-endian platform).

use crate::chunk::LevelChunk;
use crate::storage::{Nibbles, Section};

pub const SAVE_FILE_VERSION_COMPRESSED_CHUNK_STORAGE: i16 = 8;
pub const SAVE_FILE_VERSION_CHUNK_INHABITED_TIME: i16 = 9;

pub struct ParsedChunk {
    pub version: i16,
    pub game_time: i64,
    pub chunk: LevelChunk,
    /// The trailing NBT blob (`TileEntities`, `Entities`, `TileTicks`), uncompressed, starting with the root tag.
    pub nbt_tail: Vec<u8>,
}

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.p + n > self.b.len() {
            return Err(format!("unexpected end of chunk data at {} (+{n})", self.p));
        }
        let s = &self.b[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    fn i16(&mut self) -> Result<i16, String> {
        Ok(i16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i64(&mut self) -> Result<i64, String> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }
}

/// `getIndex(block, tile)` of `CompressedTileStorage`.
#[inline]
fn tile_index(block: usize, tile: usize) -> usize {
    (((block & 0x180) << 6) | ((block & 0x060) << 4) | ((block & 0x01f) << 2)) | (((tile & 0x30) << 7) | ((tile & 0x0c) << 5) | (tile & 0x03))
}

/// `CompressedTileStorage::read` + `getData`: returns 32768 tiles in `x << 11 | z << 7 | y` order.
fn read_tiles(r: &mut Reader) -> Result<Vec<u8>, String> {
    let alloc = r.i32()?;
    let mut out = vec![0u8; 32768];
    if alloc <= 0 {
        return Ok(out);
    }
    let raw = r.take(alloc as usize)?;
    if raw.len() < 1024 {
        return Err("compressed tile storage smaller than its index".into());
    }
    let data = &raw[1024..];
    let get = |off: usize, len: usize| -> Result<&[u8], String> {
        data.get(off..off + len).ok_or_else(|| "compressed tile offset out of range".to_string())
    };
    for i in 0..512usize {
        let idx = u16::from_le_bytes([raw[2 * i], raw[2 * i + 1]]) as usize;
        let ty = idx & 3;
        if ty == 3 {
            if idx & 4 != 0 {
                let val = ((idx >> 8) & 0xff) as u8;
                for j in 0..64 {
                    out[tile_index(i, j)] = val;
                }
            } else {
                let off = (idx >> 1) & 0x7ffe;
                let packed = get(off, 64)?;
                for j in 0..64 {
                    out[tile_index(i, j)] = packed[j];
                }
            }
        } else {
            let bits = 1usize << ty; // 1, 2, 4
            let count = 1usize << bits; // 2, 4, 16
            let mask = count - 1;
            let indexshift = 3 - ty;
            let indexmask_bits = 7 >> ty;
            let indexmask_bytes = 62 >> indexshift;
            let off = (idx >> 1) & 0x7ffe;
            let blob = get(off, count + 64 * bits / 8)?;
            let (types, packed) = blob.split_at(count);
            for j in 0..64usize {
                let bi = (j >> indexshift) & indexmask_bytes;
                let bit = (j & indexmask_bits) * bits;
                out[tile_index(i, j)] = types[((packed[bi] >> bit) as usize) & mask];
            }
        }
    }
    Ok(out)
}

/// `SparseLightStorage` / `SparseDataStorage` `read` + `getData`: 16384 bytes in Java `DataLayer` order.
/// `all0`/`all15` are the special plane indices (128/129); the data storage only has `all0`.
fn read_sparse(r: &mut Reader) -> Result<Vec<u8>, String> {
    let count = r.i32()?;
    if !(0..=128).contains(&count) {
        return Err(format!("bad sparse plane count {count}"));
    }
    let blob = r.take(count as usize * 128 + 128)?;
    let (planes, data) = blob.split_at(128);
    let mut out = vec![0u8; 16384];
    for y in 0..128usize {
        let pi = planes[y] as usize;
        let shift = 4 * (y & 1);
        let base = y >> 1;
        if pi == 128 {
            continue;
        } else if pi == 129 {
            for xz in 0..256 {
                out[base + xz * 64] |= 15 << shift;
            }
        } else {
            if pi >= count as usize {
                return Err(format!("plane index {pi} >= count {count}"));
            }
            let plane = &data[pi * 128..pi * 128 + 128];
            for k in 0..128 {
                let lo = plane[k] & 15;
                let hi = plane[k] >> 4;
                out[base + (2 * k) * 64] |= lo << shift;
                out[base + (2 * k + 1) * 64] |= hi << shift;
            }
        }
    }
    Ok(out)
}

/// Parse one chunk record as stored in a region file (after region decompression).
pub fn parse_chunk(bytes: &[u8]) -> Result<ParsedChunk, String> {
    let mut r = Reader { b: bytes, p: 0 };
    let version = r.i16()?;
    if version < SAVE_FILE_VERSION_COMPRESSED_CHUNK_STORAGE {
        return Err(format!("chunk format version {version} (< 8) is not supported"));
    }
    let x = r.i32()?;
    let z = r.i32()?;
    let game_time = r.i64()?;
    let mut chunk = LevelChunk::empty(x, z);
    if version >= SAVE_FILE_VERSION_CHUNK_INHABITED_TIME {
        chunk.inhabited_time = r.i64()?;
    }
    let lower_blocks = read_tiles(&mut r)?;
    let upper_blocks = read_tiles(&mut r)?;
    let lower_data = read_sparse(&mut r)?;
    let upper_data = read_sparse(&mut r)?;
    let lower_sky = read_sparse(&mut r)?;
    let upper_sky = read_sparse(&mut r)?;
    let lower_bl = read_sparse(&mut r)?;
    let upper_bl = read_sparse(&mut r)?;
    chunk.heightmap.copy_from_slice(r.take(256)?);
    chunk.terrain_populated = r.i16()? as u16;
    chunk.biomes.copy_from_slice(r.take(256)?);
    let (lo, hi) = chunk.sections_mut();
    fill(lo, &lower_blocks, &lower_data, &lower_sky, &lower_bl);
    fill(hi, &upper_blocks, &upper_data, &upper_sky, &upper_bl);
    chunk.loaded = true;
    Ok(ParsedChunk { version, game_time, chunk, nbt_tail: bytes[r.p..].to_vec() })
}

fn fill(s: &mut Section, blocks: &[u8], data: &[u8], sky: &[u8], bl: &[u8]) {
    s.set_blocks(blocks);
    s.data = Nibbles::from_bytes(data, 0);
    s.sky = Nibbles::from_bytes(sky, 15);
    s.block_light = Nibbles::from_bytes(bl, 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn be32(v: &mut Vec<u8>, x: i32) {
        v.extend_from_slice(&x.to_be_bytes());
    }

    /// All-`tile` compressed storage: 512 zero-bit index entries and no data.
    fn uniform_tiles(v: &mut Vec<u8>, tile: u8) {
        be32(v, 1024);
        for _ in 0..512 {
            let idx: u16 = ((tile as u16) << 8) | 0b111;
            v.extend_from_slice(&idx.to_le_bytes());
        }
    }

    /// Sparse storage with `count` = 0 and every plane ALL_0 / ALL_15.
    fn uniform_sparse(v: &mut Vec<u8>, plane: u8) {
        be32(v, 0);
        v.extend(std::iter::repeat(plane).take(128));
    }

    #[test]
    fn parses_a_synthetic_chunk() {
        let mut v = Vec::new();
        v.extend_from_slice(&9i16.to_be_bytes());
        be32(&mut v, 3);
        be32(&mut v, -2);
        v.extend_from_slice(&1234i64.to_be_bytes());
        v.extend_from_slice(&99i64.to_be_bytes());
        uniform_tiles(&mut v, 1); // lower: stone
        uniform_tiles(&mut v, 0); // upper: air
        uniform_sparse(&mut v, 128); // data lower (all 0)
        uniform_sparse(&mut v, 128); // data upper
        uniform_sparse(&mut v, 129); // sky lower: all 15
        uniform_sparse(&mut v, 129); // sky upper
        uniform_sparse(&mut v, 128); // block light lower
        uniform_sparse(&mut v, 128); // block light upper
        v.extend(std::iter::repeat(64u8).take(256)); // heightmap
        v.extend_from_slice(&0x7feu16.to_be_bytes());
        v.extend(std::iter::repeat(4u8).take(256)); // biomes
        v.extend_from_slice(&[0x0a, 0, 0, 0]); // empty compound NBT
        let p = parse_chunk(&v).unwrap();
        assert_eq!((p.chunk.x, p.chunk.z, p.game_time, p.chunk.inhabited_time), (3, -2, 1234, 99));
        assert_eq!(p.chunk.get_tile(5, 100, 7), 1);
        assert_eq!(p.chunk.get_tile(5, 200, 7), 0);
        assert_eq!(p.chunk.heightmap[17], 64);
        assert_eq!(p.chunk.terrain_populated, 0x7fe);
        assert_eq!(p.chunk.biomes[255], 4);
        assert_eq!(p.chunk.get_brightness(crate::chunk::LightLayer::Sky, false, 5, 50, 7), 15);
        assert_eq!(p.nbt_tail.len(), 4);
    }

    #[test]
    fn tile_index_is_a_bijection() {
        let mut seen = vec![false; 32768];
        for b in 0..512 {
            for t in 0..64 {
                let i = tile_index(b, t);
                assert!(!seen[i]);
                seen[i] = true;
            }
        }
    }
}
