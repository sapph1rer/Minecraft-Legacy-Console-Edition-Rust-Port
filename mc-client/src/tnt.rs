//! Primed TNT (`PrimedTnt`, `TntTile`, `TntRenderer`): the fuse, the bounce and the flashing block.

use crate::entity::Body;
use crate::game::Game;
use mc_level::level::{Level, UPDATE_ALL};
use mc_world::tile_ids as t;

pub struct PrimedTnt {
    pub body: Body,
    pub life: i32,
    pub removed: bool,
}

impl PrimedTnt {
    /// Centred on the cell (x, y, z), thrown up a little in a random direction (`PrimedTnt::PrimedTnt`).
    pub fn new(x: f64, y: f64, z: f64, rnd: f32) -> PrimedTnt {
        let mut body = Body::new(0.98, 0.98, 0.49);
        body.set_pos(x, y, z);
        let a = rnd as f64 * std::f64::consts::PI * 2.0;
        body.xd = -a.sin() * 0.02;
        body.yd = 0.2;
        body.zd = -a.cos() * 0.02;
        PrimedTnt { body, life: 80, removed: false }
    }

    /// `PrimedTnt::tick`; true when the fuse ran out.
    pub fn tick(&mut self, lv: &Level) -> bool {
        let b = &mut self.body;
        b.xo = b.x;
        b.yo = b.y;
        b.zo = b.z;
        b.yd -= 0.04;
        let (xd, yd, zd) = (b.xd, b.yd, b.zd);
        b.move_by(lv, xd, yd, zd);
        b.xd *= 0.98;
        b.yd *= 0.98;
        b.zd *= 0.98;
        if b.on_ground {
            b.xd *= 0.7;
            b.zd *= 0.7;
            b.yd *= -0.5;
        }
        self.life -= 1;
        if self.life <= 0 {
            self.removed = true;
            return true;
        }
        false
    }
}

impl Game {
    /// `TntTile::destroy` with the explode bit: the block turns into a lit TNT entity.
    pub fn ignite_tnt(&mut self, p: (i32, i32, i32), _by_player: Option<()>) {
        if self.level.get_tile(p.0, p.1, p.2) != t::TNT {
            return;
        }
        self.level.set_tile(p.0, p.1, p.2, 0, UPDATE_ALL);
        self.mark(p.0, p.2);
        self.redstone.notify(p);
        let r = self.random.next_float();
        self.entities.primed.push(PrimedTnt::new(p.0 as f64 + 0.5, p.1 as f64 + 0.5, p.2 as f64 + 0.5, r));
        self.play("random.fuse", p.0 as f64 + 0.5, p.1 as f64 + 0.5, p.2 as f64 + 0.5, 1.0, 1.0);
    }

    /// `TntTile::wasExploded`: caught in a blast, the block lights with a short random fuse.
    pub fn tnt_chain(&mut self, p: (i32, i32, i32)) {
        let r = self.random.next_float();
        let mut e = PrimedTnt::new(p.0 as f64 + 0.5, p.1 as f64 + 0.5, p.2 as f64 + 0.5, r);
        e.life = self.random.next_int_bounded(80 / 4) + 80 / 8;
        self.entities.primed.push(e);
    }

    pub fn tick_primed(&mut self) {
        let mut list = std::mem::take(&mut self.entities.primed);
        let mut boom = Vec::new();
        for e in list.iter_mut() {
            if e.tick(&self.level) {
                boom.push((e.body.x, e.body.y, e.body.z));
            }
        }
        list.retain(|e| !e.removed);
        // newly chained TNT was pushed into `entities.primed` meanwhile
        let added = std::mem::take(&mut self.entities.primed);
        list.extend(added);
        self.entities.primed = list;
        for (x, y, z) in boom {
            self.explode(x, y, z, 4.0);
        }
    }
}
