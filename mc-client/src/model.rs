//! Port of `Model`, `ModelPart`, `Cube`, `_Polygon` and the humanoid / quadruped / chicken / creeper models.
//! Geometry is baked on the CPU into world-space vertices by [`Emitter`] (replaces the fixed-function GL
//! matrix stack + display lists + `Lighting::turnOn` of the original).

use crate::mesh::{MeshData, Vertex};
use glam::{Mat4, Vec3};

pub const PI: f32 = std::f32::consts::PI;
pub const HALF_PI: f32 = PI / 2.0;
/// `ModelPart::RAD`
const RAD: f32 = 180.0 / PI;

/// Light directions of `Lighting::turnOn`.
fn light_dirs() -> [Vec3; 2] {
    [Vec3::new(0.2, 1.0, -0.7).normalize(), Vec3::new(-0.2, 1.0, 0.7).normalize()]
}

/// Collects transformed polygons into a vertex/index mesh with `Lighting::turnOn` style shading
/// (ambient 0.4 + two 0.6 diffuse lights) baked into the vertex colour.
pub struct Emitter {
    pub mesh: MeshData,
    /// glColor
    pub color: [f32; 4],
    /// (sky, block) light for the lightmap lookup
    pub light: [f32; 2],
    /// maps model-space normals (after the model matrix) into the frame the light directions are given in
    pub normal_xf: Mat4,
    pub lit: bool,
    lights: [Vec3; 2],
}

impl Emitter {
    pub fn new() -> Emitter {
        Emitter { mesh: MeshData::default(), color: [1.0; 4], light: [15.0, 0.0], normal_xf: Mat4::IDENTITY, lit: true, lights: light_dirs() }
    }

    fn shade(&self, n: Vec3) -> f32 {
        if !self.lit {
            return 1.0;
        }
        let n = self.normal_xf.transform_vector3(n).normalize_or_zero();
        (0.4 + 0.6 * n.dot(self.lights[0]).max(0.0) + 0.6 * n.dot(self.lights[1]).max(0.0)).min(1.0)
    }

    /// A lit quad with an explicit normal (item meshes). `tint` multiplies the colour.
    pub fn quad(&mut self, pos: [[f32; 3]; 4], uv: [[f32; 2]; 4], n: Vec3, m: &Mat4, tint: [f32; 3]) {
        let s = self.shade(m.transform_vector3(n));
        let col = [self.color[0] * tint[0] * s, self.color[1] * tint[1] * s, self.color[2] * tint[2] * s, self.color[3]];
        let b = self.mesh.vertices.len() as u32;
        for i in 0..4 {
            let w = m.transform_point3(Vec3::from(pos[i]));
            self.mesh.vertices.push(Vertex { pos: w.to_array(), uv: uv[i], color: col, light: self.light });
        }
        self.mesh.indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }

    /// `_Polygon::render`
    pub fn polygon(&mut self, p: &Polygon, m: &Mat4, scale: f32) {
        let v0 = Vec3::from(p.v[0].0) - Vec3::from(p.v[1].0);
        let v1 = Vec3::from(p.v[2].0) - Vec3::from(p.v[1].0);
        let mut n = v1.cross(v0).normalize_or_zero();
        if p.flip_normal {
            n = -n;
        }
        let wn = m.transform_vector3(n);
        let s = self.shade(wn);
        let col = [self.color[0] * s, self.color[1] * s, self.color[2] * s, self.color[3]];
        let b = self.mesh.vertices.len() as u32;
        for (pos, uv) in &p.v {
            let w = m.transform_point3(Vec3::from(*pos) * scale);
            self.mesh.vertices.push(Vertex { pos: w.to_array(), uv: *uv, color: col, light: self.light });
        }
        self.mesh.indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
}

impl Default for Emitter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct Polygon {
    /// (position in model units, uv 0..1)
    pub v: [([f32; 3], [f32; 2]); 4],
    pub flip_normal: bool,
}

impl Polygon {
    /// `_Polygon(vertices, u0, v0, u1, v1, xTexSize, yTexSize)`
    fn new(pos: [[f32; 3]; 4], u0: f32, v0: f32, u1: f32, v1: f32, xs: f32, ys: f32) -> Polygon {
        let us = if u1 > u0 { 0.1 / xs } else { -0.1 / xs };
        let vs = if v1 > v0 { 0.1 / ys } else { -0.1 / ys };
        Polygon {
            v: [
                (pos[0], [u1 / xs - us, v0 / ys + vs]),
                (pos[1], [u0 / xs + us, v0 / ys + vs]),
                (pos[2], [u0 / xs + us, v1 / ys - vs]),
                (pos[3], [u1 / xs - us, v1 / ys - vs]),
            ],
            flip_normal: false,
        }
    }
    fn mirror(&mut self) {
        self.v.reverse();
    }
}

#[derive(Clone, Debug)]
pub struct Cube {
    pub x0: f32,
    pub y0: f32,
    pub z0: f32,
    pub x1: f32,
    pub y1: f32,
    pub z1: f32,
    pub polys: Vec<Polygon>,
}

impl Cube {
    #[allow(clippy::too_many_arguments)]
    pub fn new(mirror: bool, xs: f32, ys: f32, xo: i32, yo: i32, x0: f32, y0: f32, z0: f32, w: i32, h: i32, d: i32, g: f32, face_mask: u32, flip3: bool) -> Cube {
        let (ox0, oy0, oz0) = (x0, y0, z0);
        let (ox1, oy1, oz1) = (x0 + w as f32, y0 + h as f32, z0 + d as f32);
        let (mut x0, mut y0, mut z0, mut x1, mut y1, mut z1) = (x0 - g, y0 - g, z0 - g, ox1 + g, oy1 + g, oz1 + g);
        if mirror {
            std::mem::swap(&mut x0, &mut x1);
        }
        let u0 = [x0, y0, z0];
        let u1 = [x1, y0, z0];
        let u2 = [x1, y1, z0];
        let u3 = [x0, y1, z0];
        let l0 = [x0, y0, z1];
        let l1 = [x1, y0, z1];
        let l2 = [x1, y1, z1];
        let l3 = [x0, y1, z1];
        let (xo, yo, w, h, d) = (xo as f32, yo as f32, w as f32, h as f32, d as f32);
        let mut polys = Vec::new();
        let mut add = |bit: u32, v: [[f32; 3]; 4], a: f32, b: f32, c: f32, e: f32| {
            if face_mask & bit != 0 {
                polys.push(Polygon::new(v, a, b, c, e, xs, ys));
            }
        };
        add(1, [l1, u1, u2, l2], xo + d + w, yo + d, xo + d + w + d, yo + d + h); // right
        add(2, [u0, l0, l3, u3], xo, yo + d, xo + d, yo + d + h); // left
        add(4, [l1, l0, u0, u1], xo + d, yo, xo + d + w, yo + d); // up
        if flip3 {
            add(8, [u2, u3, l3, l2], xo + d + w, yo, xo + d + w + w, yo + d); // down
        } else {
            add(8, [u2, u3, l3, l2], xo + d + w, yo + d, xo + d + w + w, yo); // down
        }
        add(16, [u1, u0, u3, u2], xo + d, yo + d, xo + d + w, yo + d + h); // front
        add(32, [l0, l1, l2, l3], xo + d + w + d, yo + d, xo + d + w + d + w, yo + d + h); // back
        if mirror {
            for p in &mut polys {
                p.mirror();
            }
        }
        Cube { x0: ox0, y0: oy0, z0: oz0, x1: ox1, y1: oy1, z1: oz1, polys }
    }
}

#[derive(Clone, Debug)]
pub struct ModelPart {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub x_rot: f32,
    pub y_rot: f32,
    pub z_rot: f32,
    pub translate: Vec3,
    pub visible: bool,
    pub never_render: bool,
    pub mirror: bool,
    pub tex_w: f32,
    pub tex_h: f32,
    tex_x: i32,
    tex_y: i32,
    pub cubes: Vec<Cube>,
    pub children: Vec<ModelPart>,
}

impl ModelPart {
    /// `new ModelPart(model, xTexOffs, yTexOffs)` with the model's texture size.
    pub fn new(tex_w: i32, tex_h: i32, tx: i32, ty: i32) -> ModelPart {
        ModelPart {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            x_rot: 0.0,
            y_rot: 0.0,
            z_rot: 0.0,
            translate: Vec3::ZERO,
            visible: true,
            never_render: false,
            mirror: false,
            tex_w: tex_w as f32,
            tex_h: tex_h as f32,
            tex_x: tx,
            tex_y: ty,
            cubes: Vec::new(),
            children: Vec::new(),
        }
    }
    pub fn tex_offs(&mut self, x: i32, y: i32) -> &mut Self {
        self.tex_x = x;
        self.tex_y = y;
        self
    }
    pub fn set_pos(&mut self, x: f32, y: f32, z: f32) -> &mut Self {
        self.x = x;
        self.y = y;
        self.z = z;
        self
    }
    fn push(&mut self, x0: f32, y0: f32, z0: f32, w: i32, h: i32, d: i32, g: f32, mask: u32, flip: bool) -> &mut Self {
        let c = Cube::new(self.mirror, self.tex_w, self.tex_h, self.tex_x, self.tex_y, x0, y0, z0, w, h, d, g, mask, flip);
        self.cubes.push(c);
        self
    }
    /// `addBox(x0,y0,z0,w,h,d,g)`
    pub fn add_box(&mut self, x0: f32, y0: f32, z0: f32, w: i32, h: i32, d: i32, g: f32) -> &mut Self {
        self.push(x0, y0, z0, w, h, d, g, 63, false)
    }
    /// `addHumanoidBox` (flipped bottom-face UVs so player skins show correctly)
    pub fn add_humanoid_box(&mut self, x0: f32, y0: f32, z0: f32, w: i32, h: i32, d: i32, g: f32) -> &mut Self {
        self.push(x0, y0, z0, w, h, d, g, 63, true)
    }

    /// `ModelPart::render`
    pub fn render(&self, scale: f32, m: &Mat4, em: &mut Emitter, hide_self: bool) {
        if self.never_render || !self.visible {
            return;
        }
        let m1 = *m * Mat4::from_translation(self.translate);
        let rotated = self.x_rot != 0.0 || self.y_rot != 0.0 || self.z_rot != 0.0;
        let mut m2 = m1;
        if rotated || self.x != 0.0 || self.y != 0.0 || self.z != 0.0 {
            m2 *= Mat4::from_translation(Vec3::new(self.x, self.y, self.z) * scale);
        }
        if rotated {
            if self.z_rot != 0.0 {
                m2 *= Mat4::from_rotation_z(self.z_rot);
            }
            if self.y_rot != 0.0 {
                m2 *= Mat4::from_rotation_y(self.y_rot);
            }
            if self.x_rot != 0.0 {
                m2 *= Mat4::from_rotation_x(self.x_rot);
            }
        }
        if !hide_self {
            for c in &self.cubes {
                for p in &c.polys {
                    em.polygon(p, &m2, scale);
                }
            }
        }
        for ch in &self.children {
            ch.render(scale, &m2, em, false);
        }
    }

    /// `ModelPart::translateTo` – the matrix to attach something to this part (hand item, etc.).
    pub fn translate_to(&self, scale: f32, m: &Mat4) -> Mat4 {
        let mut r = *m;
        let rotated = self.x_rot != 0.0 || self.y_rot != 0.0 || self.z_rot != 0.0;
        if rotated || self.x != 0.0 || self.y != 0.0 || self.z != 0.0 {
            r *= Mat4::from_translation(Vec3::new(self.x, self.y, self.z) * scale);
        }
        if rotated {
            if self.z_rot != 0.0 {
                r *= Mat4::from_rotation_z(self.z_rot);
            }
            if self.y_rot != 0.0 {
                r *= Mat4::from_rotation_y(self.y_rot);
            }
            if self.x_rot != 0.0 {
                r *= Mat4::from_rotation_x(self.x_rot);
            }
        }
        r
    }
}

fn part(w: i32, h: i32, tx: i32, ty: i32) -> ModelPart {
    ModelPart::new(w, h, tx, ty)
}

// ---------------------------------------------------------------------------------------------------------------
// Humanoid (player / zombie / skeleton)
// ---------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HumanoidKind {
    Player,
    Zombie,
    Skeleton,
}

#[derive(Clone)]
pub struct HumanoidModel {
    pub kind: HumanoidKind,
    pub head: ModelPart,
    pub hair: ModelPart,
    pub body: ModelPart,
    pub arm0: ModelPart,
    pub arm1: ModelPart,
    pub leg0: ModelPart,
    pub leg1: ModelPart,
    pub holding_left: i32,
    pub holding_right: i32,
    pub sneaking: bool,
    pub riding: bool,
    pub young: bool,
    pub attack_time: f32,
    pub tex_w: i32,
    pub tex_h: i32,
}

impl HumanoidModel {
    /// `HumanoidModel(g, yOffset, texW, texH)`; the Skeleton/Zombie constructors follow.
    pub fn new(kind: HumanoidKind, g: f32, y_offset: f32, tex_w: i32, tex_h: i32) -> HumanoidModel {
        let (w, h) = (tex_w, tex_h);
        let mut head = part(w, h, 0, 0);
        head.add_humanoid_box(-4.0, -8.0, -4.0, 8, 8, 8, g).set_pos(0.0, y_offset, 0.0);
        let mut hair = part(w, h, 32, 0);
        hair.add_humanoid_box(-4.0, -8.0, -4.0, 8, 8, 8, g + 0.5).set_pos(0.0, y_offset, 0.0);
        let mut body = part(w, h, 16, 16);
        body.add_humanoid_box(-4.0, 0.0, -2.0, 8, 12, 4, g).set_pos(0.0, y_offset, 0.0);
        let mut arm0 = part(w, h, 40, 16);
        let mut arm1 = part(w, h, 40, 16);
        arm1.mirror = true;
        let mut leg0 = part(w, h, 0, 16);
        let mut leg1 = part(w, h, 0, 16);
        leg1.mirror = true;
        if kind == HumanoidKind::Skeleton {
            // SkeletonModel::_init replaces the limbs with 2-wide ones
            arm0.add_box(-1.0, -2.0, -1.0, 2, 12, 2, g).set_pos(-5.0, 2.0, 0.0);
            arm1.add_box(-1.0, -2.0, -1.0, 2, 12, 2, g).set_pos(5.0, 2.0, 0.0);
            leg0.add_box(-1.0, 0.0, -1.0, 2, 12, 2, g).set_pos(-2.0, 12.0, 0.0);
            leg1.add_box(-1.0, 0.0, -1.0, 2, 12, 2, g).set_pos(2.0, 12.0, 0.0);
        } else {
            arm0.add_humanoid_box(-3.0, -2.0, -2.0, 4, 12, 4, g).set_pos(-5.0, 2.0 + y_offset, 0.0);
            arm1.add_humanoid_box(-1.0, -2.0, -2.0, 4, 12, 4, g).set_pos(5.0, 2.0 + y_offset, 0.0);
            leg0.add_humanoid_box(-2.0, 0.0, -2.0, 4, 12, 4, g).set_pos(-1.9, 12.0 + y_offset, 0.0);
            leg1.add_humanoid_box(-2.0, 0.0, -2.0, 4, 12, 4, g).set_pos(1.9, 12.0 + y_offset, 0.0);
        }
        HumanoidModel { kind, head, hair, body, arm0, arm1, leg0, leg1, holding_left: 0, holding_right: 0, sneaking: false, riding: false, young: false, attack_time: 0.0, tex_w, tex_h }
    }

    pub fn player() -> HumanoidModel {
        HumanoidModel::new(HumanoidKind::Player, 0.0, 0.0, 64, 32)
    }
    pub fn zombie() -> HumanoidModel {
        HumanoidModel::new(HumanoidKind::Zombie, 0.0, 0.0, 64, 32)
    }
    pub fn skeleton() -> HumanoidModel {
        HumanoidModel::new(HumanoidKind::Skeleton, 0.0, 0.0, 64, 32)
    }

    /// `HumanoidModel::setupAnim` (+ `ZombieModel` / `SkeletonModel` overrides). Angles: `y_rot`/`x_rot` in degrees.
    pub fn setup_anim(&mut self, time: f32, r: f32, bob: f32, y_rot: f32, x_rot: f32) {
        let at = self.attack_time;
        self.head.y_rot = y_rot / RAD;
        self.head.x_rot = x_rot / RAD;
        self.hair.y_rot = self.head.y_rot;
        self.hair.x_rot = self.head.x_rot;
        self.body.z = 0.0;
        self.arm0.x_rot = ((time * 0.6662 + PI).cos() * 2.0) * r * 0.5;
        self.arm1.x_rot = ((time * 0.6662).cos() * 2.0) * r * 0.5;
        self.arm0.z_rot = 0.0;
        self.arm1.z_rot = 0.0;
        self.leg0.y_rot = 0.0;
        self.leg1.y_rot = 0.0;
        if self.riding {
            self.arm0.x_rot += -HALF_PI * 0.4;
            self.arm1.x_rot += -HALF_PI * 0.4;
            self.leg0.x_rot = -HALF_PI * 0.8;
            self.leg1.x_rot = -HALF_PI * 0.8;
            self.leg0.y_rot = HALF_PI * 0.2;
            self.leg1.y_rot = -HALF_PI * 0.2;
        } else {
            self.leg0.x_rot = (time * 0.6662).cos() * 1.4 * r;
            self.leg1.x_rot = (time * 0.6662 + PI).cos() * 1.4 * r;
        }
        if self.holding_left != 0 {
            self.arm1.x_rot = self.arm1.x_rot * 0.5 - HALF_PI * 0.2 * self.holding_left as f32;
        }
        if self.holding_right != 0 {
            self.arm0.x_rot = self.arm0.x_rot * 0.5 - HALF_PI * 0.2 * self.holding_right as f32;
        }
        self.arm0.y_rot = 0.0;
        self.arm1.y_rot = 0.0;
        if at > -9990.0 {
            let mut swing = at;
            self.body.y_rot = (swing.sqrt() * PI * 2.0).sin() * 0.2;
            self.arm0.z = self.body.y_rot.sin() * 5.0;
            self.arm0.x = -self.body.y_rot.cos() * 5.0;
            self.arm1.z = -self.body.y_rot.sin() * 5.0;
            self.arm1.x = self.body.y_rot.cos() * 5.0;
            self.arm0.y_rot += self.body.y_rot;
            self.arm1.y_rot += self.body.y_rot;
            self.arm1.x_rot += self.body.y_rot;
            swing = 1.0 - at;
            swing *= swing;
            swing *= swing;
            swing = 1.0 - swing;
            let aa = (swing * PI).sin();
            let bb = (at * PI).sin() * -(self.head.x_rot - 0.7) * 0.75;
            self.arm0.x_rot -= aa * 1.2 + bb;
            self.arm0.y_rot += self.body.y_rot * 2.0;
            self.arm0.z_rot = (at * PI).sin() * -0.4;
        }
        if self.sneaking {
            self.body.x_rot = 0.5;
            self.arm0.x_rot += 0.4;
            self.arm1.x_rot += 0.4;
            self.leg0.z = 4.0;
            self.leg1.z = 4.0;
            self.body.y = 0.0;
            self.arm0.y = 2.0;
            self.arm1.y = 2.0;
            self.leg0.y = 9.0;
            self.leg1.y = 9.0;
            self.head.y = 1.0;
            self.hair.y = 1.0;
        } else {
            self.body.x_rot = 0.0;
            self.leg0.z = 0.1;
            self.leg1.z = 0.1;
            self.leg0.y = 12.0;
            self.leg1.y = 12.0;
            self.body.y = 0.0;
            self.arm0.y = 2.0;
            self.arm1.y = 2.0;
            self.head.y = 0.0;
            self.hair.y = 0.0;
        }
        // idle breathing sway
        self.arm0.z_rot += (bob * 0.09).cos() * 0.05 + 0.05;
        self.arm1.z_rot -= (bob * 0.09).cos() * 0.05 + 0.05;
        self.arm0.x_rot += (bob * 0.067).sin() * 0.05;
        self.arm1.x_rot -= (bob * 0.067).sin() * 0.05;
        match self.kind {
            HumanoidKind::Zombie | HumanoidKind::Skeleton => {
                // SkeletonModel extends ZombieModel: same arms-forward pose
                let attack2 = (at * PI).sin();
                let attack = ((1.0 - (1.0 - at) * (1.0 - at)) * PI).sin();
                self.arm0.z_rot = 0.0;
                self.arm1.z_rot = 0.0;
                self.arm0.y_rot = -(0.1 - attack2 * 0.6);
                self.arm1.y_rot = 0.1 - attack2 * 0.6;
                self.arm0.x_rot = -HALF_PI;
                self.arm1.x_rot = -HALF_PI;
                self.arm0.x_rot -= attack2 * 1.2 - attack * 0.4;
                self.arm1.x_rot -= attack2 * 1.2 - attack * 0.4;
                self.arm0.z_rot += (bob * 0.09).cos() * 0.05 + 0.05;
                self.arm1.z_rot -= (bob * 0.09).cos() * 0.05 + 0.05;
                self.arm0.x_rot += (bob * 0.067).sin() * 0.05;
                self.arm1.x_rot -= (bob * 0.067).sin() * 0.05;
            }
            HumanoidKind::Player => {}
        }
    }

    /// `HumanoidModel::render` after `setupAnim`.
    pub fn render(&self, scale: f32, m: &Mat4, em: &mut Emitter) {
        if self.young {
            let ss = 2.0;
            let mh = *m * Mat4::from_scale(Vec3::splat(1.5 / ss)) * Mat4::from_translation(Vec3::new(0.0, 16.0 * scale, 0.0));
            self.head.render(scale, &mh, em, false);
            let mb = *m * Mat4::from_scale(Vec3::splat(1.0 / ss)) * Mat4::from_translation(Vec3::new(0.0, 24.0 * scale, 0.0));
            for p in [&self.body, &self.arm0, &self.arm1, &self.leg0, &self.leg1, &self.hair] {
                p.render(scale, &mb, em, false);
            }
        } else {
            self.head.render(scale, m, em, false);
            for p in [&self.body, &self.arm0, &self.arm1, &self.leg0, &self.leg1] {
                p.render(scale, m, em, false);
            }
            self.hair.render(scale, m, em, false);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Quadruped (pig / cow / sheep / sheep fur)
// ---------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum QuadKind {
    Pig,
    Cow,
    Sheep,
    SheepFur,
}

#[derive(Clone)]
pub struct QuadrupedModel {
    pub kind: QuadKind,
    pub head: ModelPart,
    pub body: ModelPart,
    pub leg: [ModelPart; 4],
    pub y_head_offs: f32,
    pub z_head_offs: f32,
    pub young: bool,
    /// sheep: `getHeadEatAngleScale`
    pub head_x_rot: f32,
}

impl QuadrupedModel {
    /// `QuadrupedModel(legSize, g)`
    fn base(leg_size: i32, g: f32) -> QuadrupedModel {
        let ls = leg_size as f32;
        let mut head = part(64, 32, 0, 0);
        head.add_box(-4.0, -4.0, -8.0, 8, 8, 8, g).set_pos(0.0, 12.0 + 6.0 - ls, -6.0);
        let mut body = part(64, 32, 28, 8);
        body.add_box(-5.0, -10.0, -7.0, 10, 16, 8, g).set_pos(0.0, 11.0 + 6.0 - ls, 2.0);
        let mk = |x: f32, z: f32| {
            let mut l = part(64, 32, 0, 16);
            l.add_box(-2.0, 0.0, -2.0, 4, leg_size, 4, g).set_pos(x, 18.0 + 6.0 - ls, z);
            l
        };
        QuadrupedModel { kind: QuadKind::Pig, head, body, leg: [mk(-3.0, 7.0), mk(3.0, 7.0), mk(-3.0, -5.0), mk(3.0, -5.0)], y_head_offs: 8.0, z_head_offs: 4.0, young: false, head_x_rot: 0.0 }
    }

    pub fn pig(grow: f32) -> QuadrupedModel {
        let mut m = QuadrupedModel::base(6, grow);
        m.kind = QuadKind::Pig;
        m.head.tex_offs(16, 16).add_box(-2.0, 0.0, -9.0, 4, 3, 1, grow);
        m.y_head_offs = 4.0;
        m
    }

    pub fn cow() -> QuadrupedModel {
        let mut m = QuadrupedModel::base(12, 0.0);
        m.kind = QuadKind::Cow;
        let mut head = part(64, 32, 0, 0);
        head.add_box(-4.0, -4.0, -6.0, 8, 8, 6, 0.0).set_pos(0.0, 12.0 - 6.0 - 2.0, -8.0);
        head.tex_offs(22, 0).add_box(-5.0, -5.0, -4.0, 1, 3, 1, 0.0);
        head.tex_offs(22, 0).add_box(4.0, -5.0, -4.0, 1, 3, 1, 0.0);
        m.head = head;
        let mut body = part(64, 32, 18, 4);
        body.add_box(-6.0, -10.0, -7.0, 12, 18, 10, 0.0).set_pos(0.0, 11.0 + 6.0 - 12.0, 2.0);
        body.tex_offs(52, 0).add_box(-2.0, 2.0, -8.0, 4, 6, 1, 0.0);
        m.body = body;
        m.leg[0].x -= 1.0;
        m.leg[1].x += 1.0;
        m.leg[2].x -= 1.0;
        m.leg[3].x += 1.0;
        m.leg[2].z -= 1.0;
        m.leg[3].z -= 1.0;
        m.z_head_offs += 2.0;
        m
    }

    pub fn sheep() -> QuadrupedModel {
        let mut m = QuadrupedModel::base(12, 0.0);
        m.kind = QuadKind::Sheep;
        let mut head = part(64, 32, 0, 0);
        head.add_box(-3.0, -4.0, -6.0, 6, 6, 8, 0.0).set_pos(0.0, 12.0 - 6.0, -8.0);
        m.head = head;
        let mut body = part(64, 32, 28, 8);
        body.add_box(-4.0, -10.0, -7.0, 8, 16, 6, 0.0).set_pos(0.0, 11.0 + 6.0 - 12.0, 2.0);
        m.body = body;
        m
    }

    pub fn sheep_fur() -> QuadrupedModel {
        let mut m = QuadrupedModel::base(12, 0.0);
        m.kind = QuadKind::SheepFur;
        let mut head = part(64, 32, 0, 0);
        head.add_box(-3.0, -4.0, -4.0, 6, 6, 6, 0.6).set_pos(0.0, 12.0 - 6.0, -8.0);
        m.head = head;
        let mut body = part(64, 32, 28, 8);
        body.add_box(-4.0, -10.0, -7.0, 8, 16, 6, 1.75).set_pos(0.0, 11.0 + 6.0 - 12.0, 2.0);
        m.body = body;
        let g = 0.5;
        let mk = |x: f32, z: f32| {
            let mut l = part(64, 32, 0, 16);
            l.add_box(-2.0, 0.0, -2.0, 4, 6, 4, g).set_pos(x, 18.0 + 6.0 - 12.0, z);
            l
        };
        m.leg = [mk(-3.0, 7.0), mk(3.0, 7.0), mk(-3.0, -5.0), mk(3.0, -5.0)];
        m
    }

    /// `QuadrupedModel::setupAnim` (+ sheep head override). `eat_y`: sheep `getHeadEatPositionScale`.
    pub fn setup_anim(&mut self, time: f32, r: f32, y_rot: f32, x_rot: f32, eat_y: f32, eat_x: f32) {
        self.head.x_rot = x_rot / RAD;
        self.head.y_rot = y_rot / RAD;
        self.body.x_rot = 90.0 / RAD;
        self.leg[0].x_rot = (time * 0.6662).cos() * 1.4 * r;
        self.leg[1].x_rot = (time * 0.6662 + PI).cos() * 1.4 * r;
        self.leg[2].x_rot = (time * 0.6662 + PI).cos() * 1.4 * r;
        self.leg[3].x_rot = (time * 0.6662).cos() * 1.4 * r;
        if matches!(self.kind, QuadKind::Sheep | QuadKind::SheepFur) {
            self.head.y = 6.0 + eat_y * 9.0;
            self.head_x_rot = eat_x;
            self.head.x_rot = eat_x;
        }
    }

    pub fn render(&self, scale: f32, m: &Mat4, em: &mut Emitter) {
        if self.young {
            let ss = 2.0;
            let mh = *m * Mat4::from_translation(Vec3::new(0.0, self.y_head_offs * scale, self.z_head_offs * scale));
            self.head.render(scale, &mh, em, false);
            let mb = *m * Mat4::from_scale(Vec3::splat(1.0 / ss)) * Mat4::from_translation(Vec3::new(0.0, 24.0 * scale, 0.0));
            self.body.render(scale, &mb, em, false);
            for l in &self.leg {
                l.render(scale, &mb, em, false);
            }
        } else {
            self.head.render(scale, m, em, false);
            self.body.render(scale, m, em, false);
            for l in &self.leg {
                l.render(scale, m, em, false);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Chicken
// ---------------------------------------------------------------------------------------------------------------

#[derive(Clone)]
pub struct ChickenModel {
    pub head: ModelPart,
    pub beak: ModelPart,
    pub red_thing: ModelPart,
    pub body: ModelPart,
    pub leg0: ModelPart,
    pub leg1: ModelPart,
    pub wing0: ModelPart,
    pub wing1: ModelPart,
    pub young: bool,
}

impl ChickenModel {
    pub fn new() -> ChickenModel {
        let yo = 16.0;
        let mut head = part(64, 32, 0, 0);
        head.add_box(-2.0, -6.0, -2.0, 4, 6, 3, 0.0).set_pos(0.0, -1.0 + yo, -4.0);
        let mut beak = part(64, 32, 14, 0);
        beak.add_box(-2.0, -4.0, -4.0, 4, 2, 2, 0.0).set_pos(0.0, -1.0 + yo, -4.0);
        let mut red_thing = part(64, 32, 14, 4);
        red_thing.add_box(-1.0, -2.0, -3.0, 2, 2, 2, 0.0).set_pos(0.0, -1.0 + yo, -4.0);
        let mut body = part(64, 32, 0, 9);
        body.add_box(-3.0, -4.0, -3.0, 6, 8, 6, 0.0).set_pos(0.0, yo, 0.0);
        let mut leg0 = part(64, 32, 26, 0);
        leg0.add_box(-1.0, 0.0, -3.0, 3, 5, 3, 0.0).set_pos(-2.0, 3.0 + yo, 1.0);
        let mut leg1 = part(64, 32, 26, 0);
        leg1.add_box(-1.0, 0.0, -3.0, 3, 5, 3, 0.0).set_pos(1.0, 3.0 + yo, 1.0);
        let mut wing0 = part(64, 32, 24, 13);
        wing0.add_box(0.0, 0.0, -3.0, 1, 4, 6, 0.0).set_pos(-4.0, -3.0 + yo, 0.0);
        let mut wing1 = part(64, 32, 24, 13);
        wing1.add_box(-1.0, 0.0, -3.0, 1, 4, 6, 0.0).set_pos(4.0, -3.0 + yo, 0.0);
        ChickenModel { head, beak, red_thing, body, leg0, leg1, wing0, wing1, young: false }
    }

    pub fn setup_anim(&mut self, time: f32, r: f32, bob: f32, y_rot: f32, x_rot: f32) {
        self.head.x_rot = x_rot / RAD;
        self.head.y_rot = y_rot / RAD;
        self.beak.x_rot = self.head.x_rot;
        self.beak.y_rot = self.head.y_rot;
        self.red_thing.x_rot = self.head.x_rot;
        self.red_thing.y_rot = self.head.y_rot;
        self.body.x_rot = 90.0 / RAD;
        self.leg0.x_rot = (time * 0.6662).cos() * 1.4 * r;
        self.leg1.x_rot = (time * 0.6662 + PI).cos() * 1.4 * r;
        self.wing0.z_rot = bob;
        self.wing1.z_rot = -bob;
    }

    pub fn render(&self, scale: f32, m: &Mat4, em: &mut Emitter) {
        if self.young {
            let ss = 2.0;
            let mh = *m * Mat4::from_translation(Vec3::new(0.0, 5.0 * scale, 2.0 * scale));
            for p in [&self.head, &self.beak, &self.red_thing] {
                p.render(scale, &mh, em, false);
            }
            let mb = *m * Mat4::from_scale(Vec3::splat(1.0 / ss)) * Mat4::from_translation(Vec3::new(0.0, 24.0 * scale, 0.0));
            for p in [&self.body, &self.leg0, &self.leg1, &self.wing0, &self.wing1] {
                p.render(scale, &mb, em, false);
            }
        } else {
            for p in [&self.head, &self.beak, &self.red_thing, &self.body, &self.leg0, &self.leg1, &self.wing0, &self.wing1] {
                p.render(scale, m, em, false);
            }
        }
    }
}

impl Default for ChickenModel {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Creeper
// ---------------------------------------------------------------------------------------------------------------

#[derive(Clone)]
pub struct CreeperModel {
    pub head: ModelPart,
    pub hair: ModelPart,
    pub body: ModelPart,
    pub leg: [ModelPart; 4],
}

impl CreeperModel {
    pub fn new(g: f32) -> CreeperModel {
        let mut head = part(64, 32, 0, 0);
        head.add_box(-4.0, -8.0, -4.0, 8, 8, 8, g).set_pos(0.0, 6.0, 0.0);
        let mut hair = part(64, 32, 32, 0);
        hair.add_box(-4.0, -8.0, -4.0, 8, 8, 8, g + 0.5).set_pos(0.0, 6.0, 0.0);
        let mut body = part(64, 32, 16, 16);
        body.add_box(-4.0, 0.0, -2.0, 8, 12, 4, g).set_pos(0.0, 6.0, 0.0);
        let mk = |x: f32, z: f32| {
            let mut l = part(64, 32, 0, 16);
            l.add_box(-2.0, 0.0, -2.0, 4, 6, 4, g).set_pos(x, 18.0, z);
            l
        };
        CreeperModel { head, hair, body, leg: [mk(-2.0, 4.0), mk(2.0, 4.0), mk(-2.0, -4.0), mk(2.0, -4.0)] }
    }

    pub fn setup_anim(&mut self, time: f32, r: f32, y_rot: f32, x_rot: f32) {
        self.head.y_rot = y_rot / RAD;
        self.head.x_rot = x_rot / RAD;
        self.leg[0].x_rot = (time * 0.6662).cos() * 1.4 * r;
        self.leg[1].x_rot = (time * 0.6662 + PI).cos() * 1.4 * r;
        self.leg[2].x_rot = (time * 0.6662 + PI).cos() * 1.4 * r;
        self.leg[3].x_rot = (time * 0.6662).cos() * 1.4 * r;
    }

    pub fn render(&self, scale: f32, m: &Mat4, em: &mut Emitter) {
        self.head.render(scale, m, em, false);
        self.body.render(scale, m, em, false);
        for l in &self.leg {
            l.render(scale, m, em, false);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Spider (`SpiderModel`)
// ---------------------------------------------------------------------------------------------------------------

#[derive(Clone)]
pub struct SpiderModel {
    pub head: ModelPart,
    pub body0: ModelPart,
    pub body1: ModelPart,
    pub leg: [ModelPart; 8],
}

impl SpiderModel {
    pub fn new() -> SpiderModel {
        let g = 0.0;
        let yo = (18 + 6 - 9) as f32;
        let mut head = part(64, 32, 32, 4);
        head.add_box(-4.0, -4.0, -8.0, 8, 8, 8, g).set_pos(0.0, yo, -3.0);
        let mut body0 = part(64, 32, 0, 0);
        body0.add_box(-3.0, -3.0, -3.0, 6, 6, 6, g).set_pos(0.0, yo, 0.0);
        let mut body1 = part(64, 32, 0, 12);
        body1.add_box(-5.0, -4.0, -6.0, 10, 8, 12, g).set_pos(0.0, yo, 3.0 + 6.0);
        // legs 0, 2, 4, 6 are the left ones (box from -15 to 1), 1, 3, 5, 7 the right ones (-1 to 15)
        let mk = |left: bool, x: f32, z: f32| {
            let mut l = part(64, 32, 18, 0);
            l.add_box(if left { -15.0 } else { -1.0 }, -1.0, -1.0, 16, 2, 2, g).set_pos(x, yo, z);
            l
        };
        let leg = [mk(true, -4.0, 2.0), mk(false, 4.0, 2.0), mk(true, -4.0, 1.0), mk(false, 4.0, 1.0), mk(true, -4.0, 0.0), mk(false, 4.0, 0.0), mk(true, -4.0, -1.0), mk(false, 4.0, -1.0)];
        SpiderModel { head, body0, body1, leg }
    }

    pub fn setup_anim(&mut self, time: f32, r: f32, y_rot: f32, x_rot: f32) {
        self.head.y_rot = y_rot / RAD;
        self.head.x_rot = x_rot / RAD;
        let sr = PI / 4.0;
        let z = [-sr, sr, -sr * 0.74, sr * 0.74, -sr * 0.74, sr * 0.74, -sr, sr];
        let ur = PI / 8.0;
        let ro = 0.0;
        let y = [ur * 2.0 + ro, -ur * 2.0 - ro, ur + ro, -ur - ro, -ur + ro, ur - ro, -ur * 2.0 + ro, ur * 2.0 - ro];
        for i in 0..8 {
            self.leg[i].z_rot = z[i];
            self.leg[i].y_rot = y[i];
        }
        let c = |k: f32| -((time * 0.6662 * 2.0 + PI * 2.0 * k / 4.0).cos() * 0.4) * r;
        let sn = |k: f32| ((time * 0.6662 + PI * 2.0 * k / 4.0).sin() * 0.4).abs() * r;
        let (c0, c1, c2, c3) = (c(0.0), c(2.0), c(1.0), c(3.0));
        let (s0, s1, s2, s3) = (sn(0.0), sn(2.0), sn(1.0), sn(3.0));
        self.leg[0].y_rot += c0;
        self.leg[1].y_rot += -c0;
        self.leg[2].y_rot += c1;
        self.leg[3].y_rot += -c1;
        self.leg[4].y_rot += c2;
        self.leg[5].y_rot += -c2;
        self.leg[6].y_rot += c3;
        self.leg[7].y_rot += -c3;
        self.leg[0].z_rot += s0;
        self.leg[1].z_rot += -s0;
        self.leg[2].z_rot += s1;
        self.leg[3].z_rot += -s1;
        self.leg[4].z_rot += s2;
        self.leg[5].z_rot += -s2;
        self.leg[6].z_rot += s3;
        self.leg[7].z_rot += -s3;
    }

    pub fn render(&self, scale: f32, m: &Mat4, em: &mut Emitter) {
        self.head.render(scale, m, em, false);
        self.body0.render(scale, m, em, false);
        self.body1.render(scale, m, em, false);
        for l in &self.leg {
            l.render(scale, m, em, false);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Slime (`SlimeModel`) and lava slime (`LavaSlimeModel`)
// ---------------------------------------------------------------------------------------------------------------

/// `SlimeModel(vOffs)`: `vOffs = 16` is the inner body with the face, `vOffs = 0` the big translucent shell.
pub struct SlimeModel {
    pub cube: ModelPart,
    pub face: Option<[ModelPart; 3]>,
}

impl SlimeModel {
    pub fn new(v_offs: i32) -> SlimeModel {
        let g = 0.0;
        let mut cube = part(64, 32, 0, v_offs);
        cube.add_box(-4.0, 16.0, -4.0, 8, 8, 8, g);
        if v_offs > 0 {
            cube = part(64, 32, 0, v_offs);
            cube.add_box(-3.0, 16.0 + 1.0, -3.0, 6, 6, 6, g);
            let mut eye0 = part(64, 32, 32, 0);
            eye0.add_box(-3.0 - 0.25, 16.0 + 2.0, -3.5, 2, 2, 2, g);
            let mut eye1 = part(64, 32, 32, 4);
            eye1.add_box(1.0 + 0.25, 16.0 + 2.0, -3.5, 2, 2, 2, g);
            let mut mouth = part(64, 32, 32, 8);
            mouth.add_box(0.0, 16.0 + 5.0, -3.5, 1, 1, 1, g);
            SlimeModel { cube, face: Some([eye0, eye1, mouth]) }
        } else {
            SlimeModel { cube, face: None }
        }
    }

    pub fn render(&self, scale: f32, m: &Mat4, em: &mut Emitter) {
        self.cube.render(scale, m, em, false);
        if let Some(f) = &self.face {
            for p in f {
                p.render(scale, m, em, false);
            }
        }
    }
}

pub const LAVA_BODY_CUBES: usize = 8;

/// `LavaSlimeModel`: eight 1-high slices that spread apart with the squish, and the core
pub struct LavaSlimeModel {
    pub body: Vec<ModelPart>,
    pub inside: ModelPart,
}

impl LavaSlimeModel {
    pub fn new() -> LavaSlimeModel {
        let mut body = Vec::new();
        for i in 0..LAVA_BODY_CUBES {
            let (mut u, mut v) = (0, i as i32);
            if i == 2 {
                u = 24;
                v = 10;
            } else if i == 3 {
                u = 24;
                v = 19;
            }
            let mut p = part(64, 32, u, v);
            p.add_box(-4.0, 16.0 + i as f32, -4.0, 8, 1, 8, 0.0);
            body.push(p);
        }
        let mut inside = part(64, 32, 0, 16);
        inside.add_box(-2.0, 16.0 + 2.0, -2.0, 4, 4, 4, 0.0);
        LavaSlimeModel { body, inside }
    }

    /// `LavaSlimeModel::prepareMobModel`
    pub fn prepare(&mut self, squish: f32) {
        let s = squish.max(0.0);
        for (i, p) in self.body.iter_mut().enumerate() {
            p.y = -(4.0 - i as f32) * s * 1.7;
        }
    }

    pub fn render(&self, scale: f32, m: &Mat4, em: &mut Emitter) {
        self.inside.render(scale, m, em, false);
        for p in &self.body {
            p.render(scale, m, em, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn polys(p: &ModelPart) -> usize {
        p.cubes.iter().map(|c| c.polys.len()).sum()
    }

    #[test]
    fn cube_has_six_faces_and_uv_in_range() {
        let c = Cube::new(false, 64.0, 32.0, 0, 0, -4.0, -8.0, -4.0, 8, 8, 8, 0.0, 63, false);
        assert_eq!(c.polys.len(), 6);
        for p in &c.polys {
            for (_, uv) in &p.v {
                assert!((0.0..=1.0).contains(&uv[0]) && (0.0..=1.0).contains(&uv[1]), "{uv:?}");
            }
        }
    }

    #[test]
    fn head_front_face_uv() {
        // front face = polygon index 4: u = (d .. d+w)/64 = 8..16, v = (d .. d+h)/32 = 8..16
        let c = Cube::new(false, 64.0, 32.0, 0, 0, -4.0, -8.0, -4.0, 8, 8, 8, 0.0, 63, false);
        let f = &c.polys[4];
        assert!((f.v[0].1[0] - (16.0 / 64.0 - 0.1 / 64.0)).abs() < 1e-6);
        assert!((f.v[1].1[0] - (8.0 / 64.0 + 0.1 / 64.0)).abs() < 1e-6);
        assert!((f.v[0].1[1] - (8.0 / 32.0 + 0.1 / 32.0)).abs() < 1e-6);
    }

    #[test]
    fn pig_parts() {
        let m = QuadrupedModel::pig(0.0);
        assert_eq!(polys(&m.head), 12); // head + snout
        assert_eq!(polys(&m.body), 6);
        assert_eq!(m.y_head_offs, 4.0);
    }

    #[test]
    fn humanoid_renders_all_parts() {
        let mut m = HumanoidModel::player();
        m.setup_anim(0.0, 0.0, 0.0, 0.0, 0.0);
        let mut em = Emitter::new();
        m.render(1.0 / 16.0, &Mat4::IDENTITY, &mut em);
        // head, body, 2 arms, 2 legs, hair = 7 boxes × 6 faces × 4 vertices
        assert_eq!(em.mesh.vertices.len(), 7 * 6 * 4);
        // model height ≈ 32 px / 16 = 2 blocks (feet at y = 24/16 below the origin)
        let (min, max) = em.mesh.vertices.iter().fold((f32::MAX, f32::MIN), |a, v| (a.0.min(v.pos[1]), a.1.max(v.pos[1])));
        assert!((max - min - 2.0).abs() < 0.1, "{min} {max}");
    }

    #[test]
    fn mirror_flips_x() {
        let mut p = part(64, 32, 0, 0);
        p.mirror = true;
        p.add_box(0.0, 0.0, 0.0, 2, 2, 2, 0.0);
        // mirrored cubes swap x0/x1 → the first vertex of the polygon list carries x = 2 in the "left" slot
        let xs: Vec<f32> = p.cubes[0].polys.iter().flat_map(|q| q.v.iter().map(|v| v.0[0])).collect();
        assert!(xs.contains(&0.0) && xs.contains(&2.0));
    }

    #[test]
    fn creeper_and_chicken_build() {
        let mut c = CreeperModel::new(0.0);
        c.setup_anim(1.0, 1.0, 0.0, 0.0);
        let mut em = Emitter::new();
        c.render(1.0 / 16.0, &Mat4::IDENTITY, &mut em);
        assert_eq!(em.mesh.vertices.len(), 6 * 6 * 4);
        let mut ch = ChickenModel::new();
        ch.setup_anim(0.0, 0.0, 0.0, 0.0, 0.0);
        let mut em = Emitter::new();
        ch.render(1.0 / 16.0, &Mat4::IDENTITY, &mut em);
        assert_eq!(em.mesh.vertices.len(), 8 * 6 * 4);
    }
}
