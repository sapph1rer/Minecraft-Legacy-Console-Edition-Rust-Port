//! Port of `RegionFileCache.cpp` — maps chunk coordinates to the right
//! `RegionFile`, keeping up to 256 open.
//!
//! Regions cover 32×32 chunks normally. Xbox One / PS4 saves (and Windows saves
//! with DLC-supplied level generation) use *split saves* with 16×16-chunk
//! regions; the C++ asks the global `LevelGenerationOptions` for the Windows
//! case, here the caller passes the answer in as `split_saves`.

use crate::error::Result;
use crate::file_header::SavePlatform;
use crate::region_file::RegionFile;
use crate::save_file::SaveFile;
use std::collections::HashMap;

pub const MAX_CACHE_SIZE: usize = 256;

#[derive(Debug, Default)]
pub struct RegionFileCache {
    cache: HashMap<String, RegionFile>,
}

/// `RegionFileCache::useSplitSaves`
pub fn use_split_saves(platform: SavePlatform, level_gen_from_dlc: bool) -> bool {
    match platform {
        SavePlatform::XboxOne | SavePlatform::Ps4 => true,
        SavePlatform::Win64 => level_gen_from_dlc,
        _ => false,
    }
}

impl RegionFileCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn shift(split: bool) -> (u32, i32) {
        if split { (4, 15) } else { (5, 31) }
    }

    /// File name of the region containing chunk (`chunk_x`, `chunk_z`).
    pub fn region_name(prefix: &str, chunk_x: i32, chunk_z: i32, split: bool) -> String {
        let (shift, _) = Self::shift(split);
        format!("{prefix}r.{}.{}.mcr", chunk_x >> shift, chunk_z >> shift)
    }

    /// `_getRegionFile`: opens on demand; drops (closes) *all* cached regions when full.
    pub fn get_region_file(
        &mut self,
        save: &mut SaveFile,
        prefix: &str,
        chunk_x: i32,
        chunk_z: i32,
        split: bool,
    ) -> Result<&mut RegionFile> {
        let name = Self::region_name(prefix, chunk_x, chunk_z, split);
        if !self.cache.contains_key(&name) {
            if self.cache.len() >= MAX_CACHE_SIZE {
                self.clear();
            }
            let rf = RegionFile::open(save, &name)?;
            self.cache.insert(name.clone(), rf);
        }
        Ok(self.cache.get_mut(&name).expect("just inserted"))
    }

    /// `_clear`
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// `_getSizeDelta`
    pub fn get_size_delta(
        &mut self,
        save: &mut SaveFile,
        prefix: &str,
        chunk_x: i32,
        chunk_z: i32,
        split: bool,
    ) -> Result<i32> {
        Ok(self.get_region_file(save, prefix, chunk_x, chunk_z, split)?.take_size_delta())
    }

    /// `_getChunkDataInputStream`, returning the decompressed bytes.
    pub fn read_chunk(
        &mut self,
        save: &mut SaveFile,
        prefix: &str,
        chunk_x: i32,
        chunk_z: i32,
        split: bool,
    ) -> Result<Option<Vec<u8>>> {
        let (_, mask) = Self::shift(split);
        let r = self.get_region_file(save, prefix, chunk_x, chunk_z, split)?;
        r.read_chunk(save, chunk_x & mask, chunk_z & mask)
    }

    /// `_getChunkDataOutputStream` + close: compresses and stores.
    pub fn write_chunk(
        &mut self,
        save: &mut SaveFile,
        prefix: &str,
        chunk_x: i32,
        chunk_z: i32,
        split: bool,
        data: &[u8],
    ) -> Result<()> {
        let (_, mask) = Self::shift(split);
        let r = self.get_region_file(save, prefix, chunk_x, chunk_z, split)?;
        r.write_chunk(save, chunk_x & mask, chunk_z & mask, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_names() {
        assert_eq!(RegionFileCache::region_name("", 0, 0, false), "r.0.0.mcr");
        assert_eq!(RegionFileCache::region_name("", 31, 32, false), "r.0.1.mcr");
        assert_eq!(RegionFileCache::region_name("DIM-1/", -1, -33, false), "DIM-1/r.-1.-2.mcr");
        assert_eq!(RegionFileCache::region_name("", 16, -1, true), "r.1.-1.mcr");
    }

    #[test]
    fn split_decision() {
        assert!(use_split_saves(SavePlatform::XboxOne, false));
        assert!(use_split_saves(SavePlatform::Ps4, false));
        assert!(!use_split_saves(SavePlatform::Win64, false));
        assert!(use_split_saves(SavePlatform::Win64, true));
        assert!(!use_split_saves(SavePlatform::X360, true));
    }

    #[test]
    fn routes_chunks_to_regions_including_negative_coords() {
        let mut save = SaveFile::new();
        let mut cache = RegionFileCache::new();
        let coords = [(0, 0), (31, 31), (32, 0), (-1, -1), (-32, 5), (-33, 5), (100, -100)];
        for (i, &(x, z)) in coords.iter().enumerate() {
            let data = vec![i as u8 + 1; 2000 + i * 100];
            cache.write_chunk(&mut save, "", x, z, false, &data).unwrap();
        }
        assert_eq!(cache.len(), 6); // (-32,5)/(−1,−1)... distinct regions
        for (i, &(x, z)) in coords.iter().enumerate() {
            let want = vec![i as u8 + 1; 2000 + i * 100];
            assert_eq!(cache.read_chunk(&mut save, "", x, z, false).unwrap().unwrap(), want, "{x},{z}");
        }
        assert_eq!(cache.read_chunk(&mut save, "", 1, 1, false).unwrap(), None);

        // survive a full save round trip with a fresh cache
        let disk = save.to_disk_bytes();
        let mut save2 = SaveFile::open(&disk).unwrap();
        let mut cache2 = RegionFileCache::new();
        for (i, &(x, z)) in coords.iter().enumerate() {
            let want = vec![i as u8 + 1; 2000 + i * 100];
            assert_eq!(cache2.read_chunk(&mut save2, "", x, z, false).unwrap().unwrap(), want);
        }
    }

    #[test]
    fn split_saves_use_16_chunk_regions() {
        let mut save = SaveFile::new();
        let mut cache = RegionFileCache::new();
        cache.write_chunk(&mut save, "", 15, 0, true, b"a").unwrap();
        cache.write_chunk(&mut save, "", 16, 0, true, b"b").unwrap();
        assert!(save.does_file_exist("r.0.0.mcr"));
        assert!(save.does_file_exist("r.1.0.mcr"));
        assert_eq!(cache.read_chunk(&mut save, "", 16, 0, true).unwrap().unwrap(), b"b");
    }

    #[test]
    fn cache_evicts_when_full() {
        let mut save = SaveFile::new();
        let mut cache = RegionFileCache::new();
        for i in 0..MAX_CACHE_SIZE as i32 {
            cache.get_region_file(&mut save, "", i * 32, 0, false).unwrap();
        }
        assert_eq!(cache.len(), MAX_CACHE_SIZE);
        cache.get_region_file(&mut save, "", 9999 * 32, 0, false).unwrap();
        assert_eq!(cache.len(), 1); // cleared, then the new one inserted
    }

    #[test]
    fn size_delta_through_cache() {
        let mut save = SaveFile::new();
        let mut cache = RegionFileCache::new();
        assert_eq!(cache.get_size_delta(&mut save, "", 0, 0, false).unwrap(), 8192);
        assert_eq!(cache.get_size_delta(&mut save, "", 0, 0, false).unwrap(), 0);
    }
}
