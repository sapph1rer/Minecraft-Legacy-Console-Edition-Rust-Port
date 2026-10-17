//! Chests: the boxed model, the lid that swings open (`ChestTileEntity` openness) and the set of chests whose
//! lid is currently drawn by the entity layer instead of the chunk mesh.

use crate::shapes::{Aabb3, Part};
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

type P = (i32, i32, i32);

fn animating() -> &'static Mutex<HashSet<P>> {
    static S: OnceLock<Mutex<HashSet<P>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashSet::new()))
}

pub fn set_animating(p: P, on: bool) {
    if let Ok(mut s) = animating().lock() {
        if on {
            s.insert(p);
        } else {
            s.remove(&p);
        }
    }
}

pub fn is_animating(p: P) -> bool {
    animating().lock().map_or(false, |s| s.contains(&p))
}

/// Rotation (about the block's vertical centre) from "front = -z" to the chest's facing (data 2 N, 3 S, 4 W, 5 E).
pub fn facing_turn(data: u8) -> i32 {
    match data & 7 {
        5 => -90,
        3 => 180,
        4 => 90,
        _ => 0,
    }
}

fn turn_box(b: Aabb3, deg: i32) -> Aabb3 {
    let f = |x: f32, z: f32| -> (f32, f32) {
        let (a, c) = (x - 0.5, z - 0.5);
        let (a2, c2) = match deg {
            90 => (c, -a),
            -90 => (-c, a),
            180 => (-a, -c),
            _ => (a, c),
        };
        (a2 + 0.5, c2 + 0.5)
    };
    let (p, q) = (f(b.0[0], b.0[2]), f(b.1[0], b.1[2]));
    ([p.0.min(q.0), b.0[1], p.1.min(q.1)], [p.0.max(q.0), b.1[1], p.1.max(q.1)])
}

/// Atlas cells (row, column) of the chest faces: lid top / lid bottom, base top (the dark inside) / base bottom,
/// the four wall cells and nothing else (the lock is part of the front texture).
#[derive(Clone, Copy)]
pub struct ChestCells {
    pub lid_top: (u8, u8),
    pub lid_bottom: (u8, u8),
    pub base_top: (u8, u8),
    pub base_bottom: (u8, u8),
    pub side: (u8, u8),
    pub front: (u8, u8),
    pub back: (u8, u8),
    pub lock: (u8, u8),
}

impl ChestCells {
    pub fn lookup() -> Self {
        let c = |n: &str, d: (u8, u8)| crate::icons::icon(n).unwrap_or(d);
        ChestCells {
            lid_top: c("chest_top", (31, 0)),
            side: c("chest_side", (31, 1)),
            front: c("chest_front", (31, 2)),
            back: c("chest_back", (31, 4)),
            lock: c("chest_lock", (31, 3)),
            lid_bottom: c("chest_lid_bottom", (31, 5)),
            base_top: c("chest_base_top", (31, 6)),
            base_bottom: c("chest_base_bottom", (31, 7)),
        }
    }
}

/// The neighbouring chest this one is joined to (`ChestTile::updateShape` order: -z, +z, -x, +x), as a (dx, dz) step.
pub fn partner(id: u8, x: i32, y: i32, z: i32, nb: crate::shapes::Nb) -> Option<(i32, i32)> {
    [(0, -1), (0, 1), (-1, 0), (1, 0)].into_iter().find(|&(dx, dz)| nb(x + dx, y, z + dz).0 == id)
}

/// A world step seen from the chest's own frame (front = -z).
pub fn to_local(deg: i32, (wx, wz): (i32, i32)) -> (i32, i32) {
    match deg {
        90 => (-wz, wx),
        -90 => (wz, -wx),
        180 => (-wx, -wz),
        _ => (wx, wz),
    }
}

/// Atlas cell of a half-chest face: `kind` 0 lid top, 1 lid bottom, 2 base top, 3 base bottom, 4 front, 5 back;
/// `variant` 0 / 1 = joint at the left / right edge of the cell, 2 / 3 = joint at its top / bottom edge.
pub fn half_cell(kind: usize, variant: usize) -> (u8, u8) {
    let i = kind * 4 + variant;
    (28 + (i / 12) as u8, (i % 12) as u8)
}

/// Chest boxes (`ChestModel`): the 14 x 10 x 14 base, plus the 14 x 5 x 14 lid when `lid` (it overlaps the base by one pixel).
/// `partner` is the world step to a joined chest: this half then reaches to the shared edge and carries half the lock.
pub fn chest_parts(data: u8, lid: bool, cl: ChestCells, partner: Option<(i32, i32)>) -> Vec<Part> {
    let s = 1.0f32 / 16.0;
    let deg = facing_turn(data);
    // face index of the front after turning: -z, +x, +z, -x
    let front = match deg {
        -90 => 1,
        180 => 5,
        90 => 0,
        _ => 4,
    };
    let back = front ^ 1;
    // the joint must lie along the chest's width (facing keeps it that way); `ls` = side of the partner in the local x
    let ls = partner.and_then(|w| {
        let (lx, lz) = to_local(deg, w);
        (lz == 0).then_some(lx)
    });
    let mk = |mn: [f32; 3], mx: [f32; 3], cells: [(u8, u8); 6]| -> Part {
        let (mn2, mx2) = turn_box((mn, mx), deg);
        Part { min: mn2, max: mx2, cells: Some(cells), rot: [0; 6] }
    };
    let mut walls = [cl.side; 6];
    walls[front] = cl.front;
    walls[back] = cl.back;
    let (x0, x1) = match ls {
        Some(1) => (s, 1.0),
        Some(_) => (0.0, 15.0 * s),
        None => (s, 15.0 * s),
    };
    // the faces that run along the joint get the cells with the shared edge left open
    let joined = |kinds: [Option<usize>; 6], mut cells: [(u8, u8); 6]| -> [(u8, u8); 6] {
        let (Some(ls), Some((wx, wz))) = (ls, partner) else { return cells };
        let (axis, sgn) = if wx != 0 { (0, wx) } else { (2, wz) };
        let _ = ls;
        for f in 0..6 {
            let Some(k) = kinds[f] else { continue };
            // (axis, sign) of the texture u direction on this face, and of v for the horizontal faces
            let (ua, us) = match f {
                0 => (2, 1),
                1 => (2, -1),
                4 => (0, -1),
                _ => (0, 1),
            };
            let variant = if f == 2 || f == 3 {
                if axis == 0 {
                    if sgn > 0 { 1 } else { 0 }
                } else {
                    let vs = if f == 3 { 1 } else { -1 };
                    if sgn * vs > 0 { 3 } else { 2 }
                }
            } else if ua == axis {
                if sgn * us > 0 { 1 } else { 0 }
            } else {
                continue;
            };
            cells[f] = half_cell(k, variant);
        }
        cells
    };
    let mut bw = walls;
    bw[2] = cl.base_bottom;
    bw[3] = cl.base_top;
    let mut kinds = [None; 6];
    kinds[front] = Some(4);
    kinds[back] = Some(5);
    let mut bk = kinds;
    bk[2] = Some(3);
    bk[3] = Some(2);
    let mut v = vec![mk([x0, 0.0, s], [x1, 10.0 * s, 15.0 * s], joined(bk, bw))];
    if lid {
        let mut lw = walls;
        lw[2] = cl.lid_bottom;
        lw[3] = cl.lid_top;
        let mut lk = kinds;
        lk[2] = Some(1);
        lk[3] = Some(0);
        v.push(mk([x0, 9.0 * s, s], [x1, 14.0 * s, 15.0 * s], joined(lk, lw)));
        // the lock (texture offset 0,0: 2 x 4 x 1) on the front, across the lid seam; a double chest has it on the joint
        let (l0, l1) = match ls {
            Some(1) => (15.0 * s, 1.0),
            Some(_) => (0.0, s),
            None => (7.0 * s, 9.0 * s),
        };
        v.push(mk([l0, 7.0 * s, 0.0], [l1, 11.0 * s, s], [cl.lock; 6]));
    }
    v
}

/// Paints the chest faces from the shipped `chest.png` (64 x 64, embedded as raw RGBA) into free cells of the
/// terrain atlas, row 31: 0 lid top, 1 side, 2 front, 4 back, 5 lid bottom, 6 base top, 7 base bottom. The 14-pixel
/// boxes sample columns 1..15 of a cell; the wall cells stack the lid rows (5) over the base rows 1..9.
pub fn paint_icons(atlas: &mut [u8], aw: u32) {
    const TEX: &[u8] = include_bytes!("chest_tex.rgba");
    let tex = |x: u32, y: u32| -> [u8; 4] {
        let i = ((y * 64 + x) * 4) as usize;
        [TEX[i], TEX[i + 1], TEX[i + 2], TEX[i + 3]]
    };
    // experience orb (cell 9): a soft round glow, tinted green-yellow at draw time
    for y in 0..16u32 {
        for x in 0..16u32 {
            let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
            let r = (dx * dx + dy * dy).sqrt() / 7.5;
            let j = (((31 * 16 + y) * aw + 9 * 16 + x) * 4) as usize;
            if r <= 1.0 && j + 3 < atlas.len() {
                let l = (255.0 * (1.0 - 0.45 * r * r)) as u8;
                atlas[j..j + 4].copy_from_slice(&[l, l, l, 255]);
            } else if j + 3 < atlas.len() {
                atlas[j..j + 4].copy_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    // piston arm (`TileRenderer::renderPistonArm*`): the top four rows of piston_side, laid along the rod. Cell rows
    // 0..3 are the stub in the base cell (arm pixels 3..0), rows 4..15 the part in the head cell (pixels 15..4).
    for r in 0..16u32 {
        let u = if r < 4 { 3 - r } else { 19 - r };
        for x in 6..10u32 {
            let i = (((6 * 16 + (x - 6)) * aw + 12 * 16 + u) * 4) as usize;
            if i + 3 < atlas.len() {
                let c = [atlas[i], atlas[i + 1], atlas[i + 2], atlas[i + 3]];
                let j = (((31 * 16 + r) * aw + 8 * 16 + x) * 4) as usize;
                atlas[j..j + 4].copy_from_slice(&c);
            }
        }
    }
    let mut put = |cell: u32, x: u32, y: u32, c: [u8; 4]| {
        let (px, py) = (cell * 16 + x, 31 * 16 + y);
        let i = ((py * aw + px) * 4) as usize;
        if i + 3 < atlas.len() {
            atlas[i..i + 4].copy_from_slice(&c);
        }
    };
    for y in 0..16 {
        for x in 0..16 {
            put(3, x, y, tex(1 + x % 2, 1 + y % 4));
        }
    }
    // flat 14 x 14 faces: (cell, source x, source y)
    // `ChestModel`: lid texOffs (0, 0), base texOffs (0, 19); top at (14, y), bottom at (28, y)
    for (cell, sx, sy) in [(0u32, 14u32, 0u32), (5, 28, 0), (6, 14, 19), (7, 28, 19)] {
        for y in 0..14 {
            for x in 0..14 {
                put(cell, x + 1, y + 1, tex(sx + x, sy + y));
            }
        }
    }
    // walls: tile column 0..3 of the lid row (y 14..19) and base row (y 33..43)
    for (cell, tile) in [(1u32, 2u32), (2, 1), (4, 3)] {
        for x in 0..14 {
            for r in 0..5 {
                put(cell, x + 1, 2 + r, tex(tile * 14 + x, 14 + r));
            }
            for r in 1..10 {
                put(cell, x + 1, 6 + r, tex(tile * 14 + x, 33 + r));
            }
        }
    }
}

/// Half-chest cells (`half_cell`), cut from the shipped `largechest.png` (128 x 64, `LargeChestModel`: 30 pixel wide
/// faces). A half is 15 pixels of the width: variant 0 / 1 = joint at the left / right edge of the cell (the cell shows the
/// far / near half of the 30 pixels, the outer end carrying the rim), 2 / 3 = the same along the cell's vertical axis (the
/// face is turned a quarter, used by the top and bottom faces of a chest facing east / west). Run after `paint_icons`.
pub fn paint_half_cells(atlas: &mut [u8], aw: u32) {
    const TEX: &[u8] = include_bytes!("largechest_tex.rgba");
    let tex = |x: i32, y: i32| -> [u8; 4] {
        let i = ((y * 128 + x) * 4) as usize;
        [TEX[i], TEX[i + 1], TEX[i + 2], TEX[i + 3]]
    };
    // index along the 30 pixel width for a cell coordinate `c`, by variant (None = outside the box)
    let wide = |variant: usize, c: i32| -> Option<i32> {
        let w = if variant & 1 == 0 { c + 15 } else { c - 1 };
        let ok = if variant & 1 == 0 { (0..15).contains(&c) } else { (1..16).contains(&c) };
        ok.then_some(w)
    };
    for variant in 0..4usize {
        let mut put = |cell: (u8, u8), x: i32, y: i32, c: [u8; 4]| {
            let i = ((((cell.0 as u32) * 16 + y as u32) * aw + (cell.1 as u32) * 16 + x as u32) * 4) as usize;
            if i + 3 < atlas.len() {
                atlas[i..i + 4].copy_from_slice(&c);
            }
        };
        // flat faces: 30 x 14 at lid (14, 0) / (44, 0) and base (14, 19) / (44, 19)
        for (kind, sx, sy) in [(0usize, 14, 0), (1, 44, 0), (2, 14, 19), (3, 44, 19)] {
            let cell = half_cell(kind, variant);
            for y in 0..16i32 {
                for x in 0..16i32 {
                    // (along the width, along the depth)
                    let (wc, dc) = if variant < 2 { (x, y) } else { (y, x) };
                    let px = match (wide(variant, wc), (1..15).contains(&dc)) {
                        (Some(w), true) => tex(sx + w, sy + dc - 1),
                        _ => [0, 0, 0, 0],
                    };
                    put(cell, x, y, px);
                }
            }
        }
        // walls: the front / back are 30 wide, the lid rows (y 14..19) over the base rows (y 33..43), as in `paint_icons`
        for (kind, sx) in [(4usize, 14), (5, 58)] {
            let cell = half_cell(kind, variant);
            for y in 0..16i32 {
                for x in 0..16i32 {
                    let w = wide(variant & 1, x);
                    let px = match w {
                        Some(w) if (2..7).contains(&y) => tex(sx + w, 14 + (y - 2)),
                        Some(w) if (7..16).contains(&y) => tex(sx + w, 33 + (y - 6)),
                        _ => [0, 0, 0, 0],
                    };
                    put(cell, x, y, px);
                }
            }
        }
    }
}

/// The eight generic particle tiles of `particles.png` (row 0), 2x upscaled, go into atlas row 30 (cols 0..7).
pub fn paint_particle_tiles(atlas: &mut [u8], aw: u32) {
    const T: &[u8] = include_bytes!("particle_tiles.rgba");
    for y in 0..16u32 {
        for x in 0..128u32 {
            let j = (((30 * 16 + y) * aw + x) * 4) as usize;
            let i = ((y * 128 + x) * 4) as usize;
            if j + 3 < atlas.len() {
                atlas[j..j + 4].copy_from_slice(&T[i..i + 4]);
            }
        }
    }
}

/// Lid angle for an openness in 0..=1 (`ChestRenderer`: 1 - (1 - o)^3, then a quarter turn).
pub fn lid_angle(openness: f32) -> f32 {
    let o = 1.0 - openness;
    (1.0 - o * o * o) * std::f32::consts::FRAC_PI_2
}

/// A chest being animated: openness and the previous value.
#[derive(Clone, Copy, Debug, Default)]
pub struct ChestAnim {
    pub open: f32,
    pub prev: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entrender::chest_layers;
    use mc_level::level::Level;
    use mc_world::tile_ids as t;
    use std::collections::HashMap;

    fn bounds(data: u8) -> ([f32; 3], [f32; 3]) {
        let mut lv = Level::new(54, false);
        lv.lighting = false;
        let raw = vec![0u8; 32768];
        let mut c = mc_level::chunk::LevelChunk::from_blocks(0, 0, &raw);
        c.recalc_heightmap(false);
        lv.insert_chunk(c);
        lv.set_tile_and_data(0, 1, 0, t::CHEST, data, 0);
        lv.set_data(0, 1, 0, data, 0);
        let mut m = HashMap::new();
        m.insert((0, 1, 0), ChestAnim { open: 0.0, prev: 0.0 });
        let l = chest_layers(&m, &lv, 0.0);
        let (mut mn, mut mx) = ([9.0f32; 3], [-9.0f32; 3]);
        for (_, md) in l {
            for v in md.vertices {
                for k in 0..3 {
                    let o = if k == 1 { 1.0 } else { 0.0 };
                    mn[k] = mn[k].min(v.pos[k] - o);
                    mx[k] = mx[k].max(v.pos[k] - o);
                }
            }
        }
        (mn, mx)
    }

    #[test]
    fn closed_lid_matches_the_static_model() {
        for data in [2u8, 3, 4, 5] {
            let (mn, mx) = bounds(data);
            let st = chest_parts(data, true, ChestCells::lookup(), None);
            let (mut smn, mut smx) = ([9.0f32; 3], [-9.0f32; 3]);
            for p in st.iter().skip(1) {
                for k in 0..3 {
                    smn[k] = smn[k].min(p.min[k]);
                    smx[k] = smx[k].max(p.max[k]);
                }
            }
            for k in 0..3 {
                assert!((mn[k] - smn[k]).abs() < 1e-4 && (mx[k] - smx[k]).abs() < 1e-4, "data {data}: {mn:?} {mx:?} vs {smn:?} {smx:?}");
            }
        }
    }
}

#[cfg(test)]
mod sign_tests {
    use crate::entrender::sign_layers;
    use mc_level::level::Level;
    use mc_level::tile_entity::TileEntity;
    use mc_world::tile_ids as t;

    #[test]
    fn standing_sign_model_sits_in_its_cell_for_every_rotation() {
        let mut lv = Level::new(54, false);
        lv.lighting = false;
        let raw = vec![0u8; 32768];
        let mut c = mc_level::chunk::LevelChunk::from_blocks(0, 0, &raw);
        c.recalc_heightmap(false);
        lv.insert_chunk(c);
        for rot in 0..16u8 {
            lv.set_tile_and_data(4, 5, 4, t::SIGN, rot, 0);
            lv.tile_entities.insert((4, 5, 4), TileEntity::Sign { lines: ["hi".into(), String::new(), String::new(), String::new()] });
            let layers = sign_layers(&lv, &[6u8; 256], (128.0, 128.0), glam::Vec3::new(4.5, 6.0, 8.0));
            assert_eq!(layers.len(), 2);
            for (_, m) in layers {
                for v in m.vertices {
                    assert!((3.4..5.6).contains(&v.pos[0]) && (5.0..6.2).contains(&v.pos[1]) && (3.4..5.6).contains(&v.pos[2]), "rot {rot}: {:?}", v.pos);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------- game side

use crate::game::Game;
use mc_level::level::UPDATE_ALL;
use mc_world::tile_ids as t;

const STEPS: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

impl Game {
    fn chest_at(&self, x: i32, y: i32, z: i32) -> bool {
        self.level.get_tile(x, y, z) == t::CHEST
    }

    fn is_full_chest(&self, x: i32, y: i32, z: i32) -> bool {
        self.chest_at(x, y, z) && STEPS.iter().any(|&(dx, dz)| self.chest_at(x + dx, y, z + dz))
    }

    /// `ChestTile::mayPlace`: at most one neighbouring chest, and that one must not be part of a pair already.
    pub fn chest_may_place(&self, x: i32, y: i32, z: i32) -> bool {
        let n = STEPS.iter().filter(|&&(dx, dz)| self.chest_at(x + dx, y, z + dz)).count();
        n <= 1 && !STEPS.iter().any(|&(dx, dz)| self.is_full_chest(x + dx, y, z + dz))
    }

    /// `ChestTile::recalcLockDir`: the facing a chest guesses from the walls around it and its partner.
    fn chest_recalc_lock_dir(&mut self, x: i32, y: i32, z: i32) {
        let id = t::CHEST;
        let tile = |g: &Game, x: i32, y: i32, z: i32| g.level.get_tile(x, y, z);
        let solid = |g: &Game, x: i32, y: i32, z: i32| g.level.get_tile(x, y, z) != 0 && crate::blocks::style(g.level.get_tile(x, y, z), 0).opaque;
        let (n, s, w, e) = (tile(self, x, y, z - 1), tile(self, x, y, z + 1), tile(self, x - 1, y, z), tile(self, x + 1, y, z));
        let sol = |g: &Game, dx: i32, dz: i32| solid(g, x + dx, y, z + dz);
        let lock;
        if n == id || s == id {
            let oz = if n == id { -1 } else { 1 };
            let (w2, e2) = (sol(self, -1, oz), sol(self, 1, oz));
            let mut l = 5;
            let other = self.level.get_data(x, y, z + oz);
            if other == 4 {
                l = 4;
            }
            let (ws, es) = (sol(self, -1, 0), sol(self, 1, 0));
            if (ws || w2) && !es && !e2 {
                l = 5;
            }
            if (es || e2) && !ws && !w2 {
                l = 4;
            }
            lock = l;
        } else if w == id || e == id {
            let ox = if w == id { -1 } else { 1 };
            let (n2, s2) = (sol(self, ox, -1), sol(self, ox, 1));
            let mut l = 3;
            let other = self.level.get_data(x + ox, y, z);
            if other == 2 {
                l = 2;
            }
            let (ns, ss) = (sol(self, 0, -1), sol(self, 0, 1));
            if (ns || n2) && !ss && !s2 {
                l = 3;
            }
            if (ss || s2) && !ns && !n2 {
                l = 2;
            }
            lock = l;
        } else {
            let (ns, ss, ws, es) = (sol(self, 0, -1), sol(self, 0, 1), sol(self, -1, 0), sol(self, 1, 0));
            let mut l = 3;
            if ns && !ss {
                l = 3;
            }
            if ss && !ns {
                l = 2;
            }
            if ws && !es {
                l = 5;
            }
            if es && !ws {
                l = 4;
            }
            lock = l;
        }
        self.level.set_data(x, y, z, lock, UPDATE_ALL);
    }

    /// `ChestTile::onPlace` followed by `setPlacedBy`: neighbours re-guess their facing, then the placed chest takes the
    /// player's facing (and drags its partner along when the pair would otherwise point the wrong way).
    pub fn chest_placed(&mut self, x: i32, y: i32, z: i32) {
        self.chest_recalc_lock_dir(x, y, z);
        for (dx, dz) in STEPS {
            if self.chest_at(x + dx, y, z + dz) {
                self.chest_recalc_lock_dir(x + dx, y, z + dz);
            }
        }
        let facing = [2u8, 5, 3, 4][self.facing4() as usize];
        let (n, s, w, e) = (self.chest_at(x, y, z - 1), self.chest_at(x, y, z + 1), self.chest_at(x - 1, y, z), self.chest_at(x + 1, y, z));
        if !(n || s || w || e) {
            self.level.set_data(x, y, z, facing, UPDATE_ALL);
        } else {
            if (n || s) && (facing == 4 || facing == 5) {
                self.level.set_data(x, y, if n { z - 1 } else { z + 1 }, facing, UPDATE_ALL);
                self.level.set_data(x, y, z, facing, UPDATE_ALL);
            }
            if (w || e) && (facing == 2 || facing == 3) {
                self.level.set_data(if w { x - 1 } else { x + 1 }, y, z, facing, UPDATE_ALL);
                self.level.set_data(x, y, z, facing, UPDATE_ALL);
            }
        }
        for (dx, dz) in STEPS {
            if self.chest_at(x + dx, y, z + dz) {
                self.mark(x + dx, z + dz);
            }
        }
    }

    fn chest_blocked(&self, x: i32, y: i32, z: i32) -> bool {
        crate::mesh::full_cube(&self.level, x, y + 1, z)
    }

    /// `ChestTile::getContainer`: the chests that make up the container opened at (x, y, z) in slot order (a pair is
    /// `-x`/`-z` chest first), or none when something solid sits on top of any of them.
    pub fn chest_group_at(&self, x: i32, y: i32, z: i32) -> Option<Vec<P>> {
        if !self.chest_at(x, y, z) || self.chest_blocked(x, y, z) {
            return None;
        }
        let mut group = vec![(x, y, z)];
        for (dx, dz) in STEPS {
            if self.chest_at(x + dx, y, z + dz) {
                if self.chest_blocked(x + dx, y, z + dz) {
                    return None;
                }
                if dx < 0 || dz < 0 {
                    group.insert(0, (x + dx, y, z + dz));
                } else {
                    group.push((x + dx, y, z + dz));
                }
            }
        }
        Some(group)
    }

    /// Opens the chest screen's container; false when blocked.
    pub fn open_chest(&mut self, pos: P) -> bool {
        let Some(group) = self.chest_group_at(pos.0, pos.1, pos.2) else { return false };
        for &p in &group {
            self.chest_anim.entry(p).or_default();
            set_animating(p, true);
            self.mark(p.0, p.2);
        }
        self.chest_pos = Some(pos);
        self.chest_group = group;
        true
    }

    /// The slots of the open container: 27 per chest, in group order.
    pub fn chest_slots(&mut self) -> Vec<Option<mc_level::tile_entity::ItemStack>> {
        let mut v = Vec::new();
        for p in self.chest_group.clone() {
            match self.level.tile_entity_mut(p.0, p.1, p.2) {
                Some(mc_level::tile_entity::TileEntity::Chest { items }) => {
                    let mut it = items.clone();
                    it.resize(27, None);
                    v.extend(it);
                }
                _ => v.extend(std::iter::repeat(None).take(27)),
            }
        }
        if v.is_empty() {
            v = vec![None; 27];
        }
        v
    }

    /// Writes the container back and closes it.
    pub fn close_chest(&mut self, contents: Vec<Option<mc_level::tile_entity::ItemStack>>) {
        let group = std::mem::take(&mut self.chest_group);
        for (i, p) in group.iter().enumerate() {
            if let Some(mc_level::tile_entity::TileEntity::Chest { items }) = self.level.tile_entity_mut(p.0, p.1, p.2) {
                *items = contents.iter().skip(i * 27).take(27).cloned().collect();
            }
        }
        if let Some(p) = self.chest_pos.take() {
            let (px, py, pz) = (p.0 as f64 + 0.5, p.1 as f64 + 0.5, p.2 as f64 + 0.5);
            self.play("random.chestclosed", px, py, pz, 0.5, 0.9);
        }
    }
}

#[cfg(test)]
mod game_tests {
    use crate::game::Game;
    use crate::physics::Input;
    use crate::world::{generate, Dim};
    use mc_level::level::UPDATE_ALL;
    use mc_world::tile_ids as t;

    fn game() -> Game {
        let lv = generate(Dim::Flat, 1, 3);
        let mut g = Game::new(lv, Dim::Flat, [0.5, 4.0, 0.5]);
        for _ in 0..5 {
            g.tick(&Input::default());
        }
        g
    }

    #[test]
    fn two_chests_make_one_large_container() {
        let mut g = game();
        let y = 4;
        g.level.set_tile_and_data(10, y, 10, t::CHEST, 2, UPDATE_ALL);
        g.level.set_tile_and_data(11, y, 10, t::CHEST, 2, UPDATE_ALL);
        g.chest_placed(11, y, 10);
        assert!(g.open_chest((11, y, 10)));
        // the -x chest comes first whichever one was clicked
        assert_eq!(g.chest_group, vec![(10, y, 10), (11, y, 10)]);
        let mut slots = g.chest_slots();
        assert_eq!(slots.len(), 54);
        slots[0] = Some(mc_level::tile_entity::ItemStack { id: 1, count: 3, damage: 0, enchantments: vec![] });
        slots[30] = Some(mc_level::tile_entity::ItemStack { id: 2, count: 4, damage: 0, enchantments: vec![] });
        g.close_chest(slots);
        assert!(g.chest_group.is_empty());
        let items = |g: &mut Game, x: i32| match g.level.tile_entity_mut(x, y, 10) {
            Some(mc_level::tile_entity::TileEntity::Chest { items }) => items.clone(),
            _ => panic!("no chest at {x}"),
        };
        assert_eq!(items(&mut g, 10)[0].as_ref().map(|s| s.id), Some(1));
        assert_eq!(items(&mut g, 11)[3].as_ref().map(|s| s.id), Some(2));
    }

    #[test]
    fn a_third_chest_cannot_join_a_pair_and_a_block_on_top_blocks_opening() {
        let mut g = game();
        let y = 4;
        g.level.set_tile_and_data(10, y, 10, t::CHEST, 2, UPDATE_ALL);
        g.level.set_tile_and_data(11, y, 10, t::CHEST, 2, UPDATE_ALL);
        assert!(!g.chest_may_place(12, y, 10), "next to a full chest");
        assert!(g.chest_may_place(20, y, 20));
        g.level.set_tile_and_data(11, y + 1, 10, t::STONE, 0, UPDATE_ALL);
        assert!(g.chest_group_at(10, y, 10).is_none(), "stone over the partner");
    }

    #[test]
    fn levers_and_buttons_do_not_stick_to_chests() {
        let mut g = game();
        let y = 4;
        g.level.set_tile_and_data(10, y, 10, t::CHEST, 2, UPDATE_ALL);
        g.level.set_tile_and_data(10, y, 12, t::STONE, 0, UPDATE_ALL);
        for id in [t::LEVER, t::BUTTON_STONE] {
            // click the south face of the chest (face 5), then the north face of the stone (face 4)
            assert!(g.oriented(id, 0, 5, (10, y, 11)).is_none(), "chest is no support");
            assert!(g.oriented(id, 0, 4, (10, y, 11)).is_some(), "stone is");
        }
    }
}
