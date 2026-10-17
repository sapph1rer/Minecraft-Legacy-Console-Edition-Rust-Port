//! Port of `Biome` and its subclasses' constructors — the 23 biome definitions.
//!
//! Each subclass only changed a few fields in its constructor (surface blocks, `BiomeDecorator`
//! counts, mob lists), so a single data struct is enough. What is *not* here:
//! * mob spawn lists (`enemies`, `friendlies`, …) — belong with the entity layer;
//! * grass/foliage/water/sky colours — they come from the texture pack's colour table;
//! * the decorator's behaviour (`BiomeDecorator::decorate`) — it needs a `Level`, layer 5.
//!   Only its per-biome *counts* are kept, in [`DecoratorConfig`].
//!
//! The table is built with `const fn` builders in the same order as `Biome::staticCtor`, so
//! later setters override earlier ones exactly like the fluent C++ calls.

use crate::tile::ids;

pub const BIOME_COUNT: usize = 23;

/// Biome ids (`Biome::xxx->id`).
pub mod id {
    pub const OCEAN: u8 = 0;
    pub const PLAINS: u8 = 1;
    pub const DESERT: u8 = 2;
    pub const EXTREME_HILLS: u8 = 3;
    pub const FOREST: u8 = 4;
    pub const TAIGA: u8 = 5;
    pub const SWAMPLAND: u8 = 6;
    pub const RIVER: u8 = 7;
    pub const HELL: u8 = 8;
    pub const SKY: u8 = 9;
    pub const FROZEN_OCEAN: u8 = 10;
    pub const FROZEN_RIVER: u8 = 11;
    pub const ICE_FLATS: u8 = 12;
    pub const ICE_MOUNTAINS: u8 = 13;
    pub const MUSHROOM_ISLAND: u8 = 14;
    pub const MUSHROOM_ISLAND_SHORE: u8 = 15;
    pub const BEACHES: u8 = 16;
    pub const DESERT_HILLS: u8 = 17;
    pub const FOREST_HILLS: u8 = 18;
    pub const TAIGA_HILLS: u8 = 19;
    pub const SMALLER_EXTREME_HILLS: u8 = 20;
    pub const JUNGLE: u8 = 21;
    pub const JUNGLE_HILLS: u8 = 22;
}

/// Per-biome counts from `BiomeDecorator`'s constructor, as modified by the biome subclass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecoratorConfig {
    pub waterlily_count: i32,
    pub tree_count: i32,
    pub flower_count: i32,
    pub grass_count: i32,
    pub dead_bush_count: i32,
    pub mushroom_count: i32,
    pub reeds_count: i32,
    pub cactus_count: i32,
    pub gravel_count: i32,
    pub sand_count: i32,
    pub clay_count: i32,
    pub huge_mushrooms: i32,
}

impl DecoratorConfig {
    /// `BiomeDecorator::BiomeDecorator` defaults.
    pub const DEFAULT: DecoratorConfig = DecoratorConfig {
        waterlily_count: 0,
        tree_count: 0,
        flower_count: 2,
        grass_count: 1,
        dead_bush_count: 0,
        mushroom_count: 0,
        reeds_count: 0,
        cactus_count: 0,
        gravel_count: 1,
        sand_count: 3,
        clay_count: 1,
        huge_mushrooms: 0,
    };
}

#[derive(Clone, Copy, Debug)]
pub struct Biome {
    pub id: u8,
    pub name: &'static str,
    /// Debug/map colour (`setColor`).
    pub color: u32,
    /// `topMaterial` — block id of the surface layer.
    pub top_material: u8,
    /// `material` — block id under the surface.
    pub material: u8,
    pub leaf_color: u32,
    pub depth: f32,
    pub scale: f32,
    pub temperature: f32,
    pub downfall: f32,
    has_rain: bool,
    /// `setSnowCovered` — recorded but unused: the 4J build decides snow from temperature.
    pub snow_covered: bool,
    pub decorator: DecoratorConfig,
}

impl Biome {
    /// `Biome::Biome(id)` defaults.
    const fn new(id: u8) -> Biome {
        Biome {
            id,
            name: "",
            color: 0,
            top_material: ids::GRASS,
            material: ids::DIRT,
            leaf_color: 0x4EE031,
            depth: 0.1,
            scale: 0.3,
            temperature: 0.5,
            downfall: 0.5,
            has_rain: true,
            snow_covered: false,
            decorator: DecoratorConfig::DEFAULT,
        }
    }
    const fn color(mut self, c: u32) -> Biome {
        self.color = c;
        self
    }
    const fn name(mut self, n: &'static str) -> Biome {
        self.name = n;
        self
    }
    const fn leaf(mut self, c: u32) -> Biome {
        self.leaf_color = c;
        self
    }
    const fn temp(mut self, t: f32, d: f32) -> Biome {
        self.temperature = t;
        self.downfall = d;
        self
    }
    const fn depth_scale(mut self, d: f32, s: f32) -> Biome {
        self.depth = d;
        self.scale = s;
        self
    }
    const fn no_rain(mut self) -> Biome {
        self.has_rain = false;
        self
    }
    const fn snow(mut self) -> Biome {
        self.snow_covered = true;
        self
    }
    const fn surface(mut self, top: u8, mat: u8) -> Biome {
        self.top_material = top;
        self.material = mat;
        self
    }
    const fn deco(mut self, d: DecoratorConfig) -> Biome {
        self.decorator = d;
        self
    }

    // --- subclass constructors -------------------------------------------------------------
    const fn desert(id: u8) -> Biome {
        let mut d = DecoratorConfig::DEFAULT;
        d.tree_count = -999;
        d.dead_bush_count = 2;
        d.reeds_count = 50;
        d.cactus_count = 10;
        Biome::new(id).surface(ids::SAND, ids::SAND).deco(d)
    }
    const fn beach(id: u8) -> Biome {
        let mut d = DecoratorConfig::DEFAULT;
        d.tree_count = -999;
        d.dead_bush_count = 0;
        d.reeds_count = 0;
        d.cactus_count = 0;
        Biome::new(id).surface(ids::SAND, ids::SAND).deco(d)
    }
    const fn plains(id: u8) -> Biome {
        let mut d = DecoratorConfig::DEFAULT;
        d.tree_count = -999;
        d.flower_count = 4;
        d.grass_count = 10;
        Biome::new(id).deco(d)
    }
    const fn forest(id: u8) -> Biome {
        let mut d = DecoratorConfig::DEFAULT;
        d.tree_count = 10;
        d.grass_count = 2;
        Biome::new(id).deco(d)
    }
    const fn taiga(id: u8) -> Biome {
        let mut d = DecoratorConfig::DEFAULT;
        d.tree_count = 10;
        d.grass_count = 1;
        Biome::new(id).deco(d)
    }
    const fn swamp(id: u8) -> Biome {
        let mut d = DecoratorConfig::DEFAULT;
        d.tree_count = 2;
        d.flower_count = -999;
        d.dead_bush_count = 1;
        d.mushroom_count = 8;
        d.reeds_count = 10;
        d.clay_count = 1;
        d.waterlily_count = 4;
        Biome::new(id).deco(d)
    }
    const fn jungle(id: u8) -> Biome {
        let mut d = DecoratorConfig::DEFAULT;
        d.tree_count = 50;
        d.grass_count = 25;
        d.flower_count = 4;
        Biome::new(id).deco(d)
    }
    const fn mushroom_island(id: u8) -> Biome {
        let mut d = DecoratorConfig::DEFAULT;
        d.tree_count = -100;
        d.flower_count = -100;
        d.grass_count = -100;
        d.mushroom_count = 1;
        d.huge_mushrooms = 1;
        // `materialSet` stays dirt; only the top changes
        Biome::new(id).surface(ids::MYCEL, ids::DIRT).deco(d)
    }
    const fn the_end(id: u8) -> Biome {
        // TheEndBiomeDecorator is used instead of the normal one (no counts apply).
        Biome::new(id).surface(ids::DIRT, ids::DIRT)
    }

    // --- queries --------------------------------------------------------------------------
    /// `Biome::hasSnow` — snow is temperature driven in this build.
    pub fn has_snow(&self) -> bool {
        if !self.has_rain {
            return false;
        }
        self.temperature < 0.15
    }

    pub fn has_rain(&self) -> bool {
        if self.has_snow() {
            return false;
        }
        self.has_rain
    }

    pub fn is_humid(&self) -> bool {
        self.downfall > 0.85
    }

    pub fn creature_probability(&self) -> f32 {
        0.1
    }

    /// `getDownfallInt`: `(int)(downfall * 65536)` in `f32`.
    pub fn downfall_int(&self) -> i32 {
        (self.downfall * 65536.0) as i32
    }

    pub fn temperature_int(&self) -> i32 {
        (self.temperature * 65536.0) as i32
    }
}

/// Mirrors `Biome::staticCtor`, indexed by biome id.
pub static BIOMES: [Biome; BIOME_COUNT] = [
    Biome::new(0).color(0x000070).name("Ocean").depth_scale(-1.0, 0.4),
    Biome::plains(1).color(0x8db360).name("Plains").temp(0.8, 0.4),
    Biome::desert(2).color(0xFA9418).name("Desert").no_rain().temp(2.0, 0.0).depth_scale(0.1, 0.2),
    Biome::new(3).color(0x606060).name("Extreme Hills").depth_scale(0.3, 1.5).temp(0.2, 0.3),
    Biome::forest(4).color(0x056621).name("Forest").leaf(0x4EBA31).temp(0.7, 0.8),
    // 4J - brought forward Taiga temperature change from 0.3f to 0.05f, from 1.2.3
    Biome::taiga(5).color(0x0b6659).name("Taiga").leaf(0x4EBA31).snow().temp(0.05, 0.8).depth_scale(0.1, 0.4),
    Biome::swamp(6).color(0x07F9B2).name("Swampland").leaf(0x8BAF48).depth_scale(-0.2, 0.1).temp(0.8, 0.9),
    Biome::new(7).color(0x0000ff).name("River").depth_scale(-0.5, 0.0),
    Biome::new(8).color(0xff0000).name("Hell").no_rain().temp(2.0, 0.0),
    Biome::the_end(9).color(0x8080ff).name("Sky").no_rain(),
    Biome::new(10).color(0x9090a0).name("FrozenOcean").snow().depth_scale(-1.0, 0.5).temp(0.0, 0.5),
    Biome::new(11).color(0xa0a0ff).name("FrozenRiver").snow().depth_scale(-0.5, 0.0).temp(0.0, 0.5),
    Biome::new(12).color(0xffffff).name("Ice Plains").snow().temp(0.0, 0.5),
    Biome::new(13).color(0xa0a0a0).name("Ice Mountains").snow().depth_scale(0.3, 1.3).temp(0.0, 0.5),
    Biome::mushroom_island(14).color(0xff00ff).name("MushroomIsland").temp(0.9, 1.0).depth_scale(0.2, 1.0),
    Biome::mushroom_island(15).color(0xa000ff).name("MushroomIslandShore").temp(0.9, 1.0).depth_scale(-1.0, 0.1),
    Biome::beach(16).color(0xfade55).name("Beach").temp(0.8, 0.4).depth_scale(0.0, 0.1),
    Biome::desert(17).color(0xd25f12).name("DesertHills").no_rain().temp(2.0, 0.0).depth_scale(0.3, 0.8),
    Biome::forest(18).color(0x22551c).name("ForestHills").leaf(0x4EBA31).temp(0.7, 0.8).depth_scale(0.3, 0.7),
    Biome::taiga(19).color(0x163933).name("TaigaHills").snow().leaf(0x4EBA31).temp(0.05, 0.8).depth_scale(0.3, 0.8),
    Biome::new(20).color(0x72789a).name("Extreme Hills Edge").depth_scale(0.2, 0.8).temp(0.2, 0.3),
    Biome::jungle(21).color(0x537b09).name("Jungle").leaf(0x537b09).temp(1.2, 0.9).depth_scale(0.2, 0.4),
    Biome::jungle(22).color(0x2c4205).name("JungleHills").leaf(0x537b09).temp(1.2, 0.9).depth_scale(1.8, 0.5),
];

/// `Biome::biomes[id]`; `None` for ids outside the 23 defined biomes.
pub fn get(id: i32) -> Option<&'static Biome> {
    if (0..BIOME_COUNT as i32).contains(&id) {
        Some(&BIOMES[id as usize])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_match_indices() {
        for (i, b) in BIOMES.iter().enumerate() {
            assert_eq!(b.id as usize, i, "{}", b.name);
            assert!(!b.name.is_empty());
        }
        assert_eq!(BIOMES[id::JUNGLE_HILLS as usize].name, "JungleHills");
        assert!(get(23).is_none() && get(-1).is_none());
    }

    #[test]
    fn surface_blocks() {
        let b = |i: u8| &BIOMES[i as usize];
        assert_eq!((b(id::PLAINS).top_material, b(id::PLAINS).material), (ids::GRASS, ids::DIRT));
        assert_eq!((b(id::DESERT).top_material, b(id::DESERT).material), (ids::SAND, ids::SAND));
        assert_eq!((b(id::DESERT_HILLS).top_material, b(id::DESERT_HILLS).material), (ids::SAND, ids::SAND));
        assert_eq!((b(id::BEACHES).top_material, b(id::BEACHES).material), (ids::SAND, ids::SAND));
        assert_eq!((b(id::MUSHROOM_ISLAND).top_material, b(id::MUSHROOM_ISLAND).material), (ids::MYCEL, ids::DIRT));
        assert_eq!(b(id::MUSHROOM_ISLAND_SHORE).top_material, ids::MYCEL);
        assert_eq!((b(id::SKY).top_material, b(id::SKY).material), (ids::DIRT, ids::DIRT));
    }

    #[test]
    fn terrain_shape_parameters() {
        let b = |i: u8| &BIOMES[i as usize];
        assert_eq!((b(id::OCEAN).depth, b(id::OCEAN).scale), (-1.0, 0.4));
        assert_eq!((b(id::PLAINS).depth, b(id::PLAINS).scale), (0.1, 0.3)); // defaults
        assert_eq!((b(id::EXTREME_HILLS).depth, b(id::EXTREME_HILLS).scale), (0.3, 1.5));
        assert_eq!((b(id::RIVER).depth, b(id::RIVER).scale), (-0.5, 0.0));
        assert_eq!((b(id::JUNGLE_HILLS).depth, b(id::JUNGLE_HILLS).scale), (1.8, 0.5));
        assert_eq!((b(id::SWAMPLAND).depth, b(id::SWAMPLAND).scale), (-0.2, 0.1));
    }

    #[test]
    fn rain_and_snow_follow_temperature() {
        let b = |i: u8| &BIOMES[i as usize];
        assert!(b(id::PLAINS).has_rain() && !b(id::PLAINS).has_snow());
        assert!(b(id::ICE_FLATS).has_snow() && !b(id::ICE_FLATS).has_rain());
        assert!(b(id::TAIGA).has_snow()); // 0.05 < 0.15
        assert!(!b(id::DESERT).has_rain() && !b(id::DESERT).has_snow());
        assert!(!b(id::HELL).has_snow());
        assert!(b(id::JUNGLE).is_humid() && !b(id::PLAINS).is_humid());
        // extreme hills (0.2) is above the snow threshold
        assert!(!b(id::EXTREME_HILLS).has_snow());
    }

    #[test]
    fn fixed_point_climate() {
        assert_eq!(BIOMES[id::PLAINS as usize].temperature_int(), (0.8f32 * 65536.0) as i32);
        assert_eq!(BIOMES[id::MUSHROOM_ISLAND as usize].downfall_int(), 65536);
    }

    #[test]
    fn decorator_counts() {
        let d = |i: u8| BIOMES[i as usize].decorator;
        assert_eq!(d(id::FOREST).tree_count, 10);
        assert_eq!(d(id::DESERT).cactus_count, 10);
        assert_eq!(d(id::DESERT).tree_count, -999);
        assert_eq!(d(id::MUSHROOM_ISLAND).huge_mushrooms, 1);
        assert_eq!(d(id::SWAMPLAND).waterlily_count, 4);
        assert_eq!(d(id::OCEAN), DecoratorConfig::DEFAULT);
    }
}
