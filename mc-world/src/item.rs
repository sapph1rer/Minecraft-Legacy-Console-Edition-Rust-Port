//! Items (`Item`, `ItemInstance` and the tool / weapon / armour / food subclasses), layer 6 of the port plan.
//!
//! The item table itself ([`ITEM_TABLE`]) is generated from `Item::staticCtor`; this file holds the types, the
//! tool behaviour (`getDestroySpeed`, `canDestroySpecial`) and `ItemInstance`.
//! Ids below 256 are blocks (`BlockItem`), ids from 256 up are items (`256 + constructor id`).

use crate::item_table::ITEM_TABLE;
use crate::material::Material;
use crate::tile::{ids as t, tile_info};
use crate::tile_props;

pub use crate::item_table::ids;

/// `Item::Tier` (`_Tier`): harvest level, uses, speed, attack bonus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Wood,
    Stone,
    Iron,
    Diamond,
    Gold,
}

impl Tier {
    pub const fn level(self) -> i32 {
        match self {
            Tier::Wood | Tier::Gold => 0,
            Tier::Stone => 1,
            Tier::Iron => 2,
            Tier::Diamond => 3,
        }
    }
    pub const fn uses(self) -> u16 {
        match self {
            Tier::Wood => 59,
            Tier::Stone => 131,
            Tier::Iron => 250,
            Tier::Diamond => 1561,
            Tier::Gold => 32,
        }
    }
    pub const fn speed(self) -> f32 {
        match self {
            Tier::Wood => 2.0,
            Tier::Stone => 4.0,
            Tier::Iron => 6.0,
            Tier::Diamond => 8.0,
            Tier::Gold => 12.0,
        }
    }
    pub const fn attack_bonus(self) -> f32 {
        match self {
            Tier::Wood | Tier::Gold => 0.0,
            Tier::Stone => 1.0,
            Tier::Iron => 2.0,
            Tier::Diamond => 3.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArmorMaterial {
    Cloth,
    Chain,
    Iron,
    Gold,
    Diamond,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArmorSlot {
    Head = 0,
    Torso = 1,
    Legs = 2,
    Feet = 3,
}

impl ArmorMaterial {
    /// `ArmorMaterial::getDefenseForSlot` (helmet, chestplate, leggings, boots)
    pub const fn defense(self, slot: ArmorSlot) -> i32 {
        let a: [i32; 4] = match self {
            ArmorMaterial::Cloth => [1, 3, 2, 1],
            ArmorMaterial::Chain => [2, 5, 4, 1],
            ArmorMaterial::Iron => [2, 6, 5, 2],
            ArmorMaterial::Gold => [2, 5, 3, 1],
            ArmorMaterial::Diamond => [3, 8, 6, 3],
        };
        a[slot as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemKind {
    Plain,
    Sword(Tier),
    Shovel(Tier),
    Pickaxe(Tier),
    Axe(Tier),
    Hoe(Tier),
    Armor { material: ArmorMaterial, slot: ArmorSlot },
    Food { nutrition: u8, saturation: f32, meat: bool },
    Door,
    /// `BucketItem` with the block it places (0 = empty bucket)
    Bucket(u8),
    /// `TilePlanterItem`: places this block
    Planter(u8),
    Coal,
    Dye,
    Sign,
    Bed,
}

#[derive(Clone, Copy, Debug)]
pub struct ItemDef {
    pub id: u16,
    /// name of the `Item::xxx` static
    pub name: &'static str,
    pub display: &'static str,
    /// texture key in `items.png`
    pub icon: &'static str,
    /// (row, column) of the icon in the 16×16 items atlas
    pub icon_pos: (u8, u8),
    pub kind: ItemKind,
    pub max_stack: u8,
    pub max_damage: u16,
}

pub fn item_def(id: u16) -> Option<&'static ItemDef> {
    ITEM_TABLE.binary_search_by_key(&id, |d| d.id).ok().map(|i| &ITEM_TABLE[i])
}

/// `ItemInstance`: an id (block or item), a count and the damage / aux value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemStack {
    pub id: u16,
    pub count: u8,
    pub damage: u16,
}

impl ItemStack {
    pub const fn new(id: u16, count: u8) -> ItemStack {
        ItemStack { id, count, damage: 0 }
    }
    pub const fn with_damage(id: u16, count: u8, damage: u16) -> ItemStack {
        ItemStack { id, count, damage }
    }
    pub fn block(id: u8, count: u8) -> ItemStack {
        ItemStack::new(id as u16, count)
    }
    pub fn is_block(&self) -> bool {
        self.id < 256
    }
    pub fn def(&self) -> Option<&'static ItemDef> {
        item_def(self.id)
    }
    pub fn kind(&self) -> ItemKind {
        self.def().map_or(ItemKind::Plain, |d| d.kind)
    }
    /// `ItemInstance::getMaxStackSize`
    pub fn max_stack(&self) -> u8 {
        if self.id < 256 {
            // blocks: 64, except a few single-stack ones in the item table (none) — beds etc. are items here
            64
        } else {
            self.def().map_or(64, |d| d.max_stack)
        }
    }
    pub fn max_damage(&self) -> u16 {
        self.def().map_or(0, |d| d.max_damage)
    }
    pub fn is_damageable(&self) -> bool {
        self.max_damage() > 0 && self.max_stack() == 1
    }
    pub fn is_stackable(&self) -> bool {
        self.max_stack() > 1 && (!self.is_damageable() || self.damage == 0)
    }
    /// `ItemInstance::sameItemWithTags`-ish: same id and aux value
    pub fn same_item(&self, o: &ItemStack) -> bool {
        self.id == o.id && self.damage == o.damage
    }
    /// `ItemInstance::hurtAndBreak`: returns true when the item broke.
    pub fn hurt_and_break(&mut self, amount: u16) -> bool {
        if !self.is_damageable() {
            return false;
        }
        self.damage = self.damage.saturating_add(amount);
        self.damage >= self.max_damage()
    }
    pub fn display_name(&self) -> String {
        if self.id >= 256 {
            if self.id == ids::COAL && self.damage == 1 {
                return "Charcoal".into();
            }
            if self.id == ids::DYE_POWDER {
                return format!("{} Dye", DYE_NAMES[(self.damage & 15) as usize]);
            }
            return self.def().map_or_else(|| format!("Item {}", self.id), |d| d.display.to_string());
        }
        block_display_name(self.id as u8, self.damage as u8)
    }
}

pub const DYE_NAMES: [&str; 16] = [
    "Ink Sac", "Rose Red", "Cactus Green", "Cocoa Beans", "Lapis Lazuli", "Purple", "Cyan", "Light Gray", "Gray", "Pink", "Lime", "Dandelion Yellow", "Light Blue", "Magenta", "Orange", "Bone Meal",
];
pub const WOOL_NAMES: [&str; 16] = [
    "White", "Orange", "Magenta", "Light Blue", "Yellow", "Lime", "Pink", "Gray", "Light Gray", "Cyan", "Purple", "Blue", "Brown", "Green", "Red", "Black",
];

/// English name of a block-item (`IDS_TILE_*`).
pub fn block_display_name(id: u8, data: u8) -> String {
    let wool = |s: &str| format!("{} {}", WOOL_NAMES[(data & 15) as usize], s);
    match id {
        t::WOOL => return wool("Wool"),
        t::STAINED_GLASS => return wool("Stained Glass"),
        t::WOOD => return ["Oak Wood Planks", "Spruce Wood Planks", "Birch Wood Planks", "Jungle Wood Planks"][(data & 3) as usize].into(),
        t::TREE_TRUNK => return ["Oak Wood", "Spruce Wood", "Birch Wood", "Jungle Wood"][(data & 3) as usize].into(),
        t::LEAVES => return ["Oak Leaves", "Spruce Leaves", "Birch Leaves", "Jungle Leaves"][(data & 3) as usize].into(),
        t::SAPLING => return ["Oak Sapling", "Spruce Sapling", "Birch Sapling", "Jungle Sapling"][(data & 3) as usize].into(),
        t::SAND_STONE => return ["Sandstone", "Chiseled Sandstone", "Smooth Sandstone"][(data as usize).min(2)].into(),
        t::STONE_SLAB_HALF | t::STONE_SLAB => return ["Stone Slab", "Sandstone Slab", "Wooden Slab", "Cobblestone Slab", "Brick Slab", "Stone Brick Slab", "Nether Brick Slab", "Quartz Slab"][(data & 7) as usize].into(),
        t::WOOD_SLAB | t::WOOD_SLAB_HALF => return "Wooden Slab".into(),
        t::STONE_BRICK => return ["Stone Bricks", "Mossy Stone Bricks", "Cracked Stone Bricks", "Chiseled Stone Bricks"][(data & 3) as usize].into(),
        t::TALLGRASS => return ["Dead Shrub", "Tall Grass", "Fern"][(data as usize).min(2)].into(),
        t::COBBLE_WALL => return if data == 1 { "Mossy Cobblestone Wall".into() } else { "Cobblestone Wall".into() },
        _ => {}
    }
    let Some(info) = tile_info(id) else { return format!("Block {id}") };
    let n = match info.name {
        "dirt" => "Dirt".into(),
        "workBench" => "Crafting Table".to_string(),
        "furnace" | "furnace_lit" => "Furnace".into(),
        "treeTrunk" => "Wood".into(),
        "ironOre" => "Iron Ore".into(),
        "stoneBrick" => "Stone Bricks".into(),
        "wood" => "Wood Planks".into(),
        other => {
            // camelCase / snake_case -> Title Case
            let mut out = String::new();
            let mut prev_lower = false;
            for c in other.chars() {
                if c == '_' {
                    out.push(' ');
                    prev_lower = false;
                    continue;
                }
                if c.is_uppercase() && prev_lower {
                    out.push(' ');
                }
                out.push(c);
                prev_lower = c.is_lowercase();
            }
            out.split(' ').map(|w| {
                let mut cs = w.chars();
                cs.next().map(|f| f.to_uppercase().collect::<String>() + cs.as_str()).unwrap_or_default()
            }).collect::<Vec<_>>().join(" ")
        }
    };
    n
}

// ------------------------------------------------------------------------------------------ tools

const PICKAXE_DIGGABLES: &[u8] = &[
    t::COBBLESTONE, t::STONE_SLAB, t::STONE_SLAB_HALF, t::STONE, t::SAND_STONE, t::MOSSY_COBBLESTONE, t::IRON_ORE, t::IRON_BLOCK, t::COAL_ORE, t::GOLD_BLOCK, t::GOLD_ORE, t::DIAMOND_ORE, t::DIAMOND_BLOCK, t::ICE, t::NETHER_RACK, t::LAPIS_ORE, t::LAPIS_BLOCK, t::RED_STONE_ORE, t::RED_STONE_ORE_LIT, t::RAIL, t::DETECTOR_RAIL, t::GOLDEN_RAIL,
];
const SHOVEL_DIGGABLES: &[u8] = &[t::GRASS, t::DIRT, t::SAND, t::GRAVEL, t::TOP_SNOW, t::SNOW, t::CLAY, t::FARMLAND, t::SOULSAND, t::MYCEL];
const HATCHET_DIGGABLES: &[u8] = &[t::WOOD, t::BOOKSHELF, t::TREE_TRUNK, t::CHEST, t::STONE_SLAB, t::STONE_SLAB_HALF, t::PUMPKIN, t::LIT_PUMPKIN];

/// `Item::getDestroySpeed(itemInstance, tile)` for the selected item (`None` = bare hand).
pub fn item_destroy_speed(stack: Option<&ItemStack>, tile: u8) -> f32 {
    let Some(s) = stack else { return 1.0 };
    let mat = tile_props::material(tile);
    let list = |l: &[u8], tier: Tier| if l.contains(&tile) { tier.speed() } else { 1.0 };
    match s.kind() {
        ItemKind::Pickaxe(tier) => {
            if matches!(mat, Material::Metal | Material::HeavyMetal | Material::Stone) {
                tier.speed()
            } else {
                list(PICKAXE_DIGGABLES, tier)
            }
        }
        ItemKind::Shovel(tier) => list(SHOVEL_DIGGABLES, tier),
        ItemKind::Axe(tier) => {
            if matches!(mat, Material::Wood | Material::Plant | Material::ReplaceablePlant) {
                tier.speed()
            } else {
                list(HATCHET_DIGGABLES, tier)
            }
        }
        ItemKind::Hoe(_) => 1.0,
        ItemKind::Sword(_) => {
            if tile == t::WEB {
                15.0
            } else if matches!(mat, Material::Plant | Material::ReplaceablePlant | Material::Coral | Material::Leaves | Material::Vegetable) {
                1.5
            } else {
                1.0
            }
        }
        _ => {
            if s.id == ids::SHEARS {
                if tile == t::WEB || mat == Material::Leaves {
                    return 15.0;
                }
                if tile == t::WOOL {
                    return 5.0;
                }
            }
            1.0
        }
    }
}

/// `Item::canDestroySpecial(tile)`: does the item harvest blocks the bare hand cannot?
pub fn item_can_destroy_special(stack: Option<&ItemStack>, tile: u8) -> bool {
    let Some(s) = stack else { return false };
    let mat = tile_props::material(tile);
    match s.kind() {
        ItemKind::Pickaxe(tier) => {
            let lvl = tier.level();
            if tile == t::OBSIDIAN {
                return lvl == 3;
            }
            if matches!(tile, t::DIAMOND_BLOCK | t::DIAMOND_ORE | t::EMERALD_BLOCK | t::EMERALD_ORE | t::GOLD_BLOCK | t::GOLD_ORE | t::RED_STONE_ORE | t::RED_STONE_ORE_LIT) {
                return lvl >= 2;
            }
            if matches!(tile, t::IRON_BLOCK | t::IRON_ORE | t::LAPIS_BLOCK | t::LAPIS_ORE) {
                return lvl >= 1;
            }
            matches!(mat, Material::Stone | Material::Metal | Material::HeavyMetal)
        }
        ItemKind::Shovel(_) => tile == t::TOP_SNOW || tile == t::SNOW,
        ItemKind::Sword(_) => tile == t::WEB,
        _ => s.id == ids::SHEARS && (tile == t::WEB || tile == t::WOOL || mat == Material::Leaves),
    }
}

/// `Player::canDestroy`: bare hands harvest "always destroyable" materials; otherwise the item must be the right tool.
pub fn can_harvest(stack: Option<&ItemStack>, tile: u8) -> bool {
    tile_props::material(tile).is_always_destroyable() || item_can_destroy_special(stack, tile)
}

/// Damage dealt by an attack with this item (`WeaponItem`: 4 + tier bonus, digger tools: 1+base+bonus, else 1).
pub fn item_attack_damage(stack: Option<&ItemStack>) -> f32 {
    let Some(s) = stack else { return 1.0 };
    match s.kind() {
        ItemKind::Sword(t) => 4.0 + t.attack_bonus(),
        ItemKind::Shovel(t) => 1.0 + t.attack_bonus(),
        ItemKind::Pickaxe(t) => 2.0 + t.attack_bonus(),
        ItemKind::Axe(t) => 3.0 + t.attack_bonus(),
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_looks_up() {
        assert!(ITEM_TABLE.windows(2).all(|w| w[0].id < w[1].id));
        let d = item_def(ids::PICK_AXE_IRON).unwrap();
        assert_eq!(d.kind, ItemKind::Pickaxe(Tier::Iron));
        assert_eq!(d.max_stack, 1);
        assert_eq!(d.max_damage, 250);
        assert_eq!(item_def(ids::STICK).unwrap().display, "Stick");
        assert_eq!(ItemStack::new(ids::PICK_AXE_WOOD, 1).display_name(), "Wooden Pickaxe");
    }

    #[test]
    fn tool_speeds_and_harvest_levels() {
        let wood = ItemStack::new(ids::PICK_AXE_WOOD, 1);
        let iron = ItemStack::new(ids::PICK_AXE_IRON, 1);
        let dia = ItemStack::new(ids::PICK_AXE_DIAMOND, 1);
        assert_eq!(item_destroy_speed(Some(&wood), t::STONE), 2.0);
        assert_eq!(item_destroy_speed(Some(&dia), t::STONE), 8.0);
        assert_eq!(item_destroy_speed(None, t::STONE), 1.0);
        assert!(!can_harvest(None, t::STONE));
        assert!(can_harvest(Some(&wood), t::STONE));
        assert!(!can_harvest(Some(&wood), t::IRON_ORE));
        assert!(can_harvest(Some(&iron), t::DIAMOND_ORE));
        assert!(!can_harvest(Some(&ItemStack::new(ids::PICK_AXE_STONE, 1)), t::DIAMOND_ORE));
        assert!(can_harvest(Some(&dia), t::OBSIDIAN));
        assert!(can_harvest(None, t::DIRT));
        let axe = ItemStack::new(ids::HATCHET_IRON, 1);
        assert_eq!(item_destroy_speed(Some(&axe), t::TREE_TRUNK), 6.0);
    }

    #[test]
    fn stacks() {
        let mut p = ItemStack::new(ids::PICK_AXE_WOOD, 1);
        assert!(!p.is_stackable() || p.damage == 0);
        assert!(!p.hurt_and_break(58));
        assert!(p.hurt_and_break(1));
        assert!(ItemStack::block(t::STONE, 1).is_stackable());
        assert_eq!(ItemStack::block(t::WOOL, 1).display_name(), "White Wool");
    }
}
