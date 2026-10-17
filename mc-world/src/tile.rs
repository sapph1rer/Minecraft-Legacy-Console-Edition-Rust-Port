//! The block (`Tile`) table.
//!
//! This is the *data* half of `Tile::staticCtor`: id, icon name, hardness (`destroySpeed`),
//! blast resistance, light opacity/emission and — where the registration line names it —
//! the `Material`. It was extracted mechanically from `Tile.cpp`, see `tile_table.rs`.
//!
//! Not here (layer 6): the per-class behaviour (`tick`, `use`, shapes, drops, …).
//!
//! Notes on the extracted values
//! * `explosion_resistance` replays the setter chain: `setExplodeable(x)` stores `3x`, and
//!   `setDestroyTime(t)` raises it to at least `5t`. Order matters and is preserved.
//! * `destroy_time == -1.0` means indestructible (`Tile::INDESTRUCTIBLE_DESTROY_TIME`).
//! * `material` is `None` when the material is chosen inside the tile subclass's constructor
//!   (e.g. `new WoolCarpetTile(171)`); it will be filled in when those classes are ported.
//! * `light_block` is `None` when `setLightBlock` is not called; the default depends on whether
//!   the tile `isSolidRender()` (255) or not (0) — also decided in the subclass.
//! * The four slab entries (43, 44, 125, 126) are not registered with a literal id in
//!   `Tile.cpp` (they use class constants) so their numbers are the usual 1.6 values.

use crate::material::Material;
use crate::tile_table::TILE_TABLE;

pub use crate::tile_table::ids;

#[derive(Clone, Copy, Debug)]
pub struct TileInfo {
    pub id: u8,
    /// Name of the `Tile::xxx` static (and of the `xxx_Id` constant).
    pub name: &'static str,
    /// `setIconName` — texture atlas key.
    pub icon: &'static str,
    /// C++ class instantiated for it.
    pub class: &'static str,
    pub material: Option<Material>,
    pub destroy_time: f32,
    pub explosion_resistance: f32,
    pub light_block: Option<u8>,
    pub light_emission: u8,
}

impl TileInfo {
    pub fn is_indestructible(&self) -> bool {
        self.destroy_time < 0.0
    }
}

/// Looks a block up by id. `None` for air (0) and for unregistered ids.
pub fn tile_info(id: u8) -> Option<&'static TileInfo> {
    TILE_TABLE
        .binary_search_by_key(&id, |t| t.id)
        .ok()
        .map(|i| &TILE_TABLE[i])
}

/// All registered blocks, sorted by id.
pub fn all_tiles() -> &'static [TileInfo] {
    TILE_TABLE
}

/// Material of a block id (see [`crate::tile_props`]); air for 0, `None` for unregistered ids.
pub fn material_of(id: u8) -> Option<Material> {
    if id == 0 {
        return Some(Material::Air);
    }
    if crate::tile_props::is_registered(id) {
        return Some(crate::tile_props::material(id));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_unique() {
        let t = all_tiles();
        assert!(t.len() >= 160);
        for w in t.windows(2) {
            assert!(w[0].id < w[1].id, "{} !< {}", w[0].id, w[1].id);
        }
    }

    #[test]
    fn well_known_ids() {
        assert_eq!(ids::STONE, 1);
        assert_eq!(ids::GRASS, 2);
        assert_eq!(ids::DIRT, 3);
        assert_eq!(ids::UNBREAKABLE, 7);
        assert_eq!(ids::WATER, 8);
        assert_eq!(ids::CALM_WATER, 9);
        assert_eq!(ids::LAVA, 10);
        assert_eq!(ids::CALM_LAVA, 11);
        assert_eq!(ids::SAND, 12);
        assert_eq!(ids::SAND_STONE, 24);
        assert_eq!(ids::ICE, 79);
        assert_eq!(ids::MYCEL, 110);
        assert_eq!(ids::COAL_BLOCK, 173);
    }

    #[test]
    fn names_match_ids() {
        let t = tile_info(ids::CALM_WATER).unwrap();
        assert_eq!(t.name, "calmWater");
        assert_eq!(t.material, Some(Material::Water));
        assert_eq!(t.light_block, Some(3));
        let lava = tile_info(ids::CALM_LAVA).unwrap();
        assert_eq!(lava.material, Some(Material::Lava));
        assert_eq!(lava.light_emission, 15);
        assert_eq!(lava.light_block, Some(255));
    }

    #[test]
    fn hardness_and_resistance_replay_setter_chain() {
        // stone: setDestroyTime(1.5) then setExplodeable(10) -> 30
        let s = tile_info(ids::STONE).unwrap();
        assert_eq!(s.destroy_time, 1.5);
        assert_eq!(s.explosion_resistance, 30.0);
        // dirt: only setDestroyTime(0.5) -> resistance raised to 2.5
        let d = tile_info(ids::DIRT).unwrap();
        assert_eq!(d.destroy_time, 0.5);
        assert_eq!(d.explosion_resistance, 2.5);
        // bedrock is indestructible and has a huge resistance
        let b = tile_info(ids::UNBREAKABLE).unwrap();
        assert!(b.is_indestructible());
        assert_eq!(b.explosion_resistance, 18_000_000.0);
    }

    #[test]
    fn material_lookup() {
        assert_eq!(material_of(0), Some(Material::Air));
        assert_eq!(material_of(ids::STONE), Some(Material::Stone));
        assert_eq!(material_of(ids::SAND), Some(Material::Sand));
        assert_eq!(material_of(ids::GRASS), Some(Material::Grass));
        assert!(tile_info(200).is_none());
    }
}
