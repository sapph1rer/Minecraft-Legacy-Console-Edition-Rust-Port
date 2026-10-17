//! Redstone: dust, levers, buttons, plates, torches, repeaters and the things they drive (lamps, doors,
//! trapdoors, fence gates). Follows the structure of `RedStoneDustTile`, `Level::getIndirectPowerLevelTo` and
//! `DiodeTile`: sources provide *weak* power to the cells around them and *strong* power to the block they
//! attach to; solid blocks pass strong power on; dust loses one level per step.

use crate::blocks::style;
use mc_level::level::{Level, UPDATE_ALL, UPDATE_NONE};
use mc_world::tile_ids as t;
use std::collections::{HashMap, HashSet};

pub type P = (i32, i32, i32);

/// Face order shared with the mesher: -x, +x, -y, +y, -z, +z.
pub const DIRS: [[i32; 3]; 6] = [[-1, 0, 0], [1, 0, 0], [0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1]];
const DOWN: usize = 2;
const UP: usize = 3;

fn add(p: P, d: usize) -> P {
    (p.0 + DIRS[d][0], p.1 + DIRS[d][1], p.2 + DIRS[d][2])
}

fn tile(lv: &Level, p: P) -> (u8, u8) {
    if (0..256).contains(&p.1) {
        (lv.get_tile(p.0, p.1, p.2), lv.get_data(p.0, p.1, p.2))
    } else {
        (0, 0)
    }
}

/// `Tile::isSignalSource`
pub fn can_provide_power(id: u8) -> bool {
    matches!(id, t::LEVER | t::BUTTON_STONE | t::BUTTON_WOOD | t::PRESSURE_PLATE_STONE | t::PRESSURE_PLATE_WOOD | t::REDSTONE_TORCH_ON | t::REDSTONE_TORCH_OFF | t::REDSTONE_BLOCK | t::DIODE_ON | t::DIODE_OFF | t::RED_STONE_DUST)
}

/// A solid block that carries power (`Level::isSolidBlockingTile` without the signal sources).
pub fn is_conductor(id: u8, data: u8) -> bool {
    id != 0 && !can_provide_power(id) && id != t::PISTON_BASE && id != t::PISTON_STICKY_BASE && style(id, data).opaque
}

/// Direction (from the block) of the block a repeater / torch / lever is attached to.
pub fn attach_dir(id: u8, data: u8) -> usize {
    match id {
        t::LEVER => match data & 7 {
            1 => 0,
            2 => 1,
            3 => 4,
            4 => 5,
            5 | 6 => DOWN,
            _ => UP,
        },
        _ => match data & 7 {
            1 => 0,
            2 => 1,
            3 => 4,
            4 => 5,
            _ => DOWN,
        },
    }
}

/// Direction a repeater sends its signal.
pub fn front_dir(data: u8) -> usize {
    [5usize, 0, 4, 1][(data & 3) as usize]
}

fn axis_of(d: usize) -> usize {
    d / 2
}

type Nb<'a> = &'a dyn Fn(P) -> (u8, u8);

/// Which horizontal neighbours the dust at `p` joins: -x, +x, -z, +z (`RedStoneDustTile::isPowerProviderOrWire`).
pub fn wire_connections(nb: Nb, p: P) -> [bool; 4] {
    let mut out = [false; 4];
    let above_free = !{
        let (i, d) = nb(add(p, UP));
        is_conductor(i, d)
    };
    for (k, &d) in [0usize, 1, 4, 5].iter().enumerate() {
        let q = add(p, d);
        let (qid, qd) = nb(q);
        let mut c = qid == t::RED_STONE_DUST;
        if !c && can_provide_power(qid) {
            c = match qid {
                t::DIODE_ON | t::DIODE_OFF => {
                    // a repeater joins dust in line with it
                    let f = front_dir(qd);
                    axis_of(f) == axis_of(d)
                }
                _ => true,
            };
        }
        if !c && !is_conductor(qid, qd) {
            c = nb(add(q, DOWN)).0 == t::RED_STONE_DUST;
        }
        if !c && above_free {
            c = nb(add(q, UP)).0 == t::RED_STONE_DUST;
        }
        out[k] = c;
    }
    out
}

/// Axis (0 = x, 1 = z) the dust points along, if it is a straight line (or a dead end).
pub fn wire_line(conn: [bool; 4]) -> Option<usize> {
    let x = conn[0] || conn[1];
    let z = conn[2] || conn[3];
    match (x, z) {
        (true, false) => Some(0),
        (false, true) => Some(1),
        _ => None,
    }
}

/// Dust colour for a power level (`RedStoneDustTile::getColor`).
pub fn wire_colour(power: u8) -> [f32; 3] {
    let p = power as f32 / 15.0;
    let r = if power == 0 { 0.3 } else { p * 0.6 + 0.4 };
    let g = (p * p * 0.7 - 0.5).max(0.0);
    let b = (p * p * 0.6 - 0.7).max(0.0);
    [r, g, b]
}

fn wire_weak(lv: &Level, p: P, toward: usize, data: u8) -> u8 {
    let pw = data & 15;
    if pw == 0 {
        return 0;
    }
    if toward == DOWN {
        return pw;
    }
    if toward == UP {
        return 0;
    }
    let nb = |q: P| tile(lv, q);
    match wire_line(wire_connections(&nb, p)) {
        Some(a) if a == axis_of(toward) => pw,
        _ => 0,
    }
}

/// Power the block at `p` hands to the neighbour in direction `toward` (weak: dust, torches, ...).
fn weak_out(lv: &Level, p: P, toward: usize, excl_wires: bool) -> u8 {
    let (id, data) = tile(lv, p);
    match id {
        t::LEVER | t::BUTTON_STONE | t::BUTTON_WOOD => {
            if data & 8 != 0 {
                15
            } else {
                0
            }
        }
        t::PRESSURE_PLATE_STONE | t::PRESSURE_PLATE_WOOD => {
            if data & 1 != 0 {
                15
            } else {
                0
            }
        }
        t::REDSTONE_TORCH_ON => {
            if toward != attach_dir(id, data) {
                15
            } else {
                0
            }
        }
        t::DIODE_ON => {
            if toward == front_dir(data) {
                15
            } else {
                0
            }
        }
        t::REDSTONE_BLOCK => 15,
        t::RED_STONE_DUST if !excl_wires => wire_weak(lv, p, toward, data),
        _ => 0,
    }
}

/// Power handed *strongly* to the block in direction `toward` (it then passes it on).
fn strong_out(lv: &Level, p: P, toward: usize, excl_wires: bool) -> u8 {
    let (id, data) = tile(lv, p);
    match id {
        t::LEVER | t::BUTTON_STONE | t::BUTTON_WOOD => {
            if data & 8 != 0 && toward == attach_dir(id, data) {
                15
            } else {
                0
            }
        }
        t::PRESSURE_PLATE_STONE | t::PRESSURE_PLATE_WOOD => {
            if data & 1 != 0 && toward == DOWN {
                15
            } else {
                0
            }
        }
        t::REDSTONE_TORCH_ON => {
            if toward == UP {
                15
            } else {
                0
            }
        }
        t::DIODE_ON => {
            if toward == front_dir(data) {
                15
            } else {
                0
            }
        }
        t::RED_STONE_DUST if !excl_wires => wire_weak(lv, p, toward, data),
        _ => 0,
    }
}

/// Strongest power delivered into the solid block at `p` (`Level::getBlockPowerInput`).
fn block_power_input(lv: &Level, p: P, excl_wires: bool) -> u8 {
    let mut m = 0;
    for e in 0..6 {
        m = m.max(strong_out(lv, add(p, e), e ^ 1, excl_wires));
    }
    m
}

/// Power a consumer at `c` sees from its neighbour in direction `d` (`Level::getIndirectPowerLevelTo`).
fn sample(lv: &Level, c: P, d: usize, excl_wires: bool) -> u8 {
    let n = add(c, d);
    let (id, data) = tile(lv, n);
    if is_conductor(id, data) {
        block_power_input(lv, n, excl_wires)
    } else {
        weak_out(lv, n, d ^ 1, excl_wires)
    }
}

/// `Level::hasSignal(x, y, z, dir)`: the power cell `q` hands out towards the consumer that sits opposite to `dir`
/// (`dir` uses the original facing numbers: 0 down, 1 up, 2 north, 3 south, 4 west, 5 east).
pub fn has_signal(lv: &Level, q: P, dir: usize) -> bool {
    let d = [DOWN, UP, 4, 5, 0, 1][dir];
    sample(lv, add(q, d ^ 1), d, false) > 0
}

/// `Level::hasNeighborSignal`
pub fn is_powered(lv: &Level, c: P) -> bool {
    (0..6).any(|d| sample(lv, c, d, false) > 0)
}

fn repeater_input(lv: &Level, p: P, data: u8) -> u8 {
    let f = front_dir(data);
    let back = add(p, f ^ 1);
    let (bid, bd) = tile(lv, back);
    let mut v = sample(lv, p, f ^ 1, false);
    if bid == t::RED_STONE_DUST {
        v = v.max(bd & 15);
    }
    v
}

fn torch_powered(lv: &Level, p: P, data: u8) -> bool {
    let a = attach_dir(t::REDSTONE_TORCH_ON, data);
    let n = add(p, a);
    let (id, d) = tile(lv, n);
    if is_conductor(id, d) {
        block_power_input(lv, n, false) > 0
    } else {
        weak_out(lv, n, a ^ 1, false) > 0
    }
}

/// Something that happened this tick that the game should show: block changed, a sound.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Changed(P),
    Sound(&'static str, P, f32, f32),
    /// a piston base or TNT next to changed power: look at it
    Piston(P),
    Ignite(P),
}

#[derive(Default)]
pub struct Redstone {
    dirty: HashSet<P>,
    scheduled: Vec<(P, i32)>,
    door_power: HashMap<P, bool>,
    pub events: Vec<Event>,
}

impl Redstone {
    /// Something near `p` changed: look at everything within two blocks again next tick.
    pub fn notify(&mut self, p: P) {
        for dx in -2i32..=2 {
            for dy in -2i32..=2 {
                for dz in -2i32..=2 {
                    if dx.abs() + dy.abs() + dz.abs() <= 2 {
                        self.dirty.insert((p.0 + dx, p.1 + dy, p.2 + dz));
                    }
                }
            }
        }
    }

    pub fn is_idle(&self) -> bool {
        self.dirty.is_empty() && self.scheduled.is_empty()
    }

    fn schedule(&mut self, p: P, delay: i32) {
        if !self.scheduled.iter().any(|e| e.0 == p) {
            self.scheduled.push((p, delay));
        }
    }

    fn set(&mut self, lv: &mut Level, p: P, id: u8, data: u8, flags: i32) {
        lv.set_tile_and_data(p.0, p.1, p.2, id, data, flags);
        self.events.push(Event::Changed(p));
    }

    /// One game tick: due delayed updates first, then everything marked dirty.
    pub fn tick(&mut self, lv: &mut Level) {
        let mut due = Vec::new();
        for e in self.scheduled.iter_mut() {
            e.1 -= 1;
            if e.1 <= 0 {
                due.push(e.0);
            }
        }
        self.scheduled.retain(|e| e.1 > 0);
        for p in due {
            self.run_scheduled(lv, p);
        }
        if self.dirty.is_empty() {
            return;
        }
        let mut cells: Vec<P> = std::mem::take(&mut self.dirty).into_iter().collect();
        cells.sort();
        let mut visited = HashSet::new();
        for p in cells {
            let (id, data) = tile(lv, p);
            match id {
                t::RED_STONE_DUST => {
                    if !visited.contains(&p) {
                        self.update_wires(lv, p, &mut visited);
                    }
                }
                t::DIODE_ON | t::DIODE_OFF => {
                    let want = repeater_input(lv, p, data) > 0;
                    if want != (id == t::DIODE_ON) {
                        self.schedule(p, ((data as i32 >> 2) & 3) * 2 + 2);
                    }
                }
                t::REDSTONE_TORCH_ON | t::REDSTONE_TORCH_OFF => {
                    if torch_powered(lv, p, data) == (id == t::REDSTONE_TORCH_ON) {
                        self.schedule(p, 2);
                    }
                }
                t::REDSTONE_LIGHT | t::REDSTONE_LIGHT_LIT => {
                    let on = is_powered(lv, p);
                    if on && id == t::REDSTONE_LIGHT {
                        self.set(lv, p, t::REDSTONE_LIGHT_LIT, 0, UPDATE_ALL);
                    } else if !on && id == t::REDSTONE_LIGHT_LIT {
                        self.schedule(p, 4);
                    }
                }
                t::DOOR_WOOD | t::DOOR_IRON if data & 8 == 0 => {
                    let on = is_powered(lv, p) || is_powered(lv, add(p, UP));
                    self.drive_door(lv, p, id, data, on);
                }
                t::PISTON_BASE | t::PISTON_STICKY_BASE => self.events.push(Event::Piston(p)),
                t::TNT => {
                    if is_powered(lv, p) {
                        self.events.push(Event::Ignite(p));
                    }
                }
                t::TRAPDOOR | t::FENCE_GATE => {
                    let on = is_powered(lv, p);
                    self.drive_openable(lv, p, id, data, on);
                }
                _ => {}
            }
        }
    }

    fn drive_door(&mut self, lv: &mut Level, p: P, id: u8, data: u8, on: bool) {
        let last = self.door_power.insert(p, on).unwrap_or(false);
        if last == on || (data & 4 != 0) == on {
            return;
        }
        let nd = if on { data | 4 } else { data & !4 };
        self.set(lv, p, id, nd, UPDATE_NONE);
        self.events.push(Event::Sound(if on { "random.door_open" } else { "random.door_close" }, p, 1.0, 0.9));
    }

    fn drive_openable(&mut self, lv: &mut Level, p: P, id: u8, data: u8, on: bool) {
        let last = self.door_power.insert(p, on).unwrap_or(false);
        if last == on || (data & 4 != 0) == on {
            return;
        }
        self.set(lv, p, id, if on { data | 4 } else { data & !4 }, UPDATE_NONE);
        self.events.push(Event::Sound(if on { "random.door_open" } else { "random.door_close" }, p, 1.0, 0.9));
    }

    fn run_scheduled(&mut self, lv: &mut Level, p: P) {
        let (id, data) = tile(lv, p);
        match id {
            t::DIODE_ON | t::DIODE_OFF => {
                let want = repeater_input(lv, p, data) > 0;
                if want != (id == t::DIODE_ON) {
                    self.set(lv, p, if want { t::DIODE_ON } else { t::DIODE_OFF }, data, UPDATE_NONE);
                    self.notify(p);
                }
            }
            t::REDSTONE_TORCH_ON | t::REDSTONE_TORCH_OFF => {
                let powered = torch_powered(lv, p, data);
                if powered && id == t::REDSTONE_TORCH_ON {
                    self.set(lv, p, t::REDSTONE_TORCH_OFF, data, UPDATE_ALL);
                    self.events.push(Event::Sound("random.fizz", p, 0.5, 2.6));
                    self.notify(p);
                } else if !powered && id == t::REDSTONE_TORCH_OFF {
                    self.set(lv, p, t::REDSTONE_TORCH_ON, data, UPDATE_ALL);
                    self.notify(p);
                }
            }
            t::REDSTONE_LIGHT_LIT => {
                if !is_powered(lv, p) {
                    self.set(lv, p, t::REDSTONE_LIGHT, 0, UPDATE_ALL);
                }
            }
            _ => {}
        }
    }

    /// Recompute the strength of the whole dust network that touches `start`.
    fn update_wires(&mut self, lv: &mut Level, start: P, visited: &mut HashSet<P>) {
        let wire_at = |lv: &Level, q: P| tile(lv, q).0 == t::RED_STONE_DUST;
        // collect the connected network
        let mut net: Vec<P> = vec![start];
        let mut seen: HashSet<P> = HashSet::from([start]);
        let mut i = 0;
        let mut links: HashMap<P, Vec<P>> = HashMap::new();
        while i < net.len() && net.len() < 800 {
            let p = net[i];
            i += 1;
            let mut ns = Vec::new();
            let above_free = {
                let (a, d) = tile(lv, add(p, UP));
                !is_conductor(a, d)
            };
            for &d in &[0usize, 1, 4, 5] {
                let q = add(p, d);
                let (qid, qd) = tile(lv, q);
                if wire_at(lv, q) {
                    ns.push(q);
                }
                if !is_conductor(qid, qd) && wire_at(lv, add(q, DOWN)) {
                    ns.push(add(q, DOWN));
                }
                if above_free && wire_at(lv, add(q, UP)) {
                    ns.push(add(q, UP));
                }
            }
            for &q in &ns {
                if seen.insert(q) {
                    net.push(q);
                }
            }
            links.insert(p, ns);
        }
        let input: HashMap<P, u8> = net.iter().map(|&p| (p, (0..6).map(|d| sample(lv, p, d, true)).max().unwrap_or(0))).collect();
        let mut val: HashMap<P, u8> = input.clone();
        for _ in 0..64 {
            let mut changed = false;
            for &p in &net {
                let mut m = val[&p];
                if let Some(ns) = links.get(&p) {
                    for q in ns {
                        if let Some(&v) = val.get(q) {
                            m = m.max(v.saturating_sub(1));
                        }
                    }
                }
                if m > val[&p] {
                    val.insert(p, m);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for &p in &net {
            visited.insert(p);
            let (id, data) = tile(lv, p);
            let nv = val[&p].min(15);
            if id == t::RED_STONE_DUST && data & 15 != nv {
                lv.set_tile_and_data(p.0, p.1, p.2, id, nv, UPDATE_NONE);
                self.events.push(Event::Changed(p));
                self.notify(p);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_level::chunk::LevelChunk;

    fn world() -> Level {
        let mut lv = Level::new(54, false);
        lv.lighting = false;
        for cx in -1..2 {
            for cz in -1..2 {
                let mut raw = vec![0u8; 32768];
                for x in 0..16usize {
                    for z in 0..16usize {
                        for y in 0..=3usize {
                            raw[(x << 11) | (z << 7) | y] = t::STONE;
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

    fn put(lv: &mut Level, rs: &mut Redstone, x: i32, y: i32, z: i32, id: u8, d: u8) {
        lv.set_tile_and_data(x, y, z, id, d, UPDATE_NONE);
        rs.notify((x, y, z));
    }

    fn run(lv: &mut Level, rs: &mut Redstone, n: usize) {
        for _ in 0..n {
            rs.tick(lv);
        }
    }

    #[test]
    fn lever_powers_a_dust_line_that_fades_with_distance() {
        let mut lv = world();
        let mut rs = Redstone::default();
        // lever on the floor at x=0, dust along +x on y=4
        put(&mut lv, &mut rs, 0, 4, 0, t::LEVER, 5 | 8);
        for x in 1..=20 {
            put(&mut lv, &mut rs, x, 4, 0, t::RED_STONE_DUST, 0);
        }
        run(&mut lv, &mut rs, 6);
        assert_eq!(lv.get_data(1, 4, 0) & 15, 15);
        assert_eq!(lv.get_data(5, 4, 0) & 15, 11);
        assert_eq!(lv.get_data(16, 4, 0) & 15, 0);
        // switch off
        put(&mut lv, &mut rs, 0, 4, 0, t::LEVER, 5);
        run(&mut lv, &mut rs, 6);
        assert_eq!(lv.get_data(1, 4, 0) & 15, 0);
        assert_eq!(lv.get_data(5, 4, 0) & 15, 0);
    }

    #[test]
    fn dust_lights_a_lamp_and_opens_an_iron_door() {
        let mut lv = world();
        let mut rs = Redstone::default();
        put(&mut lv, &mut rs, 0, 4, 0, t::LEVER, 5 | 8);
        put(&mut lv, &mut rs, 1, 4, 0, t::RED_STONE_DUST, 0);
        put(&mut lv, &mut rs, 2, 4, 0, t::RED_STONE_DUST, 0);
        // a lamp under the dust, an iron door at the dust's end
        put(&mut lv, &mut rs, 2, 3, 0, t::REDSTONE_LIGHT, 0);
        put(&mut lv, &mut rs, 3, 4, 0, t::DOOR_IRON, 0);
        put(&mut lv, &mut rs, 3, 5, 0, t::DOOR_IRON, 8);
        run(&mut lv, &mut rs, 6);
        assert_eq!(lv.get_tile(2, 3, 0), t::REDSTONE_LIGHT_LIT);
        assert_eq!(lv.get_data(3, 4, 0) & 4, 4, "iron door open");
        assert_eq!(lv.get_data(3, 5, 0) & 4, 0, "only the lower half keeps the open bit");
        put(&mut lv, &mut rs, 0, 4, 0, t::LEVER, 5);
        run(&mut lv, &mut rs, 12);
        assert_eq!(lv.get_tile(2, 3, 0), t::REDSTONE_LIGHT);
        assert_eq!(lv.get_data(3, 4, 0) & 4, 0, "closed again");
    }

    #[test]
    fn repeater_delays_and_restores_the_signal() {
        let mut lv = world();
        let mut rs = Redstone::default();
        put(&mut lv, &mut rs, 0, 4, 0, t::LEVER, 5);
        for x in 1..=3 {
            put(&mut lv, &mut rs, x, 4, 0, t::RED_STONE_DUST, 0);
        }
        // repeater at x=4 flowing +x (dir 3), delay setting 1 (data bits 2-3)
        put(&mut lv, &mut rs, 4, 4, 0, t::DIODE_OFF, 3 | (1 << 2));
        for x in 5..=7 {
            put(&mut lv, &mut rs, x, 4, 0, t::RED_STONE_DUST, 0);
        }
        run(&mut lv, &mut rs, 4);
        put(&mut lv, &mut rs, 0, 4, 0, t::LEVER, 5 | 8);
        run(&mut lv, &mut rs, 1);
        assert_eq!(lv.get_tile(4, 4, 0), t::DIODE_OFF, "not yet");
        run(&mut lv, &mut rs, 10);
        assert_eq!(lv.get_tile(4, 4, 0), t::DIODE_ON);
        assert_eq!(lv.get_data(5, 4, 0) & 15, 15, "restored to full strength");
        assert_eq!(lv.get_data(7, 4, 0) & 15, 13);
    }

    #[test]
    fn torch_inverts_a_powered_block() {
        let mut lv = world();
        let mut rs = Redstone::default();
        // block at x=1 powered by a lever on its side; torch on its far side
        put(&mut lv, &mut rs, 1, 4, 0, t::STONE, 0);
        put(&mut lv, &mut rs, 2, 4, 0, t::REDSTONE_TORCH_ON, 1); // attached to x-1
        run(&mut lv, &mut rs, 6);
        assert_eq!(lv.get_tile(2, 4, 0), t::REDSTONE_TORCH_ON);
        put(&mut lv, &mut rs, 1, 4, 1, t::LEVER, 4 | 8); // attached to z+1? (block at z=2 absent) – power block via lever on it
        put(&mut lv, &mut rs, 1, 5, 0, t::LEVER, 6 | 8); // floor lever on top of the block: strongly powers it
        run(&mut lv, &mut rs, 8);
        assert_eq!(lv.get_tile(2, 4, 0), t::REDSTONE_TORCH_OFF);
    }

    #[test]
    fn dust_connections_and_line_axis() {
        let mut lv = world();
        let mut rs = Redstone::default();
        put(&mut lv, &mut rs, 0, 4, 0, t::RED_STONE_DUST, 0);
        put(&mut lv, &mut rs, 1, 4, 0, t::RED_STONE_DUST, 0);
        let nb = |p: P| tile(&lv, p);
        let c = wire_connections(&nb, (0, 4, 0));
        assert_eq!(c, [false, true, false, false]);
        assert_eq!(wire_line(c), Some(0), "dead end points along its line");
        put(&mut lv, &mut rs, 0, 4, 1, t::RED_STONE_DUST, 0);
        let nb = |p: P| tile(&lv, p);
        assert_eq!(wire_line(wire_connections(&nb, (0, 4, 0))), None, "a bend points nowhere");
    }

    #[test]
    fn dust_climbs_a_block() {
        let mut lv = world();
        let mut rs = Redstone::default();
        put(&mut lv, &mut rs, 0, 4, 0, t::LEVER, 5 | 8);
        put(&mut lv, &mut rs, 1, 4, 0, t::RED_STONE_DUST, 0);
        put(&mut lv, &mut rs, 2, 4, 0, t::STONE, 0);
        put(&mut lv, &mut rs, 2, 5, 0, t::RED_STONE_DUST, 0);
        put(&mut lv, &mut rs, 3, 5, 0, t::RED_STONE_DUST, 0);
        run(&mut lv, &mut rs, 6);
        assert_eq!(lv.get_data(2, 5, 0) & 15, 14);
        assert_eq!(lv.get_data(3, 5, 0) & 15, 13);
    }
}
