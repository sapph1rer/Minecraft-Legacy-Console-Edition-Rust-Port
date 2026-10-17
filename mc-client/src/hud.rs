//! 2-D overlay geometry: crosshair and hotbar (block icons come from the terrain atlas).

use crate::blocks::{style, Tint};
use crate::mesh::{tint_colour, uv_of};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HudVertex {
    pub pos: [f32; 2],
    /// `uv.x < 0` = solid colour
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

pub struct HudBuilder {
    pub w: f32,
    pub h: f32,
    pub verts: Vec<HudVertex>,
}

impl HudBuilder {
    pub fn new(w: u32, h: u32) -> HudBuilder {
        HudBuilder { w: w as f32, h: h as f32, verts: Vec::new() }
    }
    fn ndc(&self, x: f32, y: f32) -> [f32; 2] {
        [x / self.w * 2.0 - 1.0, 1.0 - y / self.h * 2.0]
    }
    /// Pixel rectangle (origin top-left), solid colour.
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.quad(x, y, w, h, [[-1.0, 0.0]; 4], color);
    }
    fn quad(&mut self, x: f32, y: f32, w: f32, h: f32, uv: [[f32; 2]; 4], color: [f32; 4]) {
        // corners: TL, TR, BR, BL
        let p = [self.ndc(x, y), self.ndc(x + w, y), self.ndc(x + w, y + h), self.ndc(x, y + h)];
        for k in [0, 1, 2, 0, 2, 3] {
            self.verts.push(HudVertex { pos: p[k], uv: uv[k], color });
        }
    }
    /// A terrain-atlas cell, tinted.
    pub fn icon(&mut self, x: f32, y: f32, size: f32, cell: (u8, u8), rgb: [f32; 3]) {
        // uv_of corners: 0 = BL, 1 = BR, 2 = TR, 3 = TL
        let uv = [uv_of(cell, 3), uv_of(cell, 2), uv_of(cell, 1), uv_of(cell, 0)];
        self.quad(x, y, size, size, uv, [rgb[0], rgb[1], rgb[2], 1.0]);
    }

    pub fn crosshair(&mut self) {
        let ui = self.ui();
        let (cx, cy) = (self.w / 2.0, self.h / 2.0);
        let (len, th) = (5.0 * ui, 1.0 * ui);
        let c = [1.0, 1.0, 1.0, 0.85];
        self.rect(cx - len, cy - th / 2.0, 2.0 * len, th, c);
        self.rect(cx - th / 2.0, cy - len, th, 2.0 * len, c);
    }

    fn ui(&self) -> f32 {
        (self.h / 270.0).floor().max(1.0)
    }

    /// `slots` are `(block id, data)`; `selected` is highlighted.
    pub fn hotbar(&mut self, slots: &[(u8, u8)], selected: usize) {
        let ui = self.ui();
        let slot = 20.0 * ui;
        let total = slot * slots.len() as f32;
        let x0 = (self.w - total) / 2.0;
        let y0 = self.h - slot - 4.0 * ui;
        self.rect(x0 - ui, y0 - ui, total + 2.0 * ui, slot + 2.0 * ui, [0.0, 0.0, 0.0, 0.55]);
        for (i, &(id, data)) in slots.iter().enumerate() {
            let x = x0 + i as f32 * slot;
            if i == selected {
                self.rect(x - ui, y0 - ui, slot + 2.0 * ui, slot + 2.0 * ui, [1.0, 1.0, 1.0, 0.9]);
                self.rect(x + ui, y0 + ui, slot - 2.0 * ui, slot - 2.0 * ui, [0.1, 0.1, 0.1, 0.9]);
            } else {
                self.rect(x + ui, y0 + ui, slot - 2.0 * ui, slot - 2.0 * ui, [0.25, 0.25, 0.25, 0.6]);
            }
            let s = style(id, data);
            // grass shows its top, everything else its side
            let (cell, tint) = if s.tint[3] == Tint::Grass { (s.cells[3], s.tint[3]) } else { (s.cells[5], s.tint[5]) };
            self.icon(x + 3.0 * ui, y0 + 3.0 * ui, slot - 6.0 * ui, cell, tint_colour(tint, 0.8));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotbar_geometry_is_inside_the_screen() {
        let mut b = HudBuilder::new(1280, 720);
        b.crosshair();
        b.hotbar(&[(1, 0), (2, 0), (3, 0)], 1);
        assert!(!b.verts.is_empty() && b.verts.len() % 6 == 0);
        assert!(b.verts.iter().all(|v| v.pos[0].abs() <= 1.0 && v.pos[1].abs() <= 1.0));
    }
}
