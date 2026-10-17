//! Game ↔ mob world glue: ticking, applying mob events, player damage / death, attacking and feeding mobs, explosions.

use crate::entity::ItemEntity;
use crate::game::Game;
use crate::mob::*;
use crate::mobs::Mobs;
use mc_core::random::Random;
use mc_level::level::UPDATE_ALL;
use mc_world::item::{ItemKind, ItemStack};
use mc_world::tile::tile_info;
use mc_world::tile_ids as t;

impl Game {
    /// Switch mobs on and populate the generated chunks (`MobSpawner::postProcessSpawnMobs`).
    pub fn enable_mobs(&mut self) {
        self.spawn_mobs = true;
        let mut m = Mobs::default();
        m.world_seed = self.world_seed;
        m.flat = self.dim == crate::world::Dim::Flat;
        let mut coords: Vec<(i32, i32)> = self.level.chunk_coords().collect();
        coords.sort();
        for (cx, cz) in coords {
            m.populate_chunk(&self.level, cx, cz);
        }
        self.mobs = m;
    }

    pub fn player_info(&self) -> PlayerInfo {
        let p = &self.player;
        PlayerInfo { x: p.x, y: p.y, z: p.z, bb: p.bb, alive: self.health > 0, invulnerable: self.creative, holding: self.inv.selected_item().map(|s| s.id), yaw: p.yaw, pitch: p.pitch }
    }

    /// (isDay, skyDampen, brightness ramp)
    pub fn sky_state(&self) -> (bool, i32, [f32; 16]) {
        let td = crate::sky::time_of_day(self.dim, self.day_time, 1.0);
        let br = ((crate::sky::sky_darken(td, 0.0, 0.0) - 0.2) / 0.8).clamp(0.0, 1.0);
        let dampen = ((1.0 - br) * 11.0) as i32;
        (dampen < 4, dampen, crate::sky::brightness_ramp(self.dim))
    }

    pub(crate) fn tick_mobs(&mut self) {
        if self.invuln > 0 {
            self.invuln -= 1;
        }
        if self.hurt_time > 0 {
            self.hurt_time -= 1;
        }
        if !self.spawn_mobs {
            return;
        }
        let info = self.player_info();
        let (day, dampen, ramp) = self.sky_state();
        self.mobs.moon_brightness = [1.0, 0.75, 0.5, 0.25, 0.0, 0.25, 0.5, 0.75][crate::sky::moon_phase(self.day_time)];
        if self.difficulty > 0 || self.level.game_time % 400 == 0 {
            let sp = self.spawn_pos;
            let gt = self.level.game_time;
            self.mobs.natural_spawn(&self.level, &info, sp, gt, dampen, self.difficulty);
        }
        let ev = self.mobs.tick(&self.level, info, day, dampen, ramp, self.difficulty);
        // peaceful: monsters vanish
        if self.difficulty == 0 {
            self.mobs.list.retain(|m| m.kind.is_animal());
        }
        self.apply_mob_events(ev);
    }

    pub(crate) fn apply_mob_events(&mut self, evs: Vec<Ev>) {
        for e in evs {
            match e {
                Ev::Sound { name, x, y, z, vol, pitch } => self.play(name, x, y, z, vol, pitch),
                Ev::Item { id, damage, count, x, y, z } => {
                    let it = ItemEntity::new(x, y + 0.0, z, ItemStack { id, count, damage }, &mut self.random);
                    self.entities.items.push(it);
                }
                Ev::HurtPlayer { dmg, x, z } => {
                    self.hurt_player(dmg, x, z);
                }
                Ev::TouchHurt { dmg, x, z, sound } => {
                    // `Slime::playerTouch`: the attack sound only when `player->hurt` went through
                    if self.hurt_player(dmg, x, z) {
                        let (px, py, pz) = (self.player.x, self.player.y, self.player.z);
                        let pitch = (self.random.next_float() - self.random.next_float()) * 0.2 + 1.0;
                        self.play(sound, px, py, pz, 1.0, pitch);
                    }
                }
                Ev::Particle { lava, x, y, z } => {
                    // `eParticleType_slime` is a `BreakingItemParticle` of the slimeball; the flame particle is not ported yet
                    if !lava {
                        let part = crate::particles::Particle::item_crack(&mut self.random, x, y, z, 0.0, 0.0, 0.0, (1, 14));
                        self.particles.add(part);
                    }
                }
                Ev::Effect { id, duration, amplifier } => {
                    if !self.creative {
                        self.add_effect(crate::survival::Effect { id, duration, amplifier });
                    }
                }
                Ev::Xp { amount, x, y, z } => self.spawn_xp(x, y, z, amount),
                Ev::Explode { x, y, z, radius } => self.explode(x, y, z, radius),
                Ev::Block { x, y, z, id, data } => {
                    self.level.set_tile_and_data(x, y, z, id, data, UPDATE_ALL);
                    self.mark(x, z);
                }
                _ => {}
            }
        }
    }

    /// `Player::hurt` for damage coming from a mob / arrow at (sx, sz).
    pub fn hurt_player(&mut self, dmg: f32, sx: f64, sz: f64) -> bool {
        if self.creative || self.health <= 0 || self.difficulty == 0 {
            return false;
        }
        let mut dmg = dmg;
        if self.difficulty == 1 {
            dmg = (dmg / 2.0 + 1.0).min(dmg);
        } else if self.difficulty == 3 {
            dmg = dmg * 3.0 / 2.0;
        }
        if self.invuln > 10 {
            if dmg <= self.last_hurt {
                return false;
            }
            let d = dmg - self.last_hurt;
            self.last_hurt = dmg;
            self.damage_player(d);
        } else {
            self.last_hurt = dmg;
            self.invuln = 20;
            self.hurt_time = 10;
            self.hurt_dir = ((sz - self.player.z).atan2(sx - self.player.x).to_degrees() as f32) - self.player.yaw;
            self.damage_player(dmg);
            let (px, pz) = (self.player.x, self.player.z);
            let (mut xd, mut zd) = (sx - px, sz - pz);
            while xd * xd + zd * zd < 0.0001 {
                xd = (self.random.next_double() - self.random.next_double()) * 0.01;
                zd = (self.random.next_double() - self.random.next_double()) * 0.01;
            }
            let dd = (xd * xd + zd * zd).sqrt();
            let p = &mut self.player;
            p.xd /= 2.0;
            p.yd /= 2.0;
            p.zd /= 2.0;
            p.xd -= xd / dd * 0.4;
            p.zd -= zd / dd * 0.4;
            p.yd += 0.4;
            if p.yd > 0.4 {
                p.yd = 0.4;
            }
        }
        let (x, y, z) = (self.player.x, self.player.y, self.player.z);
        let pitch = (self.random.next_float() - self.random.next_float()) * 0.2 + 1.0;
        self.play("damage.hit", x, y, z, 1.0, pitch);
        if self.health <= 0 {
            self.kill_player();
        }
        true
    }

    fn damage_player(&mut self, d: f32) {
        let armor = self.inv.armor_value();
        let d = d * (25 - armor.min(25)) as f32 / 25.0;
        self.last_health = self.health;
        self.health -= d.ceil() as i32;
        // `Player::actuallyHurt`: damage that gets through costs exhaustion
        self.add_exhaustion(crate::survival::fc::EXHAUSTION_ATTACK);
    }

    /// Survival death (`Player::die`): the inventory and some experience spill, the death screen opens and the
    /// player stays dead until `respawn`.
    pub(crate) fn kill_player(&mut self) {
        if self.dead {
            return;
        }
        self.fall_distance = 0.0;
        let (x, y, z) = (self.player.x, self.player.y, self.player.z);
        for i in 0..self.inv.items.len() {
            if let Some(st) = self.inv.items[i].take() {
                let mut e = ItemEntity::new(x, y - 1.0, z, st, &mut self.random);
                e.throw_time = 40;
                self.entities.items.push(e);
            }
        }
        // `Player::getExperienceReward`
        let xp = (self.xp_level * 7).min(100);
        if xp > 0 {
            self.spawn_xp(x, y - 1.0, z, xp);
        }
        self.health = 0;
        self.dead = true;
        self.eat_left = 0;
        self.open_screen = Some(crate::screens::ScreenId::Death);
    }

    /// The respawn button: back at the spawn point with a fresh body (the score is kept for the screen only).
    pub fn respawn(&mut self) {
        self.dead = false;
        self.health = 20;
        self.last_health = 20;
        self.food = 20;
        self.last_food = 20;
        self.saturation = 5.0;
        self.exhaustion = 0.0;
        self.food_timer = 0;
        self.effects.clear();
        self.air = crate::survival::TOTAL_AIR_SUPPLY;
        self.xp_level = 0;
        self.xp_progress = 0.0;
        self.xp_total = 0;
        self.score = 0;
        self.invuln = 0;
        self.hurt_time = 0;
        let sp = self.spawn_pos;
        self.player.set_pos(sp[0], sp[1], sp[2]);
        self.player.xd = 0.0;
        self.player.yd = 0.0;
        self.player.zd = 0.0;
    }

    /// `Player::attack` on the mob under the crosshair. Returns true if the click was spent on a mob.
    pub(crate) fn try_attack_mob(&mut self) -> bool {
        let Some((i, _)) = self.mob_target else { return false };
        if i >= self.mobs.list.len() {
            return false;
        }
        let held = self.inv.selected_item().copied();
        let mut dmg = 1.0f32;
        if let Some(h) = held {
            dmg += match h.kind() {
                ItemKind::Sword(tr) => 3.0 + tr.attack_bonus(),
                ItemKind::Axe(tr) => 2.0 + tr.attack_bonus(),
                ItemKind::Pickaxe(tr) => 1.0 + tr.attack_bonus(),
                ItemKind::Shovel(tr) => tr.attack_bonus(),
                _ => 0.0,
            };
        }
        // critical hit: falling
        let p = &self.player;
        if p.yd < 0.0 && !p.on_ground && !p.in_water && !p.flying {
            dmg += self.random.next_int_bounded(dmg as i32 / 2 + 2) as f32;
        }
        let (px, pz) = (self.player.x, self.player.z);
        self.hurt_mob(i, Src::Player { x: px, z: pz }, dmg);
        self.add_exhaustion(crate::survival::fc::EXHAUSTION_ATTACK);
        if !self.creative {
            if let Some(h) = held {
                let wear = match h.kind() {
                    ItemKind::Sword(_) => 1,
                    ItemKind::Pickaxe(_) | ItemKind::Shovel(_) | ItemKind::Axe(_) => 2,
                    _ => 0,
                };
                if wear > 0 && self.inv.damage_selected(wear) {
                    self.play("random.break", px, self.player.y, pz, 0.8, 1.0);
                }
            }
        }
        true
    }

    pub fn hurt_mob(&mut self, i: usize, src: Src, dmg: f32) {
        let info = self.player_info();
        let (day, dampen, ramp) = self.sky_state();
        let mut ev = Vec::new();
        {
            let others: Vec<OtherInfo> = Vec::new();
            let mut c = Ctx { lv: &self.level, player: info, others: &others, day, sky_dampen: dampen, ramp, difficulty: self.difficulty, ev: &mut ev };
            self.mobs.list[i].hurt(src, dmg, &mut c);
        }
        self.apply_mob_events(ev);
    }

    /// `Animal::mobInteract` (feeding) — right click on the mob under the crosshair.
    pub(crate) fn try_interact_mob(&mut self) -> bool {
        let Some((i, _)) = self.mob_target else { return false };
        let Some(held) = self.inv.selected_item().copied() else { return false };
        let Some(m) = self.mobs.list.get_mut(i) else { return false };
        if !m.kind.is_animal() || !m.kind.food().contains(&held.id) || m.age != 0 || m.love > 0 {
            return false;
        }
        m.love = 600;
        m.target = false;
        if !self.creative {
            self.inv.use_selected();
        }
        self.swing();
        true
    }

    /// `Explosion::explode` + `finalizeExplosion` (blocks, entity damage, drops 1/radius).
    pub fn explode(&mut self, x: f64, y: f64, z: f64, radius: f32) {
        let ep = (1.0 + (self.random.next_float() - self.random.next_float()) * 0.2) * 0.7;
        self.play("random.explode", x, y, z, 4.0, ep);
        let mut destroyed: std::collections::HashSet<(i32, i32, i32)> = std::collections::HashSet::new();
        let size = 16;
        for xx in 0..size {
            for yy in 0..size {
                for zz in 0..size {
                    if !(xx == 0 || xx == size - 1 || yy == 0 || yy == size - 1 || zz == 0 || zz == size - 1) {
                        continue;
                    }
                    let mut xd = xx as f64 / (size - 1) as f64 * 2.0 - 1.0;
                    let mut yd = yy as f64 / (size - 1) as f64 * 2.0 - 1.0;
                    let mut zd = zz as f64 / (size - 1) as f64 * 2.0 - 1.0;
                    let d = (xd * xd + yd * yd + zd * zd).sqrt();
                    xd /= d;
                    yd /= d;
                    zd /= d;
                    let mut remaining = radius * (0.7 + self.random.next_float() * 0.6);
                    let (mut xp, mut yp, mut zp) = (x, y, z);
                    let stepsize = 0.3;
                    while remaining > 0.0 {
                        let (xt, yt, zt) = (xp.floor() as i32, yp.floor() as i32, zp.floor() as i32);
                        let id = self.level.get_tile(xt, yt, zt);
                        if id > 0 {
                            let res = tile_info(id).map_or(0.0, |i| i.explosion_resistance) / 5.0;
                            remaining -= (res + 0.3) * stepsize;
                        }
                        if remaining > 0.0 && id != 0 {
                            destroyed.insert((xt, yt, zt));
                        }
                        xp += xd * stepsize as f64;
                        yp += yd * stepsize as f64;
                        zp += zd * stepsize as f64;
                        remaining -= stepsize * 0.75;
                    }
                }
            }
        }
        // entities
        let r2 = radius as f64 * 2.0;
        let bb = mc_core::aabb::Aabb::new(x - r2 - 1.0, y - r2 - 1.0, z - r2 - 1.0, x + r2 + 1.0, y + r2 + 1.0, z + r2 + 1.0);
        let seen = |lv: &mc_level::level::Level, b: &mc_core::aabb::Aabb| -> f32 {
            let (xs, ys, zs) = (1.0 / ((b.x1 - b.x0) * 2.0 + 1.0), 1.0 / ((b.y1 - b.y0) * 2.0 + 1.0), 1.0 / ((b.z1 - b.z0) * 2.0 + 1.0));
            let (mut hit, mut count) = (0, 0);
            let mut xx = 0.0;
            while xx <= 1.0 {
                let mut yy = 0.0;
                while yy <= 1.0 {
                    let mut zz = 0.0;
                    while zz <= 1.0 {
                        let p = [b.x0 + (b.x1 - b.x0) * xx, b.y0 + (b.y1 - b.y0) * yy, b.z0 + (b.z1 - b.z0) * zz];
                        if crate::mob::line_of_sight(lv, [x, y, z], p) {
                            hit += 1;
                        }
                        count += 1;
                        zz += zs;
                    }
                    yy += ys;
                }
                xx += xs;
            }
            hit as f32 / count as f32
        };
        let mut hurts: Vec<(usize, f32, f64, f64)> = Vec::new();
        for (i, m) in self.mobs.list.iter().enumerate() {
            if !m.b.bb.intersects(&bb) {
                continue;
            }
            let (ex, ey, ez) = (m.b.x, m.b.bb.y0 + m.b.h * 0.0 + m.b.h * 0.5 - 0.0, m.b.z);
            let dist = (((ex - x).powi(2) + (m.b.y - y).powi(2) + (ez - z).powi(2)).sqrt()) / r2;
            let _ = ey;
            if dist > 1.0 {
                continue;
            }
            let mut xa = ex - x;
            let mut ya = m.b.y + m.b.h * 0.0 + m.head_height() - y;
            let mut za = ez - z;
            let l = (xa * xa + ya * ya + za * za).sqrt();
            if l == 0.0 {
                continue;
            }
            xa /= l;
            ya /= l;
            za /= l;
            let sp = seen(&self.level, &m.b.bb);
            let pow = (1.0 - dist) * sp as f64;
            let d = ((pow * pow + pow) / 2.0 * 8.0 * r2 + 1.0) as f32;
            hurts.push((i, d, xa * pow, za * pow));
        }
        for (i, d, kx, kz) in hurts {
            self.hurt_mob(i, Src::Explosion, d);
            if let Some(m) = self.mobs.list.get_mut(i) {
                m.b.xd += kx;
                m.b.zd += kz;
            }
        }
        // the player
        if !self.creative {
            let p = &self.player;
            let (ex, ez) = (p.x, p.z);
            let dist = ((ex - x).powi(2) + (p.y - 1.62 - y).powi(2) + (ez - z).powi(2)).sqrt() / r2;
            if dist <= 1.0 {
                let (mut xa, mut ya, mut za) = (ex - x, p.y - y, ez - z);
                let l = (xa * xa + ya * ya + za * za).sqrt();
                if l > 0.0 {
                    xa /= l;
                    ya /= l;
                    za /= l;
                    let pb = p.bb;
                    let sp = seen(&self.level, &pb);
                    let pow = (1.0 - dist) * sp as f64;
                    let d = ((pow * pow + pow) / 2.0 * 8.0 * r2 + 1.0) as f32;
                    // explosion damage bypasses the difficulty scaling of mob hits only in peaceful
                    if self.difficulty > 0 {
                        self.invuln = 0;
                        let dd = if self.difficulty == 1 { (d / 2.0 + 1.0).min(d) } else if self.difficulty == 3 { d * 3.0 / 2.0 } else { d };
                        self.damage_player(dd);
                        self.hurt_time = 10;
                        self.invuln = 20;
                        self.last_hurt = dd;
                        if self.health <= 0 {
                            self.kill_player();
                        }
                    }
                    self.player.xd += xa * pow;
                    self.player.yd += ya * pow;
                    self.player.zd += za * pow;
                }
            }
        }
        // blocks
        let mut list: Vec<_> = destroyed.into_iter().collect();
        list.sort();
        let rr = radius;
        for (bx, by, bz) in list {
            let id = self.level.get_tile(bx, by, bz);
            if id == 0 || id == t::UNBREAKABLE {
                continue;
            }
            let data = self.level.get_data(bx, by, bz);
            if id == t::TNT {
                self.level.set_tile(bx, by, bz, 0, UPDATE_ALL);
                self.mark(bx, bz);
                self.tnt_chain((bx, by, bz));
                continue;
            }
            if self.random.next_float() < 1.0 / rr {
                for d in mc_world::drops::block_drops(id, data, &mut self.random, false, 0) {
                    let e = ItemEntity::pop(bx, by, bz, d, &mut self.random);
                    self.entities.items.push(e);
                }
            }
            if self.random.next_float() < 0.1 {
                self.particles.destroy(&self.level, bx, by, bz, id, data);
            }
            self.level.set_tile(bx, by, bz, 0, UPDATE_ALL);
            self.mark(bx, bz);
        }
        let _ = Random::new;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{generate, Dim};

    fn game() -> Game {
        let lv = generate(Dim::Flat, 1, 4);
        let g = (0..128).rev().find(|&y| lv.get_tile(0, y, 0) != 0).unwrap() as f32 + 1.0;
        let mut game = Game::new(lv, Dim::Flat, [0.5, g, 0.5]);
        game.creative = false;
        game.spawn_mobs = true;
        game.level.game_time = 1;
        game
    }

    #[test]
    fn mob_hits_hurt_the_player_with_invulnerability_frames() {
        let mut g = game();
        g.hurt_player(3.0, 5.0, 0.5);
        assert_eq!(g.health, 17);
        assert!(g.player.xd < 0.0, "knocked away from the source");
        g.hurt_player(3.0, 5.0, 0.5); // inside the 10 tick window: ignored
        assert_eq!(g.health, 17);
        g.hurt_player(5.0, 5.0, 0.5); // only the difference
        assert_eq!(g.health, 15);
    }

    #[test]
    fn peaceful_and_creative_take_no_damage() {
        let mut g = game();
        g.difficulty = 0;
        g.hurt_player(5.0, 0.0, 0.0);
        assert_eq!(g.health, 20);
        let mut g = game();
        g.creative = true;
        g.hurt_player(5.0, 0.0, 0.0);
        assert_eq!(g.health, 20);
    }

    #[test]
    fn explosion_digs_a_crater_and_hurts() {
        let mut g = game();
        let y = g.player.y as i32 - 2;
        assert_ne!(g.level.get_tile(10, y - 1, 0), 0);
        g.explode(10.5, y as f64, 0.5, 3.0);
        assert_eq!(g.level.get_tile(10, y - 1, 0), 0);
    }

    #[test]
    fn hitting_and_killing_a_cow_drops_leather_or_beef() {
        let mut g = game();
        let gy = g.player.y - 1.62;
        g.mobs.spawn(MobKind::Cow, 0.5, gy, 3.5);
        g.player.pitch = 12.0;
        g.player.yaw = 0.0;
        g.update_target(1.0);
        assert!(g.mob_target.is_some(), "cow in front of the crosshair");
        g.inv.items[0] = Some(ItemStack::new(276, 1));
        g.inv.selected = 0;
        assert!(g.try_attack_mob());
        assert!(g.mobs.list[0].health < 10.0, "diamond sword hits for 7");
        g.hurt_mob(0, Src::Player { x: 0.5, z: 0.5 }, 100.0);
        for _ in 0..30 {
            g.tick_mobs();
        }
        assert!(g.mobs.list.iter().all(|m| m.kind != MobKind::Cow));
        assert!(!g.entities.items.is_empty(), "cows drop beef");
    }

    #[test]
    fn feeding_wheat_puts_a_cow_in_love_and_consumes_it() {
        let mut g = game();
        let gy = g.player.y - 1.62;
        g.mobs.spawn(MobKind::Cow, 0.5, gy, 3.0);
        g.inv.items[0] = Some(ItemStack::new(crate::mob::ids::WHEAT, 2));
        g.inv.selected = 0;
        g.player.pitch = 12.0;
        g.update_target(1.0);
        assert!(g.try_interact_mob());
        assert_eq!(g.mobs.list[0].love, 600);
        assert_eq!(g.inv.items[0].unwrap().count, 1);
    }
}
