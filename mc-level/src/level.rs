//! Port of the block-access and lighting core of `Level` (Windows64, `_LARGE_WORLDS`).
//!
//! Notes on fidelity:
//! * `checkLight` follows the 4J version (bounded 32³ work buffer, edge-only second pass after a
//!   darkening). The per-thread light cache of the original is a pure optimisation and is not
//!   ported; where the cached and uncached paths differ (emission of opaque emitters, lightBlock
//!   masked to 4 bits) the *cached* semantics are used, because that is what the game actually
//!   relies on.
//! * Tile callbacks (`onPlace`, `onRemove`, neighbour updates) are not part of this layer yet.

use crate::chunk::*;
use mc_world::tile_props;
use mc_world::{MAX_BRIGHTNESS, MAX_BUILD_HEIGHT};
use mc_core::Random;
use std::collections::{HashMap, VecDeque};

pub const MAX_LEVEL_SIZE: i32 = 30_000_000;

// Tile::UPDATE_* flags
pub const UPDATE_NEIGHBORS: i32 = 1;
pub const UPDATE_CLIENTS: i32 = 2;
pub const UPDATE_INVISIBLE: i32 = 4;
pub const UPDATE_INVISIBLE_NO_LIGHT: i32 = 8 | 4;
pub const UPDATE_NONE: i32 = UPDATE_INVISIBLE;
pub const UPDATE_ALL: i32 = UPDATE_NEIGHBORS | UPDATE_CLIENTS;

const STEP: [(i32, i32, i32); 6] = [(0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1), (-1, 0, 0), (1, 0, 0)];

pub struct Level {
    chunks: HashMap<(i32, i32), LevelChunk>,
    empty_chunk: LevelChunk,
    pub has_ceiling: bool,
    /// `dimension->getXZSize()`: world width in chunks.
    pub xz_size: i32,
    /// When false, `set_tile_and_data` skips `checkLight` (bulk generation).
    pub lighting: bool,
    to_check: Vec<i32>,
    /// `Level::random` (used for instant ticks, not for world generation).
    pub random: Random,
    /// `Level::getInstaTick()` (thread-local in the original).
    pub insta_tick: bool,
    /// `HeavyTile::instaFall`
    pub insta_fall: bool,
    /// cells where a sand / gravel / anvil block wants to start falling (drained by the game)
    pub fall_requests: Vec<(i32, i32, i32)>,
    /// `dimension->ultraWarm` (Nether).
    pub ultra_warm: bool,
    pub sea_level: i32,
    /// `Level::m_iHighestY` (used by the End decorator).
    pub highest_y: i32,
    /// Ticks scheduled with `addToTickNextTick` that were not run instantly.
    pub pending_ticks: Vec<PendingTick>,
    /// Positions whose tile or data changed since the log was last drained (only filled while `log_changes` is set).
    pub change_log: Vec<(i32, i32, i32)>,
    pub log_changes: bool,
    /// Game time used as the base of scheduled ticks.
    pub game_time: i64,
    /// Per-tile-instance state of `LiquidTileDynamic::tick` (index 0 = water, 1 = lava).
    pub(crate) liq_iterative: [bool; 2],
    pub(crate) liq_queue: [VecDeque<(i32, i32, i32)>; 2],
    /// Tile entities by block position.
    pub tile_entities: HashMap<(i32, i32, i32), crate::tile_entity::TileEntity>,
    /// Chest minecarts spawned during generation: position and the 27 slots of loot (entities are not modelled).
    /// Mobs spawned by structure generation (e.g. the swamp hut witch): entity id and block position (entities are not modelled).
    pub spawned_mobs: Vec<(&'static str, i32, i32, i32)>,
    pub minecart_chests: Vec<((i32, i32, i32), Vec<Option<crate::tile_entity::ItemStack>>)>,
}

/// A tick scheduled for a later game time (`TickNextTickData`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingTick {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub tile: u8,
    pub time: i64,
}

impl Level {
    pub fn new(xz_size_chunks: i32, has_ceiling: bool) -> Level {
        Level {
            chunks: HashMap::new(),
            empty_chunk: LevelChunk::new_empty_chunk(),
            has_ceiling,
            xz_size: xz_size_chunks,
            lighting: true,
            to_check: vec![0; 32 * 32 * 32],
            random: Random::with_seed(0),
            insta_tick: false,
            insta_fall: false,
            fall_requests: Vec::new(),
            ultra_warm: false,
            sea_level: 63,
            highest_y: -1000,
            pending_ticks: Vec::new(),
            change_log: Vec::new(),
            log_changes: false,
            game_time: 0,
            liq_iterative: [false; 2],
            liq_queue: [VecDeque::new(), VecDeque::new()],
            tile_entities: HashMap::new(),
            spawned_mobs: Vec::new(),
            minecart_chests: Vec::new(),
        }
    }

    #[inline]
    pub fn min_xz(&self) -> i32 {
        -(self.xz_size * 16) / 2
    }
    #[inline]
    pub fn max_xz(&self) -> i32 {
        (self.xz_size * 16) / 2 - 1
    }

    // ------------------------------------------------------------------ chunk access

    pub fn insert_chunk(&mut self, mut c: LevelChunk) {
        c.loaded = true;
        self.chunks.insert((c.x, c.z), c);
    }
    pub fn remove_chunk(&mut self, x: i32, z: i32) -> Option<LevelChunk> {
        self.chunks.remove(&(x, z))
    }
    #[inline]
    pub fn has_chunk(&self, x: i32, z: i32) -> bool {
        self.chunks.contains_key(&(x, z))
    }
    pub fn chunk(&self, x: i32, z: i32) -> Option<&LevelChunk> {
        self.chunks.get(&(x, z))
    }
    pub fn chunk_mut(&mut self, x: i32, z: i32) -> Option<&mut LevelChunk> {
        self.chunks.get_mut(&(x, z))
    }
    /// `getChunk`: the shared empty chunk when the real one is absent.
    pub fn get_chunk(&self, x: i32, z: i32) -> &LevelChunk {
        self.chunks.get(&(x, z)).unwrap_or(&self.empty_chunk)
    }
    pub fn chunk_coords(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.chunks.keys().copied()
    }
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn has_chunk_at(&self, x: i32, y: i32, z: i32) -> bool {
        if !(0..MAX_BUILD_HEIGHT).contains(&y) {
            return false;
        }
        self.has_chunk(x >> 4, z >> 4)
    }

    pub fn has_chunks_at(&self, x: i32, y: i32, z: i32, r: i32) -> bool {
        self.has_chunks_in(x - r, y - r, z - r, x + r, y + r, z + r)
    }

    pub fn has_chunks_in(&self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32) -> bool {
        if y1 < 0 || y0 >= MAX_BUILD_HEIGHT {
            return false;
        }
        for cx in (x0 >> 4)..=(x1 >> 4) {
            for cz in (z0 >> 4)..=(z1 >> 4) {
                if !self.has_chunk(cx, cz) {
                    return false;
                }
            }
        }
        true
    }

    // ------------------------------------------------------------------ tiles

    #[inline]
    fn out_of_world(x: i32, z: i32) -> bool {
        x < -MAX_LEVEL_SIZE || z < -MAX_LEVEL_SIZE || x >= MAX_LEVEL_SIZE || z >= MAX_LEVEL_SIZE
    }

    pub fn get_tile(&self, x: i32, y: i32, z: i32) -> u8 {
        if Self::out_of_world(x, z) || !(0..MAX_BUILD_HEIGHT).contains(&y) {
            return 0;
        }
        match self.chunks.get(&(x >> 4, z >> 4)) {
            Some(c) => c.get_tile(x & 15, y, z & 15),
            None => 0,
        }
    }

    pub fn get_data(&self, x: i32, y: i32, z: i32) -> u8 {
        if Self::out_of_world(x, z) || !(0..MAX_BUILD_HEIGHT).contains(&y) {
            return 0;
        }
        match self.chunks.get(&(x >> 4, z >> 4)) {
            Some(c) => c.get_data(x & 15, y, z & 15),
            None => 0,
        }
    }

    pub fn get_material(&self, x: i32, y: i32, z: i32) -> mc_world::Material {
        tile_props::material(self.get_tile(x, y, z))
    }

    pub fn get_heightmap(&self, x: i32, z: i32) -> i32 {
        if Self::out_of_world(x, z) {
            return 0;
        }
        match self.chunks.get(&(x >> 4, z >> 4)) {
            Some(c) => c.get_heightmap(x & 15, z & 15),
            None => 0,
        }
    }

    /// `getTopSolidBlock`-style helper used by decorators: heightmap lookup (first free y above ground).
    pub fn get_top_rain_block(&mut self, x: i32, z: i32) -> i32 {
        match self.chunks.get_mut(&(x >> 4, z >> 4)) {
            Some(c) => c.get_top_rain_block(x & 15, z & 15),
            None => 0,
        }
    }

    pub fn is_sky_lit(&self, x: i32, y: i32, z: i32) -> bool {
        if Self::out_of_world(x, z) || self.has_ceiling || y < 0 {
            return false;
        }
        if y >= MAX_BUILD_HEIGHT {
            return true;
        }
        match self.chunks.get(&(x >> 4, z >> 4)) {
            Some(c) => c.is_sky_lit(x & 15, y, z & 15),
            None => false,
        }
    }

    /// `canSeeSky`: unlike `isSkyLit` this goes through `getChunk` (empty chunk when absent).
    pub fn can_see_sky(&self, x: i32, y: i32, z: i32) -> bool {
        self.get_chunk(x >> 4, z >> 4).is_sky_lit(x & 15, y, z & 15)
    }

    // ------------------------------------------------------------------ light values

    pub fn get_brightness(&self, layer: LightLayer, x: i32, y: i32, z: i32) -> i32 {
        let half = self.xz_size / 2;
        let (xc, zc) = (x >> 4, z >> 4);
        let (ix, iz) = (xc + half, zc + half);
        if ix < 0 || ix >= self.xz_size || iz < 0 || iz >= self.xz_size {
            return 0;
        }
        match self.chunks.get(&(xc, zc)) {
            None => layer.surrounding(),
            Some(c) => {
                let y = y.clamp(0, MAX_BUILD_HEIGHT - 1);
                c.get_brightness(layer, self.has_ceiling, x & 15, y, z & 15)
            }
        }
    }

    pub fn set_brightness(&mut self, layer: LightLayer, x: i32, y: i32, z: i32, v: i32) {
        if Self::out_of_world(x, z) || !(0..MAX_BUILD_HEIGHT).contains(&y) {
            return;
        }
        let hc = self.has_ceiling;
        if let Some(c) = self.chunks.get_mut(&(x >> 4, z >> 4)) {
            c.set_brightness(layer, hc, x & 15, y, z & 15, v);
        }
    }

    /// `getRawBrightness(x, y, z, propagate=false)` with sky dampen 0 (what generators need).
    pub fn get_raw_brightness(&self, x: i32, y: i32, z: i32) -> i32 {
        if Self::out_of_world(x, z) || y < 0 {
            return 15;
        }
        let y = y.min(MAX_BUILD_HEIGHT - 1);
        self.get_chunk(x >> 4, z >> 4).get_raw_brightness(self.has_ceiling, x & 15, y, z & 15, 0)
    }

    // ------------------------------------------------------------------ setTile

    /// `Level::setTileAndData`. Returns whether anything changed.
    pub fn set_tile_and_data(&mut self, x: i32, y: i32, z: i32, tile: u8, data: u8, flags: i32) -> bool {
        if Self::out_of_world(x, z) || !(0..MAX_BUILD_HEIGHT).contains(&y) {
            return false;
        }
        let (cx, cz) = (x >> 4, z >> 4);
        if !self.chunks.contains_key(&(cx, cz)) {
            return false;
        }
        let old_tile = self.get_tile(x, y, z);
        let result = self.chunk_set_tile_and_data(cx, cz, x & 15, y, z & 15, tile, data);
        if result && old_tile != tile {
            self.tile_entities.remove(&(x, y, z));
        }
        if result && tile != 0 {
            self.on_place(x, y, z, tile);
        }
        if flags != UPDATE_INVISIBLE_NO_LIGHT && self.lighting {
            self.check_light(x, y, z);
        }
        if result && flags & UPDATE_NEIGHBORS != 0 {
            self.tile_updated(x, y, z, old_tile);
        }
        result
    }

    /// `Level::getTileEntity`: chests and spawners get their entity created on demand.
    pub fn tile_entity_mut(&mut self, x: i32, y: i32, z: i32) -> Option<&mut crate::tile_entity::TileEntity> {
        use crate::tile_entity::TileEntity;
        let tile = self.get_tile(x, y, z);
        if tile != mc_world::tile_ids::CHEST && tile != mc_world::tile_ids::MOB_SPAWNER && tile != mc_world::tile_ids::DISPENSER && tile != mc_world::tile_ids::SIGN && tile != mc_world::tile_ids::WALL_SIGN {
            return None;
        }
        Some(self.tile_entities.entry((x, y, z)).or_insert_with(|| {
            if tile == mc_world::tile_ids::CHEST {
                TileEntity::new_chest()
            } else if tile == mc_world::tile_ids::SIGN || tile == mc_world::tile_ids::WALL_SIGN {
                TileEntity::Sign { lines: Default::default() }
            } else if tile == mc_world::tile_ids::DISPENSER {
                TileEntity::Dispenser { items: vec![None; crate::tile_entity::DISPENSER_SIZE] }
            } else {
                TileEntity::Spawner { entity_id: String::new() }
            }
        }))
    }

    pub fn tile_entity(&self, x: i32, y: i32, z: i32) -> Option<&crate::tile_entity::TileEntity> {
        self.tile_entities.get(&(x, y, z))
    }

    /// `Level::tileUpdated`
    pub fn tile_updated(&mut self, x: i32, y: i32, z: i32, tile: u8) {
        self.update_neighbors_at(x, y, z, tile);
    }

    /// `Level::updateNeighborsAt`
    pub fn update_neighbors_at(&mut self, x: i32, y: i32, z: i32, tile: u8) {
        self.neighbor_changed(x - 1, y, z, tile);
        self.neighbor_changed(x + 1, y, z, tile);
        self.neighbor_changed(x, y - 1, z, tile);
        self.neighbor_changed(x, y + 1, z, tile);
        self.neighbor_changed(x, y, z - 1, tile);
        self.neighbor_changed(x, y, z + 1, tile);
    }

    /// `Level::setData` (no tile callbacks beyond the neighbour update).
    pub fn set_data(&mut self, x: i32, y: i32, z: i32, data: u8, flags: i32) -> bool {
        if Self::out_of_world(x, z) || !(0..MAX_BUILD_HEIGHT).contains(&y) {
            return false;
        }
        let Some(c) = self.chunks.get_mut(&(x >> 4, z >> 4)) else { return false };
        if c.get_data(x & 15, y, z & 15) == data {
            return false;
        }
        c.set_data_raw(x & 15, y, z & 15, data);
        c.unsaved = true;
        if self.log_changes {
            self.change_log.push((x, y, z));
        }
        if flags & UPDATE_NEIGHBORS != 0 {
            let t = self.get_tile(x, y, z);
            self.tile_updated(x, y, z, t);
        }
        true
    }

    pub fn remove_tile(&mut self, x: i32, y: i32, z: i32) -> bool {
        self.set_tile_and_data(x, y, z, 0, 0, UPDATE_ALL)
    }

    pub fn set_tile_and_update(&mut self, x: i32, y: i32, z: i32, tile: u8) -> bool {
        self.set_tile_and_data(x, y, z, tile, 0, UPDATE_ALL)
    }

    /// `Level::destroyTile` (drops are not simulated).
    pub fn destroy_tile(&mut self, x: i32, y: i32, z: i32) -> bool {
        if self.get_tile(x, y, z) > 0 {
            self.set_tile_and_data(x, y, z, 0, 0, UPDATE_ALL)
        } else {
            false
        }
    }

    pub fn is_empty_tile(&self, x: i32, y: i32, z: i32) -> bool {
        self.get_tile(x, y, z) == 0
    }

    /// `Level::getTopSolidBlock`: first y above the highest tile that blocks motion and is not leaves.
    pub fn get_top_solid_block(&self, x: i32, z: i32) -> i32 {
        let Some(c) = self.chunks.get(&(x >> 4, z >> 4)) else { return -1 };
        let mut y = c.highest_section_position() + 15;
        let (lx, lz) = (x & 15, z & 15);
        while y > 0 {
            let t = c.get_tile(lx, y, lz);
            let m = tile_props::material(t);
            if t == 0 || !m.blocks_motion() || m == mc_world::Material::Leaves {
                y -= 1;
            } else {
                return y + 1;
            }
        }
        -1
    }

    /// `Level::getDaytimeRawBrightness`
    pub fn get_daytime_raw_brightness(&self, x: i32, y: i32, z: i32) -> i32 {
        if y < 0 {
            return 0;
        }
        let y = y.min(MAX_BUILD_HEIGHT - 1);
        self.get_chunk(x >> 4, z >> 4).get_raw_brightness(self.has_ceiling, x & 15, y, z & 15, 0)
    }

    /// `Level::isTopSolidBlocking` for the tiles that matter during generation.
    pub fn is_top_solid_blocking(&self, x: i32, y: i32, z: i32) -> bool {
        let t = self.get_tile(x, y, z);
        if t == 0 {
            return false;
        }
        let m = tile_props::material(t);
        m.is_solid_blocking() && tile_props::is_solid_render(t)
    }

    pub fn set_tile(&mut self, x: i32, y: i32, z: i32, tile: u8, flags: i32) -> bool {
        self.set_tile_and_data(x, y, z, tile, 0, flags)
    }

    /// `Level::setTileNoUpdate`-style (UPDATE_INVISIBLE → still lights).
    pub fn set_tile_no_update(&mut self, x: i32, y: i32, z: i32, tile: u8) -> bool {
        self.set_tile_and_data(x, y, z, tile, 0, UPDATE_NONE)
    }

    /// `LevelChunk::setTileAndData` (storage, heightmap and sky bookkeeping; no tile callbacks).
    fn chunk_set_tile_and_data(&mut self, cx: i32, cz: i32, x: i32, y: i32, z: i32, tile: u8, data: u8) -> bool {
        let has_ceiling = self.has_ceiling;
        let slot = ((z << 4) | x) as usize;
        let (old, old_data, old_height) = {
            let c = self.chunks.get_mut(&(cx, cz)).unwrap();
            if y >= c.rain_heights[slot] as i32 - 1 {
                c.rain_heights[slot] = 255;
            }
            let oh = c.heightmap[slot] as i32;
            (c.get_tile(x, y, z), c.get_data(x, y, z), oh)
        };
        if old == tile && old_data == data {
            return false;
        }
        if self.log_changes {
            self.change_log.push((cx * 16 + x, y, cz * 16 + z));
        }
        {
            let c = self.chunks.get_mut(&(cx, cz)).unwrap();
            c.set_tile_raw(x, y, z, tile);
            c.set_data_raw(x, y, z, data);
            if tile_props::light_emission(tile) > 0 {
                c.emissive_added = true;
            }
        }
        let new_lb = tile_props::light_block(tile);
        if new_lb != tile_props::light_block(old) {
            if !has_ceiling {
                if new_lb != 0 {
                    if y >= old_height {
                        self.recalc_height(cx, cz, x, y + 1, z);
                    }
                } else if y == old_height - 1 {
                    self.recalc_height(cx, cz, x, y, z);
                }
            }
            self.chunks.get_mut(&(cx, cz)).unwrap().flag_light_gaps(x, z);
        }
        self.chunks.get_mut(&(cx, cz)).unwrap().unsaved = true;
        true
    }

    /// `LevelChunk::recalcHeight` (including its index quirk: the `y` handed to the original
    /// `lightColumnChanged` is irrelevant here, so only the gap lighting afterwards matters).
    fn recalc_height(&mut self, cx: i32, cz: i32, x: i32, y_start: i32, z: i32) {
        let slot = ((z << 4) | x) as usize;
        let (y_old, mut y);
        {
            let c = self.chunks.get_mut(&(cx, cz)).unwrap();
            y_old = c.heightmap[slot] as i32;
            y = y_old.max(y_start);
            while y > 0 && tile_props::light_block(c.get_tile(x, y - 1, z)) == 0 {
                y -= 1;
            }
            if y == y_old {
                return;
            }
            c.heightmap[slot] = y as u8;
            if y < c.min_height {
                c.min_height = y;
            } else {
                let mut min = MAX_BUILD_HEIGHT - 1;
                for h in c.heightmap.iter() {
                    min = min.min(*h as i32);
                }
                c.min_height = min;
            }
            if !self.has_ceiling {
                if y < y_old {
                    for yy in y..y_old {
                        c.set_sky_raw(x, yy, z, 15);
                    }
                } else {
                    for yy in y_old..y {
                        c.set_sky_raw(x, yy, z, 0);
                    }
                }
                let mut br = 15;
                let mut yy = y;
                while yy > 0 && br > 0 {
                    yy -= 1;
                    let mut block = tile_props::light_block(c.get_tile(x, yy, z)) as i32;
                    if block == 0 {
                        block = 1;
                    }
                    br -= block;
                    if br < 0 {
                        br = 0;
                    }
                    c.set_sky_raw(x, yy, z, br);
                }
            }
        }
        let x_offs = cx * 16 + x;
        let z_offs = cz * 16 + z;
        let height = self.chunks[&(cx, cz)].heightmap[slot] as i32;
        let (y1, y2) = if height < y_old { (height, y_old) } else { (y_old, height) };
        {
            let c = self.chunks.get_mut(&(cx, cz)).unwrap();
            if height < c.lowest_heightmap {
                c.lowest_heightmap = height;
            }
        }
        if !self.has_ceiling {
            self.light_gap_range(x_offs - 1, z_offs, y1, y2);
            self.light_gap_range(x_offs + 1, z_offs, y1, y2);
            self.light_gap_range(x_offs, z_offs - 1, y1, y2);
            self.light_gap_range(x_offs, z_offs + 1, y1, y2);
            self.light_gap_range(x_offs, z_offs, y1, y2);
        }
        self.chunks.get_mut(&(cx, cz)).unwrap().unsaved = true;
    }

    /// `LevelChunk::lightGap(x, z, y1, y2)`
    fn light_gap_range(&mut self, x: i32, z: i32, y1: i32, y2: i32) {
        if y2 > y1 && self.has_chunks_at(x, MAX_BUILD_HEIGHT / 2, z, 16) {
            for y in y1..y2 {
                self.check_light_layer(LightLayer::Sky, x, y, z, false, false);
            }
            if let Some(c) = self.chunks.get_mut(&(x >> 4, z >> 4)) {
                c.unsaved = true;
            }
        }
    }

    /// `LevelChunk::lightGap(x, z, source)`
    fn light_gap_source(&mut self, x: i32, z: i32, source: i32) {
        let height = self.get_heightmap(x, z);
        if height > source {
            self.light_gap_range(x, z, source, height + 1);
        } else if height < source {
            self.light_gap_range(x, z, height, source + 1);
        }
    }

    /// `LevelChunk::recheckGaps`
    pub fn recheck_gaps(&mut self, cx: i32, cz: i32, force: bool) {
        let (min_xz, max_xz) = (self.min_xz(), self.max_xz());
        match self.chunks.get(&(cx, cz)) {
            None => return,
            // isEmpty(): never true for real chunks
            Some(_) => {}
        }
        if !self.has_chunks_at(cx * 16 + 8, MAX_BUILD_HEIGHT / 2, cz * 16 + 8, 16) {
            return;
        }
        for x in 0..16 {
            for z in 0..16 {
                let flagged = {
                    let c = self.chunks.get_mut(&(cx, cz)).unwrap();
                    let f = force || c.column_flag(x, z, COLUMN_RECHECK);
                    if f {
                        c.set_column_flag(x, z, COLUMN_RECHECK, false);
                    }
                    f
                };
                if !flagged {
                    continue;
                }
                let height = self.chunks[&(cx, cz)].get_heightmap(x, z);
                let xo = cx * 16 + x;
                let zo = cz * 16 + z;
                let mut nmin = self.get_heightmap(xo, zo);
                if xo - 1 >= min_xz {
                    nmin = nmin.min(self.get_heightmap(xo - 1, zo));
                }
                if xo + 1 <= max_xz {
                    nmin = nmin.min(self.get_heightmap(xo + 1, zo));
                }
                if zo - 1 >= min_xz {
                    nmin = nmin.min(self.get_heightmap(xo, zo - 1));
                }
                if zo + 1 <= max_xz {
                    nmin = nmin.min(self.get_heightmap(xo, zo + 1));
                }
                self.light_gap_source(xo, zo, nmin);
                if !force {
                    if xo - 1 >= min_xz {
                        self.light_gap_source(xo - 1, zo, height);
                    }
                    if xo + 1 <= max_xz {
                        self.light_gap_source(xo + 1, zo, height);
                    }
                    if zo - 1 >= min_xz {
                        self.light_gap_source(xo, zo - 1, height);
                    }
                    if zo + 1 <= max_xz {
                        self.light_gap_source(xo, zo + 1, height);
                    }
                }
                self.chunks.get_mut(&(cx, cz)).unwrap().has_gaps_to_check = false;
            }
        }
    }

    /// `LevelChunk::tick` lighting part.
    pub fn tick_chunk_lighting(&mut self, cx: i32, cz: i32) {
        let pending = self.chunks.get(&(cx, cz)).map_or(false, |c| c.has_gaps_to_check);
        if pending && !self.has_ceiling {
            self.recheck_gaps(cx, cz, false);
        }
    }

    /// `LevelChunk::lightLava`
    pub fn light_lava(&mut self, cx: i32, cz: i32) {
        let ok = self.chunks.get(&(cx, cz)).map_or(false, |c| c.emissive_added);
        if !ok {
            return;
        }
        for x in 0..16 {
            for z in 0..16 {
                for y in 0..crate::storage::SECTION_HEIGHT as i32 {
                    let (emit, br) = {
                        let c = &self.chunks[&(cx, cz)];
                        (
                            tile_props::light_emission(c.get_tile(x, y, z)) as i32,
                            c.get_brightness(LightLayer::Block, self.has_ceiling, x, y, z),
                        )
                    };
                    if emit > 0 && br < emit {
                        self.check_light_layer(LightLayer::Block, cx * 16 + x, y, cz * 16 + z, true, false);
                    }
                }
            }
        }
        self.chunks.get_mut(&(cx, cz)).unwrap().emissive_added = false;
    }

    // ------------------------------------------------------------------ lighting

    pub fn check_light(&mut self, x: i32, y: i32, z: i32) {
        self.check_light_full(x, y, z, false, false);
    }

    pub fn check_light_full(&mut self, x: i32, y: i32, z: i32, force: bool, root_only_emissive: bool) {
        if !self.has_ceiling {
            self.check_light_layer(LightLayer::Sky, x, y, z, force, false);
        }
        self.check_light_layer(LightLayer::Block, x, y, z, force, root_only_emissive);
    }

    fn expected_light(&self, x: i32, y: i32, z: i32, layer: LightLayer, propagated_only: bool) -> i32 {
        if layer == LightLayer::Sky && self.can_see_sky(x, y, z) {
            return MAX_BRIGHTNESS;
        }
        let id = self.get_tile(x, y, z);
        let emission = tile_props::light_emission(id) as i32;
        let mut result = if layer == LightLayer::Sky { 0 } else { emission };
        let mut block = tile_props::light_block(id) as i32;
        if block >= MAX_BRIGHTNESS && emission > 0 {
            block = 1;
        }
        if block < 1 {
            block = 1;
        }
        if block >= MAX_BRIGHTNESS {
            return if propagated_only { 0 } else { emission & 15 };
        }
        if result >= MAX_BRIGHTNESS - 1 {
            return result;
        }
        for (dx, dy, dz) in STEP {
            let b = self.get_brightness(layer, x + dx, y + dy, z + dz) - block;
            if b > result {
                result = b;
            }
            if result >= MAX_BRIGHTNESS - 1 {
                return result;
            }
        }
        result
    }

    /// `Level::checkLight(layer, x, y, z, force, rootOnlyEmissive)`
    pub fn check_light_layer(&mut self, layer: LightLayer, xc: i32, yc: i32, zc: i32, force: bool, root_only_emissive: bool) {
        if force {
            if !self.has_chunks_at(xc, yc, zc, 0) {
                return;
            }
        } else if !self.has_chunks_at(xc, yc, zc, 17) {
            return;
        }
        let (min_xz, max_xz) = (self.min_xz(), self.max_xz());
        if xc > max_xz || xc < min_xz || zc > max_xz || zc < min_xz {
            return;
        }

        let mut to_check = std::mem::take(&mut self.to_check);
        let mut checked = 0usize;
        let mut count = 0usize;
        const CAP: usize = 32 * 32 * 32;

        let center_current = self.get_brightness(layer, xc, yc, zc);
        let center_expected = self.expected_light(xc, yc, zc, layer, false);

        if center_expected > center_current {
            to_check[count] = 32 | (32 << 6) | (32 << 12);
            count += 1;
        } else if center_expected < center_current {
            let mut tcn = 0usize;
            to_check[count] = 32 | (32 << 6) | (32 << 12) | (center_current << 18);
            count += 1;
            while checked < count {
                let p = to_check[checked];
                checked += 1;
                let x = (p & 63) - 32 + xc;
                let y = ((p >> 6) & 63) - 32 + yc;
                let z = ((p >> 12) & 63) - 32 + zc;
                let expected = (p >> 18) & 15;
                let current = self.get_brightness(layer, x, y, z);
                if current == expected {
                    self.set_brightness(layer, x, y, z, 0);
                    if expected > 0 {
                        let (xd, yd, zd) = ((x - xc).abs(), (y - yc).abs(), (z - zc).abs());
                        if xd + yd + zd < 17 {
                            let mut edge = false;
                            for (dx, dy, dz) in STEP {
                                let (xx, yy, zz) = (x + dx, y + dy, z + dz);
                                if xx > max_xz || xx < min_xz || zz > max_xz || zz < min_xz {
                                    continue;
                                }
                                if yy < 0 || yy >= MAX_BUILD_HEIGHT {
                                    continue;
                                }
                                let block = (tile_props::light_block(self.get_tile(xx, yy, zz)) as i32 & 15).max(1);
                                let cur = self.get_brightness(layer, xx, yy, zz);
                                if cur == expected - block && count < CAP {
                                    to_check[count] = (xx - xc + 32)
                                        | ((yy - yc + 32) << 6)
                                        | ((zz - zc + 32) << 12)
                                        | ((expected - block) << 18);
                                    count += 1;
                                } else if cur > expected - block {
                                    edge = true;
                                }
                            }
                            if edge {
                                to_check[tcn] = p;
                                tcn += 1;
                            }
                        }
                    }
                }
            }
            checked = 0;
            count = tcn;
        }

        while checked < count {
            let p = to_check[checked];
            checked += 1;
            let x = (p & 63) - 32 + xc;
            let y = ((p >> 6) & 63) - 32 + yc;
            let z = ((p >> 12) & 63) - 32 + zc;
            if force && !self.has_chunk_at(x, y, z) {
                continue;
            }
            let current = self.get_brightness(layer, x, y, z);
            let propagated_only =
                layer == LightLayer::Block && root_only_emissive && (x != xc || y != yc || z != zc);
            let expected = self.expected_light(x, y, z, layer, propagated_only);
            if expected != current {
                self.set_brightness(layer, x, y, z, expected);
                if expected > current {
                    let (xd, yd, zd) = ((x - xc).abs(), (y - yc).abs(), (z - zc).abs());
                    let within = count < CAP - 6;
                    if xd + yd + zd < 17 && within {
                        let mut push = |this: &Self, nx: i32, ny: i32, nz: i32, cnt: &mut usize| {
                            if this.get_brightness(layer, nx, ny, nz) < expected {
                                to_check[*cnt] = (nx - xc + 32) + ((ny - yc + 32) << 6) + ((nz - zc + 32) << 12);
                                *cnt += 1;
                            }
                        };
                        if x - 1 >= min_xz {
                            push(self, x - 1, y, z, &mut count);
                        }
                        if x + 1 <= max_xz {
                            push(self, x + 1, y, z, &mut count);
                        }
                        if y - 1 >= 0 {
                            push(self, x, y - 1, z, &mut count);
                        }
                        if y + 1 < MAX_BUILD_HEIGHT {
                            push(self, x, y + 1, z, &mut count);
                        }
                        if z - 1 >= min_xz {
                            push(self, x, y, z - 1, &mut count);
                        }
                        if z + 1 <= max_xz {
                            push(self, x, y, z + 1, &mut count);
                        }
                    }
                }
            }
        }
        self.to_check = to_check;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_world::tile_ids as t;

    /// 5×5 chunks around the origin of flat terrain (stone to y=59, grass at 63 → surface 64).
    fn flat_level() -> Level {
        let mut lv = Level::new(64, false);
        for cx in -2..=2 {
            for cz in -2..=2 {
                let mut b = vec![0u8; crate::storage::SECTION_TILES];
                for x in 0..16usize {
                    for z in 0..16usize {
                        for y in 0..64usize {
                            b[(x << 11) | (z << 7) | y] = if y < 63 { t::STONE } else { t::GRASS };
                        }
                    }
                }
                let mut c = LevelChunk::from_blocks(cx, cz, &b);
                c.recalc_heightmap(false);
                lv.insert_chunk(c);
            }
        }
        for cx in -2..=2 {
            for cz in -2..=2 {
                lv.recheck_gaps(cx, cz, true);
            }
        }
        lv
    }

    #[test]
    fn flat_world_has_consistent_sky_light() {
        let lv = flat_level();
        assert_eq!(lv.get_brightness(LightLayer::Sky, 0, 64, 0), 15);
        assert_eq!(lv.get_brightness(LightLayer::Sky, 0, 63, 0), 0);
        assert_eq!(lv.get_brightness(LightLayer::Sky, 17, 200, -5), 15);
        // in-range but unloaded chunk reports "surrounding"; out of range reports 0
        assert_eq!(lv.get_brightness(LightLayer::Sky, 5000, 64, 0), 0);
        assert_eq!(lv.get_brightness(LightLayer::Sky, 400, 64, 0), 15);
        assert_eq!(lv.get_brightness(LightLayer::Block, 400, 64, 0), 0);
    }

    #[test]
    fn block_light_falls_off_and_is_removed() {
        let mut lv = flat_level();
        lv.set_tile(3, 70, 3, t::GLOWSTONE, UPDATE_ALL);
        assert_eq!(lv.get_brightness(LightLayer::Block, 3, 70, 3), 15);
        assert_eq!(lv.get_brightness(LightLayer::Block, 4, 70, 3), 14);
        assert_eq!(lv.get_brightness(LightLayer::Block, 3, 72, 5), 11);
        assert_eq!(lv.get_brightness(LightLayer::Block, 3 + 14, 70, 3), 1);
        assert_eq!(lv.get_brightness(LightLayer::Block, 3 + 15, 70, 3), 0);
        // crosses the chunk border at x=15/16 and keeps decaying
        assert_eq!(lv.get_brightness(LightLayer::Block, 16, 70, 3), 15 - 13);
        lv.set_tile(3, 70, 3, 0, UPDATE_ALL);
        for (dx, dy, dz) in [(0, 0, 0), (1, 0, 0), (5, 2, 3), (-10, 0, 0)] {
            assert_eq!(lv.get_brightness(LightLayer::Block, 3 + dx, 70 + dy, 3 + dz), 0);
        }
    }

    #[test]
    fn roof_shadows_sky_light_and_removal_restores_it() {
        let mut lv = flat_level();
        // a 1x1 stone cap at y=70
        lv.set_tile(2, 70, 2, t::STONE, UPDATE_ALL);
        assert_eq!(lv.get_heightmap(2, 2), 71);
        assert_eq!(lv.get_brightness(LightLayer::Sky, 2, 71, 2), 15);
        // directly below the cap light only comes in sideways from the lit columns next to it
        assert_eq!(lv.get_brightness(LightLayer::Sky, 2, 69, 2), 14);
        assert_eq!(lv.get_brightness(LightLayer::Sky, 2, 66, 2), 14 - 0, "side light enters the open shaft");
        lv.set_tile(2, 70, 2, 0, UPDATE_ALL);
        assert_eq!(lv.get_heightmap(2, 2), 64);
        assert_eq!(lv.get_brightness(LightLayer::Sky, 2, 69, 2), 15);
        assert_eq!(lv.get_brightness(LightLayer::Sky, 2, 64, 2), 15);
    }

    #[test]
    fn digging_a_shaft_lights_it() {
        let mut lv = flat_level();
        for y in 55..64 {
            lv.set_tile(8, y, 8, 0, UPDATE_ALL);
        }
        assert_eq!(lv.get_heightmap(8, 8), 55);
        assert_eq!(lv.get_brightness(LightLayer::Sky, 8, 55, 8), 15);
        // sideways spill into the stone tunnel wall is blocked (stone lightBlock 255)
        assert_eq!(lv.get_brightness(LightLayer::Sky, 9, 58, 8), 0);
        // bottom of shaft re-closed
        lv.set_tile(8, 63, 8, t::GRASS, UPDATE_ALL);
        assert_eq!(lv.get_heightmap(8, 8), 64);
        assert_eq!(lv.get_brightness(LightLayer::Sky, 8, 58, 8), 0);
    }

    #[test]
    fn lighting_needs_all_neighbours_loaded() {
        let mut lv = Level::new(64, false);
        let mut c = LevelChunk::empty(0, 0);
        c.recalc_heightmap(false);
        lv.insert_chunk(c);
        lv.set_tile(1, 5, 1, t::GLOWSTONE, UPDATE_ALL);
        assert_eq!(lv.get_brightness(LightLayer::Block, 1, 5, 1), 0, "checkLight is skipped without the 17-block margin");
        lv.check_light_layer(LightLayer::Block, 1, 5, 1, true, false);
        assert_eq!(lv.get_brightness(LightLayer::Block, 1, 5, 1), 15);
    }

    #[test]
    fn nether_style_level_has_no_sky_light() {
        let mut lv = Level::new(64, true);
        for cx in -2..=2 {
            for cz in -2..=2 {
                lv.insert_chunk(LevelChunk::empty(cx, cz));
            }
        }
        lv.set_tile(0, 40, 0, t::GLOWSTONE, UPDATE_ALL);
        assert_eq!(lv.get_brightness(LightLayer::Sky, 0, 40, 0), 0);
        assert_eq!(lv.get_brightness(LightLayer::Block, 1, 40, 0), 14);
    }
}
