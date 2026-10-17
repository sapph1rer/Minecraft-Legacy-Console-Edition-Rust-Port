//! Sky, sun, moon, stars, clouds and fog parameters — ports of `Dimension::getTimeOfDay`,
//! `Level::getSkyColor/getCloudColor/getFogColor/getSkyDarken/getStarBrightness`,
//! `Dimension::getSunriseColor` and the geometry of `LevelRenderer::renderSky/renderStars/createCloudMesh`.

use crate::mesh::Vertex;
use crate::world::Dim;
use mc_core::{mth, Random};
use mc_world::colours as col;

pub const TICKS_PER_DAY: i64 = 24000;
pub const MOON_BRIGHTNESS_PER_PHASE: [f32; 8] = [1.0, 0.75, 0.5, 0.25, 0.0, 0.25, 0.5, 0.75];
const PI: f32 = std::f32::consts::PI;

pub fn rgb(v: u32) -> [f32; 3] {
    [((v >> 16) & 0xff) as f32 / 255.0, ((v >> 8) & 0xff) as f32 / 255.0, (v & 0xff) as f32 / 255.0]
}

/// `Dimension::getTimeOfDay` (the End is frozen at 0).
pub fn time_of_day(dim: Dim, day_time: i64, a: f32) -> f32 {
    if dim == Dim::End {
        return 0.0;
    }
    let day_step = (day_time % TICKS_PER_DAY) as i32;
    let mut td = (day_step as f32 + a) / TICKS_PER_DAY as f32 - 0.25;
    if td < 0.0 {
        td += 1.0;
    }
    if td > 1.0 {
        td -= 1.0;
    }
    let tdo = td;
    let t2 = 1.0 - ((((td * PI) as f64).cos() + 1.0) / 2.0) as f32;
    tdo + (t2 - tdo) / 3.0
}

pub fn moon_phase(day_time: i64) -> usize {
    ((day_time / TICKS_PER_DAY) % 8) as usize
}

/// `Level::getSkyDarken`
pub fn sky_darken(td: f32, rain: f32, thunder: f32) -> f32 {
    let mut br = 1.0 - (mth::cos(td * PI * 2.0) * 2.0 + 0.2);
    br = br.clamp(0.0, 1.0);
    br = 1.0 - br;
    br *= 1.0 - rain * 5.0 / 16.0;
    br *= 1.0 - thunder * 5.0 / 16.0;
    br * 0.8 + 0.2
}

fn desaturate(c: [f32; 3], k: f32, level: f32, strength: f32) -> [f32; 3] {
    let mid = (c[0] * 0.30 + c[1] * 0.59 + c[2] * 0.11) * k;
    let ba = 1.0 - level * strength;
    [c[0] * ba + mid * (1.0 - ba), c[1] * ba + mid * (1.0 - ba), c[2] * ba + mid * (1.0 - ba)]
}

/// `Level::getSkyColor` for the biome sky colour `biome_sky` (0xRRGGBB).
pub fn sky_color(biome_sky: u32, td: f32, rain: f32, thunder: f32) -> [f32; 3] {
    let br = (mth::cos(td * PI * 2.0) * 2.0 + 0.5).clamp(0.0, 1.0);
    let c = rgb(biome_sky);
    let mut c = [c[0] * br, c[1] * br, c[2] * br];
    if rain > 0.0 {
        c = desaturate(c, 0.6, rain, 0.75);
    }
    if thunder > 0.0 {
        c = desaturate(c, 0.2, thunder, 0.75);
    }
    c
}

/// `Level::getCloudColor`
pub fn cloud_color(td: f32, rain: f32, thunder: f32) -> [f32; 3] {
    let br = (mth::cos(td * PI * 2.0) * 2.0 + 0.5).clamp(0.0, 1.0);
    let mut c = rgb(col::CLOUD_BASE);
    if rain > 0.0 {
        c = desaturate(c, 0.6, rain, 0.95);
    }
    c[0] *= br * 0.90 + 0.10;
    c[1] *= br * 0.90 + 0.10;
    c[2] *= br * 0.85 + 0.15;
    if thunder > 0.0 {
        c = desaturate(c, 0.2, thunder, 0.95);
    }
    c
}

/// `Dimension::getFogColor(td, a)`
pub fn dimension_fog_color(dim: Dim, td: f32) -> [f32; 3] {
    match dim {
        Dim::Nether => {
            let c = col::NETHER_FOG;
            [((c >> 16) & 0xff) as f32 / 256.0, ((c >> 8) & 0xff) as f32 / 256.0, (c & 0xff) as f32 / 256.0]
        }
        Dim::End => {
            let c = rgb(col::END_FOG);
            let _br = (mth::cos(td * PI * 2.0) * 2.0 + 0.5).clamp(0.0, 1.0);
            [c[0] * 0.15, c[1] * 0.15, c[2] * 0.15]
        }
        _ => {
            let br = (mth::cos(td * PI * 2.0) * 2.0 + 0.5).clamp(0.0, 1.0);
            let c = rgb(col::DEFAULT_FOG);
            [c[0] * (br * 0.94 + 0.06), c[1] * (br * 0.94 + 0.06), c[2] * (br * 0.91 + 0.09)]
        }
    }
}

/// `Level::getStarBrightness`
pub fn star_brightness(td: f32) -> f32 {
    let br = (1.0 - (mth::cos(td * PI * 2.0) * 2.0 + 0.25)).clamp(0.0, 1.0);
    br * br * 0.5
}

/// `Dimension::getSunriseColor` → rgba (alpha is the fan's centre opacity).
pub fn sunrise_color(dim: Dim, td: f32) -> Option<[f32; 4]> {
    if dim == Dim::End {
        return None;
    }
    let c1 = rgb(col::SKY_DAWN_DARK).map(|v| v as f64);
    let c2 = rgb(col::SKY_DAWN_BRIGHT).map(|v| v as f64);
    let span = 0.4f32;
    let tt = mth::cos(td * PI * 2.0);
    let mid = 0.0f32;
    if tt >= mid - span && tt <= mid + span {
        let aa = ((tt - mid) / span) * 0.5 + 0.5;
        let mix = 1.0 - ((1.0 - mth::sin(aa * PI)) * 0.99);
        let mix = mix * mix;
        let a = aa as f64;
        Some([(a * (c2[0] - c1[0]) + c1[0]) as f32, (a * (c2[1] - c1[1]) + c1[1]) as f32, (a * (c2[2] - c1[2]) + c1[2]) as f32, mix])
    } else {
        None
    }
}

/// `GameRenderer::updateFogColor` (without potion / water / lava / cloud overrides, which `fog_for_medium` adds).
pub fn world_fog_color(dim: Dim, td: f32, sky: [f32; 3], look: [f32; 3], view_distance_option: i32, rain: f32, thunder: f32) -> [f32; 3] {
    let mut f = dimension_fog_color(dim, td);
    if view_distance_option < 2 {
        let sun_dir = if mth::sin(td * PI * 2.0) > 0.0 { [-1.0, 0.0, 0.0] } else { [1.0, 0.0, 0.0] };
        let d = (look[0] * sun_dir[0] + look[1] * sun_dir[1] + look[2] * sun_dir[2]).max(0.0);
        if d > 0.0 {
            if let Some(c) = sunrise_color(dim, td) {
                let d = d * c[3];
                for i in 0..3 {
                    f[i] = f[i] * (1.0 - d) + c[i] * d;
                }
            }
        }
    }
    let whiteness = 1.0 - (1.0 / (4 - view_distance_option) as f32).powf(0.25);
    for i in 0..3 {
        f[i] += (sky[i] - f[i]) * whiteness;
    }
    if rain > 0.0 {
        let (ba, bb) = (1.0 - rain * 0.5, 1.0 - rain * 0.4);
        f = [f[0] * ba, f[1] * ba, f[2] * bb];
    }
    if thunder > 0.0 {
        let ba = 1.0 - thunder * 0.5;
        f = [f[0] * ba, f[1] * ba, f[2] * ba];
    }
    f
}

/// Brightness ramp of a dimension (`Dimension::updateLightRamp`).
pub fn brightness_ramp(dim: Dim) -> [f32; 16] {
    let ambient = if dim == Dim::Nether { 0.10f32 } else { 0.0 };
    let mut r = [0.0; 16];
    for (i, v) in r.iter_mut().enumerate() {
        let x = 1.0 - i as f32 / 15.0;
        *v = ((1.0 - x) / (x * 3.0 + 1.0)) * (1.0 - ambient) + ambient;
    }
    r
}

/// The 16×16 RGBA light texture of `GameRenderer::updateLightTexture` (row = sky level, column = block level).
pub fn light_texture(dim: Dim, sky_darken: f32, blr: f32) -> Vec<u8> {
    let ramp = brightness_ramp(dim);
    let mut px = vec![0u8; 16 * 16 * 4];
    for i in 0..256usize {
        let darken = sky_darken * 0.95 + 0.05;
        let sky = ramp[i / 16] * darken;
        let block = ramp[i % 16] * (blr * 0.1 + 1.5);
        let k = sky_darken * 0.65 + 0.35;
        let (rs, gs, bs) = (sky * k, sky * k, sky);
        let rb = block;
        let gb = block * ((block * 0.6 + 0.4) * 0.6 + 0.4);
        let bb = block * ((block * block) * 0.6 + 0.4);
        let mut r = (rs + rb) * 0.96 + 0.03;
        let mut g = (gs + gb) * 0.96 + 0.03;
        let mut b = (bs + bb) * 0.96 + 0.03;
        if dim == Dim::End {
            r = 0.22 + rb * 0.75;
            g = 0.28 + gb * 0.75;
            b = 0.25 + bb * 0.75;
        }
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u8;
        px[i * 4] = q(r);
        px[i * 4 + 1] = q(g);
        px[i * 4 + 2] = q(b);
        px[i * 4 + 3] = 255;
    }
    px
}

// ------------------------------------------------------------------------------------------------ geometry

pub type Quads = (Vec<Vertex>, Vec<u32>);

fn push_quad(m: &mut Quads, v: [Vertex; 4]) {
    let b = m.0.len() as u32;
    m.0.extend_from_slice(&v);
    m.1.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
}

fn vtx(p: [f32; 3], uv: [f32; 2], color: [f32; 4]) -> Vertex {
    Vertex { pos: p, uv, color, light: [15.0, 0.0] }
}

/// `LevelRenderer::renderStars` — 1500 random quads on a sphere of radius 160.
pub fn star_mesh() -> Quads {
    let mut random = Random::with_seed(10842);
    let mut out: Quads = (Vec::new(), Vec::new());
    let white = [1.0; 4];
    for _ in 0..1500 {
        let mut x = (random.next_float() * 2.0 - 1.0) as f64;
        let mut y = (random.next_float() * 2.0 - 1.0) as f64;
        let mut z = (random.next_float() * 2.0 - 1.0) as f64;
        let ss = (0.15f32 + random.next_float() * 0.10) as f64;
        let mut d = x * x + y * y + z * z;
        if d < 1.0 && d > 0.01 {
            d = 1.0 / d.sqrt();
            x *= d;
            y *= d;
            z *= d;
            let (xp, yp, zp) = (x * 160.0, y * 160.0, z * 160.0);
            let y_rot = x.atan2(z);
            let (y_sin, y_cos) = (y_rot.sin(), y_rot.cos());
            let x_rot = (x * x + z * z).sqrt().atan2(y);
            let (x_sin, x_cos) = (x_rot.sin(), x_rot.cos());
            let z_rot = random.next_double() * std::f64::consts::PI * 2.0;
            let (z_sin, z_cos) = (z_rot.sin(), z_rot.cos());
            let mut q = [Vertex::default(); 4];
            for c in 0..4i32 {
                let xo0 = 0.0;
                let yo0 = (((c & 2) - 1) as f64) * ss;
                let zo0 = ((((c + 1) & 2) - 1) as f64) * ss;
                let yo1 = yo0 * z_cos - zo0 * z_sin;
                let zo1 = zo0 * z_cos + yo0 * z_sin;
                let yo2 = yo1 * x_sin + xo0 * x_cos;
                let xo2 = xo0 * x_sin - yo1 * x_cos;
                let zo2 = zo1;
                let xo = xo2 * y_sin - zo2 * y_cos;
                let yo = yo2;
                let zo = zo2 * y_sin + xo2 * y_cos;
                q[c as usize] = vtx([(xp + xo) as f32, (yp + yo) as f32, (zp + zo) as f32], [-1.0, 0.0], white);
            }
            push_quad(&mut out, q);
        }
    }
    out
}

/// The big flat planes at y = +16 (sky) and y = −16 (void "dark" plane) — `skyList` / `darkList`.
pub fn plane_mesh(y: f32) -> Quads {
    let mut out: Quads = (Vec::new(), Vec::new());
    let (s, d) = (64i32, 256 / 64 + 2);
    let c = [1.0; 4];
    let mut xx = -s * d;
    while xx <= s * d {
        let mut zz = -s * d;
        while zz <= s * d {
            let (x0, x1, z0, z1) = (xx as f32, (xx + s) as f32, zz as f32, (zz + s) as f32);
            push_quad(&mut out, [vtx([x0, y, z0], [-1.0, 0.0], c), vtx([x1, y, z0], [-1.0, 0.0], c), vtx([x1, y, z1], [-1.0, 0.0], c), vtx([x0, y, z1], [-1.0, 0.0], c)]);
            zz += s;
        }
        xx += s;
    }
    out
}

/// Sunrise/sunset fan (`renderSky`), already rotated into world space for the given sun angle.
pub fn sunrise_fan(c: [f32; 4], sun_angle: f32) -> Quads {
    // glRotatef(90, 1,0,0); glRotatef(sin(sunAngle)<0 ? 180 : 0, 0,0,1); glRotatef(90, 0,0,1)
    let flip = mth::sin(sun_angle) < 0.0;
    let rot = glam::Mat4::from_rotation_x(90f32.to_radians()) * glam::Mat4::from_rotation_z(if flip { 180f32.to_radians() } else { 0.0 }) * glam::Mat4::from_rotation_z(90f32.to_radians());
    let steps = 16;
    let mut pts = vec![(glam::Vec3::new(0.0, 100.0, 0.0), [c[0], c[1], c[2], c[3]])];
    for i in 0..=steps {
        let a = i as f32 * PI * 2.0 / steps as f32;
        let (s, co) = (mth::sin(a), mth::cos(a));
        pts.push((glam::Vec3::new(s * 120.0, co * 120.0, -co * 40.0 * c[3]), [c[0], c[1], c[2], 0.0]));
    }
    let mut out: Quads = (Vec::new(), Vec::new());
    for (p, col) in &pts {
        let w = rot.transform_point3(*p);
        out.0.push(vtx([w.x, w.y, w.z], [-1.0, 0.0], *col));
    }
    for i in 1..=steps {
        out.1.extend_from_slice(&[0, i as u32, i as u32 + 1]);
    }
    out
}

/// Sun (30×30 at distance 100) and moon (20×20 at −100, phase tile of the 4×2 atlas), rotated by the time of day.
/// Returns (sun, moon) in world space.
pub fn sun_moon(td: f32, phase: usize, alpha: f32) -> (Quads, Quads) {
    // glRotatef(-90, 0,1,0); glRotatef(td*360, 1,0,0)
    // GL applies the *last* call first to vertices: M = R(-90,y) * R(td*360,x)
    let m = glam::Mat4::from_rotation_y((-90f32).to_radians()) * glam::Mat4::from_rotation_x((td * 360.0).to_radians());
    let w = [1.0, 1.0, 1.0, alpha];
    let tf = |p: [f32; 3]| {
        let v = m.transform_point3(glam::Vec3::from(p));
        [v.x, v.y, v.z]
    };
    let ss = 30.0;
    let mut sun: Quads = (Vec::new(), Vec::new());
    push_quad(&mut sun, [vtx(tf([-ss, 100.0, -ss]), [0.0, 0.0], w), vtx(tf([ss, 100.0, -ss]), [1.0, 0.0], w), vtx(tf([ss, 100.0, ss]), [1.0, 1.0], w), vtx(tf([-ss, 100.0, ss]), [0.0, 1.0], w)]);
    let ss = 20.0;
    let (u, v) = ((phase % 4) as f32, ((phase / 4) % 2) as f32);
    let (u0, v0, u1, v1) = (u / 4.0, v / 2.0, (u + 1.0) / 4.0, (v + 1.0) / 2.0);
    let mut moon: Quads = (Vec::new(), Vec::new());
    push_quad(&mut moon, [vtx(tf([-ss, -100.0, ss]), [u1, v1], w), vtx(tf([ss, -100.0, ss]), [u0, v1], w), vtx(tf([ss, -100.0, -ss]), [u0, v0], w), vtx(tf([-ss, -100.0, -ss]), [u1, v0], w)]);
    (sun, moon)
}

/// The End's sky: a 6-sided box textured with `tunnel.png`, colour 0x282828.
pub fn end_sky_box() -> Quads {
    let mut out: Quads = (Vec::new(), Vec::new());
    let c = [0x28 as f32 / 255.0; 3];
    let col = [c[0], c[1], c[2], 1.0];
    for i in 0..6 {
        let m = match i {
            0 => glam::Mat4::IDENTITY,
            1 => glam::Mat4::from_rotation_x(90f32.to_radians()),
            2 => glam::Mat4::from_rotation_x((-90f32).to_radians()),
            3 => glam::Mat4::from_rotation_x(180f32.to_radians()),
            4 => glam::Mat4::from_rotation_z(90f32.to_radians()),
            _ => glam::Mat4::from_rotation_z((-90f32).to_radians()),
        };
        let tf = |p: [f32; 3]| {
            let v = m.transform_point3(glam::Vec3::from(p));
            [v.x, v.y, v.z]
        };
        push_quad(&mut out, [vtx(tf([-100.0, -100.0, -100.0]), [0.0, 0.0], col), vtx(tf([-100.0, -100.0, 100.0]), [0.0, 16.0], col), vtx(tf([100.0, -100.0, 100.0]), [16.0, 16.0], col), vtx(tf([100.0, -100.0, -100.0]), [16.0, 0.0], col)]);
    }
    out
}

/// Fancy clouds (`renderAdvancedClouds`): one cube per opaque `clouds.png` texel, in a 6×6 grid of 8×8-texel
/// patches around the player. `cloud_px` is the 256×256 alpha channel. Returns world-space geometry relative to
/// the camera (y already offset by the cloud height), drawn with texture = clouds (alpha taken from the texel).
pub fn cloud_mesh(alpha_px: &[u8], cloud_rgb: [f32; 3], cam_x: f64, cam_y: f32, cam_z: f64, ticks: f32, cloud_height: f32) -> Quads {
    let (ss, h) = (12.0f32, 4.0f32);
    let time = ticks as f64;
    let mut xo = (cam_x + time * 0.03f32 as f64) / ss as f64;
    let mut zo = cam_z / ss as f64 + 0.33f32 as f64;
    let yy = cloud_height - cam_y + 0.33;
    xo -= (xo / 2048.0).floor() * 2048.0;
    zo -= (zo / 2048.0).floor() * 2048.0;
    let (xi, zi) = (xo.floor() as i32, zo.floor() as i32);
    let xoffs = (xo - xo.floor()) as f32;
    let zoffs = (zo - zo.floor()) as f32;
    let texel_a = |tx: i32, tz: i32| -> u8 { alpha_px[((tz & 255) as usize) * 256 + (tx & 255) as usize] };
    let texel = |tx: i32, tz: i32| -> bool { texel_a(tx, tz) > 0 };
    let mut out: Quads = (Vec::new(), Vec::new());
    let radius = 3;
    let d = 8;
    // face colours (rgb multipliers) from createCloudMesh: bottom .7, top 1, -x/+x .9, -z/+z .8
    let shades = [0.7f32, 1.0, 0.9, 0.9, 0.8, 0.8];
    for xp in (-radius + 1)..=radius {
        for zp in (-radius + 1)..=radius {
            for zt in 0..d {
                for xt in 0..d {
                    // texel of this cube (texture matrix translate xx/256+uo, then +(xt+.5)/256)
                    let tx = xp * d + xi + xt;
                    let tz = zp * d + zi + zt;
                    if !texel(tx, tz) {
                        continue;
                    }
                    let x0 = ((xp * d + xt) as f32 - xoffs) * ss;
                    let x1 = x0 + ss;
                    let z0 = ((zp * d + zt) as f32 - zoffs) * ss;
                    let z1 = z0 + ss;
                    let (y0, y1) = (yy, yy + h);
                    let ta = texel_a(tx, tz) as f32 / 255.0 * 0.8;
                    let colr = |f: usize| [cloud_rgb[0] * shades[f], cloud_rgb[1] * shades[f], cloud_rgb[2] * shades[f], ta];
                    let uv = [-2.0, 0.0];
                    let v = |x: f32, y: f32, z: f32, f: usize| Vertex { pos: [x, y, z], uv, color: colr(f), light: [15.0, 0.0] };
                    // bottom, top
                    push_quad(&mut out, [v(x0, y0, z0, 0), v(x1, y0, z0, 0), v(x1, y0, z1, 0), v(x0, y0, z1, 0)]);
                    push_quad(&mut out, [v(x0, y1, z1, 1), v(x1, y1, z1, 1), v(x1, y1, z0, 1), v(x0, y1, z0, 1)]);
                    if !texel(tx - 1, tz) {
                        push_quad(&mut out, [v(x0, y0, z1, 2), v(x0, y1, z1, 2), v(x0, y1, z0, 2), v(x0, y0, z0, 2)]);
                    }
                    if !texel(tx + 1, tz) {
                        push_quad(&mut out, [v(x1, y0, z0, 3), v(x1, y1, z0, 3), v(x1, y1, z1, 3), v(x1, y0, z1, 3)]);
                    }
                    if !texel(tx, tz - 1) {
                        push_quad(&mut out, [v(x0, y1, z0, 4), v(x1, y1, z0, 4), v(x1, y0, z0, 4), v(x0, y0, z0, 4)]);
                    }
                    if !texel(tx, tz + 1) {
                        push_quad(&mut out, [v(x0, y0, z1, 5), v(x1, y0, z1, 5), v(x1, y1, z1, 5), v(x0, y1, z1, 5)]);
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_of_day_matches_known_points() {
        // noon (6000) → 0.0 shifted by the 1/3 smoothing: tdo=0 → 0
        assert!((time_of_day(Dim::Overworld, 6000, 0.0) - 0.0).abs() < 1e-6);
        // midnight (18000): td=0.5 -> 0.5
        assert!((time_of_day(Dim::Overworld, 18000, 0.0) - 0.5).abs() < 1e-5);
        assert_eq!(time_of_day(Dim::End, 12345, 0.5), 0.0);
    }

    #[test]
    fn sky_is_bright_by_day_and_dark_at_night() {
        let day = sky_darken(time_of_day(Dim::Overworld, 6000, 0.0), 0.0, 0.0);
        let night = sky_darken(time_of_day(Dim::Overworld, 18000, 0.0), 0.0, 0.0);
        assert!((day - 1.0).abs() < 1e-5);
        assert!((night - 0.2).abs() < 1e-5);
        assert!(star_brightness(0.5) > 0.4 && star_brightness(0.0) == 0.0);
    }

    #[test]
    fn sunrise_only_near_the_horizon() {
        assert!(sunrise_color(Dim::Overworld, 0.0).is_none());
        // td = 0.75 (just before dawn): cos(1.5π)=0 → centre of the dawn band
        assert!(sunrise_color(Dim::Overworld, 0.75).is_some());
        assert!(sunrise_color(Dim::End, 0.75).is_none());
    }

    #[test]
    fn stars_are_deterministic() {
        let a = star_mesh();
        let b = star_mesh();
        assert_eq!(a.0.len(), b.0.len());
        assert!(a.0.len() > 4 * 700);
        assert_eq!(a.0[0], b.0[0]);
    }

    #[test]
    fn light_ramp_endpoints() {
        let r = brightness_ramp(Dim::Overworld);
        assert_eq!(r[0], 0.0);
        assert!((r[15] - 1.0).abs() < 1e-6);
        let t = light_texture(Dim::Overworld, 1.0, 0.0);
        assert_eq!(t.len(), 1024);
        assert!(t[(15 * 16 + 15) * 4] > 240);
    }
}

// ------------------------------------------------------------------------------------------------ per-frame environment

/// Everything the renderer needs to draw the sky and fog for one frame.
#[derive(Clone, Debug)]
pub struct Env {
    pub dim: Dim,
    /// glClearColor == fog colour
    pub clear: [f32; 3],
    pub sky: [f32; 3],
    pub td: f32,
    pub sky_darken: f32,
    pub star_br: f32,
    pub sunrise: Option<[f32; 4]>,
    pub sun_angle: f32,
    pub moon_phase: usize,
    pub cloud_rgb: [f32; 3],
    pub cloud_height: f32,
    /// `ticks + alpha`
    pub cloud_ticks: f32,
    /// start, end, density, mode (0 = linear, 1 = exp)
    pub fog_world: [f32; 4],
    /// start, end of the sky-dome fog
    pub fog_sky: [f32; 2],
    pub rain_brightness: f32,
    pub natural: bool,
    pub below_horizon: bool,
    pub blr: f32,
}

impl Env {
    pub fn neutral(dim: Dim) -> Env {
        compute_env(&EnvInput { dim, flat: false, day_time: 6000, partial: 0.0, eye: [0.0, 100.0, 0.0], look: [0.0, 0.0, 1.0], tile_at_eye: 0, fog_br: 1.0, biome_sky: col::BIOME_COLOURS[1][3], ticks: 0.0, rain: 0.0, thunder: 0.0, flash: 0.0 })
    }
}

pub struct EnvInput {
    pub dim: Dim,
    pub flat: bool,
    pub day_time: i64,
    pub partial: f32,
    pub eye: [f32; 3],
    pub look: [f32; 3],
    pub tile_at_eye: u8,
    pub fog_br: f32,
    pub biome_sky: u32,
    pub ticks: f32,
    /// `Level::getRainLevel` / `getThunderLevel` and the lightning flash (`skyFlashTime - partial`)
    pub rain: f32,
    pub thunder: f32,
    pub flash: f32,
}

/// Console default `Options.viewDistance == 0` → `renderDistance = 16*16 >> 0`.
pub const VIEW_DISTANCE_OPTION: i32 = 0;
pub const RENDER_DISTANCE: f32 = 256.0;

pub fn compute_env(i: &EnvInput) -> Env {
    use mc_world::tile_ids as t;
    let td = time_of_day(i.dim, i.day_time, i.partial);
    let mut sky = sky_color(i.biome_sky, td, i.rain, i.thunder);
    if i.flash > 0.0 {
        // `Level::getSkyColor`: lightning washes the sky out
        let f = i.flash.min(1.0) * 0.45;
        sky = [sky[0] * (1.0 - f) + 0.8 * f, sky[1] * (1.0 - f) + 0.8 * f, sky[2] * (1.0 - f) + f];
    }
    let mut f = world_fog_color(i.dim, td, sky, i.look, VIEW_DISTANCE_OPTION, i.rain, i.thunder);
    let water = matches!(i.tile_at_eye, t::WATER | t::CALM_WATER);
    let lava = matches!(i.tile_at_eye, t::LAVA | t::CALM_LAVA);
    if water {
        let c = col::UNDER_WATER_CLEAR;
        f = [((c >> 16) & 0xff) as f32 / 256.0, ((c >> 8) & 0xff) as f32 / 256.0, (c & 0xff) as f32 / 256.0];
    } else if lava {
        let c = col::UNDER_LAVA_CLEAR;
        f = [((c >> 16) & 0xff) as f32 / 256.0, ((c >> 8) & 0xff) as f32 / 256.0, (c & 0xff) as f32 / 256.0];
    }
    for v in f.iter_mut() {
        *v *= i.fog_br;
    }
    let scale = if i.flat { 1.0 } else { 1.0 / 32.0 };
    let mut yy = i.eye[1] as f64 * scale;
    if yy < 1.0 {
        yy = yy.max(0.0);
        yy *= yy;
        for v in f.iter_mut() {
            *v *= yy as f32;
        }
    }
    let d = RENDER_DISTANCE;
    let fog_world = if water {
        [0.0, 0.0, 0.1, 1.0]
    } else if lava {
        [0.0, 0.0, 2.0, 1.0]
    } else {
        [d * 0.25, d, 0.0, 0.0]
    };
    let horizon = if i.flat { 0.0 } else { 63.0 };
    Env {
        dim: i.dim,
        clear: f,
        sky,
        td,
        sky_darken: if i.flash > 0.0 { 1.0 } else { sky_darken(td, i.rain, i.thunder) },
        star_br: star_brightness(td),
        sunrise: sunrise_color(i.dim, td),
        sun_angle: td * PI * 2.0,
        moon_phase: moon_phase(i.day_time),
        cloud_rgb: cloud_color(td, i.rain, i.thunder),
        cloud_height: if i.dim == Dim::End { 8.0 } else { 128.0 },
        cloud_ticks: i.ticks,
        fog_world,
        fog_sky: [0.0, d * 0.8],
        rain_brightness: 1.0 - i.rain,
        natural: i.dim == Dim::Overworld || i.dim == Dim::Flat,
        below_horizon: (i.eye[1] as f64) - horizon < 0.0,
        blr: 0.0,
    }
}

#[cfg(test)]
mod end_tests {
    use super::*;
    #[test]
    fn end_light_texture_is_dim() {
        let t = light_texture(Dim::End, 1.0, 0.0);
        let px = |sky: usize, blk: usize| (t[(sky * 16 + blk) * 4], t[(sky * 16 + blk) * 4 + 1], t[(sky * 16 + blk) * 4 + 2]);
        // sky light is ignored in the End; block light adds up to 0.22 + 0.75
        assert_eq!(px(15, 0), (56, 71, 63));
        assert_eq!(px(0, 0), (56, 71, 63));
        assert!(px(0, 15).0 > 200);
    }
}
