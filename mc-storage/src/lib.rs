//! Save-file storage (layer 2): the single-blob `ConsoleSaveFile` container,
//! its file table (`FileHeader`), `.mcr` region files and the chunk
//! compression (RLE + zlib on Windows/Xbox One/PS4/Vita).
//!
//! Everything operates on in-memory bytes; reading/writing the blob from disk
//! is a one-liner for the caller (`std::fs::read` → [`SaveFile::open`]).

pub mod compression;
pub mod error;
pub mod file_header;
pub mod region_cache;
pub mod region_file;
pub mod save_file;

pub use error::{Result, StorageError};
pub use file_header::{FileEntry, FileHeader, SavePlatform};
pub use region_cache::RegionFileCache;
pub use region_file::RegionFile;
pub use save_file::{FileHandle, SaveFile, SeekFrom};
