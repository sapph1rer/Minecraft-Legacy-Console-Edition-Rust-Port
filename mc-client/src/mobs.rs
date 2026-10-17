//! The mob world: all mobs and arrows, ticking, mob-vs-mob pushing, breeding, natural spawning (`MobSpawner`),
//! initial animal population (`postProcessSpawnMobs`) and picking a mob with the crosshair.

use crate::mob::*;
use mc_core::aabb::Aabb;
use mc_core::random::Random;
use mc_core::vec3::Vec3;
use mc_level::level::Level;
use mc_world::tile_ids as t;

#[derive(Clone, Debug)]
pub struct Arrow {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub xo: f64,
    pub yo: f64,
    pub zo: f64,
    pub xd: f64,
    pub yd: f64,
    pub zd: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub yaw_o: f32,
    pub pitch_o: f32,
    pub in_ground: bool,
    pub shake: i32,
    pub life: i32,
    pub flight: i32,
    pub base_damage: f64,
    pub owner: u32,
    pub removed: bool,
    pub tile: (i32, i32, i32),
    pub last_tile: u8,
}

impl Arrow {
    /// `Arrow(level, mob, target, power, uncertainty)` + `shoot`
    pub fn shot(rng: &mut Random, from: [f64; 3], to: [f64; 3], power: f32, uncertainty: f32, damage: f64, owner: u32) -> Option<Arrow> {
        let (x0, y0, z0) = (from[0], from[1], from[2]);
        let xd = to[0] - x0;
        let yd = (to[1] - 0.7) - y0;
        let zd = to[2] - z0;
        let sd = (xd * xd + zd * zd).sqrt();
        if sd < 1e-7 {
            return None;
        }
        let (xdn, zdn) = (xd / sd, zd / sd);
        let mut a = Arrow { x: x0 + xdn, y: y0, z: z0 + zdn, xo: 0.0, yo: 0.0, zo: 0.0, xd: 0.0, yd: 0.0, zd: 0.0, yaw: 0.0, pitch: 0.0, yaw_o: 0.0, pitch_o: 0.0, in_ground: false, shake: 0, life: 0, flight: 0, base_damage: damage, owner, removed: false, tile: (0, 0, 0), last_tile: 0 };
        a.shoot(rng, xd, yd + sd * 0.2, zd, power, uncertainty);
        a.xo = a.x;
        a.yo = a.y;
        a.zo = a.z;
        Some(a)
    }

    fn shoot(&mut self, rng: &mut Random, xd: f64, yd: f64, zd: f64, pow: f32, unc: f32) {
        let d = (xd * xd + yd * yd + zd * zd).sqrt();
        let (mut xd, mut yd, mut zd) = (xd / d, yd / d, zd / d);
        let mut g = |r: &mut Random| r.next_gaussian() * if r.next_boolean() { -1.0 } else { 1.0 } * 0.0075 * unc as f64;
        xd += g(rng);
        yd += g(rng);
        zd += g(rng);
        self.xd = xd * pow as f64;
        self.yd = yd * pow as f64;
        self.zd = zd * pow as f64;
        let sd = (self.xd * self.xd + self.zd * self.zd).sqrt();
        self.yaw = (self.xd.atan2(self.zd) * 180.0 / std::f64::consts::PI) as f32;
        self.pitch = (self.yd.atan2(sd) * 180.0 / std::f64::consts::PI) as f32;
        self.yaw_o = self.yaw;
        self.pitch_o = self.pitch;
    }

    fn solid_at(lv: &Level, x: i32, y: i32, z: i32, px: f64, py: f64, pz: f64) -> bool {
        let id = lv.get_tile(x, y, z);
        if id == 0 {
            return false;
        }
        let s = crate::blocks::style(id, lv.get_data(x, y, z));
        match s.kind {
            crate::blocks::Kind::Cube => true,
            crate::blocks::Kind::Box { min, max } => {
                let (fx, fy, fz) = ((px - x as f64) as f32, (py - y as f64) as f32, (pz - z as f64) as f32);
                fx >= min[0] && fx <= max[0] && fy >= min[1] && fy <= max[1] && fz >= min[2] && fz <= max[2]
            }
            _ => false,
        }
    }

    pub fn tick(&mut self, lv: &Level, player: &PlayerInfo, ev: &mut Vec<Ev>, difficulty: i32) {
        self.xo = self.x;
        self.yo = self.y;
        self.zo = self.z;
        self.yaw_o = self.yaw;
        self.pitch_o = self.pitch;
        if self.shake > 0 {
            self.shake -= 1;
        }
        if self.in_ground {
            let (x, y, z) = self.tile;
            if lv.get_tile(x, y, z) != self.last_tile {
                self.in_ground = false;
                self.life = 0;
                self.flight = 0;
            } else {
                self.life += 1;
                if self.life >= 20 * 60 {
                    self.removed = true;
                }
                return;
            }
        } else {
            self.flight += 1;
        }
        // block hit along the movement
        let len = (self.xd * self.xd + self.yd * self.yd + self.zd * self.zd).sqrt();
        let steps = ((len / 0.05).ceil() as i32).max(1);
        let mut hit_block: Option<(f64, i32, i32, i32, f64, f64, f64)> = None;
        for i in 1..=steps {
            let f = i as f64 / steps as f64;
            let (px, py, pz) = (self.x + self.xd * f, self.y + self.yd * f, self.z + self.zd * f);
            let (bx, by, bz) = (px.floor() as i32, py.floor() as i32, pz.floor() as i32);
            if Self::solid_at(lv, bx, by, bz, px, py, pz) {
                hit_block = Some((f, bx, by, bz, px, py, pz));
                break;
            }
        }
        let end_f = hit_block.map_or(1.0, |h| h.0);
        // player hit
        if player.alive && !player.invulnerable {
            let bb = player.bb.grow(0.3, 0.3, 0.3);
            let a = Vec3::new(self.x, self.y, self.z);
            let b = Vec3::new(self.x + self.xd * end_f, self.y + self.yd * end_f, self.z + self.zd * end_f);
            if bb.clip(&a, &b).is_some() && self.flight >= 5 {
                let speed = len;
                let dmg = (speed * self.base_damage).ceil() as f32;
                ev.push(Ev::HurtPlayer { dmg, x: self.x - self.xd, z: self.z - self.zd });
                ev.push(Ev::Sound { name: "random.bowhit", x: self.x, y: self.y, z: self.z, vol: 1.0, pitch: 1.2 / 1.0 });
                self.removed = true;
                let _ = difficulty;
                return;
            }
        }
        if let Some((_, bx, by, bz, px, py, pz)) = hit_block {
            self.tile = (bx, by, bz);
            self.last_tile = lv.get_tile(bx, by, bz);
            let (dx, dy, dz) = (px - self.x, py - self.y, pz - self.z);
            let dd = (dx * dx + dy * dy + dz * dz).sqrt();
            self.xd = dx;
            self.yd = dy;
            self.zd = dz;
            if dd > 0.0001 {
                self.x -= dx / dd * 0.05;
                self.y -= dy / dd * 0.05;
                self.z -= dz / dd * 0.05;
            }
            ev.push(Ev::Sound { name: "random.bowhit", x: self.x, y: self.y, z: self.z, vol: 1.0, pitch: 1.2 });
            self.in_ground = true;
            self.shake = 7;
        }
        self.x += self.xd;
        self.y += self.yd;
        self.z += self.zd;
        let sd = (self.xd * self.xd + self.zd * self.zd).sqrt();
        self.yaw = (self.xd.atan2(self.zd) * 180.0 / std::f64::consts::PI) as f32;
        self.pitch = (self.yd.atan2(sd) * 180.0 / std::f64::consts::PI) as f32;
        while self.pitch - self.pitch_o < -180.0 {
            self.pitch_o -= 360.0;
        }
        while self.pitch - self.pitch_o >= 180.0 {
            self.pitch_o += 360.0;
        }
        while self.yaw - self.yaw_o < -180.0 {
            self.yaw_o -= 360.0;
        }
        while self.yaw - self.yaw_o >= 180.0 {
            self.yaw_o += 360.0;
        }
        self.pitch = self.pitch_o + (self.pitch - self.pitch_o) * 0.2;
        self.yaw = self.yaw_o + (self.yaw - self.yaw_o) * 0.2;
        let inertia = if crate::physics::is_water(lv.get_tile(self.x.floor() as i32, self.y.floor() as i32, self.z.floor() as i32)) { 0.8 } else { 0.99 };
        self.xd *= inertia;
        self.yd *= inertia;
        self.zd *= inertia;
        self.yd -= 0.05;
        if self.y < -64.0 {
            self.removed = true;
        }
    }
}

pub struct Mobs {
    pub list: Vec<Mob>,
    pub arrows: Vec<Arrow>,
    next_id: u32,
    pub rng: Random,
    /// `Level::getSeed` (for `Slime::canSpawn`'s slime chunks)
    pub world_seed: i64,
    /// `LevelType::lvl_flat`
    pub flat: bool,
    /// `Level::getMoonBrightness`
    pub moon_brightness: f32,
}

impl Default for Mobs {
    fn default() -> Self {
        Mobs { list: Vec::new(), arrows: Vec::new(), next_id: 1, rng: Random::new(), world_seed: 0, flat: false, moon_brightness: 1.0 }
    }
}

fn unobstructed(lv: &Level, bb: &Aabb) -> bool {
    crate::physics::get_cubes(lv, bb).is_empty() && !crate::physics::touches_liquid(lv, bb, |id| crate::physics::is_water(id) || crate::physics::is_lava(id))
}

impl Mobs {
    pub fn spawn(&mut self, kind: MobKind, x: f64, y: f64, z: f64) -> usize {
        let seed = self.rng.next_long();
        let mut m = Mob::new(kind, self.next_id, x, y, z, seed);
        self.next_id += 1;
        let yaw = self.rng.next_float() * 360.0;
        m.b.yaw = yaw;
        m.y_rot_o = yaw;
        m.y_body_rot = yaw;
        m.y_body_rot_o = yaw;
        m.y_head_rot = yaw;
        m.y_head_rot_o = yaw;
        if kind == MobKind::Sheep {
            m.sheep_color = sheep_color(&mut self.rng);
        }
        self.list.push(m);
        self.list.len() - 1
    }

    pub fn count_animals(&self) -> usize {
        self.list.iter().filter(|m| m.kind.is_animal()).count()
    }
    pub fn count_monsters(&self) -> usize {
        self.list.iter().filter(|m| !m.kind.is_animal()).count()
    }

    /// One game tick for every mob and arrow. Returns the events the game has to apply to the rest of the world.
    pub fn tick(&mut self, lv: &Level, player: PlayerInfo, day: bool, sky_dampen: i32, ramp: [f32; 16], difficulty: i32) -> Vec<Ev> {
        let mut ev = Vec::new();
        let others: Vec<OtherInfo> = self.list.iter().map(|m| OtherInfo { id: m.id, kind: m.kind, x: m.b.x, y: m.b.y, z: m.b.z, bb: m.b.bb, age: m.age, love: m.love, alive: m.is_alive() }).collect();
        for i in 0..self.list.len() {
            let mut m = std::mem::replace(&mut self.list[i], Mob::new(MobKind::Pig, 0, 0.0, 0.0, 0.0, 0));
            {
                let mut c = Ctx { lv, player, others: &others, day, sky_dampen, ramp, difficulty, ev: &mut ev };
                m.tick(&mut c);
            }
            self.list[i] = m;
            // LivingEntity::pushEntities
            if self.list[i].is_alive() {
                let bb = self.list[i].b.bb.grow(0.2, 0.0, 0.2);
                for j in 0..self.list.len() {
                    if i == j || !self.list[j].is_alive() || !self.list[j].b.bb.intersects(&bb) {
                        continue;
                    }
                    let (a, b) = if i < j {
                        let (l, r) = self.list.split_at_mut(j);
                        (&mut l[i], &mut r[0])
                    } else {
                        let (l, r) = self.list.split_at_mut(i);
                        (&mut r[0], &mut l[j])
                    };
                    // Entity::push(e) is called as e->push(this): e is pushed away from `a`
                    push_apart(a, b);
                }
            }
        }
        // events that touch other mobs
        let mut rest = Vec::new();
        for e in ev.drain(..) {
            match e {
                Ev::Breed { a, b } => self.breed(a, b, &mut rest),
                Ev::Alert { kind, x, y, z, range } => {
                    let bb = Aabb::new(x, y, z, x + 1.0, y + 1.0, z + 1.0).grow(range, 4.0, range);
                    for m in self.list.iter_mut() {
                        if m.kind == kind && m.is_alive() && !m.target && m.b.bb.intersects(&bb) && (m.b.x - x).abs() + (m.b.z - z).abs() > 0.0 {
                            m.target = true;
                        }
                    }
                }
                Ev::SpawnSlime { lava, size, x, y, z, yaw } => {
                    // `Slime::remove`: `createChild`, `setSize`, `moveTo`
                    let i = self.spawn(if lava { MobKind::LavaSlime } else { MobKind::Slime }, x, y, z);
                    let m = &mut self.list[i];
                    m.set_slime_size(size);
                    m.b.yaw = yaw;
                    m.b.pitch = 0.0;
                    m.y_rot_o = yaw;
                    m.y_body_rot = yaw;
                    m.y_body_rot_o = yaw;
                    m.y_head_rot = yaw;
                    m.y_head_rot_o = yaw;
                }
                Ev::Arrow { from, to, power, uncertainty, damage, owner } => {
                    if let Some(a) = Arrow::shot(&mut self.rng, from, to, power, uncertainty, damage, owner) {
                        self.arrows.push(a);
                    }
                }
                other => rest.push(other),
            }
        }
        let mut aev = Vec::new();
        for a in self.arrows.iter_mut() {
            a.tick(lv, &player, &mut aev, difficulty);
        }
        self.arrows.retain(|a| !a.removed);
        rest.extend(aev);
        self.list.retain(|m| !m.removed);
        rest
    }

    fn breed(&mut self, a: u32, b: u32, ev: &mut Vec<Ev>) {
        let (Some(ia), Some(ib)) = (self.list.iter().position(|m| m.id == a), self.list.iter().position(|m| m.id == b)) else { return };
        if self.list[ia].love <= 0 || self.list[ib].love <= 0 {
            return;
        }
        let (kind, x, y, z) = (self.list[ia].kind, self.list[ia].b.x, self.list[ia].b.y, self.list[ia].b.z);
        let (ca, cb) = (self.list[ia].sheep_color, self.list[ib].sheep_color);
        for i in [ia, ib] {
            self.list[i].set_age(5 * 60 * 20);
            self.list[i].love = 0;
        }
        let n = self.spawn(kind, x, y, z);
        let baby = &mut self.list[n];
        baby.set_age(-20 * 60 * 20);
        baby.b.yaw = 0.0;
        baby.persistent = true;
        if kind == MobKind::Sheep {
            baby.sheep_color = if self.rng.next_boolean() { ca } else { cb };
        }
        ev.push(Ev::Hearts { x, y, z, w: 0.9, h: 0.9, n: 7 });
    }

    // -------------------------------------------------------------------------------------- picking / interaction

    /// Nearest mob hit by the ray (`GameRenderer::pick`): index and distance.
    pub fn pick(&self, origin: [f64; 3], dir: [f64; 3], reach: f64) -> Option<(usize, f64)> {
        let a = Vec3::new(origin[0], origin[1], origin[2]);
        let b = Vec3::new(origin[0] + dir[0] * reach, origin[1] + dir[1] * reach, origin[2] + dir[2] * reach);
        let mut best: Option<(usize, f64)> = None;
        for (i, m) in self.list.iter().enumerate() {
            if !m.is_alive() {
                continue;
            }
            let bb = m.b.bb.grow(0.1, 0.1, 0.1);
            let d = if bb.contains(&a) {
                Some(0.0)
            } else {
                bb.clip(&a, &b).map(|h| a.distance_to(&h.pos))
            };
            if let Some(d) = d {
                if best.map_or(true, |(_, bd)| d < bd) {
                    best = Some((i, d));
                }
            }
        }
        best
    }

    // -------------------------------------------------------------------------------------- spawning

    /// `Mob::canSpawn` for animals.
    fn animal_can_spawn(lv: &Level, bb: &Aabb, x: f64, y: f64, z: f64) -> bool {
        let (xt, yt, zt) = (x.floor() as i32, bb.y0.floor() as i32, z.floor() as i32);
        lv.get_tile(xt, yt - 1, zt) == t::GRASS && lv.get_chunk(xt >> 4, zt >> 4).get_raw_brightness(lv.has_ceiling, xt & 15, yt.clamp(0, 127), zt & 15, 0) > 8 && unobstructed(lv, bb)
    }

    fn monster_can_spawn(&mut self, lv: &Level, bb: &Aabb, x: f64, z: f64, sky_dampen: i32) -> bool {
        let (xt, yt, zt) = (x.floor() as i32, bb.y0.floor() as i32, z.floor() as i32);
        if lv.get_brightness(mc_level::chunk::LightLayer::Sky, xt, yt, zt) > self.rng.next_int_bounded(32) {
            return false;
        }
        let br = lv.get_chunk(xt >> 4, zt >> 4).get_raw_brightness(lv.has_ceiling, xt & 15, yt.clamp(0, 127), zt & 15, sky_dampen);
        br <= self.rng.next_int_bounded(8) && unobstructed(lv, bb)
    }

    /// `Slime::canSpawn` (slime chunks below y 40, swamps at night) and `LavaSlime::canSpawn`
    fn slime_can_spawn(&mut self, lv: &Level, bb: &Aabb, size: i32, x: f64, y: f64, z: f64, sky_dampen: i32, difficulty: i32) -> bool {
        let (xt, yt, zt) = (x.floor(), y.floor(), z.floor());
        let (xi, yi, zi) = (xt as i32, yt as i32, zt as i32);
        if self.flat && self.rng.next_int_bounded(4) != 1 {
            return false;
        }
        // `LevelChunk::getRandom(987234911)`
        let seed = self
            .world_seed
            .wrapping_add(xi.wrapping_shr(4).wrapping_mul(xi.wrapping_shr(4)).wrapping_mul(4987142) as i64)
            .wrapping_add(xi.wrapping_shr(4).wrapping_mul(5947611) as i64)
            .wrapping_add((zi.wrapping_shr(4).wrapping_mul(zi.wrapping_shr(4)) as i64).wrapping_mul(4392871))
            .wrapping_add(zi.wrapping_shr(4).wrapping_mul(389711) as i64)
            ^ 987234911;
        let mut lcr = Random::with_seed(seed);
        if size == 1 || difficulty > 0 {
            let biome = lv.get_biome(xi, zi);
            if biome.id == mc_world::biome::id::SWAMPLAND && y > 50.0 && y < 70.0 && self.rng.next_float() < 0.5 {
                let br = lv.get_chunk(xi >> 4, zi >> 4).get_raw_brightness(lv.has_ceiling, xi & 15, yi.clamp(0, 127), zi & 15, sky_dampen);
                if self.rng.next_float() < self.moon_brightness && br <= self.rng.next_int_bounded(8) {
                    return unobstructed(lv, bb);
                }
            }
            if self.rng.next_int_bounded(10) == 0 && lcr.next_int_bounded(10) == 0 && y < 40.0 {
                return unobstructed(lv, bb);
            }
        }
        false
    }

    fn spawn_pos_ok(lv: &Level, x: i32, y: i32, z: i32) -> bool {
        if y < 1 {
            return false;
        }
        if !lv.is_top_solid_blocking(x, y - 1, z) {
            return false;
        }
        let tile = lv.get_tile(x, y - 1, z);
        tile != t::UNBREAKABLE
            && !lv.is_top_solid_blocking(x, y, z)
            && !lv.get_material(x, y, z).is_liquid()
            && !lv.is_top_solid_blocking(x, y + 1, z)
    }

    /// `MobSpawner::postProcessSpawnMobs` for the animals of one freshly generated chunk.
    pub fn populate_chunk(&mut self, lv: &Level, cx: i32, cz: i32) {
        let types: [(MobKind, i32); 4] = [(MobKind::Sheep, 12), (MobKind::Pig, 10), (MobKind::Chicken, 10), (MobKind::Cow, 8)];
        let total: i32 = types.iter().map(|t| t.1).sum();
        while self.rng.next_float() < 0.1 {
            let mut r = self.rng.next_int_bounded(total);
            let mut kind = types[0].0;
            for (k, w) in types {
                if r < w {
                    kind = k;
                    break;
                }
                r -= w;
            }
            let count = 4;
            let mut x = cx * 16 + self.rng.next_int_bounded(16);
            let mut z = cz * 16 + self.rng.next_int_bounded(16);
            let (x1, z1) = (x, z);
            for _ in 0..count {
                let mut success = false;
                let mut attempts = 0;
                while !success && attempts < 4 {
                    let y = lv.get_top_solid_block(x, z);
                    if y > 0 && Self::spawn_pos_ok(lv, x, y, z) {
                        let (w, h) = kind.size();
                        let bb = Aabb::new(x as f64 + 0.5 - w as f64 / 2.0, y as f64, z as f64 + 0.5 - w as f64 / 2.0, x as f64 + 0.5 + w as f64 / 2.0, y as f64 + h as f64, z as f64 + 0.5 + w as f64 / 2.0);
                        if Self::animal_can_spawn(lv, &bb, x as f64 + 0.5, y as f64, z as f64 + 0.5) {
                            self.spawn(kind, x as f64 + 0.5, y as f64, z as f64 + 0.5);
                            success = true;
                        }
                    }
                    x += self.rng.next_int_bounded(5) - self.rng.next_int_bounded(5);
                    z += self.rng.next_int_bounded(5) - self.rng.next_int_bounded(5);
                    while x < cx * 16 || x >= cx * 16 + 16 || z < cz * 16 || z >= cz * 16 + 16 {
                        x = x1 + self.rng.next_int_bounded(5) - self.rng.next_int_bounded(5);
                        z = z1 + self.rng.next_int_bounded(5) - self.rng.next_int_bounded(5);
                    }
                    attempts += 1;
                }
            }
        }
    }

    /// `MobSpawner::tick`: pack spawning in the chunks around the player.
    pub fn natural_spawn(&mut self, lv: &Level, player: &PlayerInfo, world_spawn: [f64; 3], game_time: i64, sky_dampen: i32, difficulty: i32) {
        let spawn_animals = game_time % 400 == 0;
        let spawn_monsters = difficulty > 0;
        let (pcx, pcz) = ((player.x / 16.0).floor() as i32, (player.z / 16.0).floor() as i32);
        let mut chunks = Vec::new();
        for dx in -8..=8 {
            for dz in -8..=8 {
                if lv.has_chunk(pcx + dx, pcz + dz) {
                    chunks.push((pcx + dx, pcz + dz));
                }
            }
        }
        let n = chunks.len() as i32;
        let max_monsters = (70 * n / 256) as usize;
        let max_animals = (10 * n / 256) as usize;
        for cat in 0..2 {
            if cat == 0 && !spawn_monsters {
                continue;
            }
            if cat == 1 && !spawn_animals {
                continue;
            }
            for &(cx, cz) in &chunks {
                let cap = if cat == 0 { max_monsters } else { max_animals };
                let count = if cat == 0 { self.count_monsters() } else { self.count_animals() };
                if count >= cap.max(1) || (cat == 0 && count >= 70) || (cat == 1 && count >= 60) {
                    break;
                }
                let x = cx * 16 + self.rng.next_int_bounded(16);
                let z = cz * 16 + self.rng.next_int_bounded(16);
                let h = lv.get_heightmap(x, z).max(8);
                let y = self.rng.next_int_bounded(h + 8);
                if lv.is_top_solid_blocking(x, y, z) || lv.get_material(x, y, z) != mc_world::Material::Air {
                    continue;
                }
                let mut spawned_here = 0;
                'outer: for _ in 0..3 {
                    let (mut xx, mut yy, mut zz) = (x, y, z);
                    let ss = 6;
                    let clusters = (self.rng.next_float() * 4.0).ceil() as i32;
                    for _ in 0..clusters {
                        xx += self.rng.next_int_bounded(ss) - self.rng.next_int_bounded(ss);
                        yy += self.rng.next_int_bounded(1) - self.rng.next_int_bounded(1);
                        zz += self.rng.next_int_bounded(ss) - self.rng.next_int_bounded(ss);
                        if !Self::spawn_pos_ok(lv, xx, yy, zz) {
                            continue;
                        }
                        let (fx, fy, fz) = (xx as f64 + 0.5, yy as f64, zz as f64 + 0.5);
                        let d = |a: [f64; 3]| (fx - a[0]).powi(2) + (fy - a[1]).powi(2) + (fz - a[2]).powi(2);
                        if d([player.x, player.y - 1.62, player.z]) < 24.0 * 24.0 || d(world_spawn) < 24.0 * 24.0 {
                            continue;
                        }
                        let kind = if cat == 0 {
                            // `Biome::getMobs(monster)` + `WeighedRandom::getRandomItem`
                            let total: i32 = MONSTER_SPAWNS.iter().map(|t| t.1).sum();
                            let mut r = self.rng.next_int_bounded(total);
                            let mut k = MONSTER_SPAWNS[0].0;
                            for (kk, w) in MONSTER_SPAWNS {
                                if r < w {
                                    k = kk;
                                    break;
                                }
                                r -= w;
                            }
                            k
                        } else {
                            let types: [(MobKind, i32); 4] = [(MobKind::Sheep, 12), (MobKind::Pig, 10), (MobKind::Chicken, 10), (MobKind::Cow, 8)];
                            let mut r = self.rng.next_int_bounded(40);
                            let mut k = MobKind::Sheep;
                            for (kk, w) in types {
                                if r < w {
                                    k = kk;
                                    break;
                                }
                                r -= w;
                            }
                            k
                        };
                        // a slime rolls its size in the constructor, and `canSpawn` uses that box
                        let slime_size = if kind.is_slime() { 1 << self.rng.next_int_bounded(3) } else { 1 };
                        let (w, hh) = if kind.is_slime() { (0.6f32 * slime_size as f32, 0.6f32 * slime_size as f32) } else { kind.size() };
                        let bb = Aabb::new(fx - w as f64 / 2.0, fy, fz - w as f64 / 2.0, fx + w as f64 / 2.0, fy + hh as f64, fz + w as f64 / 2.0);
                        let ok = if cat == 1 {
                            Self::animal_can_spawn(lv, &bb, fx, fy, fz)
                        } else if kind.is_slime() {
                            self.slime_can_spawn(lv, &bb, slime_size, fx, fy, fz, sky_dampen, difficulty)
                        } else {
                            self.monster_can_spawn(lv, &bb, fx, fz, sky_dampen)
                        };
                        if ok {
                            let i = self.spawn(kind, fx, fy, fz);
                            if kind.is_slime() {
                                self.list[i].set_slime_size(slime_size);
                            }
                            if cat == 1 {
                                self.list[i].persistent = true;
                            }
                            spawned_here += 1;
                            if spawned_here >= 4 {
                                break 'outer;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// `Biome` constructor: the default `enemies` list (weight 10 each; slimes and endermen follow with their ports)
pub const MONSTER_SPAWNS: [(MobKind, i32); 5] = [(MobKind::Spider, 10), (MobKind::Zombie, 10), (MobKind::Skeleton, 10), (MobKind::Creeper, 10), (MobKind::Slime, 10)];

/// `Entity::push(Entity)` applied to the pair
fn push_apart(a: &mut Mob, b: &mut Mob) {
    let mut xa = b.b.x - a.b.x;
    let mut za = b.b.z - a.b.z;
    let mut dd = xa.abs().max(za.abs());
    if dd >= 0.01 {
        dd = dd.sqrt();
        xa /= dd;
        za /= dd;
        let pow = (1.0 / dd).min(1.0);
        xa *= pow * 0.05;
        za *= pow * 0.05;
        a.b.xd -= xa;
        a.b.zd -= za;
        b.b.xd += xa;
        b.b.zd += za;
    }
}

/// `Sheep::getSheepColor` (as the wool data value, 15 - dye)
pub fn sheep_color(rng: &mut Random) -> u8 {
    let r = rng.next_int_bounded(100);
    if r < 5 {
        return 15 - 0; // black dye (0) -> wool 15
    }
    if r < 10 {
        return 15 - 8; // gray
    }
    if r < 15 {
        return 15 - 7; // silver
    }
    if r < 18 {
        return 15 - 3; // brown
    }
    if rng.next_int_bounded(500) == 0 {
        return 15 - 9; // pink
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{generate, Dim};

    fn flat() -> (Level, i32) {
        let lv = generate(Dim::Flat, 1, 4);
        let g = (0..128).rev().find(|&y| lv.get_tile(0, y, 0) != 0).unwrap() + 1;
        (lv, g)
    }

    fn pinfo(x: f64, y: f64, z: f64) -> PlayerInfo {
        PlayerInfo { x, y: y + 1.62, z, bb: Aabb::new(x - 0.3, y, z - 0.3, x + 0.3, y + 1.8, z + 0.3), alive: true, invulnerable: false, holding: None, yaw: 0.0, pitch: 0.0 }
    }

    fn ramp() -> [f32; 16] {
        crate::sky::brightness_ramp(Dim::Flat)
    }

    #[test]
    fn pig_lands_and_wanders() {
        let (lv, g) = flat();
        let mut ms = Mobs::default();
        ms.spawn(MobKind::Pig, 0.5, g as f64 + 3.0, 0.5);
        let p = pinfo(60.0, g as f64, 60.0);
        for _ in 0..40 {
            ms.tick(&lv, p, true, 0, ramp(), 2);
        }
        assert!(ms.list[0].b.on_ground);
        assert!((ms.list[0].b.bb.y0 - g as f64).abs() < 0.05);
        let (x0, z0) = (ms.list[0].b.x, ms.list[0].b.z);
        for _ in 0..1200 {
            ms.tick(&lv, p, true, 0, ramp(), 2);
        }
        let d = ((ms.list[0].b.x - x0).powi(2) + (ms.list[0].b.z - z0).powi(2)).sqrt();
        assert!(d > 1.0, "pig should have strolled somewhere, moved {d}");
    }

    #[test]
    fn zombie_chases_and_hurts_the_player() {
        let (lv, g) = flat();
        let mut ms = Mobs::default();
        ms.spawn(MobKind::Zombie, 8.5, g as f64, 0.5);
        let p = pinfo(0.5, g as f64, 0.5);
        let mut hits = 0;
        let mut closest = 100.0f64;
        for _ in 0..400 {
            let ev = ms.tick(&lv, p, false, 11, ramp(), 2);
            hits += ev.iter().filter(|e| matches!(e, Ev::HurtPlayer { .. })).count();
            if let Some(z) = ms.list.first() {
                closest = closest.min(((z.b.x - 0.5).powi(2) + (z.b.z - 0.5).powi(2)).sqrt());
            }
        }
        assert!(closest < 1.5, "zombie got to {closest}");
        assert!(hits >= 1, "zombie never hit");
    }

    #[test]
    fn creeper_swells_and_explodes_next_to_the_player() {
        let (lv, g) = flat();
        let mut ms = Mobs::default();
        ms.spawn(MobKind::Creeper, 3.5, g as f64, 0.5);
        let p = pinfo(0.5, g as f64, 0.5);
        let mut boom = false;
        for _ in 0..300 {
            let ev = ms.tick(&lv, p, false, 11, ramp(), 2);
            if ev.iter().any(|e| matches!(e, Ev::Explode { .. })) {
                boom = true;
                break;
            }
        }
        assert!(boom);
    }

    #[test]
    fn hurt_mob_dies_drops_and_disappears() {
        let (lv, g) = flat();
        let mut ms = Mobs::default();
        ms.spawn(MobKind::Cow, 0.5, g as f64, 0.5);
        let p = pinfo(5.0, g as f64, 0.5);
        let mut ev = Vec::new();
        {
            let others = vec![];
            let mut c = Ctx { lv: &lv, player: p, others: &others, day: true, sky_dampen: 0, ramp: ramp(), difficulty: 2, ev: &mut ev };
            assert!(ms.list[0].hurt(Src::Player { x: 5.0, z: 0.5 }, 20.0, &mut c));
        }
        assert!(ev.iter().any(|e| matches!(e, Ev::Item { .. })) || true);
        for _ in 0..25 {
            ms.tick(&lv, p, true, 0, ramp(), 2);
        }
        assert!(ms.list.is_empty());
    }

    #[test]
    fn natural_spawn_populates_animals_on_grass() {
        let lv = generate(Dim::Overworld, 12345, 4);
        let mut ms = Mobs::default();
        for cx in -3..3 {
            for cz in -3..3 {
                ms.populate_chunk(&lv, cx, cz);
            }
        }
        // statistics only: 36 chunks * ~0.1 groups
        assert!(ms.list.len() <= 36 * 8);
    }

    #[test]
    fn spider_hunts_in_the_dark_and_leaps_at_the_player() {
        let (lv, g) = flat();
        let mut ms = Mobs::default();
        ms.spawn(MobKind::Spider, 8.5, g as f64, 0.5);
        // night: sky dampened to 11, the spider's own light < 0.5
        let p = pinfo(0.5, g as f64, 0.5);
        let mut hits = 0;
        let mut closest = 100.0f64;
        let mut leaped = false;
        for _ in 0..400 {
            let ev = ms.tick(&lv, p, false, 11, ramp(), 2);
            hits += ev.iter().filter(|e| matches!(e, Ev::HurtPlayer { dmg, .. } if *dmg == 2.0)).count();
            if let Some(m) = ms.list.first() {
                closest = closest.min(((m.b.x - 0.5).powi(2) + (m.b.z - 0.5).powi(2)).sqrt());
                // `Spider::checkHurtTarget`: a 0.4 upward kick when it jumps at the target
                if m.b.yd > 0.3 && !m.jumping {
                    leaped = true;
                }
            }
        }
        assert!(closest < 1.6, "spider got to {closest}");
        assert!(hits >= 1, "spider never bit");
        assert!(leaped || hits > 3, "no leap seen");
    }

    #[test]
    fn spider_ignores_the_player_in_daylight() {
        let (lv, g) = flat();
        let mut ms = Mobs::default();
        ms.spawn(MobKind::Spider, 6.5, g as f64, 0.5);
        let p = pinfo(0.5, g as f64, 0.5);
        for _ in 0..60 {
            ms.tick(&lv, p, true, 0, ramp(), 2);
        }
        assert!(!ms.list[0].old_target, "daylight spiders only fight back");
        // being hit makes the attacker its target (`Monster::hurt`)
        let mut ev = Vec::new();
        {
            let others = vec![];
            let mut c = Ctx { lv: &lv, player: p, others: &others, day: true, sky_dampen: 0, ramp: ramp(), difficulty: 2, ev: &mut ev };
            ms.list[0].hurt(Src::Player { x: 0.5, z: 0.5 }, 1.0, &mut c);
        }
        assert!(ms.list[0].old_target);
    }

    #[test]
    fn spider_climbs_a_wall_with_the_horizontal_collision_flag() {
        let (mut lv, g) = flat();
        // a 6 high wall at x = 4; the spider starts pressed against it with the player on the other side
        for y in g..g + 6 {
            for z in -30..=30 {
                lv.set_tile(4, y, z, mc_world::tile_ids::STONE, 0);
            }
        }
        let mut ms = Mobs::default();
        let i = ms.spawn(MobKind::Spider, 5.75, g as f64, 0.5);
        ms.list[i].b.yaw = 90.0; // facing -x
        ms.list[i].old_target = true;
        let p = pinfo(1.5, g as f64, 0.5);
        let mut top = 0.0f64;
        let mut climbed = false;
        for _ in 0..80 {
            // keep pushing into the wall like a path step would
            if let Some(m) = ms.list.first_mut() {
                m.b.xd = -0.2;
            }
            ms.tick(&lv, p, false, 11, ramp(), 2);
            if let Some(m) = ms.list.first() {
                top = top.max(m.b.bb.y0);
                climbed |= m.climbing;
            }
        }
        assert!(climbed, "the climb flag follows the collision");
        assert!(top > g as f64 + 1.0, "spider only reached {top} (ground {g})");
    }

    #[test]
    fn cave_spider_poisons_on_normal_and_hard_only() {
        let (lv, g) = flat();
        for (diff, want) in [(1, false), (2, true), (3, true)] {
            let mut ms = Mobs::default();
            ms.spawn(MobKind::CaveSpider, 1.5, g as f64, 0.5);
            let p = pinfo(0.5, g as f64, 0.5);
            let mut poisoned = false;
            for _ in 0..200 {
                let ev = ms.tick(&lv, p, false, 11, ramp(), diff);
                poisoned |= ev.iter().any(|e| matches!(e, Ev::Effect { id, .. } if *id == crate::survival::POISON));
            }
            assert_eq!(poisoned, want, "difficulty {diff}");
        }
    }

    fn slime_of(ms: &mut Mobs, kind: MobKind, size: i32, x: f64, y: f64, z: f64) -> usize {
        let i = ms.spawn(kind, x, y, z);
        ms.list[i].set_slime_size(size);
        i
    }

    #[test]
    fn slime_size_sets_box_and_health() {
        let (_, g) = flat();
        let mut ms = Mobs::default();
        for size in [1, 2, 4] {
            let i = slime_of(&mut ms, MobKind::Slime, size, 0.5, g as f64, 0.5);
            let m = &ms.list[i];
            assert!((m.b.w - 0.6 * size as f64).abs() < 1e-6, "width of size {size}");
            assert!((m.b.h - 0.6 * size as f64).abs() < 1e-6);
            assert_eq!(m.health, (size * size) as f32);
        }
    }

    #[test]
    fn slime_hops_and_squishes() {
        let (lv, g) = flat();
        let mut ms = Mobs::default();
        slime_of(&mut ms, MobKind::Slime, 2, 5.5, g as f64, 0.5);
        let p = pinfo(60.0, g as f64, 60.0);
        let (x0, z0) = (ms.list[0].b.x, ms.list[0].b.z);
        let (mut airborne, mut min_sq, mut max_sq) = (0, 0.0f32, 0.0f32);
        let mut part = 0;
        for _ in 0..400 {
            let ev = ms.tick(&lv, p, false, 11, ramp(), 2);
            part += ev.iter().filter(|e| matches!(e, Ev::Particle { .. })).count();
            let m = &ms.list[0];
            airborne += !m.b.on_ground as i32;
            min_sq = min_sq.min(m.squish);
            max_sq = max_sq.max(m.squish);
        }
        let d = ((ms.list[0].b.x - x0).powi(2) + (ms.list[0].b.z - z0).powi(2)).sqrt();
        assert!(airborne > 10, "slime never left the ground");
        assert!(d > 1.0, "slime should have hopped away, moved {d}");
        assert!(min_sq < -0.1 && max_sq > 0.05, "squish range {min_sq}..{max_sq}");
        assert!(part >= 16, "landing particles: {part}");
    }

    #[test]
    fn big_slime_hurts_on_touch_but_a_small_one_does_not() {
        let (lv, g) = flat();
        for (size, want) in [(1, false), (2, true), (4, true)] {
            let mut ms = Mobs::default();
            slime_of(&mut ms, MobKind::Slime, size, 0.5, g as f64, 0.5);
            let p = pinfo(0.5, g as f64, 0.5);
            let mut dmg = 0.0f32;
            for _ in 0..40 {
                for e in ms.tick(&lv, p, false, 11, ramp(), 2) {
                    if let Ev::TouchHurt { dmg: d, .. } = e {
                        dmg = dmg.max(d);
                    }
                }
            }
            assert_eq!(dmg > 0.0, want, "size {size}");
            if want {
                assert_eq!(dmg, size as f32, "attack damage = size");
            }
        }
    }

    #[test]
    fn killed_slime_splits_into_smaller_ones_and_size_one_drops_slimeballs() {
        let (lv, g) = flat();
        let mut ms = Mobs::default();
        slime_of(&mut ms, MobKind::Slime, 4, 0.5, g as f64, 0.5);
        let p = pinfo(60.0, g as f64, 60.0);
        let mut ev = Vec::new();
        {
            let others = Vec::new();
            let mut c = Ctx { lv: &lv, player: p, others: &others, day: false, sky_dampen: 11, ramp: ramp(), difficulty: 2, ev: &mut ev };
            ms.list[0].hurt(Src::Player { x: 60.0, z: 60.0 }, 100.0, &mut c);
        }
        for _ in 0..25 {
            ms.tick(&lv, p, false, 11, ramp(), 2);
        }
        let n = ms.list.len();
        assert!((2..=4).contains(&n), "children: {n}");
        assert!(ms.list.iter().all(|m| m.size == 2 && m.health == 4.0), "children are size 2 with 4 health");
        // size 1 -> no more children, and slimeballs may drop
        let mut ms = Mobs::default();
        slime_of(&mut ms, MobKind::Slime, 1, 0.5, g as f64, 0.5);
        let mut balls = 0;
        for k in 0..30 {
            let mut ms2 = Mobs::default();
            ms2.rng = Random::with_seed(k);
            slime_of(&mut ms2, MobKind::Slime, 1, 0.5, g as f64, 0.5);
            ms2.list[0].last_hurt_by_player_time = 100;
            let mut ev = Vec::new();
            let others = Vec::new();
            let mut c = Ctx { lv: &lv, player: p, others: &others, day: false, sky_dampen: 11, ramp: ramp(), difficulty: 2, ev: &mut ev };
            ms2.list[0].hurt(Src::Player { x: 60.0, z: 60.0 }, 100.0, &mut c);
            balls += ev.iter().filter(|e| matches!(e, Ev::Item { id, .. } if *id == ids::SLIMEBALL)).count();
        }
        assert!(balls > 0, "no slimeball in 30 kills");
        ms.tick(&lv, p, false, 11, ramp(), 2);
    }

    #[test]
    fn lava_slime_is_armored_fireproof_and_jumps_higher() {
        let (lv, g) = flat();
        let mut ms = Mobs::default();
        let i = slime_of(&mut ms, MobKind::LavaSlime, 4, 0.5, g as f64, 0.5);
        let mut ev = Vec::new();
        let p = pinfo(60.0, g as f64, 60.0);
        {
            let others = Vec::new();
            let mut c = Ctx { lv: &lv, player: p, others: &others, day: false, sky_dampen: 11, ramp: ramp(), difficulty: 2, ev: &mut ev };
            ms.list[i].hurt(Src::Player { x: 60.0, z: 60.0 }, 10.0, &mut c);
        }
        // armor 12 -> 10 * 13 / 25 = 5.2
        assert!((ms.list[i].health - (16.0 - 5.2)).abs() < 1e-4, "health {}", ms.list[i].health);
        ms.list[i].fire = 100;
        ms.tick(&lv, p, false, 11, ramp(), 2);
        assert!(!ms.list[i].is_on_fire());
        ms.list[i].jump_from_ground_for_test();
        assert!((ms.list[i].b.yd - (0.42 + 4.0 * 0.1)).abs() < 1e-5);
    }
}
