//! Port of `Pos.cpp` and `ChunkPos.cpp`.

use std::fmt;

/// Integer block position (original `Pos`). Value type: the C++ version
/// heap-allocated on every `above()/north()/...`; here they just return copies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Pos { x, y, z }
    }

    /// Original: `x + (z << 8) + (y << 16)`
    pub fn hash_code(&self) -> i32 {
        self.x
            .wrapping_add(self.z.wrapping_shl(8))
            .wrapping_add(self.y.wrapping_shl(16))
    }

    /// Orders by y, then z, then x (returns difference like the original).
    pub fn compare_to(&self, other: &Pos) -> i32 {
        if self.y == other.y {
            if self.z == other.z {
                return self.x.wrapping_sub(other.x);
            }
            return self.z.wrapping_sub(other.z);
        }
        self.y.wrapping_sub(other.y)
    }

    pub fn offset(&self, x: i32, y: i32, z: i32) -> Pos {
        Pos::new(self.x + x, self.y + y, self.z + z)
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32) {
        self.x = x;
        self.y = y;
        self.z = z;
    }

    pub fn above(&self) -> Pos { self.above_by(1) }
    pub fn above_by(&self, n: i32) -> Pos { Pos::new(self.x, self.y + n, self.z) }
    pub fn below(&self) -> Pos { self.below_by(1) }
    pub fn below_by(&self, n: i32) -> Pos { Pos::new(self.x, self.y - n, self.z) }
    pub fn north(&self) -> Pos { self.north_by(1) }
    pub fn north_by(&self, n: i32) -> Pos { Pos::new(self.x, self.y, self.z - n) }
    pub fn south(&self) -> Pos { self.south_by(1) }
    pub fn south_by(&self, n: i32) -> Pos { Pos::new(self.x, self.y, self.z + n) }
    pub fn east(&self) -> Pos { self.east_by(1) }
    pub fn east_by(&self, n: i32) -> Pos { Pos::new(self.x + n, self.y, self.z) }
    pub fn west(&self) -> Pos { self.west_by(1) }
    /// NOTE: original `Pos::west(int steps)` ignores `steps` (always moves 1).
    /// That is an upstream bug; the port honours `steps`.
    pub fn west_by(&self, n: i32) -> Pos { Pos::new(self.x - n, self.y, self.z) }

    pub fn move_by(&mut self, x: i32, y: i32, z: i32) {
        self.x += x;
        self.y += y;
        self.z += z;
    }
    pub fn move_pos(&mut self, p: Pos) { self.move_by(p.x, p.y, p.z) }
    pub fn move_x(&mut self, n: i32) { self.x += n }
    pub fn move_y(&mut self, n: i32) { self.y += n }
    pub fn move_z(&mut self, n: i32) { self.z += n }
    pub fn move_up(&mut self, n: i32) { self.y += n }
    pub fn move_down(&mut self, n: i32) { self.y -= n }
    pub fn move_east(&mut self, n: i32) { self.x += n }
    pub fn move_west(&mut self, n: i32) { self.x -= n }
    pub fn move_north(&mut self, n: i32) { self.z -= n }
    pub fn move_south(&mut self, n: i32) { self.z += n }

    pub fn dist(&self, x: i32, y: i32, z: i32) -> f64 {
        let dx = (self.x - x) as f64;
        let dy = (self.y - y) as f64;
        let dz = (self.z - z) as f64;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    pub fn dist_to(&self, p: &Pos) -> f64 {
        self.dist(p.x, p.y, p.z)
    }

    /// Computed in `f32` like the original.
    pub fn dist_sqr(&self, x: i32, y: i32, z: i32) -> f32 {
        let dx = (self.x - x) as f32;
        let dy = (self.y - y) as f32;
        let dz = (self.z - z) as f32;
        dx * dx + dy * dy + dz * dz
    }

    pub fn dist_sqr_to(&self, p: &Pos) -> f32 {
        self.dist_sqr(p.x, p.y, p.z)
    }
}

/// Chunk column coordinate (original `ChunkPos`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl ChunkPos {
    pub const fn new(x: i32, z: i32) -> Self {
        ChunkPos { x, z }
    }

    /// 64-bit key: low 32 bits = x, high 32 bits = z.
    pub fn hash_code_xz(x: i32, z: i32) -> i64 {
        (x as u32 as i64) | ((z as u32 as i64) << 32)
    }

    pub fn hash_code(&self) -> i32 {
        let hash = Self::hash_code_xz(self.x, self.z);
        (hash as i32) ^ ((hash >> 32) as i32)
    }

    pub fn distance_to_sqr(&self, px: f64, pz: f64) -> f64 {
        let x_pos = (self.x * 16 + 8) as f64;
        let z_pos = (self.z * 16 + 8) as f64;
        let xd = x_pos - px;
        let zd = z_pos - pz;
        xd * xd + zd * zd
    }

    pub fn middle_block_x(&self) -> i32 {
        (self.x << 4) + 8
    }

    pub fn middle_block_z(&self) -> i32 {
        (self.z << 4) + 8
    }

    // TODO(port): getMiddleBlockPosition(y) -> TilePos and
    // distanceToSqr(shared_ptr<Entity>) once TilePos / Entity are ported.
}

impl fmt::Display for ChunkPos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {}]", self.x, self.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pos_moves() {
        let p = Pos::new(1, 2, 3);
        assert_eq!(p.above(), Pos::new(1, 3, 3));
        assert_eq!(p.below_by(5), Pos::new(1, -3, 3));
        assert_eq!(p.north(), Pos::new(1, 2, 2));
        assert_eq!(p.south_by(2), Pos::new(1, 2, 5));
        assert_eq!(p.west_by(3), Pos::new(-2, 2, 3));
        assert_eq!(p.east(), Pos::new(2, 2, 3));
    }

    #[test]
    fn pos_hash_and_cmp() {
        assert_eq!(Pos::new(1, 2, 3).hash_code(), 1 + (3 << 8) + (2 << 16));
        assert!(Pos::new(0, 1, 0).compare_to(&Pos::new(5, 0, 5)) > 0);
        assert!(Pos::new(0, 0, 1).compare_to(&Pos::new(5, 0, 0)) > 0);
        assert_eq!(Pos::new(3, 0, 0).compare_to(&Pos::new(1, 0, 0)), 2);
    }

    #[test]
    fn pos_dist() {
        let p = Pos::new(0, 0, 0);
        assert_eq!(p.dist(3, 4, 0), 5.0);
        assert_eq!(p.dist_sqr(1, 2, 2), 9.0);
    }

    #[test]
    fn chunk_pos_hash() {
        assert_eq!(ChunkPos::hash_code_xz(1, 2), 1 | (2i64 << 32));
        // negative x must not sign-extend into the z half
        assert_eq!(ChunkPos::hash_code_xz(-1, 0), 0xffff_ffff);
        assert_eq!(ChunkPos::new(1, 2).hash_code(), 1 ^ 2);
    }

    #[test]
    fn chunk_pos_misc() {
        let c = ChunkPos::new(-1, 3);
        assert_eq!(c.middle_block_x(), -8);
        assert_eq!(c.middle_block_z(), 56);
        assert_eq!(c.to_string(), "[-1, 3]");
        assert_eq!(c.distance_to_sqr(-8.0, 56.0), 0.0);
    }
}
