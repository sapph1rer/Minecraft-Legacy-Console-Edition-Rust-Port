//! Crafting (`Recipes`, `ShapedRecipy`, `ShapelessRecipy`) and smelting (`FurnaceRecipes`).
//! The recipe lists are generated from the original source files ([`crate::recipe_table`]).

use crate::item::{ids, ItemStack};
use crate::recipe_table::{SHAPED, SHAPELESS};
use crate::tile::ids as t;

/// `Recipes::ANY_AUX_VALUE`
pub const ANY_AUX: u16 = 32767;

pub struct Shaped {
    /// (id, count, aux)
    pub result: (u16, u8, u16),
    pub width: usize,
    pub height: usize,
    /// row-major `width × height`, `(id, aux)`; `None` = empty
    pub ingredients: &'static [Option<(u16, u16)>],
}

pub struct Shapeless {
    pub result: (u16, u8, u16),
    pub ingredients: &'static [(u16, u16)],
}

fn stack_of(r: (u16, u8, u16)) -> ItemStack {
    ItemStack::with_damage(r.0, r.1, r.2)
}

fn aux_ok(expected: u16, got: u16) -> bool {
    expected == ANY_AUX || expected == got
}

/// A crafting grid: 2×2 (inventory) or 3×3 (crafting table). Slots are row-major.
pub struct CraftingGrid<'a> {
    pub width: usize,
    pub slots: &'a [Option<ItemStack>],
}

impl CraftingGrid<'_> {
    fn get(&self, x: usize, y: usize) -> Option<&ItemStack> {
        if x < self.width && y < self.width {
            self.slots.get(x + y * self.width).and_then(|s| s.as_ref())
        } else {
            None
        }
    }
}

impl Shaped {
    fn matches_at(&self, g: &CraftingGrid, xo: usize, yo: usize, flip: bool) -> bool {
        for x in 0..3 {
            for y in 0..3 {
                let expected = if x >= xo && y >= yo && x - xo < self.width && y - yo < self.height {
                    let (xs, ys) = (x - xo, y - yo);
                    let idx = if flip { (self.width - xs - 1) + ys * self.width } else { xs + ys * self.width };
                    self.ingredients[idx]
                } else {
                    None
                };
                match (g.get(x, y), expected) {
                    (None, None) => {}
                    (Some(it), Some((id, aux))) => {
                        if it.id != id || !aux_ok(aux, it.damage) {
                            return false;
                        }
                    }
                    _ => return false,
                }
            }
        }
        true
    }

    pub fn matches(&self, g: &CraftingGrid) -> bool {
        if self.width > 3 || self.height > 3 {
            return false;
        }
        for xo in 0..=(3 - self.width) {
            for yo in 0..=(3 - self.height) {
                if self.matches_at(g, xo, yo, true) || self.matches_at(g, xo, yo, false) {
                    return true;
                }
            }
        }
        false
    }
}

impl Shapeless {
    pub fn matches(&self, g: &CraftingGrid) -> bool {
        let mut left: Vec<(u16, u16)> = self.ingredients.to_vec();
        for y in 0..g.width {
            for x in 0..g.width {
                if let Some(it) = g.get(x, y) {
                    let Some(pos) = left.iter().position(|&(id, aux)| id == it.id && aux_ok(aux, it.damage)) else { return false };
                    left.remove(pos);
                }
            }
        }
        left.is_empty()
    }
}

/// `Recipes::getItemFor`: the result of the grid, if any (tool repair included).
pub fn find_recipe(g: &CraftingGrid) -> Option<ItemStack> {
    // two identical damaged tools combine into one (with a 10 % bonus)
    let items: Vec<&ItemStack> = (0..g.width * g.width).filter_map(|i| g.slots.get(i).and_then(|s| s.as_ref())).collect();
    if items.len() == 2 && items[0].id == items[1].id && items[0].count == 1 && items[1].count == 1 && items[0].is_damageable() {
        let max = items[0].max_damage() as i32;
        let r1 = max - items[0].damage as i32;
        let r2 = max - items[1].damage as i32;
        let total = r1 + r2 + max * 10 / 100;
        let dmg = (max - total).max(0) as u16;
        return Some(ItemStack::with_damage(items[0].id, 1, dmg));
    }
    for r in SHAPED {
        if r.matches(g) {
            return Some(stack_of(r.result));
        }
    }
    for r in SHAPELESS {
        if r.matches(g) {
            return Some(stack_of(r.result));
        }
    }
    None
}

/// All recipes as results (for a recipe browser).
pub fn all_results() -> impl Iterator<Item = ItemStack> {
    SHAPED.iter().map(|r| stack_of(r.result)).chain(SHAPELESS.iter().map(|r| stack_of(r.result)))
}

// ------------------------------------------------------------------------------------------ furnace

/// `FurnaceRecipes`: smelting result and experience for an input id.
pub fn furnace_result(id: u16) -> Option<(ItemStack, f32)> {
    let id8 = id as u8;
    let r = |item: u16, aux: u16, xp: f32| Some((ItemStack::with_damage(item, 1, aux), xp));
    if id < 256 {
        return match id8 {
            t::IRON_ORE => r(ids::IRON_INGOT, 0, 0.7),
            t::GOLD_ORE => r(ids::GOLD_INGOT, 0, 1.0),
            t::DIAMOND_ORE => r(ids::DIAMOND, 0, 1.0),
            t::SAND => r(t::GLASS as u16, 0, 0.1),
            t::COBBLESTONE => r(t::STONE as u16, 0, 0.1),
            t::CLAY => r(t::CLAY_HARDENED as u16, 0, 0.35),
            t::CACTUS => r(ids::DYE_POWDER, 2, 0.2),
            t::TREE_TRUNK => r(ids::COAL, 1, 0.15),
            t::EMERALD_ORE => r(ids::EMERALD, 0, 1.0),
            t::NETHER_RACK => r(ids::NETHERBRICK, 0, 0.1),
            t::COAL_ORE => r(ids::COAL, 0, 0.1),
            t::RED_STONE_ORE => r(ids::RED_STONE, 0, 0.7),
            t::LAPIS_ORE => r(ids::DYE_POWDER, 4, 0.2),
            t::NETHER_QUARTZ => r(ids::NETHER_QUARTZ, 0, 0.2),
            _ => None,
        };
    }
    match id {
        ids::PORK_CHOP_RAW => r(ids::PORK_CHOP_COOKED, 0, 0.35),
        ids::BEEF_RAW => r(ids::BEEF_COOKED, 0, 0.35),
        ids::CHICKEN_RAW => r(ids::CHICKEN_COOKED, 0, 0.35),
        ids::FISH_RAW => r(ids::FISH_COOKED, 0, 0.35),
        ids::CLAY => r(ids::BRICK, 0, 0.3),
        ids::POTATO => r(ids::POTATO_BAKED, 0, 0.35),
        _ => None,
    }
}

/// `FurnaceTileEntity::getBurnDuration` in ticks (0 = not a fuel).
pub fn fuel_ticks(s: &ItemStack) -> i32 {
    use crate::material::Material;
    if s.id < 256 {
        let id = s.id as u8;
        if id == t::SAPLING {
            return 100;
        }
        if id == t::COAL_BLOCK {
            return 16000;
        }
        if crate::tile_props::material(id) == Material::Wood {
            return 300;
        }
        return 0;
    }
    match s.id {
        ids::STICK => 100,
        ids::COAL => 1600,
        ids::BUCKET_LAVA => 20000,
        ids::BLAZE_ROD => 2400,
        _ => match s.kind() {
            crate::item::ItemKind::Sword(crate::item::Tier::Wood) | crate::item::ItemKind::Pickaxe(crate::item::Tier::Wood) | crate::item::ItemKind::Shovel(crate::item::Tier::Wood) | crate::item::ItemKind::Axe(crate::item::Tier::Wood) | crate::item::ItemKind::Hoe(crate::item::Tier::Wood) => 200,
            _ => 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(w: usize, cells: &[Option<ItemStack>]) -> Vec<Option<ItemStack>> {
        let mut v = vec![None; w * w];
        for (i, c) in cells.iter().enumerate() {
            v[i] = *c;
        }
        v
    }
    fn s(id: u16) -> Option<ItemStack> {
        Some(ItemStack::new(id, 1))
    }

    #[test]
    fn logs_make_four_planks_and_planks_make_sticks() {
        let g = grid(2, &[Some(ItemStack::with_damage(t::TREE_TRUNK as u16, 1, 0))]);
        let r = find_recipe(&CraftingGrid { width: 2, slots: &g }).unwrap();
        assert_eq!((r.id, r.count), (t::WOOD as u16, 4));
        let w = t::WOOD as u16;
        let g = grid(2, &[s(w), None, s(w), None]);
        let r = find_recipe(&CraftingGrid { width: 2, slots: &g }).unwrap();
        assert_eq!((r.id, r.count), (ids::STICK, 4));
    }

    #[test]
    fn crafting_table_needs_2x2_planks_and_pickaxe_needs_a_3x3_grid() {
        let w = t::WOOD as u16;
        let g = grid(2, &[s(w), s(w), s(w), s(w)]);
        assert_eq!(find_recipe(&CraftingGrid { width: 2, slots: &g }).unwrap().id, t::WORK_BENCH as u16);
        let c = ids::STICK;
        let cb = t::COBBLESTONE as u16;
        let g = grid(3, &[s(cb), s(cb), s(cb), None, s(c), None, None, s(c), None]);
        assert_eq!(find_recipe(&CraftingGrid { width: 3, slots: &g }).unwrap().id, ids::PICK_AXE_STONE);
        // the same shape is impossible in the 2x2 grid
        let g2 = grid(2, &[s(cb), s(cb), None, s(c)]);
        assert!(find_recipe(&CraftingGrid { width: 2, slots: &g2 }).is_none());
    }

    #[test]
    fn mirrored_shapes_and_shapeless_dye_work() {
        // axe mirrored: "XX" / "#X" / "# "
        let w = t::WOOD as u16;
        let g = grid(3, &[s(w), s(w), None, s(ids::STICK), s(w), None, s(ids::STICK), None, None]);
        assert_eq!(find_recipe(&CraftingGrid { width: 3, slots: &g }).unwrap().id, ids::HATCHET_WOOD);
        let g = grid(2, &[Some(ItemStack::with_damage(ids::DYE_POWDER, 1, 1)), Some(ItemStack::block(t::WOOL, 1))]);
        let r = find_recipe(&CraftingGrid { width: 2, slots: &g }).unwrap();
        assert_eq!((r.id, r.damage), (t::WOOL as u16, 14)); // red wool
    }

    #[test]
    fn repairing_and_smelting() {
        let g = grid(2, &[Some(ItemStack::with_damage(ids::PICK_AXE_IRON, 1, 200)), Some(ItemStack::with_damage(ids::PICK_AXE_IRON, 1, 200))]);
        let r = find_recipe(&CraftingGrid { width: 2, slots: &g }).unwrap();
        assert_eq!(r.damage, 250 - (50 + 50 + 25));
        let (out, _) = furnace_result(t::IRON_ORE as u16).unwrap();
        assert_eq!(out.id, ids::IRON_INGOT);
        assert_eq!(fuel_ticks(&ItemStack::new(ids::COAL, 1)), 1600);
    }

    #[test]
    fn all_recipes_have_valid_results() {
        let n = all_results().count();
        assert!(n > 250, "{n}");
        for r in all_results() {
            assert!(r.id >= 256 || crate::tile::tile_info(r.id as u8).is_some(), "bad result {}", r.id);
            assert!(r.id < 256 || crate::item::item_def(r.id).is_some(), "bad item {}", r.id);
        }
    }
}
