//! Player movement: a port of `Entity::move`, `LivingEntity::travel` / `aiStep` (walking, swimming,
//! flying, jumping) and a block raycast. Runs at the game's fixed 20 ticks per second.

use crate::blocks::{style, Kind};
use mc_core::aabb::Aabb;
use mc_level::level::Level;
use mc_world::{tile_ids as t, tile_props, Material};

pub const WIDTH: f64 = 0.6;
pub const HEIGHT: f64 = 1.8;
pub const EYE_HEIGHT: f64 = 1.62;
const FOOT_SIZE: f64 = 0.5;

#[derive(Clone, Copy, Default, Debug)]
pub struct Input {
    /// +1 forward, -1 back
    pub forward: f32,
    /// +1 left, -1 right (the game's `xxa`)
    pub strafe: f32,
    pub jump: bool,
    pub sneak: bool,
    pub sprint: bool,
}

/// Extra eye drop while crouching on top of the original 0.08 `ySlideOffset` dip (total ≈ Java's 0.35).
pub const SNEAK_EXTRA_DROP: f32 = 0.27;

#[derive(Clone, Debug)]
pub struct Player {
    pub x: f64,
    /// eye height (the entity origin of `Player`: `y = feet + 1.62`)
    pub y: f64,
    pub z: f64,
    pub xd: f64,
    pub yd: f64,
    pub zd: f64,
    /// degrees, game convention (0 = facing +z, increasing clockwise seen from above)
    pub yaw: f32,
    pub pitch: f32,
    pub bb: Aabb,
    pub on_ground: bool,
    pub horizontal_collision: bool,
    pub in_water: bool,
    pub in_lava: bool,
    pub flying: bool,
    pub sprinting: bool,
    /// set on the tick the player jumps from the ground (`Player::jumpFromGround`)
    pub jumped: bool,
    no_jump_delay: i32,
    y_slide_offset: f64,
    /// `Entity::walkDist` (+0.6 × horizontal distance per tick) and its previous value, for view bobbing and footsteps
    pub walk_dist: f64,
    pub walk_dist_o: f64,
    /// `Player::bob` / `tilt` and their previous values (view bobbing)
    pub bob: f32,
    pub o_bob: f32,
    pub tilt: f32,
    pub o_tilt: f32,
    /// `GameRenderer::fov` smoothed field-of-view modifier and its previous value
    pub fov_mod: f32,
    pub o_fov_mod: f32,
    /// extra camera drop while crouching (a visible Java-style crouch; the original only dips 0.08) and its previous value
    pub sneak_drop: f32,
    pub o_sneak_drop: f32,
}

/// Collision boxes of a block, relative to its corner.
fn boxes(id: u8, data: u8) -> std::borrow::Cow<'static, [([f32; 3], [f32; 3])]> {
    use std::borrow::Cow;
    const FULL: [([f32; 3], [f32; 3]); 1] = [([0.0; 3], [1.0; 3])];
    if id == 0 {
        return Cow::Borrowed(&[]);
    }
    let s = style(id, data);
    match s.kind {
        Kind::Shaped => Cow::Owned(crate::shapes::collision(id, data)),
        Kind::Cube => Cow::Borrowed(&FULL),
        Kind::Box { .. } => {
            // boxed() styles are `'static` inside the table, so leak a copy per distinct style once
            use std::collections::HashMap;
            use std::sync::{Mutex, OnceLock};
            static CACHE: OnceLock<Mutex<HashMap<u16, &'static [([f32; 3], [f32; 3])]>>> = OnceLock::new();
            let key = ((id as u16) << 4) | (data as u16 & 15);
            let mut m = CACHE.get_or_init(Default::default).lock().unwrap();
            Cow::Borrowed(*m.entry(key).or_insert_with(|| {
                let Kind::Box { min, max } = s.kind else { unreachable!() };
                Box::leak(Box::new([(min, max)]))
            }))
        }
        _ => Cow::Borrowed(&[]),
    }
}

/// `Level::getCubes`: collision boxes overlapping `bb`. Unloaded chunks are solid.
pub fn get_cubes(lv: &Level, bb: &Aabb) -> Vec<Aabb> {
    let mut out = Vec::new();
    let (x0, x1) = (bb.x0.floor() as i32, (bb.x1 + 1.0).floor() as i32);
    let (z0, z1) = (bb.z0.floor() as i32, (bb.z1 + 1.0).floor() as i32);
    let (y0, y1) = ((bb.y0 - 1.0).floor() as i32, (bb.y1 + 1.0).floor() as i32);
    for x in x0..x1 {
        for z in z0..z1 {
            let loaded = lv.chunk(x >> 4, z >> 4).is_some();
            for y in y0..y1 {
                if !loaded {
                    if (0..256).contains(&y) {
                        let b = Aabb::new(x as f64, y as f64, z as f64, x as f64 + 1.0, y as f64 + 1.0, z as f64 + 1.0);
                        if b.intersects(bb) {
                            out.push(b);
                        }
                    }
                    continue;
                }
                if !(0..256).contains(&y) {
                    continue;
                }
                let id = lv.get_tile(x, y, z);
                let data = lv.get_data(x, y, z);
                let list: std::borrow::Cow<[([f32; 3], [f32; 3])]> = if crate::shapes::is_shaped(id) {
                    let nbf = |a: i32, b: i32, c: i32| if (0..256).contains(&b) { (lv.get_tile(a, b, c), lv.get_data(a, b, c)) } else { (0, 0) };
                    std::borrow::Cow::Owned(crate::shapes::collision_in(id, data, x, y, z, &nbf))
                } else {
                    boxes(id, data)
                };
                for (mn, mx) in list.iter() {
                    let b = Aabb::new(x as f64 + mn[0] as f64, y as f64 + mn[1] as f64, z as f64 + mn[2] as f64, x as f64 + mx[0] as f64, y as f64 + mx[1] as f64, z as f64 + mx[2] as f64);
                    if b.intersects(bb) {
                        out.push(b);
                    }
                }
            }
        }
    }
    out
}

pub(crate) fn is_water(id: u8) -> bool {
    id == t::WATER || id == t::CALM_WATER
}
pub(crate) fn is_lava(id: u8) -> bool {
    id == t::LAVA || id == t::CALM_LAVA
}

/// `Level::checkAndHandleWater` / `containsMaterial`: is any liquid of the kind inside `bb` (below its surface)?
pub(crate) fn touches_liquid(lv: &Level, bb: &Aabb, pred: fn(u8) -> bool) -> bool {
    for x in bb.x0.floor() as i32..(bb.x1 + 1.0).floor() as i32 {
        for y in bb.y0.floor() as i32..(bb.y1 + 1.0).floor() as i32 {
            for z in bb.z0.floor() as i32..(bb.z1 + 1.0).floor() as i32 {
                let id = lv.get_tile(x, y, z);
                if pred(id) {
                    let mut d = (lv.get_data(x, y, z) & 15) as i32;
                    if d >= 8 {
                        d = 0;
                    }
                    let top = y as f64 + 1.0 - (d + 1) as f64 / 9.0;
                    if top >= bb.y0 {
                        return true;
                    }
                }
            }
        }
    }
    false
}

pub(crate) fn friction_of(id: u8) -> f32 {
    if id == t::ICE {
        0.98
    } else {
        0.6
    }
}

impl Player {
    pub fn new(x: f64, feet_y: f64, z: f64) -> Player {
        let mut p = Player {
            x,
            y: feet_y + EYE_HEIGHT,
            z,
            xd: 0.0,
            yd: 0.0,
            zd: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            bb: Aabb::default(),
            on_ground: false,
            horizontal_collision: false,
            in_water: false,
            in_lava: false,
            flying: false,
            sprinting: false,
            jumped: false,
            no_jump_delay: 0,
            y_slide_offset: 0.0,
            walk_dist: 0.0,
            walk_dist_o: 0.0,
            bob: 0.0,
            o_bob: 0.0,
            tilt: 0.0,
            o_tilt: 0.0,
            fov_mod: 1.0,
            o_fov_mod: 1.0,
            sneak_drop: 0.0,
            o_sneak_drop: 0.0,
        };
        p.set_pos(x, p.y, z);
        p
    }

    pub fn feet_y(&self) -> f64 {
        self.y - EYE_HEIGHT + self.y_slide_offset
    }

    pub fn set_pos(&mut self, x: f64, y: f64, z: f64) {
        self.x = x;
        self.y = y;
        self.z = z;
        let w = WIDTH / 2.0;
        let b = y - EYE_HEIGHT + self.y_slide_offset;
        self.bb = Aabb::new(x - w, b, z - w, x + w, b + HEIGHT, z + w);
    }

    /// Unit look direction (camera convention).
    pub fn look_dir(&self) -> [f64; 3] {
        let (yaw, pitch) = ((self.yaw as f64).to_radians(), (self.pitch as f64).to_radians());
        [-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()]
    }

    pub fn is_free(&self, lv: &Level, xa: f64, ya: f64, za: f64) -> bool {
        get_cubes(lv, &self.bb.clone_move(xa, ya, za)).is_empty() && !touches_liquid(lv, &self.bb.clone_move(xa, ya, za), |i| is_water(i) || is_lava(i))
    }

    /// `Entity::move`
    pub fn move_entity(&mut self, lv: &Level, mut xa: f64, mut ya: f64, mut za: f64, sneaking: bool) {
        self.y_slide_offset *= 0.4;
        let (xaorg0, yaorg, zaorg0) = (xa, ya, za);
        let mut xaorg = xa;
        let mut zaorg = za;
        let _ = (xaorg0, zaorg0);
        let bb_org = self.bb;
        let player_sneaking = self.on_ground && sneaking && !self.flying;

        if player_sneaking {
            let d = 0.05;
            while xa != 0.0 && get_cubes(lv, &self.bb.clone_move(xa, -1.0, 0.0)).is_empty() {
                if xa < d && xa >= -d {
                    xa = 0.0
                } else if xa > 0.0 {
                    xa -= d
                } else {
                    xa += d
                }
                xaorg = xa;
            }
            while za != 0.0 && get_cubes(lv, &self.bb.clone_move(0.0, -1.0, za)).is_empty() {
                if za < d && za >= -d {
                    za = 0.0
                } else if za > 0.0 {
                    za -= d
                } else {
                    za += d
                }
                zaorg = za;
            }
            while xa != 0.0 && za != 0.0 && get_cubes(lv, &self.bb.clone_move(xa, -1.0, za)).is_empty() {
                if xa < d && xa >= -d {
                    xa = 0.0
                } else if xa > 0.0 {
                    xa -= d
                } else {
                    xa += d
                }
                if za < d && za >= -d {
                    za = 0.0
                } else if za > 0.0 {
                    za -= d
                } else {
                    za += d
                }
                xaorg = xa;
                zaorg = za;
            }
        }

        let mut boxes_ = get_cubes(lv, &self.bb.expand(xa, ya, za));
        for b in &boxes_ {
            ya = b.clip_y_collide(&self.bb, ya);
        }
        self.bb.move_by(0.0, ya, 0.0);
        let og = self.on_ground || (yaorg != ya && yaorg < 0.0);
        for b in &boxes_ {
            xa = b.clip_x_collide(&self.bb, xa);
        }
        self.bb.move_by(xa, 0.0, 0.0);
        for b in &boxes_ {
            za = b.clip_z_collide(&self.bb, za);
        }
        self.bb.move_by(0.0, 0.0, za);

        if FOOT_SIZE > 0.0 && og && (player_sneaking || self.y_slide_offset < 0.05) && (xaorg != xa || zaorg != za) {
            let (xan, yan, zan) = (xa, ya, za);
            xa = xaorg;
            ya = FOOT_SIZE;
            za = zaorg;
            let normal = self.bb;
            self.bb = bb_org;
            boxes_ = get_cubes(lv, &self.bb.expand(xa, ya, za).expand(0.0, -ya, 0.0));
            for b in &boxes_ {
                ya = b.clip_y_collide(&self.bb, ya);
            }
            self.bb.move_by(0.0, ya, 0.0);
            for b in &boxes_ {
                xa = b.clip_x_collide(&self.bb, xa);
            }
            self.bb.move_by(xa, 0.0, 0.0);
            for b in &boxes_ {
                za = b.clip_z_collide(&self.bb, za);
            }
            self.bb.move_by(0.0, 0.0, za);
            // `slide` is true for every entity, so the `!slide && yaOrg != ya` reset never fires: land first
            ya = -FOOT_SIZE;
            for b in &boxes_ {
                ya = b.clip_y_collide(&self.bb, ya);
            }
            self.bb.move_by(0.0, ya, 0.0);
            if xan * xan + zan * zan >= xa * xa + za * za {
                xa = xan;
                ya = yan;
                za = zan;
                self.bb = normal;
            }
        }

        self.x = (self.bb.x0 + self.bb.x1) / 2.0;
        self.y = self.bb.y0 + EYE_HEIGHT - self.y_slide_offset;
        self.z = (self.bb.z0 + self.bb.z1) / 2.0;
        self.horizontal_collision = xaorg != xa || zaorg != za;
        self.on_ground = yaorg != ya && yaorg < 0.0;
        if xaorg != xa {
            self.xd = 0.0;
        }
        if yaorg != ya {
            self.yd = 0.0;
        }
        if zaorg != za {
            self.zd = 0.0;
        }
    }

    /// `Entity::moveRelative`
    fn move_relative(&mut self, mut xa: f32, mut za: f32, speed: f32) {
        let mut dist = xa * xa + za * za;
        if dist < 0.01 * 0.01 {
            return;
        }
        dist = dist.sqrt();
        if dist < 1.0 {
            dist = 1.0;
        }
        dist = speed / dist;
        xa *= dist;
        za *= dist;
        let (sin, cos) = (self.yaw * std::f32::consts::PI / 180.0).sin_cos();
        self.xd += (xa * cos - za * sin) as f64;
        self.zd += (za * cos + xa * sin) as f64;
    }

    fn ground_friction(&self, lv: &Level) -> f32 {
        let mut f = 0.91f32;
        if self.on_ground {
            f = 0.6 * 0.91;
            let id = lv.get_tile(self.x.floor() as i32, self.bb.y0.floor() as i32 - 1, self.z.floor() as i32);
            if id > 0 {
                f = friction_of(id) * 0.91;
            }
        }
        f
    }

    /// `LivingEntity::travel`
    fn travel(&mut self, lv: &Level, xa: f32, ya: f32, sneaking: bool) {
        let flying_speed = if self.flying { if self.sprinting { 0.1 } else { 0.05 } } else if self.sprinting { 0.02 * 1.3 } else { 0.02 };
        if self.in_water && !self.flying {
            let yo = self.y;
            self.move_relative(xa, ya, 0.02);
            let (xd, yd, zd) = (self.xd, self.yd, self.zd);
            self.move_entity(lv, xd, yd, zd, sneaking);
            self.xd *= 0.8;
            self.yd *= 0.8;
            self.zd *= 0.8;
            self.yd -= 0.02;
            if self.horizontal_collision && self.is_free(lv, self.xd, self.yd + 0.6 - self.y + yo, self.zd) {
                self.yd = 0.3;
            }
        } else if self.in_lava && !self.flying {
            let yo = self.y;
            self.move_relative(xa, ya, 0.02);
            let (xd, yd, zd) = (self.xd, self.yd, self.zd);
            self.move_entity(lv, xd, yd, zd, sneaking);
            self.xd *= 0.5;
            self.yd *= 0.5;
            self.zd *= 0.5;
            self.yd -= 0.02;
            if self.horizontal_collision && self.is_free(lv, self.xd, self.yd + 0.6 - self.y + yo, self.zd) {
                self.yd = 0.3;
            }
        } else {
            let friction = self.ground_friction(lv);
            let friction2 = (0.6f32 * 0.6 * 0.91 * 0.91 * 0.6 * 0.91) / (friction * friction * friction);
            let base = 0.1f32 * if self.sprinting { 1.3 } else { 1.0 };
            let speed = if self.on_ground { base * friction2 } else { flying_speed };
            self.move_relative(xa, ya, speed);
            let friction = self.ground_friction(lv);
            let (xd, yd, zd) = (self.xd, self.yd, self.zd);
            self.move_entity(lv, xd, yd, zd, sneaking);
            self.yd -= 0.08;
            self.yd *= 0.98;
            self.xd *= friction as f64;
            self.zd *= friction as f64;
        }
    }

    /// One game tick (1/20 s): `Entity::baseTick` liquid checks + `LivingEntity::aiStep`.
    pub fn tick(&mut self, lv: &Level, input: &Input) {
        self.jumped = false;
        // LocalPlayer::tick: sneaking drops the camera by ySlideOffset >= 0.2 (decays x0.4 per move)
        if input.sneak && self.y_slide_offset < 0.2 {
            self.y_slide_offset = 0.2;
        }
        self.walk_dist_o = self.walk_dist;
        self.o_bob = self.bob;
        self.o_tilt = self.tilt;
        let (px, pz) = (self.x, self.z);
        self.tick_motion(lv, input);
        // Entity::move: walkDist += horizontal distance * 0.6
        let (dx, dz) = (self.x - px, self.z - pz);
        self.walk_dist += (dx * dx + dz * dz).sqrt() * 0.6;
        // Player::aiStep: bob / tilt
        let mut t_bob = (self.xd * self.xd + self.zd * self.zd).sqrt() as f32;
        if self.xd * self.xd + self.zd * self.zd < 0.00001 {
            t_bob = 0.0;
        }
        let mut t_tilt = ((-self.yd * 0.2).atan() as f32) * 15.0;
        t_bob = t_bob.min(0.1);
        if !self.on_ground {
            t_bob = 0.0;
        }
        if self.on_ground {
            t_tilt = 0.0;
        }
        self.bob += (t_bob - self.bob) * 0.4;
        self.tilt += (t_tilt - self.tilt) * 0.8;
        // GameRenderer::tickFov: LocalPlayer::getFieldOfViewModifier = (speed / walkSpeed + 1) / 2, flying x1.1
        let mut target = 1.0f32;
        if self.flying {
            target *= 1.1;
        }
        target *= ((if self.sprinting { 1.3 } else { 1.0 }) + 1.0) / 2.0;
        self.o_sneak_drop = self.sneak_drop;
        let drop_target = if input.sneak && !self.flying { SNEAK_EXTRA_DROP } else { 0.0 };
        self.sneak_drop += (drop_target - self.sneak_drop) * 0.5;
        self.o_fov_mod = self.fov_mod;
        self.fov_mod = (self.fov_mod + (target - self.fov_mod) * 0.5).clamp(0.1, 1.5);
    }

    /// Camera offset of `GameRenderer::bobView` as a view-space matrix (applied after the look-at view).
    pub fn bob_view(&self, partial: f32) -> glam::Mat4 {
        let wda = (self.walk_dist - self.walk_dist_o) as f32;
        let b = -(self.walk_dist as f32 + wda * partial);
        let bob = self.o_bob + (self.bob - self.o_bob) * partial;
        let tilt = self.o_tilt + (self.tilt - self.o_tilt) * partial;
        let pi = std::f32::consts::PI;
        glam::Mat4::from_translation(glam::Vec3::new((b * pi).sin() * bob * 0.5, -((b * pi).cos() * bob).abs(), 0.0))
            * glam::Mat4::from_rotation_z((b * pi).sin() * bob * 3.0 * pi / 180.0)
            * glam::Mat4::from_rotation_x((((b * pi - 0.2).cos() * bob).abs() * 5.0) * pi / 180.0)
            * glam::Mat4::from_rotation_x(tilt * pi / 180.0)
    }

    /// Smoothed FOV in degrees (`GameRenderer::getFov`: 70 × interpolated modifier).
    pub fn fov_degrees(&self, partial: f32) -> f32 {
        70.0 * (self.o_fov_mod + (self.fov_mod - self.o_fov_mod) * partial)
    }

    fn tick_motion(&mut self, lv: &Level, input: &Input) {
        // updateInWaterState / isInLava
        self.in_water = touches_liquid(lv, &self.bb.grow(0.0, -0.4, 0.0).grow(-0.001, -0.001, -0.001), is_water);
        self.in_lava = touches_liquid(lv, &self.bb.grow(-0.1, -0.4, -0.1), is_lava);

        if self.no_jump_delay > 0 {
            self.no_jump_delay -= 1;
        }
        if self.xd.abs() < 0.005 {
            self.xd = 0.0;
        }
        if self.yd.abs() < 0.005 {
            self.yd = 0.0;
        }
        if self.zd.abs() < 0.005 {
            self.zd = 0.0;
        }

        let mut xxa = input.strafe;
        let mut yya = input.forward;
        if input.sneak {
            xxa *= 0.3;
            yya *= 0.3;
        }
        self.sprinting = input.sprint && input.forward > 0.0 && !input.sneak;

        if self.flying {
            if input.sneak {
                self.yd -= 0.15;
            }
            if input.jump {
                self.yd += 0.15;
            }
        } else if input.jump {
            if self.in_water || self.in_lava {
                self.yd += 0.04;
            } else if self.on_ground && self.no_jump_delay == 0 {
                self.yd = 0.42;
                self.jumped = true;
                if self.sprinting {
                    let rr = self.yaw * std::f32::consts::PI / 180.0;
                    self.xd -= (rr.sin() * 0.2) as f64;
                    self.zd += (rr.cos() * 0.2) as f64;
                }
                self.no_jump_delay = 10;
            }
        } else {
            self.no_jump_delay = 0;
        }
        xxa *= 0.98;
        yya *= 0.98;

        if self.flying {
            let ydo = self.yd;
            self.travel(lv, xxa, yya, input.sneak);
            self.yd = ydo * 0.6;
            // flying never counts as standing on something for jumping
        } else {
            self.travel(lv, xxa, yya, input.sneak);
        }
    }
}

// ------------------------------------------------------------------------------------------ raycast

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockHit {
    pub pos: (i32, i32, i32),
    /// face index (0 = -x, 1 = +x, 2 = -y, 3 = +y, 4 = -z, 5 = +z) of the block that was hit
    pub face: usize,
    pub distance: f64,
}

impl BlockHit {
    /// Cell adjacent to the hit face (where a block would be placed).
    pub fn adjacent(&self) -> (i32, i32, i32) {
        const N: [[i32; 3]; 6] = [[-1, 0, 0], [1, 0, 0], [0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1]];
        let n = N[self.face];
        (self.pos.0 + n[0], self.pos.1 + n[1], self.pos.2 + n[2])
    }
}

fn selectable(id: u8) -> bool {
    id != 0 && !is_water(id) && !is_lava(id)
}

/// Selection box of a block (`Tile::getAABB`): collision boxes, a slightly smaller cube for plants.
pub fn selection_box(id: u8, data: u8) -> ([f32; 3], [f32; 3]) {
    match style(id, data).kind {
        Kind::Box { min, max } => (min, max),
        Kind::Shaped => crate::shapes::selection(id, data),
        Kind::Cross => ([0.1, 0.0, 0.1], [0.9, 0.8, 0.9]),
        Kind::Flat => ([0.0, 0.0, 0.0], [1.0, 0.0625, 1.0]),
        _ => ([0.0; 3], [1.0; 3]),
    }
}

/// Selection box of the block at (x, y, z), using its neighbours (stairs, fences).
pub fn selection_at(lv: &Level, x: i32, y: i32, z: i32) -> ([f32; 3], [f32; 3]) {
    let id = lv.get_tile(x, y, z);
    let data = lv.get_data(x, y, z);
    if crate::shapes::is_shaped(id) {
        let nbf = |a: i32, b: i32, c: i32| if (0..256).contains(&b) { (lv.get_tile(a, b, c), lv.get_data(a, b, c)) } else { (0, 0) };
        return crate::shapes::selection_in(id, data, x, y, z, &nbf);
    }
    selection_box(id, data)
}

/// Ray vs block selection boxes (`Level::clip`), up to `reach` blocks.
pub fn raycast(lv: &Level, origin: [f64; 3], dir: [f64; 3], reach: f64) -> Option<BlockHit> {
    let mut cell = [origin[0].floor() as i32, origin[1].floor() as i32, origin[2].floor() as i32];
    let step = [if dir[0] > 0.0 { 1 } else { -1 }, if dir[1] > 0.0 { 1 } else { -1 }, if dir[2] > 0.0 { 1 } else { -1 }];
    let mut t_max = [0.0f64; 3];
    let mut t_delta = [f64::INFINITY; 3];
    for a in 0..3 {
        if dir[a] != 0.0 {
            let next = if dir[a] > 0.0 { cell[a] as f64 + 1.0 } else { cell[a] as f64 };
            t_max[a] = (next - origin[a]) / dir[a];
            t_delta[a] = (1.0 / dir[a]).abs();
        } else {
            t_max[a] = f64::INFINITY;
        }
    }
    for _ in 0..64 {
        if (0..256).contains(&cell[1]) {
            let id = lv.get_tile(cell[0], cell[1], cell[2]);
            if selectable(id) {
                let (mn, mx) = selection_at(lv, cell[0], cell[1], cell[2]);
                // slab test
                let mut tmin = 0.0f64;
                let mut tmax = reach;
                let mut face = 0usize;
                let mut ok = true;
                for a in 0..3 {
                    let lo = cell[a] as f64 + mn[a] as f64;
                    let hi = cell[a] as f64 + mx[a] as f64;
                    if dir[a].abs() < 1e-12 {
                        if origin[a] < lo || origin[a] > hi {
                            ok = false;
                            break;
                        }
                    } else {
                        let (mut t0, mut t1) = ((lo - origin[a]) / dir[a], (hi - origin[a]) / dir[a]);
                        let mut f = a * 2; // entering through the low side
                        if t0 > t1 {
                            std::mem::swap(&mut t0, &mut t1);
                            f = a * 2 + 1;
                        }
                        if t0 > tmin {
                            tmin = t0;
                            face = f;
                        }
                        tmax = tmax.min(t1);
                        if tmin > tmax {
                            ok = false;
                            break;
                        }
                    }
                }
                if ok && tmin <= reach {
                    return Some(BlockHit { pos: (cell[0], cell[1], cell[2]), face, distance: tmin });
                }
            }
        }
        // advance to the next cell
        let a = if t_max[0] < t_max[1] && t_max[0] < t_max[2] { 0 } else if t_max[1] < t_max[2] { 1 } else { 2 };
        if t_max[a] > reach {
            return None;
        }
        cell[a] += step[a];
        t_max[a] += t_delta[a];
    }
    None
}

/// Would a block placed at `pos` overlap the player?
pub fn block_intersects_player(p: &Player, pos: (i32, i32, i32)) -> bool {
    let b = Aabb::new(pos.0 as f64, pos.1 as f64, pos.2 as f64, pos.0 as f64 + 1.0, pos.1 as f64 + 1.0, pos.2 as f64 + 1.0);
    p.bb.intersects(&b)
}

#[allow(dead_code)]
fn _unused(_: Material, _: u8) -> bool {
    tile_props::is_registered(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_level::chunk::LevelChunk;

    fn flat_level() -> Level {
        let mut lv = Level::new(54, false);
        lv.lighting = false;
        for cx in -2..2 {
            for cz in -2..2 {
                let mut raw = vec![0u8; 32768];
                for x in 0..16usize {
                    for z in 0..16usize {
                        for y in 0..=3usize {
                            raw[(x << 11) | (z << 7) | y] = if y == 0 { t::UNBREAKABLE } else { t::STONE };
                        }
                    }
                }
                let mut c = LevelChunk::from_blocks(cx, cz, &raw);
                c.recalc_heightmap(false);
                lv.insert_chunk(c);
            }
        }
        lv
    }

    #[test]
    fn falls_and_lands_on_the_ground() {
        let lv = flat_level();
        let mut p = Player::new(0.5, 10.0, 0.5);
        for _ in 0..60 {
            p.tick(&lv, &Input::default());
        }
        assert!(p.on_ground);
        assert!((p.feet_y() - 4.0).abs() < 1e-6, "feet at {}", p.feet_y());
    }

    #[test]
    fn jump_reaches_about_1_25_blocks() {
        let lv = flat_level();
        let mut p = Player::new(0.5, 4.0, 0.5);
        p.tick(&lv, &Input::default());
        let mut top = p.feet_y();
        let jump = Input { jump: true, ..Default::default() };
        for _ in 0..30 {
            p.tick(&lv, &jump);
            top = top.max(p.feet_y());
        }
        assert!((top - 4.0 - 1.2).abs() < 0.1, "jumped {}", top - 4.0);
    }

    #[test]
    fn walking_speed_and_wall_collision() {
        let mut lv = flat_level();
        let mut p = Player::new(0.5, 4.0, 0.5);
        p.tick(&lv, &Input::default());
        // walk +z (yaw 0 faces +z) for 20 ticks: ~4.3 blocks/s
        let fwd = Input { forward: 1.0, ..Default::default() };
        let z0 = p.z;
        for _ in 0..20 {
            p.tick(&lv, &fwd);
        }
        let d = p.z - z0;
        assert!(d > 3.5 && d < 5.0, "walked {d}");
        // a 2-high wall at z = 8 stops us
        for y in 4..6 {
            for x in -3..4 {
                lv.set_tile_and_data(x, y, 8, t::STONE, 0, 0);
            }
        }
        for _ in 0..80 {
            p.tick(&lv, &fwd);
        }
        assert!(p.z < 8.0 - WIDTH / 2.0 + 1e-6 && p.z > 7.5, "z = {}", p.z);
        assert!(p.horizontal_collision);
    }

    #[test]
    fn auto_steps_up_half_blocks() {
        let mut lv = flat_level();
        lv.set_tile_and_data(0, 4, 3, t::STONE_SLAB_HALF, 0, 0);
        let mut p = Player::new(0.5, 4.0, 0.5);
        p.tick(&lv, &Input::default());
        let fwd = Input { forward: 1.0, ..Default::default() };
        for _ in 0..30 {
            p.tick(&lv, &fwd);
        }
        assert!(p.z > 4.0, "blocked by a slab at z = {}", p.z);
    }

    #[test]
    fn sneaking_does_not_walk_off_edges() {
        let mut lv = flat_level();
        // cut a trench so the ledge at z = 4 drops away
        for z in 4..16 {
            for x in -4..5 {
                for y in 1..4 {
                    lv.set_tile_and_data(x, y, z, 0, 0, 0);
                }
            }
        }
        let mut p = Player::new(0.5, 4.0, 0.5);
        p.tick(&lv, &Input::default());
        let sneak = Input { forward: 1.0, sneak: true, ..Default::default() };
        for _ in 0..100 {
            p.tick(&lv, &sneak);
        }
        assert!(p.on_ground && p.feet_y() > 3.9, "fell to {}", p.feet_y());
    }

    #[test]
    fn swimming_in_water_sinks_slowly_and_can_rise() {
        let mut lv = flat_level();
        for y in 4..14 {
            for x in -3..4 {
                for z in -3..4 {
                    lv.set_tile_and_data(x, y, z, t::CALM_WATER, 0, 0);
                }
            }
        }
        let mut p = Player::new(0.5, 8.0, 0.5);
        for _ in 0..20 {
            p.tick(&lv, &Input::default());
        }
        assert!(p.in_water);
        assert!(p.yd > -0.2, "sinks slowly, yd = {}", p.yd);
        let up = Input { jump: true, ..Default::default() };
        let y0 = p.y;
        for _ in 0..30 {
            p.tick(&lv, &up);
        }
        assert!(p.y > y0 + 1.0);
    }

    #[test]
    fn raycast_hits_the_right_face() {
        let lv = flat_level();
        let hit = raycast(&lv, [0.5, 6.5, 0.5], [0.0, -1.0, 0.0], 5.0).unwrap();
        assert_eq!(hit.pos, (0, 3, 0));
        assert_eq!(hit.face, 3);
        assert_eq!(hit.adjacent(), (0, 4, 0));
        assert!(raycast(&lv, [0.5, 6.5, 0.5], [0.0, 1.0, 0.0], 5.0).is_none());
        let side = raycast(&lv, [0.5, 3.5, -3.5], [0.0, 0.0, 1.0], 5.0);
        // inside the stone layer: starts in a solid block
        assert!(side.is_some());
    }

    #[test]
    fn flying_ignores_gravity() {
        let lv = flat_level();
        let mut p = Player::new(0.5, 20.0, 0.5);
        p.flying = true;
        for _ in 0..40 {
            p.tick(&lv, &Input::default());
        }
        assert!((p.feet_y() - 20.0).abs() < 1.0, "drifted to {}", p.feet_y());
    }
}
