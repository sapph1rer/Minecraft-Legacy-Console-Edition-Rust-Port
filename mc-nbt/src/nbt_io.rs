//! Port of `NbtIo.cpp` plus the `readNamedTag` / `writeNamedTag` / `load` /
//! `write` logic that lived in `Tag.cpp` and the individual tag classes.

use crate::{id, CompoundTag, ListTag, NbtError, NbtLimits, Result, Tag};
use mc_core::data_io::{DataInput, DataOutput};
use std::io::{Read, Write};

// ---------------------------------------------------------------- reading --

struct Ctx<'a> {
    limits: &'a NbtLimits,
    total_tags: usize,
}

/// `NbtIo::read` — root must be a named compound; the root name is discarded.
pub fn read<R: Read>(r: &mut R) -> Result<CompoundTag> {
    read_named(r).map(|(_, c)| c)
}

/// Like [`read`] but also returns the root name.
pub fn read_named<R: Read>(r: &mut R) -> Result<(String, CompoundTag)> {
    read_with_limits(r, &NbtLimits::default())
}

pub fn read_with_limits<R: Read>(r: &mut R, limits: &NbtLimits) -> Result<(String, CompoundTag)> {
    let mut ctx = Ctx { limits, total_tags: 1 };
    let ty = r.read_unsigned_byte()?;
    if ty != id::COMPOUND {
        return Err(NbtError::RootNotCompound);
    }
    let name = r.read_utf()?;
    match read_payload(r, ty, 1, &mut ctx)? {
        Tag::Compound(c) => Ok((name, c)),
        _ => unreachable!("payload type was COMPOUND"),
    }
}

fn read_len(r: &mut impl Read) -> Result<usize> {
    let n = r.read_int()?;
    usize::try_from(n).map_err(|_| NbtError::NegativeLength(n))
}

fn read_payload<R: Read>(r: &mut R, ty: u8, depth: usize, ctx: &mut Ctx) -> Result<Tag> {
    Ok(match ty {
        id::END => Tag::End,
        id::BYTE => Tag::Byte(r.read_byte()?),
        id::SHORT => Tag::Short(r.read_short()?),
        id::INT => Tag::Int(r.read_int()?),
        id::LONG => Tag::Long(r.read_long()?),
        id::FLOAT => Tag::Float(r.read_float()?),
        id::DOUBLE => Tag::Double(r.read_double()?),
        id::STRING => Tag::String(r.read_utf()?),
        id::BYTE_ARRAY => {
            let len = read_len(r)?;
            if len > ctx.limits.max_byte_array {
                return Err(NbtError::LimitExceeded("byte array length"));
            }
            let mut buf = vec![0u8; len];
            r.read_fully(&mut buf)?;
            Tag::ByteArray(buf)
        }
        id::INT_ARRAY => {
            let len = read_len(r)?;
            if len > ctx.limits.max_int_array {
                return Err(NbtError::LimitExceeded("int array length"));
            }
            let mut v = Vec::with_capacity(len);
            for _ in 0..len {
                v.push(r.read_int()?);
            }
            Tag::IntArray(v)
        }
        id::LIST => {
            if depth > ctx.limits.max_depth {
                return Err(NbtError::LimitExceeded("nesting depth"));
            }
            let elem_ty = r.read_unsigned_byte()?;
            let len = read_len(r)?;
            if len > ctx.limits.max_list_len {
                return Err(NbtError::LimitExceeded("list length"));
            }
            if len > 0 && (elem_ty == id::END || elem_ty > id::INT_ARRAY) {
                return Err(NbtError::InvalidTagType(elem_ty));
            }
            let mut items = Vec::with_capacity(len);
            for _ in 0..len {
                items.push(read_payload(r, elem_ty, depth + 1, ctx)?);
            }
            Tag::List(ListTag::from_items(items))
        }
        id::COMPOUND => {
            if depth > ctx.limits.max_depth {
                return Err(NbtError::LimitExceeded("nesting depth"));
            }
            let mut c = CompoundTag::new();
            loop {
                let child_ty = r.read_unsigned_byte()?;
                if child_ty == id::END {
                    break;
                }
                if child_ty > id::INT_ARRAY {
                    return Err(NbtError::InvalidTagType(child_ty));
                }
                ctx.total_tags += 1;
                if ctx.total_tags > ctx.limits.max_total_tags {
                    return Err(NbtError::LimitExceeded("total tag count"));
                }
                if c.len() >= ctx.limits.max_compound_entries {
                    return Err(NbtError::LimitExceeded("compound entry count"));
                }
                let name = r.read_utf()?;
                let tag = read_payload(r, child_ty, depth + 1, ctx)?;
                c.put(name, tag);
            }
            Tag::Compound(c)
        }
        other => return Err(NbtError::InvalidTagType(other)),
    })
}

// ---------------------------------------------------------------- writing --

/// `NbtIo::write` — writes the compound with an empty root name.
pub fn write<W: Write>(tag: &CompoundTag, w: &mut W) -> Result<()> {
    write_named("", tag, w)
}

pub fn write_named<W: Write>(name: &str, tag: &CompoundTag, w: &mut W) -> Result<()> {
    w.write_byte(id::COMPOUND as i8)?;
    w.write_utf(name)?;
    write_compound_payload(tag, w)
}

fn write_compound_payload<W: Write>(c: &CompoundTag, w: &mut W) -> Result<()> {
    for (name, tag) in c.iter() {
        w.write_byte(tag.id() as i8)?;
        w.write_utf(name)?;
        write_payload(tag, w)?;
    }
    w.write_byte(id::END as i8)?;
    Ok(())
}

fn write_payload<W: Write>(tag: &Tag, w: &mut W) -> Result<()> {
    match tag {
        Tag::End => {}
        Tag::Byte(v) => w.write_byte(*v)?,
        Tag::Short(v) => w.write_short(*v)?,
        Tag::Int(v) => w.write_int(*v)?,
        Tag::Long(v) => w.write_long(*v)?,
        Tag::Float(v) => w.write_float(*v)?,
        Tag::Double(v) => w.write_double(*v)?,
        Tag::String(v) => w.write_utf(v)?,
        Tag::ByteArray(v) => {
            w.write_int(v.len() as i32)?;
            w.write_all(v)?;
        }
        Tag::IntArray(v) => {
            w.write_int(v.len() as i32)?;
            for x in v {
                w.write_int(*x)?;
            }
        }
        Tag::List(l) => {
            w.write_byte(l.element_type() as i8)?;
            w.write_int(l.len() as i32)?;
            for t in l.iter() {
                write_payload(t, w)?;
            }
        }
        Tag::Compound(c) => write_compound_payload(c, w)?,
    }
    Ok(())
}

// ------------------------------------------------------- in-memory helpers --

/// `NbtIo::compress` (no actual compression in this source tree).
pub fn to_bytes(tag: &CompoundTag) -> Result<Vec<u8>> {
    let mut v = Vec::new();
    write(tag, &mut v)?;
    Ok(v)
}

/// `NbtIo::decompress`
pub fn from_bytes(buf: &[u8]) -> Result<CompoundTag> {
    read(&mut &buf[..])
}
