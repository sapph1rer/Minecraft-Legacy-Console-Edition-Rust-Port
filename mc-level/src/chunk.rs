//! Port of `LevelChunk` — the storage half. Everything that needs to look at *other* chunks
//! (light propagation into neighbours, `lightGap`, `recalcHeight`'s follow-up work) lives in
//! [`crate::level::Level`], because a chunk cannot borrow its own level in Rust.

use crate::storage::{Nibbles, Section, SECTION_HEIGHT, SECTION_TILES};
use mc_world::tile_props;
use mc_world::{MAX_BRIGHTNESS, MAX_BUILD_HEIGHT};

/// `LightLayer::variety`. The numeric value doubles as the brightness reported for chunks that are
/// not loaded ("surrounding" in the Java enum): fully lit sky, dark block light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LightLayer {
    Sky = 15,
    Block = 0,
}

impl LightLayer {
    #[inline]
    pub fn surrounding(self) -> i32 {
        self as i32
    }
}

// `LevelChunk::EColumnFlag`
pub const COLUMN_RECHECK: u8 = 1;
pub const COLUMN_BIOME_OK: u8 = 2;
pub const COLUMN_BIOME_HAS_SNOW: u8 = 4;
pub const COLUMN_BIOME_HAS_RAIN: u8 = 8;

// `LevelChunk::sTerrainPopulated*`
pub const TERRAIN_POPULATED_FROM_HERE: u16 = 2;
pub const TERRAIN_POPULATED_FROM_W: u16 = 4;
pub const TERRAIN_POPULATED_FROM_S: u16 = 8;
pub const TERRAIN_POPULATED_FROM_SW: u16 = 16;
/// All the post-processing that can actually place tiles in this chunk is complete.
pub const TERRAIN_POPULATED_ALL_AFFECTING: u16 = 30;
pub const TERRAIN_POPULATED_FROM_NW: u16 = 32;
pub const TERRAIN_POPULATED_FROM_N: u16 = 64;
pub const TERRAIN_POPULATED_FROM_NE: u16 = 128;
pub const TERRAIN_POPULATED_FROM_E: u16 = 256;
pub const TERRAIN_POPULATED_FROM_SE: u16 = 512;
pub const TERRAIN_POPULATED_ALL_NEIGHBOURS: u16 = 1022;
pub const TERRAIN_POST_POST_PROCESSED: u16 = 1024;

#[derive(Clone, Debug)]
pub struct LevelChunk {
    pub x: i32,
    pub z: i32,
    lower: Section,
    upper: Section,
    /// `heightmap[z << 4 | x]`: y of the first tile above the highest light-blocking tile.
    pub heightmap: [u8; 256],
    /// `rainHeights[x | z << 4]`, 255 = needs recalculating.
    pub rain_heights: [u8; 256],
    /// Two 4-bit flag sets per byte, `columnFlags[(x >> 1) | z * 8]`.
    pub column_flags: [u8; 128],
    /// Biome id per column (`biomes[z << 4 | x]`), 0xff = not looked up yet.
    pub biomes: [u8; 256],
    pub min_height: i32,
    pub lowest_heightmap: i32,
    pub has_gaps_to_check: bool,
    pub terrain_populated: u16,
    pub loaded: bool,
    pub unsaved: bool,
    pub dont_save: bool,
    pub emissive_added: bool,
    pub last_save_time: i64,
    pub inhabited_time: i64,
    /// True for the shared "outside the world" chunk (`EmptyLevelChunk`).
    pub is_empty_chunk: bool,
}

#[inline]
fn col(x: i32, z: i32) -> usize {
    debug_assert!((0..16).contains(&x) && (0..16).contains(&z));
    ((z << 4) | x) as usize
}

impl LevelChunk {
    fn blank(x: i32, z: i32) -> LevelChunk {
        LevelChunk {
            x,
            z,
            lower: Section::empty_lower(),
            upper: Section::empty_upper(),
            heightmap: [0; 256],
            rain_heights: [255; 256],
            column_flags: [0; 128],
            biomes: [0xff; 256],
            min_height: 0,
            lowest_heightmap: 256,
            has_gaps_to_check: false,
            terrain_populated: 0,
            loaded: false,
            unsaved: false,
            dont_save: false,
            emissive_added: true,
            last_save_time: 0,
            inhabited_time: 0,
            is_empty_chunk: false,
        }
    }

    /// An all-air chunk (`LevelChunk(level, x, z)`).
    pub fn empty(x: i32, z: i32) -> LevelChunk {
        LevelChunk::blank(x, z)
    }

    /// The chunk returned for coordinates outside the world: all air, never saved, already
    /// "fully post-processed" so nothing tries to decorate it.
    pub fn new_empty_chunk() -> LevelChunk {
        let mut c = LevelChunk::blank(0, 0);
        c.dont_save = true;
        c.is_empty_chunk = true;
        c.terrain_populated = TERRAIN_POPULATED_ALL_NEIGHBOURS | TERRAIN_POST_POST_PROCESSED;
        c
    }

    /// `LevelChunk(level, blocks, x, z)`. `blocks` is 32768 bytes (y 0..127) or 65536 (y 0..255),
    /// laid out `(x << 11 | z << 7 | y)` per 128-high section.
    pub fn from_blocks(x: i32, z: i32, blocks: &[u8]) -> LevelChunk {
        assert!(
            blocks.len() == SECTION_TILES || blocks.len() == 2 * SECTION_TILES,
            "chunk block array must be 32768 or 65536 bytes, got {}",
            blocks.len()
        );
        let mut c = LevelChunk::blank(x, z);
        c.lower.set_blocks(&blocks[..SECTION_TILES]);
        if blocks.len() > SECTION_TILES {
            c.upper.set_blocks(&blocks[SECTION_TILES..]);
        }
        c
    }

    /// Both 128-high sections `(lower, upper)`.
    pub fn sections_mut(&mut self) -> (&mut Section, &mut Section) {
        (&mut self.lower, &mut self.upper)
    }

    #[inline]
    fn section(&self, y: i32) -> (&Section, usize) {
        debug_assert!((0..MAX_BUILD_HEIGHT).contains(&y));
        if y >= SECTION_HEIGHT as i32 {
            (&self.upper, (y as usize) - SECTION_HEIGHT)
        } else {
            (&self.lower, y as usize)
        }
    }

    #[inline]
    fn section_mut(&mut self, y: i32) -> (&mut Section, usize) {
        debug_assert!((0..MAX_BUILD_HEIGHT).contains(&y));
        if y >= SECTION_HEIGHT as i32 {
            (&mut self.upper, (y as usize) - SECTION_HEIGHT)
        } else {
            (&mut self.lower, y as usize)
        }
    }

    // ---------------------------------------------------------------- tiles & data

    /// `getTile` (x, z in 0..16, y in 0..256).
    #[inline]
    pub fn get_tile(&self, x: i32, y: i32, z: i32) -> u8 {
        if self.is_empty_chunk {
            return 0;
        }
        let (s, yy) = self.section(y);
        s.block(x as usize, yy, z as usize)
    }

    #[inline]
    pub fn get_data(&self, x: i32, y: i32, z: i32) -> u8 {
        if self.is_empty_chunk {
            return 0;
        }
        let (s, yy) = self.section(y);
        s.data.get(x as usize, yy, z as usize)
    }

    /// Storage write only: no lighting, no callbacks. Used by [`crate::level::Level`].
    #[inline]
    pub fn set_tile_raw(&mut self, x: i32, y: i32, z: i32, tile: u8) {
        let (s, yy) = self.section_mut(y);
        s.set_block(x as usize, yy, z as usize, tile);
    }

    #[inline]
    pub fn set_data_raw(&mut self, x: i32, y: i32, z: i32, data: u8) {
        let (s, yy) = self.section_mut(y);
        s.data.set(x as usize, yy, z as usize, data);
    }

    /// `Tile::lightBlock` of the tile at (x, y, z) (`getTileLightBlock`).
    #[inline]
    pub fn get_tile_light_block(&self, x: i32, y: i32, z: i32) -> u8 {
        tile_props::light_block(self.get_tile(x, y, z))
    }

    // ---------------------------------------------------------------- light

    pub fn get_brightness(&self, layer: LightLayer, has_ceiling: bool, x: i32, y: i32, z: i32) -> i32 {
        if self.is_empty_chunk {
            return 0;
        }
        let (s, yy) = self.section(y);
        match layer {
            LightLayer::Sky => {
                if has_ceiling {
                    0
                } else {
                    s.sky.get(x as usize, yy, z as usize) as i32
                }
            }
            LightLayer::Block => s.block_light.get(x as usize, yy, z as usize) as i32,
        }
    }

    pub fn set_brightness(&mut self, layer: LightLayer, has_ceiling: bool, x: i32, y: i32, z: i32, v: i32) {
        if self.is_empty_chunk {
            return;
        }
        self.unsaved = true;
        let (s, yy) = self.section_mut(y);
        match layer {
            LightLayer::Sky => {
                if !has_ceiling {
                    s.sky.set(x as usize, yy, z as usize, v as u8);
                }
            }
            LightLayer::Block => s.block_light.set(x as usize, yy, z as usize, v as u8),
        }
    }

    /// Raw sky-light write used by the height bookkeeping (no ceiling check, no dirty flag).
    #[inline]
    pub fn set_sky_raw(&mut self, x: i32, y: i32, z: i32, v: i32) {
        let (s, yy) = self.section_mut(y);
        s.sky.set(x as usize, yy, z as usize, v as u8);
    }

    /// `getRawBrightness`: max(sky − dampen, block).
    pub fn get_raw_brightness(&self, has_ceiling: bool, x: i32, y: i32, z: i32, sky_dampen: i32) -> i32 {
        if self.is_empty_chunk {
            return 0;
        }
        let (s, yy) = self.section(y);
        let sky = if has_ceiling { 0 } else { s.sky.get(x as usize, yy, z as usize) as i32 };
        let light = sky - sky_dampen;
        let block = s.block_light.get(x as usize, yy, z as usize) as i32;
        light.max(block)
    }

    /// Sky light as Java-order nibbles for the whole column (lower, then upper): 2 × 16384 bytes.
    pub fn sky_light_bytes(&self) -> (Vec<u8>, Vec<u8>) {
        (self.lower.sky.to_bytes(), self.upper.sky.to_bytes())
    }

    pub fn block_light_bytes(&self) -> (Vec<u8>, Vec<u8>) {
        (self.lower.block_light.to_bytes(), self.upper.block_light.to_bytes())
    }

    pub fn data_bytes(&self) -> (Vec<u8>, Vec<u8>) {
        (self.lower.data.to_bytes(), self.upper.data.to_bytes())
    }

    /// `setSkyLightDataAllBright`
    pub fn set_sky_light_all_bright(&mut self) {
        self.lower.sky.fill_all(15);
        self.upper.sky.fill_all(15);
    }

    pub fn set_sky_light_nibbles(&mut self, lower: Nibbles, upper: Nibbles) {
        self.lower.sky = lower;
        self.upper.sky = upper;
    }

    // ---------------------------------------------------------------- heightmap

    #[inline]
    pub fn get_heightmap(&self, x: i32, z: i32) -> i32 {
        if self.is_empty_chunk {
            return 0;
        }
        self.heightmap[col(x, z)] as i32
    }

    /// `isSkyLit`
    #[inline]
    pub fn is_sky_lit(&self, x: i32, y: i32, z: i32) -> bool {
        if self.is_empty_chunk {
            return false;
        }
        y >= self.heightmap[col(x, z)] as i32
    }

    /// `getHighestSectionPosition`
    pub fn highest_section_position(&self) -> i32 {
        MAX_BUILD_HEIGHT - 16
    }

    /// `lightGaps(x, z)`: only sets the "recheck" bit; the real work happens in `recheckGaps`.
    pub fn flag_light_gaps(&mut self, x: i32, z: i32) {
        let slot = ((x >> 1) | (z * 8)) as usize;
        let shift = (x & 1) * 4;
        self.column_flags[slot] |= COLUMN_RECHECK << shift;
        self.has_gaps_to_check = true;
    }

    #[inline]
    pub fn column_flag(&self, x: i32, z: i32, flag: u8) -> bool {
        let slot = ((x >> 1) | (z * 8)) as usize;
        let shift = (x & 1) * 4;
        self.column_flags[slot] & (flag << shift) != 0
    }

    #[inline]
    pub fn set_column_flag(&mut self, x: i32, z: i32, flag: u8, on: bool) {
        let slot = ((x >> 1) | (z * 8)) as usize;
        let shift = (x & 1) * 4;
        if on {
            self.column_flags[slot] |= flag << shift;
        } else {
            self.column_flags[slot] &= !(flag << shift);
        }
    }

    /// `recalcHeightmapOnly`: heightmap and rain heights, no lighting.
    pub fn recalc_heightmap_only(&mut self) {
        let mut min = MAX_BUILD_HEIGHT - 1;
        for x in 0..16 {
            for z in 0..16 {
                self.rain_heights[col(x, z)] = 255;
                let mut y = MAX_BUILD_HEIGHT - 1;
                while y > 0 && tile_props::light_block(self.get_tile(x, y - 1, z)) == 0 {
                    y -= 1;
                }
                self.heightmap[col(x, z)] = y as u8;
                if y < min {
                    min = y;
                }
            }
        }
        self.min_height = min;
        self.unsaved = true;
    }

    /// `recalcHeightmap`: heightmap plus a straight-down sky-light fill from the top of the world,
    /// then flags every column for a gap re-check.
    pub fn recalc_heightmap(&mut self, has_ceiling: bool) {
        self.lowest_heightmap = i32::MAX;
        let mut min = MAX_BUILD_HEIGHT - 1;
        for x in 0..16 {
            for z in 0..16 {
                let mut y = MAX_BUILD_HEIGHT - 1;
                while y > 0 && tile_props::light_block(self.get_tile(x, y - 1, z)) == 0 {
                    y -= 1;
                }
                self.heightmap[col(x, z)] = y as u8;
                if y < min {
                    min = y;
                }
                if y < self.lowest_heightmap {
                    self.lowest_heightmap = y;
                }

                if !has_ceiling {
                    let mut br = MAX_BRIGHTNESS;
                    let mut yy = MAX_BUILD_HEIGHT - 1;
                    loop {
                        br -= tile_props::light_block(self.get_tile(x, yy, z)) as i32;
                        if br > 0 {
                            let (s, ry) = self.section_mut(yy);
                            s.sky.set(x as usize, ry, z as usize, br as u8);
                        }
                        yy -= 1;
                        if !(yy > 0 && br > 0) {
                            break;
                        }
                    }
                }
            }
        }
        self.min_height = min;
        for x in 0..16 {
            for z in 0..16 {
                self.flag_light_gaps(x, z);
            }
        }
        self.unsaved = true;
    }

    /// `getTopRainBlock` (cached in `rainHeights`).
    pub fn get_top_rain_block(&mut self, x: i32, z: i32) -> i32 {
        let slot = col(x, z);
        let mut h = self.rain_heights[slot] as i32;
        if h == 255 {
            let mut y = MAX_BUILD_HEIGHT - 1;
            h = -1;
            while y > 0 && h == -1 {
                let t = self.get_tile(x, y, z);
                let m = tile_props::material(t);
                if !m.blocks_motion() && !m.is_liquid() {
                    y -= 1;
                } else {
                    h = y + 1;
                }
            }
            if h == 255 {
                h = 254;
            }
            self.rain_heights[slot] = h as u8;
        }
        h
    }

    // ---------------------------------------------------------------- misc

    /// `getHighestNonEmptyY`
    pub fn highest_non_empty_y(&self) -> i32 {
        if let Some(y) = self.upper.highest_non_empty_y() {
            return y as i32 + SECTION_HEIGHT as i32;
        }
        self.lower.highest_non_empty_y().map_or(0, |y| y as i32)
    }

    /// `getBlockData`: all blocks in the Java/console order (lower section, then upper).
    pub fn block_data(&self) -> Vec<u8> {
        let mut v = self.lower.blocks_to_vec();
        v.extend(self.upper.blocks_to_vec());
        v
    }

    /// Layers 128..255 (`x << 11 | z << 7 | (y - 128)`).
    pub fn upper_blocks(&self) -> Vec<u8> {
        self.upper.blocks_to_vec()
    }

    /// The first 128 layers only, which is what the generator produces (`x << 11 | z << 7 | y`).
    pub fn lower_blocks(&self) -> Vec<u8> {
        self.lower.blocks_to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_world::tile_ids as t;

    fn flat_chunk() -> LevelChunk {
        // stone up to y=59, dirt 60..62, grass at 63
        let mut blocks = vec![0u8; SECTION_TILES];
        for x in 0..16usize {
            for z in 0..16usize {
                for y in 0..64usize {
                    blocks[(x << 11) | (z << 7) | y] = if y < 60 {
                        t::STONE
                    } else if y < 63 {
                        t::DIRT
                    } else {
                        t::GRASS
                    };
                }
            }
        }
        LevelChunk::from_blocks(3, -2, &blocks)
    }

    #[test]
    fn get_set_roundtrip() {
        let mut c = flat_chunk();
        assert_eq!(c.get_tile(1, 10, 2), t::STONE);
        assert_eq!(c.get_tile(1, 63, 2), t::GRASS);
        assert_eq!(c.get_tile(1, 64, 2), 0);
        assert_eq!(c.get_tile(1, 200, 2), 0);
        c.set_tile_raw(4, 200, 5, t::GLASS);
        assert_eq!(c.get_tile(4, 200, 5), t::GLASS);
        c.set_data_raw(4, 200, 5, 9);
        assert_eq!(c.get_data(4, 200, 5), 9);
        assert_eq!(c.highest_non_empty_y(), 200);
    }

    #[test]
    fn heightmap_and_sky_light() {
        let mut c = flat_chunk();
        c.recalc_heightmap(false);
        // first light-blocking tile is grass at y=63, so the heightmap holds 64
        assert_eq!(c.get_heightmap(0, 0), 64);
        assert_eq!(c.min_height, 64);
        assert_eq!(c.lowest_heightmap, 64);
        assert!(c.is_sky_lit(5, 64, 5));
        assert!(!c.is_sky_lit(5, 63, 5));
        // air above is fully lit, solid ground below is dark
        assert_eq!(c.get_brightness(LightLayer::Sky, false, 5, 100, 5), 15);
        assert_eq!(c.get_brightness(LightLayer::Sky, false, 5, 64, 5), 15);
        assert_eq!(c.get_brightness(LightLayer::Sky, false, 5, 63, 5), 0);
        assert_eq!(c.get_brightness(LightLayer::Sky, false, 5, 30, 5), 0);
        // every column is flagged for a gap re-check
        assert!(c.has_gaps_to_check);
        assert!(c.column_flag(15, 15, COLUMN_RECHECK));
        // with a ceiling there is no sky light at all
        assert_eq!(c.get_brightness(LightLayer::Sky, true, 5, 100, 5), 0);
    }

    #[test]
    fn leaves_attenuate_sky_light_in_the_initial_fill() {
        let mut c = flat_chunk();
        // a leaf block (lightBlock 1) at y=70 and a water block (3) at y=69 above the ground
        c.set_tile_raw(2, 70, 2, t::LEAVES);
        c.set_tile_raw(2, 69, 2, t::WATER);
        c.recalc_heightmap(false);
        assert_eq!(c.get_heightmap(2, 2), 71, "heightmap stops at the first tile with lightBlock > 0, leaves included");
        // 15 -1 (leaves at 70) = 14 at y=70, then air keeps 14 until water: 14-3 = 11
        assert_eq!(c.get_brightness(LightLayer::Sky, false, 2, 71, 2), 15);
        assert_eq!(c.get_brightness(LightLayer::Sky, false, 2, 70, 2), 14);
        assert_eq!(c.get_brightness(LightLayer::Sky, false, 2, 69, 2), 11);
        assert_eq!(c.get_brightness(LightLayer::Sky, false, 2, 68, 2), 11);
    }

    #[test]
    fn top_rain_block_is_cached_and_skips_non_motion_blocking() {
        let mut c = flat_chunk();
        c.set_tile_raw(7, 70, 7, t::TALLGRASS);
        assert_eq!(c.get_top_rain_block(7, 7), 64);
        assert_eq!(c.rain_heights[(7 << 4) | 7], 64);
        // water counts (liquid)
        c.set_tile_raw(8, 64, 8, t::CALM_WATER);
        c.rain_heights[(8 << 4) | 8] = 255;
        assert_eq!(c.get_top_rain_block(8, 8), 65);
    }

    #[test]
    fn empty_chunk_reports_air_and_darkness() {
        let c = LevelChunk::new_empty_chunk();
        assert_eq!(c.get_tile(1, 1, 1), 0);
        assert_eq!(c.get_heightmap(1, 1), 0);
        assert!(!c.is_sky_lit(1, 1, 1));
        assert_eq!(c.get_brightness(LightLayer::Sky, false, 1, 1, 1), 0);
        assert_eq!(c.terrain_populated, TERRAIN_POPULATED_ALL_NEIGHBOURS | TERRAIN_POST_POST_PROCESSED);
    }
}
