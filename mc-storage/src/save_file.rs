//! Port of `ConsoleSaveFileOriginal` — a whole world stored as one growable
//! byte blob with a file table (see [`crate::file_header`]).
//!
//! The C++ class juggled a 64 MiB virtual-memory reservation; here the data
//! area is a plain `Vec<u8>` and the table is serialised on demand.
//!
//! # On-disk wrapper
//! `saveData.ms` is `[i32 0][u32 uncompressed_len][zlib(blob)]` (little-endian);
//! if the first `i32` is non-zero the file is an *uncompressed* blob.

use crate::compression;
use crate::error::{Result, StorageError};
use crate::file_header::{FileEntry, FileHeader, SavePlatform, SAVE_FILE_HEADER_SIZE};
use std::time::{SystemTime, UNIX_EPOCH};

/// Stable identifier of a file in the save (survives other files being added/removed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileHandle(u64);

/// `FILE_BEGIN` / `FILE_END`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekFrom {
    Begin,
    End,
}

#[derive(Debug, Clone)]
pub struct SaveFile {
    header: FileHeader,
    /// Bytes `0..start_of_next_data`: 12-byte preamble (filled at serialise
    /// time) followed by each file's bytes back to back.
    data: Vec<u8>,
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl Default for SaveFile {
    fn default() -> Self {
        Self::new()
    }
}

impl SaveFile {
    /// An empty save (what the C++ constructor makes with `forceCleanSave`).
    pub fn new() -> Self {
        SaveFile {
            header: FileHeader::new(),
            data: vec![0; SAVE_FILE_HEADER_SIZE],
        }
    }

    /// Opens bytes as stored on disk (compressed wrapper or raw blob) from a Windows save.
    pub fn open(disk_bytes: &[u8]) -> Result<Self> {
        Self::open_for_platform(disk_bytes, SavePlatform::LOCAL)
    }

    pub fn open_for_platform(disk_bytes: &[u8], platform: SavePlatform) -> Result<Self> {
        compression::ensure_supported(platform)?;
        let blob_storage;
        let blob: &[u8] = if disk_bytes.len() >= 8 && disk_bytes[0..4] == [0, 0, 0, 0] {
            // compressed: [0][decomp_size][zlib]
            let size = platform.endian().u32(disk_bytes[4..8].try_into().unwrap()) as usize;
            if size == 0 {
                // "Invalid save data format" in the original: start from scratch
                return Ok(SaveFile::new());
            }
            if size > 1 << 30 {
                return Err(StorageError::Corrupt("save claims > 1 GiB"));
            }
            blob_storage = compression::zlib_decompress(&disk_bytes[8..], size)?;
            &blob_storage
        } else {
            disk_bytes
        };
        let (header, data_len) = FileHeader::read_from(blob, platform)?;
        let mut data = blob[..data_len].to_vec();
        data.resize(header.start_of_next_data() as usize, 0);
        Ok(SaveFile { header, data })
    }

    pub fn header(&self) -> &FileHeader {
        &self.header
    }

    pub fn platform(&self) -> SavePlatform {
        self.header.platform
    }

    pub fn is_save_endian_different(&self) -> bool {
        self.header.endian != crate::file_header::LOCAL_ENDIAN
    }

    pub fn save_version(&self) -> i16 {
        self.header.save_version
    }

    pub fn original_save_version(&self) -> i16 {
        self.header.original_save_version
    }

    fn index(&self, h: FileHandle) -> Result<usize> {
        self.header
            .entries
            .iter()
            .position(|e| e.id == h.0)
            .ok_or(StorageError::NoSuchFile)
    }

    /// `createFile`: returns the existing file if the name is already present.
    pub fn create_file(&mut self, name: &str) -> Result<FileHandle> {
        let i = self.header.add_file(name)?;
        Ok(FileHandle(self.header.entries[i].id))
    }

    pub fn find_file(&self, name: &str) -> Option<FileHandle> {
        self.header.index_of(name).map(|i| FileHandle(self.header.entries[i].id))
    }

    pub fn does_file_exist(&self, name: &str) -> bool {
        self.header.file_exists(name)
    }

    pub fn entry(&self, h: FileHandle) -> Result<&FileEntry> {
        Ok(&self.header.entries[self.index(h)?])
    }

    pub fn file_size(&self, h: FileHandle) -> Result<u32> {
        Ok(self.entry(h)?.length)
    }

    pub fn files_with_prefix(&self, prefix: &str) -> Vec<&FileEntry> {
        self.header.files_with_prefix(prefix)
    }

    /// `setFilePointer`
    pub fn set_file_pointer(&mut self, h: FileHandle, distance: i32, from: SeekFrom) -> Result<()> {
        let i = self.index(h)?;
        let e = &mut self.header.entries[i];
        let mut p = e.start_offset as i64 + distance as i64;
        if from == SeekFrom::End {
            p += e.length as i64;
        }
        e.current_pointer = p.clamp(0, u32::MAX as i64) as u32;
        Ok(())
    }

    /// Current pointer relative to the start of the file.
    pub fn file_pointer(&self, h: FileHandle) -> Result<u32> {
        let e = self.entry(h)?;
        Ok(e.current_pointer.saturating_sub(e.start_offset))
    }

    /// `PrepareForWrite` / `MoveDataBeyond`: grow the file so `n` more bytes fit at the pointer.
    fn prepare_for_write(&mut self, i: usize, n: usize) {
        let e = &self.header.entries[i];
        let end_needed = (e.current_pointer as i64 - e.start_offset as i64) + n as i64;
        let grow = end_needed - e.length as i64;
        if grow <= 0 {
            return;
        }
        let grow = grow as usize;
        let insert_at = (e.start_offset + e.length) as usize;
        self.data
            .splice(insert_at..insert_at, std::iter::repeat(0u8).take(grow));
        self.header.adjust_start_offsets(i, grow as u32, false);
        self.header.entries[i].length += grow as u32;
        debug_assert_eq!(self.data.len() as u32, self.header.start_of_next_data());
    }

    /// `writeFile`: writes at the pointer, growing the file as needed.
    pub fn write_file(&mut self, h: FileHandle, bytes: &[u8]) -> Result<()> {
        let i = self.index(h)?;
        self.prepare_for_write(i, bytes.len());
        let e = &mut self.header.entries[i];
        let at = e.current_pointer as usize;
        self.data[at..at + bytes.len()].copy_from_slice(bytes);
        e.current_pointer += bytes.len() as u32;
        e.last_modified_time = now_millis();
        Ok(())
    }

    /// `zeroFile`: writes `n` zero bytes at the pointer.
    pub fn zero_file(&mut self, h: FileHandle, n: usize) -> Result<()> {
        let i = self.index(h)?;
        self.prepare_for_write(i, n);
        let e = &mut self.header.entries[i];
        let at = e.current_pointer as usize;
        self.data[at..at + n].fill(0);
        e.current_pointer += n as u32;
        e.last_modified_time = now_millis();
        Ok(())
    }

    /// `readFile`: reads up to `buf.len()` bytes at the pointer; returns how many were read
    /// (fewer at end of file).
    pub fn read_file(&mut self, h: FileHandle, buf: &mut [u8]) -> Result<usize> {
        let i = self.index(h)?;
        let e = &mut self.header.entries[i];
        let end = e.start_offset as usize + e.length as usize;
        let at = e.current_pointer as usize;
        let n = buf.len().min(end.saturating_sub(at));
        buf[..n].copy_from_slice(&self.data[at..at + n]);
        e.current_pointer += n as u32;
        Ok(n)
    }

    /// Convenience: whole contents of a file.
    pub fn read_all(&self, h: FileHandle) -> Result<&[u8]> {
        let e = self.entry(h)?;
        Ok(&self.data[e.start_offset as usize..(e.start_offset + e.length) as usize])
    }

    /// `deleteFile`: removes the file and closes the gap.
    pub fn delete_file(&mut self, h: FileHandle) -> Result<()> {
        let i = self.index(h)?;
        let e = &self.header.entries[i];
        let (s, l) = (e.start_offset as usize, e.length as usize);
        self.data.drain(s..s + l);
        self.header.remove_file(i);
        debug_assert_eq!(self.data.len() as u32, self.header.start_of_next_data());
        Ok(())
    }

    /// The uncompressed blob (`finalizeWrite` + the memory image).
    pub fn to_blob(&self) -> Vec<u8> {
        let mut blob = self.data.clone();
        self.header.write_into(&mut blob);
        blob
    }

    /// What `Flush` hands to storage: `[0i32][blob_len u32][zlib(blob)]`.
    pub fn to_disk_bytes(&self) -> Vec<u8> {
        let blob = self.to_blob();
        let comp = compression::zlib_compress(&blob);
        let mut out = Vec::with_capacity(comp.len() + 8);
        out.extend_from_slice(&0i32.to_le_bytes());
        out.extend_from_slice(&(blob.len() as u32).to_le_bytes());
        out.extend_from_slice(&comp);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_write_read() {
        let mut s = SaveFile::new();
        let a = s.create_file("a.dat").unwrap();
        s.write_file(a, b"hello").unwrap();
        s.set_file_pointer(a, 0, SeekFrom::Begin).unwrap();
        let mut buf = [0u8; 16];
        assert_eq!(s.read_file(a, &mut buf).unwrap(), 5);
        assert_eq!(&buf[..5], b"hello");
        assert_eq!(s.read_file(a, &mut buf).unwrap(), 0); // at EOF
        assert_eq!(s.create_file("a.dat").unwrap(), a); // same file
        assert!(s.entry(a).unwrap().last_modified_time > 0);
    }

    #[test]
    fn growth_shifts_following_files() {
        let mut s = SaveFile::new();
        let a = s.create_file("a").unwrap();
        s.write_file(a, b"AAAA").unwrap();
        let b = s.create_file("b").unwrap();
        s.write_file(b, b"BB").unwrap();
        // grow a in the middle of the blob
        s.set_file_pointer(a, 0, SeekFrom::End).unwrap();
        s.write_file(a, b"aaaa").unwrap();
        assert_eq!(s.read_all(a).unwrap(), b"AAAAaaaa");
        assert_eq!(s.read_all(b).unwrap(), b"BB");
        assert_eq!(s.entry(b).unwrap().start_offset, 12 + 8);
        // overwrite inside, no growth
        s.set_file_pointer(a, 2, SeekFrom::Begin).unwrap();
        s.write_file(a, b"xx").unwrap();
        assert_eq!(s.read_all(a).unwrap(), b"AAxxaaaa");
        assert_eq!(s.file_size(a).unwrap(), 8);
    }

    #[test]
    fn write_past_end_leaves_zero_gap() {
        let mut s = SaveFile::new();
        let a = s.create_file("a").unwrap();
        s.set_file_pointer(a, 4, SeekFrom::Begin).unwrap();
        s.write_file(a, b"Z").unwrap();
        assert_eq!(s.read_all(a).unwrap(), &[0, 0, 0, 0, b'Z']);
    }

    #[test]
    fn zero_and_delete() {
        let mut s = SaveFile::new();
        let a = s.create_file("a").unwrap();
        let b = s.create_file("b").unwrap();
        let c = s.create_file("c").unwrap();
        s.write_file(a, b"1111").unwrap();
        s.write_file(b, b"2222").unwrap();
        s.write_file(c, b"3333").unwrap();
        s.set_file_pointer(b, 1, SeekFrom::Begin).unwrap();
        s.zero_file(b, 2).unwrap();
        assert_eq!(s.read_all(b).unwrap(), &[b'2', 0, 0, b'2']);
        s.delete_file(b).unwrap();
        assert!(s.entry(b).is_err());
        assert_eq!(s.read_all(a).unwrap(), b"1111");
        assert_eq!(s.read_all(c).unwrap(), b"3333");
        assert_eq!(s.entry(c).unwrap().start_offset, 12 + 4);
        assert!(!s.does_file_exist("b"));
    }

    #[test]
    fn blob_roundtrip() {
        let mut s = SaveFile::new();
        let a = s.create_file("level.dat").unwrap();
        s.write_file(a, &[1, 2, 3]).unwrap();
        let b = s.create_file("region/r.0.0.mcr").unwrap();
        s.write_file(b, &vec![7u8; 5000]).unwrap();

        let blob = s.to_blob();
        let r = SaveFile::open(&blob).unwrap(); // raw (first i32 != 0)
        assert_eq!(r.read_all(r.find_file("level.dat").unwrap()).unwrap(), &[1, 2, 3]);
        assert_eq!(r.read_all(r.find_file("region/r.0.0.mcr").unwrap()).unwrap().len(), 5000);
        assert_eq!(r.save_version(), 9);

        let disk = s.to_disk_bytes();
        assert_eq!(&disk[0..4], &[0, 0, 0, 0]);
        assert_eq!(u32::from_le_bytes(disk[4..8].try_into().unwrap()) as usize, blob.len());
        let r2 = SaveFile::open(&disk).unwrap();
        assert_eq!(r2.to_blob(), blob);
    }

    #[test]
    fn open_edge_cases() {
        // compressed wrapper with decomp size 0 = invalid save → fresh save
        let mut v = vec![0u8; 8];
        v.extend_from_slice(&[1, 2, 3]);
        assert!(SaveFile::open(&v).unwrap().header().entries().is_empty());
        assert!(SaveFile::open(&[1, 2, 3]).is_err());
        assert!(SaveFile::open_for_platform(&[0; 20], SavePlatform::X360).is_err());
        // truncated zlib
        let mut s = SaveFile::new();
        let a = s.create_file("a").unwrap();
        s.write_file(a, &vec![9u8; 1000]).unwrap();
        let d = s.to_disk_bytes();
        assert!(SaveFile::open(&d[..d.len() - 6]).is_err());
    }

    #[test]
    fn long_names_rejected() {
        let mut s = SaveFile::new();
        assert!(s.create_file(&"n".repeat(64)).is_err());
        assert!(s.create_file(&"n".repeat(63)).is_ok());
    }

    #[test]
    fn handles_survive_other_deletes() {
        let mut s = SaveFile::new();
        let a = s.create_file("a").unwrap();
        let b = s.create_file("b").unwrap();
        s.write_file(b, b"bbb").unwrap();
        s.delete_file(a).unwrap();
        // b's index shifted but its handle still works
        s.set_file_pointer(b, 0, SeekFrom::Begin).unwrap();
        let mut buf = [0u8; 3];
        s.read_file(b, &mut buf).unwrap();
        assert_eq!(&buf, b"bbb");
        assert!(s.write_file(a, b"x").is_err());
    }
}
