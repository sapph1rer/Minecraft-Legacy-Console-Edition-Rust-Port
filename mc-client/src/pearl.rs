//! `ThrownEnderpearl` (a `Throwable`): thrown at 1.5 blocks / tick, falls with gravity 0.03, breaks on the first block or
//! mob it meets, throws up 32 portal particles and teleports the thrower to the impact point for 5 points of fall damage.

use crate::entity::Body;
use crate::game::Game;
use crate::physics::raycast;
use mc_core::random::Random;
use mc_level::level::Level;

pub const PEARL_ITEM: u16 = 368;
const GRAVITY: f64 = 0.03;

#[derive(Clone, Debug)]
pub struct Pearl {
    pub body: Body,
    pub life: i32,
    pub removed: bool,
}

/// What a tick of a pearl did.
pub enum Outcome {
    Flying,
    /// hit something at this point (the last free position of the ray)
    Hit([f64; 3]),
    Gone,
}

impl Pearl {
    /// `Throwable::Throwable(level, mob)` + `shoot(xd, yd, zd, 1.5, 1.0)`; `eye` is the thrower's eye position.
    pub fn throw(eye: [f64; 3], yaw: f32, pitch: f32, r: &mut Random) -> Pearl {
        let mut body = Body::new(0.25, 0.25, 0.0);
        let yr = (yaw as f64).to_radians();
        let pr = (pitch as f64).to_radians();
        // the spawn offset: -cos(yaw) * 0.16, -0.1, -sin(yaw) * 0.16
        body.set_pos(eye[0] - yr.cos() * 0.16, eye[1] - 0.1, eye[2] - yr.sin() * 0.16);
        let mut xd = -yr.sin() * pr.cos();
        let mut yd = -pr.sin();
        let mut zd = yr.cos() * pr.cos();
        // `Throwable::shoot`: normalise, add 0.0075 * inaccuracy of gaussian noise, scale by the power
        let n = (xd * xd + yd * yd + zd * zd).sqrt();
        let (power, inacc) = (1.5, 1.0);
        xd = xd / n + r.next_gaussian() * 0.0075 * inacc;
        yd = yd / n + r.next_gaussian() * 0.0075 * inacc;
        zd = zd / n + r.next_gaussian() * 0.0075 * inacc;
        body.xd = xd * power;
        body.yd = yd * power;
        body.zd = zd * power;
        body.xo = body.x;
        body.yo = body.y;
        body.zo = body.z;
        Pearl { body, life: 0, removed: false }
    }

    /// `Throwable::tick`. `mob_hit` finds a mob along (origin, dir, len) and returns the distance.
    pub fn tick(&mut self, lv: &Level, mob_hit: &dyn Fn([f64; 3], [f64; 3], f64) -> Option<f64>) -> Outcome {
        let b = &mut self.body;
        b.xo = b.x;
        b.yo = b.y;
        b.zo = b.z;
        self.life += 1;
        let o = [b.x, b.y, b.z];
        let len = (b.xd * b.xd + b.yd * b.yd + b.zd * b.zd).sqrt();
        if len > 0.0 {
            let dir = [b.xd / len, b.yd / len, b.zd / len];
            let bd = raycast(lv, o, dir, len).map(|h| h.distance);
            let md = mob_hit(o, dir, len);
            let hit = match (bd, md) {
                (Some(a), Some(c)) => Some(a.min(c)),
                (a, c) => a.or(c),
            };
            if let Some(d) = hit {
                self.removed = true;
                let d = (d - 0.05).max(0.0);
                return Outcome::Hit([o[0] + dir[0] * d, o[1] + dir[1] * d, o[2] + dir[2] * d]);
            }
        }
        b.x += b.xd;
        b.y += b.yd;
        b.z += b.zd;
        b.set_pos(b.x, b.y, b.z);
        let mut inertia = 0.99;
        b.update_water(lv);
        if b.in_water {
            inertia = 0.8;
        }
        b.xd *= inertia;
        b.yd *= inertia;
        b.zd *= inertia;
        b.yd -= GRAVITY;
        if self.life > 1200 || b.y < -64.0 {
            self.removed = true;
            return Outcome::Gone;
        }
        Outcome::Flying
    }
}

impl Game {
    /// `EnderpearlItem::use`: throw one pearl (survival uses it up).
    pub fn throw_pearl(&mut self) -> bool {
        let Some(held) = self.inv.selected_item().copied() else { return false };
        if held.id != PEARL_ITEM {
            return false;
        }
        if !self.creative {
            self.inv.use_selected();
        }
        let eye = [self.player.x, self.player.y, self.player.z];
        let p = Pearl::throw(eye, self.player.yaw, self.player.pitch, &mut self.random);
        self.pearls.push(p);
        let pit = 0.4 / (self.random.next_float() * 0.4 + 0.8);
        self.play("random.bow", eye[0], eye[1], eye[2], 0.5, pit);
        self.swing();
        true
    }

    pub fn tick_pearls(&mut self) {
        if self.pearls.is_empty() {
            return;
        }
        let mut list = std::mem::take(&mut self.pearls);
        let mut hits = Vec::new();
        for p in list.iter_mut() {
            let mobs = &self.mobs;
            let f = |o: [f64; 3], d: [f64; 3], len: f64| mobs.pick(o, d, len).map(|(_, dist)| dist);
            if let Outcome::Hit(at) = p.tick(&self.level, &f) {
                hits.push(at);
            }
        }
        list.retain(|p| !p.removed);
        self.pearls = list;
        for at in hits {
            self.pearl_lands(at);
        }
    }

    fn portal_particle(&mut self, x: f64, y: f64, z: f64, xa: f64, ya: f64, za: f64) {
        let p = crate::particles::Particle::portal(&mut self.random, x, y, z, xa, ya, za);
        self.particles.add(p);
    }

    /// `ThrownEnderpearl::onHit`: 32 portal particles, then the thrower is teleported and hurt by 5.
    fn pearl_lands(&mut self, at: [f64; 3]) {
        for _ in 0..32 {
            let ya = self.random.next_double() * 2.0;
            let xa = self.random.next_gaussian();
            let za = self.random.next_gaussian();
            self.portal_particle(at[0], at[1] + ya, at[2], xa, 0.0, za);
        }
        if self.health <= 0 {
            return;
        }
        // keep the feet out of blocks: lift the target until the player's box is free
        let feet_off = self.player.y - self.player.bb.y0;
        let mut y = at[1];
        for _ in 0..4 {
            let w = 0.3;
            let bb = mc_core::aabb::Aabb::new(at[0] - w, y, at[2] - w, at[0] + w, y + 1.8, at[2] + w);
            if crate::physics::get_cubes(&self.level, &bb).is_empty() {
                break;
            }
            y += 1.0;
        }
        self.player.set_pos(at[0], y + feet_off, at[2]);
        self.player.xd = 0.0;
        self.player.yd = 0.0;
        self.player.zd = 0.0;
        self.fall_distance = 0.0;
        self.prev = [self.player.x, self.player.y, self.player.z];
        self.play("mob.endermen.portal", at[0], at[1], at[2], 1.0, 1.0);
        self.hurt_bypass(5);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::Input;
    use crate::world::{generate, Dim};
    use mc_world::item::ItemStack;

    fn game() -> Game {
        let lv = generate(Dim::Flat, 1, 3);
        let mut g = Game::new(lv, Dim::Flat, [0.5, 4.0, 0.5]);
        for _ in 0..5 {
            g.tick(&Input::default());
        }
        g.creative = false;
        g.inv = crate::inventory::Inventory::new();
        g
    }

    #[test]
    fn pearl_teleports_and_hurts() {
        let mut g = game();
        g.inv.items[g.inv.selected] = Some({ let mut s = ItemStack::new(PEARL_ITEM, 1); s.count = 2; s });
        g.player.pitch = 20.0;
        g.player.yaw = 0.0;
        let (x0, z0) = (g.player.x, g.player.z);
        assert!(g.throw_pearl());
        assert_eq!(g.inv.selected_item().unwrap().count, 1);
        for _ in 0..80 {
            g.tick(&Input::default());
            if g.pearls.is_empty() {
                break;
            }
        }
        assert!(g.pearls.is_empty());
        assert!(g.particles.particles.iter().filter(|p| p.portal.is_some()).count() >= 32, "portal particles at the landing");
        for _ in 0..20 {
            g.tick(&Input::default());
        }
        assert!((g.player.z - z0).abs() > 2.0 || (g.player.x - x0).abs() > 2.0, "moved to {} {}", g.player.x, g.player.z);
        assert_eq!(g.health, 15);
    }

    #[test]
    fn pearl_stops_in_a_wall() {
        let mut g = game();
        g.player.pitch = 0.0;
        let mut r = Random::new();
        let mut p = Pearl::throw([0.5, 5.0, 0.5], 0.0, -90.0, &mut r);
        let f = |_: [f64; 3], _: [f64; 3], _: f64| None;
        for _ in 0..200 {
            if let Outcome::Hit(_) = p.tick(&g.level, &f) {
                break;
            }
        }
        assert!(p.removed);
    }
}
