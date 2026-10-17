// @generated from Minecraft.World/Tile.cpp (Tile::staticCtor) and Tile.h by a one-off script.
// Do not edit by hand; re-generate or fix the table in tile.rs.

/// Block ids (`Tile::xxx_Id`).
#[allow(dead_code)]
pub mod ids {
    pub const STONE: u8 = 1;
    pub const GRASS: u8 = 2;
    pub const DIRT: u8 = 3;
    pub const COBBLESTONE: u8 = 4;
    pub const WOOD: u8 = 5;
    pub const SAPLING: u8 = 6;
    pub const UNBREAKABLE: u8 = 7;
    pub const WATER: u8 = 8;
    pub const CALM_WATER: u8 = 9;
    pub const LAVA: u8 = 10;
    pub const CALM_LAVA: u8 = 11;
    pub const SAND: u8 = 12;
    pub const GRAVEL: u8 = 13;
    pub const GOLD_ORE: u8 = 14;
    pub const IRON_ORE: u8 = 15;
    pub const COAL_ORE: u8 = 16;
    pub const TREE_TRUNK: u8 = 17;
    pub const LEAVES: u8 = 18;
    pub const SPONGE: u8 = 19;
    pub const GLASS: u8 = 20;
    pub const LAPIS_ORE: u8 = 21;
    pub const LAPIS_BLOCK: u8 = 22;
    pub const DISPENSER: u8 = 23;
    pub const SAND_STONE: u8 = 24;
    pub const NOTEBLOCK: u8 = 25;
    pub const BED: u8 = 26;
    pub const GOLDEN_RAIL: u8 = 27;
    pub const DETECTOR_RAIL: u8 = 28;
    pub const PISTON_STICKY_BASE: u8 = 29;
    pub const WEB: u8 = 30;
    pub const TALLGRASS: u8 = 31;
    pub const DEAD_BUSH: u8 = 32;
    pub const PISTON_BASE: u8 = 33;
    pub const PISTON_EXTENSION_PIECE: u8 = 34;
    pub const WOOL: u8 = 35;
    pub const PISTON_MOVING_PIECE: u8 = 36;
    pub const FLOWER: u8 = 37;
    pub const ROSE: u8 = 38;
    pub const MUSHROOM_BROWN: u8 = 39;
    pub const MUSHROOM_RED: u8 = 40;
    pub const GOLD_BLOCK: u8 = 41;
    pub const IRON_BLOCK: u8 = 42;
    pub const STONE_SLAB: u8 = 43;
    pub const STONE_SLAB_HALF: u8 = 44;
    pub const RED_BRICK: u8 = 45;
    pub const TNT: u8 = 46;
    pub const BOOKSHELF: u8 = 47;
    pub const MOSSY_COBBLESTONE: u8 = 48;
    pub const OBSIDIAN: u8 = 49;
    pub const TORCH: u8 = 50;
    pub const FIRE: u8 = 51;
    pub const MOB_SPAWNER: u8 = 52;
    pub const STAIRS_WOOD: u8 = 53;
    pub const CHEST: u8 = 54;
    pub const RED_STONE_DUST: u8 = 55;
    pub const DIAMOND_ORE: u8 = 56;
    pub const DIAMOND_BLOCK: u8 = 57;
    pub const WORK_BENCH: u8 = 58;
    pub const WHEAT: u8 = 59;
    pub const FARMLAND: u8 = 60;
    pub const FURNACE: u8 = 61;
    pub const FURNACE_LIT: u8 = 62;
    pub const SIGN: u8 = 63;
    pub const DOOR_WOOD: u8 = 64;
    pub const LADDER: u8 = 65;
    pub const RAIL: u8 = 66;
    pub const STAIRS_STONE: u8 = 67;
    pub const WALL_SIGN: u8 = 68;
    pub const LEVER: u8 = 69;
    pub const PRESSURE_PLATE_STONE: u8 = 70;
    pub const DOOR_IRON: u8 = 71;
    pub const PRESSURE_PLATE_WOOD: u8 = 72;
    pub const RED_STONE_ORE: u8 = 73;
    pub const RED_STONE_ORE_LIT: u8 = 74;
    pub const REDSTONE_TORCH_OFF: u8 = 75;
    pub const REDSTONE_TORCH_ON: u8 = 76;
    pub const BUTTON_STONE: u8 = 77;
    pub const TOP_SNOW: u8 = 78;
    pub const ICE: u8 = 79;
    pub const SNOW: u8 = 80;
    pub const CACTUS: u8 = 81;
    pub const CLAY: u8 = 82;
    pub const REEDS: u8 = 83;
    pub const JUKEBOX: u8 = 84;
    pub const FENCE: u8 = 85;
    pub const PUMPKIN: u8 = 86;
    pub const NETHER_RACK: u8 = 87;
    pub const SOULSAND: u8 = 88;
    pub const GLOWSTONE: u8 = 89;
    pub const PORTAL_TILE: u8 = 90;
    pub const LIT_PUMPKIN: u8 = 91;
    pub const CAKE: u8 = 92;
    pub const DIODE_OFF: u8 = 93;
    pub const DIODE_ON: u8 = 94;
    pub const STAINED_GLASS: u8 = 95;
    pub const TRAPDOOR: u8 = 96;
    pub const MONSTER_STONE_EGG: u8 = 97;
    pub const STONE_BRICK: u8 = 98;
    pub const HUGE_MUSHROOM_BROWN: u8 = 99;
    pub const HUGE_MUSHROOM_RED: u8 = 100;
    pub const IRON_FENCE: u8 = 101;
    pub const THIN_GLASS: u8 = 102;
    pub const MELON: u8 = 103;
    pub const PUMPKIN_STEM: u8 = 104;
    pub const MELON_STEM: u8 = 105;
    pub const VINE: u8 = 106;
    pub const FENCE_GATE: u8 = 107;
    pub const STAIRS_BRICKS: u8 = 108;
    pub const STAIRS_STONE_BRICK: u8 = 109;
    pub const MYCEL: u8 = 110;
    pub const WATER_LILY: u8 = 111;
    pub const NETHER_BRICK: u8 = 112;
    pub const NETHER_FENCE: u8 = 113;
    pub const STAIRS_NETHER_BRICKS: u8 = 114;
    pub const NETHER_STALK: u8 = 115;
    pub const ENCHANT_TABLE: u8 = 116;
    pub const BREWING_STAND: u8 = 117;
    pub const CAULDRON: u8 = 118;
    pub const END_PORTAL_TILE: u8 = 119;
    pub const END_PORTAL_FRAME_TILE: u8 = 120;
    pub const END_STONE: u8 = 121;
    pub const DRAGON_EGG: u8 = 122;
    pub const REDSTONE_LIGHT: u8 = 123;
    pub const REDSTONE_LIGHT_LIT: u8 = 124;
    pub const WOOD_SLAB: u8 = 125;
    pub const WOOD_SLAB_HALF: u8 = 126;
    pub const COCOA: u8 = 127;
    pub const STAIRS_SANDSTONE: u8 = 128;
    pub const EMERALD_ORE: u8 = 129;
    pub const ENDER_CHEST: u8 = 130;
    pub const TRIP_WIRE_SOURCE: u8 = 131;
    pub const TRIP_WIRE: u8 = 132;
    pub const EMERALD_BLOCK: u8 = 133;
    pub const STAIRS_SPRUCEWOOD: u8 = 134;
    pub const STAIRS_BIRCHWOOD: u8 = 135;
    pub const STAIRS_JUNGLEWOOD: u8 = 136;
    pub const COMMAND_BLOCK: u8 = 137;
    pub const BEACON: u8 = 138;
    pub const COBBLE_WALL: u8 = 139;
    pub const FLOWER_POT: u8 = 140;
    pub const CARROTS: u8 = 141;
    pub const POTATOES: u8 = 142;
    pub const BUTTON_WOOD: u8 = 143;
    pub const SKULL: u8 = 144;
    pub const ANVIL: u8 = 145;
    pub const CHEST_TRAP: u8 = 146;
    pub const WEIGHTED_PLATE_LIGHT: u8 = 147;
    pub const WEIGHTED_PLATE_HEAVY: u8 = 148;
    pub const COMPARATOR_OFF: u8 = 149;
    pub const COMPARATOR_ON: u8 = 150;
    pub const DAYLIGHT_DETECTOR: u8 = 151;
    pub const REDSTONE_BLOCK: u8 = 152;
    pub const NETHER_QUARTZ: u8 = 153;
    pub const HOPPER: u8 = 154;
    pub const QUARTZ_BLOCK: u8 = 155;
    pub const STAIRS_QUARTZ: u8 = 156;
    pub const ACTIVATOR_RAIL: u8 = 157;
    pub const DROPPER: u8 = 158;
    pub const CLAY_HARDENED_COLORED: u8 = 159;
    pub const STAINED_GLASS_PANE: u8 = 160;
    pub const HAY_BLOCK: u8 = 170;
    pub const WOOL_CARPET: u8 = 171;
    pub const CLAY_HARDENED: u8 = 172;
    pub const COAL_BLOCK: u8 = 173;
}

use crate::material::Material;
use crate::tile::TileInfo;

pub(crate) static TILE_TABLE: &[TileInfo] = &[
    TileInfo { id: 1, name: "stone", icon: "stone", class: "StoneTile", material: Some(Material::Stone), destroy_time: 1.5, explosion_resistance: 30.0, light_block: None, light_emission: 0 },
    TileInfo { id: 2, name: "grass", icon: "grass", class: "GrassTile", material: Some(Material::Grass), destroy_time: 0.6, explosion_resistance: 3.0, light_block: None, light_emission: 0 },
    TileInfo { id: 3, name: "dirt", icon: "dirt", class: "DirtTile", material: Some(Material::Dirt), destroy_time: 0.5, explosion_resistance: 2.5, light_block: None, light_emission: 0 },
    TileInfo { id: 4, name: "cobblestone", icon: "cobblestone", class: "Tile", material: Some(Material::Stone), destroy_time: 2.0, explosion_resistance: 30.0, light_block: None, light_emission: 0 },
    TileInfo { id: 5, name: "wood", icon: "planks", class: "WoodTile", material: Some(Material::Wood), destroy_time: 2.0, explosion_resistance: 15.0, light_block: None, light_emission: 0 },
    TileInfo { id: 6, name: "sapling", icon: "sapling", class: "Sapling", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 7, name: "unbreakable", icon: "bedrock", class: "Tile", material: Some(Material::Stone), destroy_time: -1.0, explosion_resistance: 18000000.0, light_block: None, light_emission: 0 },
    TileInfo { id: 8, name: "water", icon: "water_flow", class: "LiquidTileDynamic", material: Some(Material::Water), destroy_time: 100.0, explosion_resistance: 500.0, light_block: Some(3), light_emission: 0 },
    TileInfo { id: 9, name: "calmWater", icon: "water_still", class: "LiquidTileStatic", material: Some(Material::Water), destroy_time: 100.0, explosion_resistance: 500.0, light_block: Some(3), light_emission: 0 },
    TileInfo { id: 10, name: "lava", icon: "lava_flow", class: "LiquidTileDynamic", material: Some(Material::Lava), destroy_time: 0.0, explosion_resistance: 0.0, light_block: Some(255), light_emission: 15 },
    TileInfo { id: 11, name: "calmLava", icon: "lava_still", class: "LiquidTileStatic", material: Some(Material::Lava), destroy_time: 100.0, explosion_resistance: 500.0, light_block: Some(255), light_emission: 15 },
    TileInfo { id: 12, name: "sand", icon: "sand", class: "HeavyTile", material: Some(Material::Sand), destroy_time: 0.5, explosion_resistance: 2.5, light_block: None, light_emission: 0 },
    TileInfo { id: 13, name: "gravel", icon: "gravel", class: "GravelTile", material: Some(Material::Sand), destroy_time: 0.6, explosion_resistance: 3.0, light_block: None, light_emission: 0 },
    TileInfo { id: 14, name: "goldOre", icon: "gold_ore", class: "OreTile", material: Some(Material::Stone), destroy_time: 3.0, explosion_resistance: 15.0, light_block: None, light_emission: 0 },
    TileInfo { id: 15, name: "ironOre", icon: "iron_ore", class: "OreTile", material: Some(Material::Stone), destroy_time: 3.0, explosion_resistance: 15.0, light_block: None, light_emission: 0 },
    TileInfo { id: 16, name: "coalOre", icon: "coal_ore", class: "OreTile", material: Some(Material::Stone), destroy_time: 3.0, explosion_resistance: 15.0, light_block: None, light_emission: 0 },
    TileInfo { id: 17, name: "treeTrunk", icon: "log", class: "TreeTile", material: None, destroy_time: 2.0, explosion_resistance: 10.0, light_block: None, light_emission: 0 },
    TileInfo { id: 18, name: "leaves", icon: "leaves", class: "LeafTile", material: None, destroy_time: 0.2, explosion_resistance: 1.0, light_block: Some(1), light_emission: 0 },
    TileInfo { id: 19, name: "sponge", icon: "sponge", class: "Sponge", material: None, destroy_time: 0.6, explosion_resistance: 3.0, light_block: None, light_emission: 0 },
    TileInfo { id: 20, name: "glass", icon: "glass", class: "GlassTile", material: Some(Material::Glass), destroy_time: 0.3, explosion_resistance: 1.5, light_block: None, light_emission: 0 },
    TileInfo { id: 21, name: "lapisOre", icon: "lapis_ore", class: "OreTile", material: Some(Material::Stone), destroy_time: 3.0, explosion_resistance: 15.0, light_block: None, light_emission: 0 },
    TileInfo { id: 22, name: "lapisBlock", icon: "lapis_block", class: "Tile", material: Some(Material::Stone), destroy_time: 3.0, explosion_resistance: 15.0, light_block: None, light_emission: 0 },
    TileInfo { id: 23, name: "dispenser", icon: "dispenser", class: "DispenserTile", material: None, destroy_time: 3.5, explosion_resistance: 17.5, light_block: None, light_emission: 0 },
    TileInfo { id: 24, name: "sandStone", icon: "sandstone", class: "SandStoneTile", material: None, destroy_time: 0.8, explosion_resistance: 4.0, light_block: None, light_emission: 0 },
    TileInfo { id: 25, name: "noteblock", icon: "noteblock", class: "NoteBlockTile", material: None, destroy_time: 0.8, explosion_resistance: 4.0, light_block: None, light_emission: 0 },
    TileInfo { id: 26, name: "bed", icon: "bed", class: "BedTile", material: None, destroy_time: 0.2, explosion_resistance: 1.0, light_block: None, light_emission: 0 },
    TileInfo { id: 27, name: "goldenRail", icon: "rail_golden", class: "PoweredRailTile", material: None, destroy_time: 0.7, explosion_resistance: 3.5, light_block: None, light_emission: 0 },
    TileInfo { id: 28, name: "detectorRail", icon: "rail_detector", class: "DetectorRailTile", material: None, destroy_time: 0.7, explosion_resistance: 3.5, light_block: None, light_emission: 0 },
    TileInfo { id: 29, name: "pistonStickyBase", icon: "pistonStickyBase", class: "PistonBaseTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 30, name: "web", icon: "web", class: "WebTile", material: None, destroy_time: 4.0, explosion_resistance: 20.0, light_block: Some(1), light_emission: 0 },
    TileInfo { id: 31, name: "tallgrass", icon: "tallgrass", class: "TallGrass", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 32, name: "deadBush", icon: "deadbush", class: "DeadBushTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 33, name: "pistonBase", icon: "pistonBase", class: "PistonBaseTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 34, name: "pistonExtensionPiece", icon: "pistonExtension", class: "PistonExtensionTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 35, name: "wool", icon: "wool_colored", class: "ColoredTile", material: Some(Material::Cloth), destroy_time: 0.8, explosion_resistance: 4.0, light_block: None, light_emission: 0 },
    TileInfo { id: 36, name: "pistonMovingPiece", icon: "pistonMovingPiece", class: "PistonMovingPiece", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 37, name: "flower", icon: "flower_dandelion", class: "Bush", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 38, name: "rose", icon: "flower_rose", class: "Bush", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 39, name: "mushroom_brown", icon: "mushroom_brown", class: "Mushroom", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 1 },
    TileInfo { id: 40, name: "mushroom_red", icon: "mushroom_red", class: "Mushroom", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 1 },
    TileInfo { id: 41, name: "goldBlock", icon: "gold_block", class: "MetalTile", material: None, destroy_time: 3.0, explosion_resistance: 30.0, light_block: None, light_emission: 0 },
    TileInfo { id: 42, name: "ironBlock", icon: "iron_block", class: "MetalTile", material: None, destroy_time: 5.0, explosion_resistance: 30.0, light_block: None, light_emission: 0 },
    TileInfo { id: 43, name: "stoneSlab", icon: "stoneSlab", class: "Tile", material: Some(Material::Stone), destroy_time: 2.0, explosion_resistance: 30.0, light_block: None, light_emission: 0 },
    TileInfo { id: 44, name: "stoneSlabHalf", icon: "stoneSlabHalf", class: "Tile", material: Some(Material::Stone), destroy_time: 2.0, explosion_resistance: 30.0, light_block: None, light_emission: 0 },
    TileInfo { id: 45, name: "redBrick", icon: "brick", class: "Tile", material: Some(Material::Stone), destroy_time: 2.0, explosion_resistance: 30.0, light_block: None, light_emission: 0 },
    TileInfo { id: 46, name: "tnt", icon: "tnt", class: "TntTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 47, name: "bookshelf", icon: "bookshelf", class: "BookshelfTile", material: None, destroy_time: 1.5, explosion_resistance: 7.5, light_block: None, light_emission: 0 },
    TileInfo { id: 48, name: "mossyCobblestone", icon: "cobblestone_mossy", class: "Tile", material: Some(Material::Stone), destroy_time: 2.0, explosion_resistance: 30.0, light_block: None, light_emission: 0 },
    TileInfo { id: 49, name: "obsidian", icon: "obsidian", class: "ObsidianTile", material: Some(Material::Stone), destroy_time: 50.0, explosion_resistance: 6000.0, light_block: None, light_emission: 0 },
    TileInfo { id: 50, name: "torch", icon: "torch_on", class: "TorchTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 14 },
    TileInfo { id: 51, name: "fire", icon: "fire", class: "FireTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 15 },
    TileInfo { id: 52, name: "mobSpawner", icon: "mob_spawner", class: "MobSpawnerTile", material: None, destroy_time: 5.0, explosion_resistance: 25.0, light_block: None, light_emission: 0 },
    TileInfo { id: 53, name: "stairs_wood", icon: "stairsWood", class: "StairTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 54, name: "chest", icon: "chest", class: "ChestTile", material: None, destroy_time: 2.5, explosion_resistance: 12.5, light_block: None, light_emission: 0 },
    TileInfo { id: 55, name: "redStoneDust", icon: "redstone_dust", class: "RedStoneDustTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 56, name: "diamondOre", icon: "diamond_ore", class: "OreTile", material: Some(Material::Stone), destroy_time: 3.0, explosion_resistance: 15.0, light_block: None, light_emission: 0 },
    TileInfo { id: 57, name: "diamondBlock", icon: "diamond_block", class: "MetalTile", material: None, destroy_time: 5.0, explosion_resistance: 30.0, light_block: None, light_emission: 0 },
    TileInfo { id: 58, name: "workBench", icon: "crafting_table", class: "WorkbenchTile", material: None, destroy_time: 2.5, explosion_resistance: 12.5, light_block: None, light_emission: 0 },
    TileInfo { id: 59, name: "wheat", icon: "wheat", class: "CropTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 60, name: "farmland", icon: "farmland", class: "FarmTile", material: None, destroy_time: 0.6, explosion_resistance: 3.0, light_block: None, light_emission: 0 },
    TileInfo { id: 61, name: "furnace", icon: "furnace", class: "FurnaceTile", material: None, destroy_time: 3.5, explosion_resistance: 17.5, light_block: None, light_emission: 0 },
    TileInfo { id: 62, name: "furnace_lit", icon: "furnace", class: "FurnaceTile", material: None, destroy_time: 3.5, explosion_resistance: 17.5, light_block: None, light_emission: 13 },
    TileInfo { id: 63, name: "sign", icon: "sign", class: "SignTile", material: None, destroy_time: 1.0, explosion_resistance: 5.0, light_block: None, light_emission: 0 },
    TileInfo { id: 64, name: "door_wood", icon: "door_wood", class: "DoorTile", material: Some(Material::Wood), destroy_time: 3.0, explosion_resistance: 15.0, light_block: None, light_emission: 0 },
    TileInfo { id: 65, name: "ladder", icon: "ladder", class: "LadderTile", material: None, destroy_time: 0.4, explosion_resistance: 2.0, light_block: None, light_emission: 0 },
    TileInfo { id: 66, name: "rail", icon: "rail_normal", class: "RailTile", material: None, destroy_time: 0.7, explosion_resistance: 3.5, light_block: None, light_emission: 0 },
    TileInfo { id: 67, name: "stairs_stone", icon: "stairsStone", class: "StairTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 68, name: "wallSign", icon: "sign", class: "SignTile", material: None, destroy_time: 1.0, explosion_resistance: 5.0, light_block: None, light_emission: 0 },
    TileInfo { id: 69, name: "lever", icon: "lever", class: "LeverTile", material: None, destroy_time: 0.5, explosion_resistance: 2.5, light_block: None, light_emission: 0 },
    TileInfo { id: 70, name: "pressurePlate_stone", icon: "pressurePlate_stone", class: "PressurePlateTile", material: Some(Material::Stone), destroy_time: 0.5, explosion_resistance: 2.5, light_block: None, light_emission: 0 },
    TileInfo { id: 71, name: "door_iron", icon: "door_iron", class: "DoorTile", material: Some(Material::Metal), destroy_time: 5.0, explosion_resistance: 25.0, light_block: None, light_emission: 0 },
    TileInfo { id: 72, name: "pressurePlate_wood", icon: "pressurePlate_wood", class: "PressurePlateTile", material: Some(Material::Wood), destroy_time: 0.5, explosion_resistance: 2.5, light_block: None, light_emission: 0 },
    TileInfo { id: 73, name: "redStoneOre", icon: "redstone_ore", class: "RedStoneOreTile", material: None, destroy_time: 3.0, explosion_resistance: 15.0, light_block: None, light_emission: 0 },
    TileInfo { id: 74, name: "redStoneOre_lit", icon: "redstone_ore", class: "RedStoneOreTile", material: None, destroy_time: 3.0, explosion_resistance: 15.0, light_block: None, light_emission: 9 },
    TileInfo { id: 75, name: "redstoneTorch_off", icon: "redstone_torch_off", class: "NotGateTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 76, name: "redstoneTorch_on", icon: "redstone_torch_on", class: "NotGateTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 7 },
    TileInfo { id: 77, name: "button_stone", icon: "button", class: "StoneButtonTile", material: None, destroy_time: 0.5, explosion_resistance: 2.5, light_block: None, light_emission: 0 },
    TileInfo { id: 78, name: "topSnow", icon: "snow", class: "TopSnowTile", material: None, destroy_time: 0.1, explosion_resistance: 0.5, light_block: Some(0), light_emission: 0 },
    TileInfo { id: 79, name: "ice", icon: "ice", class: "IceTile", material: None, destroy_time: 0.5, explosion_resistance: 2.5, light_block: Some(3), light_emission: 0 },
    TileInfo { id: 80, name: "snow", icon: "snow", class: "SnowTile", material: None, destroy_time: 0.2, explosion_resistance: 1.0, light_block: None, light_emission: 0 },
    TileInfo { id: 81, name: "cactus", icon: "cactus", class: "CactusTile", material: None, destroy_time: 0.4, explosion_resistance: 2.0, light_block: None, light_emission: 0 },
    TileInfo { id: 82, name: "clay", icon: "clay", class: "ClayTile", material: None, destroy_time: 0.6, explosion_resistance: 3.0, light_block: None, light_emission: 0 },
    TileInfo { id: 83, name: "reeds", icon: "reeds", class: "ReedTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 84, name: "jukebox", icon: "jukebox", class: "JukeboxTile", material: None, destroy_time: 2.0, explosion_resistance: 30.0, light_block: None, light_emission: 0 },
    TileInfo { id: 85, name: "fence", icon: "fence", class: "FenceTile", material: Some(Material::Wood), destroy_time: 2.0, explosion_resistance: 15.0, light_block: None, light_emission: 0 },
    TileInfo { id: 86, name: "pumpkin", icon: "pumpkin", class: "PumpkinTile", material: None, destroy_time: 1.0, explosion_resistance: 5.0, light_block: None, light_emission: 0 },
    TileInfo { id: 87, name: "netherRack", icon: "netherrack", class: "NetherrackTile", material: None, destroy_time: 0.4, explosion_resistance: 2.0, light_block: None, light_emission: 0 },
    TileInfo { id: 88, name: "soulsand", icon: "soul_sand", class: "SoulSandTile", material: None, destroy_time: 0.5, explosion_resistance: 2.5, light_block: None, light_emission: 0 },
    TileInfo { id: 89, name: "glowstone", icon: "glowstone", class: "Glowstonetile", material: Some(Material::Glass), destroy_time: 0.3, explosion_resistance: 1.5, light_block: None, light_emission: 15 },
    TileInfo { id: 90, name: "portalTile", icon: "portal", class: "PortalTile", material: None, destroy_time: -1.0, explosion_resistance: 0.0, light_block: None, light_emission: 11 },
    TileInfo { id: 91, name: "litPumpkin", icon: "pumpkin", class: "PumpkinTile", material: None, destroy_time: 1.0, explosion_resistance: 5.0, light_block: None, light_emission: 15 },
    TileInfo { id: 92, name: "cake", icon: "cake", class: "CakeTile", material: None, destroy_time: 0.5, explosion_resistance: 2.5, light_block: None, light_emission: 0 },
    TileInfo { id: 93, name: "diode_off", icon: "repeater_off", class: "RepeaterTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 94, name: "diode_on", icon: "repeater_on", class: "RepeaterTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 9 },
    TileInfo { id: 95, name: "stained_glass", icon: "glass", class: "StainedGlassBlock", material: Some(Material::Glass), destroy_time: 0.3, explosion_resistance: 1.5, light_block: None, light_emission: 0 },
    TileInfo { id: 96, name: "trapdoor", icon: "trapdoor", class: "TrapDoorTile", material: Some(Material::Wood), destroy_time: 3.0, explosion_resistance: 15.0, light_block: None, light_emission: 0 },
    TileInfo { id: 97, name: "monsterStoneEgg", icon: "monsterStoneEgg", class: "StoneMonsterTile", material: None, destroy_time: 0.75, explosion_resistance: 3.75, light_block: None, light_emission: 0 },
    TileInfo { id: 98, name: "stoneBrick", icon: "stonebrick", class: "SmoothStoneBrickTile", material: None, destroy_time: 1.5, explosion_resistance: 30.0, light_block: None, light_emission: 0 },
    TileInfo { id: 99, name: "hugeMushroom_brown", icon: "mushroom_block", class: "HugeMushroomTile", material: Some(Material::Wood), destroy_time: 0.2, explosion_resistance: 1.0, light_block: None, light_emission: 0 },
    TileInfo { id: 100, name: "hugeMushroom_red", icon: "mushroom_block", class: "HugeMushroomTile", material: Some(Material::Wood), destroy_time: 0.2, explosion_resistance: 1.0, light_block: None, light_emission: 0 },
    TileInfo { id: 101, name: "ironFence", icon: "ironFence", class: "ThinFenceTile", material: Some(Material::Metal), destroy_time: 5.0, explosion_resistance: 30.0, light_block: None, light_emission: 0 },
    TileInfo { id: 102, name: "thinGlass", icon: "thinGlass", class: "ThinFenceTile", material: Some(Material::Glass), destroy_time: 0.3, explosion_resistance: 1.5, light_block: None, light_emission: 0 },
    TileInfo { id: 103, name: "melon", icon: "melon", class: "MelonTile", material: None, destroy_time: 1.0, explosion_resistance: 5.0, light_block: None, light_emission: 0 },
    TileInfo { id: 104, name: "pumpkinStem", icon: "pumpkin_stem", class: "StemTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 105, name: "melonStem", icon: "melon_stem", class: "StemTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 106, name: "vine", icon: "vine", class: "VineTile", material: None, destroy_time: 0.2, explosion_resistance: 1.0, light_block: None, light_emission: 0 },
    TileInfo { id: 107, name: "fenceGate", icon: "fenceGate", class: "FenceGateTile", material: None, destroy_time: 2.0, explosion_resistance: 15.0, light_block: None, light_emission: 0 },
    TileInfo { id: 108, name: "stairs_bricks", icon: "stairsBrick", class: "StairTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 109, name: "stairs_stoneBrick", icon: "stairsStoneBrickSmooth", class: "StairTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 110, name: "mycel", icon: "mycelium", class: "MycelTile", material: None, destroy_time: 0.6, explosion_resistance: 3.0, light_block: None, light_emission: 0 },
    TileInfo { id: 111, name: "waterLily", icon: "waterlily", class: "WaterlilyTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 112, name: "netherBrick", icon: "nether_brick", class: "Tile", material: Some(Material::Stone), destroy_time: 2.0, explosion_resistance: 30.0, light_block: None, light_emission: 0 },
    TileInfo { id: 113, name: "netherFence", icon: "netherFence", class: "FenceTile", material: Some(Material::Stone), destroy_time: 2.0, explosion_resistance: 30.0, light_block: None, light_emission: 0 },
    TileInfo { id: 114, name: "stairs_netherBricks", icon: "stairsNetherBrick", class: "StairTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 115, name: "netherStalk", icon: "nether_wart", class: "NetherWartTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 116, name: "enchantTable", icon: "enchanting_table", class: "EnchantmentTableTile", material: None, destroy_time: 5.0, explosion_resistance: 6000.0, light_block: None, light_emission: 0 },
    TileInfo { id: 117, name: "brewingStand", icon: "brewing_stand", class: "BrewingStandTile", material: None, destroy_time: 0.5, explosion_resistance: 2.5, light_block: None, light_emission: 1 },
    TileInfo { id: 118, name: "cauldron", icon: "cauldron", class: "CauldronTile", material: None, destroy_time: 2.0, explosion_resistance: 10.0, light_block: None, light_emission: 0 },
    TileInfo { id: 119, name: "endPortalTile", icon: "endPortalTile", class: "TheEndPortal", material: Some(Material::Portal), destroy_time: -1.0, explosion_resistance: 18000000.0, light_block: None, light_emission: 0 },
    TileInfo { id: 120, name: "endPortalFrameTile", icon: "endframe", class: "TheEndPortalFrameTile", material: None, destroy_time: -1.0, explosion_resistance: 18000000.0, light_block: None, light_emission: 1 },
    TileInfo { id: 121, name: "endStone", icon: "end_stone", class: "Tile", material: Some(Material::Stone), destroy_time: 3.0, explosion_resistance: 45.0, light_block: None, light_emission: 0 },
    TileInfo { id: 122, name: "dragonEgg", icon: "dragon_egg", class: "EggTile", material: None, destroy_time: 3.0, explosion_resistance: 45.0, light_block: None, light_emission: 1 },
    TileInfo { id: 123, name: "redstoneLight", icon: "redstone_lamp_off", class: "RedlightTile", material: None, destroy_time: 0.3, explosion_resistance: 1.5, light_block: None, light_emission: 0 },
    TileInfo { id: 124, name: "redstoneLight_lit", icon: "redstone_lamp_on", class: "RedlightTile", material: None, destroy_time: 0.3, explosion_resistance: 1.5, light_block: None, light_emission: 15 },
    TileInfo { id: 125, name: "woodSlab", icon: "woodSlab", class: "Tile", material: Some(Material::Wood), destroy_time: 2.0, explosion_resistance: 15.0, light_block: None, light_emission: 0 },
    TileInfo { id: 126, name: "woodSlabHalf", icon: "woodSlabHalf", class: "Tile", material: Some(Material::Wood), destroy_time: 2.0, explosion_resistance: 15.0, light_block: None, light_emission: 0 },
    TileInfo { id: 127, name: "cocoa", icon: "cocoa", class: "CocoaTile", material: None, destroy_time: 0.2, explosion_resistance: 15.0, light_block: None, light_emission: 0 },
    TileInfo { id: 128, name: "stairs_sandstone", icon: "stairsSandstone", class: "StairTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 129, name: "emeraldOre", icon: "emerald_ore", class: "OreTile", material: Some(Material::Stone), destroy_time: 3.0, explosion_resistance: 15.0, light_block: None, light_emission: 0 },
    TileInfo { id: 130, name: "enderChest", icon: "enderChest", class: "EnderChestTile", material: None, destroy_time: 22.5, explosion_resistance: 3000.0, light_block: None, light_emission: 7 },
    TileInfo { id: 131, name: "tripWireSource", icon: "trip_wire_source", class: "TripWireSourceTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 132, name: "tripWire", icon: "trip_wire", class: "TripWireTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 133, name: "emeraldBlock", icon: "emerald_block", class: "MetalTile", material: None, destroy_time: 5.0, explosion_resistance: 30.0, light_block: None, light_emission: 0 },
    TileInfo { id: 134, name: "stairs_sprucewood", icon: "stairsWoodSpruce", class: "StairTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 135, name: "stairs_birchwood", icon: "stairsWoodBirch", class: "StairTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 136, name: "stairs_junglewood", icon: "stairsWoodJungle", class: "StairTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 137, name: "commandBlock", icon: "command_block", class: "CommandBlock", material: None, destroy_time: -1.0, explosion_resistance: 18000000.0, light_block: None, light_emission: 0 },
    TileInfo { id: 138, name: "beacon", icon: "beacon", class: "BeaconTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 15 },
    TileInfo { id: 139, name: "cobbleWall", icon: "cobbleWall", class: "WallTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 140, name: "flowerPot", icon: "flower_pot", class: "FlowerPotTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 141, name: "carrots", icon: "carrots", class: "CarrotTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 142, name: "potatoes", icon: "potatoes", class: "PotatoTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 143, name: "button_wood", icon: "button", class: "WoodButtonTile", material: None, destroy_time: 0.5, explosion_resistance: 2.5, light_block: None, light_emission: 0 },
    TileInfo { id: 144, name: "skull", icon: "skull", class: "SkullTile", material: None, destroy_time: 1.0, explosion_resistance: 5.0, light_block: None, light_emission: 0 },
    TileInfo { id: 145, name: "anvil", icon: "anvil", class: "AnvilTile", material: None, destroy_time: 5.0, explosion_resistance: 6000.0, light_block: None, light_emission: 0 },
    TileInfo { id: 146, name: "chest_trap", icon: "chest_trap", class: "ChestTile", material: None, destroy_time: 2.5, explosion_resistance: 12.5, light_block: None, light_emission: 0 },
    TileInfo { id: 147, name: "weightedPlate_light", icon: "weightedPlate_light", class: "WeightedPressurePlateTile", material: Some(Material::Metal), destroy_time: 0.5, explosion_resistance: 2.5, light_block: None, light_emission: 0 },
    TileInfo { id: 148, name: "weightedPlate_heavy", icon: "weightedPlate_heavy", class: "WeightedPressurePlateTile", material: Some(Material::Metal), destroy_time: 0.5, explosion_resistance: 2.5, light_block: None, light_emission: 0 },
    TileInfo { id: 149, name: "comparator_off", icon: "comparator_off", class: "ComparatorTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 150, name: "comparator_on", icon: "comparator_on", class: "ComparatorTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 9 },
    TileInfo { id: 151, name: "daylightDetector", icon: "daylight_detector", class: "DaylightDetectorTile", material: None, destroy_time: 0.2, explosion_resistance: 1.0, light_block: None, light_emission: 0 },
    TileInfo { id: 152, name: "redstoneBlock", icon: "redstone_block", class: "PoweredMetalTile", material: None, destroy_time: 5.0, explosion_resistance: 30.0, light_block: None, light_emission: 0 },
    TileInfo { id: 153, name: "netherQuartz", icon: "quartz_ore", class: "OreTile", material: Some(Material::Stone), destroy_time: 3.0, explosion_resistance: 15.0, light_block: None, light_emission: 0 },
    TileInfo { id: 154, name: "hopper", icon: "hopper", class: "HopperTile", material: None, destroy_time: 3.0, explosion_resistance: 24.0, light_block: None, light_emission: 0 },
    TileInfo { id: 155, name: "quartzBlock", icon: "quartz_block", class: "QuartzBlockTile", material: None, destroy_time: 0.8, explosion_resistance: 4.0, light_block: None, light_emission: 0 },
    TileInfo { id: 156, name: "stairs_quartz", icon: "stairsQuartz", class: "StairTile", material: None, destroy_time: 0.0, explosion_resistance: 0.0, light_block: None, light_emission: 0 },
    TileInfo { id: 157, name: "activatorRail", icon: "rail_activator", class: "PoweredRailTile", material: None, destroy_time: 0.7, explosion_resistance: 3.5, light_block: None, light_emission: 0 },
    TileInfo { id: 158, name: "dropper", icon: "dropper", class: "DropperTile", material: None, destroy_time: 3.5, explosion_resistance: 17.5, light_block: None, light_emission: 0 },
    TileInfo { id: 159, name: "clayHardened_colored", icon: "hardened_clay_stained", class: "ColoredTile", material: Some(Material::Stone), destroy_time: 1.25, explosion_resistance: 21.0, light_block: None, light_emission: 0 },
    TileInfo { id: 160, name: "stained_glass_pane", icon: "glass", class: "StainedGlassPaneBlock", material: None, destroy_time: 0.3, explosion_resistance: 1.5, light_block: None, light_emission: 0 },
    TileInfo { id: 170, name: "hayBlock", icon: "hay_block", class: "HayBlockTile", material: None, destroy_time: 0.5, explosion_resistance: 2.5, light_block: None, light_emission: 0 },
    TileInfo { id: 171, name: "woolCarpet", icon: "woolCarpet", class: "WoolCarpetTile", material: None, destroy_time: 0.1, explosion_resistance: 0.5, light_block: Some(0), light_emission: 0 },
    TileInfo { id: 172, name: "clayHardened", icon: "hardened_clay", class: "Tile", material: Some(Material::Stone), destroy_time: 1.25, explosion_resistance: 21.0, light_block: None, light_emission: 0 },
    TileInfo { id: 173, name: "coalBlock", icon: "coal_block", class: "Tile", material: Some(Material::Stone), destroy_time: 5.0, explosion_resistance: 30.0, light_block: None, light_emission: 0 },
];
