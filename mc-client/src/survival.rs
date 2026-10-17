//! Survival mechanics: `FoodData` (hunger, exhaustion, saturation, natural regeneration, starvation), the few mob
//! effects food can cause (`MobEffect` hunger / poison / regeneration), drowning (`LivingEntity::baseTick`) and
//! experience (`Player::increaseXp`, `ExperienceOrb`).
//!
//! `FoodData.cpp` / `FoodConstants.cpp` are not part of the source drop; the constants below are the 1.6 values the
//! rest of the code (`FoodConstants.h`, `Player.cpp`, `Gui.cpp`) was written against.

use crate::entity::Body;
use crate::game::Game;
use mc_level::level::Level;

/// `FoodConstants`
pub mod fc {
    pub const MAX_FOOD: i32 = 20;
    pub const EXHAUSTION_DROP: f32 = 4.0;
    pub const HEALTH_TICK_COUNT: i32 = 80;
    pub const HEAL_LEVEL: i32 = 18;
    pub const STARVE_LEVEL: i32 = 0;
    pub const FOOD_SATURATION_LOW: f32 = 0.3;
    pub const EXHAUSTION_HEAL: f32 = 3.0;
    pub const EXHAUSTION_JUMP: f32 = 0.2;
    pub const EXHAUSTION_SPRINT_JUMP: f32 = 0.8;
    pub const EXHAUSTION_MINE: f32 = 0.025;
    pub const EXHAUSTION_ATTACK: f32 = 0.3;
    pub const EXHAUSTION_WALK: f32 = 0.01;
    pub const EXHAUSTION_SPRINT: f32 = 0.1;
    pub const EXHAUSTION_SWIM: f32 = 0.015;
}

pub const TOTAL_AIR_SUPPLY: i32 = 300;

/// `MobEffect` ids used here
pub const REGENERATION: u8 = 10;
pub const HUNGER: u8 = 17;
pub const POISON: u8 = 19;

/// `MobEffectInstance`
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effect {
    pub id: u8,
    pub duration: i32,
    pub amplifier: i32,
}

/// `FoodItem::setEatEffect`: (effect, seconds, amplifier, probability) for the foods that have one.
pub fn eat_effect(item: u16) -> Option<(u8, i32, i32, f32)> {
    match item {
        322 => Some((REGENERATION, 5, 1, 1.0)),
        365 => Some((HUNGER, 30, 0, 0.3)),
        367 => Some((HUNGER, 30, 0, 0.8)),
        375 => Some((POISON, 5, 0, 1.0)),
        394 => Some((POISON, 5, 0, 0.6)),
        _ => None,
    }
}

/// `FoodItem::setCanAlwaysEat`: the golden apple.
pub fn can_always_eat(item: u16) -> bool {
    item == 322
}

/// An experience orb (`ExperienceOrb`).
pub struct XpOrb {
    pub body: Body,
    pub value: i32,
    pub age: i32,
    pub throw_time: i32,
    pub tick_count: i32,
    pub removed: bool,
}

/// `ExperienceOrb::getExperienceValue`: the orb size an amount is split into.
pub fn orb_value(max: i32) -> i32 {
    for v in [2477, 1237, 617, 307, 149, 73, 37, 17, 7, 3] {
        if max >= v {
            return v;
        }
    }
    1
}

/// `Player::getXpNeededForNextLevel`
pub fn xp_needed(level: i32) -> i32 {
    if level >= 30 {
        17 + 15 * 3 + (level - 30) * 7
    } else if level >= 15 {
        17 + (level - 15) * 3
    } else {
        17
    }
}

impl XpOrb {
    pub fn new(x: f64, y: f64, z: f64, value: i32, r: &mut mc_core::random::Random) -> XpOrb {
        let mut body = Body::new(0.5, 0.5, 0.25);
        body.set_pos(x, y, z);
        body.yaw = (r.next_double() * 360.0) as f32;
        body.xd = (r.next_double() * 0.2 - 0.1) * 2.0;
        body.yd = r.next_double() * 0.2 * 2.0;
        body.zd = (r.next_double() * 0.2 - 0.1) * 2.0;
        XpOrb { body, value, age: 0, throw_time: 0, tick_count: 0, removed: false }
    }

    /// `ExperienceOrb::tick` towards the player at (px, py, pz) (py = eye height).
    pub fn tick(&mut self, lv: &Level, target: Option<[f64; 3]>) {
        let b = &mut self.body;
        b.xo = b.x;
        b.yo = b.y;
        b.zo = b.z;
        if self.throw_time > 0 {
            self.throw_time -= 1;
        }
        b.yd -= 0.03;
        b.update_water(lv);
        if b.in_lava {
            b.yd = 0.2;
        }
        let max_dist = 8.0;
        if let Some(t) = target {
            let (xdd, ydd, zdd) = ((t[0] - b.x) / max_dist, (t[1] - b.y) / max_dist, (t[2] - b.z) / max_dist);
            let dd = (xdd * xdd + ydd * ydd + zdd * zdd).sqrt();
            let mut power = 1.0 - dd;
            if power > 0.0 && dd > 0.0 {
                power *= power;
                b.xd += xdd / dd * power * 0.1;
                b.yd += ydd / dd * power * 0.1;
                b.zd += zdd / dd * power * 0.1;
            }
        }
        let (xd, yd, zd) = (b.xd, b.yd, b.zd);
        b.move_by(lv, xd, yd, zd);
        let mut friction = 0.98;
        if b.on_ground {
            friction = crate::physics::friction_of(lv.get_tile(b.x.floor() as i32, b.bb.y0.floor() as i32 - 1, b.z.floor() as i32)) as f64 * 0.98;
        }
        b.xd *= friction;
        b.yd *= 0.98;
        b.zd *= friction;
        if b.on_ground {
            b.yd *= -0.9;
        }
        self.tick_count += 1;
        self.age += 1;
        if self.age >= 6000 {
            self.removed = true;
        }
    }
}

impl Game {
    // ----------------------------------------------------------------------------------- health

    /// `LivingEntity::heal`
    pub fn heal(&mut self, n: i32) {
        if self.health > 0 && self.health < 20 {
            self.health = (self.health + n).min(20);
        }
    }

    /// Damage that skips armour and the difficulty scaling (starving, drowning, poison, falling): `source->bypassArmor()`.
    /// These sources cost no exhaustion.
    pub fn hurt_bypass(&mut self, d: i32) {
        if self.creative || self.health <= 0 || d <= 0 {
            return;
        }
        self.last_health = self.health;
        self.health -= d;
        self.hurt_time = 10;
        self.hurt_dir = 0.0;
        self.invuln = 20;
        let (x, y, z) = (self.player.x, self.player.y, self.player.z);
        let pitch = (self.random.next_float() - self.random.next_float()) * 0.2 + 1.0;
        self.play("damage.hit", x, y, z, 1.0, pitch);
        if self.health <= 0 {
            self.kill_player();
        }
    }

    // ----------------------------------------------------------------------------------- food

    /// `Player::causeFoodExhaustion` (creative players never tire).
    pub fn add_exhaustion(&mut self, amount: f32) {
        if self.creative || self.player.flying {
            return;
        }
        self.exhaustion = (self.exhaustion + amount).min(40.0);
    }

    /// `Player::canEat`
    pub fn can_eat(&self, always: bool) -> bool {
        !self.creative && (always || self.food < fc::MAX_FOOD)
    }

    /// `FoodData::eat(nutrition, saturationModifier)`
    pub fn eat_food(&mut self, nutrition: i32, modifier: f32) {
        self.food = (self.food + nutrition).min(fc::MAX_FOOD);
        self.saturation = (self.saturation + nutrition as f32 * modifier * 2.0).min(self.food as f32);
    }

    /// `FoodData::tick` (the server tick of a survival player).
    pub fn tick_food(&mut self) {
        if self.creative || self.health <= 0 {
            return;
        }
        let difficulty = self.difficulty;
        self.last_food = self.food;
        if self.exhaustion > fc::EXHAUSTION_DROP {
            self.exhaustion -= fc::EXHAUSTION_DROP;
            if self.saturation > 0.0 {
                self.saturation = (self.saturation - 1.0).max(0.0);
            } else if difficulty > 0 {
                self.food = (self.food - 1).max(0);
            }
        }
        if self.food >= fc::HEAL_LEVEL && self.health < 20 {
            self.food_timer += 1;
            if self.food_timer >= fc::HEALTH_TICK_COUNT {
                self.heal(1);
                self.add_exhaustion(fc::EXHAUSTION_HEAL);
                self.food_timer = 0;
            }
        } else if self.food <= fc::STARVE_LEVEL {
            self.food_timer += 1;
            if self.food_timer >= fc::HEALTH_TICK_COUNT {
                if self.health > 10 || difficulty >= 3 || (self.health > 1 && difficulty >= 2) {
                    self.hurt_bypass(1);
                }
                self.food_timer = 0;
            }
        } else {
            self.food_timer = 0;
        }
        // `Player::aiStep`: peaceful players heal on their own
        if difficulty == 0 && self.health < 20 && self.ticks % (20 * 12) == 0 {
            self.heal(1);
        }
    }

    /// `Player::checkMovementStatistics`: walking, sprinting and swimming cost exhaustion.
    pub fn move_exhaustion(&mut self) {
        let (dx, dy, dz) = (self.player.x - self.prev[0], self.player.y - self.prev[1], self.player.z - self.prev[2]);
        if self.head_under_water() {
            let d = ((dx * dx + dy * dy + dz * dz).sqrt() * 100.0).round();
            if d > 0.0 {
                self.add_exhaustion(fc::EXHAUSTION_SWIM * d as f32 * 0.01);
            }
        } else if self.player.in_water {
            let d = ((dx * dx + dz * dz).sqrt() * 100.0).round();
            if d > 0.0 {
                self.add_exhaustion(fc::EXHAUSTION_SWIM * d as f32 * 0.01);
            }
        } else if self.player.on_ground {
            let d = ((dx * dx + dz * dz).sqrt() * 100.0).round();
            if d > 0.0 {
                let per = if self.player.sprinting { fc::EXHAUSTION_SPRINT } else { fc::EXHAUSTION_WALK };
                self.add_exhaustion(per * d as f32 * 0.01);
            }
        }
    }

    // ----------------------------------------------------------------------------------- effects

    pub fn has_effect(&self, id: u8) -> bool {
        self.effects.iter().any(|e| e.id == id)
    }

    /// `LivingEntity::addEffect` (`MobEffectInstance::update` keeps the stronger / longer one).
    pub fn add_effect(&mut self, e: Effect) {
        if let Some(cur) = self.effects.iter_mut().find(|c| c.id == e.id) {
            if e.amplifier > cur.amplifier {
                cur.amplifier = e.amplifier;
                cur.duration = e.duration;
            } else if e.amplifier == cur.amplifier && cur.duration < e.duration {
                cur.duration = e.duration;
            }
        } else {
            self.effects.push(e);
        }
    }

    /// `LivingEntity::tickEffects`
    pub fn tick_effects(&mut self) {
        let mut hurt = 0;
        let mut heal = 0;
        let mut exhaust = 0.0f32;
        for e in self.effects.iter_mut() {
            if e.duration > 0 {
                let interval = |base: i32| base >> e.amplifier.clamp(0, 30);
                match e.id {
                    REGENERATION => {
                        let n = interval(50);
                        if n <= 0 || e.duration % n == 0 {
                            heal += 1;
                        }
                    }
                    POISON => {
                        let n = interval(25);
                        if n <= 0 || e.duration % n == 0 {
                            hurt += 1;
                        }
                    }
                    HUNGER => exhaust += 0.025 * (e.amplifier + 1) as f32,
                    _ => {}
                }
                e.duration -= 1;
            }
        }
        self.effects.retain(|e| e.duration > 0);
        if heal > 0 {
            self.heal(heal);
        }
        if hurt > 0 && self.health > 1 {
            self.hurt_bypass(1);
        }
        if exhaust > 0.0 {
            self.add_exhaustion(exhaust);
        }
    }

    // ----------------------------------------------------------------------------------- air

    /// `LivingEntity::baseTick`: the air runs out under water, then 2 damage a second.
    pub fn tick_air(&mut self) {
        if self.health <= 0 {
            return;
        }
        if self.head_under_water() {
            if self.creative {
                return;
            }
            self.air -= 1;
            if self.air == -20 {
                self.air = 0;
                let (x, y, z) = (self.player.x, self.player.y, self.player.z);
                for _ in 0..8 {
                    let (xo, yo, zo) = (
                        (self.random.next_float() - self.random.next_float()) as f64,
                        (self.random.next_float() - self.random.next_float()) as f64,
                        (self.random.next_float() - self.random.next_float()) as f64,
                    );
                    let (bx, by, bz) = ((x + xo).floor() as i32, (y + yo).floor() as i32, (z + zo).floor() as i32);
                    let id = self.level.get_tile(bx, by, bz);
                    if matches!(id, mc_world::tile_ids::WATER | mc_world::tile_ids::CALM_WATER) {
                        self.particles.tile_crack(&self.level, bx, by, bz, id, 0, x + xo, y + yo, z + zo, 0.0, 0.1, 0.0);
                    }
                }
                self.hurt_bypass(2);
            }
        } else {
            self.air = TOTAL_AIR_SUPPLY;
        }
    }

    /// The air shown by the HUD: bubbles appear once the head is under water.
    pub fn air_hud(&self) -> Option<i32> {
        (self.head_under_water() && !self.creative).then_some(self.air)
    }

    // ----------------------------------------------------------------------------------- experience

    /// `Player::increaseXp`
    pub fn give_xp(&mut self, amount: i32) {
        if amount <= 0 {
            return;
        }
        self.score += amount;
        self.xp_total += amount;
        self.xp_progress += amount as f32 / xp_needed(self.xp_level) as f32;
        while self.xp_progress >= 1.0 {
            self.xp_progress = (self.xp_progress - 1.0) * xp_needed(self.xp_level) as f32;
            self.xp_level += 1;
            if self.xp_level % 5 == 0 && self.ticks - self.last_level_up > 100 {
                let vol = if self.xp_level > 30 { 1.0 } else { self.xp_level as f32 / 30.0 };
                let (x, y, z) = (self.player.x, self.player.y, self.player.z);
                self.play("random.levelup", x, y, z, vol * 0.75, 1.0);
                self.last_level_up = self.ticks;
            }
            self.xp_progress /= xp_needed(self.xp_level) as f32;
        }
    }

    /// `ExperienceOrb::award`: split `amount` into orbs at a point.
    pub fn spawn_xp(&mut self, x: f64, y: f64, z: f64, mut amount: i32) {
        while amount > 0 {
            let v = orb_value(amount);
            amount -= v;
            let o = XpOrb::new(x, y, z, v, &mut self.random);
            self.orbs.push(o);
        }
    }

    pub fn tick_orbs(&mut self) {
        if self.take_xp_delay > 0 {
            self.take_xp_delay -= 1;
        }
        let eye = [self.player.x, self.player.y, self.player.z];
        let alive = self.health > 0;
        let mut picked = 0;
        let pb = self.player.bb;
        let mut orb_sounds = Vec::new();
        for o in self.orbs.iter_mut() {
            let d2 = (o.body.x - eye[0]).powi(2) + (o.body.y - eye[1]).powi(2) + (o.body.z - eye[2]).powi(2);
            let target = (alive && d2 < 64.0).then_some(eye);
            o.tick(&self.level, target);
            if alive && !o.removed && o.throw_time == 0 && self.take_xp_delay == 0 && pb.grow(1.0, 0.0, 1.0).intersects(&o.body.bb) {
                self.take_xp_delay = 2;
                picked += o.value;
                o.removed = true;
                let pitch = 0.5 * ((self.random.next_float() - self.random.next_float()) * 0.7 + 1.8);
                orb_sounds.push((eye, pitch));
            }
        }
        self.orbs.retain(|o| !o.removed);
        for (e, pitch) in orb_sounds {
            self.play("random.orb", e[0], e[1], e[2], 0.1, pitch);
        }
        if picked > 0 {
            self.give_xp(picked);
        }
    }

    /// Experience a mined block drops (`OreTile::spawnResources`, spawner, quartz): 0 for everything else.
    pub fn block_xp(&mut self, id: u8) -> i32 {
        use mc_world::tile_ids as t;
        let mut r = |a: i32, b: i32| a + self.random.next_int_bounded(b - a + 1);
        match id {
            t::COAL_ORE => r(0, 2),
            t::DIAMOND_ORE | t::EMERALD_ORE => r(3, 7),
            t::LAPIS_ORE => r(2, 5),
            t::RED_STONE_ORE | t::RED_STONE_ORE_LIT => r(1, 5),
            t::NETHER_QUARTZ => r(2, 5),
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::Input;
    use crate::world::generate;
    use mc_level::level::UPDATE_ALL;
    use mc_world::item::{ids, ItemStack};
    use crate::world::Dim;

    fn survival() -> Game {
        let lv = generate(Dim::Flat, 1, 3);
        let mut g = Game::new(lv, Dim::Flat, [0.5, 4.0, 0.5]);
        for _ in 0..5 {
            g.tick(&Input::default());
        }
        g.creative = false;
        g.inv = crate::inventory::Inventory::new();
        g
    }

    fn run(g: &mut Game, n: i32) {
        for _ in 0..n {
            g.tick(&Input::default());
        }
    }

    #[test]
    fn exhaustion_eats_saturation_first_then_food() {
        let mut g = survival();
        g.saturation = 1.0;
        g.add_exhaustion(4.5);
        run(&mut g, 1);
        assert_eq!((g.food, g.saturation), (20, 0.0));
        g.add_exhaustion(4.5);
        run(&mut g, 1);
        assert_eq!(g.food, 19);
        // creative players never tire
        let mut c = survival();
        c.creative = true;
        c.add_exhaustion(10.0);
        assert_eq!(c.exhaustion, 0.0);
    }

    #[test]
    fn a_full_stomach_heals_and_an_empty_one_starves() {
        let mut g = survival();
        g.health = 10;
        run(&mut g, 80);
        assert_eq!(g.health, 11, "one heart point every 80 ticks");
        assert!(g.exhaustion >= 2.9, "healing costs exhaustion {}", g.exhaustion);
        let mut s = survival();
        s.food = 0;
        s.saturation = 0.0;
        run(&mut s, 80);
        assert_eq!(s.health, 19);
        // on normal difficulty starvation stops at one heart
        s.health = 1;
        run(&mut s, 160);
        assert_eq!(s.health, 1);
        // on hard it kills
        s.difficulty = 3;
        run(&mut s, 80);
        assert!(s.dead, "starved to death");
    }

    #[test]
    fn walking_and_jumping_cost_exhaustion_but_flying_does_not() {
        let mut g = survival();
        let walk = Input { forward: 1.0, ..Default::default() };
        for _ in 0..40 {
            g.tick(&walk);
        }
        let walked = g.exhaustion + (20 - g.food) as f32 * 4.0 + (5.0 - g.saturation).max(0.0) * 4.0;
        assert!(walked > 0.05, "walking costs something: {walked}");
        let mut f = survival();
        f.creative = true;
        for _ in 0..40 {
            f.tick(&walk);
        }
        assert_eq!(f.exhaustion, 0.0);
    }

    #[test]
    fn no_sprinting_on_an_empty_stomach() {
        let mut g = survival();
        g.food = 6;
        let run_in = Input { forward: 1.0, sprint: true, ..Default::default() };
        g.tick(&run_in);
        assert!(!g.player.sprinting);
        g.food = 7;
        g.tick(&run_in);
        assert!(g.player.sprinting);
    }

    #[test]
    fn eating_follows_the_food_item_rules() {
        let mut g = survival();
        g.inv.items[0] = Some(ItemStack::new(ids::BREAD, 2));
        assert!(!g.can_eat(false), "full: bread is refused");
        g.food = 10;
        assert!(g.can_eat(false));
        // the golden apple can always be eaten and grants regeneration
        assert!(can_always_eat(322));
        assert_eq!(eat_effect(322), Some((REGENERATION, 5, 1, 1.0)));
        g.food = 20;
        g.health = 10;
        g.add_effect(Effect { id: REGENERATION, duration: 100, amplifier: 1 });
        run(&mut g, 100);
        assert!(g.health >= 14, "regeneration II heals every 25 ticks: {}", g.health);
        // poison hurts but never kills
        g.health = 3;
        g.food = 10; // below the natural-regeneration level
        g.add_effect(Effect { id: POISON, duration: 400, amplifier: 0 });
        run(&mut g, 400);
        assert_eq!(g.health, 1);
        assert!(!g.dead);
    }

    #[test]
    fn hunger_effect_drains_food() {
        let mut g = survival();
        g.saturation = 0.0;
        g.add_effect(Effect { id: HUNGER, duration: 400, amplifier: 0 });
        run(&mut g, 400);
        assert!(g.food < 20, "food {}", g.food);
    }

    #[test]
    fn the_air_runs_out_under_water_and_drowning_hurts() {
        let mut g = survival();
        let (x, z) = (g.player.x.floor() as i32, g.player.z.floor() as i32);
        for dy in 0..6 {
            for dx in -1..=1 {
                for dz in -1..=1 {
                    g.level.set_tile(x + dx, 3 + dy, z + dz, mc_world::tile_ids::CALM_WATER, UPDATE_ALL);
                }
            }
        }
        g.player.set_pos(g.player.x, 6.0, g.player.z);
        run(&mut g, 100);
        assert!(g.air < TOTAL_AIR_SUPPLY && g.air_hud().is_some(), "air {}", g.air);
        run(&mut g, 300);
        assert!(g.health <= 18, "drowning hurts: {}", g.health);
        // out of the water the air refills
        g.player.set_pos(g.player.x, 30.0, g.player.z);
        g.tick(&Input::default());
        assert_eq!(g.air, TOTAL_AIR_SUPPLY);
    }

    #[test]
    fn experience_levels_up_and_orbs_are_picked_up() {
        let mut g = survival();
        g.give_xp(17);
        assert_eq!((g.xp_level, g.xp_total), (1, 17));
        assert!(g.xp_progress.abs() < 1e-5);
        g.give_xp(100);
        assert!(g.xp_level >= 4);
        assert_eq!(xp_needed(0), 17);
        assert_eq!(xp_needed(15), 17);
        assert_eq!(xp_needed(16), 20);
        assert_eq!(xp_needed(30), 62);
        assert_eq!(orb_value(5), 3);
        assert_eq!(orb_value(2477), 2477);
        // an orb dropped at the feet is attracted and taken
        let before = g.xp_total;
        let (x, y, z) = (g.player.x, g.player.y, g.player.z);
        g.spawn_xp(x + 3.0, y, z, 5);
        assert!(!g.orbs.is_empty());
        run(&mut g, 80);
        assert!(g.orbs.is_empty(), "orbs were collected");
        assert_eq!(g.xp_total, before + 5);
    }

    #[test]
    fn dying_opens_the_death_screen_and_respawning_resets_the_body() {
        let mut g = survival();
        g.inv.items[0] = Some(ItemStack::new(ids::BREAD, 2));
        g.give_xp(40);
        g.hurt_bypass(40);
        assert!(g.dead);
        assert_eq!(g.open_screen, Some(crate::screens::ScreenId::Death));
        assert!(g.inv.items[0].is_none(), "the inventory spilled");
        // nothing moves while dead
        let y = g.player.y;
        run(&mut g, 10);
        assert_eq!(g.player.y, y);
        g.respawn();
        assert!(!g.dead);
        assert_eq!((g.health, g.food, g.xp_level), (20, 20, 0));
    }
}
