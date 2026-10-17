//! World-space entity geometry: dropped items (`ItemRenderer`).

use crate::entity::ItemEntity;
use crate::itemmesh::{block_3d, renders_as_block, sprite_3d, Alpha};
use crate::mesh::MeshData;
use crate::model::Emitter;
use crate::render::EntTex;
use glam::{Mat4, Vec3};
use mc_core::random::Random;
use mc_level::level::Level;
use mc_world::item::item_def;

/// `ItemRenderer::render` for one dropped item. Appends to the `Terrain` / `Items` meshes.
pub fn render_item_entity(e: &ItemEntity, lv: &Level, partial: f32, items: &Alpha, terrain: &mut Emitter, sprites: &mut Emitter) {
    let b = &e.body;
    let a = partial as f64;
    let (x, y, z) = (b.xo + (b.x - b.xo) * a, b.yo + (b.y - b.yo) * a, b.zo + (b.z - b.zo) * a);
    let age = e.age as f32 + partial;
    let bob = (age / 10.0 + e.bob_offs).sin() * 0.1 + 0.1;
    let spin_deg = (age / 20.0 + e.bob_offs) * (180.0 / std::f32::consts::PI);
    let st = &e.item;
    let light = {
        let (s, bl) = lv.tile_light_color(x.floor() as i32, (y + 0.1).floor() as i32, z.floor() as i32);
        [s as f32, bl as f32]
    };
    let mut rng = Random::with_seed(187);
    let base = Mat4::from_translation(Vec3::new(x as f32, y as f32 + bob, z as f32));
    let rot_y = Mat4::from_rotation_y(spin_deg.to_radians());
    if renders_as_block(st.id, st.damage as u8) {
        let count = match st.count {
            0..=1 => 1,
            2..=5 => 2,
            6..=20 => 3,
            21..=40 => 4,
            _ => 5,
        };
        let s = match crate::blocks::style(st.id as u8, st.damage as u8).kind {
            crate::blocks::Kind::Cross | crate::blocks::Kind::Flat => 0.5,
            _ => 0.25,
        };
        terrain.light = light;
        for i in 0..count {
            let mut m = base * rot_y * Mat4::from_scale(Vec3::splat(s));
            if i > 0 {
                let xo = (rng.next_float() * 2.0 - 1.0) * 0.2 / s;
                let yo = (rng.next_float() * 2.0 - 1.0) * 0.2 / s;
                let zo = (rng.next_float() * 2.0 - 1.0) * 0.2 / s;
                m *= Mat4::from_translation(Vec3::new(xo, yo, zo));
            }
            block_3d(terrain, st.id as u8, st.damage as u8, &m);
        }
    } else if let Some(def) = item_def(st.id) {
        let width = 1.0 / 16.0;
        let margin = 0.35 / 16.0;
        let count = match st.count {
            0..=1 => 1,
            2..=15 => 2,
            16..=31 => 3,
            _ => 4,
        };
        let mut m = base * Mat4::from_scale(Vec3::splat(0.5)) * rot_y;
        m *= Mat4::from_translation(Vec3::new(-0.5, -0.25, -((width + margin) * count as f32 / 2.0)));
        sprites.light = light;
        let (row, col) = def.icon_pos;
        for _ in 0..count {
            m *= Mat4::from_translation(Vec3::new(0.0, 0.0, width + margin));
            sprite_3d(sprites, items, col as u32 * 16, row as u32 * 16, &m);
        }
    }
}

/// All dropped items of the world as entity layers.
pub fn item_layers(items_list: &[ItemEntity], lv: &Level, partial: f32, items: &Alpha) -> Vec<(EntTex, MeshData)> {
    let mut terrain = Emitter::new();
    let mut sprites = Emitter::new();
    for e in items_list {
        render_item_entity(e, lv, partial, items, &mut terrain, &mut sprites);
    }
    let mut out = Vec::new();
    if !terrain.mesh.is_empty() {
        out.push((EntTex::Terrain, terrain.mesh));
    }
    if !sprites.mesh.is_empty() {
        out.push((EntTex::Items, sprites.mesh));
    }
    out
}

/// Falling blocks as full-size cubes in the terrain layer.
pub fn falling_layers(list: &[crate::entity::FallingBlock], lv: &Level, partial: f32) -> Vec<(EntTex, MeshData)> {
    if list.is_empty() {
        return Vec::new();
    }
    let mut terrain = Emitter::new();
    for e in list {
        let b = &e.body;
        let a = partial as f64;
        let (x, y, z) = (b.xo + (b.x - b.xo) * a, b.yo + (b.y - b.yo) * a, b.zo + (b.z - b.zo) * a);
        let (s, bl) = lv.tile_light_color(x.floor() as i32, y.floor() as i32, z.floor() as i32);
        terrain.light = [s as f32, bl as f32];
        // `block_3d` turns the unit cube by 90° like the item renderer; undo it so the block keeps its orientation
        let m = Mat4::from_translation(Vec3::new(x as f32, y as f32, z as f32)) * Mat4::from_rotation_y(-90f32.to_radians());
        block_3d(&mut terrain, e.id, e.data, &m);
    }
    if terrain.mesh.is_empty() {
        Vec::new()
    } else {
        vec![(EntTex::Terrain, terrain.mesh)]
    }
}

/// Blocks moved by pistons (`PistonPieceRenderer`): the carried block at its offset, and for a retracting base
/// the head sliding back into it.
pub fn piston_layers(list: &[crate::piston::MovingPiece], lv: &Level, partial: f32) -> Vec<(EntTex, MeshData)> {
    if list.is_empty() {
        return Vec::new();
    }
    use mc_world::tile_ids as t;
    let mut terrain = Emitter::new();
    let undo = Mat4::from_rotation_y(-90f32.to_radians());
    for m in list {
        let (s, bl) = lv.tile_light_color(m.pos.0, m.pos.1, m.pos.2);
        terrain.light = [s as f32, bl as f32];
        let c = Vec3::new(m.pos.0 as f32 + 0.5, m.pos.1 as f32 + 0.5, m.pos.2 as f32 + 0.5);
        if m.head {
            // the base stays put; the head slides from the front of it back in
            let p = m.get_progress(partial);
            let st = crate::piston::STEP[m.facing as usize % 6];
            let k = 1.0 - p;
            let off = Vec3::new(st[0] as f32, st[1] as f32, st[2] as f32) * k;
            block_3d(&mut terrain, m.id, m.data & 7, &(Mat4::from_translation(c) * undo));
            let sticky = if m.id == t::PISTON_STICKY_BASE { 8 } else { 0 };
            let parts = crate::shapes::piston_head_parts((m.data & 7) | sticky, true);
            crate::itemmesh::block_3d_parts(&mut terrain, t::PISTON_EXTENSION_PIECE, (m.data & 7) | sticky, &(Mat4::from_translation(c + off) * undo), Some(parts));
            continue;
        }
        let o = m.offset(partial);
        let mm = Mat4::from_translation(c + Vec3::new(o[0], o[1], o[2])) * undo;
        if m.id == t::PISTON_EXTENSION_PIECE {
            let parts = crate::shapes::piston_head_parts(m.data, m.get_progress(partial) >= 0.5 || !m.extending);
            crate::itemmesh::block_3d_parts(&mut terrain, m.id, m.data, &mm, Some(parts));
        } else {
            block_3d(&mut terrain, m.id, m.data, &mm);
        }
    }
    if terrain.mesh.is_empty() {
        Vec::new()
    } else {
        vec![(EntTex::Terrain, terrain.mesh)]
    }
}

/// Lit TNT (`TntRenderer`): swells and flashes as the fuse runs down.
pub fn primed_layers(list: &[crate::tnt::PrimedTnt], lv: &Level, partial: f32) -> Vec<(EntTex, MeshData)> {
    if list.is_empty() {
        return Vec::new();
    }
    let mut terrain = Emitter::new();
    for e in list {
        let b = &e.body;
        let a = partial as f64;
        let (x, y, z) = (b.xo + (b.x - b.xo) * a, b.yo + (b.y - b.yo) * a, b.zo + (b.z - b.zo) * a);
        let (s, bl) = lv.tile_light_color(x.floor() as i32, y.floor() as i32, z.floor() as i32);
        let flash = (e.life as f32 / 5.0) as i32 % 2 == 0;
        terrain.light = if flash { [15.0, 15.0] } else { [s as f32, bl as f32] };
        let mut g = 1.0 - (e.life as f32 - partial + 1.0) / 100.0;
        g = g.clamp(0.0, 1.0);
        g = g * g * g * g;
        let sc = 1.0 + g * 0.3;
        let m = Mat4::from_translation(Vec3::new(x as f32, y as f32, z as f32)) * Mat4::from_scale(Vec3::splat(sc)) * Mat4::from_rotation_y(-90f32.to_radians());
        block_3d(&mut terrain, mc_world::tile_ids::TNT, 0, &m);
    }
    if terrain.mesh.is_empty() {
        Vec::new()
    } else {
        vec![(EntTex::Terrain, terrain.mesh)]
    }
}

/// Chest lids that are swinging (the chunk mesh leaves out the lid of an animating chest).
pub fn chest_layers(list: &std::collections::HashMap<(i32, i32, i32), crate::chest::ChestAnim>, lv: &Level, partial: f32) -> Vec<(EntTex, MeshData)> {
    use mc_world::tile_ids as t;
    if list.is_empty() {
        return Vec::new();
    }
    let mut terrain = Emitter::new();
    let undo = Mat4::from_rotation_y(-90f32.to_radians());
    for (p, a) in list {
        if lv.get_tile(p.0, p.1, p.2) != t::CHEST {
            continue;
        }
        let data = lv.get_data(p.0, p.1, p.2);
        let (s, bl) = lv.tile_light_color(p.0, p.1, p.2);
        terrain.light = [s as f32, bl as f32];
        let o = a.prev + (a.open - a.prev) * partial;
        let ang = crate::chest::lid_angle(o);
        let hc = Vec3::new(0.0, 9.0 / 16.0 - 0.5, 7.0 / 16.0);
        let m = Mat4::from_translation(Vec3::new(p.0 as f32 + 0.5, p.1 as f32 + 0.5, p.2 as f32 + 0.5))
            * Mat4::from_rotation_y((crate::chest::facing_turn(data) as f32).to_radians())
            * Mat4::from_translation(hc)
            * Mat4::from_rotation_x(ang)
            * Mat4::from_translation(-hc)
            * undo;
        let nb = |x: i32, y: i32, z: i32| (lv.get_tile(x, y, z), lv.get_data(x, y, z));
        // the lid is built in the chest's own frame and turned with it
        let pr = crate::chest::partner(t::CHEST, p.0, p.1, p.2, &nb).map(|w| crate::chest::to_local(crate::chest::facing_turn(data), w));
        let full = crate::chest::chest_parts(2, true, crate::chest::ChestCells::lookup(), pr);
        // parts 1.. are the lid and the lock
        let lid: Vec<_> = full.into_iter().skip(1).collect();
        crate::itemmesh::block_3d_parts(&mut terrain, t::CHEST, data, &m, Some(lid));
    }
    if terrain.mesh.is_empty() {
        Vec::new()
    } else {
        vec![(EntTex::Terrain, terrain.mesh)]
    }
}

/// Signs (`SignRenderer`): the board (and post) model from `item/sign.png`, plus four lines of font glyphs on it,
/// within 16 blocks of the camera. Chunk meshes leave out signs that have a tile entity.
pub fn sign_layers(lv: &Level, widths: &[u8; 256], font_size: (f32, f32), cam: Vec3) -> Vec<(EntTex, MeshData)> {
    use crate::model::ModelPart;
    use mc_level::tile_entity::TileEntity;
    use mc_world::tile_ids as t;
    let mut text = Emitter::new();
    let mut model = Emitter::new();
    let mut board = ModelPart::new(64, 32, 0, 0);
    board.add_box(-12.0, -14.0, -1.0, 24, 12, 2, 0.0);
    let mut stick = ModelPart::new(64, 32, 0, 14);
    stick.add_box(-1.0, -2.0, -1.0, 2, 14, 2, 0.0);
    for (p, te) in lv.tile_entities.iter() {
        let TileEntity::Sign { lines } = te else { continue };
        let id = lv.get_tile(p.0, p.1, p.2);
        if id != t::SIGN && id != t::WALL_SIGN {
            continue;
        }
        let data = lv.get_data(p.0, p.1, p.2);
        let c = Vec3::new(p.0 as f32 + 0.5, p.1 as f32 + 0.5, p.2 as f32 + 0.5);
        if (c - cam).length_squared() > 48.0 * 48.0 {
            continue;
        }
        let (s, bl) = lv.tile_light_color(p.0, p.1, p.2);
        text.light = [s as f32, bl as f32];
        model.light = [s as f32, bl as f32];
        let size = 0.6666667f32;
        let rot = if id == t::SIGN {
            (data as f32) * 360.0 / 16.0
        } else {
            match data & 7 {
                2 => 180.0,
                4 => 90.0,
                5 => -90.0,
                _ => 0.0,
            }
        };
        let mut m = Mat4::from_translation(Vec3::new(p.0 as f32 + 0.5, p.1 as f32 + 0.75 * size, p.2 as f32 + 0.5)) * Mat4::from_rotation_y((-rot).to_radians());
        if id == t::WALL_SIGN {
            m *= Mat4::from_translation(Vec3::new(0.0, -0.3125, -0.4375));
        }
        let mm = m * Mat4::from_scale(Vec3::new(size, -size, -size));
        board.render(1.0 / 16.0, &mm, &mut model, false);
        if id == t::SIGN {
            stick.render(1.0 / 16.0, &mm, &mut model, false);
        }
        if (c - cam).length_squared() > 16.0 * 16.0 || lines.iter().all(|l| l.is_empty()) {
            continue;
        }
        let sc = size / 60.0;
        let tm = m * Mat4::from_translation(Vec3::new(0.0, 0.5 * size, 0.07 * size)) * Mat4::from_scale(Vec3::new(sc, -sc, sc));
        for (i, line) in lines.iter().enumerate() {
            let w: f32 = line.chars().map(|ch| widths[(ch as u32).min(255) as usize] as f32).sum();
            let mut x = -w / 2.0;
            let y = i as f32 * 10.0 - 20.0;
            for ch in line.chars() {
                let ci = (ch as u32).min(255) as usize;
                let wd = widths[ci] as f32;
                if ch != ' ' {
                    let (u, v) = ((ci % 16) as f32 * 8.0, (ci / 16) as f32 * 8.0);
                    let gw = wd - 0.01;
                    let (tw, th) = font_size;
                    let (u0, v0, u1, v1) = (u / tw, v / th, (u + gw) / tw, (v + 7.99) / th);
                    text.quad([[x, y + 7.99, 0.0], [x + gw, y + 7.99, 0.0], [x + gw, y, 0.0], [x, y, 0.0]], [[u0, v1], [u1, v1], [u1, v0], [u0, v0]], Vec3::Z, &tm, [0.0, 0.0, 0.0]);
                }
                x += wd;
            }
        }
    }
    let mut out = Vec::new();
    if !model.mesh.is_empty() {
        out.push((EntTex::Sign, model.mesh));
    }
    if !text.mesh.is_empty() {
        out.push((EntTex::Font, text.mesh));
    }
    out
}

/// Experience orbs (`ExperienceOrbRenderer`): a camera-facing glow that pulses between green and yellow.
pub fn orb_layers(list: &[crate::survival::XpOrb], lv: &Level, cam: Vec3, partial: f32) -> Vec<(EntTex, MeshData)> {
    if list.is_empty() {
        return Vec::new();
    }
    let (row, col) = crate::icons::icon("xp_orb").unwrap_or((31, 9));
    let (u0, v0) = (col as f32 / 16.0, row as f32 / 32.0);
    let (u1, v1) = (u0 + 1.0 / 16.0, v0 + 1.0 / 32.0);
    let mut e = Emitter::new();
    e.lit = false;
    for o in list {
        let b = &o.body;
        let a = partial as f64;
        let c = Vec3::new((b.xo + (b.x - b.xo) * a) as f32, (b.yo + (b.y - b.yo) * a) as f32 + 0.1, (b.zo + (b.z - b.zo) * a) as f32);
        let (s, bl) = lv.tile_light_color(c.x.floor() as i32, c.y.floor() as i32, c.z.floor() as i32);
        e.light = [s.max(bl) as f32, bl as f32];
        let t = (o.tick_count as f32 + partial) / 4.0;
        let rr = (t.sin() + 1.0) * 0.5;
        let bb = ((t + 4.1887).sin() + 1.0) * 0.1;
        e.color = [1.0, 1.0, 1.0, 1.0];
        let dir = (c - cam).normalize_or_zero();
        let right = Vec3::Y.cross(dir).normalize_or_zero();
        let up = dir.cross(right);
        let icon = [3, 7, 17, 37, 73, 149, 307, 617, 1237, 2477].iter().filter(|&&v| o.value >= v).count() as f32;
        let h = 0.14 + 0.012 * icon;
        let (r, u) = (right * h, up * h);
        e.quad([(c - r - u).to_array(), (c + r - u).to_array(), (c + r + u).to_array(), (c - r + u).to_array()], [[u0, v1], [u1, v1], [u1, v0], [u0, v0]], dir, &Mat4::IDENTITY, [rr, 1.0, bb]);
    }
    vec![(EntTex::Terrain, e.mesh)]
}

/// Thrown ender pearls: the item sprite turned to face the camera (`ThrownEnderpearl` renders through `ItemSpriteRenderer`).
pub fn pearl_layers(list: &[crate::pearl::Pearl], lv: &Level, cam: Vec3, partial: f32, items: &Alpha) -> Vec<(EntTex, MeshData)> {
    let Some(def) = item_def(crate::pearl::PEARL_ITEM) else { return Vec::new() };
    if list.is_empty() {
        return Vec::new();
    }
    let mut e = Emitter::new();
    let (row, col) = def.icon_pos;
    for p in list {
        let b = &p.body;
        let a = partial as f64;
        let c = Vec3::new((b.xo + (b.x - b.xo) * a) as f32, (b.yo + (b.y - b.yo) * a) as f32, (b.zo + (b.z - b.zo) * a) as f32);
        let (s, bl) = lv.tile_light_color(c.x.floor() as i32, c.y.floor() as i32, c.z.floor() as i32);
        e.light = [s as f32, bl as f32];
        let d = cam - c;
        let yaw = d.x.atan2(d.z);
        let m = Mat4::from_translation(c) * Mat4::from_rotation_y(yaw) * Mat4::from_scale(Vec3::splat(0.5)) * Mat4::from_translation(Vec3::new(-0.5, -0.5, -1.0 / 32.0));
        sprite_3d(&mut e, items, col as u32 * 16, row as u32 * 16, &m);
    }
    vec![(EntTex::Items, e.mesh)]
}

/// Item debris (`BreakingItemParticle`, thrown up while eating): camera-facing quarters of the items.png icon.
pub fn item_particle_layers(eng: &crate::particles::ParticleEngine, cam: Vec3, yaw: f32, pitch: f32, partial: f32, lv: &Level, items: &Alpha) -> Vec<(EntTex, MeshData)> {
    let mut e = Emitter::new();
    e.lit = false;
    let rad = std::f32::consts::PI / 180.0;
    let (xa, za) = ((yaw * rad).cos(), (yaw * rad).sin());
    let (xa2, za2) = (-za * (pitch * rad).sin(), xa * (pitch * rad).sin());
    let ya = (pitch * rad).cos();
    let (w, h) = (items.w.max(1) as f32, items.h.max(1) as f32);
    let a = partial as f64;
    for p in eng.particles.iter().filter(|p| p.item.is_some()) {
        let (row, col) = p.item.unwrap();
        let c = Vec3::new((p.xo + (p.x - p.xo) * a) as f32, (p.yo + (p.y - p.yo) * a) as f32, (p.zo + (p.z - p.zo) * a) as f32);
        if (c - cam).length_squared() < 1.0 {
            continue;
        }
        let (s, bl) = lv.tile_light_color(c.x.floor() as i32, c.y.floor() as i32, c.z.floor() as i32);
        e.light = [s as f32, bl as f32];
        let (cx, cy) = (col as f32 * 16.0, row as f32 * 16.0);
        let (u0, u1) = ((cx + p.uo * 4.0) / w, (cx + p.uo * 4.0 + 4.0) / w);
        let (v0, v1) = ((cy + p.vo * 4.0) / h, (cy + p.vo * 4.0 + 4.0) / h);
        let r = 0.1 * p.size;
        let q = |sx: f32, sy: f32| c + Vec3::new(xa * r * sx + xa2 * r * sy, ya * r * sy, za * r * sx + za2 * r * sy);
        let n = (cam - c).normalize_or_zero();
        e.quad([q(-1.0, -1.0).to_array(), q(1.0, -1.0).to_array(), q(1.0, 1.0).to_array(), q(-1.0, 1.0).to_array()], [[u0, v1], [u1, v1], [u1, v0], [u0, v0]], n, &Mat4::IDENTITY, [1.0, 1.0, 1.0]);
    }
    if e.mesh.is_empty() {
        Vec::new()
    } else {
        vec![(EntTex::Items, e.mesh)]
    }
}
