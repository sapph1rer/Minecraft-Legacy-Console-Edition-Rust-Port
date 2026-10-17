//! Mob and arrow rendering: `LivingEntityRenderer::render` + the per-species renderers + `ArrowRenderer`.

use crate::mesh::MeshData;
use crate::mob::{Mob, MobKind};
use crate::mobs::{Arrow, Mobs};
use crate::model::*;
use crate::render::EntTex;
use glam::{Mat4, Vec3};
use mc_level::level::Level;

const RAD: f32 = 180.0 / PI;

/// `LivingEntityRenderer::rotlerp`
fn rotlerp(from: f32, to: f32, a: f32) -> f32 {
    let mut d = to - from;
    while d < -180.0 {
        d += 360.0;
    }
    while d >= 180.0 {
        d -= 360.0;
    }
    from + a * d
}

/// `Sheep::COLOR`
pub const SHEEP_COLOR: [[f32; 3]; 16] = [
    [1.0, 1.0, 1.0],
    [0.85, 0.5, 0.2],
    [0.7, 0.3, 0.85],
    [0.4, 0.6, 0.85],
    [0.9, 0.9, 0.2],
    [0.5, 0.8, 0.1],
    [0.95, 0.5, 0.65],
    [0.3, 0.3, 0.3],
    [0.6, 0.6, 0.6],
    [0.3, 0.5, 0.65],
    [0.5, 0.25, 0.7],
    [0.2, 0.3, 0.7],
    [0.4, 0.3, 0.2],
    [0.4, 0.5, 0.2],
    [0.6, 0.2, 0.2],
    [0.1, 0.1, 0.1],
];

pub struct MobModels {
    zombie: HumanoidModel,
    skeleton: HumanoidModel,
    pig: QuadrupedModel,
    cow: QuadrupedModel,
    sheep: QuadrupedModel,
    fur: QuadrupedModel,
    chicken: ChickenModel,
    creeper: CreeperModel,
    spider: SpiderModel,
    slime: SlimeModel,
    slime_shell: SlimeModel,
    lava_slime: LavaSlimeModel,
}

impl Default for MobModels {
    fn default() -> Self {
        MobModels {
            zombie: HumanoidModel::zombie(),
            skeleton: HumanoidModel::skeleton(),
            pig: QuadrupedModel::pig(0.0),
            cow: QuadrupedModel::cow(),
            sheep: QuadrupedModel::sheep(),
            fur: QuadrupedModel::sheep_fur(),
            chicken: ChickenModel::new(),
            creeper: CreeperModel::new(0.0),
            spider: SpiderModel::new(),
            slime: SlimeModel::new(16),
            slime_shell: SlimeModel::new(0),
            lava_slime: LavaSlimeModel::new(),
        }
    }
}

#[derive(Default)]
struct Layers {
    em: std::collections::HashMap<usize, (EntTex, Emitter)>,
}

impl Layers {
    fn get(&mut self, t: EntTex) -> &mut Emitter {
        &mut self.em.entry(t as usize).or_insert_with(|| (t, Emitter::new())).1
    }
}

fn sheep_eat(m: &Mob, a: f32) -> (f32, f32) {
    let t = m.eat_anim as f32;
    let pos = if m.eat_anim <= 0 {
        0.0
    } else if m.eat_anim >= 4 && m.eat_anim <= 36 {
        1.0
    } else if m.eat_anim < 4 {
        (t - a) / 4.0
    } else {
        -(t - 40.0 - a) / 4.0
    };
    let ang = if m.eat_anim > 4 && m.eat_anim <= 36 {
        let v = (t - 4.0 - a) / 32.0;
        PI * 0.2 + PI * 0.07 * (v * 28.7).sin()
    } else if m.eat_anim > 0 {
        PI * 0.2
    } else {
        f32::NAN
    };
    (pos, ang)
}

impl MobModels {
    /// All mobs and arrows as world-space entity layers.
    pub fn layers(&mut self, mobs: &Mobs, lv: &Level, a: f32) -> Vec<(EntTex, MeshData)> {
        let mut ls = Layers::default();
        for m in &mobs.list {
            self.render_mob(m, lv, a, &mut ls);
        }
        for ar in &mobs.arrows {
            render_arrow(ar, lv, a, ls.get(EntTex::Arrow));
        }
        ls.em.into_values().filter(|(_, e)| !e.mesh.is_empty()).map(|(t, e)| (t, e.mesh)).collect()
    }

    fn render_mob(&mut self, m: &Mob, lv: &Level, a: f32, ls: &mut Layers) {
        let b = &m.b;
        let x = b.xo + (b.x - b.xo) * a as f64;
        let y = b.yo + (b.y - b.yo) * a as f64;
        let z = b.zo + (b.z - b.zo) * a as f64;
        let body_rot = rotlerp(m.y_body_rot_o, m.y_body_rot, a);
        let head_rot = rotlerp(m.y_head_rot_o, m.y_head_rot, a);
        let x_rot = m.x_rot_o + (b.pitch - m.x_rot_o) * a;
        let mut mat = Mat4::from_translation(Vec3::new(x as f32, y as f32 - b.height_offset as f32, z as f32));
        mat *= Mat4::from_rotation_y((180.0 - body_rot).to_radians());
        if m.death_time > 0 {
            let mut fall = (m.death_time as f32 + a - 1.0) / 20.0 * 1.6;
            fall = fall.sqrt().min(1.0);
            // `getFlipDegrees`: 90, spiders turn over completely
            let flip = if m.kind.is_spider() { 180.0_f32 } else { 90.0_f32 };
            mat *= Mat4::from_rotation_z(fall * flip.to_radians());
        }
        let s = 1.0 / 16.0;
        mat *= Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0));
        let mut swell = 0.0;
        if m.kind == MobKind::Creeper {
            let mut g = m.swelling(a);
            let wobble = 1.0 + (g * 100.0).sin() * g * 0.01;
            g = g.clamp(0.0, 1.0);
            g *= g;
            g *= g;
            let ss = (1.0 + g * 0.4) * wobble;
            let hs = (1.0 + g * 0.1) / wobble;
            mat *= Mat4::from_scale(Vec3::new(ss, hs, ss));
            swell = m.swelling(a);
        }
        if m.kind == MobKind::CaveSpider {
            // `CaveSpiderRenderer::scale`
            mat *= Mat4::from_scale(Vec3::splat(0.7));
        }
        if m.kind.is_slime() {
            // `SlimeRenderer::scale` / `LavaSlimeRenderer::scale`
            let size = m.size as f32;
            let ss = (m.o_squish + (m.squish - m.o_squish) * a) / (size * 0.5 + 1.0);
            let w = 1.0 / (ss + 1.0);
            mat *= Mat4::from_scale(Vec3::new(w * size, 1.0 / w * size, w * size));
        }
        mat *= Mat4::from_translation(Vec3::new(0.0, -24.0 * s - 0.0078125, 0.0));
        let mut walk_speed = m.walk_anim_speed_o + (m.walk_anim_speed - m.walk_anim_speed_o) * a;
        let walk_pos = m.walk_anim_pos - m.walk_anim_speed * (1.0 - a);
        walk_speed = walk_speed.min(1.0);
        let bob = m.tick_count as f32 + a;
        let yr = head_rot - body_rot;
        let (sky, blk) = lv.tile_light_color(x.floor() as i32, (y + b.h * 0.66).floor() as i32, z.floor() as i32);
        let hurt = m.hurt_time > 0 || m.death_time > 0;
        let tint = if hurt { [1.0, 0.6, 0.6, 1.0] } else { [1.0; 4] };
        let flash = m.kind == MobKind::Creeper && swell > 0.0 && (swell * 10.0) as i32 % 2 == 1;
        let setup = |em: &mut Emitter| {
            em.color = tint;
            // `LavaSlime::getLightColor` is full bright
            em.light = if flash || m.kind == MobKind::LavaSlime { [15.0, 15.0] } else { [sky as f32, blk as f32] };
            em.normal_xf = Mat4::IDENTITY;
            em.lit = true;
        };
        let baby = m.is_baby();
        match m.kind {
            MobKind::Zombie | MobKind::Skeleton => {
                let (tex, model) = if m.kind == MobKind::Zombie { (EntTex::Zombie, &mut self.zombie) } else { (EntTex::Skeleton, &mut self.skeleton) };
                model.attack_time = 0.0;
                model.young = false;
                model.setup_anim(walk_pos, walk_speed, bob, yr, x_rot);
                let em = ls.get(tex);
                setup(em);
                model.render(s, &mat, em);
            }
            MobKind::Pig | MobKind::Cow | MobKind::Sheep => {
                let (tex, model) = match m.kind {
                    MobKind::Pig => (EntTex::Pig, &mut self.pig),
                    MobKind::Cow => (EntTex::Cow, &mut self.cow),
                    _ => (EntTex::Sheep, &mut self.sheep),
                };
                let (ey, ex) = if m.kind == MobKind::Sheep { sheep_eat(m, a) } else { (0.0, f32::NAN) };
                let ex = if ex.is_nan() { x_rot / RAD } else { ex };
                model.young = baby;
                model.setup_anim(walk_pos, walk_speed, yr, x_rot, ey, ex);
                let em = ls.get(tex);
                setup(em);
                model.render(s, &mat, em);
                if m.kind == MobKind::Sheep && !m.sheared {
                    let f = &mut self.fur;
                    f.young = baby;
                    f.setup_anim(walk_pos, walk_speed, yr, x_rot, ey, ex);
                    let c = SHEEP_COLOR[(m.sheep_color & 15) as usize];
                    let em = ls.get(EntTex::SheepFur);
                    setup(em);
                    em.color = [c[0] * tint[0], c[1] * tint[1], c[2] * tint[2], 1.0];
                    f.render(s, &mat, em);
                }
            }
            MobKind::Chicken => {
                let fl = m.o_flap + (m.flap - m.o_flap) * a;
                let fs = m.o_flap_speed + (m.flap_speed - m.o_flap_speed) * a;
                let wing = (fl.sin() + 1.0) * fs;
                let model = &mut self.chicken;
                model.young = baby;
                model.setup_anim(walk_pos, walk_speed, wing, yr, x_rot);
                let em = ls.get(EntTex::Chicken);
                setup(em);
                model.render(s, &mat, em);
            }
            MobKind::Spider | MobKind::CaveSpider => {
                let model = &mut self.spider;
                model.setup_anim(walk_pos, walk_speed, yr, x_rot);
                let tex = if m.kind == MobKind::Spider { EntTex::Spider } else { EntTex::CaveSpider };
                let em = ls.get(tex);
                setup(em);
                model.render(s, &mat, em);
                // `SpiderRenderer::prepareArmor`: the eyes, additive and full bright
                let em = ls.get(EntTex::SpiderEyes);
                em.color = [1.0; 4];
                em.light = [15.0, 15.0];
                em.normal_xf = Mat4::IDENTITY;
                em.lit = true;
                model.render(s, &mat, em);
            }
            MobKind::Slime => {
                // the inner body with the face, then the translucent shell (`SlimeRenderer::prepareArmor`, layer 0)
                let em = ls.get(EntTex::Slime);
                setup(em);
                self.slime.render(s, &mat, em);
                em.color = [tint[0], tint[1], tint[2], 1.0];
                self.slime_shell.render(s, &mat, em);
            }
            MobKind::LavaSlime => {
                let model = &mut self.lava_slime;
                model.prepare(m.o_squish + (m.squish - m.o_squish) * a);
                let em = ls.get(EntTex::LavaSlime);
                setup(em);
                model.render(s, &mat, em);
            }
            MobKind::Creeper => {
                let model = &mut self.creeper;
                model.setup_anim(walk_pos, walk_speed, yr, x_rot);
                let em = ls.get(EntTex::Creeper);
                setup(em);
                model.render(s, &mat, em);
            }
        }
    }
}

/// `ArrowRenderer::render`
fn render_arrow(ar: &Arrow, lv: &Level, a: f32, em: &mut Emitter) {
    let (mut yr, mut xr) = (ar.yaw, ar.pitch);
    if yr - ar.yaw_o > 180.0 {
        yr -= 360.0;
    } else if yr - ar.yaw_o < -180.0 {
        yr += 360.0;
    }
    if xr - ar.pitch_o > 180.0 {
        xr -= 360.0;
    } else if xr - ar.pitch_o < -180.0 {
        xr += 360.0;
    }
    let x = ar.xo + (ar.x - ar.xo) * a as f64;
    let y = ar.yo + (ar.y - ar.yo) * a as f64;
    let z = ar.zo + (ar.z - ar.zo) * a as f64;
    let (sky, blk) = lv.tile_light_color(x.floor() as i32, y.floor() as i32, z.floor() as i32);
    em.light = [sky as f32, blk as f32];
    em.color = [1.0; 4];
    em.lit = false;
    let mut m = Mat4::from_translation(Vec3::new(x as f32, y as f32, z as f32));
    m *= Mat4::from_rotation_y((ar.yaw_o + (yr - ar.yaw_o) * a - 90.0).to_radians());
    m *= Mat4::from_rotation_z((ar.pitch_o + (xr - ar.pitch_o) * a).to_radians());
    let shake = ar.shake as f32 - a;
    if shake > 0.0 {
        m *= Mat4::from_rotation_z((-(shake * 3.0).sin() * shake).to_radians());
    }
    let ss = 0.9 / 16.0;
    m *= Mat4::from_rotation_x(45.0_f32.to_radians());
    m *= Mat4::from_scale(Vec3::splat(ss));
    m *= Mat4::from_translation(Vec3::new(-4.0, 0.0, 0.0));
    let (u0, u1, v0, v1) = (0.0, 16.0 / 32.0, 0.0, 5.0 / 32.0);
    let (u02, u12, v02, v12) = (0.0, 5.0 / 32.0, 5.0 / 32.0, 10.0 / 32.0);
    let n = Vec3::X;
    em.quad([[-7.0, -2.0, -2.0], [-7.0, -2.0, 2.0], [-7.0, 2.0, 2.0], [-7.0, 2.0, -2.0]], [[u02, v02], [u12, v02], [u12, v12], [u02, v12]], n, &m, [1.0; 3]);
    em.quad([[-7.0, 2.0, -2.0], [-7.0, 2.0, 2.0], [-7.0, -2.0, 2.0], [-7.0, -2.0, -2.0]], [[u02, v02], [u12, v02], [u12, v12], [u02, v12]], -n, &m, [1.0; 3]);
    for i in 0..4 {
        let r = m * Mat4::from_rotation_x((90.0 * (i + 1) as f32).to_radians());
        em.quad([[-8.0, -2.0, 0.0], [8.0, -2.0, 0.0], [8.0, 2.0, 0.0], [-8.0, 2.0, 0.0]], [[u0, v0], [u1, v0], [u1, v1], [u0, v1]], Vec3::Z, &r, [1.0; 3]);
    }
    em.lit = true;
}
