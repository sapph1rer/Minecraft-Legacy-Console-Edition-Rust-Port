//! Interactive state around a `Level`: the player, the hotbar, block targeting, breaking and placing.
//! Pure logic (no GPU), so it is testable headless.

use crate::audio::SoundEvent;
use crate::entity::{Entities, ItemEntity};
use crate::inventory::Inventory;
use crate::mesh::{MeshData, Vertex};
use crate::particles::ParticleEngine;
use crate::physics::{block_intersects_player, raycast, BlockHit, Input, Player};
use crate::soundtypes::sound_type;
use crate::world::Dim;
use mc_core::random::Random;
use mc_level::level::{Level, UPDATE_ALL};
use mc_world::tile_ids as t;
use mc_world::tile::tile_info;
use mc_world::item::{can_harvest, item_destroy_speed, ItemKind, ItemStack};
use mc_world::drops::block_drops;
use mc_world::Material;

pub const REACH: f64 = 5.0;

pub struct Game {
    pub level: Level,
    pub dim: Dim,
    pub player: Player,
    pub(crate) prev: [f64; 3],
    pub inv: Inventory,
    pub entities: Entities,
    pub health: i32,
    pub last_health: i32,
    pub food: i32,
    pub saturation: f32,
    /// `FoodData`: exhaustion, the heal / starve timer and last tick's food level
    pub exhaustion: f32,
    pub food_timer: i32,
    pub last_food: i32,
    /// active effects (`MobEffectInstance`), air left (`Entity::getAirSupply`)
    pub effects: Vec<crate::survival::Effect>,
    pub air: i32,
    /// experience: `Player::experienceLevel`, `experienceProgress`, `totalExperience`, `score`
    pub xp_level: i32,
    pub xp_progress: f32,
    pub xp_total: i32,
    pub score: i32,
    pub last_level_up: i64,
    pub take_xp_delay: i32,
    pub orbs: Vec<crate::survival::XpOrb>,
    pub pearls: Vec<crate::pearl::Pearl>,
    /// dead: the death screen is up and nothing ticks until the player respawns
    pub dead: bool,
    /// ticks of eating left (`useItemDuration`), 0 = not eating
    pub eat_left: i32,
    /// a block interaction asked for a screen (crafting table, ...)
    pub weather: crate::weather::Weather,
    pub open_screen: Option<crate::screens::ScreenId>,
    /// the chest the open chest screen belongs to
    pub chest_pos: Option<(i32, i32, i32)>,
    /// every chest of the open container (one, or two for a large chest)
    pub chest_group: Vec<(i32, i32, i32)>,
    /// `Entity::fallDistance`
    pub fall_distance: f32,
    /// the sign being edited
    pub sign_pos: Option<(i32, i32, i32)>,
    pub chest_anim: std::collections::HashMap<(i32, i32, i32), crate::chest::ChestAnim>,
    /// pressed buttons and ticks until they pop back
    pub pending_unpress: Vec<((i32, i32, i32), i32)>,
    /// cells whose sand / gravel / anvil should check for support in N ticks
    pub fall_checks: Vec<((i32, i32, i32), i32)>,
    pub redstone: crate::redstone::Redstone,
    /// pressure plates currently held down and the ticks until they release
    pub plate_timers: std::collections::HashMap<(i32, i32, i32), i32>,
    /// blocks being pushed by pistons, and the piston tile events waiting for the next tick
    pub moving: Vec<crate::piston::MovingPiece>,
    pub piston_queue: Vec<((i32, i32, i32), u8, u8)>,
    /// primed TNT is in `entities.primed`
    /// `Gui::toolHighlightTimer`: ticks left to show the held item's name, and what it was last tick
    pub hl_timer: i32,
    hl_key: Option<(u16, u16)>,
    pub sneaking: bool,
    pub target: Option<BlockHit>,
    pub(crate) dirty: Vec<(i32, i32)>,
    /// `LevelData::dayTime`
    pub day_time: i64,
    /// `Level::getSeed` (used by mob spawning)
    pub world_seed: i64,
    /// client ticks (cloud drift)
    pub ticks: i64,
    pub daylight_cycle: bool,
    /// `GameType::isCreative`: blocks break at once (every 5 ticks while held) and cost nothing
    pub creative: bool,
    pub mining: Mining,
    /// block currently being cracked: (position, stage 0..=9), `LevelRenderer::destroyingBlocks`
    pub destroy_stage: Option<((i32, i32, i32), usize)>,
    pub particles: ParticleEngine,
    /// sounds produced since the last call of `take_sounds`
    pub sounds: Vec<SoundEvent>,
    /// arm swing animation ticks left (`Player::swing`)
    pub swing_time: i32,
    pub swinging: bool,
    /// first-person hand state (`ItemInHandRenderer`)
    pub hand: crate::hand::HandState,
    last_use_tick: i64,
    move_dist: f64,
    next_step: i32,
    pub(crate) random: Random,
    /// the mob world (`spawn_mobs` switches ticking and natural spawning on)
    pub mobs: crate::mobs::Mobs,
    pub spawn_mobs: bool,
    /// 0 peaceful .. 3 hard
    pub difficulty: i32,
    pub spawn_pos: [f64; 3],
    was_in_water: bool,
    /// `invulnerableTime` / `lastHurt` of the player
    pub invuln: i32,
    pub last_hurt: f32,
    pub hurt_time: i32,
    /// `LivingEntity::hurtDir`: degrees relative to the view, set by the attacker's direction (0 for falls and the like)
    pub hurt_dir: f32,
    /// mob under the crosshair: (index, distance)
    pub mob_target: Option<(usize, f64)>,
}

/// The hotbar a fresh creative game starts with.
pub fn creative_inventory() -> Inventory {
    let mut inv = Inventory::new();
    for (i, id) in [t::STONE, t::COBBLESTONE, t::DIRT, t::WOOD, t::TREE_TRUNK, t::GLASS, t::SAND, t::RED_BRICK, t::GLOWSTONE].into_iter().enumerate() {
        inv.items[i] = Some(ItemStack::block(id, 64));
    }
    inv
}

/// Held buttons for one tick (`Minecraft::tick` input handling).
#[derive(Clone, Copy, Debug, Default)]
pub struct Actions {
    /// left button went down this tick
    pub attack_pressed: bool,
    pub attack_held: bool,
    pub use_held: bool,
}

/// `MultiPlayerGameMode` destroy state + `LocalPlayer::missTime`.
#[derive(Clone, Copy, Debug)]
pub struct Mining {
    pub pos: (i32, i32, i32),
    pub is_destroying: bool,
    pub progress: f32,
    pub ticks: i32,
    pub delay: i32,
    pub miss_time: i32,
}

impl Default for Mining {
    fn default() -> Self {
        Mining { pos: (-1, -1, -1), is_destroying: false, progress: 0.0, ticks: 0, delay: 0, miss_time: 0 }
    }
}

impl Game {
    pub fn new(level: Level, dim: Dim, spawn: [f32; 3]) -> Game {
        let mut level = level;
        level.lighting = true;
        level.log_changes = true;
        level.insta_tick = false;
        let player = Player::new(spawn[0] as f64, spawn[1] as f64, spawn[2] as f64);
        let prev = [player.x, player.y, player.z];
        Game {
            level,
            dim,
            player,
            prev,
            inv: creative_inventory(),
            entities: Entities::default(),
            health: 20,
            last_health: 20,
            food: 20,
            saturation: 5.0,
            exhaustion: 0.0,
            food_timer: 0,
            last_food: 20,
            effects: Vec::new(),
            air: crate::survival::TOTAL_AIR_SUPPLY,
            xp_level: 0,
            xp_progress: 0.0,
            xp_total: 0,
            score: 0,
            last_level_up: 0,
            take_xp_delay: 0,
            orbs: Vec::new(),
            pearls: Vec::new(),
            dead: false,
            eat_left: 0,
            open_screen: None,
            weather: crate::weather::Weather::default(),
            chest_pos: None,
            chest_group: Vec::new(),
            fall_distance: 0.0,
            sign_pos: None,
            chest_anim: Default::default(),
            pending_unpress: Vec::new(),
            fall_checks: Vec::new(),
            redstone: Default::default(),
            plate_timers: Default::default(),
            moving: Vec::new(),
            piston_queue: Vec::new(),
            hl_timer: 0,
            hl_key: None,
            sneaking: false,
            target: None,
            dirty: Vec::new(),
            day_time: 0,
            world_seed: 0,
            ticks: 0,
            daylight_cycle: true,
            creative: true,
            mining: Mining::default(),
            destroy_stage: None,
            particles: ParticleEngine::new(),
            sounds: Vec::new(),
            swing_time: 0,
            swinging: false,
            hand: crate::hand::HandState::new(),
            last_use_tick: 0,
            move_dist: 0.0,
            next_step: 1,
            random: Random::new(),
            mobs: crate::mobs::Mobs::default(),
            spawn_mobs: false,
            difficulty: 2,
            spawn_pos: [spawn[0] as f64, spawn[1] as f64, spawn[2] as f64],
            was_in_water: false,
            invuln: 0,
            last_hurt: 0.0,
            hurt_time: 0,
            hurt_dir: 0.0,
            mob_target: None,
        }
    }

    /// One 20 Hz game tick without any button pressed.
    pub fn tick(&mut self, input: &Input) {
        self.tick_with(input, &Actions::default());
    }

    /// One 20 Hz game tick: mouse buttons (`Minecraft::tick`), player movement, footsteps, particles.
    pub fn tick_with(&mut self, input: &Input, act: &Actions) {
        if self.dead {
            // the death screen is up: the world keeps going, the player does not
            self.ticks += 1;
            self.tick_level();
            self.tick_mobs();
            self.particles.tick(&self.level);
            return;
        }
        self.tick_buttons();
        self.update_target(1.0);
        self.ticks += 1;
        // --- buttons
        if act.attack_pressed {
            self.handle_mouse_click_attack();
        }
        self.handle_mouse_down_attack(act.attack_held);
        if act.use_held {
            // first click, or auto-repeat every ticksPerSecond / 4 = 5 ticks (not while sprinting)
            let first = self.last_use_tick == 0;
            let repeat = self.ticks - self.last_use_tick >= 5 && !self.player.sprinting;
            if first || repeat {
                self.place_block();
                self.last_use_tick = self.ticks;
            }
            self.update_eating(true);
        } else {
            self.last_use_tick = 0;
            self.update_eating(false);
        }
        if self.mining.miss_time > 0 {
            self.mining.miss_time -= 1;
        }
        // LivingEntity::updateSwingTime (SWING_DURATION = 6)
        if self.swinging {
            self.swing_time += 1;
            if self.swing_time >= 6 {
                self.swing_time = 0;
                self.swinging = false;
            }
        } else {
            self.swing_time = 0;
        }
        // the name of the held item fades in whenever the selection changes (`Gui::tick`)
        let key = self.inv.selected_item().map(|st| (st.id, st.damage));
        if key != self.hl_key {
            self.hl_timer = if key.is_some() { 40 } else { 0 };
            self.hl_key = key;
        } else if self.hl_timer > 0 {
            self.hl_timer -= 1;
        }
        let attack_anim = self.swing_time as f32 / 6.0;
        self.hand.tick(self.inv.selected_item(), self.inv.selected, attack_anim, self.player.pitch, self.player.yaw);

        // --- movement
        self.prev = [self.player.x, self.player.y, self.player.z];
        self.sneaking = input.sneak;
        let vel = (self.player.xd, self.player.yd, self.player.zd);
        // only creative players fly
        if !self.creative {
            self.player.flying = false;
        }
        // `LocalPlayer::aiStep`: no sprinting on an empty stomach (<= 6) or while eating
        let mut inp = *input;
        if !self.creative && (self.food <= 6 || self.eat_left > 0) {
            inp.sprint = false;
        }
        self.player.tick(&self.level, &inp);
        if !self.creative {
            if self.player.jumped {
                self.add_exhaustion(if self.player.sprinting { crate::survival::fc::EXHAUSTION_SPRINT_JUMP } else { crate::survival::fc::EXHAUSTION_JUMP });
            }
            self.move_exhaustion();
        }
        self.check_fall_damage(vel.1);
        if self.player.in_water && !self.was_in_water {
            self.water_splash(vel);
        }
        self.was_in_water = self.player.in_water;
        self.step_sounds(input);
        self.sprint_particles();
        if self.daylight_cycle {
            self.day_time += 1;
        }
        self.particles.tick(&self.level);
        self.tick_entities();
        self.tick_level();
        self.tick_mobs();
        self.tick_food();
        self.tick_effects();
        self.tick_air();
        self.tick_orbs();
        self.tick_pearls();
    }

    /// `Entity::checkFallDamage` / `Player::causeFallDamage`: ceil(distance - 3) hearts of damage on landing.
    fn check_fall_damage(&mut self, _yd: f64) {
        if self.creative || self.player.flying || self.player.in_water || self.player.in_lava {
            self.fall_distance = 0.0;
            return;
        }
        let dy = self.player.y - self.prev[1];
        if self.player.on_ground {
            if self.fall_distance > 0.0 {
                let dist = self.fall_distance;
                self.fall_distance = 0.0;
                self.fall_damage(dist);
            }
        } else if dy < 0.0 {
            self.fall_distance -= dy as f32;
        }
    }

    ///  Fall damage ignores armour and the difficulty.
    pub fn fall_damage(&mut self, dist: f32) {
        let d = (dist - 3.0).ceil() as i32;
        if d <= 0 || self.health <= 0 {
            return;
        }
        let (x, y, z) = (self.player.x, self.player.y, self.player.z);
        self.play(if d > 4 { "damage.fallbig" } else { "damage.fallsmall" }, x, y, z, 1.0, 1.0);
        self.hurt_bypass(d);
    }

    /// `ChestTileEntity::tick`: the lid swings 0.1 per tick towards open / closed.
    fn tick_chests(&mut self) {
        let open = self.chest_group.clone();
        let mut done = Vec::new();
        for (p, a) in self.chest_anim.iter_mut() {
            a.prev = a.open;
            let was = a.open;
            if open.contains(p) {
                a.open = (a.open + 0.1).min(1.0);
            } else {
                a.open = (a.open - 0.1).max(0.0);
            }
            let _ = was;
            if a.open <= 0.0 && !open.contains(p) {
                done.push(*p);
            }
        }
        for p in done {
            self.chest_anim.remove(&p);
            crate::chest::set_animating(p, false);
            self.mark(p.0, p.2);
        }
    }

    /// Dropped items: physics, lava fizz, pickup by the player (`ItemEntity::playerTouch`).
    fn tick_entities(&mut self) {
        let mut fizz = Vec::new();
        self.entities.tick(&self.level, &mut self.random, &mut fizz);
        self.tick_falling();
        self.tick_redstone();
        self.tick_pistons();
        self.tick_chests();
        self.tick_primed();
        for f in fizz {
            let pitch = 2.0 + self.random.next_float() * 0.4;
            self.play("random.fizz", f[0], f[1], f[2], 0.4, pitch);
        }
        let reach = self.player.bb.grow(1.0, 0.0, 1.0);
        let mut picked = Vec::new();
        for e in self.entities.items.iter_mut() {
            if e.removed || e.throw_time > 0 || !reach.intersects(&e.body.bb) {
                continue;
            }
            let before = e.item.count;
            let mut st = e.item;
            self.inv.add(&mut st);
            if st.count != before {
                picked.push([e.body.x, e.body.y, e.body.z]);
            }
            e.item.count = st.count;
            if st.count == 0 {
                e.removed = true;
            }
        }
        self.entities.items.retain(|e| !e.removed);
        for p in picked {
            let pitch = ((self.random.next_float() - self.random.next_float()) * 0.7 + 1.0) * 2.0;
            self.play("random.pop", p[0], p[1], p[2], 0.2, pitch);
        }
    }

    /// `Entity::doWaterSplashEffect`: splash sound plus bubbles and droplets where the player hits the water.
    fn water_splash(&mut self, v: (f64, f64, f64)) {
        let (x, z, y0) = (self.player.x, self.player.z, self.player.bb.y0);
        let speed = ((v.0 * v.0 * 0.2 + v.1 * v.1 + v.2 * v.2 * 0.2).sqrt() * 0.2).min(1.0) as f32;
        let pitch = 1.0 + (self.random.next_float() - self.random.next_float()) * 0.4;
        self.play("liquid.splash", x, y0, z, speed.max(0.1), pitch);
        let yt = y0.floor();
        let w = self.player.bb.x1 - self.player.bb.x0;
        let n = (1.0 + w * 20.0) as i32;
        for k in 0..2 {
            for _ in 0..n {
                let xo = (self.random.next_float() * 2.0 - 1.0) as f64 * w;
                let zo = (self.random.next_float() * 2.0 - 1.0) as f64 * w;
                let yd = if k == 0 { v.1 - self.random.next_float() as f64 * 0.2 } else { v.1 };
                let (px, py, pz) = (x + xo, yt + 1.0, z + zo);
                let id = self.level.get_tile(px.floor() as i32, yt as i32, pz.floor() as i32);
                if !matches!(id, t::WATER | t::CALM_WATER) {
                    continue;
                }
                let (bx, by, bz) = (px.floor() as i32, yt as i32, pz.floor() as i32);
                self.particles.tile_crack(&self.level, bx, by, bz, id, 0, px, py, pz, v.0 * 0.5, yd.abs() * 0.5 + 0.2, v.2 * 0.5);
            }
        }
    }

    /// Throw a stack into the world in front of the player (`Player::drop`).
    pub fn drop_item(&mut self, st: ItemStack, from_screen: bool) {
        let p = &self.player;
        let (yaw, pitch) = ((p.yaw as f64).to_radians(), (p.pitch as f64).to_radians());
        let mut e = ItemEntity::new(p.x, p.y - 0.3 + 0.12, p.z, st, &mut self.random);
        e.throw_time = 40;
        if from_screen {
            let r = self.random.next_float() as f64 * 0.5;
            let a = self.random.next_float() as f64 * std::f64::consts::PI * 2.0;
            e.body.xd = -a.sin() * r;
            e.body.zd = a.cos() * r;
            e.body.yd = 0.2;
        } else {
            e.body.xd = -yaw.sin() * pitch.cos() * 0.3;
            e.body.zd = yaw.cos() * pitch.cos() * 0.3;
            e.body.yd = -pitch.sin() * 0.3 + 0.1;
            let a = self.random.next_float() as f64 * std::f64::consts::PI * 2.0;
            let r = 0.02 * self.random.next_float() as f64;
            e.body.xd += a.cos() * r;
            e.body.yd += (self.random.next_float() - self.random.next_float()) as f64 * 0.1;
            e.body.zd += a.sin() * r;
        }
        self.entities.items.push(e);
    }

    /// `Player::spawnItemParticles`: bits of the held item fly up from in front of the mouth.
    fn eat_particles(&mut self, item: u16, count: i32) {
        let Some(def) = mc_world::item::item_def(item) else { return };
        let (xr, yr) = ((-self.player.pitch as f64).to_radians(), (-self.player.yaw as f64).to_radians());
        let rot = |mut v: [f64; 3]| {
            // Vec3::xRot then Vec3::yRot
            let (s, c) = xr.sin_cos();
            let (y, z) = (v[1] * c + v[2] * s, v[2] * c - v[1] * s);
            v[1] = y;
            v[2] = z;
            let (s, c) = yr.sin_cos();
            let (x, z) = (v[0] * c + v[2] * s, v[2] * c - v[0] * s);
            v[0] = x;
            v[2] = z;
            v
        };
        for _ in 0..count {
            let d = rot([(self.random.next_float() as f64 - 0.5) * 0.1, self.random.next_double() * 0.1 + 0.1, 0.0]);
            let p = rot([(self.random.next_float() as f64 - 0.5) * 0.3, -(self.random.next_float() as f64) * 0.6 - 0.3, 0.6]);
            let (x, y, z) = (p[0] + self.player.x, p[1] + self.player.y, p[2] + self.player.z);
            let part = crate::particles::Particle::item_crack(&mut self.random, x, y, z, d[0], d[1] + 0.05, d[2], def.icon_pos);
            self.particles.add(part);
        }
    }

    /// Hold right button with food in hand: 32 ticks of eating (`LocalPlayer` use-item loop, `FoodItem`).
    fn update_eating(&mut self, held: bool) {
        let food = match self.inv.selected_item().map(|s| (s.id, s.kind())) {
            Some((id, ItemKind::Food { nutrition, saturation, .. })) if self.can_eat(crate::survival::can_always_eat(id)) => Some((id, nutrition, saturation)),
            _ => None,
        };
        let Some((item, nutrition, modifier)) = food else {
            self.eat_left = 0;
            return;
        };
        if !held {
            self.eat_left = 0;
            return;
        }
        if self.eat_left == 0 {
            self.eat_left = 32;
        }
        self.eat_left -= 1;
        let (x, y, z) = (self.player.x, self.player.y, self.player.z);
        if self.eat_left <= 25 && self.eat_left % 4 == 0 {
            self.eat_particles(item, 5);
        }
        if self.eat_left > 7 && self.eat_left % 4 == 0 {
            let vol = 0.5 + 0.5 * self.random.next_int_bounded(2) as f32;
            let pitch = (self.random.next_float() - self.random.next_float()) * 0.2 + 1.0;
            self.play("random.eat", x, y, z, vol, pitch);
        }
        if self.eat_left == 0 {
            // `FoodItem::useTimeDepleted`: eat, burp, then the item's effect
            self.eat_food(nutrition as i32, modifier);
            self.inv.use_selected();
            let pitch = self.random.next_float() * 0.1 + 0.9;
            self.play("random.burp", x, y, z, 0.5, pitch);
            if let Some((id, secs, amp, prob)) = crate::survival::eat_effect(item) {
                if self.random.next_float() < prob {
                    self.add_effect(crate::survival::Effect { id, duration: secs * 20, amplifier: amp });
                }
            }
        }
    }

    /// `ServerLevel::tick`: scheduled tile ticks (water/lava flow, sand), then flag the chunks whose blocks changed.
    fn tick_level(&mut self) {
        self.level.game_time += 1;
        let now = self.level.game_time;
        let mut due: Vec<_> = self.level.pending_ticks.iter().copied().filter(|p| p.time <= now).collect();
        if !due.is_empty() {
            self.level.pending_ticks.retain(|p| p.time > now);
            due.sort_by_key(|p| p.time);
            due.truncate(1000);
            for p in due {
                if self.level.has_chunks_in(p.x - 8, p.y - 8, p.z - 8, p.x + 8, p.y + 8, p.z + 8) && self.level.get_tile(p.x, p.y, p.z) == p.tile {
                    self.level.tick_tile(p.x, p.y, p.z, p.tile, &mut self.random);
                }
            }
        }
        self.tick_weather();
        self.flush_changes();
    }

    /// `Level::tickWeather` + the weather part of `tickChunk` around the player, lightning and rain sounds.
    fn tick_weather(&mut self) {
        let sky = matches!(self.dim, Dim::Overworld | Dim::Flat);
        self.weather.tick(!sky);
        if !sky {
            return;
        }
        let (pcx, pcz) = ((self.player.x / 16.0).floor() as i32, (self.player.z / 16.0).floor() as i32);
        if self.weather.rain_level > 0.0 || self.weather.raining {
            let mut strikes = Vec::new();
            for cx in pcx - 5..=pcx + 5 {
                for cz in pcz - 5..=pcz + 5 {
                    if self.level.has_chunk(cx, cz) {
                        if let Some(s) = self.weather.chunk_tick(&mut self.level, cx, cz) {
                            strikes.push(s);
                        }
                    }
                }
            }
            for (x, y, z) in strikes {
                self.weather.strike(x as f64 + 0.5, y as f64, z as f64 + 0.5);
                let (p1, p2) = (0.8 + self.random.next_float() * 0.2, 0.5 + self.random.next_float() * 0.2);
                self.play("ambient.weather.thunder", x as f64, y as f64, z as f64, 10000.0, p1);
                self.play("random.explode", x as f64, y as f64, z as f64, 2.0, p2);
            }
            // `GameRenderer::tickRain`: a rain sound from a random column near the player
            if self.weather.rain_level > 0.0 && self.random.next_int_bounded(3) == 0 {
                let (px, pz) = (self.player.x.floor() as i32, self.player.z.floor() as i32);
                let (rx, rz) = (px + self.random.next_int_bounded(21) - 10, pz + self.random.next_int_bounded(21) - 10);
                if self.level.has_chunk(rx >> 4, rz >> 4) && self.level.get_biome(rx, rz).has_rain() {
                    let top = self.level.get_top_rain_block(rx, rz);
                    let own_top = self.level.get_top_rain_block(px, pz);
                    let (py, v) = if top as f64 > self.player.y + 1.0 && own_top > self.player.y.floor() as i32 { (top, 0.1) } else { (top, 0.2) };
                    let pit = if v < 0.15 { 0.5 } else { 1.0 };
                    self.play("ambient.weather.rain", rx as f64 + 0.5, py as f64 + 0.2, rz as f64 + 0.5, v, pit);
                }
            }
        }
    }

    /// Mark the chunks of every block changed by the level simulation for re-meshing.
    pub fn flush_changes(&mut self) {
        let log = std::mem::take(&mut self.level.change_log);
        for (x, _y, z) in log {
            self.mark(x, z);
        }
    }

    /// `Entity::baseTick`: a sprinting entity kicks up tile-crack particles from the block it stands on.
    fn sprint_particles(&mut self) {
        let p = &self.player;
        if !p.sprinting || p.in_water {
            return;
        }
        let (xt, yt, zt) = (p.x.floor() as i32, (p.y - 0.2 - 1.62).floor() as i32, p.z.floor() as i32);
        let id = self.level.get_tile(xt, yt, zt);
        if id == 0 {
            return;
        }
        let data = self.level.get_data(xt, yt, zt);
        let (x, z, y0, xd, zd) = (p.x, p.z, p.bb.y0, p.xd, p.zd);
        let (rx, rz) = (self.random.next_float() as f64, self.random.next_float() as f64);
        self.particles.tile_crack(&self.level, xt, yt, zt, id, data, x + (rx - 0.5) * 0.6, y0 + 0.1, z + (rz - 0.5) * 0.6, -xd * 4.0, 1.5, -zd * 4.0);
    }

    pub(crate) fn swing(&mut self) {
        // `LivingEntity::swing`: only restarts once the previous swing is at least half way
        if !self.swinging || self.swing_time >= 3 || self.swing_time < 0 {
            self.swing_time = -1;
            self.swinging = true;
        }
    }

    pub fn take_sounds(&mut self) -> Vec<SoundEvent> {
        std::mem::take(&mut self.sounds)
    }

    pub fn play(&mut self, name: &'static str, x: f64, y: f64, z: f64, volume: f32, pitch: f32) {
        self.sounds.push(SoundEvent { name, pos: [x, y, z], volume, pitch });
    }

    // ------------------------------------------------------------------------------------ footsteps

    /// The footstep part of `Entity::move` + `Entity::playStepSound`.
    fn step_sounds(&mut self, input: &Input) {
        let p = &self.player;
        let sneaking_on_ground = p.on_ground && input.sneak;
        if p.flying || sneaking_on_ground {
            return;
        }
        let (xm, mut ym, zm) = (p.x - self.prev[0], p.y - self.prev[1], p.z - self.prev[2]);
        let (xt, yt, zt) = (p.x.floor() as i32, (p.y - 0.2 - 1.62).floor() as i32, p.z.floor() as i32);
        let tile = self.level.get_tile(xt, yt, zt);
        if tile != t::LADDER {
            ym = 0.0;
        }
        self.move_dist += (xm * xm + ym * ym + zm * zm).sqrt() * 0.6;
        if self.move_dist > self.next_step as f64 && tile > 0 {
            self.next_step = self.move_dist as i32 + 1;
            let (px, py, pz) = (p.x, p.y - 1.62, p.z);
            if p.in_water {
                let sp = ((p.xd * p.xd * 0.2 + p.yd * p.yd + p.zd * p.zd * 0.2).sqrt() * 0.35).min(1.0) as f32;
                let pitch = 1.0 + (self.random.next_float() - self.random.next_float()) * 0.4;
                self.play("liquid.swim", px, py, pz, sp, pitch);
            }
            // playStepSound: snow layer on top overrides the block's own sound; liquids are silent
            let on_snow = self.level.get_tile(xt, yt + 1, zt) == t::TOP_SNOW;
            let st = if on_snow { sound_type(t::TOP_SNOW) } else { sound_type(tile) };
            let is_liquid = tile_info(tile).and_then(|i| i.material).map_or(false, Material::is_liquid);
            if on_snow || !is_liquid {
                if let Some(name) = st.step {
                    self.play(name, px, py, pz, st.volume * 0.15, st.pitch);
                }
            }
        }
    }

    // ------------------------------------------------------------------------------------ mining

    /// `Entity::isUnderLiquid(Material::water)` for the player's head.
    pub(crate) fn head_under_water(&self) -> bool {
        let p = &self.player;
        let yp = p.y + 0.12; // Player::getHeadHeight
        let (xt, yt, zt) = (p.x.floor() as i32, yp.floor() as i32, p.z.floor() as i32);
        let id = self.level.get_tile(xt, yt, zt);
        if id == 0 || tile_info(id).and_then(|i| i.material) != Some(Material::Water) {
            return false;
        }
        let d = self.level.get_data(xt, yt, zt);
        let height = (if d >= 8 { 0 } else { d } as f32 + 1.0) / 9.0; // LiquidTile::getHeight
        yp < (yt as f32 + 1.0 - (height - 1.0 / 9.0)) as f64
    }

    /// `Tile::getDestroyProgress(player, level, x, y, z)` for an empty hand: progress made per tick.
    pub fn destroy_progress_per_tick(&self, id: u8) -> f32 {
        let Some(info) = tile_info(id) else { return 0.0 };
        let destroy_speed = info.destroy_time;
        if destroy_speed < 0.0 {
            return 0.0;
        }
        let mut speed = item_destroy_speed(self.inv.selected_item(), id); // Inventory::getDestroySpeed
        if self.head_under_water() {
            speed /= 5.0;
        }
        let can_destroy = info.material.map_or(true, Material::is_always_destroyable) || can_harvest(self.inv.selected_item(), id);
        if can_destroy {
            speed / destroy_speed / 30.0
        } else {
            speed / destroy_speed / 100.0
        }
    }

    fn set_destroy_stage(&mut self, pos: (i32, i32, i32), stage: i32) {
        // LevelRenderer::destroyTileProgress: stages outside 0..10 remove the entry
        self.destroy_stage = if (0..10).contains(&stage) { Some((pos, stage as usize)) } else { None };
    }

    /// `Minecraft::handleMouseClick(0)`
    fn handle_mouse_click_attack(&mut self) {
        if self.mining.miss_time > 0 {
            return;
        }
        self.swing();
        if self.try_attack_mob() {
            return;
        }
        match self.target {
            None => {
                if !self.creative {
                    self.mining.miss_time = 10;
                }
            }
            Some(h) => self.start_destroy_block(h.pos),
        }
    }

    /// `LocalPlayer::handleMouseDown(0, down)`
    fn handle_mouse_down_attack(&mut self, down: bool) {
        if !down {
            self.mining.miss_time = 0;
        }
        if self.mining.miss_time > 0 {
            return;
        }
        match (down, self.target) {
            (true, Some(h)) => {
                let (x, y, z) = h.pos;
                // 4J: no mining out of the bottom of the world, nor the ceiling of Nether (not in the End)
                if (y == 0 || (y == 127 && self.dim == Dim::Nether)) && self.dim != Dim::End {
                    return;
                }
                self.continue_destroy_block(h.pos);
                self.particles.crack(&self.level, x, y, z, h.face);
                self.swing();
            }
            _ => self.stop_destroy_block(),
        }
    }

    /// `MultiPlayerGameMode::startDestroyBlock`
    pub fn start_destroy_block(&mut self, pos: (i32, i32, i32)) {
        let (x, y, z) = pos;
        if self.creative {
            if self.mining.delay > 0 {
                return;
            }
            self.destroy_block(pos);
            self.mining.delay = 5;
        } else if !self.mining.is_destroying || self.mining.pos != pos {
            let id = self.level.get_tile(x, y, z);
            if id > 0 && self.destroy_progress_per_tick(id) >= 1.0 {
                self.destroy_block(pos);
            } else {
                self.mining = Mining { pos, is_destroying: true, progress: 0.0, ticks: 0, delay: self.mining.delay, miss_time: self.mining.miss_time };
                self.set_destroy_stage(pos, (self.mining.progress * 10.0) as i32 - 1);
            }
        }
    }

    /// `MultiPlayerGameMode::stopDestroyBlock`
    pub fn stop_destroy_block(&mut self) {
        self.mining.is_destroying = false;
        self.mining.progress = 0.0;
        self.mining.delay = 0;
        self.destroy_stage = None;
    }

    /// `MultiPlayerGameMode::continueDestroyBlock`
    pub fn continue_destroy_block(&mut self, pos: (i32, i32, i32)) {
        let (x, y, z) = pos;
        if self.mining.delay > 0 {
            self.mining.delay -= 1;
            return;
        }
        if self.creative {
            self.mining.delay = 5;
            self.destroy_block(pos);
            return;
        }
        if self.mining.is_destroying && self.mining.pos == pos {
            let id = self.level.get_tile(x, y, z);
            if id == 0 {
                self.mining.is_destroying = false;
                return;
            }
            self.mining.progress += self.destroy_progress_per_tick(id);
            if self.mining.ticks % 4 == 0 {
                let st = sound_type(id);
                if let Some(name) = st.step {
                    self.play(name, x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5, (st.volume + 1.0) / 8.0, st.pitch * 0.5);
                }
            }
            self.mining.ticks += 1;
            if self.mining.progress >= 1.0 {
                self.mining.is_destroying = false;
                self.destroy_block(pos);
                self.mining.progress = 0.0;
                self.mining.ticks = 0;
                self.mining.delay = 5;
            }
            self.set_destroy_stage(pos, (self.mining.progress * 10.0) as i32 - 1);
        } else {
            self.start_destroy_block(pos);
        }
    }

    /// `MultiPlayerGameMode::destroyBlock`: break sound + debris (`LevelEvent::PARTICLES_DESTROY_BLOCK`), then the
    /// block is removed.
    pub fn destroy_block(&mut self, pos: (i32, i32, i32)) -> bool {
        let (x, y, z) = pos;
        let id = self.level.get_tile(x, y, z);
        if id == 0 || id == t::UNBREAKABLE {
            return false;
        }
        let data = self.level.get_data(x, y, z);
        let st = sound_type(id);
        self.play(st.brk, x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5, (st.volume + 1.0) / 2.0, st.pitch * 0.8);
        self.particles.destroy(&self.level, x, y, z, id, data);
        let held = self.inv.selected_item().copied();
        // a chest spills its contents (`ChestTile::onRemove`)
        if id == t::CHEST {
            if let Some(mc_level::tile_entity::TileEntity::Chest { items }) = self.level.tile_entities.remove(&pos) {
                for it in items.into_iter().flatten() {
                    let st = ItemStack::with_damage(it.id.clamp(0, 65535) as u16, it.count.clamp(0, 255) as u8, it.damage.clamp(0, 65535) as u16);
                    let e = ItemEntity::pop(x, y, z, st, &mut self.random);
                    self.entities.items.push(e);
                }
            }
        }
        let changed = self.level.set_tile(x, y, z, 0, UPDATE_ALL);
        if changed && matches!(id, t::SIGN | t::WALL_SIGN) {
            self.level.tile_entities.remove(&pos);
        }
        if changed {
            if crate::piston::is_piston_base(id) {
                self.piston_base_removed(pos, data);
            } else if id == t::PISTON_EXTENSION_PIECE {
                self.piston_head_removed(pos, data);
            }
            self.break_dependents(id, data, pos);
            self.schedule_fall(x, y + 1, z);
            self.redstone.notify(pos);
        }
        if !self.creative && changed {
            self.add_exhaustion(crate::survival::fc::EXHAUSTION_MINE);
            // Tile::playerDestroy → spawnResources (only when the tool can harvest), then Item::mineBlock wear
            if can_harvest(held.as_ref(), id) {
                let shears = held.map_or(false, |h| h.id == mc_world::item::ids::SHEARS);
                for d in block_drops(id, data, &mut self.random, shears, 0) {
                    let e = ItemEntity::pop(x, y, z, d, &mut self.random);
                    self.entities.items.push(e);
                }
                let xp = self.block_xp(id);
                if xp > 0 {
                    self.spawn_xp(x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5, xp);
                }
            }
            if let Some(h) = held {
                let wear = match h.kind() {
                    ItemKind::Sword(_) => 2,
                    ItemKind::Pickaxe(_) | ItemKind::Shovel(_) | ItemKind::Axe(_) => 1,
                    _ => 0,
                };
                if wear > 0 && tile_info(id).map_or(false, |i| i.destroy_time != 0.0) && self.inv.damage_selected(wear) {
                    let (px, py, pz, pit) = (self.player.x, self.player.y, self.player.z, 0.8 + self.random.next_float() * 0.4);
                    self.play("random.break", px, py, pz, 0.8, pit);
                }
            }
        }
        self.mark(x, z);
        if self.mining.pos == pos {
            self.mining.pos = (-1, -1, -1);
        }
        changed
    }

    /// Sky / fog / light parameters for this frame (`GameRenderer::updateFogColor`, `setupFog`, `renderSky`).
    pub fn env(&self, partial: f32) -> crate::sky::Env {
        let e = self.eye(partial);
        let pl = &self.player;
        let biome = self.level.get_biome(e[0].floor() as i32, e[2].floor() as i32);
        crate::sky::compute_env(&crate::sky::EnvInput {
            dim: self.dim,
            flat: self.dim == Dim::Flat,
            day_time: self.day_time,
            partial,
            eye: e,
            look: {
                let l = pl.look_dir();
                [l[0] as f32, l[1] as f32, l[2] as f32]
            },
            tile_at_eye: self.level.get_tile(e[0].floor() as i32, e[1].floor() as i32, e[2].floor() as i32),
            fog_br: 1.0, // 1 for the console default view distance (whiteness = (3 - 0) / 3)
            biome_sky: mc_world::colours::BIOME_COLOURS[biome.id as usize % mc_world::colours::BIOME_COLOURS.len()][3],
            ticks: self.ticks as f32 + partial,
            rain: self.weather.rain(partial),
            thunder: self.weather.thunder(partial),
            flash: (self.weather.sky_flash as f32 - partial).max(0.0),
        })
    }

    /// Is the camera inside water? (`Camera::getBlockAt` is water: FOV x 60/70)
    pub fn tile_at_eye_water(&self) -> bool {
        let e = self.eye(1.0);
        matches!(self.level.get_tile(e[0].floor() as i32, e[1].floor() as i32, e[2].floor() as i32), t::WATER | t::CALM_WATER)
    }

    /// `GameRenderer::bobHurt`: the camera rolls up to 14 degrees while `hurt_time` runs down from 10.
    pub fn hurt_view(&self, partial: f32) -> glam::Mat4 {
        let hurt = self.hurt_time as f32 - partial;
        if hurt < 0.0 {
            return glam::Mat4::IDENTITY;
        }
        let h = (hurt / 10.0).powi(4);
        let h = (h * std::f32::consts::PI).sin();
        let rr = self.hurt_dir.to_radians();
        glam::Mat4::from_rotation_y(-rr) * glam::Mat4::from_rotation_z(-(h * 14.0).to_radians()) * glam::Mat4::from_rotation_y(rr)
    }

    /// Eye position between the last two ticks.
    pub fn eye(&self, partial: f32) -> [f32; 3] {
        let p = partial as f64;
        let pl = &self.player;
        let drop = pl.o_sneak_drop + (pl.sneak_drop - pl.o_sneak_drop) * partial;
        [
            (self.prev[0] + (self.player.x - self.prev[0]) * p) as f32,
            (self.prev[1] + (self.player.y - self.prev[1]) * p) as f32 - drop,
            (self.prev[2] + (self.player.z - self.prev[2]) * p) as f32,
        ]
    }

    pub fn update_target(&mut self, partial: f32) {
        let e = self.eye(partial);
        let origin = [e[0] as f64, e[1] as f64, e[2] as f64];
        self.target = raycast(&self.level, origin, self.player.look_dir(), REACH);
        // GameRenderer::pick: a mob closer than the block replaces the block target
        self.mob_target = None;
        if self.spawn_mobs {
            let reach = if self.creative { 5.0 } else { 3.0 };
            let block_d = self.target.map_or(f64::MAX, |h| h.distance);
            if let Some((i, d)) = self.mobs.pick(origin, self.player.look_dir(), reach) {
                if d < block_d {
                    self.mob_target = Some((i, d));
                    self.target = None;
                }
            }
        }
    }

    pub fn mark(&mut self, x: i32, z: i32) {
        let (cx, cz) = (x >> 4, z >> 4);
        let (lx, lz) = (x & 15, z & 15);
        let mut add = |c: (i32, i32)| {
            if !self.dirty.contains(&c) {
                self.dirty.push(c);
            }
        };
        add((cx, cz));
        let dx = if lx == 0 { -1 } else if lx == 15 { 1 } else { 0 };
        let dz = if lz == 0 { -1 } else if lz == 15 { 1 } else { 0 };
        if dx != 0 {
            add((cx + dx, cz));
        }
        if dz != 0 {
            add((cx, cz + dz));
        }
        if dx != 0 && dz != 0 {
            add((cx + dx, cz + dz));
        }
    }

    /// Chunks whose mesh must be rebuilt since the last call.
    /// Dimension name used in save file names.
    pub fn dim_name(&self) -> &'static str {
        match self.dim {
            Dim::Overworld => "overworld",
            Dim::Nether => "nether",
            Dim::End => "end",
            Dim::Flat => "flat",
        }
    }

    pub fn take_dirty(&mut self) -> Vec<(i32, i32)> {
        std::mem::take(&mut self.dirty)
    }

    /// Instant break of the targeted block (creative-style click).
    pub fn break_block(&mut self) -> bool {
        let Some(h) = self.target else { return false };
        self.swing();
        self.destroy_block(h.pos)
    }

    /// `Minecraft::handleMouseClick(1)`: use the held item on the targeted block (`ItemInstance::useOn`).
    pub fn place_block(&mut self) -> bool {
        if self.try_interact_mob() {
            return true;
        }
        if self.last_use_tick == 0 && self.throw_pearl() {
            return true;
        }
        let Some(h) = self.target else { return false };
        // blocks with a GUI take the click first (not while sneaking)
        if self.level.get_tile(h.pos.0, h.pos.1, h.pos.2) == t::WORK_BENCH && !self.sneaking {
            self.open_screen = Some(crate::screens::ScreenId::Crafting);
            self.swing();
            return true;
        }
        if self.level.get_tile(h.pos.0, h.pos.1, h.pos.2) == t::CHEST && !self.sneaking {
            if !self.open_chest(h.pos) {
                // something solid on top: the click is used up but nothing opens
                self.swing();
                return true;
            }
            self.open_screen = Some(crate::screens::ScreenId::Chest);
            let pit = 0.9 + self.random.next_float() * 0.1;
            self.play("random.chestopen", h.pos.0 as f64 + 0.5, h.pos.1 as f64 + 0.5, h.pos.2 as f64 + 0.5, 0.5, pit);
            self.swing();
            return true;
        }
        if !self.sneaking && self.use_special_block(h.pos) {
            return true;
        }
        let Some(held) = self.inv.selected_item().copied() else { return false };
        let ok = if matches!(held.id, 323 | 324 | 330 | 331 | 354 | 356) {
            self.place_special_item(held.id)
        } else {
            match held.kind() {
            ItemKind::Hoe(_) => self.use_hoe(h.pos),
            ItemKind::Planter(b) => self.place_tile(b, 0, true),
            ItemKind::Food { .. } => false,
            _ if held.id < 256 => self.place_tile(held.id as u8, held.damage as u8, false),
            _ => false,
            }
        };
        if ok {
            self.hand.item_used();
        }
        if ok && !self.creative && (held.id < 256 || matches!(held.id, 323 | 324 | 330 | 331 | 354 | 356) || matches!(held.kind(), ItemKind::Planter(_))) {
            self.inv.use_selected();
        }
        ok
    }

    /// `HoeItem::useOn`: grass / dirt under a free block becomes farmland.
    fn use_hoe(&mut self, pos: (i32, i32, i32)) -> bool {
        let (x, y, z) = pos;
        let above = self.level.get_tile(x, y + 1, z);
        let here = self.level.get_tile(x, y, z);
        if self.target.map_or(true, |h| h.face == 0) || (above != 0 && above != t::TALLGRASS) || (here != t::GRASS && here != t::DIRT) {
            return false;
        }
        self.level.set_tile_and_data(x, y, z, t::FARMLAND, 0, UPDATE_ALL);
        self.mark(x, z);
        self.swing();
        self.play("step.gravel", x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5, 1.0, 0.8);
        if !self.creative {
            self.inv.damage_selected(1);
        }
        true
    }

    /// `TileItem::useOn` / `TilePlanterItem`: set `id` in the free block next to the targeted face.
    fn place_tile(&mut self, id: u8, data: u8, planter: bool) -> bool {
        let Some(h) = self.target else { return false };
        let (x, y, z) = h.adjacent();
        if !(0..256).contains(&y) {
            return false;
        }
        // placing a half slab on / under the same kind of half slab makes a double slab
        if matches!(id, t::STONE_SLAB_HALF | t::WOOD_SLAB_HALF) {
            let (hx, hy, hz) = h.pos;
            let (hid, hd) = (self.level.get_tile(hx, hy, hz), self.level.get_data(hx, hy, hz));
            if hid == id && hd & 7 == data & 7 && ((h.face == 1 && hd & 8 == 0) || (h.face == 0 && hd & 8 != 0)) {
                self.level.set_tile_and_data(hx, hy, hz, id - 1, data & 7, UPDATE_ALL);
                self.mark(hx, hz);
                self.swing();
                let st = sound_type(id);
                self.play(st.place, hx as f64 + 0.5, hy as f64 + 0.5, hz as f64 + 0.5, (st.volume + 1.0) / 2.0, st.pitch * 0.8);
                return true;
            }
        }
        let here = self.level.get_tile(x, y, z);
        let replaceable = here == 0 || matches!(here, t::WATER | t::CALM_WATER | t::LAVA | t::CALM_LAVA | t::TALLGRASS | t::FIRE | t::TOP_SNOW | t::DEAD_BUSH | t::VINE);
        if !replaceable {
            return false;
        }
        if planter {
            // crops / stems need farmland below and a free spot
            let below = self.level.get_tile(x, y - 1, z);
            let needs_farmland = !matches!(id, t::NETHER_STALK);
            if h.face != 1 || (needs_farmland && below != t::FARMLAND) || (!needs_farmland && below != t::SOULSAND) {
                return false;
            }
        }
        let solid = crate::blocks::style(id, data).opaque;
        if solid && block_intersects_player(&self.player, (x, y, z)) {
            return false;
        }
        if id == t::CHEST && !self.chest_may_place(x, y, z) {
            return false;
        }
        let Some((id, data)) = self.oriented(id, data, h.face, (x, y, z)) else { return false };
        self.level.set_tile_and_data(x, y, z, id, data, UPDATE_ALL);
        if id == t::CHEST {
            self.chest_placed(x, y, z);
        }
        self.mark(x, z);
        self.swing();
        if Self::is_falling_block(id) {
            self.schedule_fall(x, y, z);
        }
        self.redstone.notify((x, y, z));
        if crate::piston::is_piston_base(id) {
            self.piston_check((x, y, z));
        }
        let st = sound_type(id);
        self.play(st.place, x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5, (st.volume + 1.0) / 2.0, st.pitch * 0.8);
        true
    }

    pub fn select(&mut self, slot: usize) {
        self.inv.selected = slot.min(8);
    }
    pub fn scroll(&mut self, delta: i32) {
        self.inv.selected = (self.inv.selected as i32 - delta).rem_euclid(9) as usize;
    }

    /// Geometry of the block being broken, textured with `destroy_<stage>` (`tesselateInWorldFixedTexture`).
    pub fn crack_mesh(&self) -> Option<MeshData> {
        let ((x, y, z), stage) = self.destroy_stage?;
        let id = self.level.get_tile(x, y, z);
        if id == 0 {
            return None;
        }
        let (mn, mx) = crate::physics::selection_at(&self.level, x, y, z);
        let cell = crate::icons::icon(&format!("destroy_{stage}"))?;
        let mut m = MeshData::default();
        let e = 0.0005f32;
        for face in 0..6 {
            let n = crate::mesh::FACE_NORMAL[face];
            let on_edge = match face {
                0 => mn[0] <= 0.0,
                1 => mx[0] >= 1.0,
                2 => mn[1] <= 0.0,
                3 => mx[1] >= 1.0,
                4 => mn[2] <= 0.0,
                _ => mx[2] >= 1.0,
            };
            if on_edge {
                let (nx, ny, nz) = (x + n[0], y + n[1], z + n[2]);
                let nid = self.level.get_tile(nx, ny, nz);
                if nid != 0 && crate::blocks::style(nid, self.level.get_data(nx, ny, nz)).opaque {
                    continue;
                }
            }
            let b = m.vertices.len() as u32;
            for (k, c) in crate::mesh::FACE_CORNERS[face].iter().enumerate() {
                let p = |a: usize, o: i32| {
                    let base = [x, y, z][a] as f32;
                    let v = mn[a] + c[a] * (mx[a] - mn[a]);
                    let push = if a == face / 2 { if face % 2 == 1 { e } else { -e } } else { 0.0 };
                    let _ = o;
                    base + v + push
                };
                m.vertices.push(Vertex { pos: [p(0, 0), p(1, 0), p(2, 0)], uv: crate::mesh::uv_of(cell, k), color: [1.0; 4], light: [15.0, 0.0] });
            }
            m.indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        }
        if m.is_empty() {
            None
        } else {
            Some(m)
        }
    }

    /// Black outline of the targeted block (thin boxes along its 12 edges).
    pub fn selection_mesh(&self) -> Option<MeshData> {
        let h = self.target?;
        let (x, y, z) = h.pos;
        let (mn, mx) = crate::physics::selection_at(&self.level, x, y, z);
        // roughly constant on-screen thickness (the original draws 2px lines)
        let eye = self.eye(0.0);
        let centre = [x as f32 + (mn[0] + mx[0]) / 2.0, y as f32 + (mn[1] + mx[1]) / 2.0, z as f32 + (mn[2] + mx[2]) / 2.0];
        let dist = ((centre[0] - eye[0]).powi(2) + (centre[1] - eye[1]).powi(2) + (centre[2] - eye[2]).powi(2)).sqrt();
        let e = (0.0015 * dist).clamp(0.002, 0.012);
        let lo = [x as f32 + mn[0] - e, y as f32 + mn[1] - e, z as f32 + mn[2] - e];
        let hi = [x as f32 + mx[0] + e, y as f32 + mx[1] + e, z as f32 + mx[2] + e];
        let mut m = MeshData::default();
        let w = (0.0028 * dist).clamp(0.004, 0.024);
        for a in 0..3 {
            for (b, c) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                // an edge along axis `a`, at the b/c corners of the other two axes
                let (a1, a2) = match a {
                    0 => (1, 2),
                    1 => (0, 2),
                    _ => (0, 1),
                };
                let mut min = [0.0f32; 3];
                let mut max = [0.0f32; 3];
                min[a] = lo[a];
                max[a] = hi[a];
                let o1 = if b == 0 { lo[a1] } else { hi[a1] - w };
                let o2 = if c == 0 { lo[a2] } else { hi[a2] - w };
                min[a1] = o1;
                max[a1] = o1 + w;
                min[a2] = o2;
                max[a2] = o2 + w;
                add_solid_box(&mut m, min, max, [0.0, 0.0, 0.0]);
            }
        }
        Some(m)
    }
}

fn add_solid_box(m: &mut MeshData, min: [f32; 3], max: [f32; 3], rgb: [f32; 3]) {
    let c = [rgb[0], rgb[1], rgb[2], 1.0];
    let v = |x: f32, y: f32, z: f32| Vertex { pos: [x, y, z], uv: [-1.0, 0.0], color: c, light: [15.0, 0.0] };
    let (x0, y0, z0, x1, y1, z1) = (min[0], min[1], min[2], max[0], max[1], max[2]);
    let faces = [
        [v(x0, y0, z0), v(x0, y0, z1), v(x0, y1, z1), v(x0, y1, z0)],
        [v(x1, y0, z1), v(x1, y0, z0), v(x1, y1, z0), v(x1, y1, z1)],
        [v(x0, y0, z0), v(x1, y0, z0), v(x1, y0, z1), v(x0, y0, z1)],
        [v(x0, y1, z1), v(x1, y1, z1), v(x1, y1, z0), v(x0, y1, z0)],
        [v(x1, y0, z0), v(x0, y0, z0), v(x0, y1, z0), v(x1, y1, z0)],
        [v(x0, y0, z1), v(x1, y0, z1), v(x1, y1, z1), v(x0, y1, z1)],
    ];
    for f in faces {
        let b = m.vertices.len() as u32;
        m.vertices.extend_from_slice(&f);
        m.indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
}

impl Game {
    fn is_falling_block(id: u8) -> bool {
        matches!(id, t::SAND | t::GRAVEL | t::ANVIL)
    }

    /// `FallingTile::isFree`
    fn fall_free(&self, x: i32, y: i32, z: i32) -> bool {
        if y < 0 {
            return true;
        }
        matches!(self.level.get_tile(x, y, z), 0 | t::FIRE | t::WATER | t::CALM_WATER | t::LAVA | t::CALM_LAVA | t::TALLGRASS | t::DEAD_BUSH | t::VINE | t::TOP_SNOW)
    }

    /// `HeavyTile::onPlace` / `neighborChanged`: look again at (x, y, z) in a few ticks.
    pub fn schedule_fall(&mut self, x: i32, y: i32, z: i32) {
        if !self.fall_checks.iter().any(|e| e.0 == (x, y, z)) {
            self.fall_checks.push(((x, y, z), 3));
        }
    }

    /// Run due checks and advance falling blocks; landed blocks turn back into tiles.
    fn tick_falling(&mut self) {
        let mut due = Vec::new();
        for e in self.fall_checks.iter_mut() {
            e.1 -= 1;
            if e.1 <= 0 {
                due.push(e.0);
            }
        }
        self.fall_checks.retain(|e| e.1 > 0);
        due.extend(std::mem::take(&mut self.level.fall_requests));
        for (x, y, z) in due {
            let id = self.level.get_tile(x, y, z);
            if !Self::is_falling_block(id) || y <= 0 || !self.fall_free(x, y - 1, z) {
                continue;
            }
            let data = self.level.get_data(x, y, z);
            self.level.set_tile(x, y, z, 0, UPDATE_ALL);
            self.mark(x, z);
            self.entities.falling.push(crate::entity::FallingBlock::new(x, y, z, id, data));
            // whatever stood on top is unsupported now
            self.schedule_fall(x, y + 1, z);
        }
        let mut list = std::mem::take(&mut self.entities.falling);
        let mut keep = Vec::new();
        for mut fb in list.drain(..) {
            fb.tick(&self.level);
            if fb.removed {
                continue;
            }
            if fb.landed {
                let (x, y, z) = (fb.body.x.floor() as i32, fb.body.y.floor() as i32, fb.body.z.floor() as i32);
                let free = matches!(self.level.get_tile(x, y, z), 0 | t::FIRE | t::TALLGRASS | t::DEAD_BUSH | t::VINE | t::TOP_SNOW | t::WATER | t::CALM_WATER);
                if free && (0..256).contains(&y) {
                    self.level.set_tile_and_data(x, y, z, fb.id, fb.data, UPDATE_ALL);
                    self.mark(x, z);
                    let snd = if fb.id == t::ANVIL { "random.anvil_land" } else { sound_type(fb.id).place };
                    let pit = self.random.next_float() * 0.1 + 0.9;
                    self.play(snd, x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5, 0.3, pit);
                } else {
                    for d in block_drops(fb.id, fb.data, &mut self.random, false, 0) {
                        let e = ItemEntity::pop(x, y, z, d, &mut self.random);
                        self.entities.items.push(e);
                    }
                }
                continue;
            }
            keep.push(fb);
        }
        self.entities.falling.extend(keep);
    }

    fn solid_at(&self, x: i32, y: i32, z: i32) -> bool {
        let id = self.level.get_tile(x, y, z);
        id != 0 && crate::blocks::style(id, self.level.get_data(x, y, z)).opaque
    }

    /// Height (0..1) within the block `at` where the targeted face was hit.
    fn click_y(&self, at: (i32, i32, i32)) -> f32 {
        match self.target {
            Some(h) => {
                let e = self.eye(0.0);
                let l = self.player.look_dir();
                (e[1] as f64 + l[1] * h.distance) as f32 - at.1 as f32
            }
            None => 0.0,
        }
    }

    /// `Mth::floor(yaw * 4 / 360 + 0.5) & 3`
    pub(crate) fn facing4(&self) -> u8 {
        (((self.player.yaw * 4.0 / 360.0) + 0.5).floor() as i32 & 3) as u8
    }

    fn click_sound(&mut self, name: &'static str, pos: (i32, i32, i32), vol: f32) {
        let pit = self.random.next_float() * 0.1 + 0.9;
        self.play(name, pos.0 as f64 + 0.5, pos.1 as f64 + 0.5, pos.2 as f64 + 0.5, vol, pit);
    }

    /// Right click on doors, trapdoors, fence gates, levers and buttons. Returns true if the block took the click.
    fn use_special_block(&mut self, pos: (i32, i32, i32)) -> bool {
        let (x, y, z) = pos;
        let id = self.level.get_tile(x, y, z);
        let data = self.level.get_data(x, y, z);
        match id {
            t::TNT if self.inv.selected_item().map_or(false, |s| s.id == 259) => {
                self.ignite_tnt(pos, Some(()));
                if !self.creative {
                    self.inv.damage_selected(1);
                }
                self.swing();
                return true;
            }
            t::DOOR_WOOD => {
                // `DoorTile::use`: only the lower half holds dir / open
                let ly = if data & 8 != 0 { y - 1 } else { y };
                let ld = self.level.get_data(x, ly, z);
                self.level.set_data(x, ly, z, ld ^ 4, UPDATE_ALL);
                self.mark(x, z);
                self.click_sound(if ld & 4 != 0 { "random.door_close" } else { "random.door_open" }, pos, 1.0);
            }
            t::TRAPDOOR | t::FENCE_GATE => {
                self.level.set_tile_and_data(x, y, z, id, data ^ 4, UPDATE_ALL);
                self.mark(x, z);
                self.click_sound(if data & 4 != 0 { "random.door_close" } else { "random.door_open" }, pos, 1.0);
            }
            t::LEVER => {
                self.level.set_tile_and_data(x, y, z, id, data ^ 8, UPDATE_ALL);
                self.mark(x, z);
                self.redstone.notify(pos);
                self.click_sound("random.click", pos, if data & 8 != 0 { 0.5 } else { 0.6 });
            }
            t::DIODE_OFF | t::DIODE_ON => {
                // cycle the delay (1..4 ticks)
                let nd = (data & 3) | ((((data >> 2) + 1) & 3) << 2);
                self.level.set_tile_and_data(x, y, z, id, nd, UPDATE_ALL);
                self.mark(x, z);
                self.redstone.notify(pos);
                self.click_sound("random.click", pos, 0.3);
            }
            t::BUTTON_STONE | t::BUTTON_WOOD => {
                if data & 8 == 0 {
                    self.level.set_tile_and_data(x, y, z, id, data | 8, UPDATE_ALL);
                    self.mark(x, z);
                    self.redstone.notify(pos);
                    self.click_sound("random.click", pos, 0.6);
                    self.pending_unpress.push((pos, if id == t::BUTTON_STONE { 20 } else { 30 }));
                }
            }
            _ => return false,
        }
        self.swing();
        true
    }

    /// Pressure plates, then the redstone simulation.
    fn tick_redstone(&mut self) {
        use mc_core::aabb::Aabb;
        // everything that can press a plate: (bounding box, is an item)
        let mut boxes: Vec<(Aabb, bool)> = vec![(self.player.bb, false)];
        for m in &self.mobs.list {
            if !m.removed {
                boxes.push((m.b.bb, false));
            }
        }
        for it in &self.entities.items {
            boxes.push((it.body.bb, true));
        }
        let mut pressed: std::collections::HashSet<(i32, i32, i32)> = Default::default();
        for (bb, is_item) in &boxes {
            for x in bb.x0.floor() as i32..=bb.x1.floor() as i32 {
                for z in bb.z0.floor() as i32..=bb.z1.floor() as i32 {
                    for y in (bb.y0 - 0.3).floor() as i32..=(bb.y0 + 0.1).floor() as i32 {
                        let id = self.level.get_tile(x, y, z);
                        if !matches!(id, t::PRESSURE_PLATE_STONE | t::PRESSURE_PLATE_WOOD) || (*is_item && id == t::PRESSURE_PLATE_STONE) {
                            continue;
                        }
                        let trig = Aabb::new(x as f64 + 0.125, y as f64, z as f64 + 0.125, x as f64 + 0.875, y as f64 + 0.25, z as f64 + 0.875);
                        if trig.intersects(bb) {
                            pressed.insert((x, y, z));
                        }
                    }
                }
            }
        }
        for &p in &pressed {
            let d = self.level.get_data(p.0, p.1, p.2);
            self.plate_timers.insert(p, 20);
            if d & 1 == 0 {
                let id = self.level.get_tile(p.0, p.1, p.2);
                self.level.set_tile_and_data(p.0, p.1, p.2, id, d | 1, UPDATE_ALL);
                self.mark(p.0, p.2);
                self.redstone.notify(p);
                self.play("random.click", p.0 as f64 + 0.5, p.1 as f64 + 0.1, p.2 as f64 + 0.5, 0.3, 0.6);
            }
        }
        let mut release = Vec::new();
        for (p, left) in self.plate_timers.iter_mut() {
            if !pressed.contains(p) {
                *left -= 1;
                if *left <= 0 {
                    release.push(*p);
                }
            }
        }
        for p in release {
            self.plate_timers.remove(&p);
            let id = self.level.get_tile(p.0, p.1, p.2);
            let d = self.level.get_data(p.0, p.1, p.2);
            if matches!(id, t::PRESSURE_PLATE_STONE | t::PRESSURE_PLATE_WOOD) && d & 1 != 0 {
                self.level.set_tile_and_data(p.0, p.1, p.2, id, d & !1, UPDATE_ALL);
                self.mark(p.0, p.2);
                self.redstone.notify(p);
                self.play("random.click", p.0 as f64 + 0.5, p.1 as f64 + 0.1, p.2 as f64 + 0.5, 0.3, 0.5);
            }
        }
        self.redstone.tick(&mut self.level);
        for ev in std::mem::take(&mut self.redstone.events) {
            match ev {
                crate::redstone::Event::Changed(p) => self.mark(p.0, p.2),
                crate::redstone::Event::Piston(p) => self.piston_check(p),
                crate::redstone::Event::Ignite(p) => self.ignite_tnt(p, None),
                crate::redstone::Event::Sound(name, p, vol, pit) => self.play(name, p.0 as f64 + 0.5, p.1 as f64 + 0.5, p.2 as f64 + 0.5, vol, pit),
            }
        }
    }

    /// Buttons pop back out after their delay.
    pub(crate) fn tick_buttons(&mut self) {
        let mut due = Vec::new();
        for e in self.pending_unpress.iter_mut() {
            e.1 -= 1;
            if e.1 <= 0 {
                due.push(e.0);
            }
        }
        self.pending_unpress.retain(|e| e.1 > 0);
        for pos in due {
            let id = self.level.get_tile(pos.0, pos.1, pos.2);
            if matches!(id, t::BUTTON_STONE | t::BUTTON_WOOD) {
                let d = self.level.get_data(pos.0, pos.1, pos.2);
                self.level.set_tile_and_data(pos.0, pos.1, pos.2, id, d & 7, UPDATE_ALL);
                self.mark(pos.0, pos.2);
                self.redstone.notify(pos);
                self.click_sound("random.click", pos, 0.3);
            }
        }
    }

    /// Block id + data a held block item produces when placed against `face` (None = can't be placed there).
    pub(crate) fn oriented(&self, id: u8, data: u8, face: usize, at: (i32, i32, i32)) -> Option<(u8, u8)> {
        use crate::shapes::is_stairs;
        let dir = self.facing4();
        let (x, y, z) = at;
        Some(match id {
            _ if is_stairs(id) => {
                let d = [2u8, 1, 3, 0][dir as usize];
                // `StairTile::setPlacedBy`: upside down when placed against a ceiling or on the upper half of a wall
                let upper = face != 1 && face != 0 && self.click_y(at) > 0.5;
                (id, d | if face == 0 || upper { 4 } else { 0 })
            }
            t::TRAPDOOR => {
                let d = match face {
                    4 => 0,
                    5 => 1,
                    0 => 2,
                    1 => 3,
                    _ => return None,
                };
                (id, d)
            }
            t::FENCE_GATE => (id, dir),
            t::TORCH | t::REDSTONE_TORCH_ON | t::REDSTONE_TORCH_OFF => {
                let attach = self.target.map(|h| h.pos).unwrap_or((x, y - 1, z));
                let d = match face {
                    1 => {
                        if !self.solid_at(x, y - 1, z) {
                            return None;
                        }
                        5
                    }
                    2 => return None,
                    f => {
                        if !self.solid_at(attach.0, attach.1, attach.2) {
                            return None;
                        }
                        match f {
                            4 => 4,
                            5 => 3,
                            0 => 2,
                            _ => 1,
                        }
                    }
                };
                (id, d)
            }
            t::TREE_TRUNK => {
                let axis = match face {
                    0 | 1 => 4,
                    4 | 5 => 8,
                    _ => 0,
                };
                (id, (data & 3) | axis)
            }
            t::PISTON_BASE | t::PISTON_STICKY_BASE => {
                let (px, pz) = (self.player.x, self.player.z);
                let py = self.player.feet_y() + 1.82;
                let mut f = [2u8, 5, 3, 4][dir as usize];
                if (px - (x as f64 + 0.5)).abs() < 2.0 && (pz - (z as f64 + 0.5)).abs() < 2.0 {
                    if py - y as f64 > 2.0 {
                        f = 1;
                    } else if y as f64 - py > 0.0 {
                        f = 0;
                    }
                }
                (id, f)
            }
            t::CHEST => (id, [2u8, 5, 3, 4][dir as usize]),
            t::FURNACE | t::DISPENSER | t::DROPPER => (id, [2u8, 5, 3, 4][dir as usize]),
            t::PUMPKIN | t::LIT_PUMPKIN => (id, (dir + 2) & 3),
            t::ANVIL => (id, ((dir + 1) & 3) | ((data & 3) << 2)),
            t::STONE_SLAB_HALF | t::WOOD_SLAB_HALF => {
                let upper = face == 0 || (face != 1 && self.click_y(at) > 0.5);
                (id, (data & 7) | if upper { 8 } else { 0 })
            }
            t::DIODE_OFF => {
                if !self.solid_at(x, y - 1, z) {
                    return None;
                }
                (id, dir)
            }
            t::PRESSURE_PLATE_STONE | t::PRESSURE_PLATE_WOOD | t::CAKE => {
                if !self.solid_at(x, y - 1, z) {
                    return None;
                }
                (id, 0)
            }
            t::LEVER | t::BUTTON_STONE | t::BUTTON_WOOD => {
                // `LeverTile::mayPlace` / `ButtonTile::mayPlace`: the block it sticks to must be a solid full cube
                const OFF: [(i32, i32, i32); 6] = [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)];
                let o = OFF[face ^ 1];
                if !crate::mesh::full_cube(&self.level, x + o.0, y + o.1, z + o.2) {
                    return None;
                }
                let d = match face {
                    4 => 4,
                    5 => 3,
                    0 => 2,
                    1 => 1,
                    3 if id == t::LEVER => 5,
                    2 if id == t::LEVER => 0,
                    _ => return None,
                };
                (id, d)
            }
            _ => (id, data),
        })
    }

    /// Items that place a special block: signs, doors, repeaters, cake. Returns whether something was placed.
    fn place_special_item(&mut self, item: u16) -> bool {
        use mc_world::item::ids;
        let Some(h) = self.target else { return false };
        let (x, y, z) = h.adjacent();
        if !(0..255).contains(&y) {
            return false;
        }
        let free = |g: &Game, x: i32, y: i32, z: i32| {
            let b = g.level.get_tile(x, y, z);
            b == 0 || matches!(b, t::TALLGRASS | t::DEAD_BUSH | t::TOP_SNOW | t::FIRE | t::VINE)
        };
        let (id, data, two): (u8, u8, bool) = match item {
            331 => {
                if !free(self, x, y, z) || !self.solid_at(x, y - 1, z) {
                    return false;
                }
                (t::RED_STONE_DUST, 0, false)
            }
            i if i == ids::DOOR_WOOD || i == ids::DOOR_IRON => {
                if h.face != 3 || !self.solid_at(x, y - 1, z) || !free(self, x, y, z) || !free(self, x, y + 1, z) {
                    return false;
                }
                // `DoorItem::useOn`: dir from the yaw, hinge side from the neighbours (`DoorItem::place`)
                let did = if i == ids::DOOR_WOOD { t::DOOR_WOOD } else { t::DOOR_IRON };
                let dir = ((self.player.yaw + 180.0) * 4.0 / 360.0 - 0.5).floor() as i32 & 3;
                let (xra, zra) = match dir {
                    0 => (0, 1),
                    1 => (-1, 0),
                    2 => (0, -1),
                    _ => (1, 0),
                };
                let solid = |g: &Game, dx: i32, dz: i32| g.solid_at(x + dx, y, z + dz) as i32 + g.solid_at(x + dx, y + 1, z + dz) as i32;
                let (sl, sr) = (solid(self, -xra, -zra), solid(self, xra, zra));
                let isd = |g: &Game, dx: i32, dz: i32| g.level.get_tile(x + dx, y, z + dz) == did || g.level.get_tile(x + dx, y + 1, z + dz) == did;
                let (dl, dr) = (isd(self, -xra, -zra), isd(self, xra, zra));
                let flip = (dl && !dr) || (!(dl && !dr) && sr > sl);
                (did, dir as u8 | if flip { 0x80 } else { 0 }, true)
            }
            i if i == ids::SIGN => {
                if !free(self, x, y, z) {
                    return false;
                }
                match h.face {
                    3 => {
                        let rot = ((((self.player.yaw + 180.0) * 16.0 / 360.0) + 0.5).floor() as i32 & 15) as u8;
                        (t::SIGN, rot, false)
                    }
                    2 => return false,
                    f => (t::WALL_SIGN, match f { 4 => 2, 5 => 3, 0 => 4, _ => 5 }, false),
                }
            }
            354 => {
                if h.face != 3 || !free(self, x, y, z) {
                    return false;
                }
                match self.oriented(t::CAKE, 0, 3, (x, y, z)) {
                    Some((i, d)) => (i, d, false),
                    None => return false,
                }
            }
            356 => {
                if h.face != 3 || !free(self, x, y, z) {
                    return false;
                }
                match self.oriented(t::DIODE_OFF, 0, 3, (x, y, z)) {
                    Some((i, d)) => (i, d, false),
                    None => return false,
                }
            }
            _ => return false,
        };
        if block_intersects_player(&self.player, (x, y, z)) && crate::shapes::is_door(id) {
            return false;
        }
        let flip = data & 0x80 != 0;
        let data = data & 0x7f;
        self.level.set_tile_and_data(x, y, z, id, data, UPDATE_ALL);
        if two {
            self.level.set_tile_and_data(x, y + 1, z, id, 8 | flip as u8, UPDATE_ALL);
        }
        self.mark(x, z);
        self.redstone.notify((x, y, z));
        self.swing();
        if id == t::SIGN || id == t::WALL_SIGN {
            // `SignItem::useOn`: a fresh sign opens its editor
            self.level.tile_entities.remove(&(x, y, z));
            self.level.tile_entity_mut(x, y, z);
            self.sign_pos = Some((x, y, z));
            self.mark(x, z);
            self.open_screen = Some(crate::screens::ScreenId::Sign);
        }
        let st = sound_type(id);
        self.play(st.place, x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5, (st.volume + 1.0) / 2.0, st.pitch * 0.8);
        true
    }

    /// Remove `pos` with its drops, no player involved (`Tile::dropResources` + `setTile(0)`).
    fn pop_block(&mut self, pos: (i32, i32, i32)) {
        let (x, y, z) = pos;
        let id = self.level.get_tile(x, y, z);
        if id == 0 {
            return;
        }
        let data = self.level.get_data(x, y, z);
        self.particles.destroy(&self.level, x, y, z, id, data);
        self.level.set_tile(x, y, z, 0, UPDATE_ALL);
        for d in block_drops(id, data, &mut self.random, false, 0) {
            let e = ItemEntity::pop(x, y, z, d, &mut self.random);
            self.entities.items.push(e);
        }
        self.mark(x, z);
        self.schedule_fall(x, y + 1, z);
        self.redstone.notify(pos);
    }

    /// After `(x, y, z)` was removed: knock off whatever was attached to it or standing on it.
    fn break_dependents(&mut self, removed: u8, removed_data: u8, pos: (i32, i32, i32)) {
        let (x, y, z) = pos;
        // the other half of a door
        if crate::shapes::is_door(removed) {
            let other = if removed_data & 8 != 0 { y - 1 } else { y + 1 };
            if self.level.get_tile(x, other, z) == removed {
                self.level.set_tile(x, other, z, 0, UPDATE_ALL);
                self.mark(x, z);
            }
        }
        // things that stand on top
        let above = (x, y + 1, z);
        let ab = self.level.get_tile(above.0, above.1, above.2);
        let ad = self.level.get_data(above.0, above.1, above.2);
        let stands = matches!(ab, t::PRESSURE_PLATE_STONE | t::PRESSURE_PLATE_WOOD | t::DIODE_OFF | t::DIODE_ON | t::CAKE | t::SIGN | t::RED_STONE_DUST)
            || (matches!(ab, t::TORCH | t::REDSTONE_TORCH_ON | t::REDSTONE_TORCH_OFF) && matches!(ad & 7, 0 | 5))
            || (crate::shapes::is_door(ab) && ad & 8 == 0)
            || (ab == t::LEVER && matches!(ad & 7, 5 | 6));
        if stands {
            self.pop_block(above);
        }
        // things hanging on a side
        for (dx, dy, dz, lever_data, wall_data) in [(-1, 0, 0, 2u8, 4u8), (1, 0, 0, 1, 5), (0, 0, -1, 4, 2), (0, 0, 1, 3, 3)] {
            let p = (x + dx, y + dy, z + dz);
            let b = self.level.get_tile(p.0, p.1, p.2);
            let d = self.level.get_data(p.0, p.1, p.2) & 7;
            let hangs = match b {
                t::LEVER | t::BUTTON_STONE | t::BUTTON_WOOD | t::TORCH | t::REDSTONE_TORCH_ON | t::REDSTONE_TORCH_OFF => d == lever_data,
                t::WALL_SIGN => d == wall_data,
                _ => false,
            };
            if hangs {
                self.pop_block(p);
            }
        }
        // ceiling levers
        let below = (x, y - 1, z);
        if self.level.get_tile(below.0, below.1, below.2) == t::LEVER && matches!(self.level.get_data(below.0, below.1, below.2) & 7, 0 | 7) {
            self.pop_block(below);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generate;

    fn flat_game() -> Game {
        let lv = generate(Dim::Flat, 1, 3);
        let mut g = Game::new(lv, Dim::Flat, [0.5, 4.0, 0.5]);
        for _ in 0..5 {
            g.tick(&Input::default());
        }
        g
    }

    fn survival_game() -> Game {
        let mut g = flat_game();
        g.creative = false;
        g.inv = Inventory::new();
        g.player.pitch = 90.0;
        g.update_target(1.0);
        g
    }

    #[test]
    fn survival_falling_hurts_and_flying_is_off() {
        let mut g = survival_game();
        g.player.flying = true;
        g.tick(&Input::default());
        assert!(!g.player.flying, "no flying in survival");
        let ground = g.player.y;
        g.player.set_pos(g.player.x, ground + 12.0, g.player.z);
        for _ in 0..80 {
            g.tick(&Input::default());
        }
        assert!(g.health < 20 && g.health >= 20 - 9 - 1, "health {}", g.health);
        // a creative player takes nothing
        let mut c = flat_game();
        let gy = c.player.y;
        c.player.set_pos(c.player.x, gy + 12.0, c.player.z);
        for _ in 0..80 {
            c.tick(&Input::default());
        }
        assert_eq!(c.health, 20);
    }

    /// Extend and retract a piston (normal or sticky) with a redstone torch beside it, pushing one block up.
    fn piston_cycle(base_id: u8) -> Vec<(u8, u8)> {
        let mut g = flat_game();
        let y = 10;
        for dx in -1..=1 {
            for dz in -1..=1 {
                for yy in y..y + 4 {
                    g.level.set_tile(8 + dx, yy, 8 + dz, 0, 0);
                }
                g.level.set_tile(8 + dx, y - 1, 8 + dz, t::STONE, 0);
            }
        }
        g.level.set_tile_and_data(8, y, 8, base_id, 1, 0);
        g.level.set_tile_and_data(8, y + 1, 8, t::WOOD, 0, 0);
        g.level.set_tile_and_data(9, y, 8, t::REDSTONE_TORCH_ON, 5, UPDATE_ALL);
        g.piston_check((8, y, 8));
        for _ in 0..10 {
            g.tick(&Input::default());
        }
        let ext: Vec<(u8, u8)> = (y..y + 4).map(|yy| (g.level.get_tile(8, yy, 8), g.level.get_data(8, yy, 8))).collect();
        assert!(g.moving.is_empty(), "pieces settled: {:?}", g.moving);
        assert_eq!(ext[0].0, base_id, "{ext:?}");
        assert_eq!(ext[0].1 & 8, 8, "{ext:?}");
        assert_eq!(ext[1].0, t::PISTON_EXTENSION_PIECE, "{ext:?}");
        assert_eq!(ext[2].0, t::WOOD, "{ext:?}");
        assert_eq!(ext[3].0, 0, "{ext:?}");
        // power off: the torch is removed
        g.level.set_tile(9, y, 8, 0, UPDATE_ALL);
        g.redstone.notify((8, y, 8));
        g.redstone.notify((9, y, 8));
        g.piston_check((8, y, 8));
        for _ in 0..10 {
            g.tick(&Input::default());
        }
        (y..y + 4).map(|yy| (g.level.get_tile(8, yy, 8), g.level.get_data(8, yy, 8))).collect()
    }

    #[test]
    fn piston_extends_pushes_one_block_and_settles() {
        piston_cycle(t::PISTON_BASE);
        piston_cycle(t::PISTON_STICKY_BASE);
    }

    /// The whole flow a player goes through: pick the item, place it, power it (a torch put down beside it).
    fn piston_by_hand(item: u8) -> Vec<(u8, u8)> {
        let mut g = flat_game();
        g.creative = true;
        let y = 4;
        for dx in 0..=3 {
            for yy in y..y + 3 {
                g.level.set_tile(3 + dx, yy, 0, 0, 0);
            }
        }
        g.player.flying = true;
        g.player.set_pos(3.5, 8.0, 0.5);
        g.player.pitch = 90.0;
        g.update_target(1.0);
        g.inv.items[0] = Some(ItemStack::block(item, 64));
        assert!(g.place_block(), "placed");
        let placed = (g.level.get_tile(3, y, 0), g.level.get_data(3, y, 0));
        assert_eq!(placed.0, item, "{placed:?}");
        let f = placed.1 & 7;
        // a torch on the stone beside the piston (not the pushing side)
        let (tx, tz) = match f {
            5 | 4 => (3, 1),
            _ => (4, 0),
        };
        g.level.set_tile_and_data(tx, y, tz, t::REDSTONE_TORCH_ON, 5, UPDATE_ALL);
        g.redstone.notify((tx, y, tz));
        g.redstone.notify((3, y, 0));
        for _ in 0..12 {
            g.tick(&Input::default());
        }
        vec![(g.level.get_tile(3, y, 0), g.level.get_data(3, y, 0))]
    }

    #[test]
    fn placed_pistons_work_for_both_kinds() {
        for item in [t::PISTON_BASE, t::PISTON_STICKY_BASE] {
            let r = piston_by_hand(item);
            assert_eq!(r[0].0, item);
            assert_eq!(r[0].1 & 8, 8, "item {item} did not extend: {r:?}");
        }
    }

    #[test]
    fn piston_retracts_and_sticky_pulls_back() {
        let normal = piston_cycle(t::PISTON_BASE);
        assert_eq!(normal[0], (t::PISTON_BASE, 1), "{normal:?}");
        assert_eq!(normal[1].0, 0, "head gone: {normal:?}");
        assert_eq!(normal[2].0, t::WOOD, "block stays pushed: {normal:?}");
        let sticky = piston_cycle(t::PISTON_STICKY_BASE);
        assert_eq!(sticky[0], (t::PISTON_STICKY_BASE, 1), "{sticky:?}");
        assert_eq!(sticky[1].0, t::WOOD, "block pulled back: {sticky:?}");
        assert_eq!(sticky[2].0, 0, "{sticky:?}");
    }

    #[test]
    fn survival_stone_needs_a_pickaxe_to_drop_cobblestone() {
        use mc_world::item::ids;
        let mut g = survival_game();
        // put a stone block under the feet
        g.level.set_tile(0, 3, 0, t::STONE, UPDATE_ALL);
        g.update_target(1.0);
        // bare hand: no drop
        assert!(g.destroy_block((0, 3, 0)));
        assert!(g.entities.items.is_empty());
        // wooden pickaxe: drops cobble, loses one point of durability
        g.level.set_tile(0, 3, 0, t::STONE, UPDATE_ALL);
        g.inv.items[0] = Some(ItemStack::new(ids::PICK_AXE_WOOD, 1));
        assert!(g.destroy_block((0, 3, 0)));
        assert_eq!(g.entities.items.len(), 1);
        assert_eq!(g.entities.items[0].item.id, t::COBBLESTONE as u16);
        assert_eq!(g.inv.items[0].unwrap().damage, 1);
    }

    #[test]
    fn tools_mine_faster_and_dropped_items_are_picked_up() {
        use mc_world::item::ids;
        let mut g = survival_game();
        let hand = g.destroy_progress_per_tick(t::STONE);
        g.inv.items[0] = Some(ItemStack::new(ids::PICK_AXE_IRON, 1));
        let iron = g.destroy_progress_per_tick(t::STONE);
        assert!(iron > hand * 10.0, "iron pickaxe {iron} vs hand {hand}");
        // a dirt block: drop it, walk into it, and it lands in the inventory
        g.level.set_tile(0, 3, 0, t::DIRT, UPDATE_ALL);
        g.inv.items[0] = None;
        g.destroy_block((0, 3, 0));
        assert_eq!(g.entities.items.len(), 1);
        for _ in 0..60 {
            g.tick(&Input::default());
        }
        assert!(g.entities.items.is_empty(), "picked up");
        assert_eq!(g.inv.count_of(t::DIRT as u16), 1);
    }

    #[test]
    fn placing_consumes_the_block_in_survival_only() {
        let mut g = survival_game();
        g.update_target(1.0);
        g.level.set_tile(0, 3, 0, 0, UPDATE_ALL);
        g.update_target(1.0);
        g.inv.items[0] = Some(ItemStack::block(t::GLASS, 2));
        assert!(g.place_block());
        assert_eq!(g.inv.items[0].unwrap().count, 1);
    }

    #[test]
    fn eating_takes_32_ticks_and_restores_food() {
        use mc_world::item::ids;
        let mut g = survival_game();
        g.food = 10;
        g.inv.items[0] = Some(ItemStack::new(ids::BREAD, 2));
        let act = Actions { use_held: true, ..Default::default() };
        let mut bits = false;
        for _ in 0..31 {
            g.tick_with(&Input::default(), &act);
            bits |= g.particles.particles.iter().any(|p| p.item.is_some());
        }
        assert!(bits, "bread crumbs fly while eating");
        assert_eq!(g.food, 10);
        g.tick_with(&Input::default(), &act);
        assert_eq!(g.food, 15); // bread = 5 nutrition
        assert_eq!(g.inv.items[0].unwrap().count, 1);
    }

    #[test]
    fn break_then_place_updates_the_level_and_marks_chunks() {
        let mut g = flat_game();
        g.player.pitch = 90.0; // straight down
        g.update_target(1.0);
        let h = g.target.expect("ground below");
        assert_eq!(h.pos, (0, 3, 0));
        assert!(g.break_block());
        assert_eq!(g.level.get_tile(0, 3, 0), 0);
        assert!(g.take_dirty().contains(&(0, 0)));
        // the hole now shows dirt at y = 2
        g.update_target(1.0);
        assert_eq!(g.target.unwrap().pos, (0, 2, 0));
        g.inv.selected = 5; // glass
        assert!(g.place_block());
        assert_eq!(g.level.get_tile(0, 3, 0), t::GLASS);
    }

    #[test]
    fn cannot_place_inside_the_player_or_break_bedrock() {
        let mut g = flat_game();
        g.player.pitch = 90.0;
        g.update_target(1.0);
        // placing on the ground we stand on would put a block at feet level -> inside the player
        assert!(!g.place_block());
        // dig down to bedrock
        for _ in 0..5 {
            g.update_target(1.0);
            g.break_block();
        }
        g.update_target(1.0);
        let h = g.target.unwrap();
        assert_eq!(g.level.get_tile(h.pos.0, h.pos.1, h.pos.2), t::UNBREAKABLE);
        assert!(!g.break_block());
    }

    #[test]
    fn selection_outline_has_12_boxes() {
        let mut g = flat_game();
        g.player.pitch = 90.0;
        g.update_target(1.0);
        let m = g.selection_mesh().unwrap();
        assert_eq!(m.indices.len(), 12 * 6 * 6);
    }

    fn hold(g: &mut Game, n: usize) -> usize {
        // press on the first tick, hold afterwards; returns the number of ticks until the block under the crosshair is gone
        let (x, y, z) = g.target.unwrap().pos;
        for i in 0..n {
            g.tick_with(&Input::default(), &Actions { attack_pressed: i == 0, attack_held: true, use_held: false });
            if g.level.get_tile(x, y, z) == 0 {
                return i + 1;
            }
        }
        n + 1
    }

    #[test]
    fn survival_mining_takes_as_long_as_the_original_formula_says() {
        // dirt: hardness 0.5, bare hand, material always destroyable -> (1 / 0.5) / 30 per tick -> 15 ticks
        let mut g = flat_game();
        g.creative = false;
        g.player.pitch = 90.0;
        g.update_target(1.0);
        let pos = g.target.unwrap().pos;
        let id = g.level.get_tile(pos.0, pos.1, pos.2);
        let per = g.destroy_progress_per_tick(id);
        let t = tile_info(id).unwrap();
        let expect_per = if t.material.map_or(true, Material::is_always_destroyable) { 1.0 / t.destroy_time / 30.0 } else { 1.0 / t.destroy_time / 100.0 };
        assert!((per - expect_per).abs() < 1e-7, "{per} vs {expect_per}");
        let ticks = hold(&mut g, 400);
        let want = (1.0 / per).ceil() as usize;
        assert!((ticks as i64 - want as i64).abs() <= 1, "took {ticks} ticks, formula says {want} (hardness {})", t.destroy_time);
        // cracks, a hit sound every 4 ticks, the break sound and the debris all happened
        assert!(g.particles.len() >= 64);
        let sounds = g.take_sounds();
        let breaks = sounds.iter().filter(|s| s.name.starts_with("dig.") || s.name == "random.glass").count();
        assert_eq!(breaks, 1, "{sounds:?}");
        let hits = sounds.iter().filter(|s| s.name.starts_with("step.")).count();
        assert_eq!(hits, (ticks + 3) / 4, "hit sounds every 4th tick: {sounds:?}");
        assert!(g.destroy_stage.is_none(), "the crack disappears with the block");
    }

    #[test]
    fn crack_stage_follows_progress() {
        let mut g = flat_game();
        g.creative = false;
        g.player.pitch = 90.0;
        g.update_target(1.0);
        let mut seen = Vec::new();
        for i in 0..8 {
            g.tick_with(&Input::default(), &Actions { attack_pressed: i == 0, attack_held: true, use_held: false });
            seen.push(g.destroy_stage.map(|s| s.1));
        }
        let stages: Vec<usize> = seen.iter().flatten().copied().collect();
        assert!(!stages.is_empty() && stages.windows(2).all(|w| w[0] <= w[1]), "{seen:?}");
        // releasing the button resets the progress and removes the crack
        g.tick_with(&Input::default(), &Actions::default());
        assert!(g.destroy_stage.is_none());
        assert_eq!(g.mining.progress, 0.0);
    }

    #[test]
    fn creative_breaks_every_five_ticks_while_held() {
        let mut g = flat_game();
        g.creative = true;
        g.player.pitch = 90.0;
        g.update_target(1.0);
        let mut broken_at = Vec::new();
        for i in 0..20 {
            let before = g.level.get_tile(0, 3, 0) + g.level.get_tile(0, 2, 0) + g.level.get_tile(0, 1, 0);
            g.tick_with(&Input::default(), &Actions { attack_pressed: i == 0, attack_held: true, use_held: false });
            let after = g.level.get_tile(0, 3, 0) + g.level.get_tile(0, 2, 0) + g.level.get_tile(0, 1, 0);
            if after != before {
                broken_at.push(i);
            }
        }
        assert!(broken_at.len() >= 3 && broken_at[1] - broken_at[0] >= 5, "{broken_at:?}");
    }

    #[test]
    fn holding_use_places_blocks_with_a_sound_and_auto_repeats() {
        let mut g = flat_game();
        g.player.pitch = 90.0;
        g.update_target(1.0);
        assert!(g.break_block()); // dig a hole under our feet, then fill it with glass
        g.take_sounds();
        g.update_target(1.0);
        g.inv.selected = 5; // glass
        g.tick_with(&Input::default(), &Actions { use_held: true, ..Default::default() });
        let sounds = g.take_sounds();
        assert!(sounds.iter().any(|s| s.name == "step.stone" && (s.volume - 1.0).abs() < 1e-6 && (s.pitch - 0.8).abs() < 1e-6), "glass is placed with step.stone at (1+1)/2 volume, pitch 0.8: {sounds:?}");
        assert_eq!(g.level.get_tile(0, 3, 0), t::GLASS);
    }

    #[test]
    fn walking_makes_footsteps_of_the_block_underfoot() {
        let mut g = flat_game();
        g.take_sounds();
        let input = Input { forward: 1.0, ..Default::default() };
        for _ in 0..60 {
            g.tick(&input);
        }
        let s = g.take_sounds();
        assert!(s.len() >= 4, "{s:?}");
        let ground = g.level.get_tile(g.player.x.floor() as i32, (g.player.y - 0.2 - 1.62).floor() as i32, g.player.z.floor() as i32);
        let want = crate::soundtypes::sound_type(ground);
        for e in &s {
            assert_eq!(Some(e.name), want.step, "footstep on block {ground}");
            assert!((e.volume - want.volume * 0.15).abs() < 1e-6);
        }
        // sneaking on the ground is silent
        let sneak = Input { forward: 1.0, sneak: true, ..Default::default() };
        for _ in 0..60 {
            g.tick(&sneak);
        }
        assert!(g.take_sounds().is_empty());
    }

    #[test]
    fn hotbar_scrolls_and_wraps() {
        let mut g = flat_game();
        g.scroll(-1);
        assert_eq!(g.inv.selected, 1);
        g.select(0);
        g.scroll(1);
        assert_eq!(g.inv.selected, 8);
    }

    #[test]
    fn sneaking_lowers_the_eye_and_standing_up_restores_it() {
        let mut g = flat_game();
        let stand = g.player.y;
        let sneak = Input { sneak: true, ..Default::default() };
        for _ in 0..4 {
            g.tick(&sneak);
        }
        // original dip 0.08 + extra smoothed drop 0.27
        let low = g.eye(1.0)[1] as f64;
        assert!((stand - low - 0.35).abs() < 0.02, "eye {low} vs {stand}");
        for _ in 0..12 {
            g.tick(&Input::default());
        }
        assert!((stand - g.eye(1.0)[1] as f64).abs() < 0.01);
    }

    #[test]
    fn sprinting_widens_fov_bobs_the_view_and_kicks_up_dust() {
        let mut g = flat_game();
        let walk = Input { forward: 1.0, ..Default::default() };
        for _ in 0..20 {
            g.tick(&walk);
        }
        assert!((g.player.fov_degrees(1.0) - 70.0).abs() < 0.5);
        assert!(g.player.bob > 0.05, "walking bobs: {}", g.player.bob);
        let run = Input { forward: 1.0, sprint: true, ..Default::default() };
        let before = g.particles.len();
        for _ in 0..30 {
            g.tick(&run);
        }
        assert!(g.player.fov_degrees(1.0) > 79.0, "fov {}", g.player.fov_degrees(1.0)); // 70 * 1.15
        assert!(g.particles.len() > before, "sprinting spawns tile-crack particles");
    }

    #[test]
    fn sand_falls_and_lands_on_the_ground() {
        let mut g = flat_game();
        let top = g.level.get_top_solid_block(8, 8);
        let y = top + 6;
        g.level.set_tile(8, y, 8, t::SAND, 0);
        g.schedule_fall(8, y, 8);
        for _ in 0..80 {
            g.tick(&Input::default());
        }
        assert_eq!(g.level.get_tile(8, y, 8), 0, "left its spot");
        let landed = (top..y).find(|&yy| g.level.get_tile(8, yy, 8) == t::SAND);
        assert!(landed.is_some(), "sand landed somewhere below");
        assert!(g.entities.falling.is_empty());
    }

    #[test]
    fn breaking_the_block_under_gravel_drops_it() {
        let mut g = flat_game();
        let top = g.level.get_top_solid_block(10, 10);
        g.level.set_tile(10, top + 1, 10, t::GRAVEL, 0);
        g.level.set_tile(10, top + 2, 10, t::GRAVEL, 0);
        g.destroy_block((10, top, 10));
        for _ in 0..80 {
            g.tick(&Input::default());
        }
        assert_eq!(g.level.get_tile(10, top, 10), t::GRAVEL);
        assert_eq!(g.level.get_tile(10, top + 1, 10), t::GRAVEL);
        assert_eq!(g.level.get_tile(10, top + 2, 10), 0);
    }

    #[test]
    fn stepping_on_a_plate_powers_dust_and_lights_a_lamp() {
        let mut g = flat_game();
        let y = g.level.get_top_solid_block(0, 0);
        let (px, pz) = (g.player.x.floor() as i32, g.player.z.floor() as i32);
        assert_eq!((px, pz), (0, 0));
        g.level.set_tile_and_data(1, y, 0, t::RED_STONE_DUST, 0, UPDATE_ALL);
        g.level.set_tile_and_data(2, y, 0, t::RED_STONE_DUST, 0, UPDATE_ALL);
        g.level.set_tile_and_data(3, y, 0, t::REDSTONE_LIGHT, 0, UPDATE_ALL);
        g.level.set_tile_and_data(0, y, 0, t::PRESSURE_PLATE_WOOD, 0, UPDATE_ALL);
        g.redstone.notify((0, y, 0));
        g.player.y = y as f64 + crate::physics::EYE_HEIGHT;
        for _ in 0..12 {
            g.tick(&Input::default());
        }
        assert_eq!(g.level.get_data(0, y, 0) & 1, 1, "plate pressed");
        assert_eq!(g.level.get_data(1, y, 0) & 15, 15);
        assert_eq!(g.level.get_tile(3, y, 0), t::REDSTONE_LIGHT_LIT);
        // walk away: the plate releases after a second
        g.player.set_pos(6.5, y as f64 + crate::physics::EYE_HEIGHT, 6.5);
        for _ in 0..40 {
            g.tick(&Input::default());
        }
        assert_eq!(g.level.get_data(0, y, 0) & 1, 0, "plate released");
        assert_eq!(g.level.get_tile(3, y, 0), t::REDSTONE_LIGHT);
    }

    #[test]
    fn water_spreads_over_time_and_sand_falls() {
        let mut g = flat_game();
        let (x, z) = (6, 6);
        let y = g.level.get_top_solid_block(x, z);
        g.level.set_tile_and_update(x, y, z, t::WATER);
        for _ in 0..200 {
            g.tick(&Input::default());
        }
        let wet = (-4..=4).flat_map(|dx| (-4..=4).map(move |dz| (dx, dz))).filter(|&(dx, dz)| matches!(g.level.get_tile(x + dx, y, z + dz), t::WATER | t::CALM_WATER)).count();
        assert!(wet > 8, "water spread over {wet} blocks");
        assert!(g.take_dirty().contains(&(0, 0)), "changed chunks are flagged");
        // sand placed in mid air drops to the floor
        let top = g.level.get_top_solid_block(2, 2);
        g.level.set_tile_and_update(2, top + 3, 2, t::SAND);
        for _ in 0..40 {
            g.tick(&Input::default());
        }
        assert_eq!(g.level.get_tile(2, top + 3, 2), 0);
        assert_eq!(g.level.get_tile(2, top, 2), t::SAND);
    }
}
