//! Per-block step / dig / place sounds: `Tile::SoundType` (the `SOUND_*` constants of `Tile::staticCtor`) and
//! the table of which block uses which one. Generated from `Tile.cpp` (`->setSoundType(...)` chains) plus the
//! classes that set their own sound in the constructor (stairs and walls copy their base block, pistons,
//! crops). Blocks that never call `setSoundType` keep the default `SOUND_NORMAL` (`Tile::Tile`).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundType {
    pub volume: f32,
    pub pitch: f32,
    /// `getStepSound()`; `None` = `-1` (anvil)
    pub step: Option<&'static str>,
    /// `getBreakSound()`
    pub brk: &'static str,
    /// `getPlaceSound()`
    pub place: &'static str,
}

const fn st(volume: f32, pitch: f32, step: Option<&'static str>, brk: &'static str, place: &'static str) -> SoundType {
    SoundType { volume, pitch, step, brk, place }
}

/// `SOUND_NORMAL` / `SOUND_STONE`: material STONE, volume 1, pitch 1
pub const STONE: SoundType = st(1.0, 1.0, Some("step.stone"), "dig.stone", "dig.stone");
pub const WOOD: SoundType = st(1.0, 1.0, Some("step.wood"), "dig.wood", "dig.wood");
pub const GRAVEL: SoundType = st(1.0, 1.0, Some("step.gravel"), "dig.gravel", "dig.gravel");
pub const GRASS: SoundType = st(1.0, 1.0, Some("step.grass"), "dig.grass", "dig.grass");
/// metal reuses the STONE material sounds at pitch 1.5
pub const METAL: SoundType = st(1.0, 1.5, Some("step.stone"), "dig.stone", "dig.stone");
/// glass: step.stone, break `random.glass`, place `step.stone`
pub const GLASS: SoundType = st(1.0, 1.0, Some("step.stone"), "random.glass", "step.stone");
pub const CLOTH: SoundType = st(1.0, 1.0, Some("step.cloth"), "dig.cloth", "dig.cloth");
pub const SAND: SoundType = st(1.0, 1.0, Some("step.sand"), "dig.sand", "dig.sand");
pub const SNOW: SoundType = st(1.0, 1.0, Some("step.snow"), "dig.snow", "dig.snow");
/// ladder: step.ladder, break `dig.wood`
pub const LADDER: SoundType = st(1.0, 1.0, Some("step.ladder"), "dig.wood", "dig.wood");
/// anvil: no step sound (-1), break `dig.stone`, place `random.anvil_land`
pub const ANVIL: SoundType = st(0.3, 1.0, None, "dig.stone", "random.anvil_land");

/// The sound type of block `id` (`Tile::tiles[id]->soundType`).
pub fn sound_type(id: u8) -> &'static SoundType {
    match id {
        1 | 4 | 7 | 14 | 15 | 16 | 21 | 22 | 23 | 24 | 29 | 33 | 34 | 43 | 44 | 45 | 48 | 49 | 56 | 61 | 62 | 67 | 70 | 73 | 74 | 77 | 84 | 87 | 98 | 108 | 109 | 112 | 113 | 114 | 121 | 122 | 128 | 129 | 130 | 139 | 144 | 153 | 155 | 156 | 158 | 159 | 172 | 173 => &STONE,
        2 | 6 | 18 | 19 | 31 | 32 | 37 | 38 | 39 | 40 | 46 | 59 | 83 | 106 | 110 | 111 | 141 | 142 | 170 => &GRASS,
        3 | 13 | 60 | 82 => &GRAVEL,
        5 | 17 | 47 | 50 | 51 | 53 | 54 | 58 | 63 | 64 | 68 | 69 | 72 | 75 | 76 | 85 | 86 | 91 | 93 | 94 | 96 | 99 | 100 | 103 | 104 | 105 | 107 | 125 | 126 | 127 | 134 | 135 | 136 | 143 | 146 | 147 | 148 | 149 | 150 | 151 | 154 => &WOOD,
        12 | 88 => &SAND,
        20 | 79 | 89 | 90 | 95 | 102 | 120 | 123 | 124 | 160 => &GLASS,
        27 | 28 | 41 | 42 | 52 | 57 | 66 | 71 | 101 | 133 | 152 | 157 => &METAL,
        35 | 80 | 81 | 92 | 171 => &CLOTH,
        65 => &LADDER,
        78 => &SNOW,
        145 => &ANVIL,
        _ => &STONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn well_known_blocks() {
        assert_eq!(sound_type(1), &STONE);
        assert_eq!(sound_type(2).step, Some("step.grass"));
        assert_eq!(sound_type(3).brk, "dig.gravel");
        assert_eq!(sound_type(41).pitch, 1.5); // gold block
        assert_eq!(sound_type(20).brk, "random.glass");
        assert_eq!(sound_type(65).step, Some("step.ladder"));
        assert_eq!(sound_type(145).step, None); // anvil
        assert_eq!(sound_type(8), &STONE); // water: default SOUND_NORMAL
    }
}
