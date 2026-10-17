//! Port of `AABB.cpp` (axis-aligned bounding box).
//!
//! As with `Vec3`, the per-thread object pool is dropped: `Aabb` is `Copy`.

use crate::vec3::Vec3;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Aabb {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub x1: f64,
    pub y1: f64,
    pub z1: f64,
}

/// Result of [`Aabb::clip`]. The original returns a `HitResult(0,0,0,face,pos)`.
// TODO(port): replace with the real `HitResult` once ported.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AabbHit {
    /// 0=-Y 1=+Y 2=-Z 3=+Z 4=-X 5=+X
    pub face: i32,
    pub pos: Vec3,
}

impl Aabb {
    pub const fn new(x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64) -> Self {
        Aabb { x0, y0, z0, x1, y1, z1 }
    }

    pub fn set(&mut self, x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64) -> &mut Self {
        *self = Aabb::new(x0, y0, z0, x1, y1, z1);
        self
    }

    /// Grow only in the direction of the given delta.
    pub fn expand(&self, xa: f64, ya: f64, za: f64) -> Aabb {
        let (mut x0, mut y0, mut z0, mut x1, mut y1, mut z1) =
            (self.x0, self.y0, self.z0, self.x1, self.y1, self.z1);
        if xa < 0.0 { x0 += xa }
        if xa > 0.0 { x1 += xa }
        if ya < 0.0 { y0 += ya }
        if ya > 0.0 { y1 += ya }
        if za < 0.0 { z0 += za }
        if za > 0.0 { z1 += za }
        Aabb::new(x0, y0, z0, x1, y1, z1)
    }

    /// Grow on both sides.
    pub fn grow(&self, xa: f64, ya: f64, za: f64) -> Aabb {
        Aabb::new(
            self.x0 - xa, self.y0 - ya, self.z0 - za,
            self.x1 + xa, self.y1 + ya, self.z1 + za,
        )
    }

    pub fn minmax(&self, o: &Aabb) -> Aabb {
        Aabb::new(
            self.x0.min(o.x0), self.y0.min(o.y0), self.z0.min(o.z0),
            self.x1.max(o.x1), self.y1.max(o.y1), self.z1.max(o.z1),
        )
    }

    pub fn clone_move(&self, xa: f64, ya: f64, za: f64) -> Aabb {
        Aabb::new(
            self.x0 + xa, self.y0 + ya, self.z0 + za,
            self.x1 + xa, self.y1 + ya, self.z1 + za,
        )
    }

    pub fn clip_x_collide(&self, c: &Aabb, mut xa: f64) -> f64 {
        if c.y1 <= self.y0 || c.y0 >= self.y1 { return xa }
        if c.z1 <= self.z0 || c.z0 >= self.z1 { return xa }
        if xa > 0.0 && c.x1 <= self.x0 {
            let max = self.x0 - c.x1;
            if max < xa { xa = max }
        }
        if xa < 0.0 && c.x0 >= self.x1 {
            let max = self.x1 - c.x0;
            if max > xa { xa = max }
        }
        xa
    }

    pub fn clip_y_collide(&self, c: &Aabb, mut ya: f64) -> f64 {
        if c.x1 <= self.x0 || c.x0 >= self.x1 { return ya }
        if c.z1 <= self.z0 || c.z0 >= self.z1 { return ya }
        if ya > 0.0 && c.y1 <= self.y0 {
            let max = self.y0 - c.y1;
            if max < ya { ya = max }
        }
        if ya < 0.0 && c.y0 >= self.y1 {
            let max = self.y1 - c.y0;
            if max > ya { ya = max }
        }
        ya
    }

    pub fn clip_z_collide(&self, c: &Aabb, mut za: f64) -> f64 {
        if c.x1 <= self.x0 || c.x0 >= self.x1 { return za }
        if c.y1 <= self.y0 || c.y0 >= self.y1 { return za }
        if za > 0.0 && c.z1 <= self.z0 {
            let max = self.z0 - c.z1;
            if max < za { za = max }
        }
        if za < 0.0 && c.z0 >= self.z1 {
            let max = self.z1 - c.z0;
            if max > za { za = max }
        }
        za
    }

    /// Strict overlap (touching faces do not count).
    pub fn intersects(&self, c: &Aabb) -> bool {
        self.intersects_coords(c.x0, c.y0, c.z0, c.x1, c.y1, c.z1)
    }

    pub fn intersects_coords(&self, x02: f64, y02: f64, z02: f64, x12: f64, y12: f64, z12: f64) -> bool {
        if x12 <= self.x0 || x02 >= self.x1 { return false }
        if y12 <= self.y0 || y02 >= self.y1 { return false }
        if z12 <= self.z0 || z02 >= self.z1 { return false }
        true
    }

    /// Inclusive overlap (touching faces count).
    pub fn intersects_inner(&self, c: &Aabb) -> bool {
        if c.x1 < self.x0 || c.x0 > self.x1 { return false }
        if c.y1 < self.y0 || c.y0 > self.y1 { return false }
        if c.z1 < self.z0 || c.z0 > self.z1 { return false }
        true
    }

    /// In-place move (original `move`).
    pub fn move_by(&mut self, xa: f64, ya: f64, za: f64) -> &mut Self {
        self.x0 += xa;
        self.y0 += ya;
        self.z0 += za;
        self.x1 += xa;
        self.y1 += ya;
        self.z1 += za;
        self
    }

    /// Strictly inside (all faces exclusive).
    pub fn contains(&self, p: &Vec3) -> bool {
        if p.x <= self.x0 || p.x >= self.x1 { return false }
        if p.y <= self.y0 || p.y >= self.y1 { return false }
        if p.z <= self.z0 || p.z >= self.z1 { return false }
        true
    }

    /// 4J addition: lower bound inclusive, upper exclusive.
    pub fn contains_including_lower_bound(&self, p: &Vec3) -> bool {
        if p.x < self.x0 || p.x >= self.x1 { return false }
        if p.y < self.y0 || p.y >= self.y1 { return false }
        if p.z < self.z0 || p.z >= self.z1 { return false }
        true
    }

    pub fn size(&self) -> f64 {
        // original divides by `3.0f`
        ((self.x1 - self.x0) + (self.y1 - self.y0) + (self.z1 - self.z0)) / 3.0
    }

    pub fn shrink(&self, xa: f64, ya: f64, za: f64) -> Aabb {
        Aabb::new(
            self.x0 + xa, self.y0 + ya, self.z0 + za,
            self.x1 - xa, self.y1 - ya, self.z1 - za,
        )
    }

    /// Ray/segment clip against the box faces; returns the nearest hit to `a`.
    pub fn clip(&self, a: &Vec3, b: &Vec3) -> Option<AabbHit> {
        // candidates in the original's priority order, with their face ids
        let cands: [(Option<Vec3>, i32); 6] = [
            (a.clip_x(b, self.x0).filter(|v| self.contains_x(v)), 4),
            (a.clip_x(b, self.x1).filter(|v| self.contains_x(v)), 5),
            (a.clip_y(b, self.y0).filter(|v| self.contains_y(v)), 0),
            (a.clip_y(b, self.y1).filter(|v| self.contains_y(v)), 1),
            (a.clip_z(b, self.z0).filter(|v| self.contains_z(v)), 2),
            (a.clip_z(b, self.z1).filter(|v| self.contains_z(v)), 3),
        ];
        let mut closest: Option<(Vec3, i32)> = None;
        for (v, face) in cands {
            if let Some(v) = v {
                // strict `<`: earlier candidate wins ties
                if closest.map_or(true, |(c, _)| a.distance_to_sqr(&v) < a.distance_to_sqr(&c)) {
                    closest = Some((v, face));
                }
            }
        }
        closest.map(|(pos, face)| AabbHit { face, pos })
    }

    pub fn contains_x(&self, v: &Vec3) -> bool {
        v.y >= self.y0 && v.y <= self.y1 && v.z >= self.z0 && v.z <= self.z1
    }

    pub fn contains_y(&self, v: &Vec3) -> bool {
        v.x >= self.x0 && v.x <= self.x1 && v.z >= self.z0 && v.z <= self.z1
    }

    pub fn contains_z(&self, v: &Vec3) -> bool {
        v.x >= self.x0 && v.x <= self.x1 && v.y >= self.y0 && v.y <= self.y1
    }
}

impl fmt::Display for Aabb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "box[{:.6}, {:.6}, {:.6} -> {:.6}, {:.6}, {:.6}]",
            self.x0, self.y0, self.z0, self.x1, self.y1, self.z1
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit() -> Aabb {
        Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0)
    }

    #[test]
    fn grow_shrink_expand() {
        assert_eq!(unit().grow(1.0, 0.0, 0.0), Aabb::new(-1.0, 0.0, 0.0, 2.0, 1.0, 1.0));
        assert_eq!(unit().shrink(0.25, 0.25, 0.25), Aabb::new(0.25, 0.25, 0.25, 0.75, 0.75, 0.75));
        assert_eq!(unit().expand(-2.0, 3.0, 0.0), Aabb::new(-2.0, 0.0, 0.0, 1.0, 4.0, 1.0));
    }

    #[test]
    fn intersection_rules() {
        let a = unit();
        let touching = a.clone_move(1.0, 0.0, 0.0);
        assert!(!a.intersects(&touching));
        assert!(a.intersects_inner(&touching));
        assert!(a.intersects(&a.clone_move(0.5, 0.5, 0.5)));
    }

    #[test]
    fn contains_rules() {
        let a = unit();
        assert!(a.contains(&Vec3::new(0.5, 0.5, 0.5)));
        assert!(!a.contains(&Vec3::new(0.0, 0.5, 0.5)));
        assert!(a.contains_including_lower_bound(&Vec3::new(0.0, 0.5, 0.5)));
        assert!(!a.contains_including_lower_bound(&Vec3::new(1.0, 0.5, 0.5)));
    }

    #[test]
    fn collide_clipping() {
        let wall = Aabb::new(2.0, 0.0, 0.0, 3.0, 1.0, 1.0);
        let mover = unit();
        assert_eq!(wall.clip_x_collide(&mover, 5.0), 1.0);
        assert_eq!(wall.clip_y_collide(&mover, 5.0), 5.0); // no overlap in x → unobstructed
        let below = Aabb::new(0.0, -1.0, 0.0, 1.0, 0.0, 1.0);
        assert_eq!(below.clip_y_collide(&mover, -5.0), 0.0);
    }

    #[test]
    fn segment_clip_nearest_face() {
        let b = unit();
        let a = Vec3::new(-1.0, 0.5, 0.5);
        let c = Vec3::new(2.0, 0.5, 0.5);
        let hit = b.clip(&a, &c).unwrap();
        assert_eq!(hit.face, 4);
        assert_eq!(hit.pos, Vec3::new(0.0, 0.5, 0.5));
        // from the other side hits +X face
        assert_eq!(b.clip(&c, &a).unwrap().face, 5);
        // miss
        assert!(b.clip(&Vec3::new(-1.0, 5.0, 0.5), &Vec3::new(2.0, 5.0, 0.5)).is_none());
    }

    #[test]
    fn point_distance() {
        let b = unit();
        assert_eq!(Vec3::new(0.5, 0.5, 0.5).distance_to_aabb(&b), 0.0);
        assert_eq!(Vec3::new(4.0, 0.5, 0.5).distance_to_aabb(&b), 3.0);
        assert_eq!(b.size(), 1.0);
    }
}
