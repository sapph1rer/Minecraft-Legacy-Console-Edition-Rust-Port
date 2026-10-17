//! Core registries ported from `Minecraft.World` (layer 3 of the port plan).
//!
//! * [`material`] — `Material` / `MaterialColor` (+ the Gas/Liquid/Decoration/Portal/Web subclasses)
//! * [`facing`] / [`direction`] — `Facing` and `Direction` lookup tables
//! * [`tile`] — the block id table (`Tile::staticCtor`): ids, hardness, resistance, light
//! * [`biome`] — the 23 biome definitions (`Biome::staticCtor`)
//! * [`level_type`] — `LevelType`
//!
//! Tile *behaviour* (the ~180 `Tile` subclasses) is layer 6; only the data the world
//! generator needs is here.

pub mod biome;
pub mod direction;
pub mod drops;
pub mod facing;
pub mod item;
pub mod level_type;
pub mod material;
pub mod recipe;
pub mod tile;
pub mod tile_props;

mod item_table;
mod recipe_table;
mod tile_table;

pub use biome::Biome;
pub use level_type::LevelType;
pub use material::{Material, MaterialColor};
pub use tile::{ids as tile_ids, TileInfo};

/// `Level::genDepthBits`
pub const GEN_DEPTH_BITS: i32 = 7;
/// `Level::genDepth` — height of generated terrain (the legacy console world is 128 high).
pub const GEN_DEPTH: i32 = 1 << GEN_DEPTH_BITS;
pub const GEN_DEPTH_MINUS_ONE: i32 = GEN_DEPTH - 1;
/// `Level::constSeaLevel`
pub const SEA_LEVEL: i32 = GEN_DEPTH / 2 - 1;
/// `Level::maxBuildHeight`
pub const MAX_BUILD_HEIGHT: i32 = 256;
/// `Level::MAX_BRIGHTNESS`
pub const MAX_BRIGHTNESS: i32 = 15;
pub mod colours;
