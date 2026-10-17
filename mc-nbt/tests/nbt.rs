use mc_nbt::nbt_io::{self, from_bytes, to_bytes};
use mc_nbt::{id, CompoundTag, ListTag, NbtError, NbtLimits, Tag};

/// The canonical "hello world" example from the NBT spec.
const HELLO_WORLD: &[u8] = &[
    0x0a, 0x00, 0x0b, b'h', b'e', b'l', b'l', b'o', b' ', b'w', b'o', b'r', b'l', b'd', 0x08,
    0x00, 0x04, b'n', b'a', b'm', b'e', 0x00, 0x09, b'B', b'a', b'n', b'a', b'n', b'r', b'a',
    b'm', b'a', 0x00,
];

#[test]
fn reads_spec_hello_world() {
    let (name, c) = nbt_io::read_named(&mut &HELLO_WORLD[..]).unwrap();
    assert_eq!(name, "hello world");
    assert_eq!(c.get_string("name"), "Bananrama");
    assert_eq!(c.len(), 1);
}

#[test]
fn writes_spec_hello_world_bytes() {
    let mut c = CompoundTag::new();
    c.put_string("name", "Bananrama");
    let mut out = Vec::new();
    nbt_io::write_named("hello world", &c, &mut out).unwrap();
    assert_eq!(out, HELLO_WORLD);
}

fn sample() -> CompoundTag {
    let mut c = CompoundTag::new();
    c.put_byte("b", -5);
    c.put_short("s", -1234);
    c.put_int("i", 123456789);
    c.put_long("l", i64::MIN + 7);
    c.put_float("f", 3.5);
    c.put_double("d", -0.125);
    c.put_string("str", "Thai: สวัสดี 😀");
    c.put_byte_array("ba", vec![0, 1, 2, 255]);
    c.put_int_array("ia", vec![-1, 0, 1, i32::MAX]);
    c.put_boolean("flag", true);
    let mut inner = CompoundTag::new();
    inner.put_int("x", 1);
    c.put_compound("inner", inner);
    let mut list = ListTag::new();
    list.push(10i32).unwrap();
    list.push(20i32).unwrap();
    c.put_list("list", list);
    let mut clist = ListTag::new();
    let mut e = CompoundTag::new();
    e.put_string("id", "Pig");
    clist.push(e).unwrap();
    c.put_list("entities", clist);
    c
}

#[test]
fn roundtrip_everything() {
    let c = sample();
    let bytes = to_bytes(&c).unwrap();
    let back = from_bytes(&bytes).unwrap();
    assert_eq!(c, back);
    assert_eq!(back.get_long("l"), i64::MIN + 7);
    assert!(back.get_boolean("flag"));
    assert_eq!(back.get_compound("inner").unwrap().get_int("x"), 1);
    assert_eq!(back.get_list("list").unwrap().len(), 2);
    assert_eq!(back.get_list("entities").unwrap().get(0), Some(&Tag::Compound({
        let mut e = CompoundTag::new();
        e.put_string("id", "Pig");
        e
    })));
}

#[test]
fn getters_default_when_missing_or_wrong_type() {
    let c = sample();
    assert_eq!(c.get_int("nope"), 0);
    assert_eq!(c.get_string("nope"), "");
    assert_eq!(c.get_int("str"), 0); // wrong type
    assert!(c.get_compound("nope").is_none());
    assert!(!c.contains("nope"));
}

#[test]
fn empty_list_writes_byte_type() {
    let mut c = CompoundTag::new();
    c.put_list("l", ListTag::new());
    let bytes = to_bytes(&c).unwrap();
    // 0a 0000 | 09 0001 'l' | 01 00000000 | 00
    assert_eq!(bytes, vec![0x0a, 0, 0, 0x09, 0, 1, b'l', 0x01, 0, 0, 0, 0, 0x00]);
}

#[test]
fn list_rejects_mixed_types() {
    let mut l = ListTag::new();
    l.push(1i32).unwrap();
    assert!(matches!(l.push(2i64), Err(NbtError::MixedListTypes { expected: 3, found: 4 })));
}

#[test]
fn list_equality_matches_original_semantics() {
    let mut a = ListTag::new();
    let mut b = ListTag::new();
    for v in [1, 2, 3] { a.push(v).unwrap(); }
    for v in [3, 2, 1] { b.push(v).unwrap(); }
    assert_eq!(a, b); // order-insensitive
    let mut c = ListTag::new();
    for v in [1, 2, 4] { c.push(v).unwrap(); }
    assert_ne!(a, c);
}

#[test]
fn end_tag_not_stored() {
    let mut c = CompoundTag::new();
    c.put("x", Tag::End);
    assert!(c.is_empty());
}

#[test]
fn rejects_non_compound_root_and_bad_types() {
    assert!(matches!(from_bytes(&[0x01, 0, 0, 5]), Err(NbtError::RootNotCompound)));
    assert!(matches!(from_bytes(&[0x00]), Err(NbtError::RootNotCompound)));
    // compound containing an unknown tag type 99
    let bad = [0x0a, 0, 0, 99, 0, 1, b'x'];
    assert!(matches!(from_bytes(&bad), Err(NbtError::InvalidTagType(99))));
}

#[test]
fn truncated_input_is_error() {
    let bytes = to_bytes(&sample()).unwrap();
    for cut in [1, 5, bytes.len() / 2, bytes.len() - 1] {
        assert!(from_bytes(&bytes[..cut]).is_err(), "cut at {cut}");
    }
}

#[test]
fn limits_are_enforced() {
    // byte array larger than limit
    let mut c = CompoundTag::new();
    c.put_byte_array("a", vec![0; 100]);
    let bytes = to_bytes(&c).unwrap();
    let limits = NbtLimits { max_byte_array: 10, ..Default::default() };
    assert!(matches!(
        nbt_io::read_with_limits(&mut &bytes[..], &limits),
        Err(NbtError::LimitExceeded(_))
    ));

    // nesting deeper than max_depth
    let mut deep = CompoundTag::new();
    for _ in 0..300 {
        let mut outer = CompoundTag::new();
        outer.put_compound("c", deep);
        deep = outer;
    }
    let bytes = to_bytes(&deep).unwrap();
    assert!(matches!(from_bytes(&bytes), Err(NbtError::LimitExceeded("nesting depth"))));
    let relaxed = NbtLimits { max_depth: 1000, max_total_tags: 100000, ..Default::default() };
    assert!(nbt_io::read_with_limits(&mut &bytes[..], &relaxed).is_ok());

    // list longer than the console's 512 cap
    let mut big = ListTag::new();
    for i in 0..513 { big.push(i as i32).unwrap(); }
    let mut c = CompoundTag::new();
    c.put_list("l", big);
    assert!(matches!(from_bytes(&to_bytes(&c).unwrap()), Err(NbtError::LimitExceeded("list length"))));

    // negative length
    let neg = [0x0a, 0, 0, id::BYTE_ARRAY, 0, 1, b'a', 0xff, 0xff, 0xff, 0xff, 0];
    assert!(matches!(from_bytes(&neg), Err(NbtError::NegativeLength(-1))));
}

#[test]
fn summaries_and_names() {
    assert_eq!(Tag::type_name(id::COMPOUND), "TAG_Compound");
    assert_eq!(Tag::type_name(42), "UNKNOWN");
    assert_eq!(Tag::ByteArray(vec![0; 3]).summary(), "[3 bytes]");
    assert_eq!(Tag::Compound(sample()).summary(), "13 entries");
    let mut l = ListTag::new();
    l.push("a").unwrap();
    assert_eq!(Tag::List(l).summary(), "1 entries of type TAG_String");
}

#[test]
fn deterministic_insertion_order() {
    let mut c = CompoundTag::new();
    c.put_int("z", 1);
    c.put_int("a", 2);
    let keys: Vec<_> = c.keys().cloned().collect();
    assert_eq!(keys, ["z", "a"]);
}
