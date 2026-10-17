//! Pistons: `PistonBaseTile` (extend / retract logic), `PistonExtensionTile` (the head) and the moving piece
//! (`TileEntityPiston`) that animates pushed blocks for two ticks. Facing numbers are the original's:
//! 0 down, 1 up, 2 north (-z), 3 south (+z), 4 west (-x), 5 east (+x).

use crate::game::Game;
use mc_core::aabb::Aabb;
use mc_level::level::{UPDATE_ALL, UPDATE_NONE};
use mc_world::material::PushReaction;
use mc_world::tile_ids as t;

pub type P = (i32, i32, i32);

pub const STEP: [[i32; 3]; 6] = [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];
const MAX_PUSH_DEPTH: usize = 12;
const EXTENDED_BIT: u8 = 8;
const TRIGGER_EXTEND: u8 = 0;
const TRIGGER_CONTRACT: u8 = 1;

/// A block in flight (the tile entity of the moving-piece tile).
#[derive(Clone, Debug)]
pub struct MovingPiece {
    pub pos: P,
    pub id: u8,
    pub data: u8,
    pub facing: u8,
    pub extending: bool,
    /// a retracting piston base: draw its head sliding back as well
    pub head: bool,
    pub progress: f32,
    pub progress_o: f32,
}

impl MovingPiece {
    pub fn get_progress(&self, a: f32) -> f32 {
        let a = a.min(1.0);
        self.progress_o + (self.progress - self.progress_o) * a
    }
    /// Offset of the drawn block from its cell (`TileEntityPiston::getXOff` ...).
    pub fn offset(&self, a: f32) -> [f32; 3] {
        let p = self.get_progress(a);
        let k = if self.extending { p - 1.0 } else { 1.0 - p };
        let s = STEP[self.facing as usize % 6];
        [k * s[0] as f32, k * s[1] as f32, k * s[2] as f32]
    }
}

fn add(p: P, f: usize, n: i32) -> P {
    (p.0 + STEP[f][0] * n, p.1 + STEP[f][1] * n, p.2 + STEP[f][2] * n)
}

pub fn is_piston_base(id: u8) -> bool {
    id == t::PISTON_BASE || id == t::PISTON_STICKY_BASE
}

/// `Tile::isEntityTile` for the tiles we have.
fn is_entity_tile(id: u8) -> bool {
    matches!(id, t::CHEST | t::FURNACE | t::FURNACE_LIT | t::DISPENSER | t::DROPPER | t::MOB_SPAWNER | t::SIGN | t::WALL_SIGN | t::NOTEBLOCK | t::JUKEBOX | t::ENCHANT_TABLE | t::BREWING_STAND | t::ENDER_CHEST | t::HOPPER | t::BEACON | t::SKULL | t::COMMAND_BLOCK)
}

fn push_reaction(id: u8) -> PushReaction {
    mc_world::tile::tile_info(id).and_then(|i| i.material).map_or(PushReaction::Normal, |m| m.push_reaction())
}

impl Game {
    /// `PistonBaseTile::getNewFacing`
    pub fn piston_new_facing(&self, p: P) -> u8 {
        let pl = &self.player;
        if (pl.x - p.0 as f64).abs() < 2.0 && (pl.z - p.2 as f64).abs() < 2.0 {
            let py = pl.y; // eye height
            if py - p.1 as f64 > 2.0 {
                return 1;
            }
            if p.1 as f64 - py > 0.0 {
                return 0;
            }
        }
        match self.facing4() {
            0 => 2,
            1 => 5,
            2 => 3,
            _ => 4,
        }
    }

    /// `PistonBaseTile::getNeighborSignal` (adjacent power except from the push side, plus the cell above).
    fn piston_signal(&self, p: P, facing: usize) -> bool {
        let (x, y, z) = p;
        let has = |q: P, dir: usize| crate::redstone::has_signal(&self.level, q, dir);
        if facing != 0 && has((x, y - 1, z), 0) {
            return true;
        }
        if facing != 1 && has((x, y + 1, z), 1) {
            return true;
        }
        if facing != 2 && has((x, y, z - 1), 2) {
            return true;
        }
        if facing != 3 && has((x, y, z + 1), 3) {
            return true;
        }
        if facing != 5 && has((x + 1, y, z), 5) {
            return true;
        }
        if facing != 4 && has((x - 1, y, z), 4) {
            return true;
        }
        has((x, y, z), 0) || has((x, y + 2, z), 1) || has((x, y + 1, z - 1), 2) || has((x, y + 1, z + 1), 3) || has((x - 1, y + 1, z), 4) || has((x + 1, y + 1, z), 5)
    }

    /// `PistonBaseTile::checkIfExtend`: queue the tile event the power state calls for.
    pub fn piston_check(&mut self, p: P) {
        let id = self.level.get_tile(p.0, p.1, p.2);
        if !is_piston_base(id) {
            return;
        }
        let data = self.level.get_data(p.0, p.1, p.2);
        let facing = data & 7;
        if facing > 5 {
            return;
        }
        let extend = self.piston_signal(p, facing as usize);
        let mut extended = data & EXTENDED_BIT != 0;
        if extended {
            // an extended base whose head is gone (an old save, a block placed over it) would stay stuck: repair it
            let h = add(p, facing as usize, 1);
            let ht = self.level.get_tile(h.0, h.1, h.2);
            if ht != t::PISTON_EXTENSION_PIECE && ht != t::PISTON_MOVING_PIECE {
                self.level.set_data(p.0, p.1, p.2, facing, UPDATE_NONE);
                self.mark(p.0, p.2);
                extended = false;
            }
        }
        if extend && !extended {
            if self.piston_can_push(p, facing as usize) {
                self.piston_queue.push((p, TRIGGER_EXTEND, facing));
            }
        } else if !extend && extended {
            self.level.set_data(p.0, p.1, p.2, facing, UPDATE_NONE);
            self.mark(p.0, p.2);
            self.piston_queue.push((p, TRIGGER_CONTRACT, facing));
        }
    }

    fn piston_pushable(&self, id: u8, p: P, allow_destroyable: bool) -> bool {
        if id == t::OBSIDIAN {
            return false;
        }
        if is_piston_base(id) {
            if self.level.get_data(p.0, p.1, p.2) & EXTENDED_BIT != 0 {
                return false;
            }
        } else {
            if mc_world::tile::tile_info(id).map_or(false, |i| i.is_indestructible()) {
                return false;
            }
            match push_reaction(id) {
                PushReaction::Block => return false,
                PushReaction::Destroy => return allow_destroyable,
                _ => {}
            }
        }
        !is_entity_tile(id)
    }

    fn piston_can_push(&self, base: P, facing: usize) -> bool {
        let mut c = add(base, facing, 1);
        for i in 0..=MAX_PUSH_DEPTH {
            if c.1 <= 0 || c.1 >= 255 || !self.level.has_chunk(c.0 >> 4, c.2 >> 4) {
                return false;
            }
            let id = self.level.get_tile(c.0, c.1, c.2);
            if id == 0 {
                break;
            }
            if !self.piston_pushable(id, c, true) {
                return false;
            }
            if push_reaction(id) == PushReaction::Destroy {
                break;
            }
            if i == MAX_PUSH_DEPTH {
                return false;
            }
            c = add(c, facing, 1);
        }
        true
    }

    fn set_moving(&mut self, p: P, id: u8, data: u8, facing: u8, extending: bool, head: bool, cell_data: u8, flags: i32) {
        self.level.set_tile_and_data(p.0, p.1, p.2, t::PISTON_MOVING_PIECE, cell_data, flags);
        self.moving.retain(|m| m.pos != p);
        self.moving.push(MovingPiece { pos: p, id, data, facing, extending, head, progress: 0.0, progress_o: 0.0 });
        self.mark(p.0, p.2);
    }

    /// `PistonBaseTile::createPush`
    fn piston_create_push(&mut self, base: P, facing: usize) -> bool {
        let sticky = self.level.get_tile(base.0, base.1, base.2) == t::PISTON_STICKY_BASE;
        let bid = self.level.get_tile(base.0, base.1, base.2);
        let mut c = add(base, facing, 1);
        for i in 0..=MAX_PUSH_DEPTH {
            if c.1 <= 0 || c.1 >= 255 || !self.level.has_chunk(c.0 >> 4, c.2 >> 4) {
                return false;
            }
            let id = self.level.get_tile(c.0, c.1, c.2);
            if id == 0 {
                break;
            }
            if !self.piston_pushable(id, c, true) {
                return false;
            }
            if push_reaction(id) == PushReaction::Destroy {
                let d = self.level.get_data(c.0, c.1, c.2);
                for it in mc_world::drops::block_drops(id, d, &mut self.random, false, 0) {
                    let e = crate::entity::ItemEntity::pop(c.0, c.1, c.2, it, &mut self.random);
                    self.entities.items.push(e);
                }
                self.level.set_tile(c.0, c.1, c.2, 0, UPDATE_NONE);
                self.mark(c.0, c.2);
                break;
            }
            if i == MAX_PUSH_DEPTH {
                return false;
            }
            c = add(c, facing, 1);
        }
        let end = c;
        let mut cur = end;
        let mut tiles = Vec::new();
        while cur != base {
            let n = add(cur, facing, -1);
            let id = self.level.get_tile(n.0, n.1, n.2);
            let data = self.level.get_data(n.0, n.1, n.2);
            if id == bid && n == base {
                let hd = facing as u8 | if sticky { EXTENDED_BIT } else { 0 };
                self.set_moving(cur, t::PISTON_EXTENSION_PIECE, hd, facing as u8, true, false, hd, UPDATE_NONE);
            } else {
                self.set_moving(cur, id, data, facing as u8, true, false, data, UPDATE_NONE);
            }
            tiles.push(id);
            cur = n;
        }
        let mut cur = end;
        let mut k = 0;
        while cur != base {
            let n = add(cur, facing, -1);
            self.level.update_neighbors_at(n.0, n.1, n.2, tiles[k]);
            self.redstone.notify(n);
            k += 1;
            cur = n;
        }
        self.redstone.notify(end);
        true
    }

    /// `PistonBaseTile::triggerEvent`
    fn piston_trigger(&mut self, base: P, param: u8, facing: u8) {
        let id = self.level.get_tile(base.0, base.1, base.2);
        if !is_piston_base(id) {
            return;
        }
        let f = facing as usize;
        let extend = self.piston_signal(base, f);
        if extend && param == TRIGGER_CONTRACT {
            self.level.set_data(base.0, base.1, base.2, facing | EXTENDED_BIT, UPDATE_NONE);
            self.mark(base.0, base.2);
            return;
        } else if !extend && param == TRIGGER_EXTEND {
            return;
        }
        let (x, y, z) = base;
        if param == TRIGGER_EXTEND {
            if self.piston_create_push(base, f) {
                self.level.set_data(x, y, z, facing | EXTENDED_BIT, UPDATE_NONE);
                self.mark(x, z);
                let pit = self.random.next_float() * 0.25 + 0.6;
                self.play("tile.piston.out", x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5, 0.5, pit);
            }
        } else {
            // the old head finishes where it is
            let head = add(base, f, 1);
            if let Some(i) = self.moving.iter().position(|m| m.pos == head) {
                let m = self.moving.remove(i);
                self.finish_piece(&m);
            }
            self.set_moving(base, id, facing, facing, false, true, facing, UPDATE_ALL);
            let sticky = id == t::PISTON_STICKY_BASE;
            let two = add(base, f, 2);
            let one = add(base, f, 1);
            if sticky {
                let mut block = self.level.get_tile(two.0, two.1, two.2);
                let mut bdata = self.level.get_data(two.0, two.1, two.2);
                let mut piston_piece = false;
                if block == t::PISTON_MOVING_PIECE {
                    if let Some(i) = self.moving.iter().position(|m| m.pos == two && m.facing == facing && m.extending) {
                        let m = self.moving.remove(i);
                        block = m.id;
                        bdata = m.data;
                        self.finish_piece(&m);
                        piston_piece = true;
                    }
                }
                if !piston_piece && block > 0 && self.piston_pushable(block, two, false) && (push_reaction(block) == PushReaction::Normal || is_piston_base(block)) {
                    self.set_moving(one, block, bdata, facing, false, false, bdata, UPDATE_ALL);
                    self.level.set_tile(two.0, two.1, two.2, 0, UPDATE_ALL);
                    self.mark(two.0, two.2);
                    self.redstone.notify(two);
                } else if !piston_piece {
                    self.level.set_tile(one.0, one.1, one.2, 0, UPDATE_ALL);
                    self.mark(one.0, one.2);
                }
            } else {
                self.level.set_tile(one.0, one.1, one.2, 0, UPDATE_ALL);
                self.mark(one.0, one.2);
            }
            self.redstone.notify(base);
            let pit = self.random.next_float() * 0.15 + 0.6;
            self.play("tile.piston.in", x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5, 0.5, pit);
        }
    }

    /// `TileEntityPiston::finalTick` / the end of its life: the block lands in its cell.
    fn finish_piece(&mut self, m: &MovingPiece) {
        let p = m.pos;
        if self.level.get_tile(p.0, p.1, p.2) == t::PISTON_MOVING_PIECE {
            self.level.set_tile_and_data(p.0, p.1, p.2, m.id, m.data, UPDATE_ALL);
            self.level.update_neighbors_at(p.0, p.1, p.2, m.id);
            self.mark(p.0, p.2);
            self.redstone.notify(p);
        }
    }

    /// Settle every block in flight (before saving).
    pub fn finish_all_pieces(&mut self) {
        for m in std::mem::take(&mut self.moving) {
            self.finish_piece(&m);
        }
    }

    /// Pistons placed by the player: face the player, then react to power.
    pub fn piston_placed(&mut self, p: P) {
        let f = self.piston_new_facing(p);
        self.level.set_data(p.0, p.1, p.2, f, UPDATE_NONE);
        self.mark(p.0, p.2);
        self.piston_check(p);
    }

    /// The head belongs to a base: knock the base's head off when the base disappears.
    pub fn piston_base_removed(&mut self, p: P, data: u8) {
        if data & EXTENDED_BIT != 0 {
            let f = (data & 7) as usize;
            if f < 6 {
                let h = add(p, f, 1);
                if self.level.get_tile(h.0, h.1, h.2) == t::PISTON_EXTENSION_PIECE {
                    self.level.set_tile(h.0, h.1, h.2, 0, UPDATE_ALL);
                    self.mark(h.0, h.2);
                }
            }
        }
    }

    /// The head was broken: the extended base behind it goes too (`PistonExtensionTile::onRemove`).
    pub fn piston_head_removed(&mut self, p: P, data: u8) {
        let f = (data & 7) as usize;
        if f >= 6 {
            return;
        }
        let b = add(p, f, -1);
        let id = self.level.get_tile(b.0, b.1, b.2);
        if is_piston_base(id) && self.level.get_data(b.0, b.1, b.2) & EXTENDED_BIT != 0 {
            let d = self.level.get_data(b.0, b.1, b.2);
            for it in mc_world::drops::block_drops(id, d & 7, &mut self.random, false, 0) {
                let e = crate::entity::ItemEntity::pop(b.0, b.1, b.2, it, &mut self.random);
                self.entities.items.push(e);
            }
            self.level.set_tile(b.0, b.1, b.2, 0, UPDATE_ALL);
            self.mark(b.0, b.2);
        }
    }

    /// One game tick of piston business: queued tile events, then the moving pieces.
    pub fn tick_pistons(&mut self) {
        for (p, param, facing) in std::mem::take(&mut self.piston_queue) {
            self.piston_trigger(p, param, facing);
        }
        let mut pieces = std::mem::take(&mut self.moving);
        let mut keep = Vec::new();
        for mut m in pieces.drain(..) {
            if self.level.get_tile(m.pos.0, m.pos.1, m.pos.2) != t::PISTON_MOVING_PIECE {
                continue;
            }
            m.progress_o = m.progress;
            if m.progress_o >= 1.0 {
                self.finish_piece(&m);
                continue;
            }
            m.progress = (m.progress + 0.5).min(1.0);
            if m.extending {
                self.push_entities(&m);
            }
            keep.push(m);
        }
        keep.extend(std::mem::take(&mut self.moving));
        self.moving = keep;
    }

    /// `TileEntityPiston::moveCollidedEntities`: whatever stands in the way of an extending block is carried along.
    fn push_entities(&mut self, m: &MovingPiece) {
        let o = m.offset(1.0);
        let (mn, mx) = if m.id == t::PISTON_EXTENSION_PIECE { crate::shapes::piston_head_parts(m.data, true)[0].min_max() } else { ([0.0; 3], [1.0; 3]) };
        let bb = Aabb::new(
            m.pos.0 as f64 + o[0] as f64 + mn[0] as f64,
            m.pos.1 as f64 + o[1] as f64 + mn[1] as f64,
            m.pos.2 as f64 + o[2] as f64 + mn[2] as f64,
            m.pos.0 as f64 + o[0] as f64 + mx[0] as f64,
            m.pos.1 as f64 + o[1] as f64 + mx[1] as f64,
            m.pos.2 as f64 + o[2] as f64 + mx[2] as f64,
        );
        let d = (m.progress - m.progress_o + 0.0625) as f64;
        let s = STEP[m.facing as usize % 6];
        let (dx, dy, dz) = (d * s[0] as f64, d * s[1] as f64, d * s[2] as f64);
        if self.player.bb.intersects(&bb) {
            let lv = &self.level;
            self.player.move_entity(lv, dx, dy, dz, false);
        }
        for mob in self.mobs.list.iter_mut() {
            if mob.b.bb.intersects(&bb) {
                mob.b.move_by(&self.level, dx, dy, dz);
            }
        }
        for it in self.entities.items.iter_mut() {
            if it.body.bb.intersects(&bb) {
                it.body.move_by(&self.level, dx, dy, dz);
            }
        }
    }
}
