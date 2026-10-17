use crate::file_header::SavePlatform;
use std::fmt;

#[derive(Debug)]
pub enum StorageError {
    Io(std::io::Error),
    /// Structurally invalid data.
    Corrupt(&'static str),
    /// Save comes from a platform whose compression is not implemented
    /// (Xbox 360 LZX, PS3 EdgeZLib).
    UnsupportedCompression(SavePlatform),
    /// A chunk would need 256 or more 4 KiB sectors (limit ≈ 1 MB).
    ChunkTooLarge { sectors: usize },
    /// File name does not fit the 64-UTF-16-unit table field.
    InvalidFileName,
    /// Unknown save-file version.
    UnsupportedVersion(i16),
    /// Handle does not refer to a file (deleted?).
    NoSuchFile,
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::Io(e) => write!(f, "io error: {e}"),
            StorageError::Corrupt(m) => write!(f, "corrupt save data: {m}"),
            StorageError::UnsupportedCompression(p) => {
                write!(f, "unsupported compression for platform {p:?}")
            }
            StorageError::ChunkTooLarge { sectors } => {
                write!(f, "chunk needs {sectors} sectors (max 255)")
            }
            StorageError::InvalidFileName => write!(f, "file name must be < 64 UTF-16 units"),
            StorageError::UnsupportedVersion(v) => write!(f, "unsupported save version {v}"),
            StorageError::NoSuchFile => write!(f, "no such file in save"),
        }
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StorageError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        StorageError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, StorageError>;
