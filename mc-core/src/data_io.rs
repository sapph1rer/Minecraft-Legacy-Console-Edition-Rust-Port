//! Port of `DataInputStream` / `DataOutputStream` (java.io.Data* semantics):
//! big-endian primitives and "modified UTF-8" strings, as extension traits on
//! `std::io::Read` / `std::io::Write`.
//!
//! Differences from the C++ code (deliberate):
//! * EOF / malformed data returns `Err` instead of silently yielding garbage
//!   (the C++ code folds `read() == -1` into the value, or truncates strings).
//! * Strings are Rust `String`s; the C++ code used UTF-16 `wchar_t`. The wire
//!   format is identical (surrogates are encoded as two 3-byte units), but a
//!   *lone* surrogate cannot be represented and decodes to U+FFFD.

use std::io::{self, Read, Write};

/// Console build refuses strings longer than this (Java's limit is 65535).
pub const MAX_UTF_LENGTH: usize = 32767;

fn invalid(msg: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

pub trait DataInput: Read {
    fn read_unsigned_byte(&mut self) -> io::Result<u8> {
        let mut b = [0u8; 1];
        self.read_exact(&mut b)?;
        Ok(b[0])
    }
    fn read_byte(&mut self) -> io::Result<i8> {
        Ok(self.read_unsigned_byte()? as i8)
    }
    fn read_boolean(&mut self) -> io::Result<bool> {
        Ok(self.read_unsigned_byte()? != 0)
    }
    fn read_short(&mut self) -> io::Result<i16> {
        let mut b = [0u8; 2];
        self.read_exact(&mut b)?;
        Ok(i16::from_be_bytes(b))
    }
    fn read_unsigned_short(&mut self) -> io::Result<u16> {
        let mut b = [0u8; 2];
        self.read_exact(&mut b)?;
        Ok(u16::from_be_bytes(b))
    }
    /// UTF-16 code unit.
    fn read_char(&mut self) -> io::Result<u16> {
        self.read_unsigned_short()
    }
    fn read_int(&mut self) -> io::Result<i32> {
        let mut b = [0u8; 4];
        self.read_exact(&mut b)?;
        Ok(i32::from_be_bytes(b))
    }
    fn read_long(&mut self) -> io::Result<i64> {
        let mut b = [0u8; 8];
        self.read_exact(&mut b)?;
        Ok(i64::from_be_bytes(b))
    }
    fn read_float(&mut self) -> io::Result<f32> {
        Ok(f32::from_bits(self.read_int()? as u32))
    }
    fn read_double(&mut self) -> io::Result<f64> {
        Ok(f64::from_bits(self.read_long()? as u64))
    }
    fn read_fully(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.read_exact(buf)
    }
    /// Modified UTF-8 with a 2-byte length prefix.
    fn read_utf(&mut self) -> io::Result<String> {
        let len = self.read_unsigned_short()? as usize;
        if len > MAX_UTF_LENGTH {
            return Err(invalid("UTF string longer than console limit (32767)"));
        }
        let mut buf = vec![0u8; len];
        self.read_exact(&mut buf)?;
        decode_modified_utf8(&buf)
    }
}
impl<R: Read + ?Sized> DataInput for R {}

pub trait DataOutput: Write {
    fn write_byte(&mut self, v: i8) -> io::Result<()> {
        self.write_all(&[v as u8])
    }
    fn write_boolean(&mut self, v: bool) -> io::Result<()> {
        self.write_all(&[v as u8])
    }
    fn write_short(&mut self, v: i16) -> io::Result<()> {
        self.write_all(&v.to_be_bytes())
    }
    fn write_unsigned_short(&mut self, v: u16) -> io::Result<()> {
        self.write_all(&v.to_be_bytes())
    }
    fn write_char(&mut self, v: u16) -> io::Result<()> {
        self.write_unsigned_short(v)
    }
    fn write_chars(&mut self, s: &str) -> io::Result<()> {
        for u in s.encode_utf16() {
            self.write_char(u)?;
        }
        Ok(())
    }
    fn write_int(&mut self, v: i32) -> io::Result<()> {
        self.write_all(&v.to_be_bytes())
    }
    fn write_long(&mut self, v: i64) -> io::Result<()> {
        self.write_all(&v.to_be_bytes())
    }
    fn write_float(&mut self, v: f32) -> io::Result<()> {
        self.write_int(v.to_bits() as i32)
    }
    fn write_double(&mut self, v: f64) -> io::Result<()> {
        self.write_long(v.to_bits() as i64)
    }
    fn write_utf(&mut self, s: &str) -> io::Result<()> {
        let bytes = encode_modified_utf8(s)?;
        self.write_all(&bytes)
    }
}
impl<W: Write + ?Sized> DataOutput for W {}

/// Returns the 2-byte length prefix followed by modified-UTF-8 bytes.
pub fn encode_modified_utf8(s: &str) -> io::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(s.len());
    for c in s.encode_utf16() {
        match c {
            0x0001..=0x007F => body.push(c as u8),
            // U+0000 and 0x80..=0x7FF → 2 bytes (NUL becomes C0 80)
            0x0000 | 0x0080..=0x07FF => {
                body.push(0xC0 | ((c >> 6) & 0x1F) as u8);
                body.push(0x80 | (c & 0x3F) as u8);
            }
            _ => {
                body.push(0xE0 | ((c >> 12) & 0x0F) as u8);
                body.push(0x80 | ((c >> 6) & 0x3F) as u8);
                body.push(0x80 | (c & 0x3F) as u8);
            }
        }
    }
    if body.len() > u16::MAX as usize {
        return Err(invalid("encoded string too long (> 65535 bytes)"));
    }
    let mut out = Vec::with_capacity(body.len() + 2);
    out.extend_from_slice(&(body.len() as u16).to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Decodes modified-UTF-8 bytes (without the length prefix).
pub fn decode_modified_utf8(buf: &[u8]) -> io::Result<String> {
    let mut units: Vec<u16> = Vec::with_capacity(buf.len());
    let mut i = 0;
    while i < buf.len() {
        let a = buf[i];
        if a & 0x80 == 0 {
            units.push(a as u16);
            i += 1;
        } else if a & 0xE0 == 0xC0 {
            let b = *buf.get(i + 1).ok_or_else(|| invalid("truncated 2-byte UTF sequence"))?;
            if b & 0xC0 != 0x80 {
                return Err(invalid("bad continuation byte"));
            }
            units.push((((a & 0x1F) as u16) << 6) | (b & 0x3F) as u16);
            i += 2;
        } else if a & 0xF0 == 0xE0 {
            let (b, c) = match (buf.get(i + 1), buf.get(i + 2)) {
                (Some(&b), Some(&c)) => (b, c),
                _ => return Err(invalid("truncated 3-byte UTF sequence")),
            };
            if b & 0xC0 != 0x80 || c & 0xC0 != 0x80 {
                return Err(invalid("bad continuation byte"));
            }
            units.push((((a & 0x0F) as u16) << 12) | (((b & 0x3F) as u16) << 6) | (c & 0x3F) as u16);
            i += 3;
        } else {
            // 10xxxxxx or 1111xxxx
            return Err(invalid("invalid UTF lead byte"));
        }
    }
    Ok(String::from_utf16_lossy(&units))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn primitives_roundtrip_big_endian() {
        let mut v = Vec::new();
        v.write_byte(-2).unwrap();
        v.write_boolean(true).unwrap();
        v.write_short(-300).unwrap();
        v.write_unsigned_short(65000).unwrap();
        v.write_int(0x01020304).unwrap();
        v.write_long(-0x0102030405060708).unwrap();
        v.write_float(1.5).unwrap();
        v.write_double(-2.25).unwrap();
        assert_eq!(&v[2..4], &(-300i16).to_be_bytes());
        assert_eq!(&v[6..10], &[1, 2, 3, 4]);

        let mut r = Cursor::new(v);
        assert_eq!(r.read_byte().unwrap(), -2);
        assert!(r.read_boolean().unwrap());
        assert_eq!(r.read_short().unwrap(), -300);
        assert_eq!(r.read_unsigned_short().unwrap(), 65000);
        assert_eq!(r.read_int().unwrap(), 0x01020304);
        assert_eq!(r.read_long().unwrap(), -0x0102030405060708);
        assert_eq!(r.read_float().unwrap(), 1.5);
        assert_eq!(r.read_double().unwrap(), -2.25);
        assert!(r.read_byte().is_err()); // EOF is an error
    }

    #[test]
    fn utf_known_encodings() {
        assert_eq!(encode_modified_utf8("").unwrap(), vec![0, 0]);
        assert_eq!(encode_modified_utf8("hi").unwrap(), vec![0, 2, b'h', b'i']);
        // NUL → C0 80, e-acute → C3 A9, euro → E2 82 AC
        assert_eq!(encode_modified_utf8("\0").unwrap(), vec![0, 2, 0xC0, 0x80]);
        assert_eq!(encode_modified_utf8("é").unwrap(), vec![0, 2, 0xC3, 0xA9]);
        assert_eq!(encode_modified_utf8("€").unwrap(), vec![0, 3, 0xE2, 0x82, 0xAC]);
        // astral char = surrogate pair = 2 × 3 bytes (CESU-8 style)
        let e = encode_modified_utf8("😀").unwrap();
        assert_eq!(e, vec![0, 6, 0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
    }

    #[test]
    fn utf_roundtrip() {
        for s in ["", "abc", "Thai: สวัสดี", "mix é€😀\0end"] {
            let mut v = Vec::new();
            v.write_utf(s).unwrap();
            assert_eq!(Cursor::new(v).read_utf().unwrap(), s);
        }
    }

    #[test]
    fn utf_errors() {
        assert!(decode_modified_utf8(&[0xC3]).is_err());
        assert!(decode_modified_utf8(&[0xE2, 0x82]).is_err());
        assert!(decode_modified_utf8(&[0x80]).is_err());
        assert!(decode_modified_utf8(&[0xF0, 0x9F, 0x98, 0x80]).is_err());
        // length prefix above the console limit
        let mut v = Vec::new();
        v.write_unsigned_short(40000).unwrap();
        assert!(Cursor::new(v).read_utf().is_err());
        // truncated body
        assert!(Cursor::new(vec![0, 5, b'a']).read_utf().is_err());
    }

    #[test]
    fn chars() {
        let mut v = Vec::new();
        v.write_chars("aé").unwrap();
        assert_eq!(v, vec![0, b'a', 0, 0xE9]);
    }
}
