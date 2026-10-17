//! NBT — port of the `Tag` family (`Tag.cpp`, `CompoundTag.h`, `ListTag.h`,
//! `ByteArrayTag.h`, ...) and `NbtIo.cpp`.
//!
//! # Design changes vs. the C++
//! * The class hierarchy (`Tag` + 12 subclasses, raw owning pointers) becomes one
//!   [`Tag`] enum; ownership is plain Rust ownership, so `copy()` is `Clone`.
//! * Tags no longer carry their own name. A name is the key in a
//!   [`CompoundTag`], or the root name given to [`nbt_io::write_named`].
//! * Reading is strict and returns [`NbtError`]; the C++ silently turned
//!   truncated/oversized data into `EndTag`s or zero-length arrays. The same
//!   safety limits are kept in [`NbtLimits`] (defaults equal the console values).
//! * `NbtIo::readCompressed/writeCompressed` in this source tree do *not*
//!   compress (the GZIP calls were removed), so there is no compression here.

use indexmap::IndexMap;
use std::fmt;
use std::io;

pub mod nbt_io;

/// Tag type ids (`Tag::TAG_*`).
pub mod id {
    pub const END: u8 = 0;
    pub const BYTE: u8 = 1;
    pub const SHORT: u8 = 2;
    pub const INT: u8 = 3;
    pub const LONG: u8 = 4;
    pub const FLOAT: u8 = 5;
    pub const DOUBLE: u8 = 6;
    pub const BYTE_ARRAY: u8 = 7;
    pub const STRING: u8 = 8;
    pub const LIST: u8 = 9;
    pub const COMPOUND: u8 = 10;
    pub const INT_ARRAY: u8 = 11;
}

/// Safety limits applied while reading. `Default` = the console build's values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NbtLimits {
    /// Max nesting of compounds/lists (original: 256).
    pub max_depth: usize,
    /// Max number of named tags in one document (original: 32768).
    pub max_total_tags: usize,
    /// Max entries in a single compound (original: 10000).
    pub max_compound_entries: usize,
    /// Max `TAG_Byte_Array` length (original: 2 MiB).
    pub max_byte_array: usize,
    /// Max `TAG_Int_Array` length in ints (original: 65536).
    pub max_int_array: usize,
    /// Max `TAG_List` length (original reuses `MAX_DEPTH` = 512 here).
    pub max_list_len: usize,
}

impl Default for NbtLimits {
    fn default() -> Self {
        NbtLimits {
            max_depth: 256,
            max_total_tags: 32768,
            max_compound_entries: 10000,
            max_byte_array: 2 * 1024 * 1024,
            max_int_array: 65536,
            max_list_len: 512,
        }
    }
}

#[derive(Debug)]
pub enum NbtError {
    Io(io::Error),
    /// A limit from [`NbtLimits`] was exceeded.
    LimitExceeded(&'static str),
    /// Unknown tag id.
    InvalidTagType(u8),
    /// Negative array/list length.
    NegativeLength(i32),
    /// Document root was not a (named) compound.
    RootNotCompound,
    /// List elements must all share one type.
    MixedListTypes { expected: u8, found: u8 },
}

impl fmt::Display for NbtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NbtError::Io(e) => write!(f, "io error: {e}"),
            NbtError::LimitExceeded(what) => write!(f, "NBT limit exceeded: {what}"),
            NbtError::InvalidTagType(t) => write!(f, "invalid NBT tag type {t}"),
            NbtError::NegativeLength(n) => write!(f, "negative NBT length {n}"),
            NbtError::RootNotCompound => write!(f, "NBT root must be a named compound"),
            NbtError::MixedListTypes { expected, found } => {
                write!(f, "list holds type {expected}, cannot add type {found}")
            }
        }
    }
}

impl std::error::Error for NbtError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NbtError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for NbtError {
    fn from(e: io::Error) -> Self {
        NbtError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, NbtError>;

#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    End,
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(String),
    List(ListTag),
    Compound(CompoundTag),
    IntArray(Vec<i32>),
}

impl Tag {
    pub fn id(&self) -> u8 {
        match self {
            Tag::End => id::END,
            Tag::Byte(_) => id::BYTE,
            Tag::Short(_) => id::SHORT,
            Tag::Int(_) => id::INT,
            Tag::Long(_) => id::LONG,
            Tag::Float(_) => id::FLOAT,
            Tag::Double(_) => id::DOUBLE,
            Tag::ByteArray(_) => id::BYTE_ARRAY,
            Tag::String(_) => id::STRING,
            Tag::List(_) => id::LIST,
            Tag::Compound(_) => id::COMPOUND,
            Tag::IntArray(_) => id::INT_ARRAY,
        }
    }

    /// `Tag::getTagName`
    pub fn type_name(id: u8) -> &'static str {
        match id {
            id::END => "TAG_End",
            id::BYTE => "TAG_Byte",
            id::SHORT => "TAG_Short",
            id::INT => "TAG_Int",
            id::LONG => "TAG_Long",
            id::FLOAT => "TAG_Float",
            id::DOUBLE => "TAG_Double",
            id::BYTE_ARRAY => "TAG_Byte_Array",
            id::INT_ARRAY => "TAG_Int_Array",
            id::STRING => "TAG_String",
            id::LIST => "TAG_List",
            id::COMPOUND => "TAG_Compound",
            _ => "UNKNOWN",
        }
    }

    /// `toString()` of each tag class.
    pub fn summary(&self) -> String {
        match self {
            Tag::End => "END".into(),
            Tag::Byte(v) => v.to_string(),
            Tag::Short(v) => v.to_string(),
            Tag::Int(v) => v.to_string(),
            Tag::Long(v) => v.to_string(),
            Tag::Float(v) => v.to_string(),
            Tag::Double(v) => v.to_string(),
            Tag::ByteArray(v) => format!("[{} bytes]", v.len()),
            Tag::String(v) => v.clone(),
            Tag::List(l) => format!("{} entries of type {}", l.len(), Tag::type_name(l.element_type())),
            Tag::Compound(c) => format!("{} entries", c.len()),
            // (sic) the original prints the int count as "bytes"
            Tag::IntArray(v) => format!("[{} bytes]", v.len()),
        }
    }
}

macro_rules! impl_from {
    ($($ty:ty => $variant:ident),* $(,)?) => {
        $(impl From<$ty> for Tag { fn from(v: $ty) -> Tag { Tag::$variant(v) } })*
    };
}
impl_from! {
    i8 => Byte, i16 => Short, i32 => Int, i64 => Long, f32 => Float, f64 => Double,
    String => String, CompoundTag => Compound, ListTag => List,
}
impl From<&str> for Tag {
    fn from(v: &str) -> Tag {
        Tag::String(v.to_owned())
    }
}

/// `ListTag<T>`. All elements share one type.
#[derive(Debug, Clone, Default)]
pub struct ListTag {
    items: Vec<Tag>,
}

impl ListTag {
    pub fn new() -> Self {
        Self::default()
    }

    /// Type id written to disk: first element's id, or `TAG_Byte` (1) when
    /// empty — the original does exactly this.
    pub fn element_type(&self) -> u8 {
        self.items.first().map_or(id::BYTE, Tag::id)
    }

    pub fn push(&mut self, tag: impl Into<Tag>) -> Result<()> {
        let tag = tag.into();
        if let Some(first) = self.items.first() {
            if first.id() != tag.id() {
                return Err(NbtError::MixedListTypes { expected: first.id(), found: tag.id() });
            }
        }
        self.items.push(tag);
        Ok(())
    }

    pub fn get(&self, index: usize) -> Option<&Tag> {
        self.items.get(index)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Tag> {
        self.items.iter()
    }

    pub(crate) fn from_items(items: Vec<Tag>) -> Self {
        ListTag { items }
    }
}

/// Mirrors `ListTag::equals`: same size and every element of `self` equals
/// *some* element of `other` (order-insensitive, O(n²)). Note this means
/// `[1,1,2]` equals `[1,2,2]`; kept for behavioural compatibility.
impl PartialEq for ListTag {
    fn eq(&self, other: &Self) -> bool {
        if self.items.len() != other.items.len() {
            return false;
        }
        if self.items.is_empty() {
            return true;
        }
        self.element_type() == other.element_type()
            && self.items.iter().all(|a| other.items.iter().any(|b| a == b))
    }
}

/// `CompoundTag`. Insertion-ordered (the C++ used an unordered_map, so any
/// order was valid on disk; this makes output deterministic).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CompoundTag {
    tags: IndexMap<String, Tag>,
}

macro_rules! getter {
    ($name:ident, $ret:ty, $variant:ident, $default:expr) => {
        /// Returns the default (`0`/empty) when missing or of another type, like the original's missing-key behaviour.
        pub fn $name(&self, name: &str) -> $ret {
            match self.tags.get(name) {
                Some(Tag::$variant(v)) => v.clone(),
                _ => $default,
            }
        }
    };
}

impl CompoundTag {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a tag. `Tag::End` is ignored (it would terminate the compound on disk).
    pub fn put(&mut self, name: impl Into<String>, tag: impl Into<Tag>) {
        let tag = tag.into();
        if matches!(tag, Tag::End) {
            return;
        }
        self.tags.insert(name.into(), tag);
    }

    pub fn put_byte(&mut self, name: &str, v: i8) { self.put(name, v) }
    pub fn put_short(&mut self, name: &str, v: i16) { self.put(name, v) }
    pub fn put_int(&mut self, name: &str, v: i32) { self.put(name, v) }
    pub fn put_long(&mut self, name: &str, v: i64) { self.put(name, v) }
    pub fn put_float(&mut self, name: &str, v: f32) { self.put(name, v) }
    pub fn put_double(&mut self, name: &str, v: f64) { self.put(name, v) }
    pub fn put_string(&mut self, name: &str, v: &str) { self.put(name, v) }
    pub fn put_byte_array(&mut self, name: &str, v: Vec<u8>) { self.put(name, Tag::ByteArray(v)) }
    pub fn put_int_array(&mut self, name: &str, v: Vec<i32>) { self.put(name, Tag::IntArray(v)) }
    pub fn put_compound(&mut self, name: &str, v: CompoundTag) { self.put(name, v) }
    pub fn put_list(&mut self, name: &str, v: ListTag) { self.put(name, v) }
    pub fn put_boolean(&mut self, name: &str, v: bool) { self.put_byte(name, v as i8) }

    pub fn get(&self, name: &str) -> Option<&Tag> {
        self.tags.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.tags.contains_key(name)
    }

    getter!(get_byte, i8, Byte, 0);
    getter!(get_short, i16, Short, 0);
    getter!(get_int, i32, Int, 0);
    getter!(get_long, i64, Long, 0);
    getter!(get_float, f32, Float, 0.0);
    getter!(get_double, f64, Double, 0.0);
    getter!(get_string, String, String, String::new());
    getter!(get_byte_array, Vec<u8>, ByteArray, Vec::new());
    getter!(get_int_array, Vec<i32>, IntArray, Vec::new());

    pub fn get_boolean(&self, name: &str) -> bool {
        self.get_byte(name) != 0
    }

    /// The original returns a fresh empty compound when missing; use
    /// `.unwrap_or_default()` for that behaviour.
    pub fn get_compound(&self, name: &str) -> Option<&CompoundTag> {
        match self.tags.get(name) {
            Some(Tag::Compound(c)) => Some(c),
            _ => None,
        }
    }

    pub fn get_compound_mut(&mut self, name: &str) -> Option<&mut CompoundTag> {
        match self.tags.get_mut(name) {
            Some(Tag::Compound(c)) => Some(c),
            _ => None,
        }
    }

    pub fn get_list(&self, name: &str) -> Option<&ListTag> {
        match self.tags.get(name) {
            Some(Tag::List(l)) => Some(l),
            _ => None,
        }
    }

    pub fn remove(&mut self, name: &str) -> Option<Tag> {
        self.tags.shift_remove(name)
    }

    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tags.len()
    }

    /// `getAllTags()`
    pub fn iter(&self) -> indexmap::map::Iter<'_, String, Tag> {
        self.tags.iter()
    }

    pub fn keys(&self) -> indexmap::map::Keys<'_, String, Tag> {
        self.tags.keys()
    }
}
