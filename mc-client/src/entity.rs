//! Non-player entities: the shared physics body (`Entity::move`) and `ItemEntity` (dropped items).

use crate::physics::{friction_of, get_cubes, is_lava, is_water, touches_liquid};
use mc_core::aabb::Aabb;
use mc_core::random::Random;
use mc_level::level::Level;
use mc_world::item::ItemStack;

/// `ItemEntity::LIFETIME` (5 minutes of ticks)
pub const ITEM_LIFETIME: i32 = 20 * 60 * 5;

/// A physics body: position, velocity, collision box, `Entity::move`.
#[derive(Clone, Debug)]
pub struct Body {
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
    pub bb: Aabb,
    pub w: f64,
    pub h: f64,
    /// `heightOffset`: `y = feet + height_offset`
    pub height_offset: f64,
    pub on_ground: bool,
    pub horizontal_collision: bool,
    pub in_water: bool,
    pub in_lava: bool,
    pub foot_size: f64,
    pub no_physics: bool,
}

impl Body {
    pub fn new(w: f64, h: f64, height_offset: f64) -> Body {
        let mut b = Body { x: 0.0, y: 0.0, z: 0.0, xo: 0.0, yo: 0.0, zo: 0.0, xd: 0.0, yd: 0.0, zd: 0.0, yaw: 0.0, pitch: 0.0, bb: Aabb::default(), w, h, height_offset, on_ground: false, horizontal_collision: false, in_water: false, in_lava: false, foot_size: 0.0, no_physics: false };
        b.set_pos(0.0, 0.0, 0.0);
        b
    }

    /// `Entity::setPos`
    pub fn set_pos(&mut self, x: f64, y: f64, z: f64) {
        self.x = x;
        self.y = y;
        self.z = z;
        let hw = self.w / 2.0;
        let y0 = y - self.height_offset;
        self.bb = Aabb::new(x - hw, y0, z - hw, x + hw, y0 + self.h, z + hw);
    }

    /// `Entity::move` (no sneaking, no slide offset): axis-by-axis collision, optional step-up (`foot_size`).
    pub fn move_by(&mut self, lv: &Level, mut xa: f64, mut ya: f64, mut za: f64) {
        if self.no_physics {
            self.bb.move_by(xa, ya, za);
            self.x = (self.bb.x0 + self.bb.x1) / 2.0;
            self.y = self.bb.y0 + self.height_offset;
            self.z = (self.bb.z0 + self.bb.z1) / 2.0;
            return;
        }
        let (xaorg, yaorg, zaorg) = (xa, ya, za);
        let bb_org = self.bb;
        let mut boxes = get_cubes(lv, &self.bb.expand(xa, ya, za));
        for b in &boxes {
            ya = b.clip_y_collide(&self.bb, ya);
        }
        self.bb.move_by(0.0, ya, 0.0);
        let og = self.on_ground || (yaorg != ya && yaorg < 0.0);
        for b in &boxes {
            xa = b.clip_x_collide(&self.bb, xa);
        }
        self.bb.move_by(xa, 0.0, 0.0);
        for b in &boxes {
            za = b.clip_z_collide(&self.bb, za);
        }
        self.bb.move_by(0.0, 0.0, za);
        if self.foot_size > 0.0 && og && (xaorg != xa || zaorg != za) {
            let (xan, yan, zan) = (xa, ya, za);
            xa = xaorg;
            ya = self.foot_size;
            za = zaorg;
            let normal = self.bb;
            self.bb = bb_org;
            boxes = get_cubes(lv, &self.bb.expand(xa, ya, za).expand(0.0, -ya, 0.0));
            for b in &boxes {
                ya = b.clip_y_collide(&self.bb, ya);
            }
            self.bb.move_by(0.0, ya, 0.0);
            for b in &boxes {
                xa = b.clip_x_collide(&self.bb, xa);
            }
            self.bb.move_by(xa, 0.0, 0.0);
            for b in &boxes {
                za = b.clip_z_collide(&self.bb, za);
            }
            self.bb.move_by(0.0, 0.0, za);
            ya = -self.foot_size;
            for b in &boxes {
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
        self.y = self.bb.y0 + self.height_offset;
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

    pub fn update_water(&mut self, lv: &Level) {
        self.in_water = touches_liquid(lv, &self.bb.grow(0.0, -0.4, 0.0), is_water);
        self.in_lava = touches_liquid(lv, &self.bb.grow(-0.1, -0.4, -0.1), is_lava);
    }

    /// Is the centre of the box inside a solid block (`Entity::checkInTile`, simplified to the centre block)?
    pub fn check_in_tile(&self, lv: &Level, x: f64, y: f64, z: f64) -> bool {
        let (xt, yt, zt) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
        let t = lv.get_tile(xt, yt, zt);
        crate::blocks::style(t, 0).opaque && !get_cubes(lv, &self.bb).is_empty()
    }
}

/// A dropped item stack in the world.
#[derive(Clone, Debug)]
pub struct ItemEntity {
    pub body: Body,
    pub item: ItemStack,
    pub age: i32,
    /// pickup delay (`throwTime`)
    pub throw_time: i32,
    pub health: i32,
    pub bob_offs: f32,
    pub tick_count: i32,
    pub removed: bool,
}

impl ItemEntity {
    /// `ItemEntity(level, x, y, z, item)`: random spin and a little hop.
    pub fn new(x: f64, y: f64, z: f64, item: ItemStack, rng: &mut Random) -> ItemEntity {
        let mut body = Body::new(0.25, 0.25, 0.125);
        body.set_pos(x, y, z);
        body.yaw = (rng.next_double() * 360.0) as f32;
        body.xd = (rng.next_double() * 0.2 - 0.1) as f32 as f64;
        body.yd = 0.2f32 as f64;
        body.zd = (rng.next_double() * 0.2 - 0.1) as f32 as f64;
        ItemEntity { body, item, age: 0, throw_time: 0, health: 5, bob_offs: (rng.next_double() * std::f64::consts::PI * 2.0) as f32, tick_count: 0, removed: false }
    }

    /// `Tile::popResource`: spawn a few tenths of a block around the broken block's centre, with a 10 tick pickup delay.
    pub fn pop(x: i32, y: i32, z: i32, item: ItemStack, rng: &mut Random) -> ItemEntity {
        let s = 0.7f32;
        let xo = (rng.next_float() * s) as f64 + ((1.0 - s) * 0.5) as f64;
        let yo = (rng.next_float() * s) as f64 + ((1.0 - s) * 0.5) as f64;
        let zo = (rng.next_float() * s) as f64 + ((1.0 - s) * 0.5) as f64;
        let mut e = ItemEntity::new(x as f64 + xo, y as f64 + yo, z as f64 + zo, item, rng);
        e.throw_time = 10;
        e
    }

    /// `ItemEntity::tick`. Returns true when it fizzled in lava (play the fizz sound).
    pub fn tick(&mut self, lv: &Level, rng: &mut Random) -> bool {
        let mut fizz = false;
        if self.throw_time > 0 {
            self.throw_time -= 1;
        }
        let b = &mut self.body;
        b.xo = b.x;
        b.yo = b.y;
        b.zo = b.z;
        b.yd -= 0.04f32 as f64;
        b.no_physics = b.check_in_tile(lv, b.x, (b.bb.y0 + b.bb.y1) / 2.0, b.z);
        let (xd, yd, zd) = (b.xd, b.yd, b.zd);
        b.move_by(lv, xd, yd, zd);
        let moved = b.xo as i32 != b.x as i32 || b.yo as i32 != b.y as i32 || b.zo as i32 != b.z as i32;
        if moved || self.tick_count % 25 == 0 {
            let id = lv.get_tile(b.x.floor() as i32, b.y.floor() as i32, b.z.floor() as i32);
            if id == mc_world::tile_ids::LAVA || id == mc_world::tile_ids::CALM_LAVA {
                b.yd = 0.2f32 as f64;
                b.xd = ((rng.next_float() - rng.next_float()) * 0.2) as f64;
                b.zd = ((rng.next_float() - rng.next_float()) * 0.2) as f64;
                fizz = true;
            }
        }
        let mut friction = 0.98f32;
        if b.on_ground {
            friction = 0.6 * 0.98;
            let t = lv.get_tile(b.x.floor() as i32, b.bb.y0.floor() as i32 - 1, b.z.floor() as i32);
            if t > 0 {
                friction = friction_of(t) * 0.98;
            }
        }
        b.xd *= friction as f64;
        b.yd *= 0.98f32 as f64;
        b.zd *= friction as f64;
        if b.on_ground {
            b.yd *= -0.5;
        }
        self.tick_count += 1;
        self.age += 1;
        if self.age >= ITEM_LIFETIME {
            self.removed = true;
        }
        fizz
    }

    /// `ItemEntity::merge`: fold `other` into `self` when they are the same item and fit one stack.
    pub fn merge(&mut self, other: &mut ItemEntity) -> bool {
        if self.removed || other.removed || !self.item.same_item(&other.item) {
            return false;
        }
        if self.item.count < other.item.count {
            return other.merge(self);
        }
        if self.item.count as u32 + other.item.count as u32 > self.item.max_stack() as u32 {
            return false;
        }
        self.item.count += other.item.count;
        self.throw_time = self.throw_time.max(other.throw_time);
        self.age = self.age.min(other.age);
        other.removed = true;
        true
    }
}

/// `FallingTile`: a sand / gravel / anvil block in free fall.
#[derive(Clone, Debug)]
pub struct FallingBlock {
    pub body: Body,
    pub id: u8,
    pub data: u8,
    pub time: i32,
    pub landed: bool,
    pub removed: bool,
}

impl FallingBlock {
    /// A block of `id`/`data` starting in the cell (x, y, z).
    pub fn new(x: i32, y: i32, z: i32, id: u8, data: u8) -> FallingBlock {
        let mut body = Body::new(0.98, 0.98, 0.49);
        body.set_pos(x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5);
        FallingBlock { body, id, data, time: 0, landed: false, removed: false }
    }

    /// `FallingTile::tick` (the caller places the block once `landed`).
    pub fn tick(&mut self, lv: &Level) {
        let b = &mut self.body;
        b.xo = b.x;
        b.yo = b.y;
        b.zo = b.z;
        self.time += 1;
        b.yd -= 0.04f32 as f64;
        let (xd, yd, zd) = (b.xd, b.yd, b.zd);
        b.move_by(lv, xd, yd, zd);
        b.xd *= 0.98f32 as f64;
        b.yd *= 0.98f32 as f64;
        b.zd *= 0.98f32 as f64;
        if b.on_ground {
            b.xd *= 0.7;
            b.zd *= 0.7;
            b.yd *= -0.5;
            self.landed = true;
        } else if (self.time > 100 && (b.y < 1.0 || b.y > 256.0)) || self.time > 600 {
            self.removed = true;
        }
    }
}

#[derive(Default)]
pub struct Entities {
    pub items: Vec<ItemEntity>,
    pub falling: Vec<FallingBlock>,
    pub primed: Vec<crate::tnt::PrimedTnt>,
}

impl Entities {
    /// One tick for all entities. `fizz` collects positions where an item burnt in lava.
    pub fn tick(&mut self, lv: &Level, rng: &mut Random, fizz: &mut Vec<[f64; 3]>) {
        for e in self.items.iter_mut() {
            if e.tick(lv, rng) {
                fizz.push([e.body.x, e.body.y, e.body.z]);
            }
        }
        // merge neighbours (every tick an item moves or each 25th tick; cheap enough to test all close pairs)
        let n = self.items.len();
        for i in 0..n {
            if self.items[i].removed || !(self.items[i].tick_count % 25 == 0 || self.items[i].body.xo as i32 != self.items[i].body.x as i32) {
                continue;
            }
            for j in 0..n {
                if i == j {
                    continue;
                }
                let (a, b) = if i < j {
                    let (l, r) = self.items.split_at_mut(j);
                    (&mut l[i], &mut r[0])
                } else {
                    let (l, r) = self.items.split_at_mut(i);
                    (&mut r[0], &mut l[j])
                };
                if a.body.bb.intersects(&b.body.bb.grow(0.5, 0.0, 0.5)) {
                    a.merge(b);
                }
            }
        }
        self.items.retain(|e| !e.removed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{generate, Dim};
    use mc_world::tile_ids as t;

    #[test]
    fn dropped_item_falls_lands_and_settles() {
        let lv = generate(Dim::Flat, 1, 3);
        let mut rng = Random::with_seed(1);
        let ground = (0..128).rev().find(|&y| lv.get_tile(0, y, 0) != 0).unwrap();
        let mut e = ItemEntity::new(0.5, ground as f64 + 3.0, 0.5, ItemStack::block(t::DIRT, 1), &mut rng);
        for _ in 0..80 {
            e.tick(&lv, &mut rng);
        }
        assert!(e.body.on_ground || e.body.yd.abs() < 0.05);
        // rests with its bottom on top of the ground
        assert!((e.body.bb.y0 - (ground as f64 + 1.0)).abs() < 0.05, "y0 = {}", e.body.bb.y0);
        assert!(e.body.xd.abs() < 0.01);
    }

    #[test]
    fn two_stacks_merge_up_to_the_maximum() {
        let mut rng = Random::with_seed(2);
        let mut a = ItemEntity::new(0.0, 0.0, 0.0, ItemStack::block(t::DIRT, 40), &mut rng);
        let mut b = ItemEntity::new(0.0, 0.0, 0.0, ItemStack::block(t::DIRT, 20), &mut rng);
        assert!(a.merge(&mut b));
        assert_eq!(a.item.count, 60);
        assert!(b.removed);
        let mut c = ItemEntity::new(0.0, 0.0, 0.0, ItemStack::block(t::DIRT, 10), &mut rng);
        assert!(!a.merge(&mut c)); // 70 > 64
        let mut d = ItemEntity::new(0.0, 0.0, 0.0, ItemStack::block(t::STONE, 1), &mut rng);
        assert!(!a.merge(&mut d));
    }

    #[test]
    fn items_expire_after_five_minutes() {
        let lv = generate(Dim::Flat, 1, 3);
        let mut rng = Random::with_seed(3);
        let mut ents = Entities::default();
        let mut e = ItemEntity::new(0.5, 100.0, 0.5, ItemStack::block(t::DIRT, 1), &mut rng);
        e.age = ITEM_LIFETIME - 1;
        ents.items.push(e);
        let mut fz = Vec::new();
        ents.tick(&lv, &mut rng, &mut fz);
        assert!(ents.items.is_empty());
    }
}
