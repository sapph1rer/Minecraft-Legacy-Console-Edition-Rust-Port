//! Particles: `Particle`, `TerrainParticle` and `ParticleEngine` (block-break and block-hit debris).
//!
//! Physics is `Entity::move` for a 0.2 × 0.2 box; rendering is the camera-facing quad of
//! `TerrainParticle::render` sampling a quarter of the block's bottom-face icon, tinted by the block colour
//! (`0.6 × getColor`) and lit by the light at the particle (`Entity::getLightColor`).

use crate::blocks::{style, Cell, Kind};
use crate::mesh::{tint_at, MeshData, Vertex};
use crate::physics::{get_cubes, selection_box};
use mc_core::aabb::Aabb;
use mc_core::random::Random;
use mc_level::level::Level;
use mc_world::tile_ids as t;

/// `ParticleEngine::MAX_PARTICLES_PER_LAYER` (4J reduced it from 4000)
pub const MAX_PARTICLES: usize = 200;

#[derive(Clone, Debug)]
pub struct Particle {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub xo: f64,
    pub yo: f64,
    pub zo: f64,
    pub xd: f64,
    pub yd: f64,
    pub zd: f64,
    pub bb: Aabb,
    pub on_ground: bool,
    pub age: i32,
    pub lifetime: i32,
    pub size: f32,
    pub gravity: f32,
    pub rgb: [f32; 3],
    pub alpha: f32,
    pub uo: f32,
    pub vo: f32,
    /// terrain atlas cell of the icon
    pub cell: Cell,
    /// `PortalParticle`: moves from `start` along (xd, yd, zd) with no collision and shrinks near the end
    pub portal: Option<[f64; 3]>,
    /// `BreakingItemParticle`: (row, col) of an items.png icon instead of a terrain cell
    pub item: Option<(u8, u8)>,
    pub osize: f32,
    /// width of the sampled piece of the cell in quarters (1 = a random quarter like debris, 4 = the whole cell)
    pub span: f32,
    pub removed: bool,
    bb_width: f32,
    bb_height: f32,
    height_offset: f64,
}

impl Particle {
    /// `Particle::Particle(level, x, y, z, xa, ya, za)` (the `_init` part and the random velocity).
    pub fn new(r: &mut Random, x: f64, y: f64, z: f64, xa: f64, ya: f64, za: f64) -> Particle {
        let mut p = Particle {
            x,
            y,
            z,
            xo: x,
            yo: y,
            zo: z,
            xd: 0.0,
            yd: 0.0,
            zd: 0.0,
            bb: Aabb::default(),
            on_ground: false,
            age: 0,
            lifetime: 0,
            size: 0.0,
            gravity: 0.0,
            rgb: [1.0; 3],
            alpha: 1.0,
            uo: 0.0,
            vo: 0.0,
            cell: (0, 0),
            portal: None,
            item: None,
            osize: 0.0,
            span: 1.0,
            removed: false,
            bb_width: 0.0,
            bb_height: 0.0,
            height_offset: 0.0,
        };
        p.set_size(0.2, 0.2);
        p.height_offset = p.bb_height as f64 / 2.0;
        p.set_pos(x, y, z);
        p.uo = r.next_float() * 3.0;
        p.vo = r.next_float() * 3.0;
        p.size = (r.next_float() * 0.5 + 0.5) * 2.0;
        p.lifetime = (4.0 / (r.next_float() * 0.9 + 0.1)) as i32;

        let mut rnd = || (r.next_double() * 2.0 - 1.0) as f32 as f64 * 0.4;
        p.xd = xa + rnd();
        p.yd = ya + rnd();
        p.zd = za + rnd();
        let speed = ((r.next_double() + r.next_double() + 1.0) as f32 * 0.15) as f64;
        let dd = ((p.xd * p.xd + p.yd * p.yd + p.zd * p.zd).sqrt()) as f32 as f64;
        p.xd = p.xd / dd * speed * 0.4;
        p.yd = p.yd / dd * speed * 0.4 + 0.1;
        p.zd = p.zd / dd * speed * 0.4;
        p
    }

    /// `BreakingItemParticle`: a quarter of the item icon, half-size, falling.
    pub fn item_crack(r: &mut Random, x: f64, y: f64, z: f64, xa: f64, ya: f64, za: f64, icon: (u8, u8)) -> Particle {
        let mut p = Particle::new(r, x, y, z, xa, ya, za);
        p.item = Some(icon);
        p.gravity = 1.0;
        p.size /= 2.0;
        p
    }

    /// `PortalParticle::PortalParticle`: purple glow, 40..49 ticks, drifting from (x, y, z) by (xa, ya, za).
    pub fn portal(r: &mut Random, x: f64, y: f64, z: f64, xa: f64, ya: f64, za: f64) -> Particle {
        let mut p = Particle::new(r, x, y, z, xa, ya, za);
        p.xd = xa;
        p.yd = ya;
        p.zd = za;
        p.portal = Some([x, y, z]);
        p.size = r.next_float() * 0.2 + 0.5;
        p.osize = p.size;
        let br = r.next_float() * 0.6 + 0.4;
        p.rgb = [br * 0.9, br * 0.3, br];
        p.lifetime = (r.next_float() * 10.0) as i32 + 40;
        p.gravity = 0.0;
        p.cell = (30, (r.next_float() * 8.0) as u8 as _);
        p.uo = 0.0;
        p.vo = 0.0;
        p.span = 4.0;
        p
    }

    /// `Entity::setSize` (the box grows from its min corner)
    fn set_size(&mut self, w: f32, h: f32) {
        self.bb_width = w;
        self.bb_height = h;
        self.bb.x1 = self.bb.x0 + w as f64;
        self.bb.z1 = self.bb.z0 + w as f64;
        self.bb.y1 = self.bb.y0 + h as f64;
    }

    /// `Entity::setPos`
    fn set_pos(&mut self, x: f64, y: f64, z: f64) {
        self.x = x;
        self.y = y;
        self.z = z;
        let w = (self.bb_width / 2.0) as f64;
        let h = self.bb_height as f64;
        self.bb = Aabb::new(x - w, y - self.height_offset, z - w, x + w, y - self.height_offset + h, z + w);
    }

    /// `Particle::setPower`
    pub fn set_power(&mut self, power: f32) {
        self.xd *= power as f64;
        self.yd = (self.yd - 0.1f32 as f64) * power as f64 + 0.1f32 as f64;
        self.zd *= power as f64;
    }

    /// `Particle::scale`
    pub fn scale(&mut self, s: f32) {
        self.set_size(0.2 * s, 0.2 * s);
        self.size *= s;
    }

    /// `Entity::move` for a plain entity (no step-up, `slide = true`).
    fn move_entity(&mut self, lv: &Level, mut xa: f64, mut ya: f64, mut za: f64) {
        let (xa_org, ya_org, za_org) = (xa, ya, za);
        let cubes = get_cubes(lv, &self.bb.expand(xa, ya, za));
        for c in &cubes {
            ya = c.clip_y_collide(&self.bb, ya);
        }
        self.bb.move_by(0.0, ya, 0.0);
        for c in &cubes {
            xa = c.clip_x_collide(&self.bb, xa);
        }
        self.bb.move_by(xa, 0.0, 0.0);
        for c in &cubes {
            za = c.clip_z_collide(&self.bb, za);
        }
        self.bb.move_by(0.0, 0.0, za);

        self.x = (self.bb.x0 + self.bb.x1) / 2.0;
        self.y = self.bb.y0 + self.height_offset;
        self.z = (self.bb.z0 + self.bb.z1) / 2.0;
        self.on_ground = ya_org != ya && ya_org < 0.0;
        if xa_org != xa {
            self.xd = 0.0;
        }
        if ya_org != ya {
            self.yd = 0.0;
        }
        if za_org != za {
            self.zd = 0.0;
        }
    }

    /// `Particle::tick`
    pub fn tick(&mut self, lv: &Level) {
        self.xo = self.x;
        self.yo = self.y;
        self.zo = self.z;
        if let Some(st) = self.portal {
            let mut pp = self.age as f32 / self.lifetime as f32;
            let a = pp;
            pp = -pp + pp * pp * 2.0;
            pp = 1.0 - pp;
            let pp = pp as f64;
            self.set_pos(st[0] + self.xd * pp, st[1] + self.yd * pp + (1.0 - a as f64), st[2] + self.zd * pp);
            let old = self.age;
            self.age += 1;
            if old >= self.lifetime {
                self.removed = true;
            }
            return;
        }
        let old_age = self.age;
        self.age += 1;
        if old_age >= self.lifetime {
            self.removed = true;
        }
        self.yd -= 0.04 * self.gravity as f64;
        self.move_entity(lv, self.xd, self.yd, self.zd);
        let d = 0.98f32 as f64;
        self.xd *= d;
        self.yd *= d;
        self.zd *= d;
        if self.on_ground {
            let g = 0.7f32 as f64;
            self.xd *= g;
            self.zd *= g;
        }
    }
}

pub struct ParticleEngine {
    pub particles: Vec<Particle>,
    random: Random,
}

impl Default for ParticleEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ParticleEngine {
    pub fn new() -> ParticleEngine {
        ParticleEngine { particles: Vec::new(), random: Random::new() }
    }

    pub fn len(&self) -> usize {
        self.particles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.particles.is_empty()
    }

    /// `ParticleEngine::add`: the oldest particle goes when the layer is full.
    pub fn add(&mut self, p: Particle) {
        if self.particles.len() >= MAX_PARTICLES {
            self.particles.remove(0);
        }
        self.particles.push(p);
    }

    /// `ParticleEngine::tick` (removed particles are replaced by the last one, as in the original).
    pub fn tick(&mut self, lv: &Level) {
        let mut i = 0;
        while i < self.particles.len() {
            self.particles[i].tick(lv);
            if self.particles[i].removed {
                self.particles.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// `TerrainParticle::TerrainParticle` + `init(x, y, z, data)`
    fn terrain_particle(&mut self, lv: &Level, bx: i32, _by: i32, bz: i32, id: u8, data: u8, x: f64, y: f64, z: f64, xa: f64, ya: f64, za: f64) -> Particle {
        let mut p = Particle::new(&mut self.random, x, y, z, xa, ya, za);
        let s = style(id, data);
        p.cell = s.cells[2]; // tile->getTexture(0, data): the bottom face
        p.gravity = 1.0; // Tile::gravity
        p.rgb = [0.6; 3];
        p.size /= 2.0;
        if id != t::GRASS {
            let c = tint_at(lv, s.tint[2], bx, bz);
            p.rgb = [p.rgb[0] * c[0], p.rgb[1] * c[1], p.rgb[2] * c[2]];
        }
        p
    }

    /// `PARTICLE_TILECRACK` as spawned by `Level::addParticle` (running dust, etc.).
    pub fn tile_crack(&mut self, lv: &Level, bx: i32, by: i32, bz: i32, id: u8, data: u8, x: f64, y: f64, z: f64, xa: f64, ya: f64, za: f64) {
        let p = self.terrain_particle(lv, bx, by, bz, id, data, x, y, z, xa, ya, za);
        self.add(p);
    }

    /// `ParticleEngine::destroy`: 4×4×4 pieces flying out of a broken block.
    pub fn destroy(&mut self, lv: &Level, x: i32, y: i32, z: i32, id: u8, data: u8) {
        if id == 0 {
            return;
        }
        const SD: i32 = 4;
        for xx in 0..SD {
            for yy in 0..SD {
                for zz in 0..SD {
                    let xp = x as f64 + (xx as f64 + 0.5) / SD as f64;
                    let yp = y as f64 + (yy as f64 + 0.5) / SD as f64;
                    let zp = z as f64 + (zz as f64 + 0.5) / SD as f64;
                    let _face = self.random.next_int_bounded(6);
                    let p = self.terrain_particle(lv, x, y, z, id, data, xp, yp, zp, xp - x as f64 - 0.5f32 as f64, yp - y as f64 - 0.5f32 as f64, zp - z as f64 - 0.5f32 as f64);
                    self.add(p);
                }
            }
        }
    }

    /// `ParticleEngine::crack`: one small piece on the face being hit.
    pub fn crack(&mut self, lv: &Level, x: i32, y: i32, z: i32, face: usize) {
        let id = lv.get_tile(x, y, z);
        if id == 0 {
            return;
        }
        let data = lv.get_data(x, y, z);
        let (mn, mx) = selection_box(id, data);
        let r = 0.10f32 as f64;
        let rd = |s: &mut Random, lo: f32, hi: f32| s.next_double() * (((hi - lo) as f64) - r * 2.0) + r + lo as f64;
        let mut xp = x as f64 + rd(&mut self.random, mn[0], mx[0]);
        let mut yp = y as f64 + rd(&mut self.random, mn[1], mx[1]);
        let mut zp = z as f64 + rd(&mut self.random, mn[2], mx[2]);
        match face {
            0 => yp = y as f64 + mn[1] as f64 - r,
            1 => yp = y as f64 + mx[1] as f64 + r,
            2 => zp = z as f64 + mn[2] as f64 - r,
            3 => zp = z as f64 + mx[2] as f64 + r,
            4 => xp = x as f64 + mn[0] as f64 - r,
            5 => xp = x as f64 + mx[0] as f64 + r,
            _ => {}
        }
        let mut p = self.terrain_particle(lv, x, y, z, id, data, xp, yp, zp, 0.0, 0.0, 0.0);
        p.set_power(0.2);
        p.scale(0.6);
        self.add(p);
    }

    /// All particles as camera-facing quads (`ParticleEngine::render` + `TerrainParticle::render`).
    /// `eye` is the interpolated camera position; `yaw`/`pitch` the player rotation in degrees (game convention).
    pub fn build_mesh(&self, lv: &Level, eye: [f64; 3], yaw: f32, pitch: f32, partial: f32) -> MeshData {
        let mut m = MeshData::default();
        let rad = std::f32::consts::PI / 180.0;
        let xa = (yaw * rad).cos();
        let za = (yaw * rad).sin();
        let xa2 = -za * (pitch * rad).sin();
        let za2 = xa * (pitch * rad).sin();
        let ya = (pitch * rad).cos();
        let a = partial as f64;
        for p in self.particles.iter().filter(|p| p.item.is_none()) {
            let x = (p.xo + (p.x - p.xo) * a) as f32;
            let y = (p.yo + (p.y - p.yo) * a) as f32;
            let z = (p.zo + (p.z - p.zo) * a) as f32;
            // 4J: terrain particles closer than a metre are not drawn
            let (rx, ry, rz) = (x - eye[0] as f32, y - eye[1] as f32, z - eye[2] as f32);
            if rx * rx + ry * ry + rz * rz < 1.0 {
                continue;
            }
            let (row, col) = (p.cell.0 as f32, p.cell.1 as f32);
            let (cu, cv, cw, ch) = (col / 16.0, row / 32.0, 1.0 / 16.0, 1.0 / 32.0);
            let u0 = cu + cw * (p.uo / 4.0);
            let u1 = cu + cw * ((p.uo + p.span) / 4.0);
            let v0 = cv + ch * (p.vo / 4.0);
            let v1 = cv + ch * ((p.vo + p.span) / 4.0);
            let mut size = p.size;
            if p.portal.is_some() {
                let mut s = (p.age as f32 + partial) / p.lifetime as f32;
                s = 1.0 - s;
                s *= s;
                s = 1.0 - s;
                size = p.osize * s;
            }
            let r = 0.1 * size;
            let (sx, sy, sz) = (p.x.floor() as i32, 0, p.z.floor() as i32);
            let _ = sy;
            let hh = (p.bb.y1 - p.bb.y0) * 0.66;
            let ty = (p.y - p.height_offset + hh).floor() as i32;
            let (sky, blk) = lv.get_light_color(sx, ty, sz, 0, -1);
            let col4 = [p.rgb[0], p.rgb[1], p.rgb[2], p.alpha];
            let light = [sky as f32, blk as f32];
            let v = |px: f32, py: f32, pz: f32, u: f32, w: f32| Vertex { pos: [px, py, pz], uv: [u, w], color: col4, light };
            let b = m.vertices.len() as u32;
            m.vertices.push(v(x - xa * r - xa2 * r, y - ya * r, z - za * r - za2 * r, u0, v1));
            m.vertices.push(v(x - xa * r + xa2 * r, y + ya * r, z - za * r + za2 * r, u0, v0));
            m.vertices.push(v(x + xa * r + xa2 * r, y + ya * r, z + za * r + za2 * r, u1, v0));
            m.vertices.push(v(x + xa * r - xa2 * r, y - ya * r, z + za * r - za2 * r, u1, v1));
            m.indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        }
        m
    }
}

/// Whether breaking this block makes debris at all (cross-shaped plants do too, they just use their icon).
pub fn has_icon(id: u8, data: u8) -> bool {
    !matches!(style(id, data).kind, Kind::Air)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{generate, Dim};

    fn flat() -> Level {
        generate(Dim::Flat, 1, 3)
    }

    #[test]
    fn destroy_spawns_64_pieces_with_the_original_ranges() {
        let lv = flat();
        let mut e = ParticleEngine::new();
        e.destroy(&lv, 0, 3, 0, t::STONE, 0);
        assert_eq!(e.len(), 64);
        for p in &e.particles {
            assert!(p.lifetime >= 4 && p.lifetime <= 40, "lifetime {}", p.lifetime);
            assert!(p.size >= 0.5 - 1e-6 && p.size < 1.0, "size {}", p.size); // ((0..1)*0.5+0.5)*2 / 2
            assert!(p.uo >= 0.0 && p.uo < 3.0 && p.vo >= 0.0 && p.vo < 3.0);
            assert!((p.rgb[0] - 0.6).abs() < 1e-6, "stone is untinted: {:?}", p.rgb);
            assert!(p.x >= 0.0 && p.x <= 1.0 && p.y >= 3.0 && p.y <= 4.0);
            let sp = (p.xd * p.xd + (p.yd - 0.1).powi(2) + p.zd * p.zd).sqrt();
            assert!(sp <= 0.45 * 0.4 + 1e-6, "initial speed {sp}"); // |v| = speed * 0.4, speed <= 0.45
        }
    }

    #[test]
    fn debris_falls_lands_and_expires() {
        let lv = flat();
        let mut e = ParticleEngine::new();
        // break a block 6 above the flat world's surface so the pieces have room to fall
        let top = (0..200).rev().find(|&y| lv.get_tile(0, y, 0) != 0).unwrap();
        e.destroy(&lv, 0, top + 6, 0, t::STONE, 0);
        let start_y: f64 = e.particles.iter().map(|p| p.y).sum::<f64>() / 64.0;
        for _ in 0..10 {
            e.tick(&lv);
        }
        let mid_y: f64 = e.particles.iter().map(|p| p.y).sum::<f64>() / e.len() as f64;
        assert!(mid_y < start_y, "gravity pulls the pieces down ({start_y} -> {mid_y})");
        for _ in 0..80 {
            e.tick(&lv);
        }
        assert!(e.is_empty(), "every piece lives at most 40 ticks, {} left", e.len());
    }

    #[test]
    fn particles_do_not_sink_into_the_ground() {
        let lv = flat();
        let mut e = ParticleEngine::new();
        let top = (0..200).rev().find(|&y| lv.get_tile(0, y, 0) != 0).unwrap();
        e.destroy(&lv, 0, top + 1, 0, t::DIRT, 0);
        for _ in 0..30 {
            e.tick(&lv);
            for p in &e.particles {
                assert!(p.bb.y0 >= (top + 1) as f64 - 1e-6, "box bottom {} below the surface {}", p.bb.y0, top + 1);
            }
        }
    }

    #[test]
    fn layer_limit_drops_the_oldest() {
        let lv = flat();
        let mut e = ParticleEngine::new();
        for i in 0..5 {
            e.destroy(&lv, i, 10, 0, t::STONE, 0);
        }
        assert_eq!(e.len(), MAX_PARTICLES);
    }

    #[test]
    fn mesh_has_one_quad_per_distant_particle_and_skips_near_ones() {
        let lv = flat();
        let mut e = ParticleEngine::new();
        e.destroy(&lv, 0, 10, 0, t::STONE, 0);
        let far = e.build_mesh(&lv, [0.5, 30.0, 0.5], 0.0, 0.0, 1.0);
        assert_eq!(far.indices.len(), 64 * 6);
        let near = e.build_mesh(&lv, [0.5, 10.5, 0.5], 0.0, 0.0, 1.0);
        assert!(near.indices.is_empty(), "everything is within 1 m of the camera");
        // a quad faces the camera: it is 0.1 * size wide
        for q in far.vertices.chunks(4) {
            let w = ((q[2].pos[0] - q[1].pos[0]).powi(2) + (q[2].pos[2] - q[1].pos[2]).powi(2)).sqrt();
            assert!(w > 0.0 && w <= 0.21, "{w}");
        }
    }

    #[test]
    fn crack_makes_one_small_piece_on_the_hit_face() {
        let lv = flat();
        let top = (0..200).rev().find(|&y| lv.get_tile(0, y, 0) != 0).unwrap();
        let mut e = ParticleEngine::new();
        e.crack(&lv, 0, top, 0, 1);
        assert_eq!(e.len(), 1);
        let p = &e.particles[0];
        assert!((p.y - (top as f64 + 1.0 + 0.1)).abs() < 1e-6, "sits 0.1 above the top face: {}", p.y);
        assert!(p.size >= 0.3 - 1e-6 && p.size < 0.6, "size {} should be in [0.3, 0.6)", p.size);
    }
}
