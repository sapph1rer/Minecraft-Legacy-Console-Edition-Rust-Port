//! Port of `ItemInHandRenderer` (first-person hand / held item): equip animation state and the eye-space geometry.

use crate::itemmesh::{block_3d, renders_as_block, sprite_3d, Alpha};
use crate::mesh::MeshData;
use crate::model::{Emitter, HumanoidModel, PI};
use crate::render::EntTex;
use glam::{Mat4, Vec3};
use mc_world::item::{item_def, ItemKind, ItemStack};

#[derive(Clone, Debug, Default)]
pub struct HandState {
    pub height: f32,
    pub o_height: f32,
    selected: Option<(u16, u16)>,
    last_slot: i32,
    /// `LocalPlayer::xBob` / `yBob` (smoothed view angles) and their previous values
    pub x_bob: f32,
    pub y_bob: f32,
    pub x_bob_o: f32,
    pub y_bob_o: f32,
    /// `Player::attackAnim` / `oAttackAnim`
    pub attack_anim: f32,
    pub o_attack_anim: f32,
}

impl HandState {
    pub fn new() -> HandState {
        HandState { last_slot: -1, ..Default::default() }
    }

    /// `ItemInHandRenderer::tick` + the `LocalPlayer` bob / swing bookkeeping. `attack_anim` = `Player::attackAnim`.
    pub fn tick(&mut self, sel: Option<&ItemStack>, slot: usize, attack_anim: f32, pitch: f32, yaw: f32) {
        self.o_height = self.height;
        let next = sel.map(|s| (s.id, s.damage));
        let matches = (self.last_slot == slot as i32 && next == self.selected) || (self.selected.is_none() && next.is_none());
        let max = 0.4;
        let target = if matches { 1.0 } else { 0.0 };
        let dd = (target - self.height).clamp(-max, max);
        self.height += dd;
        if self.height < 0.1 {
            self.selected = next;
            self.last_slot = slot as i32;
        }
        self.x_bob_o = self.x_bob;
        self.y_bob_o = self.y_bob;
        self.x_bob += (pitch - self.x_bob) * 0.5;
        self.y_bob += (yaw - self.y_bob) * 0.5;
        self.o_attack_anim = self.attack_anim;
        self.attack_anim = attack_anim;
    }

    /// `itemPlaced` / `itemUsed`: lower the hand and bring it back up.
    pub fn item_used(&mut self) {
        self.height = 0.0;
    }

    pub fn reset(&mut self) {
        *self = HandState::new();
    }

    /// The stack currently shown (by id/damage), `None` = empty hand.
    pub fn shown(&self) -> Option<(u16, u16)> {
        self.selected
    }
}

pub struct HandParams<'a> {
    pub hand: &'a HandState,
    pub partial: f32,
    pub pitch: f32,
    pub yaw: f32,
    /// (sky, block) light at the player
    pub light: [f32; 2],
    pub items: &'a Alpha,
    /// eating / drinking: (ticks left, total duration)
    pub eating: Option<(f32, f32)>,
    pub invisible: bool,
}

/// Geometry of the first-person hand in eye space (camera at the origin looking down -z).
pub fn build_hand(p: &HandParams) -> Vec<(EntTex, MeshData)> {
    let a = p.partial;
    let hs = p.hand;
    let h = hs.o_height + (hs.height - hs.o_height) * a;
    let xr = p.pitch;
    let yr = p.yaw;
    let xrr = hs.x_bob_o + (hs.x_bob - hs.x_bob_o) * a;
    let yrr = hs.y_bob_o + (hs.y_bob - hs.y_bob_o) * a;
    let mut sd = hs.attack_anim - hs.o_attack_anim;
    if sd < 0.0 {
        sd += 1.0; // LivingEntity::getAttackAnim wraps around
    }
    let swing = hs.o_attack_anim + sd * a;
    let rot = |deg: f32, axis: Vec3| Mat4::from_axis_angle(axis, deg.to_radians());
    let mut m = rot((xr - xrr) * 0.1, Vec3::X) * rot((yr - yrr) * 0.1, Vec3::Y);

    // Lighting::turnOn under rotate(xr, x) · rotate(yr, y): lights live in that frame
    let light_frame = rot(xr, Vec3::X) * rot(yr, Vec3::Y);
    let mut em = Emitter::new();
    em.normal_xf = light_frame.inverse();
    em.light = p.light;

    let d = 0.8;
    let mut out: Vec<(EntTex, MeshData)> = Vec::new();
    match hs.shown() {
        Some((id, damage)) => {
            let st = ItemStack { id, count: 1, damage };
            if let Some((left, dur)) = p.eating {
                let t = left - a + 1.0;
                let sw = 1.0 - t / dur;
                let mut is = 1.0 - sw;
                is = is * is * is;
                is = is * is * is;
                is = is * is * is;
                let iss = 1.0 - is;
                m *= Mat4::from_translation(Vec3::new(0.0, (t / 4.0 * PI).cos().abs() * 0.1 * if sw > 0.2 { 1.0 } else { 0.0 }, 0.0));
                m *= Mat4::from_translation(Vec3::new(iss * 0.6, -iss * 0.5, 0.0));
                m *= rot(iss * 90.0, Vec3::Y) * rot(iss * 10.0, Vec3::X) * rot(iss * 30.0, Vec3::Z);
            } else {
                let swing1 = (swing * PI).sin();
                let swing2 = (swing.sqrt() * PI).sin();
                m *= Mat4::from_translation(Vec3::new(-swing2 * 0.4, (swing.sqrt() * PI * 2.0).sin() * 0.2, -swing1 * 0.2));
            }
            m *= Mat4::from_translation(Vec3::new(0.7 * d, -0.65 * d - (1.0 - h) * 0.6, -0.9 * d));
            m *= rot(45.0, Vec3::Y);
            let swing3 = (swing * swing * PI).sin();
            let swing2 = (swing.sqrt() * PI).sin();
            m *= rot(-swing3 * 20.0, Vec3::Y) * rot(-swing2 * 20.0, Vec3::Z) * rot(-swing2 * 80.0, Vec3::X);
            m *= Mat4::from_scale(Vec3::splat(0.4));
            if renders_as_block(id, damage as u8) {
                block_3d(&mut em, id as u8, damage as u8, &m);
                out.push((EntTex::Terrain, std::mem::take(&mut em.mesh)));
            } else if let Some(def) = item_def(st.id) {
                let (row, col) = def.icon_pos;
                let mut mi = m;
                mi *= Mat4::from_translation(Vec3::new(0.0, -0.3, 0.0));
                mi *= Mat4::from_scale(Vec3::splat(1.5));
                mi *= rot(50.0, Vec3::Y) * rot(45.0 + 290.0, Vec3::Z);
                mi *= Mat4::from_translation(Vec3::new(-15.0 / 16.0, -1.0 / 16.0, 0.0));
                sprite_3d(&mut em, p.items, col as u32 * 16, row as u32 * 16, &mi);
                out.push((EntTex::Items, std::mem::take(&mut em.mesh)));
            }
        }
        None if !p.invisible => {
            let swing1 = (swing * PI).sin();
            let swing2 = (swing.sqrt() * PI).sin();
            m *= Mat4::from_translation(Vec3::new(-swing2 * 0.3, (swing.sqrt() * PI * 2.0).sin() * 0.4, -swing1 * 0.4));
            m *= Mat4::from_translation(Vec3::new(0.8 * d, -0.75 * d - (1.0 - h) * 0.6, -0.9 * d));
            m *= rot(45.0, Vec3::Y);
            let swing3 = (swing * swing * PI).sin();
            m *= rot(swing2 * 70.0, Vec3::Y) * rot(-swing3 * 20.0, Vec3::Z);
            m *= Mat4::from_translation(Vec3::new(-1.0, 3.6, 3.5));
            m *= rot(120.0, Vec3::Z) * rot(180.0 + 20.0, Vec3::X) * rot(-90.0 - 45.0, Vec3::Y);
            m *= Mat4::from_scale(Vec3::splat(1.5 / 24.0 * 16.0));
            m *= Mat4::from_translation(Vec3::new(5.6, 0.0, 0.0));
            // PlayerRenderer::renderHand: setupAnim(0, 0, 0, 0, 0) with attackTime = 0, then arm0 only
            let mut model = HumanoidModel::player();
            model.attack_time = 0.0;
            model.setup_anim(0.0, 0.0, 0.0, 0.0, 0.0);
            model.arm0.render(1.0 / 16.0, &m, &mut em, false);
            out.push((EntTex::Char, std::mem::take(&mut em.mesh)));
        }
        None => {}
    }
    out.retain(|(_, mesh)| !mesh.is_empty());
    out
}

/// `item.getUseAnimation() == eat/drink` – does the held stack show the eating motion?
pub fn is_food(st: &ItemStack) -> bool {
    matches!(st.kind(), ItemKind::Food { .. })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha() -> Alpha {
        Alpha { w: 256, h: 256, a: vec![255; 256 * 256] }
    }

    #[test]
    fn equip_animation_lowers_and_raises() {
        let mut h = HandState::new();
        let sword = ItemStack::new(mc_world::item::ids::SWORD_IRON, 1);
        // first tick: nothing shown yet → switches while the hand is still low
        h.tick(Some(&sword), 0, 0.0, 0.0, 0.0);
        for _ in 0..4 {
            h.tick(Some(&sword), 0, 0.0, 0.0, 0.0);
        }
        assert!((h.height - 1.0).abs() < 1e-5, "{}", h.height);
        assert_eq!(h.shown(), Some((sword.id, 0)));
        // switching slot drops the hand, swaps the item at the bottom, raises again
        h.tick(None, 1, 0.0, 0.0, 0.0);
        assert!(h.height < 1.0);
        for _ in 0..8 {
            h.tick(None, 1, 0.0, 0.0, 0.0);
        }
        assert!(h.shown().is_none());
        assert!((h.height - 1.0).abs() < 1e-5);
    }

    #[test]
    fn attack_anim_follows_swing_ticks() {
        let mut h = HandState::new();
        h.tick(None, 0, 0.0, 0.0, 0.0);
        assert_eq!(h.attack_anim, 0.0);
        h.tick(None, 0, 0.5, 0.0, 0.0);
        assert!((h.attack_anim - 0.5).abs() < 1e-6);
    }

    #[test]
    fn hand_geometry_is_in_front_of_the_camera() {
        let mut hs = HandState::new();
        let sword = ItemStack::new(mc_world::item::ids::SWORD_IRON, 1);
        for _ in 0..6 {
            hs.tick(Some(&sword), 0, 0.0, 0.0, 0.0);
        }
        let al = alpha();
        let p = HandParams { hand: &hs, partial: 1.0, pitch: 0.0, yaw: 0.0, light: [15.0, 0.0], items: &al, eating: None, invisible: false };
        let layers = build_hand(&p);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].0, EntTex::Items);
        for v in &layers[0].1.vertices {
            assert!(v.pos[2] < 0.0, "behind camera: {:?}", v.pos);
        }
        // empty hand → the arm
        let mut hs = HandState::new();
        for _ in 0..6 {
            hs.tick(None, 0, 0.0, 0.0, 0.0);
        }
        let p = HandParams { hand: &hs, partial: 1.0, pitch: 0.0, yaw: 0.0, light: [15.0, 0.0], items: &al, eating: None, invisible: false };
        let layers = build_hand(&p);
        assert_eq!(layers[0].0, EntTex::Char);
        assert_eq!(layers[0].1.vertices.len(), 24);
    }
}
