//! Port of `compression.cpp` for the platforms that use zlib (Windows 64,
//! Xbox One, PS4, Vita): chunk data is *RLE-encoded, then zlib-compressed*.
//!
//! Xbox 360 (LZX) and PS3 (EdgeZLib) saves are not supported here.

use crate::error::{Result, StorageError};
use crate::file_header::SavePlatform;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use std::io::{Read, Write};

/// Upper bound the original puts on a decompression buffer (16 MiB).
pub const MAX_DECOMPRESSED: usize = 16 * 1024 * 1024;

/// The RLE used before zlib:
/// * `0..=254` — literal byte
/// * `255, n` with `n` in `0..=2` — `n + 1` copies of 255
/// * `255, n, b` with `n >= 3` — `n + 1` copies of `b`
pub fn rle_encode(src: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(src.len() / 2 + 16);
    let mut i = 0;
    while i < src.len() {
        let this = src[i];
        let mut count = 1usize;
        i += 1;
        while i < src.len() && src[i] == this && count < 256 {
            i += 1;
            count += 1;
        }
        if count <= 3 {
            if this == 255 {
                out.push(255);
                out.push((count - 1) as u8);
            } else {
                out.extend(std::iter::repeat(this).take(count));
            }
        } else {
            out.push(255);
            out.push((count - 1) as u8);
            out.push(this);
        }
    }
    out
}

/// Inverse of [`rle_encode`]. `max_out` bounds the output (zip-bomb guard).
/// A dangling trailing `255` is ignored, like the original.
pub fn rle_decode(src: &[u8], max_out: usize) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(src.len().min(max_out));
    let mut i = 0;
    while i < src.len() {
        let b = src[i];
        i += 1;
        if b != 255 {
            if out.len() >= max_out {
                return Err(StorageError::Corrupt("RLE output exceeds expected size"));
            }
            out.push(b);
            continue;
        }
        let Some(&n) = src.get(i) else { break };
        i += 1;
        let (count, byte) = if n < 3 {
            (n as usize + 1, 255u8)
        } else {
            let Some(&data) = src.get(i) else { break };
            i += 1;
            (n as usize + 1, data)
        };
        if out.len() + count > max_out {
            return Err(StorageError::Corrupt("RLE run exceeds expected size"));
        }
        out.extend(std::iter::repeat(byte).take(count));
    }
    Ok(out)
}

/// zlib (RFC 1950) at the default level, like `::compress`.
pub fn zlib_compress(src: &[u8]) -> Vec<u8> {
    let mut enc = ZlibEncoder::new(Vec::with_capacity(src.len() / 2 + 64), flate2::Compression::default());
    enc.write_all(src).expect("writing to a Vec cannot fail");
    enc.finish().expect("finishing into a Vec cannot fail")
}

/// zlib decompression with an output cap.
pub fn zlib_decompress(src: &[u8], max_out: usize) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut dec = ZlibDecoder::new(src).take(max_out as u64 + 1);
    dec.read_to_end(&mut out)
        .map_err(|_| StorageError::Corrupt("zlib stream invalid"))?;
    if out.len() > max_out {
        return Err(StorageError::Corrupt("zlib output exceeds expected size"));
    }
    Ok(out)
}

/// `CompressLZXRLE` on zlib platforms (the name is historical).
pub fn compress_rle_zlib(src: &[u8]) -> Vec<u8> {
    zlib_compress(&rle_encode(src))
}

/// `DecompressLZXRLE` on zlib platforms. `expected_len` is the size recorded
/// in the chunk header; the result may be shorter, never longer.
pub fn decompress_rle_zlib(src: &[u8], expected_len: usize) -> Result<Vec<u8>> {
    if expected_len > MAX_DECOMPRESSED {
        return Err(StorageError::Corrupt("chunk claims > 16 MiB"));
    }
    // The zlib payload is the RLE stream, which can be larger than the final
    // data in the worst case (every 255 doubles), hence the 2x + slack cap.
    let rle = zlib_decompress(src, (expected_len * 2).max(200 * 1024).min(MAX_DECOMPRESSED))?;
    rle_decode(&rle, expected_len)
}

/// Plain zlib chunk (length field without the RLE flag).
pub fn decompress_zlib_only(src: &[u8], expected_len: usize) -> Result<Vec<u8>> {
    if expected_len > MAX_DECOMPRESSED {
        return Err(StorageError::Corrupt("chunk claims > 16 MiB"));
    }
    zlib_decompress(src, expected_len)
}

/// Which codec a save from `platform` uses.
pub fn ensure_supported(platform: SavePlatform) -> Result<()> {
    match platform {
        SavePlatform::X360 | SavePlatform::Ps3 => Err(StorageError::UnsupportedCompression(platform)),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rle_known_vectors() {
        assert_eq!(rle_encode(&[7]), vec![7]);
        assert_eq!(rle_encode(&[7, 7, 7]), vec![7, 7, 7]);
        assert_eq!(rle_encode(&[7, 7, 7, 7]), vec![255, 3, 7]);
        assert_eq!(rle_encode(&[255]), vec![255, 0]);
        assert_eq!(rle_encode(&[255, 255]), vec![255, 1]);
        assert_eq!(rle_encode(&[255, 255, 255]), vec![255, 2]);
        assert_eq!(rle_encode(&[255, 255, 255, 255]), vec![255, 3, 255]);
        // run of 256 → n = 255
        assert_eq!(rle_encode(&vec![9u8; 256]), vec![255, 255, 9]);
        // run of 257 → 256 + 1 literal
        assert_eq!(rle_encode(&vec![9u8; 257]), vec![255, 255, 9, 9]);
    }

    fn xorshift(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    #[test]
    fn rle_roundtrip_random() {
        let mut s = 0x1234_5678_9abc_def1u64;
        for _ in 0..200 {
            let len = (xorshift(&mut s) % 2000) as usize + 1;
            let mut v = Vec::with_capacity(len);
            while v.len() < len {
                let b = match xorshift(&mut s) % 4 {
                    0 => 255,
                    1 => 0,
                    _ => xorshift(&mut s) as u8,
                };
                let run = (xorshift(&mut s) % 6) as usize + 1;
                v.extend(std::iter::repeat(b).take(run));
            }
            let enc = rle_encode(&v);
            assert_eq!(rle_decode(&enc, v.len()).unwrap(), v);
        }
    }

    #[test]
    fn rle_decode_guards() {
        assert!(rle_decode(&[255, 255, 9], 100).is_err());
        assert!(rle_decode(&[1, 2, 3], 2).is_err());
        assert_eq!(rle_decode(&[1, 255], 10).unwrap(), vec![1]); // dangling 255 ignored
    }

    #[test]
    fn zlib_and_combined_roundtrip() {
        let data: Vec<u8> = (0..50_000u32).map(|i| if i % 97 < 60 { 0 } else { (i % 251) as u8 }).collect();
        assert_eq!(zlib_decompress(&zlib_compress(&data), data.len()).unwrap(), data);
        let c = compress_rle_zlib(&data);
        assert!(c.len() < data.len() / 4);
        assert_eq!(decompress_rle_zlib(&c, data.len()).unwrap(), data);
        assert!(zlib_decompress(&c, 10).is_err());
        assert!(decompress_rle_zlib(&[1, 2, 3], 10).is_err());
    }

    #[test]
    fn platform_support() {
        assert!(ensure_supported(SavePlatform::Win64).is_ok());
        assert!(ensure_supported(SavePlatform::Ps4).is_ok());
        assert!(ensure_supported(SavePlatform::X360).is_err());
    }
}
