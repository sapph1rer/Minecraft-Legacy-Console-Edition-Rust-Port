//! Light queries used by the renderer (`Level::getBrightnessPropagate`, `Level::getLightColor`) and a
//! one-shot "light everything" pass for freshly generated worlds.

use crate::chunk::LightLayer;
use crate::level::Level;
use mc_world::tile_props;
use mc_world::MAX_BUILD_HEIGHT;

impl Level {
    /// `Level::getBrightnessPropagate(layer, x, y, z, tileId)`; `tile_id < 0` means "look it up".
    pub fn get_brightness_propagate(&self, layer: LightLayer, x: i32, y: i32, z: i32, tile_id: i32) -> i32 {
        if self.has_ceiling && layer == LightLayer::Sky {
            return 0;
        }
        let y = y.max(0);
        if y >= MAX_BUILD_HEIGHT && layer == LightLayer::Sky {
            return layer.surrounding();
        }
        if self.chunk(x >> 4, z >> 4).is_none() {
            return layer.surrounding();
        }
        let id = if tile_id > -1 { tile_id as u8 } else { self.get_tile(x, y, z) };
        if tile_props::propagates(id) {
            let mut br = self.get_brightness(layer, x, y + 1, z);
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                br = br.max(self.get_brightness(layer, x + dx, y, z + dz));
            }
            return br;
        }
        self.get_brightness(layer, x, y, z)
    }

    /// `Level::getLightColor(x, y, z, emitt)`: `(sky, block)` light levels, block raised to `emitt`.
    pub fn get_light_color(&self, x: i32, y: i32, z: i32, emitt: i32, tile_id: i32) -> (i32, i32) {
        let s = self.get_brightness_propagate(LightLayer::Sky, x, y, z, tile_id);
        let mut b = self.get_brightness_propagate(LightLayer::Block, x, y, z, tile_id);
        if b < emitt {
            b = emitt;
        }
        (s, b)
    }

    /// `Tile::getLightColor(level, x, y, z)`: uses the emission of the tile standing at x/y/z.
    pub fn tile_light_color(&self, x: i32, y: i32, z: i32) -> (i32, i32) {
        let id = self.get_tile(x, y, z);
        self.get_light_color(x, y, z, tile_props::light_emission(id) as i32, -1)
    }

    /// Light a whole freshly generated world: re-check every sky gap, then every light emitter.
    /// (The generators run with `lighting = false` for speed; the real game lights while it places blocks.)
    pub fn light_chunk(&mut self, cx: i32, cz: i32) {
        if !self.has_ceiling {
            self.recheck_gaps(cx, cz, true);
        }
        self.light_chunk_emitters(cx, cz);
    }

    fn light_chunk_emitters(&mut self, cx: i32, cz: i32) {
        let top = self.chunk(cx, cz).map_or(0, |c| c.highest_non_empty_y().clamp(0, MAX_BUILD_HEIGHT - 1));
        for lx in 0..16 {
            for lz in 0..16 {
                for y in 0..=top {
                    let id = self.chunk(cx, cz).map_or(0, |c| c.get_tile(lx, y, lz));
                    if id != 0 && tile_props::light_emission(id) > 0 {
                        self.check_light_layer(LightLayer::Block, cx * 16 + lx, y, cz * 16 + lz, true, false);
                    }
                }
            }
        }
    }

    pub fn light_all(&mut self) {
        let coords: Vec<(i32, i32)> = self.chunk_coords().collect();
        if !self.has_ceiling {
            for &(cx, cz) in &coords {
                self.recheck_gaps(cx, cz, true);
            }
        }
        for &(cx, cz) in &coords {
            let top = self.chunk(cx, cz).map_or(0, |c| c.highest_non_empty_y().clamp(0, MAX_BUILD_HEIGHT - 1));
            for lx in 0..16 {
                for lz in 0..16 {
                    for y in 0..=top {
                        let id = self.chunk(cx, cz).map_or(0, |c| c.get_tile(lx, y, lz));
                        if id != 0 && tile_props::light_emission(id) > 0 {
                            self.check_light_layer(LightLayer::Block, cx * 16 + lx, y, cz * 16 + lz, true, false);
                        }
                    }
                }
            }
        }
    }
}
