//! Port of `Vec3.cpp`.
//!
//! The C++ class used per-thread pools of 1024 recycled `Vec3` objects
//! (`newTemp`) to avoid allocation. In Rust `Vec3` is a plain `Copy` value, so
//! the pool (and its thread-local setup functions) has no equivalent.

use crate::aabb::Aabb;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// `0.0000001f` as a double (the literal is a float in the original).
const CLIP_EPS: f64 = 1e-7_f32 as f64;

impl Vec3 {
    /// `newPermanent`: canonicalises `-0.0` to `0.0`.
    pub fn new(mut x: f64, mut y: f64, mut z: f64) -> Self {
        if x == 0.0 { x = 0.0; }
        if y == 0.0 { y = 0.0; }
        if z == 0.0 { z = 0.0; }
        Vec3 { x, y, z }
    }

    /// `newTemp`: does *not* canonicalise (matches the original).
    const fn tmp(x: f64, y: f64, z: f64) -> Self {
        Vec3 { x, y, z }
    }

    pub fn interpolate_to(&self, t: &Vec3, p: f64) -> Vec3 {
        Vec3::tmp(
            self.x + (t.x - self.x) * p,
            self.y + (t.y - self.y) * p,
            self.z + (t.z - self.z) * p,
        )
    }

    pub fn vector_to(&self, p: &Vec3) -> Vec3 {
        Vec3::tmp(p.x - self.x, p.y - self.y, p.z - self.z)
    }

    pub fn normalize(&self) -> Vec3 {
        let dist = (self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if dist < 0.0001 {
            return Vec3::tmp(0.0, 0.0, 0.0);
        }
        Vec3::tmp(self.x / dist, self.y / dist, self.z / dist)
    }

    pub fn dot(&self, p: &Vec3) -> f64 {
        self.x * p.x + self.y * p.y + self.z * p.z
    }

    pub fn cross(&self, p: &Vec3) -> Vec3 {
        Vec3::tmp(
            self.y * p.z - self.z * p.y,
            self.z * p.x - self.x * p.z,
            self.x * p.y - self.y * p.x,
        )
    }

    pub fn add(&self, x: f64, y: f64, z: f64) -> Vec3 {
        Vec3::tmp(self.x + x, self.y + y, self.z + z)
    }

    pub fn distance_to(&self, p: &Vec3) -> f64 {
        self.distance_to_sqr(p).sqrt()
    }

    pub fn distance_to_sqr(&self, p: &Vec3) -> f64 {
        self.distance_to_sqr_xyz(p.x, p.y, p.z)
    }

    pub fn distance_to_sqr_xyz(&self, x2: f64, y2: f64, z2: f64) -> f64 {
        let xd = x2 - self.x;
        let yd = y2 - self.y;
        let zd = z2 - self.z;
        xd * xd + yd * yd + zd * zd
    }

    pub fn scale(&self, l: f64) -> Vec3 {
        Vec3::tmp(self.x * l, self.y * l, self.z * l)
    }

    pub fn length(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    /// Point where segment `self -> b` crosses the plane `x = xt`, if any.
    pub fn clip_x(&self, b: &Vec3, xt: f64) -> Option<Vec3> {
        let xd = b.x - self.x;
        let yd = b.y - self.y;
        let zd = b.z - self.z;
        if xd * xd < CLIP_EPS {
            return None;
        }
        let d = (xt - self.x) / xd;
        if !(0.0..=1.0).contains(&d) {
            return None;
        }
        Some(Vec3::tmp(self.x + xd * d, self.y + yd * d, self.z + zd * d))
    }

    pub fn clip_y(&self, b: &Vec3, yt: f64) -> Option<Vec3> {
        let xd = b.x - self.x;
        let yd = b.y - self.y;
        let zd = b.z - self.z;
        if yd * yd < CLIP_EPS {
            return None;
        }
        let d = (yt - self.y) / yd;
        if !(0.0..=1.0).contains(&d) {
            return None;
        }
        Some(Vec3::tmp(self.x + xd * d, self.y + yd * d, self.z + zd * d))
    }

    pub fn clip_z(&self, b: &Vec3, zt: f64) -> Option<Vec3> {
        let xd = b.x - self.x;
        let yd = b.y - self.y;
        let zd = b.z - self.z;
        if zd * zd < CLIP_EPS {
            return None;
        }
        let d = (zt - self.z) / zd;
        if !(0.0..=1.0).contains(&d) {
            return None;
        }
        Some(Vec3::tmp(self.x + xd * d, self.y + yd * d, self.z + zd * d))
    }

    pub fn lerp(&self, v: &Vec3, a: f64) -> Vec3 {
        Vec3::tmp(
            self.x + (v.x - self.x) * a,
            self.y + (v.y - self.y) * a,
            self.z + (v.z - self.z) * a,
        )
    }

    // The rotations take radians in an f32, but trig is done in f64 (see the
    // original comment: "cos/sin were floats but seems pointless wasting precision").
    pub fn x_rot(&mut self, degs: f32) {
        let (c, s) = ((degs as f64).cos(), (degs as f64).sin());
        let yy = self.y * c + self.z * s;
        let zz = self.z * c - self.y * s;
        self.y = yy;
        self.z = zz;
    }

    pub fn y_rot(&mut self, degs: f32) {
        let (c, s) = ((degs as f64).cos(), (degs as f64).sin());
        let xx = self.x * c + self.z * s;
        let zz = self.z * c - self.x * s;
        self.x = xx;
        self.z = zz;
    }

    pub fn z_rot(&mut self, degs: f32) {
        let (c, s) = ((degs as f64).cos(), (degs as f64).sin());
        let xx = self.x * c + self.y * s;
        let yy = self.y * c - self.x * s;
        self.x = xx;
        self.y = yy;
    }

    /// 0 if inside the box, otherwise distance to it. (4J addition)
    pub fn distance_to_aabb(&self, b: &Aabb) -> f64 {
        if b.contains(self) {
            return 0.0;
        }
        let mut xd = 0.0;
        let mut yd = 0.0;
        let mut zd = 0.0;
        if self.x < b.x0 { xd = b.x0 - self.x } else if self.x > b.x1 { xd = self.x - b.x1 }
        if self.y < b.y0 { yd = b.y0 - self.y } else if self.y > b.y1 { yd = self.y - b.y1 }
        if self.z < b.z0 { zd = b.z0 - self.z } else if self.z > b.z1 { zd = self.z - b.z1 }
        (xd * xd + yd * yd + zd * zd).sqrt()
    }

    /// Closest point to `self` on segment `p1..p2`. Uses `f32` for the dot
    /// products and the parameter `t`, exactly like the original.
    pub fn closest_point_on_line(&self, p1: &Vec3, p2: &Vec3) -> Vec3 {
        let diff = Vec3::tmp(self.x - p1.x, self.y - p1.y, self.z - p1.z);
        let dir = Vec3::tmp(p2.x - p1.x, p2.y - p1.y, p2.z - p1.z);
        let dot1 = diff.dot(&dir) as f32;
        if dot1 <= 0.0 {
            return *p1;
        }
        let dot2 = dir.dot(&dir) as f32;
        if dot2 <= dot1 {
            return *p2;
        }
        let t = (dot1 / dot2) as f64;
        Vec3::tmp(p1.x + t * dir.x, p1.y + t * dir.y, p1.z + t * dir.z)
    }

    pub fn distance_from_line(&self, p1: &Vec3, p2: &Vec3) -> f64 {
        let c = self.closest_point_on_line(p1, p2);
        Vec3::tmp(self.x - c.x, self.y - c.y, self.z - c.z).length()
    }
}

impl fmt::Display for Vec3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:.6},{:.6},{:.6})", self.x, self.y, self.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basics() {
        let a = Vec3::new(1.0, 0.0, 0.0);
        let b = Vec3::new(0.0, 1.0, 0.0);
        assert_eq!(a.dot(&b), 0.0);
        assert_eq!(a.cross(&b), Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(a.distance_to(&b), 2f64.sqrt());
        assert_eq!(Vec3::new(3.0, 4.0, 0.0).length(), 5.0);
        assert_eq!(Vec3::new(0.0, 0.0, 0.00001).normalize(), Vec3::default());
        assert_eq!(a.lerp(&b, 0.5), Vec3::new(0.5, 0.5, 0.0));
        assert_eq!(a.to_string(), "(1.000000,0.000000,0.000000)");
    }

    #[test]
    fn negative_zero_canonical() {
        let v = Vec3::new(-0.0, -0.0, -0.0);
        assert!(v.x.is_sign_positive() && v.y.is_sign_positive() && v.z.is_sign_positive());
    }

    #[test]
    fn clipping() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(10.0, 10.0, 0.0);
        assert_eq!(a.clip_x(&b, 5.0), Some(Vec3::new(5.0, 5.0, 0.0)));
        assert_eq!(a.clip_x(&b, 11.0), None);
        assert_eq!(a.clip_z(&b, 1.0), None); // parallel to the plane
        assert_eq!(a.clip_y(&b, -1.0), None);
    }

    #[test]
    fn rotation() {
        let mut v = Vec3::new(1.0, 0.0, 0.0);
        v.y_rot(std::f32::consts::FRAC_PI_2);
        assert!((v.x).abs() < 1e-6 && (v.z + 1.0).abs() < 1e-6);
    }

    #[test]
    fn line_helpers() {
        let p1 = Vec3::new(0.0, 0.0, 0.0);
        let p2 = Vec3::new(10.0, 0.0, 0.0);
        let p = Vec3::new(5.0, 3.0, 0.0);
        assert_eq!(p.closest_point_on_line(&p1, &p2), Vec3::new(5.0, 0.0, 0.0));
        assert_eq!(p.distance_from_line(&p1, &p2), 3.0);
        assert_eq!(Vec3::new(-4.0, 0.0, 0.0).closest_point_on_line(&p1, &p2), p1);
        assert_eq!(Vec3::new(40.0, 0.0, 0.0).closest_point_on_line(&p1, &p2), p2);
    }
}
