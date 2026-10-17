//! Mobs: `Mob` / `PathfinderMob` / `Animal` / `Monster` and the seven species (pig, cow, sheep, chicken, zombie,
//! skeleton, creeper) with the TU19 "new AI" (`GoalSelector`, `MoveControl`, `LookControl`, `JumpControl`,
//! `BodyControl`, `PathNavigation`, `Sensing`).

use crate::entity::Body;
use crate::path::{Path, PathEnt, PathFinder};
use crate::physics::friction_of;
use mc_core::aabb::Aabb;
use mc_core::random::Random;
use mc_level::level::Level;
use mc_world::tile_ids as t;

pub const PI: f32 = std::f32::consts::PI;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MobKind {
    Pig,
    Cow,
    Sheep,
    Chicken,
    Zombie,
    Skeleton,
    Creeper,
    Spider,
    CaveSpider,
    Slime,
    LavaSlime,
}

impl MobKind {
    /// `useNewAi()` is false: the mob runs `PathfinderMob::serverAiStep`
    pub fn old_ai(self) -> bool {
        matches!(self, MobKind::Spider | MobKind::CaveSpider)
    }
    /// `Slime` / `LavaSlime` (a `Mob`, not a `PathfinderMob`: its own `serverAiStep`)
    pub fn is_slime(self) -> bool {
        matches!(self, MobKind::Slime | MobKind::LavaSlime)
    }
    pub fn is_spider(self) -> bool {
        matches!(self, MobKind::Spider | MobKind::CaveSpider)
    }
    pub fn is_animal(self) -> bool {
        matches!(self, MobKind::Pig | MobKind::Cow | MobKind::Sheep | MobKind::Chicken)
    }
    pub fn size(self) -> (f32, f32) {
        match self {
            MobKind::Pig => (0.9, 0.9),
            MobKind::Cow | MobKind::Sheep => (0.9, 1.3),
            MobKind::Chicken => (0.3, 0.7),
            MobKind::Spider => (1.4, 0.9),
            MobKind::CaveSpider => (0.7, 0.5),
            // `Slime::setSize(0.6 * size, 0.6 * size)` with size 1 (the real size is applied in `set_slime_size`)
            MobKind::Slime | MobKind::LavaSlime => (0.6, 0.6),
            _ => (0.6, 1.8),
        }
    }
    pub fn max_health(self) -> f32 {
        match self {
            MobKind::Pig | MobKind::Cow => 10.0,
            MobKind::Sheep => 8.0,
            MobKind::Chicken => 4.0,
            MobKind::Spider => 16.0,
            MobKind::CaveSpider => 12.0,
            MobKind::Slime | MobKind::LavaSlime => 1.0,
            _ => 20.0,
        }
    }
    pub fn base_speed(self) -> f32 {
        match self {
            MobKind::Pig | MobKind::Chicken | MobKind::Skeleton | MobKind::Creeper => 0.25,
            MobKind::Cow => 0.2,
            MobKind::Sheep | MobKind::Zombie => 0.23,
            // MOVEMENT_SPEED attribute (read for `yya`; the travel speed of an old-AI mob is `getSpeed()` = 0.1)
            MobKind::Spider | MobKind::CaveSpider => 0.8,
            MobKind::Slime => 0.7,
            MobKind::LavaSlime => 0.2,
        }
    }
    pub fn follow_range(self) -> f32 {
        if self == MobKind::Zombie {
            40.0
        } else {
            16.0
        }
    }
    pub fn food(self) -> &'static [u16] {
        match self {
            MobKind::Pig => &[ids::CARROT],
            MobKind::Cow | MobKind::Sheep => &[ids::WHEAT],
            MobKind::Chicken => &[ids::SEEDS_WHEAT, ids::NETHERWART_SEEDS, ids::SEEDS_MELON, ids::SEEDS_PUMPKIN],
            _ => &[],
        }
    }
    fn ambient(self) -> Option<&'static str> {
        Some(match self {
            MobKind::Pig | MobKind::Cow | MobKind::Sheep | MobKind::Chicken => match self {
                MobKind::Pig => "mob.pig.say",
                MobKind::Cow => "mob.cow.say",
                MobKind::Sheep => "mob.sheep.say",
                _ => "mob.chicken.say",
            },
            MobKind::Zombie => "mob.zombie.say",
            MobKind::Skeleton => "mob.skeleton",
            MobKind::Spider | MobKind::CaveSpider => "mob.spider.say",
            MobKind::Creeper | MobKind::Slime | MobKind::LavaSlime => return None,
        })
    }
    fn hurt_sound(self) -> &'static str {
        match self {
            MobKind::Pig => "mob.pig.say",
            MobKind::Cow => "mob.cow.hurt",
            MobKind::Sheep => "mob.sheep.say",
            MobKind::Chicken => "mob.chicken.hurt",
            MobKind::Zombie => "mob.zombie.hurt",
            MobKind::Skeleton => "mob.skeletonhurt",
            MobKind::Creeper => "mob.creeper.say",
            MobKind::Spider | MobKind::CaveSpider => "mob.spider.say",
            MobKind::Slime => "mob.slime.small",
            MobKind::LavaSlime => "mob.magmacube.small",
        }
    }
    fn death_sound(self) -> &'static str {
        match self {
            MobKind::Pig => "mob.pig.death",
            MobKind::Cow => "mob.cow.hurt",
            MobKind::Sheep => "mob.sheep.say",
            MobKind::Chicken => "mob.chicken.hurt",
            MobKind::Zombie => "mob.zombie.death",
            MobKind::Skeleton => "mob.skeleton.death",
            MobKind::Creeper => "mob.creeper.death",
            MobKind::Spider | MobKind::CaveSpider => "mob.spider.death",
            MobKind::Slime => "mob.slime.small",
            MobKind::LavaSlime => "mob.magmacube.small",
        }
    }
    fn step_sound(self) -> &'static str {
        match self {
            MobKind::Pig => "mob.pig.step",
            MobKind::Cow => "mob.cow.step",
            MobKind::Sheep => "mob.sheep.step",
            MobKind::Chicken => "mob.chicken.step",
            MobKind::Zombie => "mob.zombie.step",
            MobKind::Skeleton => "mob.skeleton.step",
            MobKind::Creeper => "",
            MobKind::Spider | MobKind::CaveSpider => "mob.spider.step",
            MobKind::Slime | MobKind::LavaSlime => "",
        }
    }
    fn sound_volume(self) -> f32 {
        if self == MobKind::Cow {
            0.4
        } else {
            1.0
        }
    }
}

pub mod ids {
    pub const WOOL: u16 = 35;
    pub const BOW: u16 = 261;
    pub const ARROW: u16 = 262;
    pub const IRON_INGOT: u16 = 265;
    pub const STRING: u16 = 287;
    pub const SPIDER_EYE: u16 = 375;
    pub const SLIMEBALL: u16 = 341;
    pub const MAGMA_CREAM: u16 = 378;
    pub const SEEDS_WHEAT: u16 = 295;
    pub const WHEAT: u16 = 296;
    pub const FEATHER: u16 = 288;
    pub const GUNPOWDER: u16 = 289;
    pub const PORK_RAW: u16 = 319;
    pub const PORK_COOKED: u16 = 320;
    pub const SADDLE: u16 = 329;
    pub const LEATHER: u16 = 334;
    pub const EGG: u16 = 344;
    pub const BONE: u16 = 352;
    pub const BEEF_RAW: u16 = 363;
    pub const BEEF_COOKED: u16 = 364;
    pub const CHICKEN_RAW: u16 = 365;
    pub const CHICKEN_COOKED: u16 = 366;
    pub const ROTTEN_FLESH: u16 = 367;
    pub const NETHERWART_SEEDS: u16 = 372;
    pub const SEEDS_PUMPKIN: u16 = 361;
    pub const SEEDS_MELON: u16 = 362;
    pub const CARROT: u16 = 391;
    pub const POTATO: u16 = 392;
}

// ------------------------------------------------------------------------------------------------ context

#[derive(Clone, Copy, Debug)]
pub struct PlayerInfo {
    pub x: f64,
    /// eye position (`Player::y`)
    pub y: f64,
    pub z: f64,
    pub bb: Aabb,
    pub alive: bool,
    pub invulnerable: bool,
    pub holding: Option<u16>,
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct OtherInfo {
    pub id: u32,
    pub kind: MobKind,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub bb: Aabb,
    pub age: i32,
    pub love: i32,
    pub alive: bool,
}

#[derive(Clone, Debug)]
pub enum Ev {
    Sound { name: &'static str, x: f64, y: f64, z: f64, vol: f32, pitch: f32 },
    Item { id: u16, damage: u16, count: u8, x: f64, y: f64, z: f64 },
    HurtPlayer { dmg: f32, x: f64, z: f64 },
    /// experience dropped by a mob the player killed
    Xp { amount: i32, x: f64, y: f64, z: f64 },
    Explode { x: f64, y: f64, z: f64, radius: f32 },
    Breed { a: u32, b: u32 },
    Block { x: i32, y: i32, z: i32, id: u8, data: u8 },
    Hearts { x: f64, y: f64, z: f64, w: f32, h: f32, n: u32 },
    Puff { x: f64, y: f64, z: f64, w: f32, h: f32, n: u32 },
    Alert { kind: MobKind, x: f64, y: f64, z: f64, range: f64 },
    /// `addEffect` on the player
    Effect { id: u8, duration: i32, amplifier: i32 },
    Arrow { from: [f64; 3], to: [f64; 3], power: f32, uncertainty: f32, damage: f64, owner: u32 },
    /// `Slime::remove`: a smaller slime of the same kind
    SpawnSlime { lava: bool, size: i32, x: f64, y: f64, z: f64, yaw: f32 },
    /// `Slime::tick` landing particles (`eParticleType_slime` / `eParticleType_flame`)
    Particle { lava: bool, x: f64, y: f64, z: f64 },
    /// `Slime::playerTouch`: `player->hurt`, and the attack sound when the hit landed
    TouchHurt { dmg: f32, x: f64, z: f64, sound: &'static str },
}

pub struct Ctx<'a> {
    pub lv: &'a Level,
    pub player: PlayerInfo,
    pub others: &'a [OtherInfo],
    pub day: bool,
    pub sky_dampen: i32,
    pub ramp: [f32; 16],
    pub difficulty: i32,
    pub ev: &'a mut Vec<Ev>,
}

impl Ctx<'_> {
    pub fn raw_brightness(&self, x: i32, y: i32, z: i32) -> i32 {
        if y < 0 {
            return 0;
        }
        let y = y.min(127);
        self.lv.get_chunk(x >> 4, z >> 4).get_raw_brightness(self.lv.has_ceiling, x & 15, y, z & 15, self.sky_dampen)
    }
    pub fn brightness(&self, x: i32, y: i32, z: i32) -> f32 {
        self.ramp[self.raw_brightness(x, y, z).clamp(0, 15) as usize]
    }
}

pub fn wrap_degrees(a: f32) -> f32 {
    let mut r = a % 360.0;
    if r >= 180.0 {
        r -= 360.0;
    }
    if r < -180.0 {
        r += 360.0;
    }
    r
}

fn rotlerp(a: f32, b: f32, max: f32) -> f32 {
    let mut d = b - a;
    while d < -180.0 {
        d += 360.0;
    }
    while d >= 180.0 {
        d -= 360.0;
    }
    a + d.clamp(-max, max)
}

/// `Level::clip(from, to) == null` — is the line free of solid blocks?
pub fn line_of_sight(lv: &Level, a: [f64; 3], b: [f64; 3]) -> bool {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let steps = (len / 0.2).ceil().max(1.0) as i32;
    for i in 1..steps {
        let f = i as f64 / steps as f64;
        let (x, y, z) = ((a[0] + d[0] * f).floor() as i32, (a[1] + d[1] * f).floor() as i32, (a[2] + d[2] * f).floor() as i32);
        let id = lv.get_tile(x, y, z);
        if id != 0 {
            let s = crate::blocks::style(id, lv.get_data(x, y, z));
            if matches!(s.kind, crate::blocks::Kind::Cube | crate::blocks::Kind::Box { .. }) && s.opaque {
                return false;
            }
        }
    }
    true
}

// ------------------------------------------------------------------------------------------------ goals

pub const F_MOVE: u8 = 1;
pub const F_LOOK: u8 = 2;
pub const F_JUMP: u8 = 4;

#[derive(Clone, Debug)]
pub enum G {
    Float,
    Panic { speed: f64, pos: [f64; 3] },
    RandomStroll { speed: f64, pos: [f64; 3] },
    LookAtPlayer { dist: f32, look_time: i32 },
    RandomLookAround { rel: [f64; 2], look_time: i32 },
    Melee { speed: f64, track: bool, attack_time: i32, recalc: i32, path: Option<Path> },
    Breed { speed: f64, partner: Option<u32>, love_time: i32 },
    Tempt { speed: f64, items: &'static [u16], calm: i32, running: bool, old_avoid: bool },
    FollowParent { speed: f64, parent: Option<u32>, recalc: i32 },
    EatTile { tick: i32 },
    Swell,
    RestrictSun,
    FleeSun { speed: f64, pos: [f64; 3] },
    RangedAttack { speed: f64, min: i32, max: i32, radius: f32, attack_time: i32, see_time: i32 },
    // target goals
    HurtBy { alert: bool, timestamp: i32 },
    Nearest { must_see: bool, unseen: i32 },
}

#[derive(Clone, Debug)]
pub struct Slot {
    pub prio: i32,
    pub flags: u8,
    pub running: bool,
    pub g: G,
}

fn flags_of(g: &G) -> u8 {
    match g {
        G::Float => F_JUMP,
        G::Panic { .. } | G::Swell | G::FleeSun { .. } => F_MOVE,
        G::LookAtPlayer { .. } => F_LOOK,
        G::EatTile { .. } => F_MOVE | F_LOOK | F_JUMP,
        G::FollowParent { .. } | G::RestrictSun => 0,
        G::HurtBy { .. } | G::Nearest { .. } => 1,
        _ => F_MOVE | F_LOOK,
    }
}

// ------------------------------------------------------------------------------------------------ navigation

#[derive(Clone, Debug, Default)]
pub struct Nav {
    pub path: Option<Path>,
    pub speed_mod: f64,
    pub tick: i32,
    pub last_stuck_check: i32,
    pub last_stuck_pos: [f64; 3],
    pub avoid_water: bool,
    pub can_float: bool,
    pub can_open_doors: bool,
    pub avoid_sun: bool,
}

impl Nav {
    pub fn is_done(&self) -> bool {
        self.path.as_ref().map_or(true, |p| p.is_done())
    }
    pub fn stop(&mut self) {
        self.path = None;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Src {
    Player { x: f64, z: f64 },
    Arrow { x: f64, z: f64, player: bool },
    Mob,
    Fall,
    Fire,
    Lava,
    Drown,
    InWall,
    Void,
    Explosion,
}

#[derive(Clone)]
pub struct Mob {
    pub id: u32,
    pub kind: MobKind,
    pub b: Body,
    pub rng: Random,
    pub removed: bool,
    pub health: f32,
    pub hurt_time: i32,
    pub hurt_duration: i32,
    pub invulnerable_time: i32,
    pub last_hurt: f32,
    pub death_time: i32,
    pub attack_time: i32,
    pub walk_anim_speed: f32,
    pub walk_anim_speed_o: f32,
    pub walk_anim_pos: f32,
    pub y_body_rot: f32,
    pub y_body_rot_o: f32,
    pub y_head_rot: f32,
    pub y_head_rot_o: f32,
    pub y_rot_o: f32,
    pub x_rot_o: f32,
    pub xxa: f32,
    pub yya: f32,
    pub jumping: bool,
    pub no_jump_delay: i32,
    pub speed: f32,
    pub fall_distance: f32,
    pub tick_count: i32,
    pub no_action_time: i32,
    pub fire: i32,
    pub air_supply: i32,
    pub ambient_time: i32,
    pub persistent: bool,
    pub move_dist: f32,
    pub next_step: i32,
    // AI
    pub goals: Vec<Slot>,
    pub target_goals: Vec<Slot>,
    pub goal_tick: i32,
    pub target_tick: i32,
    pub nav: Nav,
    pub target: bool,
    pub last_hurt_by_player: bool,
    pub last_hurt_by_ts: i32,
    pub last_hurt_by_player_time: i32,
    // move / look / jump / body control
    pub mv_wanted: Option<([f64; 3], f64)>,
    pub look_wanted: Option<([f64; 3], f32, f32)>,
    pub jump_flag: bool,
    pub body_time_still: i32,
    pub body_last_head: f32,
    pub sense_seen: Option<bool>,
    // species data
    pub age: i32,
    pub love: i32,
    pub sheep_color: u8,
    pub sheared: bool,
    pub eat_anim: i32,
    pub flap: f32,
    pub o_flap: f32,
    pub flap_speed: f32,
    pub o_flap_speed: f32,
    pub flapping: f32,
    pub egg_time: i32,
    pub swell: i32,
    pub old_swell: i32,
    pub swell_dir: i32,
    pub max_swell: i32,
    pub explosion_radius: f32,
    pub powered: bool,
    // old AI (`PathfinderMob`)
    /// `attackTarget != null` (the only targets are players)
    pub old_target: bool,
    /// `Mob::lookingAt` the nearest player, and `lookTime`
    pub looking_at: bool,
    pub look_time: i32,
    pub y_rot_a: f32,
    /// `Spider::isClimbing` (synched flag 0)
    pub climbing: bool,
    // slimes
    /// `Slime::getSize` (1, 2, 4)
    pub size: i32,
    pub squish: f32,
    pub o_squish: f32,
    pub target_squish: f32,
    pub jump_delay: i32,
}

impl Mob {
    pub fn new(kind: MobKind, id: u32, x: f64, y: f64, z: f64, seed: i64) -> Mob {
        let (w, h) = kind.size();
        let mut b = Body::new(w as f64, h as f64, 0.0);
        b.foot_size = 0.5;
        b.set_pos(x, y, z);
        b.xo = x;
        b.yo = y;
        b.zo = z;
        let mut rng = Random::with_seed(seed);
        let egg_time = if kind == MobKind::Chicken { rng.next_int_bounded(20 * 60 * 5) + 20 * 60 * 5 } else { 0 };
        let mut m = Mob {
            id,
            kind,
            b,
            rng,
            removed: false,
            health: kind.max_health(),
            hurt_time: 0,
            hurt_duration: 0,
            invulnerable_time: 0,
            last_hurt: 0.0,
            death_time: 0,
            attack_time: 0,
            walk_anim_speed: 0.0,
            walk_anim_speed_o: 0.0,
            walk_anim_pos: 0.0,
            y_body_rot: 0.0,
            y_body_rot_o: 0.0,
            y_head_rot: 0.0,
            y_head_rot_o: 0.0,
            y_rot_o: 0.0,
            x_rot_o: 0.0,
            xxa: 0.0,
            yya: 0.0,
            jumping: false,
            no_jump_delay: 0,
            speed: 0.0,
            fall_distance: 0.0,
            tick_count: 0,
            no_action_time: 0,
            fire: 0,
            air_supply: 300,
            ambient_time: 0,
            persistent: kind.is_animal(),
            move_dist: 0.0,
            next_step: 1,
            goals: Vec::new(),
            target_goals: Vec::new(),
            goal_tick: 0,
            target_tick: 0,
            nav: Nav::default(),
            target: false,
            last_hurt_by_player: false,
            last_hurt_by_ts: 0,
            last_hurt_by_player_time: 0,
            mv_wanted: None,
            look_wanted: None,
            jump_flag: false,
            body_time_still: 0,
            body_last_head: 0.0,
            sense_seen: None,
            age: 0,
            love: 0,
            sheep_color: 0,
            sheared: false,
            eat_anim: 0,
            flap: 0.0,
            o_flap: 0.0,
            flap_speed: 0.0,
            o_flap_speed: 0.0,
            flapping: 1.0,
            egg_time,
            swell: 0,
            old_swell: 0,
            swell_dir: -1,
            max_swell: 30,
            explosion_radius: 3.0,
            powered: false,
            old_target: false,
            looking_at: false,
            look_time: 0,
            y_rot_a: 0.0,
            climbing: false,
            size: 1,
            squish: 0.0,
            o_squish: 0.0,
            target_squish: 0.0,
            jump_delay: 0,
        };
        if kind.is_slime() {
            // `Slime::Slime`
            let size = 1 << m.rng.next_int_bounded(3);
            m.jump_delay = m.rng.next_int_bounded(20) + 10;
            m.set_slime_size(size);
        }
        if kind.old_ai() || kind.is_slime() {
            // `LivingEntity::registerAttributes` + `getSpeed`
            m.speed = 0.1;
        }
        m.setup_goals();
        m
    }

    fn add_goal(&mut self, prio: i32, g: G) {
        let flags = flags_of(&g);
        self.goals.push(Slot { prio, flags, running: false, g });
    }
    fn add_target(&mut self, prio: i32, g: G) {
        let flags = flags_of(&g);
        self.target_goals.push(Slot { prio, flags, running: false, g });
    }

    fn setup_goals(&mut self) {
        use MobKind::*;
        let look = |d: f32| G::LookAtPlayer { dist: d, look_time: 0 };
        let stroll = |s: f64| G::RandomStroll { speed: s, pos: [0.0; 3] };
        let panic = |s: f64| G::Panic { speed: s, pos: [0.0; 3] };
        let breed = G::Breed { speed: 1.0, partner: None, love_time: 0 };
        let tempt = |s: f64, items: &'static [u16]| G::Tempt { speed: s, items, calm: 0, running: false, old_avoid: false };
        let follow = |s: f64| G::FollowParent { speed: s, parent: None, recalc: 0 };
        let melee = |s: f64, track: bool| G::Melee { speed: s, track, attack_time: 0, recalc: 0, path: None };
        if self.kind.is_animal() {
            self.nav.avoid_water = self.kind != Chicken;
        }
        match self.kind {
            Pig => {
                self.add_goal(0, G::Float);
                self.add_goal(1, panic(1.25));
                self.add_goal(3, breed);
                self.add_goal(4, tempt(1.2, &[ids::CARROT]));
                self.add_goal(5, follow(1.1));
                self.add_goal(6, stroll(1.0));
                self.add_goal(7, look(6.0));
                self.add_goal(8, G::RandomLookAround { rel: [0.0; 2], look_time: 0 });
            }
            Cow => {
                self.add_goal(0, G::Float);
                self.add_goal(1, panic(2.0));
                self.add_goal(2, breed);
                self.add_goal(3, tempt(1.25, &[ids::WHEAT]));
                self.add_goal(4, follow(1.25));
                self.add_goal(5, stroll(1.0));
                self.add_goal(6, look(6.0));
                self.add_goal(7, G::RandomLookAround { rel: [0.0; 2], look_time: 0 });
            }
            Sheep => {
                self.add_goal(0, G::Float);
                self.add_goal(1, panic(1.25));
                self.add_goal(2, breed);
                self.add_goal(3, tempt(1.1, &[ids::WHEAT]));
                self.add_goal(4, follow(1.1));
                self.add_goal(5, G::EatTile { tick: 0 });
                self.add_goal(6, stroll(1.0));
                self.add_goal(7, look(6.0));
                self.add_goal(8, G::RandomLookAround { rel: [0.0; 2], look_time: 0 });
            }
            Chicken => {
                self.add_goal(0, G::Float);
                self.add_goal(1, panic(1.4));
                self.add_goal(2, breed);
                self.add_goal(3, tempt(1.0, &[ids::SEEDS_WHEAT]));
                self.add_goal(4, follow(1.1));
                self.add_goal(5, stroll(1.0));
                self.add_goal(6, look(6.0));
                self.add_goal(7, G::RandomLookAround { rel: [0.0; 2], look_time: 0 });
            }
            Zombie => {
                self.nav.can_open_doors = true;
                self.add_goal(0, G::Float);
                self.add_goal(2, melee(1.0, false));
                self.add_goal(6, stroll(1.0));
                self.add_goal(7, look(8.0));
                self.add_goal(7, G::RandomLookAround { rel: [0.0; 2], look_time: 0 });
                self.add_target(1, G::HurtBy { alert: true, timestamp: 0 });
                self.add_target(2, G::Nearest { must_see: true, unseen: 0 });
            }
            Skeleton => {
                self.add_goal(1, G::Float);
                self.add_goal(2, G::RestrictSun);
                self.add_goal(3, G::FleeSun { speed: 1.0, pos: [0.0; 3] });
                self.add_goal(4, G::RangedAttack { speed: 1.0, min: 20, max: 60, radius: 15.0, attack_time: -1, see_time: 0 });
                self.add_goal(5, stroll(1.0));
                self.add_goal(6, look(8.0));
                self.add_goal(6, G::RandomLookAround { rel: [0.0; 2], look_time: 0 });
                self.add_target(1, G::HurtBy { alert: false, timestamp: 0 });
                self.add_target(2, G::Nearest { must_see: true, unseen: 0 });
            }
            Spider | CaveSpider | Slime | LavaSlime => {}
            Creeper => {
                self.add_goal(1, G::Float);
                self.add_goal(2, G::Swell);
                self.add_goal(4, melee(1.0, false));
                self.add_goal(5, stroll(0.8));
                self.add_goal(6, look(8.0));
                self.add_goal(6, G::RandomLookAround { rel: [0.0; 2], look_time: 0 });
                self.add_target(1, G::Nearest { must_see: true, unseen: 0 });
                self.add_target(2, G::HurtBy { alert: false, timestamp: 0 });
            }
        }
        if self.kind != Creeper {
            // new-AI mobs float
            self.nav.can_float = true;
        } else {
            self.nav.can_float = true;
        }
    }

    pub fn is_baby(&self) -> bool {
        self.age < 0
    }
    pub fn is_alive(&self) -> bool {
        !self.removed && self.health > 0.0
    }
    pub fn head_height(&self) -> f64 {
        self.b.h * 0.85
    }
    pub fn is_on_fire(&self) -> bool {
        // `LavaSlime::isOnFire`
        self.fire > 0 && self.kind != MobKind::LavaSlime
    }
    /// `Slime::setSize(int)`: bounding box 0.6 × size, `MAX_HEALTH = size²`, full health
    pub fn set_slime_size(&mut self, size: i32) {
        self.size = size;
        let s = 0.6f32 * size as f32;
        self.b.w = s as f64;
        self.b.h = s as f64;
        let (x, y, z) = (self.b.x, self.b.y, self.b.z);
        self.b.set_pos(x, y, z);
        self.health = (size * size) as f32;
    }

    /// `Slime::getSquishSound` (`LavaSlime` overrides it)
    fn squish_sound(&self) -> &'static str {
        match (self.kind, self.size > 1) {
            (MobKind::LavaSlime, true) => "mob.magmacube.big",
            (MobKind::LavaSlime, false) => "mob.magmacube.small",
            (_, true) => "mob.slime.big",
            _ => "mob.slime.small",
        }
    }

    fn hurt_snd(&self) -> &'static str {
        if self.kind.is_slime() { self.squish_sound() } else { self.kind.hurt_sound() }
    }

    fn death_snd(&self) -> &'static str {
        if self.kind.is_slime() { self.squish_sound() } else { self.kind.death_sound() }
    }

    /// `getSoundVolume`: slimes are 0.4 × size
    fn sound_vol(&self) -> f32 {
        if self.kind.is_slime() { 0.4 * self.size as f32 } else { self.kind.sound_volume() }
    }

    /// `LavaSlime::getArmorValue`
    fn armor_value(&self) -> i32 {
        if self.kind == MobKind::LavaSlime { self.size * 3 } else { 0 }
    }

    #[cfg(test)]
    pub fn jump_from_ground_for_test(&mut self) {
        self.jump_from_ground();
    }

    /// `LivingEntity::jumpFromGround` (`LavaSlime` overrides it)
    fn jump_from_ground(&mut self) {
        self.b.yd = if self.kind == MobKind::LavaSlime { (0.42f32 + self.size as f32 * 0.1) as f64 } else { 0.42f32 as f64 };
    }

    fn update_size(&mut self) {
        let (w, h) = self.kind.size();
        let s = if self.is_baby() { 0.5 } else { 1.0 };
        self.b.w = (w * s) as f64;
        self.b.h = (h * s) as f64;
        let (x, y, z) = (self.b.x, self.b.y, self.b.z);
        self.b.set_pos(x, y, z);
    }
    pub fn set_age(&mut self, age: i32) {
        self.age = age;
        self.update_size();
    }
    pub fn path_ent(&self) -> PathEnt {
        PathEnt { x: self.b.x, y: self.b.y, z: self.b.z, bb: self.b.bb, w: self.b.w as f32, h: self.b.h as f32, in_water: self.b.in_water, in_lava: self.b.in_lava }
    }
    fn dist_sqr_to(&self, x: f64, y: f64, z: f64) -> f64 {
        let (dx, dy, dz) = (self.b.x - x, self.b.y - y, self.b.z - z);
        dx * dx + dy * dy + dz * dz
    }
    pub fn brightness(&self, c: &Ctx) -> f32 {
        let hh = (self.b.bb.y1 - self.b.bb.y0) * 0.66;
        c.brightness(self.b.x.floor() as i32, (self.b.y + hh).floor() as i32, self.b.z.floor() as i32)
    }
    pub fn play(&self, c: &mut Ctx, name: &'static str, vol: f32, pitch: f32) {
        if !name.is_empty() {
            c.ev.push(Ev::Sound { name, x: self.b.x, y: self.b.y, z: self.b.z, vol, pitch });
        }
    }
    fn voice_pitch(&mut self) -> f32 {
        let d = (self.rng.next_float() - self.rng.next_float()) * 0.2;
        if self.is_baby() {
            d + 1.5
        } else {
            d + 1.0
        }
    }

    // -------------------------------------------------------------------------------------- walk target value

    pub fn walk_target_value(&self, c: &Ctx, x: i32, y: i32, z: i32) -> f32 {
        if self.kind.is_animal() {
            if c.lv.get_tile(x, y - 1, z) == t::GRASS {
                return 10.0;
            }
            c.brightness(x, y, z) - 0.5
        } else {
            0.5 - c.brightness(x, y, z)
        }
    }

    /// `RandomPos::getPos`
    fn random_pos(&mut self, c: &Ctx, xz: i32, yd: i32) -> Option<[f64; 3]> {
        let mut best: Option<[i32; 3]> = None;
        let mut bv = -99999.0f32;
        for _ in 0..10 {
            let xt = self.rng.next_int_bounded(2 * xz) - xz;
            let zt = self.rng.next_int_bounded(2 * xz) - xz;
            let yt = self.rng.next_int_bounded(2 * yd) - yd;
            let (xt, yt, zt) = (xt + self.b.x.floor() as i32, yt + self.b.y.floor() as i32, zt + self.b.z.floor() as i32);
            let v = self.walk_target_value(c, xt, yt, zt);
            if v > bv {
                bv = v;
                best = Some([xt, yt, zt]);
            }
        }
        best.map(|p| [p[0] as f64, p[1] as f64, p[2] as f64])
    }

    // -------------------------------------------------------------------------------------- navigation

    fn can_update_path(&self) -> bool {
        self.b.on_ground || (self.nav.can_float && (self.b.in_water || self.b.in_lava))
    }
    pub fn create_path_xyz(&self, c: &Ctx, x: i32, y: i32, z: i32) -> Option<Path> {
        if !self.can_update_path() {
            return None;
        }
        let mut pf = PathFinder::new(c.lv, true, self.nav.can_open_doors, self.nav.avoid_water, self.nav.can_float);
        pf.find_path(&self.path_ent(), x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5, self.kind.follow_range())
    }
    pub fn create_path_to(&self, c: &Ctx, tx: f64, ty: f64, tz: f64) -> Option<Path> {
        if !self.can_update_path() {
            return None;
        }
        let mut pf = PathFinder::new(c.lv, true, self.nav.can_open_doors, self.nav.avoid_water, self.nav.can_float);
        pf.find_path(&self.path_ent(), tx, ty, tz, self.kind.follow_range())
    }
    fn create_path_player(&self, c: &Ctx) -> Option<Path> {
        self.create_path_to(c, c.player.x, c.player.bb.y0, c.player.z)
    }
    pub fn move_to_xyz(&mut self, c: &Ctx, x: f64, y: f64, z: f64, speed: f64) -> bool {
        let p = self.create_path_xyz(c, x.floor() as i32, y as i32, z.floor() as i32);
        self.move_to_path(c, p, speed)
    }
    pub fn move_to_path(&mut self, c: &Ctx, p: Option<Path>, speed: f64) -> bool {
        let Some(mut np) = p else {
            self.nav.path = None;
            return false;
        };
        if let Some(old) = &self.nav.path {
            if np.same_as(old) {
                np = old.clone();
            }
        }
        if self.nav.avoid_sun {
            self.trim_path_from_sun(c, &mut np);
        }
        if np.nodes.is_empty() {
            self.nav.path = Some(np);
            return false;
        }
        self.nav.path = Some(np);
        self.nav.speed_mod = speed;
        self.nav.last_stuck_check = self.nav.tick;
        self.nav.last_stuck_pos = self.nav_pos();
        true
    }
    pub fn move_to_player(&mut self, c: &Ctx, speed: f64) -> bool {
        let p = self.create_path_player(c);
        if p.is_some() {
            self.move_to_path(c, p, speed)
        } else {
            false
        }
    }
    pub fn move_to_other(&mut self, c: &Ctx, o: &OtherInfo, speed: f64) -> bool {
        let p = self.create_path_to(c, o.x, o.bb.y0, o.z);
        if p.is_some() {
            self.move_to_path(c, p, speed)
        } else {
            false
        }
    }
    fn nav_pos(&self) -> [f64; 3] {
        [self.b.x, self.surface_y(), self.b.z]
    }
    fn surface_y(&self) -> f64 {
        if !self.b.in_water || !self.nav.can_float {
            return (self.b.bb.y0 + 0.5).trunc();
        }
        self.b.bb.y0.trunc()
    }
    fn trim_path_from_sun(&self, c: &Ctx, p: &mut Path) {
        if c.lv.can_see_sky(self.b.x.floor() as i32, (self.b.bb.y0 + 0.5) as i32, self.b.z.floor() as i32) {
            return;
        }
        for i in 0..p.nodes.len() {
            let n = p.nodes[i];
            if c.lv.can_see_sky(n[0], n[1], n[2]) {
                p.nodes.truncate(i.saturating_sub(1));
                return;
            }
        }
    }

    fn nav_tick(&mut self, c: &Ctx) {
        self.nav.tick += 1;
        if self.nav.is_done() {
            return;
        }
        if self.can_update_path() {
            self.update_path(c);
        }
        if self.nav.is_done() {
            return;
        }
        let w = self.b.w as f32;
        let (idx, p) = {
            let pa = self.nav.path.as_ref().unwrap();
            (pa.index, pa.pos(pa.index, w))
        };
        let _ = idx;
        self.mv_wanted = Some((p, self.nav.speed_mod));
    }

    fn update_path(&mut self, c: &Ctx) {
        let mob_pos = self.nav_pos();
        let w = self.b.w as f32;
        let Some(path) = self.nav.path.as_mut() else { return };
        let mut first_elev = path.nodes.len();
        for i in path.index..path.nodes.len() {
            if path.nodes[i][1] as f64 != mob_pos[1] {
                first_elev = i;
                break;
            }
        }
        let wr = (w * w) as f64;
        for i in path.index..first_elev {
            let pp = path.pos(i, w);
            let d = (mob_pos[0] - pp[0]).powi(2) + (mob_pos[1] - pp[1]).powi(2) + (mob_pos[2] - pp[2]).powi(2);
            if d < wr {
                path.index = i + 1;
            }
        }
        let sx = (self.b.w as f32).ceil() as i32;
        let sy = self.b.h as i32 + 1;
        let sz = sx;
        let mut i = first_elev as i32 - 1;
        while i >= path.index as i32 {
            let pp = path.pos(i as usize, w);
            if can_move_directly(c.lv, &mob_pos, &pp, sx, sy, sz, self.b.in_water) {
                path.index = i as usize;
                break;
            }
            i -= 1;
        }
        if self.nav.tick - self.nav.last_stuck_check > 100 {
            let l = self.nav.last_stuck_pos;
            let d = (mob_pos[0] - l[0]).powi(2) + (mob_pos[1] - l[1]).powi(2) + (mob_pos[2] - l[2]).powi(2);
            if d < 1.5 * 1.5 {
                self.nav.path = None;
            }
            self.nav.last_stuck_check = self.nav.tick;
            self.nav.last_stuck_pos = mob_pos;
        }
    }

    // -------------------------------------------------------------------------------------- controls

    fn set_look_at_xyz(&mut self, x: f64, y: f64, z: f64, ymax: f32, xmax: f32) {
        self.look_wanted = Some(([x, y, z], ymax, xmax));
    }
    fn set_look_at_player(&mut self, c: &Ctx, ymax: f32, xmax: f32) {
        // Player::getHeadHeight is 0.12 above `y`? `y` is already the eye, LookControl uses y + headHeight
        self.look_wanted = Some(([c.player.x, c.player.y + 0.12, c.player.z], ymax, xmax));
    }
    fn set_look_at_other(&mut self, o: &OtherInfo, ymax: f32, xmax: f32) {
        self.look_wanted = Some(([o.x, o.y + (o.bb.y1 - o.bb.y0) * 0.85, o.z], ymax, xmax));
    }

    fn move_control_tick(&mut self) {
        self.yya = 0.0;
        let Some((w, sm)) = self.mv_wanted.take() else { return };
        let yfloor = (self.b.bb.y0 + 0.5).floor();
        let xd = w[0] - self.b.x;
        let zd = w[2] - self.b.z;
        let yd = w[1] - yfloor;
        let dd = xd * xd + yd * yd + zd * zd;
        if dd < (0.0005f64 * 0.0005) {
            return;
        }
        let yrd = (zd.atan2(xd) * 180.0 / std::f64::consts::PI) as f32 - 90.0;
        self.b.yaw = rotlerp(self.b.yaw, yrd, 30.0);
        self.speed = (sm * self.kind.base_speed() as f64) as f32;
        self.yya = self.speed;
        if yd > 0.0 && xd * xd + zd * zd < 1.0 {
            self.jump_flag = true;
        }
    }

    fn look_control_tick(&mut self) {
        self.b.pitch = 0.0;
        if let Some((p, ymax, xmax)) = self.look_wanted.take() {
            let xd = p[0] - self.b.x;
            let yd = p[1] - (self.b.y + self.head_height());
            let zd = p[2] - self.b.z;
            let sd = (xd * xd + zd * zd).sqrt();
            let yrd = (zd.atan2(xd) * 180.0 / std::f64::consts::PI) as f32 - 90.0;
            let xrd = -((yd.atan2(sd) * 180.0 / std::f64::consts::PI) as f32);
            self.b.pitch = rotlerp(self.b.pitch, xrd, xmax);
            self.y_head_rot = rotlerp(self.y_head_rot, yrd, ymax);
        } else {
            self.y_head_rot = rotlerp(self.y_head_rot, self.y_body_rot, 10.0);
        }
        let diff = wrap_degrees(self.y_head_rot - self.y_body_rot);
        if !self.nav.is_done() {
            if diff < -75.0 {
                self.y_head_rot = self.y_body_rot - 75.0;
            }
            if diff > 75.0 {
                self.y_head_rot = self.y_body_rot + 75.0;
            }
        }
    }

    fn jump_control_tick(&mut self) {
        self.jumping = self.jump_flag;
        self.jump_flag = false;
    }

    fn body_control_tick(&mut self) {
        fn clamp(to: f32, from: f32, angle: f32) -> f32 {
            let mut d = wrap_degrees(to - from);
            if d < -angle {
                d = -angle;
            }
            if d >= angle {
                d = angle;
            }
            to - d
        }
        let xd = self.b.x - self.b.xo;
        let zd = self.b.z - self.b.zo;
        if xd * xd + zd * zd > 0.0005 * 0.0005 {
            self.y_body_rot = self.b.yaw;
            self.y_head_rot = clamp(self.y_body_rot, self.y_head_rot, 75.0);
            self.body_last_head = self.y_head_rot;
            self.body_time_still = 0;
            return;
        }
        let mut angle = 75.0;
        if (self.y_head_rot - self.body_last_head).abs() > 15.0 {
            self.body_time_still = 0;
            self.body_last_head = self.y_head_rot;
        } else {
            self.body_time_still += 1;
            if self.body_time_still > 10 {
                angle = (1.0 - (self.body_time_still - 10) as f32 / 10.0).max(0.0) * 75.0;
            }
        }
        self.y_body_rot = clamp(self.y_head_rot, self.y_body_rot, angle);
    }

    // -------------------------------------------------------------------------------------- sensing

    pub fn can_see_player(&mut self, c: &Ctx) -> bool {
        if let Some(v) = self.sense_seen {
            return v;
        }
        let v = line_of_sight(c.lv, [self.b.x, self.b.y + self.head_height(), self.b.z], [c.player.x, c.player.y + 0.12, c.player.z]);
        self.sense_seen = Some(v);
        v
    }

    // -------------------------------------------------------------------------------------- goal selector

    fn tick_selector(&mut self, c: &mut Ctx, targets: bool) {
        let mut goals = std::mem::take(if targets { &mut self.target_goals } else { &mut self.goals });
        let tick = if targets { &mut self.target_tick } else { &mut self.goal_tick };
        let reeval = *tick % 3 == 0;
        *tick += 1;
        let mut to_start = Vec::new();
        if reeval {
            for i in 0..goals.len() {
                if goals[i].running {
                    if can_use_in_system(&goals, i) && self.goal_can_continue(&mut goals[i].g, c) {
                        continue;
                    }
                    self.goal_stop(&mut goals[i].g, c);
                    goals[i].running = false;
                }
                if !can_use_in_system(&goals, i) || !self.goal_can_use(&mut goals[i].g, c) {
                    continue;
                }
                goals[i].running = true;
                to_start.push(i);
            }
        } else {
            for i in 0..goals.len() {
                if goals[i].running && !self.goal_can_continue(&mut goals[i].g, c) {
                    self.goal_stop(&mut goals[i].g, c);
                    goals[i].running = false;
                }
            }
        }
        for i in to_start {
            self.goal_start(&mut goals[i].g, c);
        }
        for i in 0..goals.len() {
            if goals[i].running {
                self.goal_tick(&mut goals[i].g, c);
            }
        }
        if targets {
            self.target_goals = goals;
        } else {
            self.goals = goals;
        }
    }

    fn near_player(&self, c: &Ctx, dist: f64) -> bool {
        c.player.alive && self.dist_sqr_to(c.player.x, c.player.y, c.player.z) <= dist * dist
    }

    fn can_attack_player(&mut self, c: &Ctx, must_see: bool) -> bool {
        if !c.player.alive || c.player.invulnerable {
            return false;
        }
        if must_see && !self.can_see_player(c) {
            return false;
        }
        true
    }

    fn goal_can_use(&mut self, g: &mut G, c: &mut Ctx) -> bool {
        match g {
            G::Float => self.b.in_water || self.b.in_lava,
            G::Panic { pos, .. } => {
                if !self.last_hurt_by_player && !self.is_on_fire() {
                    return false;
                }
                let ts = self.last_hurt_by_ts;
                if ts == 0 {
                    return false;
                }
                let window = 60 + self.rng.next_int_bounded(41);
                if self.tick_count - ts > window {
                    return false;
                }
                match self.random_pos(c, 5, 4) {
                    Some(p) => {
                        *pos = p;
                        true
                    }
                    None => false,
                }
            }
            G::RandomStroll { pos, .. } => {
                if self.no_action_time < 100 && self.rng.next_int_bounded(120) == 0 {
                    return match self.random_pos(c, 10, 7) {
                        Some(p) => {
                            *pos = p;
                            true
                        }
                        None => false,
                    };
                }
                false
            }
            G::LookAtPlayer { dist, .. } => {
                if self.rng.next_float() >= 0.02 {
                    return false;
                }
                self.near_player(c, *dist as f64)
            }
            G::RandomLookAround { .. } => self.rng.next_float() < 0.02,
            G::Melee { path, .. } => {
                if !self.target || !c.player.alive {
                    return false;
                }
                *path = self.create_path_player(c);
                path.is_some()
            }
            G::Breed { partner, .. } => {
                if self.love <= 0 {
                    return false;
                }
                let me = self.id;
                let mut best = None;
                let mut bd = f64::MAX;
                let bbg = self.b.bb.grow(8.0, 8.0, 8.0);
                for o in c.others.iter() {
                    if o.id != me && o.kind == self.kind && o.alive && o.love > 0 && self.love > 0 && bbg.intersects(&o.bb) {
                        let d = self.dist_sqr_to(o.x, o.y, o.z);
                        if d < bd {
                            bd = d;
                            best = Some(o.id);
                        }
                    }
                }
                *partner = best;
                best.is_some()
            }
            G::Tempt { calm, items, running, .. } => {
                if *calm > 0 {
                    *calm -= 1;
                    return false;
                }
                let _ = running;
                if !self.near_player(c, 10.0) {
                    return false;
                }
                matches!(c.player.holding, Some(h) if items.contains(&h))
            }
            G::FollowParent { parent, .. } => {
                if self.age >= 0 {
                    return false;
                }
                let bbg = self.b.bb.grow(8.0, 4.0, 8.0);
                let mut best = None;
                let mut bd = f64::MAX;
                for o in c.others.iter() {
                    if o.kind != self.kind || o.age < 0 || !o.bb.intersects(&bbg) {
                        continue;
                    }
                    let d = self.dist_sqr_to(o.x, o.y, o.z);
                    if d > bd {
                        continue;
                    }
                    bd = d;
                    best = Some(o.id);
                }
                match best {
                    Some(id) if bd >= 9.0 => {
                        *parent = Some(id);
                        true
                    }
                    _ => false,
                }
            }
            G::EatTile { .. } => {
                if self.rng.next_int_bounded(if self.is_baby() { 50 } else { 1000 }) != 0 {
                    return false;
                }
                let (x, y, z) = (self.b.x.floor() as i32, self.b.y.floor() as i32, self.b.z.floor() as i32);
                (c.lv.get_tile(x, y, z) == t::TALLGRASS && c.lv.get_data(x, y, z) == 1) || c.lv.get_tile(x, y - 1, z) == t::GRASS
            }
            G::Swell => self.swell_dir > 0 || (self.target && self.dist_sqr_to(c.player.x, c.player.y, c.player.z) < 9.0),
            G::RestrictSun => c.day,
            G::FleeSun { pos, .. } => {
                if !c.day || !self.is_on_fire() {
                    return false;
                }
                if !c.lv.can_see_sky(self.b.x.floor() as i32, self.b.bb.y0 as i32, self.b.z.floor() as i32) {
                    return false;
                }
                for _ in 0..10 {
                    let xt = (self.b.x + self.rng.next_int_bounded(20) as f64 - 10.0).floor() as i32;
                    let yt = (self.b.bb.y0 + self.rng.next_int_bounded(6) as f64 - 3.0).floor() as i32;
                    let zt = (self.b.z + self.rng.next_int_bounded(20) as f64 - 10.0).floor() as i32;
                    if !c.lv.can_see_sky(xt, yt, zt) && self.walk_target_value(c, xt, yt, zt) < 0.0 {
                        *pos = [xt as f64, yt as f64, zt as f64];
                        return true;
                    }
                }
                false
            }
            G::RangedAttack { .. } => self.target && c.player.alive,
            G::HurtBy { timestamp, .. } => self.last_hurt_by_ts != *timestamp && self.last_hurt_by_player && self.can_attack_player(c, false),
            G::Nearest { .. } => {
                let within = self.kind.follow_range() as f64;
                let p = c.player;
                if !(p.bb.x1 > self.b.bb.x0 - within && p.bb.x0 < self.b.bb.x1 + within && p.bb.z1 > self.b.bb.z0 - within && p.bb.z0 < self.b.bb.z1 + within && p.bb.y1 > self.b.bb.y0 - 4.0 && p.bb.y0 < self.b.bb.y1 + 4.0) {
                    return false;
                }
                self.can_attack_player(c, true)
            }
        }
    }

    fn goal_can_continue(&mut self, g: &mut G, c: &mut Ctx) -> bool {
        match g {
            G::Panic { .. } => (self.last_hurt_by_player || self.is_on_fire()) && !self.nav.is_done(),
            G::RandomStroll { .. } | G::FleeSun { .. } => !self.nav.is_done(),
            G::LookAtPlayer { dist, look_time } => {
                if !c.player.alive || self.dist_sqr_to(c.player.x, c.player.y, c.player.z) > (*dist * *dist) as f64 {
                    return false;
                }
                *look_time > 0
            }
            G::RandomLookAround { look_time, .. } => *look_time >= 0,
            G::Melee { track, .. } => {
                if !self.target || !c.player.alive {
                    return false;
                }
                if !*track {
                    return !self.nav.is_done();
                }
                true
            }
            G::Breed { partner, love_time, .. } => match partner {
                Some(pid) => *love_time < 60 && c.others.iter().any(|o| o.id == *pid && o.alive && o.love > 0),
                None => false,
            },
            G::Tempt { .. } => self.goal_can_use(g, c),
            G::FollowParent { parent, .. } => {
                let Some(pid) = *parent else { return false };
                let Some(o) = c.others.iter().find(|o| o.id == pid && o.alive) else { return false };
                let d = self.dist_sqr_to(o.x, o.y, o.z);
                !(d < 9.0 || d > 256.0)
            }
            G::EatTile { tick } => *tick > 0,
            G::RangedAttack { .. } => (self.target && c.player.alive) && true,
            G::HurtBy { .. } | G::Nearest { .. } => {
                if !self.target || !c.player.alive {
                    return false;
                }
                let within = self.kind.follow_range() as f64;
                if self.dist_sqr_to(c.player.x, c.player.y, c.player.z) > within * within {
                    return false;
                }
                if let G::Nearest { must_see, unseen } = g {
                    if *must_see {
                        if self.can_see_player(c) {
                            *unseen = 0;
                        } else {
                            *unseen += 1;
                            if *unseen > 60 {
                                return false;
                            }
                        }
                    }
                }
                true
            }
            _ => self.goal_can_use(g, c),
        }
    }

    fn goal_start(&mut self, g: &mut G, c: &mut Ctx) {
        match g {
            G::Panic { pos, speed } | G::RandomStroll { pos, speed } | G::FleeSun { pos, speed } => {
                let (p, s) = (*pos, *speed);
                self.move_to_xyz(c, p[0], p[1], p[2], s);
            }
            G::LookAtPlayer { look_time, .. } => *look_time = 40 + self.rng.next_int_bounded(40),
            G::RandomLookAround { rel, look_time } => {
                let r = 2.0 * std::f64::consts::PI * self.rng.next_double();
                *rel = [r.cos(), r.sin()];
                *look_time = 20 + self.rng.next_int_bounded(20);
            }
            G::Melee { path, speed, recalc, .. } => {
                let p = path.take();
                let s = *speed;
                *recalc = 0;
                self.move_to_path(c, p, s);
            }
            G::Tempt { running, old_avoid, .. } => {
                *running = true;
                *old_avoid = self.nav.avoid_water;
                self.nav.avoid_water = false;
            }
            G::FollowParent { recalc, .. } => *recalc = 0,
            G::EatTile { tick } => {
                *tick = 40;
                self.eat_anim = 40;
                self.nav.stop();
            }
            G::Swell => {
                self.nav.stop();
            }
            G::RestrictSun => self.nav.avoid_sun = true,
            G::RangedAttack { attack_time, .. } => *attack_time = -1,
            G::HurtBy { timestamp, alert } => {
                self.target = true;
                *timestamp = self.last_hurt_by_ts;
                if *alert {
                    c.ev.push(Ev::Alert { kind: self.kind, x: self.b.x, y: self.b.y, z: self.b.z, range: self.kind.follow_range() as f64 });
                }
            }
            G::Nearest { unseen, .. } => {
                self.target = true;
                *unseen = 0;
            }
            _ => {}
        }
    }

    fn goal_stop(&mut self, g: &mut G, _c: &mut Ctx) {
        match g {
            G::LookAtPlayer { .. } => {}
            G::Melee { .. } => self.nav.stop(),
            G::Breed { partner, love_time, .. } => {
                *partner = None;
                *love_time = 0;
            }
            G::Tempt { running, old_avoid, calm, .. } => {
                self.nav.stop();
                *calm = 100;
                *running = false;
                self.nav.avoid_water = *old_avoid;
            }
            G::FollowParent { parent, .. } => *parent = None,
            G::EatTile { tick } => {
                *tick = 0;
                self.eat_anim = 0;
            }
            G::RestrictSun => self.nav.avoid_sun = false,
            G::HurtBy { .. } | G::Nearest { .. } => self.target = false,
            G::RangedAttack { see_time, .. } => {
                *see_time = 0;
                self.target = false;
                self.nav.stop();
            }
            _ => {}
        }
    }

    fn goal_tick(&mut self, g: &mut G, c: &mut Ctx) {
        match g {
            G::Float => {
                if self.rng.next_float() < 0.8 {
                    self.jump_flag = true;
                }
            }
            G::LookAtPlayer { look_time, .. } => {
                self.set_look_at_player(c, 10.0, 40.0);
                *look_time -= 1;
            }
            G::RandomLookAround { rel, look_time } => {
                *look_time -= 1;
                let (x, y, z) = (self.b.x + rel[0], self.b.y + self.head_height(), self.b.z + rel[1]);
                self.set_look_at_xyz(x, y, z, 10.0, 40.0);
            }
            G::Melee { speed, track, attack_time, recalc, .. } => {
                self.set_look_at_player(c, 30.0, 30.0);
                if *track || self.can_see_player(c) {
                    *recalc -= 1;
                    if *recalc <= 0 {
                        *recalc = 4 + self.rng.next_int_bounded(7);
                        let s = *speed;
                        self.move_to_player(c, s);
                    }
                }
                *attack_time = (*attack_time - 1).max(0);
                let radius = (self.b.w * 2.0) * (self.b.w * 2.0) + 0.6;
                if self.dist_sqr_to(c.player.x, c.player.bb.y0, c.player.z) > radius {
                    return;
                }
                if *attack_time > 0 {
                    return;
                }
                *attack_time = 20;
                self.do_hurt_target(c);
            }
            G::Breed { speed, partner, love_time } => {
                let Some(pid) = *partner else { return };
                let Some(o) = c.others.iter().find(|o| o.id == pid).copied() else { return };
                self.set_look_at_other(&o, 10.0, 40.0);
                let s = *speed;
                self.move_to_other(c, &o, s);
                *love_time += 1;
                if *love_time >= 60 && self.dist_sqr_to(o.x, o.y, o.z) < 9.0 {
                    c.ev.push(Ev::Breed { a: self.id, b: pid });
                    *love_time = 0;
                }
            }
            G::Tempt { speed, .. } => {
                self.set_look_at_player(c, 30.0, 40.0);
                if self.dist_sqr_to(c.player.x, c.player.y, c.player.z) < 6.25 {
                    self.nav.stop();
                } else {
                    let s = *speed;
                    self.move_to_player(c, s);
                }
            }
            G::FollowParent { speed, parent, recalc } => {
                *recalc -= 1;
                if *recalc > 0 {
                    return;
                }
                *recalc = 10;
                if let Some(pid) = *parent {
                    if let Some(o) = c.others.iter().find(|o| o.id == pid).copied() {
                        let s = *speed;
                        self.move_to_other(c, &o, s);
                    }
                }
            }
            G::EatTile { tick } => {
                *tick = (*tick - 1).max(0);
                self.eat_anim = *tick;
                if *tick != 4 {
                    return;
                }
                let (x, y, z) = (self.b.x.floor() as i32, self.b.y.floor() as i32, self.b.z.floor() as i32);
                if c.lv.get_tile(x, y, z) == t::TALLGRASS {
                    c.ev.push(Ev::Block { x, y, z, id: 0, data: 0 });
                    self.ate();
                } else if c.lv.get_tile(x, y - 1, z) == t::GRASS {
                    c.ev.push(Ev::Block { x, y: y - 1, z, id: t::DIRT, data: 0 });
                    self.ate();
                }
            }
            G::Swell => {
                let ok = self.target && self.dist_sqr_to(c.player.x, c.player.y, c.player.z) <= 49.0 && self.can_see_player(c);
                self.swell_dir = if ok { 1 } else { -1 };
            }
            G::RangedAttack { speed, min, max, radius, attack_time, see_time } => {
                let d = self.dist_sqr_to(c.player.x, c.player.bb.y0, c.player.z);
                let see = self.can_see_player(c);
                if see {
                    *see_time += 1;
                } else {
                    *see_time = 0;
                }
                let r2 = (*radius * *radius) as f64;
                if d > r2 || *see_time < 20 {
                    let s = *speed;
                    self.move_to_player(c, s);
                } else {
                    self.nav.stop();
                }
                self.set_look_at_player(c, 30.0, 30.0);
                *attack_time -= 1;
                if *attack_time == 0 {
                    if d > r2 || !see {
                        return;
                    }
                    let dist = (d.sqrt() as f32 / *radius).clamp(0.1, 1.0);
                    let power = dist;
                    let (mn, mx) = (*min as f32, *max as f32);
                    *attack_time = (dist * (mx - mn) + mn).floor() as i32;
                    self.shoot_arrow(c, power);
                } else if *attack_time < 0 {
                    let dist = d.sqrt() as f32 / *radius;
                    let (mn, mx) = (*min as f32, *max as f32);
                    *attack_time = (dist * (mx - mn) + mn).floor() as i32;
                }
            }
            _ => {}
        }
    }

    fn shoot_arrow(&mut self, c: &mut Ctx, power: f32) {
        let dmg = power as f64 * 2.0 + (self.rng.next_gaussian() * 0.25 + c.difficulty as f64 * 0.11);
        c.ev.push(Ev::Arrow {
            from: [self.b.x, self.b.y + self.head_height() - 0.1, self.b.z],
            to: [c.player.x, c.player.y + 0.12, c.player.z],
            power: 1.6,
            uncertainty: (14 - c.difficulty * 4) as f32,
            damage: dmg,
            owner: self.id,
        });
        let p = 1.0 / (self.rng.next_float() * 0.4 + 0.8);
        self.play(c, "random.bow", 1.0, p);
    }

    fn ate(&mut self) {
        self.sheared = false;
        if self.is_baby() {
            self.age = (self.age + 60 * 20).min(0);
            self.update_size();
        }
    }

    fn do_hurt_target(&mut self, c: &mut Ctx) -> bool {
        match self.kind {
            MobKind::Zombie => {
                c.ev.push(Ev::HurtPlayer { dmg: 3.0, x: self.b.x, z: self.b.z });
                true
            }
            MobKind::Spider | MobKind::CaveSpider => {
                // `Monster::doHurtTarget` with the default ATTACK_DAMAGE of 2
                c.ev.push(Ev::HurtPlayer { dmg: 2.0, x: self.b.x, z: self.b.z });
                if self.kind == MobKind::CaveSpider {
                    // `CaveSpider::doHurtTarget`: poison on Normal (7 s) and Hard (15 s)
                    let secs = match c.difficulty {
                        2 => 7,
                        3 => 15,
                        _ => 0,
                    };
                    if secs > 0 {
                        c.ev.push(Ev::Effect { id: crate::survival::POISON, duration: secs * 20, amplifier: 0 });
                    }
                }
                true
            }
            _ => true,
        }
    }

    // -------------------------------------------------------------------------------------- damage

    pub fn hurt(&mut self, src: Src, mut dmg: f32, c: &mut Ctx) -> bool {
        self.no_action_time = 0;
        if self.health <= 0.0 {
            return false;
        }
        if self.kind.is_animal() {
            // Animal::hurt
            self.love = 0;
        }
        if dmg <= 0.0 {
            dmg = 0.0;
        }
        let mut sound = true;
        if self.invulnerable_time > 10 {
            if dmg <= self.last_hurt {
                return false;
            }
            self.health -= self.absorb_armor(dmg - self.last_hurt);
            self.last_hurt = dmg;
            sound = false;
        } else {
            self.last_hurt = dmg;
            self.invulnerable_time = 20;
            self.health -= self.absorb_armor(dmg);
            self.hurt_time = 10;
            self.hurt_duration = 10;
        }
        self.walk_anim_speed = 1.5;
        let by_player = matches!(src, Src::Player { .. } | Src::Arrow { player: true, .. });
        if self.kind.old_ai() && by_player && !c.player.invulnerable {
            // `Monster::hurt`: the attacker becomes the attack target
            self.old_target = true;
        }
        if by_player {
            self.last_hurt_by_player = true;
            self.last_hurt_by_ts = self.tick_count;
            self.last_hurt_by_player_time = 100;
        }
        if sound {
            let origin = match src {
                Src::Player { x, z } | Src::Arrow { x, z, .. } => Some((x, z)),
                _ => None,
            };
            if let Some((sx, sz)) = origin {
                let mut xd = sx - self.b.x;
                let mut zd = sz - self.b.z;
                while xd * xd + zd * zd < 0.0001 {
                    xd = (self.rng.next_double() - self.rng.next_double()) * 0.01;
                    zd = (self.rng.next_double() - self.rng.next_double()) * 0.01;
                }
                self.knockback(xd, zd);
            }
        }
        let v = self.sound_vol();
        if self.health <= 0.0 {
            if sound {
                let p = self.voice_pitch();
                self.play(c, self.death_snd(), v, p);
            }
            self.die(c);
        } else if sound {
            let p = self.voice_pitch();
            self.play(c, self.hurt_snd(), v, p);
        }
        true
    }

    /// `LivingEntity::getDamageAfterArmorAbsorb`
    fn absorb_armor(&self, dmg: f32) -> f32 {
        let armor = self.armor_value();
        if armor == 0 {
            return dmg;
        }
        let absorb = 25 - armor;
        (dmg * absorb as f32) / 25.0
    }

    pub fn knockback(&mut self, xd: f64, zd: f64) {
        if self.rng.next_double() < 0.0 {
            return;
        }
        let dd = (xd * xd + zd * zd).sqrt();
        let pow = 0.4f64;
        self.b.xd /= 2.0;
        self.b.yd /= 2.0;
        self.b.zd /= 2.0;
        self.b.xd -= xd / dd * pow;
        self.b.yd += pow;
        self.b.zd -= zd / dd * pow;
        if self.b.yd > 0.4 {
            self.b.yd = 0.4;
        }
    }

    fn spawn_item(&self, c: &mut Ctx, id: u16, damage: u16, count: u8) {
        c.ev.push(Ev::Item { id, damage, count, x: self.b.x, y: self.b.y, z: self.b.z });
    }

    fn die(&mut self, c: &mut Ctx) {
        if self.is_baby() {
            return;
        }
        let by_player = self.last_hurt_by_player_time > 0;
        let burning = self.is_on_fire();
        match self.kind {
            MobKind::Pig => {
                let n = self.rng.next_int_bounded(3) + 1 + self.rng.next_int_bounded(1);
                for _ in 0..n {
                    self.spawn_item(c, if burning { ids::PORK_COOKED } else { ids::PORK_RAW }, 0, 1);
                }
            }
            MobKind::Cow => {
                let n = self.rng.next_int_bounded(3) + self.rng.next_int_bounded(1);
                for _ in 0..n {
                    self.spawn_item(c, ids::LEATHER, 0, 1);
                }
                let n = self.rng.next_int_bounded(3) + 1 + self.rng.next_int_bounded(1);
                for _ in 0..n {
                    self.spawn_item(c, if burning { ids::BEEF_COOKED } else { ids::BEEF_RAW }, 0, 1);
                }
            }
            MobKind::Sheep => {
                if !self.sheared {
                    self.spawn_item(c, ids::WOOL, self.sheep_color as u16, 1);
                }
            }
            MobKind::Chicken => {
                let n = self.rng.next_int_bounded(3) + self.rng.next_int_bounded(1);
                for _ in 0..n {
                    self.spawn_item(c, ids::FEATHER, 0, 1);
                }
                self.spawn_item(c, if burning { ids::CHICKEN_COOKED } else { ids::CHICKEN_RAW }, 0, 1);
            }
            MobKind::Zombie | MobKind::Creeper => {
                let loot = if self.kind == MobKind::Zombie { ids::ROTTEN_FLESH } else { ids::GUNPOWDER };
                let n = self.rng.next_int_bounded(3);
                for _ in 0..n {
                    self.spawn_item(c, loot, 0, 1);
                }
            }
            MobKind::Spider | MobKind::CaveSpider => {
                // `Mob::dropDeathLoot` (getDeathLoot = string) + `Spider::dropDeathLoot`
                let n = self.rng.next_int_bounded(3);
                for _ in 0..n {
                    self.spawn_item(c, ids::STRING, 0, 1);
                }
                if by_player && (self.rng.next_int_bounded(3) == 0 || self.rng.next_int_bounded(1) > 0) {
                    self.spawn_item(c, ids::SPIDER_EYE, 0, 1);
                }
            }
            MobKind::Slime => {
                // `Slime::getDeathLoot` (size 1 only) + `Mob::dropDeathLoot`
                if self.size == 1 {
                    let n = self.rng.next_int_bounded(3);
                    for _ in 0..n {
                        self.spawn_item(c, ids::SLIMEBALL, 0, 1);
                    }
                }
            }
            MobKind::LavaSlime => {
                // `LavaSlime::dropDeathLoot`
                if self.size > 1 {
                    let n = self.rng.next_int_bounded(4) - 2;
                    for _ in 0..n {
                        self.spawn_item(c, ids::MAGMA_CREAM, 0, 1);
                    }
                }
            }
            MobKind::Skeleton => {
                let n = self.rng.next_int_bounded(3);
                for _ in 0..n {
                    self.spawn_item(c, ids::ARROW, 0, 1);
                }
                let n = self.rng.next_int_bounded(3);
                for _ in 0..n {
                    self.spawn_item(c, ids::BONE, 0, 1);
                }
            }
        }
        if by_player {
            // `Mob::getExperienceReward`: monsters 5, animals 1..3
            let amount = if self.kind.is_animal() {
                1 + self.rng.next_int_bounded(3)
            } else if self.kind.is_slime() {
                // `xpReward = size`
                self.size
            } else {
                5
            };
            c.ev.push(Ev::Xp { amount, x: self.b.x, y: self.b.y, z: self.b.z });
            let rare = self.rng.next_int_bounded(200);
            if rare < 5 && self.kind == MobKind::Zombie {
                let id = match self.rng.next_int_bounded(3) {
                    0 => ids::IRON_INGOT,
                    1 => ids::CARROT,
                    _ => ids::POTATO,
                };
                self.spawn_item(c, id, 0, 1);
            }
        }
    }

    fn cause_fall_damage(&mut self, dist: f32, c: &mut Ctx) {
        match self.kind {
            MobKind::Chicken | MobKind::LavaSlime => return,
            MobKind::Creeper => {
                self.swell = (self.swell as f32 + dist * 1.5) as i32;
                if self.swell > self.max_swell - 5 {
                    self.swell = self.max_swell - 5;
                }
            }
            _ => {}
        }
        let dmg = (dist - 3.0).ceil() as i32;
        if dmg > 0 {
            self.play(c, if dmg > 4 { "damage.fallbig" } else { "damage.fallsmall" }, 1.0, 1.0);
            self.hurt(Src::Fall, dmg as f32, c);
        }
    }

    // -------------------------------------------------------------------------------------- tick

    pub fn tick(&mut self, c: &mut Ctx) {
        self.tick_count += 1;
        // `Slime::tick` (before `Mob::tick`)
        let mut was_on_ground = false;
        if self.kind.is_slime() {
            self.squish += (self.target_squish - self.squish) * 0.5;
            self.o_squish = self.squish;
            was_on_ground = self.b.on_ground;
        }
        // Entity::baseTick
        self.b.xo = self.b.x;
        self.b.yo = self.b.y;
        self.b.zo = self.b.z;
        self.y_rot_o = self.b.yaw;
        self.x_rot_o = self.b.pitch;
        let was_water = self.b.in_water;
        self.b.update_water(c.lv);
        if self.kind == MobKind::LavaSlime {
            // `LavaSlime::isInLava` is false (it moves freely on the bottom of lava) and `fireImmune`
            self.b.in_lava = false;
            self.fire = 0;
        }
        if self.b.in_water {
            self.fall_distance = 0.0;
            if !was_water {
                self.fire = 0;
            }
            self.fire = 0;
        }
        if self.b.in_lava {
            self.hurt(Src::Lava, 4.0, c);
            self.fire = 300;
        }
        if self.fire > 0 {
            if self.fire % 20 == 0 {
                self.hurt(Src::Fire, 1.0, c);
            }
            self.fire -= 1;
        }
        if self.b.y < -64.0 {
            self.hurt(Src::Void, 4.0, c);
        }
        // LivingEntity::baseTick
        if self.is_alive() {
            let (ex, ey, ez) = (self.b.x.floor() as i32, (self.b.y + self.head_height()).floor() as i32, self.b.z.floor() as i32);
            let id = c.lv.get_tile(ex, ey, ez);
            if id != 0 && id != t::WATER && id != t::CALM_WATER && crate::blocks::style(id, 0).opaque {
                self.hurt(Src::InWall, 1.0, c);
            }
            if crate::physics::is_water(id) {
                self.air_supply -= 1;
                if self.air_supply == -20 {
                    self.air_supply = 0;
                    self.hurt(Src::Drown, 2.0, c);
                }
            } else {
                self.air_supply = 300;
            }
        }
        if self.attack_time > 0 {
            self.attack_time -= 1;
        }
        if self.hurt_time > 0 {
            self.hurt_time -= 1;
        }
        if self.invulnerable_time > 0 {
            self.invulnerable_time -= 1;
        }
        if self.health <= 0.0 {
            self.tick_death(c);
        }
        if self.last_hurt_by_player_time > 0 {
            self.last_hurt_by_player_time -= 1;
        }
        self.y_body_rot_o = self.y_body_rot;
        self.y_head_rot_o = self.y_head_rot;
        // Mob::baseTick ambient sound
        if self.is_alive() && self.rng.next_int_bounded(1000) < self.ambient_time {
            self.ambient_time += 1;
            self.ambient_time = -80;
            if let Some(s) = self.kind.ambient() {
                let p = self.voice_pitch();
                self.play(c, s, self.kind.sound_volume(), p);
            }
        } else {
            self.ambient_time += 1;
        }

        // kind specific tick (Creeper::tick runs before Monster::tick)
        if self.kind == MobKind::Creeper {
            self.creeper_tick(c);
            if self.removed {
                return;
            }
        }
        if !self.kind.is_animal() && c.difficulty == 0 {
            self.removed = true;
            return;
        }
        self.ai_step(c);
        if self.kind.is_spider() {
            // `Spider::tick`: the climb state follows the horizontal collision
            self.climbing = self.b.horizontal_collision;
        }

        if self.kind.is_slime() && self.is_alive() {
            self.slime_player_touch(c);
        }

        // LivingEntity::tick after aiStep
        let xd = self.b.x - self.b.xo;
        let zd = self.b.z - self.b.zo;
        let _ = (xd, zd);
        self.body_control_tick();
        while self.b.yaw - self.y_rot_o < -180.0 {
            self.y_rot_o -= 360.0;
        }
        while self.b.yaw - self.y_rot_o >= 180.0 {
            self.y_rot_o += 360.0;
        }
        while self.y_body_rot - self.y_body_rot_o < -180.0 {
            self.y_body_rot_o -= 360.0;
        }
        while self.y_body_rot - self.y_body_rot_o >= 180.0 {
            self.y_body_rot_o += 360.0;
        }
        while self.b.pitch - self.x_rot_o < -180.0 {
            self.x_rot_o -= 360.0;
        }
        while self.b.pitch - self.x_rot_o >= 180.0 {
            self.x_rot_o += 360.0;
        }
        while self.y_head_rot - self.y_head_rot_o < -180.0 {
            self.y_head_rot_o -= 360.0;
        }
        while self.y_head_rot - self.y_head_rot_o >= 180.0 {
            self.y_head_rot_o += 360.0;
        }
        if self.kind.is_slime() {
            self.slime_after_tick(c, was_on_ground);
        }
    }

    /// The part of `Slime::tick` that follows `Mob::tick`: landing / take-off squish, particles and sound.
    fn slime_after_tick(&mut self, c: &mut Ctx, was_on_ground: bool) {
        if self.b.on_ground && !was_on_ground {
            let size = self.size;
            let lava = self.kind == MobKind::LavaSlime;
            for _ in 0..size * 8 {
                let dir = self.rng.next_float() * PI * 2.0;
                let d = self.rng.next_float() * 0.5 + 0.5;
                let xd = dir.sin() * size as f32 * 0.5 * d;
                let zd = dir.cos() * size as f32 * 0.5 * d;
                c.ev.push(Ev::Particle { lava, x: self.b.x + xd as f64, y: self.b.bb.y0, z: self.b.z + zd as f64 });
            }
            // `doPlayLandSound`: LavaSlime always, Slime from size 4
            if lava || size > 2 {
                let p = ((self.rng.next_float() - self.rng.next_float()) * 0.2 + 1.0) / 0.8;
                self.play(c, self.squish_sound(), self.sound_vol(), p);
            }
            self.target_squish = -0.5;
        } else if !self.b.on_ground && was_on_ground {
            self.target_squish = 1.0;
        }
        // `decreaseSquish`
        self.target_squish *= if self.kind == MobKind::LavaSlime { 0.90 } else { 0.6 };
    }

    /// `Slime::playerTouch` (called every tick for mobs in the player's pickup area `bb.grow(1, 0.5, 1)`)
    fn slime_player_touch(&mut self, c: &mut Ctx) {
        let p = c.player;
        if !p.alive || !self.b.bb.intersects(&p.bb.grow(1.0, 0.5, 1.0)) {
            return;
        }
        // `isDealsDamage`: LavaSlime always, Slime from size 2
        if !(self.kind == MobKind::LavaSlime || self.size > 1) {
            return;
        }
        let size = self.size as f64;
        let seen = line_of_sight(c.lv, [self.b.x, self.b.y + self.head_height(), self.b.z], [p.x, p.y + 0.12, p.z]);
        if seen && self.dist_sqr_to(p.x, p.y, p.z) < (0.6 * size) * (0.6 * size) {
            let dmg = (self.size + if self.kind == MobKind::LavaSlime { 2 } else { 0 }) as f32;
            c.ev.push(Ev::TouchHurt { dmg, x: self.b.x, z: self.b.z, sound: "mob.slime.attack" });
        }
    }

    /// `Slime::serverAiStep`
    fn slime_server_ai_step(&mut self, c: &mut Ctx) {
        self.check_despawn(c);
        let near = c.player.alive && !c.player.invulnerable && self.dist_sqr_to(c.player.x, c.player.y, c.player.z) < 16.0 * 16.0;
        if near {
            self.old_look_at_player(c, 10.0, 20.0);
        }
        let jd = self.jump_delay;
        let ready = self.b.on_ground && {
            self.jump_delay -= 1;
            jd <= 0
        };
        if ready {
            // `getJumpDelay` (LavaSlime × 4)
            self.jump_delay = (self.rng.next_int_bounded(20) + 10) * if self.kind == MobKind::LavaSlime { 4 } else { 1 };
            if near {
                self.jump_delay /= 3;
            }
            self.jumping = true;
            if self.size > 0 {
                let p = ((self.rng.next_float() - self.rng.next_float()) * 0.2 + 1.0) * 0.8;
                self.play(c, self.squish_sound(), self.sound_vol(), p);
            }
            self.xxa = 1.0 - self.rng.next_float() * 2.0;
            self.yya = self.size as f32;
        } else {
            self.jumping = false;
            if self.b.on_ground {
                self.xxa = 0.0;
                self.yya = 0.0;
            }
        }
    }

    fn tick_death(&mut self, c: &mut Ctx) {
        self.death_time += 1;
        if self.death_time == 20 {
            if self.kind.is_slime() && self.size > 1 {
                // `Slime::remove`: split into 2..4 smaller slimes
                let size = self.size;
                let count = 2 + self.rng.next_int_bounded(3);
                let same = c.others.iter().filter(|o| o.kind == self.kind && o.alive).count();
                for i in 0..count {
                    if i == 0 || same < 35 {
                        let xd = ((i % 2) as f32 - 0.5) * size as f32 / 4.0;
                        let zd = ((i / 2) as f32 - 0.5) * size as f32 / 4.0;
                        let yaw = self.rng.next_float() * 360.0;
                        c.ev.push(Ev::SpawnSlime { lava: self.kind == MobKind::LavaSlime, size: size / 2, x: self.b.x + xd as f64, y: self.b.y + 0.5, z: self.b.z + zd as f64, yaw });
                    }
                }
            }
            self.removed = true;
            c.ev.push(Ev::Puff { x: self.b.x, y: self.b.y, z: self.b.z, w: self.b.w as f32, h: self.b.h as f32, n: 20 });
        }
    }

    fn creeper_tick(&mut self, c: &mut Ctx) {
        self.old_swell = self.swell;
        if self.is_alive() {
            let dir = self.swell_dir;
            if dir > 0 && self.swell == 0 {
                self.play(c, "random.fuse", 1.0, 0.5);
            }
            self.swell += dir;
            if self.swell < 0 {
                self.swell = 0;
            }
            if self.swell >= self.max_swell {
                self.swell = self.max_swell;
                let r = if self.powered { self.explosion_radius * 2.0 } else { self.explosion_radius };
                c.ev.push(Ev::Explode { x: self.b.x, y: self.b.y, z: self.b.z, radius: r });
                self.removed = true;
            }
        }
    }

    pub fn swelling(&self, a: f32) -> f32 {
        (self.old_swell as f32 + (self.swell - self.old_swell) as f32 * a) / (self.max_swell - 2) as f32
    }

    fn burn_in_sun(&mut self, c: &mut Ctx) {
        let br = self.brightness(c);
        if br > 0.5 && self.rng.next_float() * 30.0 < (br - 0.4) * 2.0 && c.lv.can_see_sky(self.b.x.floor() as i32, (self.b.y + 0.5).floor() as i32, self.b.z.floor() as i32) {
            self.fire = 160;
        }
    }

    fn ai_step(&mut self, c: &mut Ctx) {
        // Zombie / Skeleton::aiStep
        if matches!(self.kind, MobKind::Zombie | MobKind::Skeleton) && c.day {
            self.burn_in_sun(c);
        }
        if !self.kind.is_animal() {
            // Monster::aiStep
            let br = self.brightness(c);
            if br > 0.5 {
                self.no_action_time += 2;
            }
        }
        // LivingEntity::aiStep
        if self.no_jump_delay > 0 {
            self.no_jump_delay -= 1;
        }
        if self.b.xd.abs() < 0.005 {
            self.b.xd = 0.0;
        }
        if self.b.yd.abs() < 0.005 {
            self.b.yd = 0.0;
        }
        if self.b.zd.abs() < 0.005 {
            self.b.zd = 0.0;
        }
        if self.health <= 0.0 {
            self.jumping = false;
            self.xxa = 0.0;
            self.yya = 0.0;
        } else if self.kind.old_ai() {
            self.old_server_ai_step(c);
            self.y_head_rot = self.b.yaw;
        } else if self.kind.is_slime() {
            self.slime_server_ai_step(c);
            self.y_head_rot = self.b.yaw;
        } else {
            self.new_server_ai_step(c);
        }
        if self.jumping {
            if self.b.in_water || self.b.in_lava {
                self.b.yd += 0.04;
            } else if self.b.on_ground && self.no_jump_delay == 0 {
                self.jump_from_ground();
                self.no_jump_delay = 10;
            }
        } else {
            self.no_jump_delay = 0;
        }
        self.xxa *= 0.98;
        self.yya *= 0.98;
        self.y_rot_a *= 0.9;
        self.travel(c, self.xxa, self.yya);

        // Animal::aiStep
        if self.kind.is_animal() {
            if self.age < 0 {
                self.age += 1;
                if self.age == 0 {
                    self.update_size();
                } else {
                    self.update_size();
                }
            } else if self.age > 0 {
                self.age -= 1;
            }
            if self.age != 0 {
                self.love = 0;
            }
            if self.love > 0 {
                self.love -= 1;
                if self.love % 10 == 0 {
                    c.ev.push(Ev::Hearts { x: self.b.x, y: self.b.y, z: self.b.z, w: self.b.w as f32, h: self.b.h as f32, n: 1 });
                }
            }
        }
        if self.kind == MobKind::Chicken {
            self.chicken_flap(c);
        }
    }

    fn chicken_flap(&mut self, c: &mut Ctx) {
        self.o_flap = self.flap;
        self.o_flap_speed = self.flap_speed;
        self.flap_speed += (if self.b.on_ground { -1.0 } else { 4.0 }) * 0.3;
        self.flap_speed = self.flap_speed.clamp(0.0, 1.0);
        if !self.b.on_ground && self.flapping < 1.0 {
            self.flapping = 1.0;
        }
        self.flapping *= 0.9;
        if !self.b.on_ground && self.b.yd < 0.0 {
            self.b.yd *= 0.6;
        }
        self.flap += self.flapping * 2.0;
        if !self.is_baby() {
            self.egg_time -= 1;
            if self.egg_time <= 0 {
                let p = (self.rng.next_float() - self.rng.next_float()) * 0.2 + 1.0;
                self.play(c, "mob.chicken.plop", 1.0, p);
                self.spawn_item(c, ids::EGG, 0, 1);
                self.egg_time = self.rng.next_int_bounded(20 * 60 * 5) + 20 * 60 * 5;
            }
        }
    }

    /// `LivingEntity::onLadder` (ladder / vine at the feet), `Spider::onLadder` (the synched climb flag)
    pub fn on_ladder(&self, lv: &Level) -> bool {
        if self.kind.is_spider() {
            return self.climbing;
        }
        let id = lv.get_tile(self.b.x.floor() as i32, self.b.bb.y0.floor() as i32, self.b.z.floor() as i32);
        id == t::LADDER || id == t::VINE
    }

    /// `Level::findPath(from, to, maxDist, canPassDoors, canOpenDoors, avoidWater, canFloat)` with the flags `PathfinderMob` uses.
    fn old_path_to_player(&self, c: &Ctx) -> Option<Path> {
        let mut pf = PathFinder::new(c.lv, true, false, false, true);
        pf.find_path(&self.path_ent(), c.player.x, c.player.bb.y0, c.player.z, 16.0)
    }

    /// `Level::findPath(from, x, y, z, maxDist, ...)`
    fn old_path_to_xyz(&self, c: &Ctx, x: i32, y: i32, z: i32) -> Option<Path> {
        let mut pf = PathFinder::new(c.lv, true, false, false, true);
        pf.find_path(&self.path_ent(), x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5, 10.0)
    }

    /// `Monster::findAttackTarget` / `Spider::findAttackTarget` (a player within 16, spiders only in the dark)
    fn old_find_target(&mut self, c: &Ctx) -> bool {
        if !c.player.alive || c.player.invulnerable {
            return false;
        }
        if self.kind.is_spider() {
            // br < 0.5: the attack range is 16
            if self.brightness(c) >= 0.5 {
                return false;
            }
            return self.dist_sqr_to(c.player.x, c.player.y, c.player.z) < 16.0 * 16.0;
        }
        self.dist_sqr_to(c.player.x, c.player.y, c.player.z) < 16.0 * 16.0 && self.can_see_player(c)
    }

    /// `Monster::checkHurtTarget`
    fn old_check_hurt_target(&mut self, c: &mut Ctx, d: f32) {
        if self.kind.is_spider() {
            // `Spider::checkHurtTarget`
            let br = self.brightness(c);
            if br > 0.5 && self.rng.next_int_bounded(100) == 0 {
                self.old_target = false;
                return;
            }
            if d > 2.0 && d < 6.0 && self.rng.next_int_bounded(10) == 0 {
                if self.b.on_ground {
                    let xdd = c.player.x - self.b.x;
                    let zdd = c.player.z - self.b.z;
                    let dd = ((xdd * xdd + zdd * zdd).sqrt()) as f32 as f64;
                    self.b.xd = (xdd / dd * 0.5f32 as f64) * 0.8f32 as f64 + self.b.xd * 0.2f32 as f64;
                    self.b.zd = (zdd / dd * 0.5f32 as f64) * 0.8f32 as f64 + self.b.zd * 0.2f32 as f64;
                    self.b.yd = 0.4f32 as f64;
                }
                return;
            }
        }
        let p = c.player;
        if self.attack_time <= 0 && d < 2.0 && p.bb.y1 > self.b.bb.y0 && p.bb.y0 < self.b.bb.y1 {
            self.attack_time = 20;
            self.do_hurt_target(c);
        }
    }

    /// `PathfinderMob::serverAiStep` (the pre-goal AI), followed by `Mob::serverAiStep` when there is no path.
    fn old_server_ai_step(&mut self, c: &mut Ctx) {
        let max_dist = 16.0f32;
        let _ = max_dist;
        if !self.old_target {
            if self.old_find_target(c) {
                self.old_target = true;
                self.nav.path = self.old_path_to_player(c);
            }
        } else if c.player.alive {
            // `Entity::distanceTo`: player.y is the eye height as in the original (Player::y)
            let d = ((self.b.x - c.player.x).powi(2) + (self.b.y - c.player.y).powi(2) + (self.b.z - c.player.z).powi(2)).sqrt() as f32;
            if self.can_see_player(c) {
                self.old_check_hurt_target(c, d);
            }
        } else {
            self.old_target = false;
        }

        if self.old_target && (self.nav.path.is_none() || self.rng.next_int_bounded(20) == 0) {
            self.nav.path = self.old_path_to_player(c);
        } else if (self.nav.path.is_none() && self.rng.next_int_bounded(180) == 0) || self.rng.next_int_bounded(120) == 0 {
            // `findRandomStrollLocation` (only while it has been active in the last 5 seconds)
            if self.no_action_time < 20 * 5 {
                self.old_stroll(c);
            }
        }

        let y_floor = (self.b.bb.y0 + 0.5).floor();
        let in_water = self.b.in_water;
        let in_lava = self.b.in_lava;
        self.b.pitch = 0.0;
        if self.nav.path.is_none() || self.rng.next_int_bounded(100) == 0 {
            self.mob_server_ai_step(c);
            self.nav.path = None;
            return;
        }

        // follow the path
        let w = self.b.w as f32;
        let r = (w * 2.0) as f64;
        let mut target = self.old_current_pos();
        while let Some(t) = target {
            let dx = t[0] - self.b.x;
            let dz = t[2] - self.b.z;
            if dx * dx + 0.0f64.powi(2) + dz * dz >= r * r {
                break;
            }
            if let Some(p) = self.nav.path.as_mut() {
                p.index += 1;
                if p.is_done() {
                    target = None;
                    self.nav.path = None;
                } else {
                    target = self.old_current_pos();
                }
            }
        }
        self.jumping = false;
        if let Some(t) = target {
            let xd = t[0] - self.b.x;
            let zd = t[2] - self.b.z;
            let yd = t[1] - y_floor;
            let y_rot_d = (zd.atan2(xd) * 180.0 / std::f64::consts::PI) as f32 - 90.0;
            let mut rot_diff = wrap_degrees(y_rot_d - self.b.yaw);
            self.yya = self.kind.base_speed();
            rot_diff = rot_diff.clamp(-30.0, 30.0);
            self.b.yaw += rot_diff;
            if yd > 0.0 {
                self.jumping = true;
            }
        }
        if self.old_target {
            self.old_look_at_player(c, 30.0, 30.0);
        }
        if self.b.horizontal_collision && self.nav.path.is_none() {
            self.jumping = true;
        }
        if self.rng.next_float() < 0.8 && (in_water || in_lava) {
            self.jumping = true;
        }
    }

    fn old_current_pos(&self) -> Option<[f64; 3]> {
        let p = self.nav.path.as_ref()?;
        if p.is_done() {
            return None;
        }
        Some(p.pos(p.index, self.b.w as f32))
    }

    /// `PathfinderMob::findRandomStrollLocation` (10 samples, best `getWalkTargetValue`)
    fn old_stroll(&mut self, c: &Ctx) {
        let mut best = -99999.0f32;
        let mut pos: Option<(i32, i32, i32)> = None;
        for _ in 0..10 {
            let yt = (self.b.y + self.rng.next_int_bounded(7) as f64 - 3.0).floor() as i32;
            let xt = (self.b.x + self.rng.next_int_bounded(13) as f64 - 6.0).floor() as i32;
            let zt = (self.b.z + self.rng.next_int_bounded(13) as f64 - 6.0).floor() as i32;
            let value = self.walk_target_value(c, xt, yt, zt);
            if value > best {
                best = value;
                pos = Some((xt, yt, zt));
            }
        }
        if let Some((x, y, z)) = pos {
            self.nav.path = self.old_path_to_xyz(c, x, y, z);
        }
    }

    /// `Mob::lookAt(entity, yMax, xMax)` for the player
    fn old_look_at_player(&mut self, c: &Ctx, ymax: f32, xmax: f32) {
        let xd = c.player.x - self.b.x;
        let zd = c.player.z - self.b.z;
        let yd = (c.player.y + 0.12) - (self.b.y + self.head_height());
        let sd = (xd * xd + zd * zd).sqrt();
        let y_rot_d = (zd.atan2(xd) * 180.0 / std::f64::consts::PI) as f32 - 90.0;
        let x_rot_d = -((yd.atan2(sd) * 180.0 / std::f64::consts::PI) as f32);
        self.b.pitch = rotlerp(self.b.pitch, x_rot_d, xmax);
        self.b.yaw = rotlerp(self.b.yaw, y_rot_d, ymax);
    }

    /// `Mob::serverAiStep` (despawn check, glance at a nearby player, random turning)
    fn mob_server_ai_step(&mut self, c: &mut Ctx) {
        self.no_action_time += 1; // LivingEntity::serverAiStep
        self.xxa = 0.0;
        self.yya = 0.0;
        self.check_despawn(c);
        let look_distance = 8.0f64;
        if self.rng.next_float() < 0.02 {
            if c.player.alive && self.dist_sqr_to(c.player.x, c.player.y, c.player.z) < look_distance * look_distance {
                self.looking_at = true;
                self.look_time = 10 + self.rng.next_int_bounded(20);
            } else {
                self.y_rot_a = (self.rng.next_float() - 0.5) * 20.0;
            }
        }
        if self.looking_at {
            self.old_look_at_player(c, 10.0, 40.0);
            let t = self.look_time;
            self.look_time -= 1;
            if t <= 0 || !c.player.alive || self.dist_sqr_to(c.player.x, c.player.y, c.player.z) > look_distance * look_distance {
                self.looking_at = false;
            }
        } else {
            if self.rng.next_float() < 0.05 {
                self.y_rot_a = (self.rng.next_float() - 0.5) * 20.0;
            }
            self.b.yaw += self.y_rot_a;
            self.b.pitch = 0.0;
        }
        if self.b.in_water || self.b.in_lava {
            self.jumping = self.rng.next_float() < 0.8;
        }
    }

    fn check_despawn(&mut self, c: &Ctx) {
        if self.persistent {
            self.no_action_time = 0;
            return;
        }
        let sd = self.dist_sqr_to(c.player.x, c.player.y, c.player.z);
        if sd > 128.0 * 128.0 {
            self.removed = true;
        }
        if self.no_action_time > 600 && self.rng.next_int_bounded(800) == 0 && sd > 32.0 * 32.0 {
            self.removed = true;
        } else if sd < 32.0 * 32.0 {
            self.no_action_time = 0;
        }
    }

    fn new_server_ai_step(&mut self, c: &mut Ctx) {
        self.no_action_time += 1;
        self.check_despawn(c);
        self.sense_seen = None;
        self.tick_selector(c, true);
        self.tick_selector(c, false);
        self.nav_tick(c);
        self.move_control_tick();
        self.look_control_tick();
        self.jump_control_tick();
    }

    fn is_free(&self, lv: &Level, xa: f64, ya: f64, za: f64) -> bool {
        crate::physics::get_cubes(lv, &self.b.bb.clone_move(xa, ya, za)).is_empty() && !crate::physics::touches_liquid(lv, &self.b.bb.clone_move(xa, ya, za), crate::physics::is_water)
    }

    fn move_relative(&mut self, xa: f32, za: f32, speed: f32) {
        let d = xa * xa + za * za;
        if d < 0.0001 {
            return;
        }
        let mut d = d.sqrt();
        if d < 1.0 {
            d = 1.0;
        }
        d = speed / d;
        let (xa, za) = (xa * d, za * d);
        let (s, co) = ((self.b.yaw * PI / 180.0).sin(), (self.b.yaw * PI / 180.0).cos());
        self.b.xd += (xa * co - za * s) as f64;
        self.b.zd += (za * co + xa * s) as f64;
    }

    fn travel(&mut self, c: &mut Ctx, xa: f32, ya: f32) {
        let lv = c.lv;
        if self.b.in_water {
            let yo = self.b.y;
            self.move_relative(xa, ya, 0.04);
            self.do_move(c, self.b.xd, self.b.yd, self.b.zd);
            self.b.xd *= 0.8;
            self.b.yd *= 0.8;
            self.b.zd *= 0.8;
            self.b.yd -= 0.02;
            if self.b.horizontal_collision && self.is_free(lv, self.b.xd, self.b.yd + 0.6 - self.b.y + yo, self.b.zd) {
                self.b.yd = 0.3;
            }
        } else if self.b.in_lava {
            let yo = self.b.y;
            self.move_relative(xa, ya, 0.02);
            self.do_move(c, self.b.xd, self.b.yd, self.b.zd);
            self.b.xd *= 0.5;
            self.b.yd *= 0.5;
            self.b.zd *= 0.5;
            self.b.yd -= 0.02;
            if self.b.horizontal_collision && self.is_free(lv, self.b.xd, self.b.yd + 0.6 - self.b.y + yo, self.b.zd) {
                self.b.yd = 0.3;
            }
        } else {
            let ground_friction = |s: &Mob| -> f32 {
                if s.b.on_ground {
                    let id = lv.get_tile(s.b.x.floor() as i32, s.b.bb.y0.floor() as i32 - 1, s.b.z.floor() as i32);
                    if id > 0 {
                        friction_of(id) * 0.91
                    } else {
                        0.6 * 0.91
                    }
                } else {
                    0.91
                }
            };
            let friction = ground_friction(self);
            let f2 = (0.6f32 * 0.6 * 0.91 * 0.91 * 0.6 * 0.91) / (friction * friction * friction);
            let speed = if self.b.on_ground { self.speed * f2 } else { 0.02 };
            self.move_relative(xa, ya, speed);
            let friction = ground_friction(self);
            if self.on_ladder(lv) {
                // `LivingEntity::travel`: climbing is capped at 0.15 in every direction
                let max = 0.15;
                self.b.xd = self.b.xd.clamp(-max, max);
                self.b.zd = self.b.zd.clamp(-max, max);
                self.fall_distance = 0.0;
                if self.b.yd < -0.15 {
                    self.b.yd = -0.15;
                }
            }
            self.do_move(c, self.b.xd, self.b.yd, self.b.zd);
            if self.b.horizontal_collision && self.on_ladder(lv) {
                self.b.yd = 0.2;
            }
            if lv.has_chunk_at(self.b.x as i32, 0, self.b.z as i32) {
                self.b.yd -= 0.08;
            } else if self.b.y > 0.0 {
                self.b.yd = -0.1;
            } else {
                self.b.yd = 0.0;
            }
            self.b.yd *= 0.98;
            self.b.xd *= friction as f64;
            self.b.zd *= friction as f64;
        }
        self.walk_anim_speed_o = self.walk_anim_speed;
        let xx = self.b.x - self.b.xo;
        let zz = self.b.z - self.b.zo;
        let mut wst = ((xx * xx + zz * zz).sqrt() * 4.0) as f32;
        if wst > 1.0 {
            wst = 1.0;
        }
        self.walk_anim_speed += (wst - self.walk_anim_speed) * 0.4;
        self.walk_anim_pos += self.walk_anim_speed;
    }

    /// `Entity::move` + fall damage + step sounds
    fn do_move(&mut self, c: &mut Ctx, xa: f64, ya: f64, za: f64) {
        let (ox, oy, oz) = (self.b.x, self.b.y, self.b.z);
        self.b.move_by(c.lv, xa, ya, za);
        let dy = self.b.y - oy;
        // checkFallDamage
        if self.b.on_ground {
            if self.fall_distance > 0.0 {
                let d = self.fall_distance;
                self.cause_fall_damage(d, c);
                self.fall_distance = 0.0;
            }
        } else if dy < 0.0 {
            self.fall_distance -= dy as f32;
        }
        // step sound
        if !self.kind.step_sound().is_empty() && !self.b.in_water {
            let xm = self.b.x - ox;
            let zm = self.b.z - oz;
            self.move_dist += ((xm * xm + zm * zm).sqrt() * 0.6) as f32;
            if self.move_dist > self.next_step as f32 {
                let id = c.lv.get_tile(self.b.x.floor() as i32, (self.b.y - 0.2).floor() as i32, self.b.z.floor() as i32);
                self.next_step = self.move_dist as i32 + 1;
                if id > 0 {
                    self.play(c, self.kind.step_sound(), 0.15, 1.0);
                }
            }
        }
    }
}

fn can_use_in_system(goals: &[Slot], i: usize) -> bool {
    let me = &goals[i];
    for (j, o) in goals.iter().enumerate() {
        if j == i {
            continue;
        }
        if me.prio >= o.prio {
            if o.running && (me.flags & o.flags) != 0 {
                return false;
            }
        }
    }
    true
}

fn can_move_directly(lv: &Level, start: &[f64; 3], stop: &[f64; 3], mut sx: i32, sy: i32, mut sz: i32, in_water: bool) -> bool {
    let mut gx = start[0].floor() as i32;
    let mut gz = start[2].floor() as i32;
    let mut dx = stop[0] - start[0];
    let mut dz = stop[2] - start[2];
    let dsq = dx * dx + dz * dz;
    if dsq < 1e-8 {
        return false;
    }
    let nf = 1.0 / dsq.sqrt();
    dx *= nf;
    dz *= nf;
    sx += 2;
    sz += 2;
    if !can_walk_on(lv, gx, start[1] as i32, gz, sx, sy, sz, start, dx, dz, in_water) {
        return false;
    }
    sx -= 2;
    sz -= 2;
    let delta_x = 1.0 / dx.abs();
    let delta_z = 1.0 / dz.abs();
    let mut max_x = gx as f64 - start[0];
    let mut max_z = gz as f64 - start[2];
    if dx >= 0.0 {
        max_x += 1.0;
    }
    if dz >= 0.0 {
        max_z += 1.0;
    }
    max_x /= dx;
    max_z /= dz;
    let step_x = if dx < 0.0 { -1 } else { 1 };
    let step_z = if dz < 0.0 { -1 } else { 1 };
    let goal_x = stop[0].floor() as i32;
    let goal_z = stop[2].floor() as i32;
    let mut cdx = goal_x - gx;
    let mut cdz = goal_z - gz;
    let mut guard = 0;
    while cdx * step_x > 0 || cdz * step_z > 0 {
        guard += 1;
        if guard > 64 {
            return false;
        }
        if max_x < max_z {
            max_x += delta_x;
            gx += step_x;
            cdx = goal_x - gx;
        } else {
            max_z += delta_z;
            gz += step_z;
            cdz = goal_z - gz;
        }
        if !can_walk_on(lv, gx, start[1] as i32, gz, sx, sy, sz, start, dx, dz, in_water) {
            return false;
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn can_walk_on(lv: &Level, x: i32, y: i32, z: i32, sx: i32, sy: i32, sz: i32, start: &[f64; 3], gdx: f64, gdz: f64, in_water: bool) -> bool {
    let sxx = x - sx / 2;
    let szz = z - sz / 2;
    // canWalkAbove
    for xx in sxx..sxx + sx {
        for yy in y..y + sy {
            for zz in szz..szz + sz {
                let dx = xx as f64 + 0.5 - start[0];
                let dz = zz as f64 + 0.5 - start[2];
                if dx * gdx + dz * gdz < 0.0 {
                    continue;
                }
                let id = lv.get_tile(xx, yy, zz);
                if id == 0 {
                    continue;
                }
                if !mc_world::tile_props::material(id).blocks_motion() {
                    continue;
                }
                if crate::physics::is_water(id) {
                    continue;
                }
                return false;
            }
        }
    }
    for xx in sxx..sxx + sx {
        for zz in szz..szz + sz {
            let dx = xx as f64 + 0.5 - start[0];
            let dz = zz as f64 + 0.5 - start[2];
            if dx * gdx + dz * gdz < 0.0 {
                continue;
            }
            let id = lv.get_tile(xx, y - 1, zz);
            if id == 0 {
                return false;
            }
            if crate::physics::is_water(id) && !in_water {
                return false;
            }
            if crate::physics::is_lava(id) {
                return false;
            }
        }
    }
    true
}
