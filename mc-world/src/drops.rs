//! What a block drops when a player breaks it (`Tile::getResource` / `getResourceCount` /
//! `getSpawnResourcesAuxValue` and the `spawnResources` overrides of the tile subclasses).

use crate::item::{ids, ItemStack};
use crate::tile::ids as t;
use mc_core::Random;

/// Items dropped by the block `id`/`data`. `shears` = the player holds shears (`playerDestroy` overrides).
/// `bonus` is the Fortune level (0 here: enchantments are not ported).
pub fn block_drops(id: u8, data: u8, random: &mut Random, shears: bool, bonus: i32) -> Vec<ItemStack> {
    let one = |item: u16, aux: u16| vec![ItemStack::with_damage(item, 1, aux)];
    let many = |item: u16, aux: u16, n: i32| if n > 0 { vec![ItemStack::with_damage(item, n.min(64) as u8, aux)] } else { Vec::new() };
    let itself = |aux: u8| vec![ItemStack::with_damage(id as u16, 1, aux as u16)];
    match id {
        0 | t::WATER | t::CALM_WATER | t::LAVA | t::CALM_LAVA | t::FIRE | t::PORTAL_TILE | t::END_PORTAL_TILE | t::END_PORTAL_FRAME_TILE | t::UNBREAKABLE | t::MOB_SPAWNER | t::GLASS | t::STAINED_GLASS | t::THIN_GLASS | t::STAINED_GLASS_PANE | t::ICE | t::CAKE | t::PISTON_EXTENSION_PIECE | t::PISTON_MOVING_PIECE | t::MONSTER_STONE_EGG | t::SKULL | t::COMMAND_BLOCK => Vec::new(),
        t::STONE => one(t::COBBLESTONE as u16, 0),
        t::GRASS | t::FARMLAND | t::MYCEL => one(t::DIRT as u16, 0),
        t::COAL_ORE => many(ids::COAL, 0, 1),
        t::DIAMOND_ORE => many(ids::DIAMOND, 0, 1 + fortune(random, bonus)),
        t::EMERALD_ORE => many(ids::EMERALD, 0, 1 + fortune(random, bonus)),
        t::NETHER_QUARTZ => many(ids::NETHER_QUARTZ, 0, 1 + fortune(random, bonus)),
        t::LAPIS_ORE => many(ids::DYE_POWDER, 4, 4 + random.next_int_bounded(5)),
        t::RED_STONE_ORE | t::RED_STONE_ORE_LIT => many(ids::RED_STONE, 0, 4 + random.next_int_bounded(2) + random.next_int_bounded(bonus + 1)),
        t::GRAVEL => {
            let b = bonus.min(3);
            if random.next_int_bounded(10 - b * 3) == 0 {
                one(ids::FLINT, 0)
            } else {
                itself(0)
            }
        }
        t::LEAVES => {
            if shears {
                return itself(data & 3);
            }
            let mut out = Vec::new();
            let mut chance = if data & 3 == 3 { 40 } else { 20 };
            if bonus > 0 {
                chance = (chance - (2 << bonus)).max(10);
            }
            if random.next_int_bounded(chance) == 0 {
                out.push(ItemStack::with_damage(t::SAPLING as u16, 1, (data & 3) as u16));
            }
            let mut chance = 200;
            if bonus > 0 {
                chance = (chance - (10 << bonus)).max(40);
            }
            if data & 3 == 0 && random.next_int_bounded(chance) == 0 {
                out.push(ItemStack::new(ids::APPLE, 1));
            }
            out
        }
        t::TALLGRASS => {
            if shears {
                return itself(data);
            }
            if random.next_int_bounded(8) == 0 {
                many(ids::SEEDS_WHEAT, 0, 1 + random.next_int_bounded(bonus * 2 + 1))
            } else {
                Vec::new()
            }
        }
        t::DEAD_BUSH | t::VINE => {
            if shears {
                itself(0)
            } else {
                Vec::new()
            }
        }
        t::WEB => {
            if shears {
                itself(0)
            } else {
                one(ids::STRING, 0)
            }
        }
        t::GLOWSTONE => many(ids::YELLOW_DUST, 0, (2 + random.next_int_bounded(3 + bonus)).min(4)),
        t::BOOKSHELF => many(ids::BOOK, 0, 3),
        t::CLAY => many(ids::CLAY, 0, 4),
        t::SNOW => many(ids::SNOW_BALL, 0, 4),
        t::TOP_SNOW => many(ids::SNOW_BALL, 0, (data & 7) as i32 + 1),
        t::MELON => many(ids::MELON, 0, (3 + random.next_int_bounded(5) + random.next_int_bounded(bonus + 1)).min(9)),
        t::REEDS => one(ids::REEDS, 0),
        t::RED_STONE_DUST => one(ids::RED_STONE, 0),
        t::REDSTONE_TORCH_OFF | t::REDSTONE_TORCH_ON => itself_as(t::REDSTONE_TORCH_ON),
        t::DOOR_WOOD => {
            if data & 8 != 0 {
                Vec::new()
            } else {
                one(ids::DOOR_WOOD, 0)
            }
        }
        t::DOOR_IRON => {
            if data & 8 != 0 {
                Vec::new()
            } else {
                one(ids::DOOR_IRON, 0)
            }
        }
        t::SIGN | t::WALL_SIGN => one(ids::SIGN, 0),
        t::BED => {
            if data & 8 != 0 {
                one(ids::BED, 0)
            } else {
                Vec::new()
            }
        }
        t::DIODE_OFF | t::DIODE_ON => one(ids::REPEATER, 0),
        t::COMPARATOR_OFF | t::COMPARATOR_ON => one(ids::COMPARATOR, 0),
        t::CAULDRON => one(ids::CAULDRON, 0),
        t::BREWING_STAND => one(ids::BREWING_STAND, 0),
        t::FLOWER_POT => one(ids::FLOWER_POT, 0),
        t::FURNACE_LIT => itself_as(t::FURNACE),
        t::TRIP_WIRE => one(ids::STRING, 0),
        t::WHEAT => {
            let mut out = many(ids::SEEDS_WHEAT, 0, 1);
            if data >= 7 {
                out.push(ItemStack::new(ids::WHEAT, 1));
                out.push(ItemStack::new(ids::SEEDS_WHEAT, (random.next_int_bounded(4)).max(0) as u8));
                out.retain(|s| s.count > 0);
            }
            out
        }
        t::CARROTS => many(ids::CARROTS, 0, if data >= 7 { 1 + random.next_int_bounded(4) } else { 1 }),
        t::POTATOES => {
            let mut v = many(ids::POTATO, 0, if data >= 7 { 1 + random.next_int_bounded(4) } else { 1 });
            if data >= 7 && random.next_int_bounded(50) == 0 {
                v.push(ItemStack::new(ids::POTATO_POISONOUS, 1));
            }
            v
        }
        t::NETHER_STALK => many(ids::NETHERWART_SEEDS, 0, if data >= 3 { 2 + random.next_int_bounded(3) } else { 1 }),
        t::PUMPKIN_STEM => many(ids::SEEDS_PUMPKIN, 0, 0),
        t::MELON_STEM => many(ids::SEEDS_MELON, 0, 0),
        t::COCOA => many(ids::DYE_POWDER, 3, if (data >> 2) >= 2 { 3 } else { 1 }),
        t::HUGE_MUSHROOM_BROWN | t::HUGE_MUSHROOM_RED => {
            let n = (random.next_int_bounded(10) - 7).max(0);
            many(if id == t::HUGE_MUSHROOM_BROWN { t::MUSHROOM_BROWN as u16 } else { t::MUSHROOM_RED as u16 }, 0, n)
        }
        t::STONE_SLAB => many(t::STONE_SLAB_HALF as u16, (data & 7) as u16, 2),
        t::WOOD_SLAB => many(t::WOOD_SLAB_HALF as u16, (data & 7) as u16, 2),
        t::STONE_SLAB_HALF | t::WOOD_SLAB_HALF => itself(data & 7),
        t::WOOD | t::WOOL | t::SAND_STONE | t::STONE_BRICK | t::CLAY_HARDENED_COLORED | t::WOOL_CARPET | t::COBBLE_WALL => itself(data),
        t::QUARTZ_BLOCK => itself(if data >= 2 { 2 } else { data }),
        t::TREE_TRUNK | t::SAPLING => itself(data & 3),
        t::BEACON | t::ANVIL => itself(0),
        _ => itself(0),
    }
}

fn fortune(random: &mut Random, bonus: i32) -> i32 {
    if bonus > 0 {
        (random.next_int_bounded(bonus + 2) - 1).max(0)
    } else {
        0
    }
}

fn itself_as(id: u8) -> Vec<ItemStack> {
    vec![ItemStack::new(id as u16, 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_drops() {
        let mut r = Random::with_seed(1);
        assert_eq!(block_drops(t::STONE, 0, &mut r, false, 0), vec![ItemStack::new(t::COBBLESTONE as u16, 1)]);
        assert_eq!(block_drops(t::GRASS, 0, &mut r, false, 0)[0].id, t::DIRT as u16);
        let lapis = block_drops(t::LAPIS_ORE, 0, &mut r, false, 0);
        assert_eq!(lapis[0].id, ids::DYE_POWDER);
        assert_eq!(lapis[0].damage, 4);
        assert!((4..=8).contains(&lapis[0].count));
        assert!(block_drops(t::GLASS, 0, &mut r, false, 0).is_empty());
        assert_eq!(block_drops(t::WOOL, 5, &mut r, false, 0)[0].damage, 5);
        assert_eq!(block_drops(t::STONE_SLAB, 3, &mut r, false, 0)[0].count, 2);
        // leaves with shears drop themselves; sapling chance is about 1 in 20 without
        assert_eq!(block_drops(t::LEAVES, 1, &mut r, true, 0)[0].id, t::LEAVES as u16);
        let saplings: usize = (0..2000).map(|_| block_drops(t::LEAVES, 0, &mut r, false, 0).iter().filter(|s| s.id == t::SAPLING as u16).count()).sum();
        assert!((60..140).contains(&saplings), "{saplings}");
        let flint: usize = (0..1000).map(|_| block_drops(t::GRAVEL, 0, &mut r, false, 0).iter().filter(|s| s.id == ids::FLINT).count()).sum();
        assert!((60..140).contains(&flint), "{flint}");
    }
}
