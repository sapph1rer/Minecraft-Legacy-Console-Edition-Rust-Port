//! `PathFinder` (A*), `Path`, `Node` — the pathfinding used by `PathNavigation`.

use crate::physics::{is_lava, is_water};
use mc_core::aabb::Aabb;
use mc_level::level::Level;
use mc_world::tile_ids as t;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub nodes: Vec<[i32; 3]>,
    pub index: usize,
}

impl Path {
    pub fn is_done(&self) -> bool {
        self.index >= self.nodes.len()
    }
    /// `Path::getPos(entity, i)`
    pub fn pos(&self, i: usize, bb_width: f32) -> [f64; 3] {
        let n = self.nodes[i];
        let o = (bb_width as i32 + 1) as f64 * 0.5;
        [n[0] as f64 + o, n[1] as f64, n[2] as f64 + o]
    }
    pub fn same_as(&self, o: &Path) -> bool {
        self.nodes == o.nodes
    }
    pub fn last(&self) -> Option<[i32; 3]> {
        self.nodes.last().copied()
    }
}

/// What the path finder needs to know about the walker.
#[derive(Clone, Copy)]
pub struct PathEnt {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub bb: Aabb,
    pub w: f32,
    pub h: f32,
    pub in_water: bool,
    pub in_lava: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Blocked,
    Open,
    Walkable,
    Fence,
    Trap,
    Water,
    Lava,
}

struct PNode {
    p: [i32; 3],
    g: f32,
    f: f32,
    came: Option<usize>,
    closed: bool,
    open: bool,
}

pub struct PathFinder<'a> {
    pub lv: &'a Level,
    pub can_pass_doors: bool,
    pub can_open_doors: bool,
    pub avoid_water: bool,
    pub can_float: bool,
    map: HashMap<[i32; 3], usize>,
    nodes: Vec<PNode>,
    heap: BinaryHeap<Reverse<(u32, u32, usize)>>,
    counter: u32,
}

fn dist_sqr(a: [i32; 3], b: [i32; 3]) -> f32 {
    let (x, y, z) = ((b[0] - a[0]) as f32, (b[1] - a[1]) as f32, (b[2] - a[2]) as f32);
    x * x + y * y + z * z
}

/// `Tile::isPathfindable`
fn pathfindable(lv: &Level, id: u8, x: i32, y: i32, z: i32) -> bool {
    if id == t::DOOR_WOOD || id == t::DOOR_IRON {
        // DoorTile::isPathfindable: open doors only (the half that carries the data)
        let mut d = lv.get_data(x, y, z);
        if d & 8 != 0 {
            d = lv.get_data(x, y - 1, z);
        }
        return d & 4 != 0;
    }
    if id == t::TRAPDOOR {
        return lv.get_data(x, y, z) & 4 != 0;
    }
    if is_water(id) {
        return true;
    }
    if is_lava(id) {
        return false;
    }
    !mc_world::tile_props::material(id).blocks_motion()
}

impl<'a> PathFinder<'a> {
    pub fn new(lv: &'a Level, can_pass_doors: bool, can_open_doors: bool, avoid_water: bool, can_float: bool) -> Self {
        PathFinder { lv, can_pass_doors, can_open_doors, avoid_water, can_float, map: HashMap::new(), nodes: Vec::new(), heap: BinaryHeap::new(), counter: 0 }
    }

    fn node(&mut self, p: [i32; 3]) -> usize {
        if let Some(&i) = self.map.get(&p) {
            return i;
        }
        self.nodes.push(PNode { p, g: 0.0, f: 0.0, came: None, closed: false, open: false });
        let i = self.nodes.len() - 1;
        self.map.insert(p, i);
        i
    }

    /// `PathFinder::findPath(entity, xt, yt, zt, maxDist)`
    pub fn find_path(&mut self, e: &PathEnt, xt: f64, yt: f64, zt: f64, max_dist: f32) -> Option<Path> {
        self.map.clear();
        self.nodes.clear();
        self.heap.clear();
        let reset_avoid = self.avoid_water;
        let mut start_y = (e.bb.y0 + 0.5).floor() as i32;
        if self.can_float && e.in_water {
            start_y = e.bb.y0 as i32;
            let mut id = self.lv.get_tile(e.x.floor() as i32, start_y, e.z.floor() as i32);
            while is_water(id) {
                start_y += 1;
                id = self.lv.get_tile(e.x.floor() as i32, start_y, e.z.floor() as i32);
            }
            self.avoid_water = false;
        }
        let from = self.node([e.bb.x0.floor() as i32, start_y, e.bb.z0.floor() as i32]);
        let to = self.node([(xt - e.w as f64 / 2.0).floor() as i32, yt.floor() as i32, (zt - e.w as f64 / 2.0).floor() as i32]);
        let size = [(e.w + 1.0).floor() as i32, (e.h + 1.0).floor() as i32, (e.w + 1.0).floor() as i32];
        let r = self.search(e, from, to, size, max_dist);
        self.avoid_water = reset_avoid;
        r
    }

    fn push(&mut self, i: usize) {
        self.counter += 1;
        self.heap.push(Reverse((self.nodes[i].f.to_bits(), self.counter, i)));
    }

    fn search(&mut self, e: &PathEnt, from: usize, to: usize, size: [i32; 3], max_dist: f32) -> Option<Path> {
        let top = self.nodes[to].p;
        self.nodes[from].g = 0.0;
        self.nodes[from].f = dist_sqr(self.nodes[from].p, top);
        self.nodes[from].open = true;
        self.push(from);
        let mut closest = from;
        let mut iter = 0;
        while let Some(Reverse((fb, _, xi))) = self.heap.pop() {
            if self.nodes[xi].closed || self.nodes[xi].f.to_bits() != fb {
                continue;
            }
            iter += 1;
            if iter > 4000 {
                break;
            }
            self.nodes[xi].open = false;
            if xi == to {
                return Some(self.reconstruct(to));
            }
            if dist_sqr(self.nodes[xi].p, top) < dist_sqr(self.nodes[closest].p, top) {
                closest = xi;
            }
            self.nodes[xi].closed = true;
            for yi in self.neighbors(e, xi, size, top, max_dist) {
                let tg = self.nodes[xi].g + dist_sqr(self.nodes[xi].p, self.nodes[yi].p);
                if !self.nodes[yi].open || tg < self.nodes[yi].g {
                    self.nodes[yi].came = Some(xi);
                    self.nodes[yi].g = tg;
                    self.nodes[yi].f = tg + dist_sqr(self.nodes[yi].p, top);
                    self.nodes[yi].open = true;
                    self.push(yi);
                }
            }
        }
        if closest == from {
            return None;
        }
        Some(self.reconstruct(closest))
    }

    fn neighbors(&mut self, e: &PathEnt, xi: usize, size: [i32; 3], target: [i32; 3], max_dist: f32) -> Vec<usize> {
        let p = self.nodes[xi].p;
        let mut jump = 0;
        if self.is_free(e, [p[0], p[1] + 1, p[2]], size) == Kind::Open {
            jump = 1;
        }
        let mut out = Vec::new();
        for d in [[0, 1], [-1, 0], [1, 0], [0, -1]] {
            if let Some(n) = self.get_node(e, [p[0] + d[0], p[1], p[2] + d[1]], size, jump) {
                if !self.nodes[n].closed && dist_sqr(self.nodes[n].p, target).sqrt() < max_dist {
                    out.push(n);
                }
            }
        }
        out
    }

    fn get_node(&mut self, e: &PathEnt, mut p: [i32; 3], size: [i32; 3], jump: i32) -> Option<usize> {
        let mut best = None;
        let kind = self.is_free(e, p, size);
        if kind == Kind::Walkable {
            return Some(self.node(p));
        }
        if kind == Kind::Open {
            best = Some(self.node(p));
        }
        if best.is_none() && jump > 0 && kind != Kind::Fence && kind != Kind::Trap && self.is_free(e, [p[0], p[1] + jump, p[2]], size) == Kind::Open {
            p[1] += jump;
            best = Some(self.node(p));
        }
        if let Some(mut b) = best {
            let mut drop = 0;
            let mut cost = Kind::Open;
            while p[1] > 0 {
                cost = self.is_free(e, [p[0], p[1] - 1, p[2]], size);
                if self.avoid_water && cost == Kind::Water {
                    return None;
                }
                if cost != Kind::Open {
                    break;
                }
                drop += 1;
                if drop >= 4 {
                    return None;
                }
                p[1] -= 1;
                if p[1] > 0 {
                    b = self.node(p);
                }
            }
            if cost == Kind::Lava {
                return None;
            }
            return Some(b);
        }
        None
    }

    /// `PathFinder::isFree`
    pub fn is_free(&self, e: &PathEnt, p: [i32; 3], size: [i32; 3]) -> Kind {
        let lv = self.lv;
        let mut walkable = false;
        for xx in p[0]..p[0] + size[0] {
            for yy in p[1]..p[1] + size[1] {
                for zz in p[2]..p[2] + size[2] {
                    let id = lv.get_tile(xx, yy, zz);
                    if id == 0 {
                        continue;
                    }
                    if id == t::TRAPDOOR {
                        walkable = true;
                    } else if is_water(id) {
                        if self.avoid_water {
                            return Kind::Water;
                        }
                        walkable = true;
                    } else if !self.can_pass_doors && id == t::DOOR_WOOD {
                        return Kind::Blocked;
                    }
                    if pathfindable(lv, id, xx, yy, zz) {
                        continue;
                    }
                    if self.can_open_doors && id == t::DOOR_WOOD {
                        continue;
                    }
                    if id == t::FENCE || id == t::FENCE_GATE || id == 139 {
                        return Kind::Fence;
                    }
                    if id == t::TRAPDOOR {
                        return Kind::Trap;
                    }
                    if is_lava(id) {
                        if e.in_lava {
                            continue;
                        }
                        return Kind::Lava;
                    }
                    return Kind::Blocked;
                }
            }
        }
        if walkable {
            Kind::Walkable
        } else {
            Kind::Open
        }
    }

    fn reconstruct(&self, to: usize) -> Path {
        let mut v = vec![self.nodes[to].p];
        let mut n = to;
        while let Some(c) = self.nodes[n].came {
            v.push(self.nodes[c].p);
            n = c;
        }
        v.reverse();
        Path { nodes: v, index: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{generate, Dim};

    #[test]
    fn finds_a_straight_path_on_flat_ground() {
        let lv = generate(Dim::Flat, 1, 3);
        let ground = (0..128).rev().find(|&y| lv.get_tile(0, y, 0) != 0).unwrap() + 1;
        let e = PathEnt { x: 0.5, y: ground as f64, z: 0.5, bb: Aabb::new(0.2, ground as f64, 0.2, 0.8, ground as f64 + 1.8, 0.8), w: 0.6, h: 1.8, in_water: false, in_lava: false };
        let mut pf = PathFinder::new(&lv, true, false, false, false);
        let p = pf.find_path(&e, 6.5, ground as f64, 0.5, 16.0).expect("path");
        assert!(p.nodes.len() >= 6, "{:?}", p.nodes);
        let l = p.last().unwrap();
        assert!((l[0] - 6).abs() <= 1 && l[2].abs() <= 1);
    }
}
