//! `LevelType` — the world-type selector. Only the identity matters to world generation.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LevelType {
    /// `lvl_normal` — default world.
    Normal,
    /// `lvl_normal_1_1` — pre-jungle biome list (no jungle).
    Normal11,
    /// `lvl_flat` — superflat.
    Flat,
    /// `lvl_largeBiomes` — biome layers zoomed two extra times.
    LargeBiomes,
}

impl Default for LevelType {
    fn default() -> Self {
        LevelType::Normal
    }
}
