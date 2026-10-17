//! The slice of per-tile behaviour (`onPlace`, `neighborChanged`, `tick`, `canSurvive`, `mayPlace`)
//! that world generation depends on: liquids, small plants, reeds, cactus, falling blocks.
//! Everything that only matters during play (drops, sounds, particles, entities) is left out.

use crate::level::*;
use mc_core::Random;
use mc_world::tile_ids as t;
use mc_world::tile_props;
use mc_world::Material;

#[inline]
fn is_liquid_static(id: u8) -> bool {
    id == t::CALM_WATER || id == t::CALM_LAVA
}
#[inline]
fn is_liquid_dynamic(id: u8) -> bool {
    id == t::WATER || id == t::LAVA
}
#[inline]
#[allow(dead_code)]
fn is_liquid(id: u8) -> bool {
    is_liquid_static(id) || is_liquid_dynamic(id)
}
/// Material of the liquid family an id belongs to.
#[inline]
fn liquid_material(id: u8) -> Material {
    if id == t::WATER || id == t::CALM_WATER {
        Material::Water
    } else {
        Material::Lava
    }
}
#[inline]
fn is_bush(id: u8) -> bool {
    matches!(
        id,
        t::SAPLING | t::FLOWER | t::ROSE | t::MUSHROOM_BROWN | t::MUSHROOM_RED | t::TALLGRASS | t::DEAD_BUSH | t::WATER_LILY
    )
}

impl Level {
    // ------------------------------------------------------------ can survive / may place

    /// `Bush::mayPlaceOn` and its overrides.
    fn bush_may_place_on(&self, id: u8, below: u8) -> bool {
        match id {
            t::DEAD_BUSH => below == t::SAND,
            t::MUSHROOM_BROWN | t::MUSHROOM_RED => tile_props::is_solid_render(below),
            t::WATER_LILY => below == t::CALM_WATER,
            _ => below == t::GRASS || below == t::DIRT || below == t::FARMLAND,
        }
    }

    /// `Tile::canSurvive` for the tiles that override it; `true` for everything else.
    pub fn can_survive(&self, id: u8, x: i32, y: i32, z: i32) -> bool {
        match id {
            t::MUSHROOM_BROWN | t::MUSHROOM_RED => {
                if !(0..256).contains(&y) {
                    return false;
                }
                let below = self.get_tile(x, y - 1, z);
                below == t::MYCEL || (self.get_daytime_raw_brightness(x, y, z) < 13 && self.bush_may_place_on(id, below))
            }
            t::WATER_LILY => {
                if !(0..256).contains(&y) {
                    return false;
                }
                self.get_material(x, y - 1, z) == Material::Water && self.get_data(x, y - 1, z) == 0
            }
            t::SAPLING | t::FLOWER | t::ROSE | t::TALLGRASS | t::DEAD_BUSH => {
                (self.get_daytime_raw_brightness(x, y, z) >= 8 || self.can_see_sky(x, y, z))
                    && self.bush_may_place_on(id, self.get_tile(x, y - 1, z))
            }
            t::REEDS => self.may_place(id, x, y, z),
            t::CACTUS => {
                if self.get_material(x - 1, y, z).is_solid()
                    || self.get_material(x + 1, y, z).is_solid()
                    || self.get_material(x, y, z - 1).is_solid()
                    || self.get_material(x, y, z + 1).is_solid()
                {
                    return false;
                }
                let below = self.get_tile(x, y - 1, z);
                below == t::CACTUS || below == t::SAND
            }
            _ => true,
        }
    }

    /// `Tile::mayPlace(level, x, y, z)` for the tiles generation asks about.
    pub fn may_place(&self, id: u8, x: i32, y: i32, z: i32) -> bool {
        let here = self.get_tile(x, y, z);
        let base = here == 0 || tile_props::material(here).is_replaceable();
        match id {
            t::REEDS => {
                let below = self.get_tile(x, y - 1, z);
                if below == id {
                    return true;
                }
                if below != t::GRASS && below != t::DIRT && below != t::SAND {
                    return false;
                }
                self.get_material(x - 1, y - 1, z) == Material::Water
                    || self.get_material(x + 1, y - 1, z) == Material::Water
                    || self.get_material(x, y - 1, z - 1) == Material::Water
                    || self.get_material(x, y - 1, z + 1) == Material::Water
            }
            t::CACTUS => base && self.can_survive(id, x, y, z),
            t::PUMPKIN | t::LIT_PUMPKIN => base && self.is_top_solid_blocking(x, y - 1, z),
            t::MUSHROOM_BROWN | t::MUSHROOM_RED => {
                base && self.bush_may_place_on(id, self.get_tile(x, y - 1, z)) && self.can_survive(id, x, y, z)
            }
            t::WATER_LILY => base && self.bush_may_place_on(id, self.get_tile(x, y - 1, z)),
            t::SAPLING | t::FLOWER | t::ROSE | t::TALLGRASS | t::DEAD_BUSH => {
                base && self.bush_may_place_on(id, self.get_tile(x, y - 1, z))
            }
            _ => base,
        }
    }

    // ------------------------------------------------------------ biome / weather queries

    /// `Level::getBiome`: the chunk's stored biome, plains if unknown.
    pub fn get_biome(&self, x: i32, z: i32) -> &'static mc_world::Biome {
        let id = self
            .chunk(x >> 4, z >> 4)
            .map(|c| c.biomes[(((z & 15) << 4) | (x & 15)) as usize])
            .unwrap_or(0xff);
        mc_world::biome::get(if id == 0xff { 1 } else { id as i32 }).unwrap_or_else(|| mc_world::biome::get(1).unwrap())
    }

    /// `Level::shouldFreeze(x, y, z, checkNeighbors)`
    pub fn should_freeze(&self, x: i32, y: i32, z: i32, check_neighbors: bool) -> bool {
        let temp = self.get_biome(x, z).temperature;
        if temp > 0.15 {
            return false;
        }
        if (0..256).contains(&y) && self.get_brightness(crate::chunk::LightLayer::Block, x, y, z) < 10 {
            let cur = self.get_tile(x, y, z);
            if (cur == t::CALM_WATER || cur == t::WATER) && self.get_data(x, y, z) == 0 {
                if !check_neighbors {
                    return true;
                }
                let surrounded = self.get_material(x - 1, y, z) == Material::Water
                    && self.get_material(x + 1, y, z) == Material::Water
                    && self.get_material(x, y, z - 1) == Material::Water
                    && self.get_material(x, y, z + 1) == Material::Water;
                if !surrounded {
                    return true;
                }
            }
        }
        false
    }

    pub fn should_freeze_ignore_neighbors(&self, x: i32, y: i32, z: i32) -> bool {
        self.should_freeze(x, y, z, false)
    }

    /// `TopSnowTile::mayPlace`
    pub fn top_snow_may_place(&self, x: i32, y: i32, z: i32) -> bool {
        let tt = self.get_tile(x, y - 1, z);
        if tt == 0 {
            return false;
        }
        if tt == t::TOP_SNOW && (self.get_data(x, y - 1, z) & 7) == 6 + 1 {
            return true;
        }
        if tt != t::LEAVES && !tile_props::is_solid_render(tt) {
            return false;
        }
        tile_props::material(tt).blocks_motion()
    }

    /// `Level::shouldSnow`
    pub fn should_snow(&self, x: i32, y: i32, z: i32) -> bool {
        if self.get_biome(x, z).temperature > 0.15 {
            return false;
        }
        if (0..256).contains(&y) && self.get_brightness(crate::chunk::LightLayer::Block, x, y, z) < 10 {
            let below = self.get_tile(x, y - 1, z);
            if self.get_tile(x, y, z) == 0
                && self.top_snow_may_place(x, y, z)
                && below != 0
                && below != t::ICE
                && tile_props::material(below).blocks_motion()
            {
                return true;
            }
        }
        false
    }

    /// `Tile::isCubeShaped` approximated for the neighbours vines care about.
    pub fn is_cube_shaped(&self, id: u8) -> bool {
        id != 0 && (tile_props::is_solid_render(id) || matches!(id, t::LEAVES | t::GLASS | t::STAINED_GLASS | t::ICE))
    }

    /// `VineTile::mayPlace(level, x, y, z, face)`
    pub fn vine_may_place(&self, x: i32, y: i32, z: i32, face: i32) -> bool {
        let ok = |id: u8| self.is_cube_shaped(id) && tile_props::material(id).blocks_motion();
        match face {
            1 => ok(self.get_tile(x, y + 1, z)),
            2 => ok(self.get_tile(x, y, z + 1)),
            3 => ok(self.get_tile(x, y, z - 1)),
            5 => ok(self.get_tile(x - 1, y, z)),
            4 => ok(self.get_tile(x + 1, y, z)),
            _ => false,
        }
    }

    // ------------------------------------------------------------ callbacks

    /// `Tile::onPlace`
    pub(crate) fn on_place(&mut self, x: i32, y: i32, z: i32, id: u8) {
        if is_liquid_dynamic(id) {
            self.update_liquid(x, y, z, id);
            if self.get_tile(x, y, z) == id {
                let d = self.liquid_tick_delay(id);
                self.add_to_tick_next_tick(x, y, z, id, d);
            }
        } else if is_liquid_static(id) {
            self.update_liquid(x, y, z, id);
        } else if id == t::SAND || id == t::GRAVEL || id == t::ANVIL {
            self.add_to_tick_next_tick(x, y, z, id, 2);
        } else if id == t::CHEST {
            self.chest_on_place(x, y, z);
        }
    }

    /// `ChestTile::onPlace`
    fn chest_on_place(&mut self, x: i32, y: i32, z: i32) {
        let id = t::CHEST;
        self.chest_recalc_lock_dir(x, y, z);
        if self.get_tile(x, y, z - 1) == id {
            self.chest_recalc_lock_dir(x, y, z - 1);
        }
        if self.get_tile(x, y, z + 1) == id {
            self.chest_recalc_lock_dir(x, y, z + 1);
        }
        if self.get_tile(x - 1, y, z) == id {
            self.chest_recalc_lock_dir(x - 1, y, z);
        }
        if self.get_tile(x + 1, y, z) == id {
            self.chest_recalc_lock_dir(x + 1, y, z);
        }
    }

    /// `ChestTile::recalcLockDir`
    fn chest_recalc_lock_dir(&mut self, x: i32, y: i32, z: i32) {
        let id = t::CHEST;
        let n = self.get_tile(x, y, z - 1);
        let s = self.get_tile(x, y, z + 1);
        let w = self.get_tile(x - 1, y, z);
        let e = self.get_tile(x + 1, y, z);
        let solid = |v: u8| tile_props::is_solid_render(v);
        let lock_dir;
        if n == id || s == id {
            let zz = if n == id { z - 1 } else { z + 1 };
            let w2 = self.get_tile(x - 1, y, zz);
            let e2 = self.get_tile(x + 1, y, zz);
            let mut d = 5;
            let other = self.get_data(x, y, zz);
            if other == 4 {
                d = 4;
            }
            if (solid(w) || solid(w2)) && !solid(e) && !solid(e2) {
                d = 5;
            }
            if (solid(e) || solid(e2)) && !solid(w) && !solid(w2) {
                d = 4;
            }
            lock_dir = d;
        } else if w == id || e == id {
            let xx = if w == id { x - 1 } else { x + 1 };
            let n2 = self.get_tile(xx, y, z - 1);
            let s2 = self.get_tile(xx, y, z + 1);
            let mut d = 3;
            let other = self.get_data(xx, y, z);
            if other == 2 {
                d = 2;
            }
            if (solid(n) || solid(n2)) && !solid(s) && !solid(s2) {
                d = 3;
            }
            if (solid(s) || solid(s2)) && !solid(n) && !solid(n2) {
                d = 2;
            }
            lock_dir = d;
        } else {
            let mut d = 3;
            if solid(n) && !solid(s) {
                d = 3;
            }
            if solid(s) && !solid(n) {
                d = 2;
            }
            if solid(w) && !solid(e) {
                d = 5;
            }
            if solid(e) && !solid(w) {
                d = 4;
            }
            lock_dir = d;
        }
        self.set_data(x, y, z, lock_dir, UPDATE_ALL);
    }

    /// `Level::neighborChanged`
    pub(crate) fn neighbor_changed(&mut self, x: i32, y: i32, z: i32, from: u8) {
        let id = self.get_tile(x, y, z);
        if id == 0 {
            return;
        }
        if is_liquid_static(id) {
            self.update_liquid(x, y, z, id);
            if self.get_tile(x, y, z) == id {
                // setDynamic
                let d = self.get_data(x, y, z);
                self.set_tile_and_data(x, y, z, id - 1, d, UPDATE_CLIENTS);
                let delay = self.liquid_tick_delay(id - 1);
                self.add_to_tick_next_tick(x, y, z, id - 1, delay);
            }
        } else if is_liquid_dynamic(id) {
            self.update_liquid(x, y, z, id);
        } else if is_bush(id) {
            if !self.can_survive(id, x, y, z) {
                self.set_tile_and_data(x, y, z, 0, 0, UPDATE_CLIENTS);
            }
        } else if id == t::REEDS {
            if !self.can_survive(id, x, y, z) {
                self.remove_tile(x, y, z);
            }
        } else if id == t::CACTUS {
            if !self.can_survive(id, x, y, z) {
                self.destroy_tile(x, y, z);
            }
        } else if id == t::SAND || id == t::GRAVEL || id == t::ANVIL {
            self.add_to_tick_next_tick(x, y, z, id, 2);
        }
        let _ = from;
    }

    /// `ServerLevel::addToTickNextTick`
    pub fn add_to_tick_next_tick(&mut self, x: i32, y: i32, z: i32, id: u8, mut delay: i32) {
        if self.insta_tick && id > 0 {
            // Tile::canInstantlyTick is true for every tile except fire
            if id != t::FIRE {
                if self.has_chunks_in(x - 8, y - 8, z - 8, x + 8, y + 8, z + 8) && self.get_tile(x, y, z) == id {
                    let mut r = std::mem::replace(&mut self.random, Random::with_seed(0));
                    self.tick_tile(x, y, z, id, &mut r);
                    self.random = r;
                }
                return;
            }
            delay = 1;
        }
        if self.has_chunks_in(x, y, z, x, y, z) && id > 0 {
            let p = PendingTick { x, y, z, tile: id, time: self.game_time + delay as i64 };
            if !self.pending_ticks.contains(&p) {
                self.pending_ticks.push(p);
            }
        }
    }

    /// `Tile::tick` for the tiles generation can trigger.
    pub fn tick_tile(&mut self, x: i32, y: i32, z: i32, id: u8, random: &mut Random) {
        if is_liquid_dynamic(id) {
            self.liquid_dynamic_tick(x, y, z, id, random);
        } else if id == t::CALM_LAVA {
            self.lava_static_tick(x, y, z, random);
        } else if id == t::SAND || id == t::GRAVEL || id == t::ANVIL {
            self.heavy_check_slide(x, y, z, id);
        }
    }

    fn heavy_is_free(&self, x: i32, y: i32, z: i32) -> bool {
        let tt = self.get_tile(x, y, z);
        if tt == 0 || tt == t::FIRE {
            return true;
        }
        let m = tile_props::material(tt);
        m == Material::Water || m == Material::Lava
    }

    /// `HeavyTile::checkSlide` (only the instant-fall branch; falling-entity spawning is not simulated)
    fn heavy_check_slide(&mut self, x: i32, mut y: i32, z: i32, id: u8) {
        if self.heavy_is_free(x, y - 1, z) && y >= 0 {
            let r = 32;
            if self.insta_fall || !self.has_chunks_in(x - r, y - r, z - r, x + r, y + r, z + r) {
                self.remove_tile(x, y, z);
                while self.heavy_is_free(x, y - 1, z) && y > 0 {
                    y -= 1;
                }
                if y > 0 {
                    self.set_tile_and_update(x, y, z, id);
                }
            } else {
                // the caller turns this into a falling-block entity
                self.fall_requests.push((x, y, z));
            }
        }
    }

    // ------------------------------------------------------------ liquids

    fn liquid_tick_delay(&self, id: u8) -> i32 {
        if liquid_material(id) == Material::Water {
            5
        } else if self.has_ceiling {
            10
        } else {
            30
        }
    }

    fn liquid_depth(&self, id: u8, x: i32, y: i32, z: i32) -> i32 {
        if self.get_material(x, y, z) == liquid_material(id) {
            self.get_data(x, y, z) as i32
        } else {
            -1
        }
    }

    /// `LiquidTile::updateLiquid`: lava touching water becomes obsidian / cobblestone.
    fn update_liquid(&mut self, x: i32, y: i32, z: i32, id: u8) {
        if self.get_tile(x, y, z) != id || liquid_material(id) != Material::Lava {
            return;
        }
        let water = self.get_material(x, y, z - 1) == Material::Water
            || self.get_material(x, y, z + 1) == Material::Water
            || self.get_material(x - 1, y, z) == Material::Water
            || self.get_material(x + 1, y, z) == Material::Water
            || self.get_material(x, y + 1, z) == Material::Water;
        if water {
            let data = self.get_data(x, y, z);
            if data == 0 {
                self.set_tile_and_update(x, y, z, t::OBSIDIAN);
            } else if data <= 4 {
                self.set_tile_and_update(x, y, z, t::COBBLESTONE);
            }
            // fizz(): sound and particles only
        }
    }

    /// `LiquidTileStatic::tick` (only lava ticks): occasionally ignites flammable neighbours.
    fn lava_static_tick(&mut self, mut x: i32, mut y: i32, mut z: i32, random: &mut Random) {
        let h = random.next_int_bounded(3);
        for _ in 0..h {
            x += random.next_int_bounded(3) - 1;
            y += 1;
            z += random.next_int_bounded(3) - 1;
            let tt = self.get_tile(x, y, z);
            if tt == 0 {
                let fl = |l: &Level, a, b, c| l.get_material(a, b, c).is_flammable();
                if fl(self, x - 1, y, z)
                    || fl(self, x + 1, y, z)
                    || fl(self, x, y, z - 1)
                    || fl(self, x, y, z + 1)
                    || fl(self, x, y - 1, z)
                    || fl(self, x, y + 1, z)
                {
                    self.set_tile_and_update(x, y, z, t::FIRE);
                    return;
                }
            } else if tile_props::material(tt).blocks_motion() {
                return;
            }
        }
        if h == 0 {
            let (ox, oz) = (x, z);
            for _ in 0..3 {
                x = ox + random.next_int_bounded(3) - 1;
                z = oz + random.next_int_bounded(3) - 1;
                if self.is_empty_tile(x, y + 1, z) && self.get_material(x, y, z).is_flammable() {
                    self.set_tile_and_update(x, y + 1, z, t::FIRE);
                }
            }
        }
    }

    /// `LiquidTileDynamic::tick` including the 4J iterative insta-tick queue.
    fn liquid_dynamic_tick(&mut self, x: i32, y: i32, z: i32, id: u8, random: &mut Random) {
        let k = if id == t::WATER { 0 } else { 1 };
        if !self.liq_iterative[k] && self.insta_tick {
            self.liq_iterative[k] = true;
            self.liq_queue[k].push_back((x, y, z));
            let mut failsafe = 100;
            while failsafe > 0 {
                let Some((qx, qy, qz)) = self.liq_queue[k].pop_front() else { break };
                self.liquid_main_tick(qx, qy, qz, id, random);
                failsafe -= 1;
            }
            self.liq_queue[k].clear();
            self.liq_iterative[k] = false;
        } else if self.liq_iterative[k] && self.insta_tick {
            self.liq_queue[k].push_back((x, y, z));
        } else {
            self.liquid_main_tick(x, y, z, id, random);
        }
    }

    fn liquid_main_tick(&mut self, x: i32, y: i32, z: i32, id: u8, random: &mut Random) {
        let mat = liquid_material(id);
        let mut depth = self.liquid_depth(id, x, y, z);
        let mut drop_off = 1;
        if mat == Material::Lava && !self.ultra_warm {
            drop_off = 2;
        }
        let mut tick_delay = self.liquid_tick_delay(id);
        if depth > 0 {
            let mut highest = -100;
            let mut max_count = 0;
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                highest = self.liquid_highest(id, x + dx, y, z + dz, highest, &mut max_count);
            }
            let mut new_depth = highest + drop_off;
            if new_depth >= 8 || highest < 0 {
                new_depth = -1;
            }
            if self.liquid_depth(id, x, y + 1, z) >= 0 {
                let above = self.liquid_depth(id, x, y + 1, z);
                new_depth = if above >= 8 { above } else { above + 8 };
            }
            if max_count >= 2 && mat == Material::Water {
                if self.get_material(x, y - 1, z).is_solid() {
                    new_depth = 0;
                } else if self.get_material(x, y - 1, z) == mat && self.get_data(x, y - 1, z) == 0 {
                    new_depth = 0;
                }
            }
            if mat == Material::Lava && depth < 8 && new_depth < 8 && new_depth > depth && random.next_int_bounded(4) != 0 {
                tick_delay *= 4;
            }
            if new_depth == depth {
                self.liquid_set_static(x, y, z, id);
            } else {
                depth = new_depth;
                if depth < 0 {
                    self.remove_tile(x, y, z);
                } else {
                    self.set_data(x, y, z, depth as u8, UPDATE_CLIENTS);
                    self.add_to_tick_next_tick(x, y, z, id, tick_delay);
                    self.update_neighbors_at(x, y, z, id);
                }
            }
        } else {
            self.liquid_set_static(x, y, z, id);
        }

        if self.liquid_can_spread_to(id, x, y - 1, z) {
            if mat == Material::Lava && self.get_material(x, y - 1, z) == Material::Water {
                self.set_tile_and_update(x, y - 1, z, t::STONE);
                return;
            }
            if depth >= 8 {
                self.liquid_try_spread_to(id, x, y - 1, z, depth);
            } else {
                self.liquid_try_spread_to(id, x, y - 1, z, depth + 8);
            }
        } else if depth >= 0 && (depth == 0 || self.is_water_blocking(x, y - 1, z)) {
            let spreads = self.liquid_spread(id, x, y, z);
            let mut neighbor = depth + drop_off;
            if depth >= 8 {
                neighbor = 1;
            }
            if neighbor >= 8 {
                return;
            }
            if spreads[0] {
                self.liquid_try_spread_to(id, x - 1, y, z, neighbor);
            }
            if spreads[1] {
                self.liquid_try_spread_to(id, x + 1, y, z, neighbor);
            }
            if spreads[2] {
                self.liquid_try_spread_to(id, x, y, z - 1, neighbor);
            }
            if spreads[3] {
                self.liquid_try_spread_to(id, x, y, z + 1, neighbor);
            }
        }
    }

    fn liquid_set_static(&mut self, x: i32, y: i32, z: i32, id: u8) {
        let d = self.get_data(x, y, z);
        self.set_tile_and_data(x, y, z, id + 1, d, UPDATE_CLIENTS);
    }

    fn liquid_highest(&self, id: u8, x: i32, y: i32, z: i32, current: i32, max_count: &mut i32) -> i32 {
        let mut d = self.liquid_depth(id, x, y, z);
        if d < 0 {
            return current;
        }
        if d == 0 {
            *max_count += 1;
        }
        if d >= 8 {
            d = 0;
        }
        if current < 0 || d < current {
            d
        } else {
            current
        }
    }

    fn liquid_can_spread_to(&self, id: u8, x: i32, y: i32, z: i32) -> bool {
        let half = self.xz_size / 2;
        let (ix, iz) = ((x >> 4) + half, (z >> 4) + half);
        if ix < 0 || ix >= self.xz_size || iz < 0 || iz >= self.xz_size {
            return false;
        }
        let target = self.get_material(x, y, z);
        if target == liquid_material(id) || target == Material::Lava {
            return false;
        }
        !self.is_water_blocking(x, y, z)
    }

    fn liquid_try_spread_to(&mut self, id: u8, x: i32, y: i32, z: i32, neighbor: i32) {
        if self.liquid_can_spread_to(id, x, y, z) {
            // old tile: lava fizzes, water would drop resources — neither matters here
            self.set_tile_and_data(x, y, z, id, neighbor as u8, UPDATE_ALL);
        }
    }

    pub(crate) fn is_water_blocking(&self, x: i32, y: i32, z: i32) -> bool {
        let tt = self.get_tile(x, y, z);
        if matches!(tt, t::DOOR_WOOD | t::DOOR_IRON | t::SIGN | t::LADDER | t::REEDS) {
            return true;
        }
        if tt == 0 {
            return false;
        }
        let m = tile_props::material(tt);
        m == Material::Portal || m.blocks_motion()
    }

    fn liquid_slope_distance(&self, id: u8, x: i32, y: i32, z: i32, pass: i32, from: usize) -> i32 {
        let mat = liquid_material(id);
        let mut lowest = 1000;
        for d in 0..4usize {
            if (d == 0 && from == 1) || (d == 1 && from == 0) || (d == 2 && from == 3) || (d == 3 && from == 2) {
                continue;
            }
            let (mut xx, yy, mut zz) = (x, y, z);
            match d {
                0 => xx -= 1,
                1 => xx += 1,
                2 => zz -= 1,
                _ => zz += 1,
            }
            if self.is_water_blocking(xx, yy, zz) {
                continue;
            } else if self.get_material(xx, yy, zz) == mat && self.get_data(xx, yy, zz) == 0 {
                continue;
            } else if self.is_water_blocking(xx, yy - 1, zz) {
                if pass < 4 {
                    let v = self.liquid_slope_distance(id, xx, yy, zz, pass + 1, d);
                    if v < lowest {
                        lowest = v;
                    }
                }
            } else {
                return pass;
            }
        }
        lowest
    }

    fn liquid_spread(&self, id: u8, x: i32, y: i32, z: i32) -> [bool; 4] {
        let mat = liquid_material(id);
        let mut dist = [1000i32; 4];
        for d in 0..4usize {
            let (mut xx, yy, mut zz) = (x, y, z);
            match d {
                0 => xx -= 1,
                1 => xx += 1,
                2 => zz -= 1,
                _ => zz += 1,
            }
            if self.is_water_blocking(xx, yy, zz) {
                continue;
            } else if self.get_material(xx, yy, zz) == mat && self.get_data(xx, yy, zz) == 0 {
                continue;
            }
            dist[d] = if self.is_water_blocking(xx, yy - 1, zz) { self.liquid_slope_distance(id, xx, yy, zz, 1, d) } else { 0 };
        }
        let lowest = *dist.iter().min().unwrap();
        [dist[0] == lowest, dist[1] == lowest, dist[2] == lowest, dist[3] == lowest]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::LevelChunk;

    /// stone to y=59, air above, 5x5 chunks.
    fn cave_level() -> Level {
        let mut lv = Level::new(64, false);
        lv.lighting = false;
        for cx in -2..=2 {
            for cz in -2..=2 {
                let mut b = vec![0u8; crate::storage::SECTION_TILES];
                for x in 0..16usize {
                    for z in 0..16usize {
                        for y in 0..60usize {
                            b[(x << 11) | (z << 7) | y] = t::STONE;
                        }
                    }
                }
                let mut c = LevelChunk::from_blocks(cx, cz, &b);
                c.recalc_heightmap(false);
                lv.insert_chunk(c);
            }
        }
        lv
    }

    #[test]
    fn water_source_spreads_on_flat_floor_then_goes_static() {
        let mut lv = cave_level();
        lv.insta_tick = true;
        lv.set_tile_and_data(0, 60, 0, t::WATER, 0, UPDATE_CLIENTS);
        lv.insta_tick = false;
        // a source block spreads to depth 1..7 around it on the floor
        assert_eq!(lv.get_data(1, 60, 0), 1);
        assert_eq!(lv.get_data(3, 60, 0), 3);
        // the iterative insta-tick has a failsafe of 100 ticks, so the flow stops at depth 5 and
        // the outer ring is left as still-flowing water
        assert_eq!(lv.get_data(5, 60, 0), 5);
        assert_eq!(lv.get_tile(6, 60, 0), 0);
        assert_eq!(lv.get_tile(1, 60, 0), t::CALM_WATER);
        assert_eq!(lv.get_tile(5, 60, 0), t::WATER);
    }

    #[test]
    fn lava_meeting_water_makes_obsidian_and_cobble() {
        let mut lv = cave_level();
        lv.set_tile_and_data(0, 60, 0, t::CALM_WATER, 0, UPDATE_CLIENTS);
        lv.set_tile_and_data(1, 60, 0, t::CALM_LAVA, 0, UPDATE_CLIENTS);
        assert_eq!(lv.get_tile(1, 60, 0), t::OBSIDIAN);
        lv.set_tile_and_data(0, 60, 5, t::CALM_WATER, 0, UPDATE_CLIENTS);
        lv.set_tile_and_data(1, 60, 5, t::CALM_LAVA, 3, UPDATE_CLIENTS);
        assert_eq!(lv.get_tile(1, 60, 5), t::COBBLESTONE);
    }

    #[test]
    fn plants_pop_off_when_support_is_removed() {
        let mut lv = cave_level();
        lv.set_tile_and_data(2, 59, 2, t::GRASS, 0, UPDATE_CLIENTS);
        // chunk heightmap was computed before; refresh sky light so the flower can survive
        lv.lighting = true;
        for cx in -2..=2 {
            for cz in -2..=2 {
                lv.recheck_gaps(cx, cz, true);
            }
        }
        lv.set_tile_and_data(2, 60, 2, t::FLOWER, 0, UPDATE_CLIENTS);
        assert_eq!(lv.get_tile(2, 60, 2), t::FLOWER);
        lv.set_tile_and_data(2, 59, 2, 0, 0, UPDATE_ALL);
        assert_eq!(lv.get_tile(2, 60, 2), 0, "flower without grass is removed by neighborChanged");
    }

    #[test]
    fn cactus_and_reeds_rules() {
        let mut lv = cave_level();
        lv.set_tile_and_data(0, 59, 0, t::SAND, 0, UPDATE_NONE);
        assert!(lv.may_place(t::CACTUS, 0, 60, 0));
        lv.set_tile_and_data(1, 60, 0, t::STONE, 0, UPDATE_NONE);
        assert!(!lv.may_place(t::CACTUS, 0, 60, 0), "solid neighbour blocks cactus");
        // reeds: need water beside the soil block
        lv.set_tile_and_data(5, 59, 5, t::DIRT, 0, UPDATE_NONE);
        assert!(!lv.may_place(t::REEDS, 5, 60, 5));
        lv.set_tile_and_data(6, 59, 5, t::CALM_WATER, 0, UPDATE_NONE);
        assert!(lv.may_place(t::REEDS, 5, 60, 5));
    }
}
