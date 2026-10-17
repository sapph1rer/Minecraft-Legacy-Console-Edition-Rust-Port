//! Port of `Mth.cpp` — math helpers.
//!
//! Float/double widths follow the original on purpose (the sine table is
//! built in `f32`, `Mth::floor(float)` vs `floor(double)` are separate, ...).

use crate::random::Random;
use std::sync::OnceLock;

/// `#define PI (3.141592654f)` from Definitions.h
#[allow(clippy::approx_constant)]
pub const PI: f32 = 3.141592654;
#[allow(clippy::approx_constant)]
pub const HALF_PI: f32 = 1.570796327;

pub const DEGRAD: f32 = PI / 180.0;
pub const RADDEG: f32 = 180.0 / PI;
pub const RAD_TO_GRAD: f32 = PI / 180.0;

const BIG_ENOUGH_INT: i32 = 1024;
const BIG_ENOUGH_FLOAT: f32 = BIG_ENOUGH_INT as f32;
const SIN_SCALE: f32 = 65536.0 / (PI * 2.0);

const UUID_VERSION: i64 = 0x0000_0000_0000_f000;
const UUID_VERSION_TYPE_4: i64 = 0x0000_0000_0000_4000;
const UUID_VARIANT: u64 = 0xc000_0000_0000_0000;
const UUID_VARIANT_2: u64 = 0x8000_0000_0000_0000;

static SIN_TABLE: OnceLock<Box<[f32; 65536]>> = OnceLock::new();

fn table() -> &'static [f32; 65536] {
    SIN_TABLE.get_or_init(|| {
        let mut t = Box::new([0f32; 65536]);
        for (i, v) in t.iter_mut().enumerate() {
            // (float)::sin(i * PI * 2 / 65536.0f)   — argument computed in f32
            let arg = (i as f32 * PI * 2.0) / 65536.0;
            *v = (arg as f64).sin() as f32;
        }
        t
    })
}

/// Table-based sine (65536 entries), like the original.
pub fn sin(i: f32) -> f32 {
    table()[((i * SIN_SCALE) as i32 & 65535) as usize]
}

pub fn cos(i: f32) -> f32 {
    table()[((i * SIN_SCALE + 16384.0) as i32 & 65535) as usize]
}

pub fn sqrt_f32(x: f32) -> f32 {
    x.sqrt()
}

pub fn sqrt_f64(x: f64) -> f32 {
    x.sqrt() as f32
}

pub fn floor_f32(v: f32) -> i32 {
    let i = v as i32;
    if v < i as f32 { i - 1 } else { i }
}

pub fn floor_f64(v: f64) -> i32 {
    let i = v as i32;
    if v < i as f64 { i - 1 } else { i }
}

pub fn lfloor(v: f64) -> i64 {
    let i = v as i64;
    if v < i as f64 { i - 1 } else { i }
}

pub fn fast_floor(x: f64) -> i32 {
    ((x + BIG_ENOUGH_FLOAT as f64) as i32) - BIG_ENOUGH_INT
}

pub fn abs_floor(v: f64) -> i32 {
    (if v >= 0.0 { v } else { -v + 1.0 }) as i32
}

pub fn abs_f32(v: f32) -> f32 {
    if v >= 0.0 { v } else { -v }
}

pub fn abs_i32(v: i32) -> i32 {
    if v >= 0 { v } else { v.wrapping_neg() }
}

pub fn ceil(v: f32) -> i32 {
    let i = v as i32;
    if v > i as f32 { i + 1 } else { i }
}

pub fn clamp_i32(value: i32, min: i32, max: i32) -> i32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

pub fn clamp_f32(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// `asbMax` (sic) — larger of |a| and |b|.
pub fn abs_max(a: f64, b: f64) -> f64 {
    let (a, b) = (a.abs(), b.abs());
    if a > b { a } else { b }
}

pub fn int_floor_div(a: i32, b: i32) -> i32 {
    if a < 0 {
        -((-a - 1) / b) - 1
    } else {
        a / b
    }
}

/// Uniform int in `[min_inclusive, max_inclusive]`.
pub fn next_int(random: &mut Random, min_inclusive: i32, max_inclusive: i32) -> i32 {
    if min_inclusive >= max_inclusive {
        return min_inclusive;
    }
    random
        .next_int_bounded(max_inclusive - min_inclusive + 1)
        .wrapping_add(min_inclusive)
}

pub fn next_float(random: &mut Random, min: f32, max: f32) -> f32 {
    if min >= max {
        return min;
    }
    random.next_float() * (max - min) + min
}

pub fn next_double(random: &mut Random, min: f64, max: f64) -> f64 {
    if min >= max {
        return min;
    }
    random.next_double() * (max - min) + min
}

pub fn wrap_degrees_f32(mut input: f32) -> f32 {
    while input >= 180.0 {
        input -= 360.0;
    }
    while input < -180.0 {
        input += 360.0;
    }
    input
}

pub fn wrap_degrees_f64(mut input: f64) -> f64 {
    while input >= 180.0 {
        input -= 360.0;
    }
    while input < -180.0 {
        input += 360.0;
    }
    input
}

/// 32-hex-digit "UUID" without dashes (version 4 / variant 2 bits set).
pub fn create_insecure_uuid(random: &mut Random) -> String {
    let mut high = (random.next_long() & !UUID_VERSION) | UUID_VERSION_TYPE_4;
    let mut low = ((random.next_long() as u64 & !UUID_VARIANT) | UUID_VARIANT_2) as i64;
    let mut out = ['0'; 32];
    let hex = |n: i64| char::from_digit((n & 0xf) as u32, 16).unwrap();
    for i in 0..16 {
        out[31 - i] = hex(low);
        out[15 - i] = hex(high);
        high >>= 4;
        low >>= 4;
    }
    out.iter().collect()
}

// NOTE: the original ignores `def` when parsing fails (stringstream yields 0).
// The port returns `def` on parse failure, which is what the signature implies.
pub fn get_int(input: &str, def: i32) -> i32 {
    input.trim().parse().unwrap_or(def)
}

pub fn get_int_min(input: &str, def: i32, min: i32) -> i32 {
    get_int(input, def).max(min)
}

pub fn get_double(input: &str, def: f64) -> f64 {
    input.trim().parse().unwrap_or(def)
}

pub fn get_double_min(input: &str, def: f64, min: f64) -> f64 {
    let r = get_double(input, def);
    if r < min { min } else { r }
}

pub fn almost_equals(a: f64, b: f64, precision: f64) -> bool {
    (a - b).abs() <= precision
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_cos_table() {
        assert_eq!(sin(0.0), 0.0);
        assert!((cos(0.0) - 1.0).abs() < 1e-6);
        assert!((sin(HALF_PI) - 1.0).abs() < 1e-3);
        assert!((cos(PI) + 1.0).abs() < 1e-3);
        // negative angles wrap through the mask
        assert!((sin(-HALF_PI) + 1.0).abs() < 1e-3);
    }

    #[test]
    fn floors() {
        assert_eq!(floor_f32(-0.5), -1);
        assert_eq!(floor_f64(2.9), 2);
        assert_eq!(floor_f64(-2.1), -3);
        assert_eq!(lfloor(-0.1), -1);
        assert_eq!(fast_floor(-0.5), -1);
        assert_eq!(fast_floor(3.99), 3);
        assert_eq!(ceil(1.1), 2);
        assert_eq!(ceil(-1.1), -1);
        assert_eq!(abs_floor(-1.5), 2);
    }

    #[test]
    fn floor_div() {
        assert_eq!(int_floor_div(7, 2), 3);
        assert_eq!(int_floor_div(-1, 16), -1);
        assert_eq!(int_floor_div(-16, 16), -1);
        assert_eq!(int_floor_div(-17, 16), -2);
    }

    #[test]
    fn wrap() {
        assert_eq!(wrap_degrees_f32(190.0), -170.0);
        assert_eq!(wrap_degrees_f64(-190.0), 170.0);
        assert_eq!(wrap_degrees_f32(180.0), -180.0);
    }

    #[test]
    fn uuid_shape() {
        let mut r = Random::with_seed(7);
        let u = create_insecure_uuid(&mut r);
        assert_eq!(u.len(), 32);
        // version nibble (char 12) is '4', variant nibble (char 16) in 8..=b
        assert_eq!(u.as_bytes()[12], b'4');
        assert!(matches!(u.as_bytes()[16], b'8' | b'9' | b'a' | b'b'));
    }

    #[test]
    fn ranged_random() {
        let mut r = Random::with_seed(3);
        for _ in 0..200 {
            let v = next_int(&mut r, -5, 5);
            assert!((-5..=5).contains(&v));
        }
        assert_eq!(next_int(&mut r, 4, 4), 4);
    }
}
