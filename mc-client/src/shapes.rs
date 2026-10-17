//! Blocks made of several boxes (stairs, doors, trapdoors, fence gates, signs, levers, buttons, plates, repeaters, cake).
//! Boxes follow `Tile::setShape` / `updateDefaultShape` of the matching C++ tiles.

use crate::blocks::Cell;
use crate::icons::icon;
use mc_world::tile_ids as t;

pub type Aabb3 = ([f32; 3], [f32; 3]);

#[derive(Clone, Copy, Debug)]
pub struct Part {
    pub min: [f32; 3],
    pub max: [f32; 3],
    /// per-part textures (else the block style's)
    pub cells: Option<[Cell; 6]>,
    /// texture quarter turns per face
    pub rot: [u8; 6],
}

pub fn is_shaped(id: u8) -> bool {
    matches!(
        id,
        t::STAIRS_WOOD
            | t::STAIRS_STONE
            | t::STAIRS_BRICKS
            | t::STAIRS_STONE_BRICK
            | t::STAIRS_NETHER_BRICKS
            | t::STAIRS_SANDSTONE
            | t::STAIRS_SPRUCEWOOD
            | t::STAIRS_BIRCHWOOD
            | t::STAIRS_JUNGLEWOOD
            | t::STAIRS_QUARTZ
            | t::DOOR_WOOD
            | t::DOOR_IRON
            | t::TRAPDOOR
            | t::FENCE_GATE
            | t::SIGN
            | t::WALL_SIGN
            | t::LEVER
            | t::BUTTON_STONE
            | t::BUTTON_WOOD
            | t::PRESSURE_PLATE_STONE
            | t::PRESSURE_PLATE_WOOD
            | t::DIODE_OFF
            | t::DIODE_ON
            | t::CAKE
            | t::FENCE
            | t::NETHER_FENCE
            | t::ANVIL
            | t::PISTON_EXTENSION_PIECE
    )
}

/// Shaped for this metadata (an extended piston base is a shortened box).
pub fn is_shaped_data(id: u8, data: u8) -> bool {
    is_shaped(id) || id == t::CHEST || (matches!(id, t::PISTON_BASE | t::PISTON_STICKY_BASE) && data & 8 != 0)
}

impl Part {
    pub fn min_max(&self) -> Aabb3 {
        (self.min, self.max)
    }
}

pub fn is_stairs(id: u8) -> bool {
    matches!(id, t::STAIRS_WOOD | t::STAIRS_STONE | t::STAIRS_BRICKS | t::STAIRS_STONE_BRICK | t::STAIRS_NETHER_BRICKS | t::STAIRS_SANDSTONE | t::STAIRS_SPRUCEWOOD | t::STAIRS_BIRCHWOOD | t::STAIRS_JUNGLEWOOD | t::STAIRS_QUARTZ)
}

pub fn is_door(id: u8) -> bool {
    id == t::DOOR_WOOD || id == t::DOOR_IRON
}

fn part(min: [f32; 3], max: [f32; 3]) -> Part {
    Part { min, max, cells: None, rot: [0; 6] }
}

fn with(mut p: Part, cell: Cell) -> Part {
    p.cells = Some([cell; 6]);
    p
}

/// `DoorTile::getCompositeData`: lower half's dir/open bits (0..=2), 8 = this is the upper half,
/// 16 = right hinge (bit 0 of the upper half's data).
pub fn door_composite(data: u8, x: i32, y: i32, z: i32, nb: Nb) -> u8 {
    let upper = data & 8 != 0;
    let (lower, up) = if upper { (nb(x, y - 1, z).1, data) } else { (data, nb(x, y + 1, z).1) };
    (lower & 7) | if upper { 8 } else { 0 } | if up & 1 != 0 { 16 } else { 0 }
}

/// `DoorTile::setShape` for a composite value.
fn door_box(c: u8) -> Aabb3 {
    let r = 3.0f32 / 16.0;
    let dir = c & 3;
    let open = c & 4 != 0;
    let hinge = c & 16 != 0;
    match (dir, open) {
        (0, true) => if !hinge { ([0.0, 0.0, 0.0], [1.0, 1.0, r]) } else { ([0.0, 0.0, 1.0 - r], [1.0, 1.0, 1.0]) },
        (0, false) => ([0.0, 0.0, 0.0], [r, 1.0, 1.0]),
        (1, true) => if !hinge { ([1.0 - r, 0.0, 0.0], [1.0, 1.0, 1.0]) } else { ([0.0, 0.0, 0.0], [r, 1.0, 1.0]) },
        (1, false) => ([0.0, 0.0, 0.0], [1.0, 1.0, r]),
        (2, true) => if !hinge { ([0.0, 0.0, 1.0 - r], [1.0, 1.0, 1.0]) } else { ([0.0, 0.0, 0.0], [1.0, 1.0, r]) },
        (2, false) => ([1.0 - r, 0.0, 0.0], [1.0, 1.0, 1.0]),
        (_, true) => if !hinge { ([0.0, 0.0, 0.0], [r, 1.0, 1.0]) } else { ([1.0 - r, 0.0, 0.0], [1.0, 1.0, 1.0]) },
        (_, false) => ([0.0, 0.0, 1.0 - r], [1.0, 1.0, 1.0]),
    }
}

/// Does the door leaf's texture get mirrored on this (Rust-ordered) face? `DoorTile::getTexture`.
fn door_flip(c: u8, face: usize) -> bool {
    // Rust faces -x,+x,-y,+y,-z,+z → original 4,5,0,1,2,3
    let cf = [4, 5, 0, 1, 2, 3][face];
    let dir = c & 3;
    let mut flip = false;
    if c & 4 != 0 {
        flip = matches!((dir, cf), (0, 2) | (1, 5) | (2, 3) | (3, 4));
    } else {
        flip = matches!((dir, cf), (0, 5) | (1, 3) | (2, 4) | (3, 2));
        if c & 16 != 0 {
            flip = !flip;
        }
    }
    flip
}

/// Attachment side of levers / buttons: (min, max) for data 1..=4 (wall) and the floor / ceiling variants.
fn attached_box(data: u8, r: f32, y0: f32, y1: f32, depth: f32) -> Aabb3 {
    match data & 7 {
        1 => ([0.0, y0, 0.5 - r], [depth, y1, 0.5 + r]),
        2 => ([1.0 - depth, y0, 0.5 - r], [1.0, y1, 0.5 + r]),
        3 => ([0.5 - r, y0, 0.0], [0.5 + r, y1, depth]),
        4 => ([0.5 - r, y0, 1.0 - depth], [0.5 + r, y1, 1.0]),
        7 | 0 => ([0.5 - r, 1.0 - y1.min(0.6), 0.5 - r], [0.5 + r, 1.0, 0.5 + r]),
        _ => ([0.5 - r, 0.0, 0.5 - r], [0.5 + r, 0.6, 0.5 + r]),
    }
}

fn fence_gate_parts(data: u8) -> Vec<Part> {
    let along_x = data & 1 == 0; // dir 0 / 2 run along x
    let open = data & 4 != 0;
    let c = icon("planks_oak").unwrap_or((0, 4));
    let (a, b) = (0.4375f32, 0.5625f32);
    let mk = |l0: f32, l1: f32, y0: f32, y1: f32, p0: f32, p1: f32| {
        if along_x {
            with(part([l0, y0, p0], [l1, y1, p1]), c)
        } else {
            with(part([p0, y0, l0], [p1, y1, l1]), c)
        }
    };
    let mut v = vec![mk(0.0, 0.125, 0.3125, 1.0, a, b), mk(0.875, 1.0, 0.3125, 1.0, a, b)];
    if open {
        // the leaves swing 90° around the posts
        for (l0, l1) in [(0.0, 0.125), (0.875, 1.0)] {
            v.push(mk(l0, l1, 0.375, 0.5625, 0.125, a));
            v.push(mk(l0, l1, 0.75, 0.9375, 0.125, a));
        }
    } else {
        v.push(mk(0.125, 0.875, 0.375, 0.5625, a + 0.0625, b - 0.0625 + 0.0625));
        v.push(mk(0.125, 0.875, 0.75, 0.9375, a + 0.0625, b - 0.0625 + 0.0625));
        v.push(mk(0.4375, 0.5625, 0.5625, 0.75, a + 0.0625, b - 0.0625 + 0.0625));
    }
    v
}

/// `PistonBaseTile::updateShape`: the body of an extended base is 4/16 shorter on the side of the head.
pub fn piston_base_box(data: u8) -> Aabb3 {
    let th = 4.0f32 / 16.0;
    if data & 8 == 0 {
        return ([0.0; 3], [1.0; 3]);
    }
    match data & 7 {
        0 => ([0.0, th, 0.0], [1.0, 1.0, 1.0]),
        1 => ([0.0, 0.0, 0.0], [1.0, 1.0 - th, 1.0]),
        2 => ([0.0, 0.0, th], [1.0, 1.0, 1.0]),
        3 => ([0.0, 0.0, 0.0], [1.0, 1.0, 1.0 - th]),
        4 => ([th, 0.0, 0.0], [1.0, 1.0, 1.0]),
        _ => ([0.0, 0.0, 0.0], [1.0 - th, 1.0, 1.0]),
    }
}

/// The head of a piston (`PistonExtensionTile`): a 4/16 plate in front of the cell, plus the rod behind it
/// (`arm` false draws only half of it, as the original does while the head is still starting to move).
pub fn piston_head_parts(data: u8, arm: bool) -> Vec<Part> {
    let th = 4.0f32 / 16.0;
    let (r0, r1) = (6.0f32 / 16.0, 10.0f32 / 16.0);
    let f = (data & 7).min(5) as usize;
    let rod_end: f32 = if arm { 1.0 } else { 0.5 };
    // plate and rod boxes per facing
    let (plate, rod): (Aabb3, Aabb3) = match f {
        0 => (([0.0, 0.0, 0.0], [1.0, th, 1.0]), ([r0, th, r0], [r1, rod_end.max(th + 0.01), r1])),
        1 => (([0.0, 1.0 - th, 0.0], [1.0, 1.0, 1.0]), ([r0, 1.0 - rod_end.max(th + 0.01), r0], [r1, 1.0 - th, r1])),
        2 => (([0.0, 0.0, 0.0], [1.0, 1.0, th]), ([r0, r0, th], [r1, r1, rod_end.max(th + 0.01)])),
        3 => (([0.0, 0.0, 1.0 - th], [1.0, 1.0, 1.0]), ([r0, r0, 1.0 - rod_end.max(th + 0.01)], [r1, r1, 1.0 - th])),
        4 => (([0.0, 0.0, 0.0], [th, 1.0, 1.0]), ([th, r0, r0], [rod_end.max(th + 0.01), r1, r1])),
        _ => (([1.0 - th, 0.0, 0.0], [1.0, 1.0, 1.0]), ([1.0 - rod_end.max(th + 0.01), r0, r0], [1.0 - th, r1, r1])),
    };
    let side = icon("piston_side").unwrap_or((6, 12));
    let top = icon(if data & 8 != 0 { "piston_top_sticky" } else { "piston_top" }).unwrap_or((6, 11));
    let plain = icon("piston_top").unwrap_or((6, 11));
    let dirv = [[0.0f32, -1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0], [0.0, 0.0, 1.0], [-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]][f];
    let front = [2usize, 3, 4, 5, 0, 1][f];
    let back = front ^ 1;
    let mk = |bx: Aabb3, ends: bool| {
        let mut p = part(bx.0, bx.1);
        let mut cells = [side; 6];
        if ends {
            cells[front] = top;
            cells[back] = plain;
        }
        p.cells = Some(cells);
        for face in 0..6 {
            if !ends || (face != front && face != back) {
                p.rot[face] = crate::mesh::rot_for_up(face, dirv);
            }
        }
        p
    };
    let mut v = vec![mk(plate, true)];
    let mut r = mk(rod, false);
    r.cells = Some([icon("piston_arm").unwrap_or(side); 6]);
    v.push(r);
    v
}

/// Neighbour lookup: (tile, data) at absolute coordinates.
pub type Nb<'a> = &'a dyn Fn(i32, i32, i32) -> (u8, u8);

/// Render parts of block `id`/`data` without neighbour information.
pub fn parts(id: u8, data: u8) -> Vec<Part> {
    parts_in(id, data, 0, 0, 0, &|_, _, _| (0, 0))
}

fn stairs_at(nb: Nb, x: i32, y: i32, z: i32) -> Option<u8> {
    let (i, d) = nb(x, y, z);
    is_stairs(i).then_some(d)
}

/// `StairTile::isLockAttached`: the cell holds a stair of the same half and direction.
fn lock_attached(nb: Nb, x: i32, y: i32, z: i32, data: u8) -> bool {
    stairs_at(nb, x, y, z).map_or(false, |d| d & 4 == data & 4 && d & 3 == data & 3)
}

/// Stair boxes: the half slab, the step and (at corners) the extra quarter, as `StairTile::setStepShape` /
/// `setInnerPieceShape` compute them.
fn stair_parts(data: u8, x: i32, y: i32, z: i32, nb: Nb) -> Vec<Part> {
    let up = data & 4 != 0;
    let dir = data & 3;
    let (slab_y, step_y) = if up { ((0.5, 1.0), (0.0, 0.5)) } else { ((0.0, 0.5), (0.5, 1.0)) };
    let same_half = |d: u8| d & 4 == data & 4;
    // step: (west, east, north, south)
    let (mut w, mut e, mut n, mut sth) = match dir {
        0 => (0.5, 1.0, 0.0, 1.0),
        1 => (0.0, 0.5, 0.0, 1.0),
        2 => (0.0, 1.0, 0.5, 1.0),
        _ => (0.0, 1.0, 0.0, 0.5),
    };
    let (bx, bz) = match dir {
        0 => (1, 0),
        1 => (-1, 0),
        2 => (0, 1),
        _ => (0, -1),
    };
    let mut check_inner = true;
    if let Some(bd) = stairs_at(nb, x + bx, y, z + bz).filter(|d| same_half(*d)) {
        let bdir = bd & 3;
        match (dir, bdir) {
            (0, 3) | (1, 3) if !lock_attached(nb, x, y, z + 1, data) => {
                sth = 0.5;
                check_inner = false;
            }
            (0, 2) | (1, 2) if !lock_attached(nb, x, y, z - 1, data) => {
                n = 0.5;
                check_inner = false;
            }
            (2, 1) | (3, 1) if !lock_attached(nb, x + 1, y, z, data) => {
                e = 0.5;
                check_inner = false;
            }
            (2, 0) | (3, 0) if !lock_attached(nb, x - 1, y, z, data) => {
                w = 0.5;
                check_inner = false;
            }
            _ => {}
        }
    }
    let mut v = vec![part([0.0, slab_y.0, 0.0], [1.0, slab_y.1, 1.0]), part([w, step_y.0, n], [e, step_y.1, sth])];
    if check_inner {
        let (fx, fz) = (-bx, -bz);
        if let Some(fd) = stairs_at(nb, x + fx, y, z + fz).filter(|d| same_half(*d)) {
            let fdir = fd & 3;
            let inner = match (dir, fdir) {
                (0, 3) if !lock_attached(nb, x, y, z - 1, data) => Some(([0.0, 0.0], [0.5, 0.5])),
                (0, 2) if !lock_attached(nb, x, y, z + 1, data) => Some(([0.0, 0.5], [0.5, 1.0])),
                (1, 3) if !lock_attached(nb, x, y, z - 1, data) => Some(([0.5, 0.0], [1.0, 0.5])),
                (1, 2) if !lock_attached(nb, x, y, z + 1, data) => Some(([0.5, 0.5], [1.0, 1.0])),
                (2, 1) if !lock_attached(nb, x - 1, y, z, data) => Some(([0.0, 0.0], [0.5, 0.5])),
                (2, 0) if !lock_attached(nb, x + 1, y, z, data) => Some(([0.5, 0.0], [1.0, 0.5])),
                (3, 1) if !lock_attached(nb, x - 1, y, z, data) => Some(([0.0, 0.5], [0.5, 1.0])),
                (3, 0) if !lock_attached(nb, x + 1, y, z, data) => Some(([0.5, 0.5], [1.0, 1.0])),
                _ => None,
            };
            if let Some(([x0, z0], [x1, z1])) = inner {
                v.push(part([x0, step_y.0, z0], [x1, step_y.1, z1]));
            }
        }
    }
    v
}

fn fence_connects(id: u8, nb: Nb, x: i32, y: i32, z: i32) -> bool {
    let (n, d) = nb(x, y, z);
    n == id || n == t::FENCE_GATE || (n != 0 && n != t::PUMPKIN && crate::blocks::style(n, d).opaque)
}

fn fence_parts(id: u8, x: i32, y: i32, z: i32, nb: Nb) -> Vec<Part> {
    let c = if id == t::NETHER_FENCE { icon("nether_brick").unwrap_or((0, 4)) } else { icon("planks_oak").unwrap_or((0, 4)) };
    let mut v = vec![with(part([0.375, 0.0, 0.375], [0.625, 1.0, 0.625]), c)];
    for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        if !fence_connects(id, nb, x + dx, y, z + dz) {
            continue;
        }
        let (x0, x1) = match dx {
            -1 => (0.0, 0.5),
            1 => (0.5, 1.0),
            _ => (0.4375, 0.5625),
        };
        let (z0, z1) = match dz {
            -1 => (0.0, 0.5),
            1 => (0.5, 1.0),
            _ => (0.4375, 0.5625),
        };
        for (y0, y1) in [(0.375, 0.5625), (0.75, 0.9375)] {
            v.push(with(part([x0, y0, z0], [x1, y1, z1]), c));
        }
    }
    v
}

fn anvil_parts(data: u8) -> Vec<Part> {
    let base = icon("anvil_base").unwrap_or((13, 7));
    let top = icon(["anvil_top", "anvil_top_damaged_1", "anvil_top_damaged_2", "anvil_top_damaged_2"][((data >> 2) & 3) as usize]).unwrap_or((14, 7));
    let swap = data & 1 != 0;
    let mk = |a: [f32; 3], b: [f32; 3], cells: [Cell; 6]| {
        let f = |p: [f32; 3]| if swap { [p[2], p[1], p[0]] } else { p };
        let (lo, hi) = (f(a), f(b));
        let mut p = part([lo[0].min(hi[0]), lo[1].min(hi[1]), lo[2].min(hi[2])], [lo[0].max(hi[0]), lo[1].max(hi[1]), lo[2].max(hi[2])]);
        p.cells = Some(cells);
        p
    };
    let u = 1.0 / 16.0;
    vec![
        mk([2.0 * u, 0.0, 2.0 * u], [14.0 * u, 4.0 * u, 14.0 * u], [base; 6]),
        mk([4.0 * u, 4.0 * u, 3.0 * u], [12.0 * u, 5.0 * u, 13.0 * u], [base; 6]),
        mk([6.0 * u, 5.0 * u, 4.0 * u], [10.0 * u, 10.0 * u, 12.0 * u], [base; 6]),
        mk([3.0 * u, 10.0 * u, 0.0], [13.0 * u, 1.0, 1.0], [base, base, base, top, base, base]),
    ]
}

/// Render parts of block `id`/`data` at (x, y, z).
pub fn parts_in(id: u8, data: u8, x: i32, y: i32, z: i32, nb: Nb) -> Vec<Part> {
    let mut v = Vec::new();
    match id {
        t::FENCE | t::NETHER_FENCE => v = fence_parts(id, x, y, z, nb),
        t::PISTON_BASE | t::PISTON_STICKY_BASE => {
            let (mn, mx) = piston_base_box(data);
            let st = crate::blocks::style(id, data);
            let mut p = part(mn, mx);
            p.cells = Some(st.cells);
            p.rot = st.rot;
            v.push(p);
            if data & 8 != 0 {
                // the rod reaches 4/16 into this cell, filling the gap the shortened body leaves
                let (th, r0, r1) = (4.0f32 / 16.0, 6.0f32 / 16.0, 10.0f32 / 16.0);
                let (a, b): ([f32; 3], [f32; 3]) = match data & 7 {
                    0 => ([r0, 0.0, r0], [r1, th, r1]),
                    1 => ([r0, 1.0 - th, r0], [r1, 1.0, r1]),
                    2 => ([r0, r0, 0.0], [r1, r1, th]),
                    3 => ([r0, r0, 1.0 - th], [r1, r1, 1.0]),
                    4 => ([0.0, r0, r0], [th, r1, r1]),
                    _ => ([1.0 - th, r0, r0], [1.0, r1, r1]),
                };
                let arm = icon("piston_arm").unwrap_or((6, 12));
                let dirv = [[0.0f32, -1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0], [0.0, 0.0, 1.0], [-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]][(data & 7).min(5) as usize];
                let mut p = with(part(a, b), arm);
                for face in 0..6 {
                    p.rot[face] = crate::mesh::rot_for_up(face, dirv);
                }
                v.push(p);
            }
        }
        t::PISTON_EXTENSION_PIECE => v = piston_head_parts(data, true),
        t::ANVIL => v = anvil_parts(data),
        t::CHEST => {
            let animated = crate::chest::is_animating((x, y, z));
            v = crate::chest::chest_parts(data, !animated, crate::chest::ChestCells::lookup(), crate::chest::partner(id, x, y, z, nb));
        }
        _ if is_stairs(id) => v = stair_parts(data, x, y, z, nb),
        t::DOOR_WOOD | t::DOOR_IRON => {
            let comp = door_composite(data, x, y, z, nb);
            let (mn, mx) = door_box(comp);
            let name = match (id == t::DOOR_WOOD, data & 8 != 0) {
                (true, true) => "door_wood_upper",
                (true, false) => "door_wood_lower",
                (false, true) => "door_iron_upper",
                (false, false) => "door_iron_lower",
            };
            let cell = icon(name).unwrap_or((0, 4));
            let lower = icon(if id == t::DOOR_WOOD { "door_wood_lower" } else { "door_iron_lower" }).unwrap_or(cell);
            let mut p = with(part(mn, mx), cell);
            if let Some(c) = p.cells.as_mut() {
                c[2] = lower;
                c[3] = lower;
            }
            for f in [0usize, 1, 4, 5] {
                if door_flip(comp, f) {
                    p.rot[f] |= 0x10;
                }
            }
            v.push(p);
        }
        t::TRAPDOOR => {
            let th = 0.1875;
            let (mn, mx) = if data & 4 != 0 {
                match data & 3 {
                    0 => ([0.0, 0.0, 1.0 - th], [1.0, 1.0, 1.0]),
                    1 => ([0.0, 0.0, 0.0], [1.0, 1.0, th]),
                    2 => ([1.0 - th, 0.0, 0.0], [1.0, 1.0, 1.0]),
                    _ => ([0.0, 0.0, 0.0], [th, 1.0, 1.0]),
                }
            } else if data & 8 != 0 {
                ([0.0, 1.0 - th, 0.0], [1.0, 1.0, 1.0])
            } else {
                ([0.0, 0.0, 0.0], [1.0, th, 1.0])
            };
            v.push(with(part(mn, mx), icon("trapdoor").unwrap_or((5, 4))));
        }
        t::FENCE_GATE => v = fence_gate_parts(data),
        t::SIGN => {
            let c = icon("planks_oak").unwrap_or((0, 4));
            let along_x = (((data as i32 + 2) >> 2) & 3) % 2 == 0;
            let (mn, mx) = if along_x { ([0.0, 0.5, 0.4375], [1.0, 1.0, 0.5625]) } else { ([0.4375, 0.5, 0.0], [0.5625, 1.0, 1.0]) };
            v.push(with(part(mn, mx), c));
            v.push(with(part([0.4375, 0.0, 0.4375], [0.5625, 0.5, 0.5625]), c));
        }
        t::WALL_SIGN => {
            let c = icon("planks_oak").unwrap_or((0, 4));
            let (mn, mx) = match data & 7 {
                2 => ([0.0, 0.28125, 0.875], [1.0, 0.78125, 1.0]),
                3 => ([0.0, 0.28125, 0.0], [1.0, 0.78125, 0.125]),
                4 => ([0.875, 0.28125, 0.0], [1.0, 0.78125, 1.0]),
                _ => ([0.0, 0.28125, 0.0], [0.125, 0.78125, 1.0]),
            };
            v.push(with(part(mn, mx), c));
        }
        t::LEVER => {
            // `TileRenderer::tesselateLeverInWorld`: a cobblestone base plate (the tilted stick is added by the mesher)
            let (w1, w2, h) = (4.0f32 / 16.0, 3.0f32 / 16.0, 3.0f32 / 16.0);
            let (mn, mx) = match data & 7 {
                5 => ([0.5 - w2, 0.0, 0.5 - w1], [0.5 + w2, h, 0.5 + w1]),
                6 => ([0.5 - w1, 0.0, 0.5 - w2], [0.5 + w1, h, 0.5 + w2]),
                4 => ([0.5 - w2, 0.5 - w1, 1.0 - h], [0.5 + w2, 0.5 + w1, 1.0]),
                3 => ([0.5 - w2, 0.5 - w1, 0.0], [0.5 + w2, 0.5 + w1, h]),
                2 => ([1.0 - h, 0.5 - w1, 0.5 - w2], [1.0, 0.5 + w1, 0.5 + w2]),
                1 => ([0.0, 0.5 - w1, 0.5 - w2], [h, 0.5 + w1, 0.5 + w2]),
                0 => ([0.5 - w1, 1.0 - h, 0.5 - w2], [0.5 + w1, 1.0, 0.5 + w2]),
                _ => ([0.5 - w2, 1.0 - h, 0.5 - w1], [0.5 + w2, 1.0, 0.5 + w1]),
            };
            v.push(with(part(mn, mx), icon("cobblestone").unwrap_or((1, 0))));
        }
        t::BUTTON_STONE | t::BUTTON_WOOD => {
            // `ButtonTile::updateShape`
            let c = if id == t::BUTTON_STONE { icon("stone").unwrap_or((0, 1)) } else { icon("planks_oak").unwrap_or((0, 4)) };
            let (h0, h1, r) = (6.0f32 / 16.0, 10.0f32 / 16.0, 3.0f32 / 16.0);
            let d = if data & 8 != 0 { 1.0f32 / 16.0 } else { 2.0 / 16.0 };
            let (mn, mx) = match data & 7 {
                2 => ([1.0 - d, h0, 0.5 - r], [1.0, h1, 0.5 + r]),
                3 => ([0.5 - r, h0, 0.0], [0.5 + r, h1, d]),
                4 => ([0.5 - r, h0, 1.0 - d], [0.5 + r, h1, 1.0]),
                _ => ([0.0, h0, 0.5 - r], [d, h1, 0.5 + r]),
            };
            v.push(with(part(mn, mx), c));
        }
        t::PRESSURE_PLATE_STONE | t::PRESSURE_PLATE_WOOD => {
            let c = if id == t::PRESSURE_PLATE_STONE { icon("stone").unwrap_or((0, 1)) } else { icon("planks_oak").unwrap_or((0, 4)) };
            let h = if data & 1 != 0 { 0.03125 } else { 0.0625 };
            v.push(with(part([0.0625, 0.0, 0.0625], [0.9375, h, 0.9375]), c));
        }
        t::DIODE_OFF | t::DIODE_ON => {
            // the slab; its torches are added by the mesher (`tesselateRepeaterInWorld`)
            let top = icon(if id == t::DIODE_ON { "repeater_on" } else { "repeater_off" }).unwrap_or((8, 3));
            let side = icon("stoneslab_side").unwrap_or((0, 5));
            let mut p = part([0.0, 0.0, 0.0], [1.0, 2.0 / 16.0, 1.0]);
            p.cells = Some([side, side, side, top, side, side]);
            p.rot[3] = data & 3;
            v.push(p);
        }
        t::CAKE => {
            let bites = (data & 7).min(6) as f32;
            let e = 0.0625;
            let mut p = part([e + bites * 0.125, 0.0, e], [1.0 - e, 0.5, 1.0 - e]);
            let side = icon("cake_side").unwrap_or((7, 10));
            let top = icon("cake_top").unwrap_or((7, 9));
            let bot = icon("cake_bottom").unwrap_or((7, 12));
            let inner = icon("cake_inner").unwrap_or((7, 11));
            p.cells = Some([if bites > 0.0 { inner } else { side }, side, bot, top, side, side]);
            v.push(p);
        }
        _ => {}
    }
    v
}

/// Collision boxes (`Tile::addAABBs`).
pub fn collision_in(id: u8, data: u8, x: i32, y: i32, z: i32, nb: Nb) -> Vec<Aabb3> {
    match id {
        t::PRESSURE_PLATE_STONE | t::PRESSURE_PLATE_WOOD | t::BUTTON_STONE | t::BUTTON_WOOD | t::LEVER | t::SIGN | t::WALL_SIGN => Vec::new(),
        t::FENCE_GATE => {
            if data & 4 != 0 {
                Vec::new()
            } else if data & 1 == 0 {
                vec![([0.0, 0.0, 0.375], [1.0, 1.5, 0.625])]
            } else {
                vec![([0.375, 0.0, 0.0], [0.625, 1.5, 1.0])]
            }
        }
        t::PISTON_BASE | t::PISTON_STICKY_BASE => vec![([0.0; 3], [1.0; 3])],
        t::FENCE | t::NETHER_FENCE => {
            let mut v = vec![([0.375, 0.0, 0.375], [0.625, 1.5, 0.625])];
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                if fence_connects(id, nb, x + dx, y, z + dz) {
                    let (x0, x1) = match dx {
                        -1 => (0.0, 0.5),
                        1 => (0.5, 1.0),
                        _ => (0.375, 0.625),
                    };
                    let (z0, z1) = match dz {
                        -1 => (0.0, 0.5),
                        1 => (0.5, 1.0),
                        _ => (0.375, 0.625),
                    };
                    v.push(([x0, 0.0, z0], [x1, 1.5, z1]));
                }
            }
            v
        }
        _ => parts_in(id, data, x, y, z, nb).into_iter().map(|p| (p.min, p.max)).collect(),
    }
}

/// Selection box: the union of the parts (fence gates and fences use the full height).
pub fn selection_in(id: u8, data: u8, x: i32, y: i32, z: i32, nb: Nb) -> Aabb3 {
    if matches!(id, t::PISTON_BASE | t::PISTON_STICKY_BASE) {
        return piston_base_box(data);
    }
    if id == t::PISTON_EXTENSION_PIECE {
        // `PistonExtensionTile::updateShape`: the plate only
        return piston_head_parts(data, true)[0].min_max();
    }
    if id == t::LEVER {
        // `LeverTile::updateShape`
        let mut r = 3.0f32 / 16.0;
        return match data & 7 {
            1 => ([0.0, 0.2, 0.5 - r], [r * 2.0, 0.8, 0.5 + r]),
            2 => ([1.0 - r * 2.0, 0.2, 0.5 - r], [1.0, 0.8, 0.5 + r]),
            3 => ([0.5 - r, 0.2, 0.0], [0.5 + r, 0.8, r * 2.0]),
            4 => ([0.5 - r, 0.2, 1.0 - r * 2.0], [0.5 + r, 0.8, 1.0]),
            5 | 6 => {
                r = 4.0 / 16.0;
                ([0.5 - r, 0.0, 0.5 - r], [0.5 + r, 0.6, 0.5 + r])
            }
            _ => {
                r = 4.0 / 16.0;
                ([0.5 - r, 0.4, 0.5 - r], [0.5 + r, 1.0, 0.5 + r])
            }
        };
    }
    let ps = parts_in(id, data, x, y, z, nb);
    if ps.is_empty() {
        return ([0.0; 3], [1.0; 3]);
    }
    if is_stairs(id) {
        return ([0.0; 3], [1.0; 3]);
    }
    let mut mn = [1.0f32; 3];
    let mut mx = [0.0f32; 3];
    for p in &ps {
        for a in 0..3 {
            mn[a] = mn[a].min(p.min[a]);
            mx[a] = mx[a].max(p.max[a]);
        }
    }
    if matches!(id, t::FENCE_GATE | t::FENCE | t::NETHER_FENCE) {
        mn[1] = 0.0;
        mx[1] = 1.0;
    }
    (mn, mx)
}

pub fn collision(id: u8, data: u8) -> Vec<Aabb3> {
    collision_in(id, data, 0, 0, 0, &|_, _, _| (0, 0))
}

pub fn selection(id: u8, data: u8) -> Aabb3 {
    selection_in(id, data, 0, 0, 0, &|_, _, _| (0, 0))
}

/// Does the block toggle open / closed on use (`DoorTile::use`, `TrapDoorTile::use`, `FenceGateTile::use`)?
pub fn use_toggles(id: u8) -> bool {
    matches!(id, t::DOOR_WOOD | t::TRAPDOOR | t::FENCE_GATE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stairs_have_two_boxes_and_cover_a_full_cube() {
        for d in 0..8u8 {
            let p = parts(t::STAIRS_WOOD, d);
            assert_eq!(p.len(), 2);
            let vol: f32 = p.iter().map(|b| (b.max[0] - b.min[0]) * (b.max[1] - b.min[1]) * (b.max[2] - b.min[2])).sum();
            assert!((vol - 0.75).abs() < 1e-5, "data {d}: {vol}");
        }
    }

    #[test]
    fn upside_down_stairs_put_the_step_on_top() {
        let up = parts(t::STAIRS_STONE, 4);
        assert!((up[0].min[1] - 0.5).abs() < 1e-6 && (up[0].max[1] - 1.0).abs() < 1e-6, "slab on top");
        assert!((up[1].min[1]).abs() < 1e-6 && (up[1].max[1] - 0.5).abs() < 1e-6, "step below");
    }

    #[test]
    fn open_door_and_closed_door_differ() {
        assert_ne!(door_box(0), door_box(4));
        assert_ne!(door_box(4), door_box(4 | 16));
        let (mn, mx) = door_box(0);
        assert!((mx[0] - mn[0] - 0.1875).abs() < 1e-6);
    }

    #[test]
    fn open_gate_does_not_block() {
        assert!(collision(t::FENCE_GATE, 4).is_empty());
        assert_eq!(collision(t::FENCE_GATE, 0).len(), 1);
    }

    #[test]
    fn every_shaped_block_has_valid_parts() {
        for id in 0..=255u8 {
            if !is_shaped(id) {
                continue;
            }
            for d in 0..16u8 {
                let ps = parts(id, d);
                assert!(!ps.is_empty(), "{id}/{d}");
                for p in ps {
                    for a in 0..3 {
                        assert!(p.min[a] < p.max[a] && p.min[a] >= 0.0 && p.max[a] <= 1.0001, "{id}/{d}: {:?}", p);
                    }
                }
            }
        }
    }
}
