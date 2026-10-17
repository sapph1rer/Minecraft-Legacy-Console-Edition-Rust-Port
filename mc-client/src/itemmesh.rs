//! Held / dropped item geometry: `ItemInHandRenderer::renderItem3D` (a sprite extruded pixel by pixel, 1/16 deep) and
//! `TileRenderer::renderTile` (a block item drawn as a small cube).

use crate::blocks::{style, Kind};
use crate::mesh::{tint_colour, uv_of};
use crate::model::Emitter;
use glam::{Mat4, Vec3};

/// Alpha channel of a sprite sheet (decides which pixels get extruded).
#[derive(Clone, Default)]
pub struct Alpha {
    pub w: u32,
    pub h: u32,
    pub a: Vec<u8>,
}

impl Alpha {
    pub fn from_rgba(px: &[u8], w: u32, h: u32) -> Alpha {
        Alpha { w, h, a: px.chunks_exact(4).map(|p| p[3]).collect() }
    }
    fn at(&self, x: u32, y: u32) -> u8 {
        if x < self.w && y < self.h {
            self.a.get((y * self.w + x) as usize).copied().unwrap_or(0)
        } else {
            0
        }
    }
}

/// `ItemInHandRenderer::renderItem3D` for the 16×16 icon whose top-left texel is (`cx`, `cy`). The sprite occupies
/// x, y ∈ [0, 1] and z ∈ [-1/16, 0]; pixel (xp, yp) shows texel (15 - xp, 15 - yp).
pub fn sprite_3d(em: &mut Emitter, alpha: &Alpha, cx: u32, cy: u32, m: &Mat4) {
    let dd = 1.0 / 16.0;
    let (w, h) = (alpha.w.max(1) as f32, alpha.h.max(1) as f32);
    let texel = |xp: i32, yp: i32| -> u8 {
        if !(0..16).contains(&xp) || !(0..16).contains(&yp) {
            return 0;
        }
        alpha.at(cx + (15 - xp) as u32, cy + (15 - yp) as u32)
    };
    let present = |xp: i32, yp: i32| texel(xp, yp) > 8;
    let solid = |xp: i32, yp: i32| texel(xp, yp) >= 250;
    let t = [1.0f32; 3];
    for yp in 0..16i32 {
        for xp in 0..16i32 {
            if !present(xp, yp) {
                continue;
            }
            let u = (cx as f32 + (15 - xp) as f32 + 0.5) / w;
            let v = (cy as f32 + (15 - yp) as f32 + 0.5) / h;
            let uv = [[u, v]; 4];
            let (x0, x1) = (xp as f32 / 16.0, (xp + 1) as f32 / 16.0);
            let (y0, y1) = (yp as f32 / 16.0, (yp + 1) as f32 / 16.0);
            let (z0, z1) = (0.0, -dd);
            em.quad([[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0]], uv, Vec3::Z, m, t);
            em.quad([[x0, y1, z1], [x1, y1, z1], [x1, y0, z1], [x0, y0, z1]], uv, -Vec3::Z, m, t);
            if !solid(xp - 1, yp) {
                em.quad([[x0, y0, z1], [x0, y0, z0], [x0, y1, z0], [x0, y1, z1]], uv, -Vec3::X, m, t);
            }
            if !solid(xp + 1, yp) {
                em.quad([[x1, y1, z1], [x1, y1, z0], [x1, y0, z0], [x1, y0, z1]], uv, Vec3::X, m, t);
            }
            // (the original emits these two with swapped normals; kept as is)
            if !solid(xp, yp - 1) {
                em.quad([[x1, y0, z0], [x0, y0, z0], [x0, y0, z1], [x1, y0, z1]], uv, Vec3::Y, m, t);
            }
            if !solid(xp, yp + 1) {
                em.quad([[x1, y1, z1], [x0, y1, z1], [x0, y1, z0], [x1, y1, z0]], uv, -Vec3::Y, m, t);
            }
        }
    }
}

/// Does a block item draw as a 3D block (`TileRenderer::canRender`) rather than as its flat sprite?
pub fn renders_as_block(id: u16, data: u8) -> bool {
    id < 256 && id != 0 && !matches!(style(id as u8, data).kind, Kind::Air | Kind::Liquid)
}

/// Metadata an inventory/hand icon is drawn with (`renderBlockAsItem`): pistons show their platform on top, the
/// furnace-like blocks face the viewer.
pub fn icon_data(id: u8, data: u8) -> u8 {
    use mc_world::tile_ids as t;
    match id {
        t::PISTON_BASE | t::PISTON_STICKY_BASE => 1,
        t::DISPENSER | t::DROPPER | t::FURNACE | t::FURNACE_LIT if data & 7 < 2 => 3,
        // the lock faces the viewer's left, as in the original inventory icon
        t::CHEST => 3,
        _ => data,
    }
}

/// `TileRenderer::renderTile`: the block as a unit cube centred on the origin (after a 90° turn about Y).
pub fn block_3d(em: &mut Emitter, id: u8, data: u8, m: &Mat4) {
    block_3d_parts(em, id, data, m, None);
}

/// `block_3d` with explicit boxes for a multi-box block (a piston head with a short rod, say).
pub fn block_3d_parts(em: &mut Emitter, id: u8, data: u8, m: &Mat4, over: Option<Vec<crate::shapes::Part>>) {
    let data = if over.is_some() { data } else { icon_data(id, data) };
    let flat_lever;
    let s = if id == mc_world::tile_ids::LEVER {
        flat_lever = crate::blocks::Style::flat("lever");
        &flat_lever
    } else {
        style(id, data)
    };
    let m = *m * Mat4::from_rotation_y(90f32.to_radians()) * Mat4::from_translation(Vec3::splat(-0.5));
    let c = |cell: (u8, u8), tint: crate::blocks::Tint| -> ([[f32; 2]; 4], [f32; 3]) {
        ([uv_of(cell, 0), uv_of(cell, 1), uv_of(cell, 2), uv_of(cell, 3)], tint_colour(tint, 0.8))
    };
    match s.kind {
        Kind::Cube | Kind::Box { .. } | Kind::Shaped => {
          let list: Vec<([f32; 3], [f32; 3], [(u8, u8); 6], [u8; 6])> = match s.kind {
              Kind::Box { min, max } => vec![(min, max, s.cells, s.rot)],
              Kind::Shaped => over.unwrap_or_else(|| crate::shapes::parts(id, data)).into_iter().map(|p| (p.min, p.max, p.cells.unwrap_or(s.cells), p.rot)).collect(),
              _ => vec![([0.0; 3], [1.0; 3], s.cells, s.rot)],
          };
          for (mn, mx, cells, rots) in list {
            let partial = mn != [0.0; 3] || mx != [1.0; 3];
            let grass = id == mc_world::tile_ids::GRASS;
            let overlay = crate::icons::icon("grass_side_overlay").unwrap_or((2, 6));
            // (positions BL, BR, TR, TL as seen from outside; normal; cell index)
            let p = |x: f32, y: f32, z: f32| -> [f32; 3] { [mn[0] + (mx[0] - mn[0]) * x, mn[1] + (mx[1] - mn[1]) * y, mn[2] + (mx[2] - mn[2]) * z] };
            let faces: [([[f32; 3]; 4], Vec3, usize); 6] = [
                ([p(0., 0., 0.), p(0., 0., 1.), p(0., 1., 1.), p(0., 1., 0.)], -Vec3::X, 0),
                ([p(1., 0., 1.), p(1., 0., 0.), p(1., 1., 0.), p(1., 1., 1.)], Vec3::X, 1),
                ([p(0., 0., 0.), p(1., 0., 0.), p(1., 0., 1.), p(0., 0., 1.)], -Vec3::Y, 2),
                ([p(0., 1., 1.), p(1., 1., 1.), p(1., 1., 0.), p(0., 1., 0.)], Vec3::Y, 3),
                ([p(1., 0., 0.), p(0., 0., 0.), p(0., 1., 0.), p(1., 1., 0.)], -Vec3::Z, 4),
                ([p(0., 0., 1.), p(1., 0., 1.), p(1., 1., 1.), p(0., 1., 1.)], Vec3::Z, 5),
            ];
            for (pos, n, f) in faces {
                // grass: only the top (and the side overlay) is tinted in the inventory
                let tint = if grass && f != 3 { crate::blocks::Tint::None } else { s.tint[f] };
                let (mut uv, col) = c(cells[f], tint);
                if partial || rots[f] & 3 != 0 {
                    for k in 0..4 {
                        let (mut u, mut v) = (crate::mesh::axis_frac(f, k, 0, pos[k]), crate::mesh::axis_frac(f, k, 1, pos[k]));
                        for _ in 0..rots[f] & 3 {
                            let nu = v;
                            v = 1.0 - u;
                            u = nu;
                        }
                        uv[k] = crate::mesh::uv_frac(cells[f], u.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
                    }
                }
                em.quad(pos, uv, n, &m, col);
                if grass && f != 2 && f != 3 {
                    let (uv, col) = c(overlay, crate::blocks::Tint::Grass);
                    em.quad(pos, uv, n, &m, col);
                }
            }
          }
        }
        Kind::Cross | Kind::Flat => {
            let (uv, col) = c(s.cells[5], s.tint[5]);
            em.quad([[0., 0., 0.], [1., 0., 1.], [1., 1., 1.], [0., 1., 0.]], uv, Vec3::Y, &m, col);
            em.quad([[0., 0., 1.], [1., 0., 0.], [1., 1., 0.], [0., 1., 1.]], uv, Vec3::Y, &m, col);
        }
        Kind::Air | Kind::Liquid => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_sprite_has_only_outer_edges() {
        let alpha = Alpha { w: 256, h: 256, a: vec![255; 256 * 256] };
        let mut em = Emitter::new();
        sprite_3d(&mut em, &alpha, 0, 0, &Mat4::IDENTITY);
        // 256 front + 256 back + 4 × 16 edge quads
        assert_eq!(em.mesh.vertices.len() / 4, 256 * 2 + 64);
    }

    #[test]
    fn transparent_sprite_is_empty() {
        let alpha = Alpha { w: 256, h: 256, a: vec![0; 256 * 256] };
        let mut em = Emitter::new();
        sprite_3d(&mut em, &alpha, 16, 16, &Mat4::IDENTITY);
        assert!(em.mesh.vertices.is_empty());
    }

    #[test]
    fn stone_block_is_a_unit_cube() {
        let mut em = Emitter::new();
        block_3d(&mut em, mc_world::tile_ids::STONE, 0, &Mat4::IDENTITY);
        assert_eq!(em.mesh.vertices.len(), 24);
        for v in &em.mesh.vertices {
            for c in v.pos {
                assert!((c.abs() - 0.5).abs() < 1e-5, "{c}");
            }
        }
    }
}
