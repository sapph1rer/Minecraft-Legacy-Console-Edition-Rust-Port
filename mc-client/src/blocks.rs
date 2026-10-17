//! Block → rendering style (shape, per-face terrain icon, tint), hand-curated from `Tile::registerIcons`
//! in the C++ sources. Face order everywhere: 0 = -x, 1 = +x, 2 = -y, 3 = +y, 4 = -z, 5 = +z.

use crate::icons::icon;
use mc_world::tile_ids as t;
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tint {
    None,
    Grass,
    /// tall grass / fern: the colour of the block's own biome (no 3×3 average)
    GrassPlant,
    Foliage,
    FoliageDefault,
    Lily,
    Spruce,
    Birch,
    Water,
    Redstone,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Air,
    Cube,
    /// two crossed quads (plants, torches, fire)
    Cross,
    /// axis-aligned box inside the block (slabs, fence posts, snow layers, ...)
    Box { min: [f32; 3], max: [f32; 3] },
    Liquid,
    /// a flat quad lying just above the floor (rails, lily pad, redstone dust)
    Flat,
    /// several boxes, see `shapes::parts`
    Shaped,
}

pub type Cell = (u8, u8);

#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub kind: Kind,
    pub cells: [Cell; 6],
    pub tint: [Tint; 6],
    /// Hides the faces of neighbouring blocks (full, opaque cube).
    pub opaque: bool,
    /// Drawn in the blended pass (water, ice, portal).
    pub translucent: bool,
    /// quarter turns of the texture per face
    pub rot: [u8; 6],
}

fn c(name: &str) -> Cell {
    match icon(name) {
        Some(v) => v,
        None => {
            #[cfg(test)]
            panic!("unknown terrain icon {name}");
            #[allow(unreachable_code)]
            (0, 1)
        }
    }
}

impl Style {
    fn new(kind: Kind, cells: [Cell; 6]) -> Style {
        Style { kind, cells, tint: [Tint::None; 6], opaque: matches!(kind, Kind::Cube), translucent: false, rot: [0; 6] }
    }
    fn cube(top: &str, bottom: &str, side: &str) -> Style {
        let (tp, bt, sd) = (c(top), c(bottom), c(side));
        Style::new(Kind::Cube, [sd, sd, bt, tp, sd, sd])
    }
    fn all(name: &str) -> Style {
        Style::cube(name, name, name)
    }
    fn cross(name: &str) -> Style {
        let mut s = Style::new(Kind::Cross, [c(name); 6]);
        s.opaque = false;
        s
    }
    pub fn flat(name: &str) -> Style {
        let mut s = Style::new(Kind::Flat, [c(name); 6]);
        s.opaque = false;
        s
    }
    fn boxed(mut self, min: [f32; 3], max: [f32; 3]) -> Style {
        self.kind = Kind::Box { min, max };
        self.opaque = false;
        self
    }
    fn tinted(mut self, which: &[usize], tint: Tint) -> Style {
        for &i in which {
            self.tint[i] = tint;
        }
        self
    }
    fn tint_all(mut self, tint: Tint) -> Style {
        self.tint = [tint; 6];
        self
    }
    fn see_through(mut self) -> Style {
        self.opaque = false;
        self
    }
    fn blended(mut self) -> Style {
        self.opaque = false;
        self.translucent = true;
        self
    }
    /// Put `front` on the face `face` (0..6).
    fn front(mut self, face: usize, front: &str) -> Style {
        self.cells[face] = c(front);
        self
    }
}

const WOOL: [&str; 16] = [
    "wool_colored_white",
    "wool_colored_orange",
    "wool_colored_magenta",
    "wool_colored_light_blue",
    "wool_colored_yellow",
    "wool_colored_lime",
    "wool_colored_pink",
    "wool_colored_gray",
    "wool_colored_silver",
    "wool_colored_cyan",
    "wool_colored_purple",
    "wool_colored_blue",
    "wool_colored_brown",
    "wool_colored_green",
    "wool_colored_red",
    "wool_colored_black",
];
const CLAY: [&str; 16] = [
    "hardened_clay_stained_white",
    "hardened_clay_stained_orange",
    "hardened_clay_stained_magenta",
    "hardened_clay_stained_light_blue",
    "hardened_clay_stained_yellow",
    "hardened_clay_stained_lime",
    "hardened_clay_stained_pink",
    "hardened_clay_stained_gray",
    "hardened_clay_stained_silver",
    "hardened_clay_stained_cyan",
    "hardened_clay_stained_purple",
    "hardened_clay_stained_blue",
    "hardened_clay_stained_brown",
    "hardened_clay_stained_green",
    "hardened_clay_stained_red",
    "hardened_clay_stained_black",
];
const GLASS: [&str; 16] = [
    "glass_white",
    "glass_orange",
    "glass_magenta",
    "glass_light_blue",
    "glass_yellow",
    "glass_lime",
    "glass_pink",
    "glass_gray",
    "glass_silver",
    "glass_cyan",
    "glass_purple",
    "glass_blue",
    "glass_brown",
    "glass_green",
    "glass_red",
    "glass_black",
];

const PLANKS: [&str; 4] = ["planks_oak", "planks_spruce", "planks_birch", "planks_jungle"];

/// Face (0..6) that a horizontally-facing block's front is on, from `data` using the `Facing` numbering
/// (2 = north/-z, 3 = south/+z, 4 = west/-x, 5 = east/+x).
fn facing_face(data: u8) -> usize {
    match data & 7 {
        2 => 4,
        3 => 5,
        4 => 0,
        5 => 1,
        _ => 4,
    }
}

fn slab_cells(kind: u8) -> (&'static str, &'static str) {
    match kind & 7 {
        0 => ("stoneslab_top", "stoneslab_side"),
        1 => ("sandstone_top", "sandstone_side"),
        2 => ("planks_oak", "planks_oak"),
        3 => ("cobblestone", "cobblestone"),
        4 => ("brick", "brick"),
        _ => ("stonebrick", "stonebrick"),
    }
}

fn compute(id: u8, d: u8) -> Style {
    let post = Style::all("planks_oak").boxed([0.375, 0.0, 0.375], [0.625, 1.0, 0.625]);
    match id {
        0 => Style::new(Kind::Air, [(0, 0); 6]),
        t::STONE => Style::all("stone"),
        t::GRASS => Style::cube("grass_top", "dirt", "grass_side").tinted(&[3], Tint::Grass),
        t::DIRT => Style::all("dirt"),
        t::COBBLESTONE => Style::all("cobblestone"),
        t::WOOD => Style::all(PLANKS[(d & 3) as usize]),
        t::SAPLING => Style::cross(["sapling", "sapling_spruce", "sapling_birch", "sapling_jungle"][(d & 3) as usize]),
        t::UNBREAKABLE => Style::all("bedrock"),
        t::WATER | t::CALM_WATER => {
            let mut s = Style::new(Kind::Liquid, [c("water"); 6]).tint_all(Tint::Water);
            s.translucent = true;
            s
        }
        t::LAVA | t::CALM_LAVA => Style::new(Kind::Liquid, [c("lava"); 6]),
        t::SAND => Style::all("sand"),
        t::GRAVEL => Style::all("gravel"),
        t::GOLD_ORE => Style::all("gold_ore"),
        t::IRON_ORE => Style::all("iron_ore"),
        t::COAL_ORE => Style::all("coal_ore"),
        t::TREE_TRUNK => {
            let (top, side) = match d & 3 {
                0 => ("log_oak_top", "log_oak"),
                1 => ("log_spruce_top", "log_spruce"),
                2 => ("log_birch_top", "log_birch"),
                _ => ("log_jungle_top", "log_jungle"),
            };
            let mut s = Style::cube(top, top, side);
            match d & 12 {
                4 => {
                    // lying along x: ends on the x faces, bark grain turned on the rest
                    s.cells = [c(top), c(top), c(side), c(side), c(side), c(side)];
                    s.rot = [0, 0, 1, 1, 1, 1];
                }
                8 => {
                    s.cells = [c(side), c(side), c(side), c(side), c(top), c(top)];
                    s.rot = [1, 1, 0, 0, 0, 0];
                }
                _ => {}
            }
            s
        }
        t::LEAVES => match d & 3 {
            0 => Style::all("leaves").tint_all(Tint::Foliage).see_through(),
            1 => Style::all("leaves_spruce").tint_all(Tint::Spruce).see_through(),
            2 => Style::all("leaves").tint_all(Tint::Birch).see_through(),
            _ => Style::all("leaves_jungle").tint_all(Tint::Foliage).see_through(),
        },
        t::SPONGE => Style::all("sponge"),
        t::GLASS => Style::all("glass").see_through(),
        t::LAPIS_ORE => Style::all("lapis_ore"),
        t::LAPIS_BLOCK => Style::all("lapis_block"),
        t::DISPENSER | t::DROPPER => {
            let front = if id == t::DISPENSER { "dispenser_front" } else { "dropper_front_horizontal" };
            Style::cube("furnace_top", "furnace_top", "furnace_side").front(facing_face(d), front)
        }
        t::SAND_STONE => Style::cube("sandstone_top", "sandstone_bottom", "sandstone_side"),
        t::NOTEBLOCK => Style::all("noteblock"),
        t::BED => Style::all("planks_oak").boxed([0.0, 0.0, 0.0], [1.0, 0.5625, 1.0]),
        t::GOLDEN_RAIL => Style::flat("rail_golden"),
        t::DETECTOR_RAIL => Style::flat("detectorRail"),
        t::RAIL => Style::flat("rail_normal"),
        t::ACTIVATOR_RAIL => Style::flat("rail_activator"),
        t::PISTON_STICKY_BASE | t::PISTON_BASE => {
            let top = if id == t::PISTON_BASE { "piston_top" } else { "piston_top_sticky" };
            let front_name = if d & 8 != 0 { "piston_inner_top" } else { top };
            // facing 0 down, 1 up, 2 north (-z), 3 south (+z), 4 west (-x), 5 east (+x) -> our face index
            let front = [2usize, 3, 4, 5, 0, 1][(d & 7).min(5) as usize];
            let back = front ^ 1;
            let mut s = Style::all("piston_side");
            s.cells[front] = c(front_name);
            s.cells[back] = c("piston_bottom");
            let dirv = [[0.0f32, -1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0], [0.0, 0.0, 1.0], [-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]][(d & 7).min(5) as usize];
            for face in 0..6 {
                if face != front && face != back {
                    s.rot[face] = crate::mesh::rot_for_up(face, dirv);
                }
            }
            s
        }
        t::WEB => Style::cross("web"),
        t::TALLGRASS => Style::cross(match d { 0 => "deadbush", 2 => "fern", _ => "tallgrass" }).tint_all(if d == 0 { Tint::None } else { Tint::GrassPlant }),
        t::DEAD_BUSH => Style::cross("deadbush"),
        t::WOOL => Style::all(WOOL[(d & 15) as usize]),
        t::FLOWER => Style::cross("flower_dandelion"),
        t::ROSE => Style::cross("flower_rose"),
        t::MUSHROOM_BROWN => Style::cross("mushroom_brown"),
        t::MUSHROOM_RED => Style::cross("mushroom_red"),
        t::GOLD_BLOCK => Style::all("gold_block"),
        t::IRON_BLOCK => Style::all("iron_block"),
        t::STONE_SLAB | t::STONE_SLAB_HALF => {
            let (top, side) = slab_cells(d);
            let s = Style::cube(top, top, side);
            if id == t::STONE_SLAB_HALF {
                if d & 8 != 0 {
                    s.boxed([0.0, 0.5, 0.0], [1.0, 1.0, 1.0])
                } else {
                    s.boxed([0.0, 0.0, 0.0], [1.0, 0.5, 1.0])
                }
            } else {
                s
            }
        }
        t::RED_BRICK => Style::all("brick"),
        t::TNT => Style::cube("tnt_top", "tnt_bottom", "tnt_side"),
        t::BOOKSHELF => Style::cube("planks_oak", "planks_oak", "bookshelf"),
        t::MOSSY_COBBLESTONE => Style::all("cobblestone_mossy"),
        t::OBSIDIAN => Style::all("obsidian"),
        t::TORCH => Style::cross("torch_on"),
        t::FIRE => Style::cross("fire_0"),
        t::MOB_SPAWNER => Style::all("mob_spawner").see_through(),
        t::STAIRS_WOOD | t::STAIRS_SPRUCEWOOD | t::STAIRS_BIRCHWOOD | t::STAIRS_JUNGLEWOOD => Style::all(match id {
            t::STAIRS_WOOD => "planks_oak",
            t::STAIRS_SPRUCEWOOD => "planks_spruce",
            t::STAIRS_BIRCHWOOD => "planks_birch",
            _ => "planks_jungle",
        }),
        t::CHEST | t::CHEST_TRAP | t::ENDER_CHEST => Style::all(if id == t::ENDER_CHEST { "obsidian" } else { "planks_oak" })
            .boxed([0.0625, 0.0, 0.0625], [0.9375, 0.875, 0.9375]),
        t::RED_STONE_DUST => Style::flat("redstone_dust_line").tint_all(Tint::Redstone),
        t::DIAMOND_ORE => Style::all("diamond_ore"),
        t::DIAMOND_BLOCK => Style::all("diamond_block"),
        t::WORK_BENCH => Style::cube("workbench_top", "planks_oak", "workbench_side").front(facing_face(2), "workbench_front"),
        t::WHEAT => Style::cross(["crops_0", "crops_1", "crops_2", "crops_3", "crops_4", "crops_5", "crops_6", "crops_7"][(d & 7) as usize]),
        t::FARMLAND => Style::cube("farmland_dry", "dirt", "dirt").boxed([0.0, 0.0, 0.0], [1.0, 0.9375, 1.0]),
        t::FURNACE | t::FURNACE_LIT => {
            let front = if id == t::FURNACE { "furnace_front" } else { "furnace_front_lit" };
            Style::cube("furnace_top", "furnace_top", "furnace_side").front(facing_face(d), front)
        }
        t::LADDER => Style::flat("ladder"),
        t::STAIRS_STONE => Style::all("cobblestone"),
        t::RED_STONE_ORE | t::RED_STONE_ORE_LIT => Style::all("redstone_ore"),
        t::REDSTONE_TORCH_OFF => Style::cross("redstone_torch_off"),
        t::REDSTONE_TORCH_ON => Style::cross("redstone_torch_on"),
        t::TOP_SNOW => {
            let h = ((d & 7) + 1) as f32 / 8.0;
            Style::all("snow").boxed([0.0, 0.0, 0.0], [1.0, h, 1.0])
        }
        t::ICE => Style::all("ice").blended(),
        t::SNOW => Style::all("snow"),
        t::CACTUS => Style::cube("cactus_top", "cactus_bottom", "cactus_side").boxed([0.0625, 0.0, 0.0625], [0.9375, 1.0, 0.9375]),
        t::CLAY => Style::all("clay"),
        t::REEDS => Style::cross("reeds"),
        t::JUKEBOX => Style::cube("jukebox_top", "jukebox_side", "jukebox_side"),
        t::FENCE => post,
        t::PUMPKIN | t::LIT_PUMPKIN => {
            let front = if id == t::PUMPKIN { "pumpkin_face_off" } else { "pumpkin_face_on" };
            let face = match d & 3 {
                0 => 5,
                1 => 0,
                2 => 4,
                _ => 1,
            };
            Style::cube("pumpkin_top", "pumpkin_top", "pumpkin_side").front(face, front)
        }
        t::NETHER_RACK => Style::all("netherrack"),
        t::SOULSAND => Style::all("soul_sand"),
        t::GLOWSTONE => Style::all("glowstone"),
        t::PORTAL_TILE => Style::all("portal").blended(),
        t::STAINED_GLASS => Style::all(GLASS[(d & 15) as usize]).see_through(),
        t::MONSTER_STONE_EGG => Style::all("stone"),
        t::STONE_BRICK => Style::all(["stonebrick", "stonebrick_mossy", "stonebrick_cracked", "stonebrick_carved"][(d & 3) as usize]),
        t::HUGE_MUSHROOM_BROWN => Style::all("mushroom_block_skin_brown"),
        t::HUGE_MUSHROOM_RED => Style::all("mushroom_block_skin_red"),
        t::IRON_FENCE => Style::all("iron_bars").see_through(),
        t::THIN_GLASS => Style::all("glass").see_through(),
        t::STAINED_GLASS_PANE => Style::all(GLASS[(d & 15) as usize]).see_through(),
        t::MELON => Style::cube("melon_top", "melon_top", "melon_side"),
        t::VINE => Style::cross("vine").tint_all(Tint::FoliageDefault),
        t::STAIRS_BRICKS => Style::all("brick"),
        t::STAIRS_STONE_BRICK => Style::all("stonebrick"),
        t::MYCEL => Style::cube("mycel_top", "dirt", "mycel_side"),
        t::WATER_LILY => Style::flat("waterlily").tint_all(Tint::Lily),
        t::NETHER_BRICK => Style::all("nether_brick"),
        t::NETHER_FENCE => Style::all("nether_brick").boxed([0.375, 0.0, 0.375], [0.625, 1.0, 0.625]),
        t::STAIRS_NETHER_BRICKS => Style::all("nether_brick"),
        t::NETHER_STALK => Style::cross(["nether_wart_stage_0", "nether_wart_stage_1", "nether_wart_stage_1", "nether_wart_stage_2"][(d & 3) as usize]),
        t::ENCHANT_TABLE => Style::cube("enchantment_top", "enchantment_bottom", "enchantment_side").boxed([0.0, 0.0, 0.0], [1.0, 0.75, 1.0]),
        t::BREWING_STAND => Style::cross("brewing_stand"),
        t::CAULDRON => Style::cube("cauldron_top", "cauldron_bottom", "cauldron_side"),
        t::END_PORTAL_FRAME_TILE => Style::cube("endframe_top", "end_stone", "endframe_side").boxed([0.0, 0.0, 0.0], [1.0, 0.8125, 1.0]),
        t::END_STONE => Style::all("end_stone"),
        t::DRAGON_EGG => Style::all("dragon_egg").boxed([0.0625, 0.0, 0.0625], [0.9375, 1.0, 0.9375]),
        t::REDSTONE_LIGHT => Style::all("redstoneLight"),
        t::REDSTONE_LIGHT_LIT => Style::all("redstoneLight_lit"),
        t::WOOD_SLAB | t::WOOD_SLAB_HALF => {
            let p = PLANKS[(d & 3) as usize];
            let s = Style::all(p);
            if id == t::WOOD_SLAB_HALF {
                if d & 8 != 0 {
                    s.boxed([0.0, 0.5, 0.0], [1.0, 1.0, 1.0])
                } else {
                    s.boxed([0.0, 0.0, 0.0], [1.0, 0.5, 1.0])
                }
            } else {
                s
            }
        }
        t::STAIRS_SANDSTONE => Style::cube("sandstone_top", "sandstone_bottom", "sandstone_side"),
        t::EMERALD_ORE => Style::all("emerald_ore"),
        t::EMERALD_BLOCK => Style::all("emerald_block"),
        t::COMMAND_BLOCK => Style::all("command_block"),
        t::BEACON => Style::all("beacon").see_through(),
        t::COBBLE_WALL => Style::all(if d == 1 { "cobblestone_mossy" } else { "cobblestone" }).boxed([0.25, 0.0, 0.25], [0.75, 1.0, 0.75]),
        t::CARROTS => Style::cross(["carrots_stage_0", "carrots_stage_0", "carrots_stage_1", "carrots_stage_1", "carrots_stage_2", "carrots_stage_2", "carrots_stage_2", "carrots_stage_3"][(d & 7) as usize]),
        t::POTATOES => Style::cross(["potatoes_stage_0", "potatoes_stage_0", "potatoes_stage_1", "potatoes_stage_1", "potatoes_stage_2", "potatoes_stage_2", "potatoes_stage_2", "potatoes_stage_3"][(d & 7) as usize]),
        t::ANVIL => Style::cube("anvil_top", "anvil_base", "anvil_base"),
        t::DAYLIGHT_DETECTOR => Style::cube("daylight_detector_top", "daylight_detector_side", "daylight_detector_side").boxed([0.0, 0.0, 0.0], [1.0, 0.375, 1.0]),
        t::REDSTONE_BLOCK => Style::all("redstone_block"),
        t::NETHER_QUARTZ => Style::all("quartz_ore"),
        t::HOPPER => Style::cube("hopper_top", "hopper_outside", "hopper_outside"),
        t::QUARTZ_BLOCK => match d {
            1 => Style::cube("quartz_block_chiseled_top", "quartz_block_bottom", "quartz_block_chiseled"),
            2..=4 => Style::cube("quartz_block_lines_top", "quartz_block_lines_top", "quartz_block_lines"),
            _ => Style::cube("quartz_block_top", "quartz_block_bottom", "quartz_block_side"),
        },
        t::STAIRS_QUARTZ => Style::cube("quartz_block_top", "quartz_block_bottom", "quartz_block_side"),
        t::CLAY_HARDENED_COLORED => Style::all(CLAY[(d & 15) as usize]),
        t::HAY_BLOCK => Style::cube("hay_block_top", "hay_block_top", "hay_block_side"),
        t::WOOL_CARPET => Style::all(WOOL[(d & 15) as usize]).boxed([0.0, 0.0, 0.0], [1.0, 0.0625, 1.0]),
        t::CLAY_HARDENED => Style::all("hardened_clay"),
        t::COAL_BLOCK => Style::all("coal_block"),
        // no terrain icon (doors, signs, levers, buttons, plates, repeaters, skulls, ...): not drawn
        t::SIGN | t::WALL_SIGN | t::DOOR_WOOD | t::DOOR_IRON | t::LEVER | t::BUTTON_STONE | t::BUTTON_WOOD
        | t::PRESSURE_PLATE_STONE | t::PRESSURE_PLATE_WOOD | t::DIODE_OFF | t::DIODE_ON | t::TRAPDOOR | t::FENCE_GATE
        | t::CAKE => {
            let mut s = Style::all("planks_oak");
            s.kind = Kind::Shaped;
            s.opaque = false;
            s
        }
        t::TRIP_WIRE_SOURCE => Style::flat("trip_wire_source"),
        t::TRIP_WIRE => Style::flat("trip_wire"),
        t::WEIGHTED_PLATE_LIGHT => Style::all("gold_block").boxed([0.0625, 0.0, 0.0625], [0.9375, 0.0625, 0.9375]),
        t::WEIGHTED_PLATE_HEAVY => Style::all("iron_block").boxed([0.0625, 0.0, 0.0625], [0.9375, 0.0625, 0.9375]),
        t::COMPARATOR_OFF | t::COMPARATOR_ON => {
            let mut s = Style::all("stone").boxed([0.0, 0.0, 0.0], [1.0, 0.125, 1.0]);
            s.cells[3] = icon(if id == t::COMPARATOR_ON { "comparator_on" } else { "comparator_off" }).unwrap_or((11, 11));
            s
        }
        t::SKULL | t::FLOWER_POT | t::COCOA | t::PUMPKIN_STEM
        | t::MELON_STEM
        | t::END_PORTAL_TILE | t::PISTON_MOVING_PIECE => Style::new(Kind::Air, [(0, 0); 6]),
        t::PISTON_EXTENSION_PIECE => Style::all("piston_side"),
        // anything else: a visible placeholder
        _ => Style::all("stone").tint_all(Tint::Redstone),
    }
}

/// Style of block `id` with metadata `data` (cached; 256 × 16 entries).
pub fn style(id: u8, data: u8) -> &'static Style {
    static TABLE: OnceLock<Vec<Style>> = OnceLock::new();
    let table = TABLE.get_or_init(|| (0..4096usize).map(|i| { let mut st = compute((i >> 4) as u8, (i & 15) as u8); if crate::shapes::is_shaped_data((i >> 4) as u8, (i & 15) as u8) { st.kind = Kind::Shaped; st.opaque = false; } st }).collect());
    &table[((id as usize) << 4) | (data as usize & 15)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_named_icon_exists() {
        // `c()` silently falls back to stone; make sure no table entry relies on that
        for id in 0..=255u8 {
            for d in 0..16u8 {
                let _ = style(id, d);
            }
        }
        for name in ["grass_top", "dirt", "leaves", "water", "lava", "portal", "end_stone", "netherrack", "furnace_front_lit"] {
            assert!(icon(name).is_some(), "{name}");
        }
    }

    #[test]
    fn grass_is_tinted_on_top_only() {
        let s = style(t::GRASS, 0);
        assert_eq!(s.tint[3], Tint::Grass);
        assert_eq!(s.tint[0], Tint::None);
        assert!(s.opaque);
    }
}
