//! Port of `FileHeader.h/.cpp` — the file table that lives at the *end* of a
//! save blob.
//!
//! Blob layout (offsets in bytes):
//! ```text
//! 0   u32  header_offset   where the file table starts
//! 4   u32  header_size     number of entries (v2+) / byte length (v1)
//! 8   i16  original_version
//! 10  i16  version
//! 12  ...  file data, files stored back to back
//! header_offset  entries[]  (144 bytes each for v2+, 136 for v1)
//! ```
//! Entry (v2): `u16 filename[64]` (NUL padded UTF-16), `u32 length`,
//! `u32 start_offset`, `i64 last_modified_ms`.

use crate::error::{Result, StorageError};

pub const SAVE_FILE_HEADER_SIZE: usize = 12;
pub const ENTRY_SIZE_V1: usize = 136;
pub const ENTRY_SIZE_V2: usize = 144;
pub const MAX_NAME_UNITS: usize = 64;

/// `ESaveVersions`
pub mod version {
    pub const PRE_LAUNCH: i16 = 1;
    pub const LAUNCH: i16 = 2;
    pub const POST_LAUNCH: i16 = 3;
    pub const NEW_END: i16 = 4;
    pub const MOVED_STRONGHOLD: i16 = 5;
    pub const CHANGE_MAP_DATA_MAPPING_SIZE: i16 = 6;
    pub const DURANGO_CHANGE_MAP_DATA_MAPPING_SIZE: i16 = 7;
    pub const COMPRESSED_CHUNK_STORAGE: i16 = 8;
    pub const CHUNK_INHABITED_TIME: i16 = 9;
    /// `SAVE_FILE_VERSION_NUMBER` (= NEXT - 1)
    pub const CURRENT: i16 = CHUNK_INHABITED_TIME;
}

const fn fourcc(a: u8, b: u8, c: u8, d: u8) -> u32 {
    // Windows MAKE_FOURCC packs the first character in the low byte
    (a as u32) | ((b as u32) << 8) | ((c as u32) << 16) | ((d as u32) << 24)
}

/// `ESavePlatform`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SavePlatform {
    None,
    X360,
    XboxOne,
    Ps3,
    Ps4,
    PsVita,
    Win64,
}

impl SavePlatform {
    pub const LOCAL: SavePlatform = SavePlatform::Win64;

    pub fn fourcc(self) -> u32 {
        match self {
            SavePlatform::None => fourcc(b'N', b'O', b'N', b'E'),
            SavePlatform::X360 => fourcc(b'X', b'3', b'6', b'0'),
            SavePlatform::XboxOne => fourcc(b'X', b'B', b'1', b'_'),
            SavePlatform::Ps3 => fourcc(b'P', b'S', b'3', b'_'),
            SavePlatform::Ps4 => fourcc(b'P', b'S', b'4', b'_'),
            SavePlatform::PsVita => fourcc(b'P', b'S', b'V', b'_'),
            SavePlatform::Win64 => fourcc(b'W', b'I', b'N', b'_'),
        }
    }

    pub fn from_fourcc(v: u32) -> Option<SavePlatform> {
        [
            SavePlatform::None,
            SavePlatform::X360,
            SavePlatform::XboxOne,
            SavePlatform::Ps3,
            SavePlatform::Ps4,
            SavePlatform::PsVita,
            SavePlatform::Win64,
        ]
        .into_iter()
        .find(|p| p.fourcc() == v)
    }

    /// Byte order of data stored by that platform (`FileHeader::getEndian`).
    pub fn endian(self) -> ByteOrder {
        match self {
            SavePlatform::X360 | SavePlatform::Ps3 => ByteOrder::Big,
            _ => ByteOrder::Little,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    Little,
    Big,
}

pub const LOCAL_ENDIAN: ByteOrder = ByteOrder::Little;

impl ByteOrder {
    pub fn u32(self, b: [u8; 4]) -> u32 {
        match self {
            ByteOrder::Little => u32::from_le_bytes(b),
            ByteOrder::Big => u32::from_be_bytes(b),
        }
    }
    pub fn i16(self, b: [u8; 2]) -> i16 {
        match self {
            ByteOrder::Little => i16::from_le_bytes(b),
            ByteOrder::Big => i16::from_be_bytes(b),
        }
    }
    pub fn i64(self, b: [u8; 8]) -> i64 {
        match self {
            ByteOrder::Little => i64::from_le_bytes(b),
            ByteOrder::Big => i64::from_be_bytes(b),
        }
    }
    pub fn u16(self, b: [u8; 2]) -> u16 {
        match self {
            ByteOrder::Little => u16::from_le_bytes(b),
            ByteOrder::Big => u16::from_be_bytes(b),
        }
    }
}

/// One file in the save (`FileEntry` + `FileEntrySaveDataV2`).
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    /// Length in bytes.
    pub length: u32,
    /// Absolute offset of the first byte inside the blob.
    pub start_offset: u32,
    /// Milliseconds since the Unix epoch (0 for v1 saves).
    pub last_modified_time: i64,
    pub(crate) current_pointer: u32,
    pub(crate) id: u64,
}

impl FileEntry {
    pub fn file_size(&self) -> u32 {
        self.length
    }
    /// Region entries of split saves have an empty name; never stored in a normal table.
    pub fn is_region_file(&self) -> bool {
        self.name.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct FileHeader {
    pub(crate) entries: Vec<FileEntry>,
    pub platform: SavePlatform,
    pub endian: ByteOrder,
    pub save_version: i16,
    pub original_save_version: i16,
    next_id: u64,
}

impl Default for FileHeader {
    fn default() -> Self {
        FileHeader {
            entries: Vec::new(),
            platform: SavePlatform::LOCAL,
            endian: LOCAL_ENDIAN,
            // ctor: m_saveVersion = 0, original = latest
            save_version: 0,
            original_save_version: version::CURRENT,
            next_id: 1,
        }
    }
}

fn encode_name(name: &str) -> Result<[u16; MAX_NAME_UNITS]> {
    let units: Vec<u16> = name.encode_utf16().collect();
    if units.len() >= MAX_NAME_UNITS {
        return Err(StorageError::InvalidFileName);
    }
    let mut out = [0u16; MAX_NAME_UNITS];
    out[..units.len()].copy_from_slice(&units);
    Ok(out)
}

fn decode_name(units: &[u16]) -> String {
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}

impl FileHeader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    /// `GetStartOfNextData` — end of the data area (= where the table is written).
    pub fn start_of_next_data(&self) -> u32 {
        SAVE_FILE_HEADER_SIZE as u32 + self.entries.iter().map(|e| e.length).sum::<u32>()
    }

    /// `GetFileSize` — total blob size including the table.
    pub fn file_size(&self) -> u32 {
        self.start_of_next_data() + (ENTRY_SIZE_V2 * self.entries.len()) as u32
    }

    pub(crate) fn index_of(&self, name: &str) -> Option<usize> {
        self.entries.iter().position(|e| e.name == name)
    }

    pub fn file_exists(&self, name: &str) -> bool {
        self.index_of(name).is_some()
    }

    /// `AddFile`: returns the existing entry's index if the name is known.
    pub(crate) fn add_file(&mut self, name: &str) -> Result<usize> {
        encode_name(name)?; // validates length
        if let Some(i) = self.index_of(name) {
            return Ok(i);
        }
        let start = self.start_of_next_data();
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(FileEntry {
            name: name.to_owned(),
            length: 0,
            start_offset: start,
            last_modified_time: 0,
            current_pointer: start,
            id,
        });
        Ok(self.entries.len() - 1)
    }

    /// `AdjustStartOffsets` — shift every file after `index`.
    pub(crate) fn adjust_start_offsets(&mut self, index: usize, bytes: u32, subtract: bool) {
        for e in &mut self.entries[index + 1..] {
            if subtract {
                e.start_offset -= bytes;
                e.current_pointer -= bytes;
            } else {
                e.start_offset += bytes;
                e.current_pointer += bytes;
            }
        }
    }

    /// `RemoveFile` (offset fix-up + removal; the caller removes the data bytes).
    pub(crate) fn remove_file(&mut self, index: usize) {
        let len = self.entries[index].length;
        self.adjust_start_offsets(index, len, true);
        self.entries.remove(index);
    }

    pub fn files_with_prefix(&self, prefix: &str) -> Vec<&FileEntry> {
        self.entries.iter().filter(|e| e.name.starts_with(prefix)).collect()
    }

    /// `WriteHeader`: fills the 12-byte preamble of `blob[..12]` and appends
    /// the table at `start_of_next_data`. `blob` must be exactly the data area
    /// (`start_of_next_data` bytes); the table is appended. Always little-endian
    /// (local), like the original.
    pub fn write_into(&self, blob: &mut Vec<u8>) {
        let header_offset = self.start_of_next_data();
        debug_assert_eq!(blob.len(), header_offset as usize);
        blob[0..4].copy_from_slice(&header_offset.to_le_bytes());
        blob[4..8].copy_from_slice(&(self.entries.len() as u32).to_le_bytes());
        blob[8..10].copy_from_slice(&self.original_save_version.to_le_bytes());
        blob[10..12].copy_from_slice(&version::CURRENT.to_le_bytes());
        for e in &self.entries {
            let name = encode_name(&e.name).expect("validated at add_file");
            for u in name {
                blob.extend_from_slice(&u.to_le_bytes());
            }
            blob.extend_from_slice(&e.length.to_le_bytes());
            blob.extend_from_slice(&e.start_offset.to_le_bytes());
            blob.extend_from_slice(&e.last_modified_time.to_le_bytes());
        }
    }

    /// `ReadHeader`. Returns the header and the length of the data area
    /// (= `header_offset`).
    pub fn read_from(blob: &[u8], platform: SavePlatform) -> Result<(FileHeader, usize)> {
        if platform == SavePlatform::None {
            return Err(StorageError::Corrupt("unknown save platform"));
        }
        let endian = platform.endian();
        if blob.len() < SAVE_FILE_HEADER_SIZE {
            return Err(StorageError::Corrupt("save shorter than its preamble"));
        }
        let rd32 = |o: usize| endian.u32(blob[o..o + 4].try_into().unwrap());
        let header_offset = rd32(0) as usize;
        let header_size = rd32(4) as usize;
        let original_version = endian.i16(blob[8..10].try_into().unwrap());
        let save_version = endian.i16(blob[10..12].try_into().unwrap());

        if header_offset < SAVE_FILE_HEADER_SIZE || header_offset > blob.len() {
            return Err(StorageError::Corrupt("file table offset out of range"));
        }
        let table = &blob[header_offset..];

        let mut h = FileHeader {
            platform,
            endian,
            save_version,
            original_save_version: original_version,
            ..FileHeader::default()
        };

        let push = |h: &mut FileHeader, name: String, length: u32, start: u32, ts: i64| {
            let id = h.next_id;
            h.next_id += 1;
            h.entries.push(FileEntry {
                name,
                length,
                start_offset: start,
                last_modified_time: ts,
                current_pointer: start,
                id,
            });
        };

        match save_version {
            version::LAUNCH..=version::CHUNK_INHABITED_TIME => {
                if header_size.checked_mul(ENTRY_SIZE_V2).map_or(true, |n| n > table.len()) {
                    return Err(StorageError::Corrupt("file table truncated"));
                }
                for i in 0..header_size {
                    let e = &table[i * ENTRY_SIZE_V2..(i + 1) * ENTRY_SIZE_V2];
                    let units: Vec<u16> = (0..MAX_NAME_UNITS)
                        .map(|k| endian.u16(e[k * 2..k * 2 + 2].try_into().unwrap()))
                        .collect();
                    let length = endian.u32(e[128..132].try_into().unwrap());
                    let start = endian.u32(e[132..136].try_into().unwrap());
                    let ts = endian.i64(e[136..144].try_into().unwrap());
                    push(&mut h, decode_name(&units), length, start, ts);
                }
            }
            version::PRE_LAUNCH => {
                // here header_size is a BYTE count; entries have no timestamp and the
                // original did no byte swapping for this legacy layout
                let mut i = 0;
                while i < header_size {
                    let e = table
                        .get(i..i + ENTRY_SIZE_V1)
                        .ok_or(StorageError::Corrupt("v1 file table truncated"))?;
                    let units: Vec<u16> = (0..MAX_NAME_UNITS)
                        .map(|k| u16::from_le_bytes(e[k * 2..k * 2 + 2].try_into().unwrap()))
                        .collect();
                    let length = u32::from_le_bytes(e[128..132].try_into().unwrap());
                    let start = u32::from_le_bytes(e[132..136].try_into().unwrap());
                    push(&mut h, decode_name(&units), length, start, 0);
                    i += ENTRY_SIZE_V1;
                }
            }
            v => return Err(StorageError::UnsupportedVersion(v)),
        }

        for e in &h.entries {
            if e.start_offset as usize + e.length as usize > header_offset {
                return Err(StorageError::Corrupt("file extends past data area"));
            }
        }
        Ok((h, header_offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fourcc_values() {
        assert_eq!(SavePlatform::Win64.fourcc(), u32::from_le_bytes(*b"WIN_"));
        assert_eq!(SavePlatform::from_fourcc(u32::from_le_bytes(*b"X360")), Some(SavePlatform::X360));
        assert_eq!(SavePlatform::from_fourcc(1), None);
        assert_eq!(SavePlatform::Ps3.endian(), ByteOrder::Big);
        assert_eq!(SavePlatform::Ps4.endian(), ByteOrder::Little);
    }

    #[test]
    fn add_file_and_offsets() {
        let mut h = FileHeader::new();
        let a = h.add_file("a").unwrap();
        h.entries[a].length = 10;
        let b = h.add_file("b").unwrap();
        assert_eq!(h.entries[b].start_offset, 12 + 10);
        assert_eq!(h.add_file("a").unwrap(), a); // existing
        assert!(h.add_file(&"x".repeat(64)).is_err());
        assert_eq!(h.start_of_next_data(), 22);
        assert_eq!(h.file_size(), 22 + 2 * 144);
        h.entries[b].length = 5;
        h.adjust_start_offsets(a, 3, false);
        assert_eq!(h.entries[b].start_offset, 25);
    }

    fn sample_blob(h: &FileHeader) -> Vec<u8> {
        let mut blob = vec![0u8; h.start_of_next_data() as usize];
        h.write_into(&mut blob);
        blob
    }

    #[test]
    fn write_read_roundtrip() {
        let mut h = FileHeader::new();
        let a = h.add_file("level.dat").unwrap();
        h.entries[a].length = 7;
        h.entries[a].last_modified_time = 1_700_000_000_123;
        let b = h.add_file("region/r.0.0.mcr").unwrap();
        h.entries[b].length = 100;
        let blob = sample_blob(&h);
        assert_eq!(blob.len() as u32, h.file_size());
        assert_eq!(&blob[0..4], &(12u32 + 107).to_le_bytes());
        assert_eq!(&blob[4..8], &2u32.to_le_bytes());
        assert_eq!(&blob[8..12], &[9, 0, 9, 0]);

        let (r, data_len) = FileHeader::read_from(&blob, SavePlatform::Win64).unwrap();
        assert_eq!(data_len, 119);
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[0].name, "level.dat");
        assert_eq!(r.entries[0].last_modified_time, 1_700_000_000_123);
        assert_eq!(r.entries[1].name, "region/r.0.0.mcr");
        assert_eq!(r.entries[1].start_offset, 19);
        assert_eq!(r.save_version, 9);
    }

    #[test]
    fn reads_big_endian_x360_header() {
        // hand-built: one file "a", length 2, start 12, ts 5; version 3
        let mut b = vec![0u8; 14];
        b.extend_from_slice(&[0u8; 0]);
        b[0..4].copy_from_slice(&14u32.to_be_bytes());
        b[4..8].copy_from_slice(&1u32.to_be_bytes());
        b[8..10].copy_from_slice(&2i16.to_be_bytes());
        b[10..12].copy_from_slice(&3i16.to_be_bytes());
        let mut e = vec![0u8; ENTRY_SIZE_V2];
        e[0..2].copy_from_slice(&(b'a' as u16).to_be_bytes());
        e[128..132].copy_from_slice(&2u32.to_be_bytes());
        e[132..136].copy_from_slice(&12u32.to_be_bytes());
        e[136..144].copy_from_slice(&5i64.to_be_bytes());
        b.extend_from_slice(&e);
        let (h, _) = FileHeader::read_from(&b, SavePlatform::X360).unwrap();
        assert_eq!(h.entries[0].name, "a");
        assert_eq!((h.entries[0].length, h.entries[0].start_offset, h.entries[0].last_modified_time), (2, 12, 5));
        assert_eq!((h.original_save_version, h.save_version), (2, 3));
        // same bytes read as little-endian must fail validation rather than panic
        assert!(FileHeader::read_from(&b, SavePlatform::Win64).is_err());
    }

    #[test]
    fn reads_v1_header() {
        let mut b = vec![0u8; 12 + 3];
        b[0..4].copy_from_slice(&15u32.to_le_bytes());
        b[4..8].copy_from_slice(&(ENTRY_SIZE_V1 as u32).to_le_bytes()); // BYTE count in v1
        b[8..10].copy_from_slice(&1i16.to_le_bytes());
        b[10..12].copy_from_slice(&1i16.to_le_bytes());
        let mut e = vec![0u8; ENTRY_SIZE_V1];
        for (i, c) in "old".encode_utf16().enumerate() {
            e[i * 2..i * 2 + 2].copy_from_slice(&c.to_le_bytes());
        }
        e[128..132].copy_from_slice(&3u32.to_le_bytes());
        e[132..136].copy_from_slice(&12u32.to_le_bytes());
        b.extend_from_slice(&e);
        let (h, _) = FileHeader::read_from(&b, SavePlatform::Win64).unwrap();
        assert_eq!(h.entries.len(), 1);
        assert_eq!(h.entries[0].name, "old");
        assert_eq!(h.entries[0].last_modified_time, 0);
    }

    #[test]
    fn rejects_garbage() {
        assert!(FileHeader::read_from(&[0; 5], SavePlatform::Win64).is_err());
        let mut b = vec![0u8; 12];
        b[0..4].copy_from_slice(&999u32.to_le_bytes());
        assert!(FileHeader::read_from(&b, SavePlatform::Win64).is_err());
        let mut b = vec![0u8; 12];
        b[0..4].copy_from_slice(&12u32.to_le_bytes());
        b[10..12].copy_from_slice(&77i16.to_le_bytes());
        assert!(matches!(
            FileHeader::read_from(&b, SavePlatform::Win64),
            Err(StorageError::UnsupportedVersion(77))
        ));
        assert!(FileHeader::read_from(&b, SavePlatform::None).is_err());
    }

    #[test]
    fn prefix_and_remove() {
        let mut h = FileHeader::new();
        for (n, l) in [("DIM-1/r.0.0.mcr", 4u32), ("DIM-1/r.0.1.mcr", 6), ("other", 2)] {
            let i = h.add_file(n).unwrap();
            h.entries[i].length = l;
        }
        assert_eq!(h.files_with_prefix("DIM-1/").len(), 2);
        h.remove_file(0);
        assert_eq!(h.entries[0].start_offset, 12);
        assert_eq!(h.entries[1].start_offset, 18);
    }
}
