//! Port of `Random.cpp` — java.util.Random compatible 48-bit LCG.
//!
//! World generation depends on this producing *bit-identical* sequences to
//! Java's `Random`, so the arithmetic below deliberately mirrors the original.

const MULTIPLIER: i64 = 0x5DEECE66D;
const ADDEND: i64 = 0xB;
const MASK: i64 = (1i64 << 48) - 1;

#[derive(Debug, Clone)]
pub struct Random {
    seed: i64,
    have_next_next_gaussian: bool,
    next_next_gaussian: f64,
}

impl Default for Random {
    fn default() -> Self {
        Self::new()
    }
}

impl Random {
    /// Seeded from the system clock (original used QueryPerformanceCounter).
    pub fn new() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0);
        Self::with_seed(nanos.wrapping_add(8682522807148012))
    }

    pub fn with_seed(seed: i64) -> Self {
        let mut r = Random {
            seed: 0,
            have_next_next_gaussian: false,
            next_next_gaussian: 0.0,
        };
        r.set_seed(seed);
        r
    }

    pub fn set_seed(&mut self, s: i64) {
        self.seed = (s ^ MULTIPLIER) & MASK;
        self.have_next_next_gaussian = false;
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(ADDEND) & MASK;
        (self.seed >> (48 - bits)) as i32
    }

    pub fn next_bytes(&mut self, bytes: &mut [u8]) {
        for b in bytes {
            *b = self.next(8) as u8;
        }
    }

    pub fn next_double(&mut self) -> f64 {
        let hi = (self.next(26) as i64) << 27;
        let lo = self.next(27) as i64;
        (hi + lo) as f64 / (1i64 << 53) as f64
    }

    pub fn next_gaussian(&mut self) -> f64 {
        if self.have_next_next_gaussian {
            self.have_next_next_gaussian = false;
            return self.next_next_gaussian;
        }
        loop {
            let v1 = 2.0 * self.next_double() - 1.0;
            let v2 = 2.0 * self.next_double() - 1.0;
            let s = v1 * v1 + v2 * v2;
            if s < 1.0 && s != 0.0 {
                let multiplier = (-2.0 * s.ln() / s).sqrt();
                self.next_next_gaussian = v2 * multiplier;
                self.have_next_next_gaussian = true;
                return v1 * multiplier;
            }
        }
    }

    /// `nextInt()` — all 32 bits.
    pub fn next_int(&mut self) -> i32 {
        self.next(32)
    }

    /// `nextInt(n)` — uniform in `[0, n)`. Panics if `n <= 0` (original asserts).
    pub fn next_int_bounded(&mut self, n: i32) -> i32 {
        assert!(n > 0, "Random::next_int_bounded requires n > 0");
        if n & n.wrapping_neg() == n {
            // power of two
            return ((self.next(31) as i64 * n as i64) >> 31) as i32;
        }
        loop {
            let bits = self.next(31);
            let val = bits % n;
            if bits.wrapping_sub(val).wrapping_add(n - 1) >= 0 {
                return val;
            }
        }
    }

    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 / (1i32 << 24) as f32
    }

    pub fn next_long(&mut self) -> i64 {
        ((self.next(32) as i64) << 32).wrapping_add(self.next(32) as i64)
    }

    pub fn next_boolean(&mut self) -> bool {
        self.next(1) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Reference values are the well-known java.util.Random outputs.
    #[test]
    fn matches_java_next_int() {
        assert_eq!(Random::with_seed(42).next_int(), -1170105035);
        assert_eq!(Random::with_seed(0).next_int(), -1155484576);
    }

    #[test]
    fn matches_java_bounded() {
        let mut r = Random::with_seed(42);
        let v: Vec<i32> = (0..5).map(|_| r.next_int_bounded(10)).collect();
        // well-known java.util.Random(42).nextInt(10) sequence
        assert_eq!(v, vec![0, 3, 8, 4, 0]);
    }

    #[test]
    fn matches_java_long_double_gaussian() {
        assert_eq!(Random::with_seed(0).next_long(), -4962768465676381896);
        let d = Random::with_seed(0).next_double();
        assert!((d - 0.730967787376657).abs() < 1e-15, "{d}");
        let g = Random::with_seed(0).next_gaussian();
        assert!((g - 0.8025330637390305).abs() < 1e-15, "{g}");
    }

    #[test]
    fn power_of_two_bound() {
        let mut r = Random::with_seed(1);
        for _ in 0..1000 {
            let v = r.next_int_bounded(16);
            assert!((0..16).contains(&v));
        }
    }
}
