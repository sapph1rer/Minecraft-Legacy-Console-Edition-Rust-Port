//! Chunk mesher: turns a `Level` chunk into vertex/index lists (world-space positions), with face culling,
//! the vanilla smooth-lighting (ambient occlusion) algorithm of `TileRenderer`, per-vertex sky/block light
//! (looked up in the light texture at draw time) and the per-biome colour tints of the colour table.

use crate::blocks::{style, Kind, Style, Tint};
use mc_level::level::Level;
use mc_world::colours as col;
use mc_world::tile_props;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    /// rgb multiplier (shading × tint), alpha
    pub color: [f32; 4],
    /// (sky, block) light levels 0..15 (fractional after smoothing)
    pub light: [f32; 2],
}

#[derive(Default, Clone)]
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
    fn quad(&mut self, v: [Vertex; 4], flip: bool) {
        let b = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&v);
        let idx: [u32; 6] = if flip { [1, 2, 3, 1, 3, 0] } else { [0, 1, 2, 0, 2, 3] };
        self.indices.extend(idx.iter().map(|i| b + i));
    }
}

#[derive(Default, Clone)]
pub struct ChunkMesh {
    pub opaque: MeshData,
    pub translucent: MeshData,
}

/// Corners (BL, BR, TR, TL as seen from outside) of each face of the unit cube.
pub(crate) const FACE_CORNERS: [[[f32; 3]; 4]; 6] = [
    [[0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.]], // -x
    [[1., 0., 1.], [1., 0., 0.], [1., 1., 0.], [1., 1., 1.]], // +x
    [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]], // -y
    [[0., 1., 1.], [1., 1., 1.], [1., 1., 0.], [0., 1., 0.]], // +y
    [[1., 0., 0.], [0., 0., 0.], [0., 1., 0.], [1., 1., 0.]], // -z
    [[0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.]], // +z
];
pub(crate) const FACE_NORMAL: [[i32; 3]; 6] = [[-1, 0, 0], [1, 0, 0], [0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1]];
/// `TileRenderer` face brightness constants: x faces 0.6, z faces 0.8, bottom 0.5, top 1.0.
const FACE_SHADE: [f32; 6] = [0.6, 0.6, 0.5, 1.0, 0.8, 0.8];
const CORNER_UV: [[f32; 2]; 4] = [[0., 1.], [1., 1.], [1., 0.], [0., 0.]];
/// tangent axes of each face
const FACE_AXES: [[usize; 2]; 6] = [[1, 2], [1, 2], [0, 2], [0, 2], [0, 1], [0, 1]];

// ------------------------------------------------------------------------------------------------ tints

fn avg_biome(lv: &Level, x: i32, z: i32, which: usize) -> [f32; 3] {
    let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
    for oz in -1..=1 {
        for ox in -1..=1 {
            let c = col::BIOME_COLOURS[lv.get_biome(x + ox, z + oz).id as usize % col::BIOME_COLOURS.len()][which];
            r += (c >> 16) & 0xff;
            g += (c >> 8) & 0xff;
            b += c & 0xff;
        }
    }
    [(r / 9) as f32 / 255.0, (g / 9) as f32 / 255.0, (b / 9) as f32 / 255.0]
}

fn rgb(v: u32) -> [f32; 3] {
    [((v >> 16) & 0xff) as f32 / 255.0, ((v >> 8) & 0xff) as f32 / 255.0, (v & 0xff) as f32 / 255.0]
}

/// Tint of a block at x/z: `GrassTile/LeafTile/LiquidTile/TallGrass::getColor(level, x, y, z)`.
pub fn tint_at(lv: &Level, tint: Tint, x: i32, z: i32) -> [f32; 3] {
    match tint {
        Tint::None => [1.0; 3],
        Tint::Grass => avg_biome(lv, x, z, 0),
        Tint::Foliage => avg_biome(lv, x, z, 1),
        Tint::Water => avg_biome(lv, x, z, 2),
        Tint::GrassPlant => rgb(col::BIOME_COLOURS[lv.get_biome(x, z).id as usize % col::BIOME_COLOURS.len()][0]),
        Tint::Spruce => rgb(col::FOLIAGE_EVERGREEN),
        Tint::Birch => rgb(col::FOLIAGE_BIRCH),
        Tint::FoliageDefault => rgb(col::FOLIAGE_DEFAULT),
        Tint::Lily => rgb(0x208030),
        Tint::Redstone => rgb(col::REDSTONE_UNLIT),
    }
}

/// Tint for UI icons (hotbar): the plains colours of the table.
pub fn tint_colour(tint: Tint, _biome_temp: f32) -> [f32; 3] {
    let plains = col::BIOME_COLOURS[1];
    match tint {
        Tint::None => [1.0; 3],
        Tint::Grass | Tint::GrassPlant => rgb(plains[0]),
        Tint::Foliage => rgb(plains[1]),
        Tint::Water => rgb(plains[2]),
        Tint::Spruce => rgb(col::FOLIAGE_EVERGREEN),
        Tint::Birch => rgb(col::FOLIAGE_BIRCH),
        Tint::FoliageDefault => rgb(col::FOLIAGE_DEFAULT),
        Tint::Lily => rgb(0x208030),
        Tint::Redstone => rgb(col::REDSTONE_DUST),
    }
}

// ------------------------------------------------------------------------------------------------ light

struct Ctx<'a> {
    lv: &'a Level,
}

/// Is the block at (x, y, z) a solid full cube (hides the faces next to it, carries levers / buttons / torches)?
pub fn full_cube(lv: &Level, x: i32, y: i32, z: i32) -> bool {
    if !(0..256).contains(&y) {
        return false;
    }
    let id = lv.get_tile(x, y, z);
    if id == 0 || !style(id, 0).opaque {
        return false;
    }
    // not full cubes: an extended piston base (shortened body), chests (14 pixels wide)
    use mc_world::tile_ids as t;
    match id {
        t::PISTON_BASE | t::PISTON_STICKY_BASE => lv.get_data(x, y, z) & 8 == 0,
        t::CHEST | t::CHEST_TRAP | t::ENDER_CHEST => false,
        _ => true,
    }
}

impl Ctx<'_> {
    #[inline]
    fn occludes(&self, x: i32, y: i32, z: i32) -> bool {
        full_cube(self.lv, x, y, z)
    }
    /// `Tile::getLightColor`: (sky, block) at a cell.
    #[inline]
    fn lc(&self, x: i32, y: i32, z: i32) -> (i32, i32) {
        self.lv.tile_light_color(x, y, z)
    }
    /// `Tile::getShadeBrightness`: 0.2 inside solid blocking tiles, 1.0 elsewhere.
    #[inline]
    fn shade(&self, x: i32, y: i32, z: i32) -> f32 {
        let id = self.lv.get_tile(x, y, z);
        if id != 0 && tile_props::material(id).is_solid_blocking() && matches!(style(id, 0).kind, Kind::Cube) {
            0.2
        } else {
            1.0
        }
    }
    #[inline]
    fn translucent(&self, x: i32, y: i32, z: i32) -> bool {
        tile_props::is_translucent(self.lv.get_tile(x, y, z))
    }
}

/// `TileRenderer::blend(a, b, c, def)`: zero entries are replaced by `def`, then averaged (kept fractional).
fn blend(a: (i32, i32), b: (i32, i32), c: (i32, i32), def: (i32, i32)) -> [f32; 2] {
    let fix = |v: (i32, i32)| if v == (0, 0) { def } else { v };
    let (a, b, c) = (fix(a), fix(b), fix(c));
    [(a.0 + b.0 + c.0 + def.0) as f32 / 4.0, (a.1 + b.1 + c.1 + def.1) as f32 / 4.0]
}

pub fn uv_of(cell: (u8, u8), corner: usize) -> [f32; 2] {
    let (r, c) = (cell.0 as f32, cell.1 as f32);
    let e = 0.0004;
    let u = CORNER_UV[corner][0];
    let v = CORNER_UV[corner][1];
    [(c + e + u * (1.0 - 2.0 * e)) / 16.0, (r + e + v * (1.0 - 2.0 * e)) / 32.0]
}

pub fn uv_frac(cell: (u8, u8), u: f32, v: f32) -> [f32; 2] {
    let (r, c) = (cell.0 as f32, cell.1 as f32);
    let e = 0.0004;
    [(c + e + u * (1.0 - 2.0 * e)) / 16.0, (r + e + v * (1.0 - 2.0 * e)) / 32.0]
}

/// Texture coordinate (`axis` 0 = u, 1 = v) of corner `k` of `face` for a point `rel` of the cell: the cell axis that
/// runs along that texture direction, flipped when the corner order runs against it.
pub(crate) fn axis_frac(face: usize, k: usize, axis: usize, rel: [f32; 3]) -> f32 {
    let corners = FACE_CORNERS[face];
    let want = |i: usize| CORNER_UV[i][axis];
    for a in 0..3 {
        if (0..4).all(|i| corners[i][a] == want(i)) {
            return rel[a];
        }
        if (0..4).all(|i| corners[i][a] == 1.0 - want(i)) {
            return 1.0 - rel[a];
        }
    }
    let _ = k;
    want(k)
}

#[derive(Clone, Copy, PartialEq)]
enum Lighting {
    /// `tesselateBlockInWorldWithAmbienceOcclusionTexLighting`
    Smooth,
    /// one light value per face (neighbour cell, or own cell when the neighbour is opaque)
    Flat,
}

fn add_cube_face(out: &mut MeshData, cx: &Ctx, s: &Style, x: i32, y: i32, z: i32, face: usize, min: [f32; 3], max: [f32; 3], light: Lighting, overlay: Option<((u8, u8), Tint)>) {
    let tint = tint_at(cx.lv, s.tint[face], x, z);
    draw_face(out, cx, s.cells[face], tint, s.translucent, x, y, z, face, min, max, light, 0.0, s.rot[face]);
    if let Some((cell, tn)) = overlay {
        let tint = tint_at(cx.lv, tn, x, z);
        draw_face(out, cx, cell, tint, false, x, y, z, face, min, max, light, 0.002, 0);
    }
}

/// Quarter turns of a texture on `face` so that its upper edge points along `up` (a unit axis vector).
pub fn rot_for_up(face: usize, up: [f32; 3]) -> u8 {
    let c = FACE_CORNERS[face];
    for r in 0..4usize {
        // vertices showing the texture's top edge (uv v = 0): those whose shifted corner index is 2 or 3
        let (mut top, mut bot) = ([0.0f32; 3], [0.0f32; 3]);
        for k in 0..4 {
            let uvi = (k + r) % 4;
            let dst = if uvi >= 2 { &mut top } else { &mut bot };
            for a in 0..3 {
                dst[a] += c[k][a] / 2.0;
            }
        }
        let d = [top[0] - bot[0], top[1] - bot[1], top[2] - bot[2]];
        if d[0] * up[0] + d[1] * up[1] + d[2] * up[2] > 0.99 {
            return r as u8;
        }
    }
    0
}

#[allow(clippy::too_many_arguments)]
fn draw_face(out: &mut MeshData, cx: &Ctx, cell: (u8, u8), tint: [f32; 3], _translucent: bool, x: i32, y: i32, z: i32, face: usize, min: [f32; 3], max: [f32; 3], light: Lighting, push: f32, rot: u8) {
    let n = FACE_NORMAL[face];
    let base = FACE_SHADE[face];
    let axes = FACE_AXES[face];
    let mut verts = [Vertex::default(); 4];
    let mut sums = [0.0f32; 4];
    // does the face sit on the block boundary? (then the light is taken from the neighbouring cell)
    let on_edge = match face {
        0 => min[0] <= 0.0,
        1 => max[0] >= 1.0,
        2 => min[1] <= 0.0,
        3 => max[1] >= 1.0,
        4 => min[2] <= 0.0,
        _ => max[2] >= 1.0,
    };
    let nb = [x + n[0], y + n[1], z + n[2]];
    let centre_cell = if on_edge { nb } else { [x, y, z] };
    let nb_opaque = cx.occludes(nb[0], nb[1], nb[2]);
    // centre light: neighbour layer unless it is a solid render tile (then own cell), as in the original
    let c0 = if on_edge && !nb_opaque { cx.lc(centre_cell[0], centre_cell[1], centre_cell[2]) } else if on_edge { cx.lc(x, y, z) } else { cx.lc(x, y, z) };
    let s0 = cx.shade(centre_cell[0], centre_cell[1], centre_cell[2]);
    let partial = min != [0.0; 3] || max != [1.0; 3];
    // bit 4 of `rot`: the icon is mirrored horizontally (`FlippedIcon`, door leaves)
    let flip = rot & 0x10 != 0;
    let rot = rot & 3;
    for (k, corner) in FACE_CORNERS[face].iter().enumerate() {
        let p = [
            x as f32 + min[0] + corner[0] * (max[0] - min[0]) + n[0] as f32 * push,
            y as f32 + min[1] + corner[1] * (max[1] - min[1]) + n[1] as f32 * push,
            z as f32 + min[2] + corner[2] * (max[2] - min[2]) + n[2] as f32 * push,
        ];
        let (mut lt, mut sh) = ([c0.0 as f32, c0.1 as f32], 1.0f32);
        if light == Lighting::Smooth {
            let sgn = |ax: usize| if corner[ax] > 0.5 { 1 } else { -1 };
            let (a1, a2) = (axes[0], axes[1]);
            let mut o1 = centre_cell;
            o1[a1] += sgn(a1);
            let mut o2 = centre_cell;
            o2[a2] += sgn(a2);
            let mut oc = o1;
            oc[a2] += sgn(a2);
            let (l1, h1) = (cx.lc(o1[0], o1[1], o1[2]), cx.shade(o1[0], o1[1], o1[2]));
            let (l2, h2) = (cx.lc(o2[0], o2[1], o2[2]), cx.shade(o2[0], o2[1], o2[2]));
            let t1 = cx.translucent(o1[0], o1[1], o1[2]);
            let t2 = cx.translucent(o2[0], o2[1], o2[2]);
            let (lcn, hcn) = if t1 || t2 { (cx.lc(oc[0], oc[1], oc[2]), cx.shade(oc[0], oc[1], oc[2])) } else { (l1, h1) };
            sh = (hcn + h1 + h2 + s0) / 4.0;
            lt = blend(lcn, l1, l2, c0);
        }
        sums[k] = sh;
        let b = base * sh;
        let uv = if partial {
            // the original crops the texture to the box: u/v follow the corner's absolute position inside the cell
            let rel = [min[0] + corner[0] * (max[0] - min[0]), min[1] + corner[1] * (max[1] - min[1]), min[2] + corner[2] * (max[2] - min[2])];
            let (mut u, mut v) = (axis_frac(face, k, 0, rel), axis_frac(face, k, 1, rel));
            if flip {
                u = 1.0 - u;
            }
            for _ in 0..rot {
                let nu = v;
                v = 1.0 - u;
                u = nu;
            }
            uv_frac(cell, u.clamp(0.0, 1.0), v.clamp(0.0, 1.0))
        } else {
            uv_of(cell, (k + rot as usize) % 4)
        };
        verts[k] = Vertex { pos: p, uv, color: [tint[0] * b, tint[1] * b, tint[2] * b, 1.0], light: lt };
    }
    // flip the quad diagonal to follow the brighter corners (`renderFace*` in the original keeps a fixed split; this avoids artefacts)
    let flip = sums[0] + sums[2] < sums[1] + sums[3];
    out.quad(verts, flip);
}

/// Both windings of a quad (the original draws these thin models without culling).
fn quad2(out: &mut MeshData, v: [Vertex; 4]) {
    out.quad(v, false);
    out.quad([v[3], v[2], v[1], v[0]], false);
}

/// `TileRenderer::tesselateTorch`: four thin vertical planes sheared by (`xxa`, `zza`) plus a small top and bottom
/// square. `(x, y, z)` is the cell corner, the texture is the whole `cell`.
fn torch_quads(out: &mut MeshData, cell: (u8, u8), light: [f32; 2], x: f32, y: f32, z: f32, xxa: f32, zza: f32) {
    let col = [1.0, 1.0, 1.0, 1.0];
    let uv = |u: f32, v: f32| uv_frac(cell, u / 16.0, v / 16.0);
    let (u0, v0, u1, v1) = (0.0, 0.0, 16.0, 16.0);
    let (ut0, vt0, ut1, vt1) = (7.0, 6.0, 9.0, 8.0);
    let (ub0, vb0, ub1, vb1) = (7.0, 13.0, 9.0, 15.0);
    let (x, z) = (x + 0.5, z + 0.5);
    let (xa, xb, za, zb) = (x - 0.5, x + 0.5, z - 0.5, z + 0.5);
    let r = 1.0 / 16.0;
    let h = 10.0 / 16.0;
    let mk = |px: f32, py: f32, pz: f32, u: f32, v: f32| Vertex { pos: [px, py, pz], uv: uv(u, v), color: col, light };
    // top square
    let (cx, cz) = (x + xxa * (1.0 - h), z + zza * (1.0 - h));
    quad2(out, [mk(cx - r, y + h, cz - r, ut0, vt0), mk(cx - r, y + h, cz + r, ut0, vt1), mk(cx + r, y + h, cz + r, ut1, vt1), mk(cx + r, y + h, cz - r, ut1, vt0)]);
    // bottom square
    quad2(out, [mk(x + r + xxa, y, z - r + zza, ub1, vb0), mk(x + r + xxa, y, z + r + zza, ub1, vb1), mk(x - r + xxa, y, z + r + zza, ub0, vb1), mk(x - r + xxa, y, z - r + zza, ub0, vb0)]);
    // four planes
    quad2(out, [mk(x - r, y + 1.0, za, u0, v0), mk(x - r + xxa, y, za + zza, u0, v1), mk(x - r + xxa, y, zb + zza, u1, v1), mk(x - r, y + 1.0, zb, u1, v0)]);
    quad2(out, [mk(x + r, y + 1.0, zb, u0, v0), mk(x + xxa + r, y, zb + zza, u0, v1), mk(x + xxa + r, y, za + zza, u1, v1), mk(x + r, y + 1.0, za, u1, v0)]);
    quad2(out, [mk(xa, y + 1.0, z + r, u0, v0), mk(xa + xxa, y, z + r + zza, u0, v1), mk(xb + xxa, y, z + r + zza, u1, v1), mk(xb, y + 1.0, z + r, u1, v0)]);
    quad2(out, [mk(xb, y + 1.0, z - r, u0, v0), mk(xb + xxa, y, z - r + zza, u0, v1), mk(xa + xxa, y, z - r + zza, u1, v1), mk(xa, y + 1.0, z - r, u1, v0)]);
}

/// `TileRenderer::tesselateTorchInWorld`.
fn add_torch(out: &mut MeshData, cx: &Ctx, s: &Style, x: i32, y: i32, z: i32, data: u8) {
    let (sky, blk) = cx.lc(x, y, z);
    let light = [sky as f32, blk as f32];
    let (r, r2, h) = (0.40f32, 0.5f32 - 0.40, 0.20f32);
    let (fx, fy, fz) = (x as f32, y as f32, z as f32);
    match data {
        1 => torch_quads(out, s.cells[0], light, fx - r2, fy + h, fz, -r, 0.0),
        2 => torch_quads(out, s.cells[0], light, fx + r2, fy + h, fz, r, 0.0),
        3 => torch_quads(out, s.cells[0], light, fx, fy + h, fz - r2, 0.0, -r),
        4 => torch_quads(out, s.cells[0], light, fx, fy + h, fz + r2, 0.0, r),
        _ => torch_quads(out, s.cells[0], light, fx, fy, fz, 0.0, 0.0),
    }
}

/// The extras of a repeater: its two torches (`tesselateRepeaterInWorld`); the slab is an ordinary part.
fn add_repeater_torches(out: &mut MeshData, cx: &Ctx, x: i32, y: i32, z: i32, data: u8) {
    let Some(cell) = crate::icons::icon("redstone_torch_on") else { return };
    let (sky, blk) = cx.lc(x, y, z);
    let light = [sky as f32, blk as f32];
    let delay = ((data >> 2) & 3) as usize;
    let off = [-1.0f32 / 16.0, 1.0 / 16.0, 3.0 / 16.0, 5.0 / 16.0][delay];
    let (mut tx, mut tz, mut rx, mut rz) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    match data & 3 {
        0 => {
            rz = -5.0 / 16.0;
            tz = off;
        }
        2 => {
            rz = 5.0 / 16.0;
            tz = -off;
        }
        3 => {
            rx = -5.0 / 16.0;
            tx = off;
        }
        _ => {
            rx = 5.0 / 16.0;
            tx = -off;
        }
    }
    let h = -3.0f32 / 16.0;
    torch_quads(out, cell, light, x as f32 + tx, y as f32 + h, z as f32 + tz, 0.0, 0.0);
    torch_quads(out, cell, light, x as f32 + rx, y as f32 + h, z as f32 + rz, 0.0, 0.0);
}

/// The stick of a lever (`tesselateLeverInWorld`): a 2×2 px box 10/16 long, tilted 40° and placed per attachment.
fn add_lever_handle(out: &mut MeshData, cx: &Ctx, x: i32, y: i32, z: i32, data: u8) {
    let Some(cell) = crate::icons::icon("lever") else { return };
    let (sky, blk) = cx.lc(x, y, z);
    let light = [sky as f32, blk as f32];
    let dir = data & 7;
    let flipped = data & 8 != 0;
    let (xv, zv, yv) = (1.0f64 / 16.0, 1.0f64 / 16.0, 10.0f64 / 16.0);
    let mut c: [[f64; 3]; 8] = [
        [-xv, 0.0, -zv], [xv, 0.0, -zv], [xv, 0.0, zv], [-xv, 0.0, zv],
        [-xv, yv, -zv], [xv, yv, -zv], [xv, yv, zv], [-xv, yv, zv],
    ];
    let x_rot = |p: &mut [f64; 3], a: f64| {
        let (co, si) = (a.cos(), a.sin());
        let (yy, zz) = (p[1] * co + p[2] * si, p[2] * co - p[1] * si);
        p[1] = yy;
        p[2] = zz;
    };
    let y_rot = |p: &mut [f64; 3], a: f64| {
        let (co, si) = (a.cos(), a.sin());
        let (xx, zz) = (p[0] * co + p[2] * si, p[2] * co - p[0] * si);
        p[0] = xx;
        p[2] = zz;
    };
    let z_rot = |p: &mut [f64; 3], a: f64| {
        let (co, si) = (a.cos(), a.sin());
        let (xx, yy) = (p[0] * co + p[1] * si, p[1] * co - p[0] * si);
        p[0] = xx;
        p[1] = yy;
    };
    let rad = |d: f64| d * std::f64::consts::PI / 180.0;
    let (fx, fy, fz) = (x as f64, y as f64, z as f64);
    for p in c.iter_mut() {
        if flipped {
            p[2] -= 1.0 / 16.0;
            x_rot(p, rad(40.0));
        } else {
            p[2] += 1.0 / 16.0;
            x_rot(p, rad(-40.0));
        }
        if dir == 0 || dir == 7 {
            z_rot(p, rad(180.0));
        }
        if dir == 6 || dir == 0 {
            y_rot(p, rad(90.0));
        }
        if dir > 0 && dir < 5 {
            p[1] -= 6.0 / 16.0;
            x_rot(p, rad(90.0));
            match dir {
                3 => y_rot(p, rad(180.0)),
                2 => y_rot(p, rad(90.0)),
                1 => y_rot(p, rad(-90.0)),
                _ => {}
            }
            p[0] += fx + 0.5;
            p[1] += fy + 0.5;
            p[2] += fz + 0.5;
        } else if dir == 0 || dir == 7 {
            p[0] += fx + 0.5;
            p[1] += fy + 14.0 / 16.0;
            p[2] += fz + 0.5;
        } else {
            p[0] += fx + 0.5;
            p[1] += fy + 2.0 / 16.0;
            p[2] += fz + 0.5;
        }
    }
    let faces: [[usize; 4]; 6] = [[0, 1, 2, 3], [7, 6, 5, 4], [1, 0, 4, 5], [2, 1, 5, 6], [3, 2, 6, 7], [0, 3, 7, 4]];
    for (i, f) in faces.iter().enumerate() {
        let (u0, v0, u1, v1) = if i < 2 { (7.0, 6.0, 9.0, 8.0) } else { (7.0, 6.0, 9.0, 16.0) };
        let uv = |u: f32, v: f32| uv_frac(cell, u / 16.0, v / 16.0);
        let mk = |k: usize, u: f32, v: f32| Vertex { pos: [c[f[k]][0] as f32, c[f[k]][1] as f32, c[f[k]][2] as f32], uv: uv(u, v), color: [1.0; 4], light };
        quad2(out, [mk(0, u0, v1), mk(1, u1, v1), mk(2, u1, v0), mk(3, u0, v0)]);
    }
}

/// Redstone dust (`TileRenderer::tesselateDustInWorld`): cross or line icon trimmed by the connections, tinted by the
/// power, with the overlay on top, plus the strips that climb a wall to dust above.
fn add_dust(out: &mut MeshData, cx: &Ctx, x: i32, y: i32, z: i32, data: u8) {
    let lv = cx.lv;
    let nb = |p: (i32, i32, i32)| if (0..256).contains(&p.1) { (lv.get_tile(p.0, p.1, p.2), lv.get_data(p.0, p.1, p.2)) } else { (0, 0) };
    let conn = crate::redstone::wire_connections(&nb, (x, y, z));
    let (w, e, n, s) = (conn[0], conn[1], conn[2], conn[3]);
    let colour = crate::redstone::wire_colour(data & 15);
    let (sky, blk) = cx.lc(x, y, z);
    let light = [sky as f32, blk as f32];
    let (Some(cross), Some(line), Some(cross_o), Some(line_o)) = (
        crate::icons::icon("redstone_dust_cross"),
        crate::icons::icon("redstone_dust_line"),
        crate::icons::icon("redstone_dust_cross_overlay"),
        crate::icons::icon("redstone_dust_line_overlay"),
    ) else {
        return;
    };
    let off = 0.25 / 16.0;
    let (fx, fy, fz) = (x as f32, y as f32, z as f32);
    let tint = [colour[0], colour[1], colour[2], 1.0];
    let white = [1.0f32; 4];
    let pic = if (w || e) && (!n && !s) { 1 } else if (n || s) && (!e && !w) { 2 } else { 0 };
    let uvp = |cell: (u8, u8), u: f32, v: f32| uv_frac(cell, u / 16.0, v / 16.0);
    let flat = |out: &mut MeshData, cell: (u8, u8), col: [f32; 4], pts: [(f32, f32, f32, f32); 4]| {
        let v: Vec<Vertex> = pts.iter().map(|&(px, pz, u, vv)| Vertex { pos: [px, fy + off, pz], uv: uvp(cell, u, vv), color: col, light }).collect();
        quad2(out, [v[0], v[1], v[2], v[3]]);
    };
    let (x0, x1, z0, z1) = (fx, fx + 1.0, fz, fz + 1.0);
    if pic == 0 {
        let (mut cu0, mut cv0, mut cu1, mut cv1) = (0.0f32, 0.0f32, 16.0f32, 16.0f32);
        let (mut ax0, mut ax1, mut az0, mut az1) = (x0, x1, z0, z1);
        let cut = 5.0f32;
        if !w {
            ax0 += cut / 16.0;
            cu0 += cut;
        }
        if !e {
            ax1 -= cut / 16.0;
            cu1 -= cut;
        }
        if !n {
            az0 += cut / 16.0;
            cv0 += cut;
        }
        if !s {
            az1 -= cut / 16.0;
            cv1 -= cut;
        }
        for (cell, col) in [(cross, tint), (cross_o, white)] {
            flat(out, cell, col, [(ax1, az1, cu1, cv1), (ax1, az0, cu1, cv0), (ax0, az0, cu0, cv0), (ax0, az1, cu0, cv1)]);
        }
    } else if pic == 1 {
        for (cell, col) in [(line, tint), (line_o, white)] {
            flat(out, cell, col, [(x1, z1, 16.0, 16.0), (x1, z0, 16.0, 0.0), (x0, z0, 0.0, 0.0), (x0, z1, 0.0, 16.0)]);
        }
    } else {
        for (cell, col) in [(line, tint), (line_o, white)] {
            flat(out, cell, col, [(x1, z1, 16.0, 16.0), (x1, z0, 0.0, 16.0), (x0, z0, 0.0, 0.0), (x0, z1, 16.0, 0.0)]);
        }
    }
    // strips up the side of a solid block carrying dust on top
    if !cx.occludes(x, y + 1, z) {
        let ys = 0.35 / 16.0;
        let dust_on_top = |dx: i32, dz: i32| cx.occludes(x + dx, y, z + dz) && nb((x + dx, y + 1, z + dz)).0 == mc_world::tile_ids::RED_STONE_DUST;
        let side = |out: &mut MeshData, pts: [([f32; 3], f32, f32); 4]| {
            for (cell, col) in [(line, tint), (line_o, white)] {
                let v: Vec<Vertex> = pts.iter().map(|&(p, u, vv)| Vertex { pos: p, uv: uvp(cell, u, vv), color: col, light }).collect();
                quad2(out, [v[0], v[1], v[2], v[3]]);
            }
        };
        let top = fy + 1.0 + ys;
        if dust_on_top(-1, 0) {
            let px = fx + off;
            side(out, [([px, top, fz + 1.0], 16.0, 0.0), ([px, fy, fz + 1.0], 0.0, 0.0), ([px, fy, fz], 0.0, 16.0), ([px, top, fz], 16.0, 16.0)]);
        }
        if dust_on_top(1, 0) {
            let px = fx + 1.0 - off;
            side(out, [([px, fy, fz + 1.0], 0.0, 16.0), ([px, top, fz + 1.0], 16.0, 16.0), ([px, top, fz], 16.0, 0.0), ([px, fy, fz], 0.0, 0.0)]);
        }
        if dust_on_top(0, -1) {
            let pz = fz + off;
            side(out, [([fx + 1.0, fy, pz], 0.0, 16.0), ([fx + 1.0, top, pz], 16.0, 16.0), ([fx, top, pz], 16.0, 0.0), ([fx, fy, pz], 0.0, 0.0)]);
        }
        if dust_on_top(0, 1) {
            let pz = fz + 1.0 - off;
            side(out, [([fx + 1.0, top, pz], 16.0, 0.0), ([fx + 1.0, fy, pz], 0.0, 0.0), ([fx, fy, pz], 0.0, 16.0), ([fx, top, pz], 16.0, 16.0)]);
        }
    }
}

fn add_cross(out: &mut MeshData, cx: &Ctx, s: &Style, x: i32, y: i32, z: i32) {
    let tint = tint_at(cx.lv, s.tint[0], x, z);
    let col = [tint[0], tint[1], tint[2], 1.0];
    let (sky, blk) = cx.lc(x, y, z);
    let light = [sky as f32, blk as f32];
    let (fx, fy, fz) = (x as f32, y as f32, z as f32);
    for diag in 0..2 {
        let (a, bq) = if diag == 0 { ([0.0f32, 0.0], [1.0f32, 1.0]) } else { ([0.0, 1.0], [1.0, 0.0]) };
        let mk = |px: f32, pz: f32, py: f32, k: usize| Vertex { pos: [fx + px, fy + py, fz + pz], uv: uv_of(s.cells[0], k), color: col, light };
        let v = [mk(a[0], a[1], 0.0, 0), mk(bq[0], bq[1], 0.0, 1), mk(bq[0], bq[1], 1.0, 2), mk(a[0], a[1], 1.0, 3)];
        out.quad(v, false);
        let v2 = [v[1], v[0], v[3], v[2]];
        out.quad([Vertex { uv: uv_of(s.cells[0], 0), ..v2[0] }, Vertex { uv: uv_of(s.cells[0], 1), ..v2[1] }, Vertex { uv: uv_of(s.cells[0], 2), ..v2[2] }, Vertex { uv: uv_of(s.cells[0], 3), ..v2[3] }], false);
    }
}

fn add_flat(out: &mut MeshData, cx: &Ctx, s: &Style, x: i32, y: i32, z: i32) {
    let tint = tint_at(cx.lv, s.tint[3], x, z);
    let (fx, fy, fz) = (x as f32, y as f32 + 0.03, z as f32);
    let col = [tint[0], tint[1], tint[2], 1.0];
    let (sky, blk) = cx.lc(x, y, z);
    let light = [sky as f32, blk as f32];
    let mk = |px: f32, pz: f32, k: usize| Vertex { pos: [fx + px, fy, fz + pz], uv: uv_of(s.cells[3], k), color: col, light };
    out.quad([mk(0.0, 1.0, 0), mk(1.0, 1.0, 1), mk(1.0, 0.0, 2), mk(0.0, 0.0, 3)], false);
}

/// Water/lava surface height: full cubes are used under another liquid, otherwise slightly lowered.
/// `LiquidTile::getHeight`: fraction of a block filled for a flow level (source / falling = 8/9).
fn liquid_height(d: u8) -> f32 {
    let d = if d >= 8 { 0 } else { d };
    (d as f32 + 1.0) / 9.0
}

/// `TileRenderer::getWaterHeight`: height of the liquid surface at the corner (x, z) shared by four blocks.
fn water_height(lv: &Level, x: i32, y: i32, z: i32, same: &dyn Fn(u8) -> bool) -> f32 {
    let (mut count, mut h) = (0.0f32, 0.0f32);
    for i in 0..4 {
        let (xx, zz) = (x - (i & 1), z - ((i >> 1) & 1));
        if same(lv.get_tile(xx, y + 1, zz)) {
            return 1.0;
        }
        let id = lv.get_tile(xx, y, zz);
        if same(id) {
            let d = lv.get_data(xx, y, zz);
            if d >= 8 || d == 0 {
                h += liquid_height(d) * 10.0;
                count += 10.0;
            }
            h += liquid_height(d);
            count += 1.0;
        } else if !tile_props::material(id).is_solid() {
            h += 1.0;
            count += 1.0;
        }
    }
    1.0 - h / count
}

/// `TileRenderer::tesselateWaterInWorld` (the animated flow texture is not reproduced: the still cell is used everywhere).
fn add_liquid(out_opaque: &mut MeshData, out_trans: &mut MeshData, cx: &Ctx, s: &Style, id: u8, x: i32, y: i32, z: i32) {
    let water = id == mc_world::tile_ids::WATER || id == mc_world::tile_ids::CALM_WATER;
    let same = move |i: u8| {
        if water {
            i == mc_world::tile_ids::WATER || i == mc_world::tile_ids::CALM_WATER
        } else {
            i == mc_world::tile_ids::LAVA || i == mc_world::tile_ids::CALM_LAVA
        }
    };
    let out = if s.translucent { out_trans } else { out_opaque };
    // LiquidTile::shouldRenderFace
    let show = |nx: i32, ny: i32, nz: i32, up: bool| -> bool {
        let n = cx.lv.get_tile(nx, ny, nz);
        if same(n) {
            return false;
        }
        if up {
            return true;
        }
        if n == mc_world::tile_ids::ICE {
            return false;
        }
        !style(n, 0).opaque
    };
    let up = cx.lv.get_tile(x, y + 1, z) != mc_world::tile_ids::ICE && show(x, y + 1, z, true) && !(cx.lv.get_tile(x, y + 1, z) != 0 && style(cx.lv.get_tile(x, y + 1, z), 0).opaque);
    let down = show(x, y - 1, z, false);
    let dirs = [show(x, y, z - 1, false), show(x, y, z + 1, false), show(x - 1, y, z, false), show(x + 1, y, z, false)];
    if !up && !down && !dirs.iter().any(|d| *d) {
        return;
    }
    let (h0, h1, h2, h3) = (water_height(cx.lv, x, y, z, &same), water_height(cx.lv, x, y, z + 1, &same), water_height(cx.lv, x + 1, y, z + 1, &same), water_height(cx.lv, x + 1, y, z, &same));
    let offs = 0.001f32;
    let cell = s.cells[0];
    let (fx, fy, fz) = (x as f32, y as f32, z as f32);
    let rgb = |face: usize, shade: f32| {
        let t = tint_at(cx.lv, s.tint[face], x, z);
        [t[0] * shade, t[1] * shade, t[2] * shade, 1.0]
    };
    let lc = |a: i32, b: i32, c: i32| {
        let (sky, blk) = cx.lc(a, b, c);
        [sky as f32, blk as f32]
    };
    if up {
        let col = rgb(3, 1.0);
        let light = lc(x, y, z);
        let v = |px: f32, ph: f32, pz: f32, k: usize| Vertex { pos: [fx + px, fy + ph - offs, fz + pz], uv: uv_of(cell, k), color: col, light };
        let q = [v(0.0, h0, 0.0, 3), v(0.0, h1, 1.0, 0), v(1.0, h2, 1.0, 1), v(1.0, h3, 0.0, 2)];
        let first = out.indices.len();
        out.quad(q, false);
        // the surface is also seen from below while swimming
        let tris: Vec<u32> = out.indices[first..].chunks(3).flat_map(|t| [t[0], t[2], t[1]]).collect();
        out.indices.extend(tris);
    }
    if down {
        let col = rgb(2, 0.5);
        let light = lc(x, y - 1, z);
        let q: Vec<Vertex> = FACE_CORNERS[2].iter().enumerate().map(|(k, c)| Vertex { pos: [fx + c[0], fy + offs, fz + c[2]], uv: uv_of(cell, k), color: col, light }).collect();
        out.quad([q[0], q[1], q[2], q[3]], false);
    }
    for (face, shown) in dirs.iter().enumerate() {
        if !*shown {
            continue;
        }
        let (hh0, hh1, x0, z0, x1, z1) = match face {
            0 => (h0, h3, fx, fz + offs, fx + 1.0, fz + offs),
            1 => (h2, h1, fx + 1.0, fz + 1.0 - offs, fx, fz + 1.0 - offs),
            2 => (h1, h0, fx + offs, fz + 1.0, fx + offs, fz),
            _ => (h3, h2, fx + 1.0 - offs, fz, fx + 1.0 - offs, fz + 1.0),
        };
        let (nx, nz) = match face {
            0 => (x, z - 1),
            1 => (x, z + 1),
            2 => (x - 1, z),
            _ => (x + 1, z),
        };
        let shade = if face < 2 { 0.8 } else { 0.6 };
        let col = rgb(face + 2, shade);
        let light = lc(nx, y, nz);
        // texture rows follow the surface height so the pattern does not stretch
        let uvp = |u: f32, v: f32| {
            let (r, c) = (cell.0 as f32, cell.1 as f32);
            let e = 0.0004;
            [(c + e + u * (1.0 - 2.0 * e)) / 16.0, (r + e + v * (1.0 - 2.0 * e)) / 32.0]
        };
        let v = |px: f32, py: f32, pz: f32, u: f32, vv: f32| Vertex { pos: [px, py, pz], uv: uvp(u, vv), color: col, light };
        out.quad(
            [
                v(x0, fy + hh0, z0, 0.0, 1.0 - hh0),
                v(x1, fy + hh1, z1, 1.0, 1.0 - hh1),
                v(x1, fy, z1, 1.0, 1.0),
                v(x0, fy, z0, 0.0, 1.0),
            ],
            false,
        );
    }
}

pub fn mesh_chunk(lv: &Level, cx_: i32, cz_: i32) -> ChunkMesh {
    let mut mesh = ChunkMesh::default();
    let Some(chunk) = lv.chunk(cx_, cz_) else { return mesh };
    let ctx = Ctx { lv };
    let (x0, z0) = (cx_ * 16, cz_ * 16);
    let top = chunk.highest_non_empty_y().clamp(0, 255) + 1;
    for lx in 0..16 {
        for lz in 0..16 {
            let (x, z) = (x0 + lx, z0 + lz);
            for y in 0..top.min(256) {
                let id = chunk.get_tile(lx, y, lz);
                if id == 0 {
                    continue;
                }
                let data = chunk.get_data(lx, y, lz);
                let s = style(id, data);
                match s.kind {
                    Kind::Air => {}
                    Kind::Cube => {
                        for face in 0..6 {
                            let n = FACE_NORMAL[face];
                            let (nx, ny, nz) = (x + n[0], y + n[1], z + n[2]);
                            if ctx.occludes(nx, ny, nz) {
                                continue;
                            }
                            // identical see-through neighbours (glass, leaves of the same kind) don't show inner faces for glass
                            let nid = if (0..256).contains(&ny) { lv.get_tile(nx, ny, nz) } else { 0 };
                            if nid == id && (id == mc_world::tile_ids::GLASS || id == mc_world::tile_ids::ICE || id == mc_world::tile_ids::STAINED_GLASS) {
                                continue;
                            }
                            let overlay = (id == mc_world::tile_ids::GRASS && face != 2 && face != 3).then(|| (crate::icons::icon("grass_side_overlay").unwrap_or((2, 6)), Tint::Grass));
                            let out = if s.translucent { &mut mesh.translucent } else { &mut mesh.opaque };
                            add_cube_face(out, &ctx, s, x, y, z, face, [0.0; 3], [1.0; 3], Lighting::Smooth, overlay);
                        }
                    }
                    Kind::Box { min, max } => {
                        for face in 0..6 {
                            let n = FACE_NORMAL[face];
                            let (nx, ny, nz) = (x + n[0], y + n[1], z + n[2]);
                            // a face on the block boundary is hidden by an opaque neighbour
                            let on_edge = match face {
                                0 => min[0] == 0.0,
                                1 => max[0] == 1.0,
                                2 => min[1] == 0.0,
                                3 => max[1] == 1.0,
                                4 => min[2] == 0.0,
                                _ => max[2] == 1.0,
                            };
                            if on_edge && ctx.occludes(nx, ny, nz) {
                                continue;
                            }
                            let out = if s.translucent { &mut mesh.translucent } else { &mut mesh.opaque };
                            add_cube_face(out, &ctx, s, x, y, z, face, min, max, Lighting::Flat, None);
                        }
                    }
                    // a sign with a tile entity is drawn by the entity layer (`entrender::sign_layers`)
                    Kind::Shaped if matches!(id, mc_world::tile_ids::SIGN | mc_world::tile_ids::WALL_SIGN) && lv.tile_entities.contains_key(&(x, y, z)) => {}
                    Kind::Shaped => {
                        let nbf = |a: i32, b: i32, c: i32| if (0..256).contains(&b) { (lv.get_tile(a, b, c), lv.get_data(a, b, c)) } else { (0, 0) };
                        for part in crate::shapes::parts_in(id, data, x, y, z, &nbf) {
                            let cells = part.cells.unwrap_or(s.cells);
                            let ps = Style { cells, rot: part.rot, ..*s };
                            for face in 0..6 {
                                let n = FACE_NORMAL[face];
                                let (nx, ny, nz) = (x + n[0], y + n[1], z + n[2]);
                                let on_edge = match face {
                                    0 => part.min[0] <= 0.0,
                                    1 => part.max[0] >= 1.0,
                                    2 => part.min[1] <= 0.0,
                                    3 => part.max[1] >= 1.0,
                                    4 => part.min[2] <= 0.0,
                                    _ => part.max[2] >= 1.0,
                                };
                                if on_edge && ctx.occludes(nx, ny, nz) {
                                    continue;
                                }
                                add_cube_face(&mut mesh.opaque, &ctx, &ps, x, y, z, face, part.min, part.max, Lighting::Flat, None);
                            }
                        }
                        match id {
                            mc_world::tile_ids::LEVER => add_lever_handle(&mut mesh.opaque, &ctx, x, y, z, data),
                            mc_world::tile_ids::DIODE_OFF | mc_world::tile_ids::DIODE_ON => add_repeater_torches(&mut mesh.opaque, &ctx, x, y, z, data),
                            _ => {}
                        }
                    }
                    Kind::Cross if matches!(id, mc_world::tile_ids::TORCH | mc_world::tile_ids::REDSTONE_TORCH_ON | mc_world::tile_ids::REDSTONE_TORCH_OFF) => add_torch(&mut mesh.opaque, &ctx, s, x, y, z, data),
                    Kind::Cross => add_cross(&mut mesh.opaque, &ctx, s, x, y, z),
                    Kind::Flat if id == mc_world::tile_ids::RED_STONE_DUST => add_dust(&mut mesh.opaque, &ctx, x, y, z, data),
                    Kind::Flat => add_flat(&mut mesh.opaque, &ctx, s, x, y, z),
                    Kind::Liquid => {
                        let (a, b) = (&mut mesh.opaque, &mut mesh.translucent);
                        add_liquid(a, b, &ctx, s, id, x, y, z);
                    }
                }
            }
        }
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_level::chunk::LevelChunk;
    use mc_world::tile_ids as t;

    fn level_with(blocks: &[((i32, i32, i32), u8)]) -> Level {
        let mut lv = Level::new(54, false);
        lv.lighting = false;
        let mut raw = vec![0u8; 32768];
        for &((x, y, z), id) in blocks {
            raw[((x as usize) << 11) | ((z as usize) << 7) | y as usize] = id;
        }
        let mut c = LevelChunk::from_blocks(0, 0, &raw);
        c.recalc_heightmap(false);
        lv.insert_chunk(c);
        lv
    }

    #[test]
    fn stairs_and_doors_are_meshed() {
        for id in [t::STAIRS_WOOD, t::DOOR_WOOD, t::TRAPDOOR, t::FENCE_GATE, t::LEVER, t::PRESSURE_PLATE_STONE, t::CAKE, t::DIODE_OFF] {
            let lv = level_with(&[((5, 5, 5), id)]);
            let m = mesh_chunk(&lv, 0, 0);
            assert!(m.opaque.indices.len() >= 6 * 6, "{id}: {}", m.opaque.indices.len());
        }
    }

    #[test]
    fn neighbours_of_partial_blocks_keep_their_faces() {
        // a stone cube next to an extended piston base / a chest: its face toward them must still be drawn
        for (id, data) in [(t::PISTON_BASE, 9u8), (t::PISTON_STICKY_BASE, 9), (t::CHEST, 2)] {
            let mut lv = level_with(&[((5, 5, 5), t::STONE), ((6, 5, 5), id)]);
            lv.set_data(6, 5, 5, data, 0);
            assert!(!Ctx { lv: &lv }.occludes(6, 5, 5), "{id}: a partial block must not hide its neighbours' faces");
        }
        // a retracted piston base is a full cube again
        let lv = level_with(&[((6, 5, 5), t::PISTON_BASE)]);
        assert!(Ctx { lv: &lv }.occludes(6, 5, 5));
    }

    #[test]
    fn lone_cube_has_six_faces() {
        let lv = level_with(&[((5, 5, 5), t::STONE)]);
        let m = mesh_chunk(&lv, 0, 0);
        assert_eq!(m.opaque.indices.len(), 6 * 6);
        assert!(m.translucent.is_empty());
    }

    #[test]
    fn touching_cubes_hide_shared_faces() {
        let lv = level_with(&[((5, 5, 5), t::STONE), ((6, 5, 5), t::STONE)]);
        let m = mesh_chunk(&lv, 0, 0);
        assert_eq!(m.opaque.indices.len(), 10 * 6);
    }

    #[test]
    fn grass_gets_overlay_on_sides() {
        let lv = level_with(&[((5, 5, 5), t::GRASS)]);
        let m = mesh_chunk(&lv, 0, 0);
        assert_eq!(m.opaque.indices.len(), (6 + 4) * 6);
    }

    #[test]
    fn plants_are_crossed_quads_and_water_is_translucent() {
        let lv = level_with(&[((5, 5, 5), t::FLOWER), ((8, 5, 8), t::CALM_WATER)]);
        let m = mesh_chunk(&lv, 0, 0);
        assert_eq!(m.opaque.indices.len(), 4 * 6);
        assert!(!m.translucent.is_empty());
    }

    #[test]
    fn indices_stay_in_range() {
        let lv = level_with(&[((1, 1, 1), t::STONE), ((2, 1, 1), t::WOOD), ((3, 1, 1), t::STONE_SLAB_HALF)]);
        let m = mesh_chunk(&lv, 0, 0);
        assert!(m.opaque.indices.iter().all(|&i| (i as usize) < m.opaque.vertices.len()));
    }

    #[test]
    fn flowing_water_is_lower_than_a_full_block_and_slopes_towards_the_edge() {
        let mut lv = level_with(&[((8, 5, 8), t::WATER), ((9, 5, 8), t::WATER)]);
        lv.set_data(8, 5, 8, 0, 0);
        lv.set_data(9, 5, 8, 4, 0); // a flowing block, level 4
        let m = mesh_chunk(&lv, 0, 0);
        let ys: Vec<f32> = m.translucent.vertices.iter().map(|v| v.pos[1]).collect();
        let top = ys.iter().cloned().fold(f32::MIN, f32::max);
        assert!(top < 6.0 && top > 5.7, "source surface at 8/9 height: {top}");
        assert!(ys.iter().any(|&y| y < 5.8 && y > 5.2), "the flowing block has a lower corner");
    }
}
