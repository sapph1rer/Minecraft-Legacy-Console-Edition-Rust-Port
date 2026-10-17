//! Port of `RegionFile.cpp` — a 32×32-chunk `.mcr` file stored *inside* a
//! [`SaveFile`].
//!
//! ```text
//! sector 0       1024 × u32   offsets   ((first_sector << 8) | sector_count), 0 = absent
//! sector 1       1024 × i32   timestamps (seconds)
//! sector 2..     chunks:      u32 comp_len | 0x8000_0000 (RLE flag)
//!                             u32 decompressed_len
//!                             comp_len bytes (zlib over RLE)
//! ```
//! Chunk (x, z) lives at index `x + z * 32`. Sectors are 4096 bytes.
//! This differs from Java's `.mcr` (which has a 5-byte chunk header).
//!
//! Unlike the C++ class, a `RegionFile` does not own a pointer to the save;
//! every operation takes `&mut SaveFile`.

use crate::compression;
use crate::error::{Result, StorageError};
use crate::save_file::{FileHandle, SaveFile, SeekFrom};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SECTOR_BYTES: usize = 4096;
pub const SECTOR_INTS: usize = SECTOR_BYTES / 4;
pub const CHUNK_HEADER_SIZE: usize = 8;
const RLE_FLAG: u32 = 0x8000_0000;

#[derive(Debug)]
pub struct RegionFile {
    name: String,
    handle: FileHandle,
    offsets: Vec<u32>,
    timestamps: Vec<i32>,
    sector_free: Vec<bool>,
    size_delta: i32,
    /// True until the first write: the file is not materialised (no header
    /// sectors) so that merely opening a region doesn't touch the save.
    is_empty: bool,
}

fn out_of_bounds(x: i32, z: i32) -> bool {
    !(0..32).contains(&x) || !(0..32).contains(&z)
}

fn index(x: i32, z: i32) -> usize {
    (x + z * 32) as usize
}

fn read_u32_le_or_be(save: &SaveFile, b: [u8; 4]) -> u32 {
    if save.is_save_endian_different() {
        u32::from_be_bytes(b)
    } else {
        u32::from_le_bytes(b)
    }
}

impl RegionFile {
    /// Opens (creating if necessary) the region file `name` inside the save.
    pub fn open(save: &mut SaveFile, name: &str) -> Result<RegionFile> {
        let handle = save.create_file(name)?;
        save.set_file_pointer(handle, 0, SeekFrom::End)?;

        let mut size_delta = 0i32;
        let is_empty = save.file_size(handle)? < SECTOR_BYTES as u32;
        if is_empty {
            size_delta += (SECTOR_BYTES * 2) as i32;
        }

        let size = save.file_size(handle)? as usize;
        if size & 0xfff != 0 {
            // not a multiple of 4 KiB: grow it
            save.zero_file(handle, 0x1000 - (size & 0xfff))?;
        }

        let n_sectors = if is_empty {
            2
        } else {
            (save.file_size(handle)? as usize / SECTOR_BYTES).max(2)
        };
        let mut sector_free = vec![true; n_sectors];
        sector_free[0] = false; // chunk offset table
        sector_free[1] = false; // timestamps

        let mut offsets = vec![0u32; SECTOR_INTS];
        let mut timestamps = vec![0i32; SECTOR_INTS];

        save.set_file_pointer(handle, 0, SeekFrom::Begin)?;
        if !is_empty {
            let mut raw = vec![0u8; SECTOR_BYTES * 2];
            // a file shorter than 8 KiB leaves the tail zeroed, like the original
            let got = save.read_file(handle, &mut raw)?;
            debug_assert!(got <= raw.len());
            for i in 0..SECTOR_INTS {
                let offset = read_u32_le_or_be(save, raw[i * 4..i * 4 + 4].try_into().unwrap());
                offsets[i] = offset;
                let first = (offset >> 8) as usize;
                let count = (offset & 0xFF) as usize;
                if offset != 0 && first + count <= sector_free.len() {
                    for s in 0..count {
                        sector_free[first + s] = false;
                    }
                }
                timestamps[i] = read_u32_le_or_be(
                    save,
                    raw[SECTOR_BYTES + i * 4..SECTOR_BYTES + i * 4 + 4].try_into().unwrap(),
                ) as i32;
            }
        }

        Ok(RegionFile {
            name: name.to_owned(),
            handle,
            offsets,
            timestamps,
            sector_free,
            size_delta,
            is_empty,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// How much the file has grown since the last call (`getSizeDelta`).
    pub fn take_size_delta(&mut self) -> i32 {
        std::mem::take(&mut self.size_delta)
    }

    pub fn has_chunk(&self, x: i32, z: i32) -> bool {
        !out_of_bounds(x, z) && self.offsets[index(x, z)] != 0
    }

    /// Seconds-since-epoch the chunk was last written (0 if never).
    pub fn timestamp(&self, x: i32, z: i32) -> i32 {
        if out_of_bounds(x, z) { 0 } else { self.timestamps[index(x, z)] }
    }

    /// Returns the *decompressed* chunk bytes, `None` if absent or invalid
    /// (out of bounds, bad sector range, impossible length), `Err` if the
    /// compressed payload itself is corrupt.
    pub fn read_chunk(&self, save: &mut SaveFile, x: i32, z: i32) -> Result<Option<Vec<u8>>> {
        if out_of_bounds(x, z) {
            return Ok(None);
        }
        let offset = self.offsets[index(x, z)];
        if offset == 0 {
            return Ok(None);
        }
        let sector = (offset >> 8) as usize;
        let num_sectors = (offset & 0xFF) as usize;
        if sector + num_sectors > self.sector_free.len() {
            return Ok(None);
        }

        compression::ensure_supported(save.platform())?;
        save.set_file_pointer(self.handle, (sector * SECTOR_BYTES) as i32, SeekFrom::Begin)?;

        let mut hdr = [0u8; CHUNK_HEADER_SIZE];
        if save.read_file(self.handle, &mut hdr)? < CHUNK_HEADER_SIZE {
            return Ok(None);
        }
        let mut length = read_u32_le_or_be(save, hdr[0..4].try_into().unwrap());
        let decomp_len = read_u32_le_or_be(save, hdr[4..8].try_into().unwrap()) as usize;
        let use_rle = length & RLE_FLAG != 0;
        length &= !RLE_FLAG;

        if length as usize > SECTOR_BYTES * num_sectors {
            return Ok(None);
        }
        let mut data = vec![0u8; length as usize];
        let got = save.read_file(self.handle, &mut data)?;
        if got < data.len() {
            return Ok(None);
        }

        let out = if use_rle {
            compression::decompress_rle_zlib(&data, decomp_len)?
        } else {
            compression::decompress_zlib_only(&data, decomp_len)?
        };
        Ok(Some(out))
    }

    /// Compresses and stores a chunk (`write(x, z, data, length)`).
    pub fn write_chunk(&mut self, save: &mut SaveFile, x: i32, z: i32, data: &[u8]) -> Result<()> {
        if out_of_bounds(x, z) {
            return Err(StorageError::Corrupt("chunk coordinates out of region bounds"));
        }
        let comp = compression::compress_rle_zlib(data);
        let sectors_needed = (comp.len() + CHUNK_HEADER_SIZE) / SECTOR_BYTES + 1;
        if sectors_needed >= 256 {
            // original silently drops the chunk; surface it instead
            return Err(StorageError::ChunkTooLarge { sectors: sectors_needed });
        }

        let offset = self.offsets[index(x, z)];
        let mut sector = (offset >> 8) as usize;
        let allocated = (offset & 0xFF) as usize;

        if sector != 0 && allocated == sectors_needed {
            // overwrite in place
            self.write_sector(save, sector, &comp, data.len())?;
        } else {
            // free what the chunk used before
            for i in 0..allocated {
                if let Some(f) = self.sector_free.get_mut(sector + i) {
                    *f = true;
                }
            }
            if allocated > 0 {
                // zero the unused area so the save compresses better until it is reused
                save.set_file_pointer(self.handle, (sector * SECTOR_BYTES) as i32, SeekFrom::Begin)?;
                save.zero_file(self.handle, SECTOR_BYTES * allocated)?;
            }

            // scan for a run of free sectors
            let mut run_start = self.sector_free.iter().position(|&f| f).unwrap_or(self.sector_free.len());
            let mut run_len = 0usize;
            for i in run_start..self.sector_free.len() {
                if run_len != 0 {
                    if self.sector_free[i] {
                        run_len += 1;
                    } else {
                        run_len = 0;
                    }
                } else if self.sector_free[i] {
                    run_start = i;
                    run_len = 1;
                }
                if run_len >= sectors_needed {
                    break;
                }
            }

            if run_len >= sectors_needed {
                sector = run_start;
                self.set_offset(save, x, z, ((sector << 8) | sectors_needed) as u32)?;
                for i in 0..sectors_needed {
                    self.sector_free[sector + i] = false;
                }
                self.write_sector(save, sector, &comp, data.len())?;
            } else {
                // no room: grow the file
                save.set_file_pointer(self.handle, 0, SeekFrom::End)?;
                sector = self.sector_free.len();
                let empty = [0u8; SECTOR_BYTES];
                for _ in 0..sectors_needed {
                    save.write_file(self.handle, &empty)?;
                    self.sector_free.push(false);
                }
                self.size_delta += (SECTOR_BYTES * sectors_needed) as i32;
                self.write_sector(save, sector, &comp, data.len())?;
                self.set_offset(save, x, z, ((sector << 8) | sectors_needed) as u32)?;
            }
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i32)
            .unwrap_or(0);
        self.set_timestamp(save, x, z, now)
    }

    fn write_sector(&self, save: &mut SaveFile, sector: usize, comp: &[u8], decomp_len: usize) -> Result<()> {
        save.set_file_pointer(self.handle, (sector * SECTOR_BYTES) as i32, SeekFrom::Begin)?;
        save.write_file(self.handle, &(comp.len() as u32 | RLE_FLAG).to_le_bytes())?;
        save.write_file(self.handle, &(decomp_len as u32).to_le_bytes())?;
        save.write_file(self.handle, comp)
    }

    /// The two header sectors are only written on the first real write.
    fn insert_initial_sectors(&mut self, save: &mut SaveFile) -> Result<()> {
        save.set_file_pointer(self.handle, 0, SeekFrom::Begin)?;
        let zero = [0u8; SECTOR_BYTES];
        save.write_file(self.handle, &zero)?;
        save.write_file(self.handle, &zero)?;
        self.is_empty = false;
        Ok(())
    }

    fn set_offset(&mut self, save: &mut SaveFile, x: i32, z: i32, offset: u32) -> Result<()> {
        if self.is_empty {
            self.insert_initial_sectors(save)?;
        }
        let i = index(x, z);
        self.offsets[i] = offset;
        save.set_file_pointer(self.handle, (i * 4) as i32, SeekFrom::Begin)?;
        save.write_file(self.handle, &offset.to_le_bytes())
    }

    fn set_timestamp(&mut self, save: &mut SaveFile, x: i32, z: i32, value: i32) -> Result<()> {
        if self.is_empty {
            self.insert_initial_sectors(save)?;
        }
        let i = index(x, z);
        self.timestamps[i] = value;
        save.set_file_pointer(self.handle, (SECTOR_BYTES + i * 4) as i32, SeekFrom::Begin)?;
        save.write_file(self.handle, &value.to_le_bytes())
    }

    /// `writeAllOffsets` — rewrite both tables (used after platform conversion).
    pub fn write_all_offsets(&self, save: &mut SaveFile) -> Result<()> {
        if self.is_empty {
            return Ok(());
        }
        let mut buf = Vec::with_capacity(SECTOR_BYTES * 2);
        for o in &self.offsets {
            buf.extend_from_slice(&o.to_le_bytes());
        }
        for t in &self.timestamps {
            buf.extend_from_slice(&t.to_le_bytes());
        }
        save.set_file_pointer(self.handle, 0, SeekFrom::Begin)?;
        save.write_file(self.handle, &buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(seed: u8, len: usize) -> Vec<u8> {
        // compressible but not trivially so
        (0..len).map(|i| if i % 13 < 8 { seed } else { (i as u8).wrapping_mul(seed) }).collect()
    }

    fn noise(len: usize, mut s: u64) -> Vec<u8> {
        (0..len)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                s as u8
            })
            .collect()
    }

    #[test]
    fn empty_region_does_not_touch_save() {
        let mut save = SaveFile::new();
        let mut r = RegionFile::open(&mut save, "r.0.0.mcr").unwrap();
        assert!(save.does_file_exist("r.0.0.mcr"));
        assert_eq!(save.file_size(save.find_file("r.0.0.mcr").unwrap()).unwrap(), 0);
        assert!(!r.has_chunk(0, 0));
        assert_eq!(r.read_chunk(&mut save, 0, 0).unwrap(), None);
        assert_eq!(r.take_size_delta(), 8192);
        assert_eq!(r.take_size_delta(), 0);
    }

    #[test]
    fn write_read_single_chunk() {
        let mut save = SaveFile::new();
        let mut r = RegionFile::open(&mut save, "r.0.0.mcr").unwrap();
        let data = chunk(5, 20_000);
        r.write_chunk(&mut save, 3, 4, &data).unwrap();
        assert!(r.has_chunk(3, 4));
        assert!(!r.has_chunk(4, 3));
        assert_eq!(r.read_chunk(&mut save, 3, 4).unwrap().unwrap(), data);
        assert!(r.timestamp(3, 4) > 1_600_000_000);
    }

    #[test]
    fn persists_through_save_roundtrip_and_reopen() {
        let mut save = SaveFile::new();
        let mut r = RegionFile::open(&mut save, "region/r.0.0.mcr").unwrap();
        let mut expected = Vec::new();
        for i in 0..40 {
            let (x, z) = (i % 32, i / 32 + (i % 3));
            let d = if i % 5 == 0 { noise(3000 + i as usize * 211, i as u64 + 1) } else { chunk(i as u8 + 1, 5000 + i as usize * 97) };
            r.write_chunk(&mut save, x, z, &d).unwrap();
            expected.push((x, z, d));
        }
        // serialise and reload the whole save, reopen the region
        let disk = save.to_disk_bytes();
        let mut save2 = SaveFile::open(&disk).unwrap();
        let r2 = RegionFile::open(&mut save2, "region/r.0.0.mcr").unwrap();
        for (x, z, d) in &expected {
            // later writes to the same (x,z) override; compare only final contents
            let last = expected.iter().rev().find(|(a, b, _)| a == x && b == z).unwrap();
            assert_eq!(r2.read_chunk(&mut save2, *x, *z).unwrap().unwrap(), last.2, "chunk {x},{z}");
            let _ = d;
        }
    }

    #[test]
    fn overwrite_same_size_stays_in_place_and_resize_moves() {
        let mut save = SaveFile::new();
        let mut r = RegionFile::open(&mut save, "r").unwrap();
        r.write_chunk(&mut save, 0, 0, &noise(6000, 1)).unwrap();
        let off1 = r.offsets[0];
        r.write_chunk(&mut save, 0, 0, &noise(6100, 2)).unwrap(); // same sector count
        assert_eq!(r.offsets[0], off1);
        r.write_chunk(&mut save, 1, 0, &noise(2000, 3)).unwrap();
        r.write_chunk(&mut save, 0, 0, &noise(30_000, 4)).unwrap(); // needs more sectors → moves
        assert_ne!(r.offsets[0], off1);
        assert_eq!(r.read_chunk(&mut save, 0, 0).unwrap().unwrap(), noise(30_000, 4));
        assert_eq!(r.read_chunk(&mut save, 1, 0).unwrap().unwrap(), noise(2000, 3));
        // freed sectors get reused by a small chunk
        r.write_chunk(&mut save, 5, 5, &noise(1000, 9)).unwrap();
        let first = (r.offsets[index(5, 5)] >> 8) as usize;
        assert!(first < r.sector_free.len() - 1);
        assert_eq!(r.read_chunk(&mut save, 5, 5).unwrap().unwrap(), noise(1000, 9));
    }

    #[test]
    fn size_delta_tracks_growth() {
        let mut save = SaveFile::new();
        let mut r = RegionFile::open(&mut save, "r").unwrap();
        assert_eq!(r.take_size_delta(), 8192);
        r.write_chunk(&mut save, 0, 0, &noise(5000, 1)).unwrap();
        let comp = compression::compress_rle_zlib(&noise(5000, 1));
        let sectors = (comp.len() + 8) / 4096 + 1;
        assert_eq!(r.take_size_delta(), (sectors * 4096) as i32);
    }

    #[test]
    fn too_large_and_out_of_bounds() {
        let mut save = SaveFile::new();
        let mut r = RegionFile::open(&mut save, "r").unwrap();
        let huge = noise(1_100_000, 7); // incompressible > 255 sectors
        assert!(matches!(r.write_chunk(&mut save, 0, 0, &huge), Err(StorageError::ChunkTooLarge { .. })));
        assert!(!r.has_chunk(0, 0));
        assert!(r.write_chunk(&mut save, 32, 0, b"x").is_err());
        assert_eq!(r.read_chunk(&mut save, -1, 0).unwrap(), None);
        assert!(!r.has_chunk(0, 99));
    }

    #[test]
    fn reads_hand_built_region() {
        // Build the bytes exactly as the format doc describes, without using write_chunk.
        let payload = chunk(3, 9000);
        let comp = compression::compress_rle_zlib(&payload);
        let mut file = vec![0u8; SECTOR_BYTES * 2];
        let sectors = (comp.len() + 8) / SECTOR_BYTES + 1;
        let idx = 7 + 2 * 32;
        file[idx * 4..idx * 4 + 4].copy_from_slice(&((2u32 << 8) | sectors as u32).to_le_bytes());
        file[SECTOR_BYTES + idx * 4..SECTOR_BYTES + idx * 4 + 4].copy_from_slice(&1234i32.to_le_bytes());
        file.extend_from_slice(&(comp.len() as u32 | 0x8000_0000).to_le_bytes());
        file.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        file.extend_from_slice(&comp);
        file.resize(SECTOR_BYTES * (2 + sectors), 0);

        let mut save = SaveFile::new();
        let h = save.create_file("r").unwrap();
        save.write_file(h, &file).unwrap();
        let r = RegionFile::open(&mut save, "r").unwrap();
        assert!(r.has_chunk(7, 2));
        assert_eq!(r.timestamp(7, 2), 1234);
        assert_eq!(r.read_chunk(&mut save, 7, 2).unwrap().unwrap(), payload);
    }

    #[test]
    fn plain_zlib_chunk_without_rle_flag() {
        let payload = chunk(9, 3000);
        let comp = compression::zlib_compress(&payload);
        let mut file = vec![0u8; SECTOR_BYTES * 2];
        file[0..4].copy_from_slice(&((2u32 << 8) | 1).to_le_bytes());
        file.extend_from_slice(&(comp.len() as u32).to_le_bytes()); // no RLE flag
        file.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        file.extend_from_slice(&comp);
        file.resize(SECTOR_BYTES * 3, 0);
        let mut save = SaveFile::new();
        let h = save.create_file("r").unwrap();
        save.write_file(h, &file).unwrap();
        let r = RegionFile::open(&mut save, "r").unwrap();
        assert_eq!(r.read_chunk(&mut save, 0, 0).unwrap().unwrap(), payload);
    }

    #[test]
    fn corrupt_entries_are_rejected_not_panics() {
        let mut file = vec![0u8; SECTOR_BYTES * 3];
        // chunk 0: sector range beyond the file
        file[0..4].copy_from_slice(&((50u32 << 8) | 4).to_le_bytes());
        // chunk 1: length larger than its sectors
        file[4..8].copy_from_slice(&((2u32 << 8) | 1).to_le_bytes());
        file[SECTOR_BYTES * 2..SECTOR_BYTES * 2 + 4].copy_from_slice(&(100_000u32 | 0x8000_0000).to_le_bytes());
        // chunk 2: garbage zlib
        file[8..12].copy_from_slice(&((2u32 << 8) | 1).to_le_bytes());
        let mut save = SaveFile::new();
        let h = save.create_file("r").unwrap();
        save.write_file(h, &file).unwrap();
        let r = RegionFile::open(&mut save, "r").unwrap();
        assert_eq!(r.read_chunk(&mut save, 0, 0).unwrap(), None);
        assert_eq!(r.read_chunk(&mut save, 1, 0).unwrap(), None);
        // chunk 2 shares sector 2 with chunk 1's bogus header → length check fails → None
        assert_eq!(r.read_chunk(&mut save, 2, 0).unwrap(), None);
    }

    #[test]
    fn unaligned_file_is_padded_on_open() {
        let mut save = SaveFile::new();
        let h = save.create_file("r").unwrap();
        save.write_file(h, &vec![0u8; 9000]).unwrap();
        let _ = RegionFile::open(&mut save, "r").unwrap();
        assert_eq!(save.file_size(h).unwrap() % 4096, 0);
        assert_eq!(save.file_size(h).unwrap(), 12288);
    }
}
