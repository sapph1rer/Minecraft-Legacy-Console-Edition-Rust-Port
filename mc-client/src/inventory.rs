//! `Inventory` (36 slots + 4 armour) and the container menus (`InventoryMenu`, `CraftingMenu`, a creative picker):
//! slot layout, pick-up / place / swap / split clicks, shift-click `quickMoveStack`, crafting result handling.

use mc_world::item::{ids, item_def, ItemKind, ItemStack};
use mc_world::recipe::{all_results, find_recipe, CraftingGrid};

pub const HOTBAR: usize = 9;
pub const INV_SIZE: usize = 36;

#[derive(Clone, Debug, PartialEq)]
pub struct Inventory {
    pub items: [Option<ItemStack>; INV_SIZE],
    /// 0 = helmet, 1 = chestplate, 2 = leggings, 3 = boots (display order of `InventoryMenu`)
    pub armor: [Option<ItemStack>; 4],
    pub selected: usize,
}

impl Default for Inventory {
    fn default() -> Self {
        Inventory { items: [None; INV_SIZE], armor: [None; 4], selected: 0 }
    }
}

impl Inventory {
    pub fn new() -> Inventory {
        Inventory::default()
    }

    pub fn selected_item(&self) -> Option<&ItemStack> {
        self.items[self.selected].as_ref()
    }

    /// `Inventory::add`: merge into matching stacks first, then use the first empty slot. Returns whether everything fit;
    /// `stack.count` holds what is left over.
    pub fn add(&mut self, stack: &mut ItemStack) -> bool {
        if stack.is_stackable() {
            for s in self.items.iter_mut().flatten() {
                if s.same_item(stack) && s.count < s.max_stack() {
                    let n = (s.max_stack() - s.count).min(stack.count);
                    s.count += n;
                    stack.count -= n;
                    if stack.count == 0 {
                        return true;
                    }
                }
            }
        }
        while stack.count > 0 {
            let Some(free) = self.items.iter().position(|s| s.is_none()) else { return false };
            let n = stack.count.min(stack.max_stack());
            let mut put = *stack;
            put.count = n;
            self.items[free] = Some(put);
            stack.count -= n;
        }
        true
    }

    /// Take one item from the selected slot (placing a block, eating, ...).
    pub fn use_selected(&mut self) {
        if let Some(s) = &mut self.items[self.selected] {
            s.count = s.count.saturating_sub(1);
            if s.count == 0 {
                self.items[self.selected] = None;
            }
        }
    }

    /// Damage the selected tool; removes it when it breaks. Returns true when it broke.
    pub fn damage_selected(&mut self, amount: u16) -> bool {
        if let Some(s) = &mut self.items[self.selected] {
            if s.hurt_and_break(amount) {
                self.items[self.selected] = None;
                return true;
            }
        }
        false
    }

    /// Total defence points of the worn armour (`Player::getArmorValue`).
    pub fn armor_value(&self) -> i32 {
        self.armor
            .iter()
            .flatten()
            .map(|s| match s.kind() {
                ItemKind::Armor { material, slot } => material.defense(slot),
                _ => 0,
            })
            .sum()
    }

    pub fn count_of(&self, id: u16) -> u32 {
        self.items.iter().flatten().filter(|s| s.id == id).map(|s| s.count as u32).sum()
    }
}

// ================================================================================================ menus

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotKind {
    Inv(usize),
    /// 0 = helmet … 3 = boots
    Armor(usize),
    Craft(usize),
    Result,
    /// an entry of the creative item pool
    Pool(usize),
    /// a slot of the open chest
    Chest(usize),
}

#[derive(Clone, Copy, Debug)]
pub struct SlotDef {
    pub kind: SlotKind,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKind {
    /// `InventoryMenu`: 2×2 crafting + armour
    Inventory,
    /// `CraftingMenu`: 3×3 crafting table
    Crafting,
    /// creative item picker
    Creative,
    /// `ContainerMenu`: a chest (3 rows)
    Chest,
}

#[derive(Clone, Debug)]
pub struct Menu {
    pub kind: MenuKind,
    pub slots: Vec<SlotDef>,
    pub craft_w: usize,
    pub craft: Vec<Option<ItemStack>>,
    pub result: Option<ItemStack>,
    pub carried: Option<ItemStack>,
    pub pool: Vec<ItemStack>,
    pub scroll: usize,
    /// creative: every pickable stack and the active category tab
    pub all_pool: Vec<ItemStack>,
    pub tab: usize,
    /// creative: search text (empty = show the category)
    pub search: String,
    /// chest: the 27 slots, and the tile entity contents they came from (to keep enchantment data untouched)
    pub chest: Vec<Option<ItemStack>>,
    chest_src: Vec<Option<mc_level::tile_entity::ItemStack>>,
}

pub const POOL_COLS: usize = 9;
pub const POOL_ROWS: usize = 5;

/// Creative tabs: (title, icon item id).
pub const CREATIVE_TABS: [(&str, u16); 5] = [("Building Blocks", 1), ("Decoration", 38), ("Tools & Combat", 276), ("Food", 260), ("Materials & Misc", 280)];

/// Which creative tab a stack lives in.
pub fn creative_category(st: &ItemStack) -> usize {
    use crate::blocks::Kind;
    if st.id < 256 {
        return match crate::blocks::style(st.id as u8, st.damage as u8).kind {
            Kind::Cube | Kind::Box { .. } | Kind::Shaped => 0,
            _ => 1,
        };
    }
    match st.kind() {
        ItemKind::Sword(_) | ItemKind::Pickaxe(_) | ItemKind::Axe(_) | ItemKind::Shovel(_) | ItemKind::Hoe(_) | ItemKind::Armor { .. } => 2,
        ItemKind::Food { .. } => 3,
        _ if matches!(st.id, 261 | 262 | 259 | 346 | 359 | 325 | 326 | 327 | 332 | 344) => 2,
        _ => 4,
    }
}

/// Every block and item the creative picker offers (original ordering by id; hidden/technical ids left out).
pub fn creative_pool() -> Vec<ItemStack> {
    use mc_world::tile_ids as t;
    let mut v = Vec::new();
    let skip_blocks = [0, 8, 9, 10, 11, 26, 34, 36, 51, 55, 59, 62, 63, 64, 68, 71, 74, 75, 83, 90, 92, 93, 94, 95, 97, 99, 100, 104, 105, 115, 117, 118, 119, 120, 124, 125, 126, 127, 140, 141, 142, 144];
    for id in 1u16..=173 {
        if skip_blocks.contains(&(id as u8)) {
            continue;
        }
        if mc_world::tile::tile_info(id as u8).is_none() {
            continue;
        }
        v.push(ItemStack::block(id as u8, 1));
        // variants with data values
        let variants: &[u16] = match id as u8 {
            t::TREE_TRUNK | t::WOOD | t::SAPLING => &[1, 2, 3],
            t::WOOL => &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            t::STONE_SLAB | t::STONE_SLAB_HALF => &[1, 2, 3, 4, 5, 6],
            t::SAND_STONE => &[1, 2],
            t::STONE_BRICK => &[1, 2, 3],
            _ => &[],
        };
        for &d in variants {
            v.push(ItemStack::with_damage(id, 1, d));
        }
    }
    let mut seen: Vec<ItemStack> = Vec::new();
    let mut ids_sorted: Vec<u16> = (256u16..2300).filter(|i| item_def(*i).is_some()).collect();
    ids_sorted.sort();
    for id in ids_sorted {
        let s = ItemStack::new(id, 1);
        if id == ids::DYE_POWDER {
            for d in 0..16 {
                seen.push(ItemStack::with_damage(id, 1, d));
            }
        } else {
            seen.push(s);
        }
    }
    v.extend(seen);
    v
}

fn slot_accepts_armor(kind: SlotKind, st: &ItemStack) -> bool {
    match kind {
        SlotKind::Armor(i) => matches!(st.kind(), ItemKind::Armor { slot, .. } if slot as usize == i),
        SlotKind::Result => false,
        _ => true,
    }
}

impl Menu {
    pub fn inventory() -> Menu {
        let mut slots = vec![SlotDef { kind: SlotKind::Result, x: 144, y: 36 }];
        for y in 0..2 {
            for x in 0..2 {
                slots.push(SlotDef { kind: SlotKind::Craft(x + y * 2), x: 88 + x as i32 * 18, y: 26 + y as i32 * 18 });
            }
        }
        for i in 0..4 {
            slots.push(SlotDef { kind: SlotKind::Armor(i), x: 8, y: 8 + i as i32 * 18 });
        }
        Self::push_player_slots(&mut slots, 84, 142);
        let mut m = Menu { kind: MenuKind::Inventory, slots, craft_w: 2, craft: vec![None; 4], result: None, carried: None, pool: Vec::new(), scroll: 0, all_pool: Vec::new(), tab: 0, search: String::new(), chest: Vec::new(), chest_src: Vec::new() };
        m.update_result();
        m
    }

    pub fn crafting() -> Menu {
        let mut slots = vec![SlotDef { kind: SlotKind::Result, x: 124, y: 35 }];
        for y in 0..3 {
            for x in 0..3 {
                slots.push(SlotDef { kind: SlotKind::Craft(x + y * 3), x: 30 + x as i32 * 18, y: 17 + y as i32 * 18 });
            }
        }
        Self::push_player_slots(&mut slots, 84, 142);
        let mut m = Menu { kind: MenuKind::Crafting, slots, craft_w: 3, craft: vec![None; 9], result: None, carried: None, pool: Vec::new(), scroll: 0, all_pool: Vec::new(), tab: 0, search: String::new(), chest: Vec::new(), chest_src: Vec::new() };
        m.update_result();
        m
    }

    pub fn creative() -> Menu {
        let all = creative_pool();
        let mut m = Menu { kind: MenuKind::Creative, slots: Vec::new(), craft_w: 0, craft: Vec::new(), result: None, carried: None, pool: Vec::new(), scroll: 0, all_pool: all, tab: 0, search: String::new(), chest: Vec::new(), chest_src: Vec::new() };
        m.set_tab(0);
        m
    }

    /// `ChestMenu` with a single chest (3 rows): 27 chest slots over the player inventory.
    pub fn chest(contents: &[Option<mc_level::tile_entity::ItemStack>]) -> Menu {
        // `ChestMenu`: 3 rows for one chest, 6 for a large one; the inventory sits `(rows - 4) * 18` lower than in the 4-row layout
        let rows = (contents.len() / 9).clamp(3, 6);
        let mut slots = Vec::new();
        for r in 0..rows {
            for c in 0..9 {
                slots.push(SlotDef { kind: SlotKind::Chest(c + r * 9), x: 8 + c as i32 * 18, y: 18 + r as i32 * 18 });
            }
        }
        let yo = (rows as i32 - 4) * 18;
        Menu::push_player_slots(&mut slots, 103 + yo, 161 + yo);
        let chest = (0..rows * 9).map(|i| contents.get(i).cloned().flatten().map(|t| ItemStack::with_damage(t.id.clamp(0, 65535) as u16, t.count.clamp(0, 255) as u8, t.damage.clamp(0, 65535) as u16))).collect();
        Menu { kind: MenuKind::Chest, slots, craft_w: 0, craft: Vec::new(), result: None, carried: None, pool: Vec::new(), scroll: 0, all_pool: Vec::new(), tab: 0, search: String::new(), chest, chest_src: contents.to_vec() }
    }

    /// The chest contents to store back (untouched slots keep their original data).
    pub fn chest_contents(&self) -> Vec<Option<mc_level::tile_entity::ItemStack>> {
        let conv = |t: &mc_level::tile_entity::ItemStack| ItemStack::with_damage(t.id.clamp(0, 65535) as u16, t.count.clamp(0, 255) as u8, t.damage.clamp(0, 65535) as u16);
        self.chest
            .iter()
            .enumerate()
            .map(|(i, cur)| {
                let orig = self.chest_src.get(i).cloned().flatten();
                match (cur, orig) {
                    (Some(c), Some(o)) if conv(&o) == *c => Some(o),
                    (Some(c), _) => Some(mc_level::tile_entity::ItemStack { id: c.id as i32, count: c.count as i32, damage: c.damage as i32, enchantments: Vec::new() }),
                    (None, _) => None,
                }
            })
            .collect()
    }

    /// Switch the creative category (`CreativeInventoryScreen` tabs).
    pub fn set_tab(&mut self, tab: usize) {
        self.search.clear();
        self.tab = tab.min(CREATIVE_TABS.len() - 1);
        self.pool = self.all_pool.iter().copied().filter(|st| creative_category(st) == self.tab).collect();
        self.scroll = 0;
        self.rebuild_creative_slots();
    }

    /// Filter the whole pool by name (`CreativeInventoryScreen` search tab); an empty text returns to the category.
    pub fn set_search(&mut self, text: &str) {
        if text.trim().is_empty() {
            let t = self.tab;
            self.set_tab(t);
            return;
        }
        let q = text.trim().to_lowercase();
        self.search = text.to_string();
        self.pool = self.all_pool.iter().copied().filter(|st| st.display_name().to_lowercase().contains(&q)).collect();
        self.scroll = 0;
        self.rebuild_creative_slots();
    }

    fn push_player_slots(slots: &mut Vec<SlotDef>, main_y: i32, hot_y: i32) {
        for y in 0..3 {
            for x in 0..9 {
                slots.push(SlotDef { kind: SlotKind::Inv(x + (y + 1) * 9), x: 8 + x as i32 * 18, y: main_y + y as i32 * 18 });
            }
        }
        for x in 0..9 {
            slots.push(SlotDef { kind: SlotKind::Inv(x), x: 8 + x as i32 * 18, y: hot_y });
        }
    }

    fn rebuild_creative_slots(&mut self) {
        self.slots.clear();
        for r in 0..POOL_ROWS {
            for c in 0..POOL_COLS {
                let idx = (self.scroll + r) * POOL_COLS + c;
                self.slots.push(SlotDef { kind: SlotKind::Pool(idx), x: 9 + c as i32 * 18, y: 20 + r as i32 * 18 });
            }
        }
        for x in 0..9 {
            self.slots.push(SlotDef { kind: SlotKind::Inv(x), x: 9 + x as i32 * 18, y: 124 });
        }
    }

    pub fn max_scroll(&self) -> usize {
        (self.pool.len() + POOL_COLS - 1) / POOL_COLS - POOL_ROWS.min((self.pool.len() + POOL_COLS - 1) / POOL_COLS)
    }

    pub fn scroll_by(&mut self, d: i32) {
        if self.kind != MenuKind::Creative {
            return;
        }
        self.scroll = (self.scroll as i32 + d).clamp(0, self.max_scroll() as i32) as usize;
        self.rebuild_creative_slots();
    }

    /// Re-run the recipe lookup (`slotsChanged`).
    pub fn update_result(&mut self) {
        if self.craft.is_empty() {
            return;
        }
        self.result = find_recipe(&CraftingGrid { width: self.craft_w, slots: &self.craft });
    }

    pub fn get(&self, inv: &Inventory, k: SlotKind) -> Option<ItemStack> {
        match k {
            SlotKind::Inv(i) => inv.items[i],
            SlotKind::Armor(i) => inv.armor[i],
            SlotKind::Craft(i) => self.craft[i],
            SlotKind::Result => self.result,
            SlotKind::Pool(i) => self.pool.get(i).copied(),
            SlotKind::Chest(i) => self.chest.get(i).copied().flatten(),
        }
    }

    fn set(&mut self, inv: &mut Inventory, k: SlotKind, v: Option<ItemStack>) {
        match k {
            SlotKind::Inv(i) => inv.items[i] = v,
            SlotKind::Armor(i) => inv.armor[i] = v,
            SlotKind::Craft(i) => {
                self.craft[i] = v;
                self.update_result();
            }
            SlotKind::Chest(i) => {
                if let Some(c) = self.chest.get_mut(i) {
                    *c = v;
                }
            }
            SlotKind::Result | SlotKind::Pool(_) => {}
        }
    }

    /// Use up one of each ingredient after the result was taken (`ResultSlot::onTake`).
    fn consume_ingredients(&mut self) {
        for s in self.craft.iter_mut() {
            if let Some(st) = s {
                // bucket-style containers stay behind
                let left = match st.id {
                    id if id == ids::BUCKET_MILK => Some(ItemStack::new(ids::BUCKET_EMPTY, 1)),
                    id if id == ids::BUCKET_WATER || id == ids::BUCKET_LAVA => Some(ItemStack::new(ids::BUCKET_EMPTY, 1)),
                    _ => None,
                };
                st.count -= 1;
                if st.count == 0 {
                    *s = left;
                }
            }
        }
        self.update_result();
    }

    /// `AbstractContainerMenu::clicked` (PICKUP) and `QUICK_MOVE`. `slot = None` is a click outside the panel.
    /// Returns stacks that fall into the world.
    pub fn click(&mut self, inv: &mut Inventory, slot: Option<usize>, right: bool, shift: bool) -> Vec<ItemStack> {
        let mut dropped = Vec::new();
        let Some(si) = slot else {
            if let Some(mut c) = self.carried.take() {
                if right && c.count > 1 {
                    let mut one = c;
                    one.count = 1;
                    c.count -= 1;
                    dropped.push(one);
                    self.carried = Some(c);
                } else {
                    dropped.push(c);
                }
            }
            return dropped;
        };
        let kind = self.slots[si].kind;
        if self.kind == MenuKind::Creative {
            self.click_creative(inv, kind, right, shift);
            return dropped;
        }
        if shift {
            self.quick_move(inv, si);
            return dropped;
        }
        if kind == SlotKind::Result {
            if let Some(res) = self.result {
                match &mut self.carried {
                    None => {
                        self.carried = Some(res);
                        self.consume_ingredients();
                    }
                    Some(c) if c.same_item(&res) && c.count as u32 + res.count as u32 <= c.max_stack() as u32 => {
                        c.count += res.count;
                        self.consume_ingredients();
                    }
                    _ => {}
                }
            }
            return dropped;
        }
        let cur = self.get(inv, kind);
        let carried = self.carried;
        match (cur, carried) {
            (None, None) => {}
            (Some(s), None) => {
                // pick up all (left) or half rounded up (right)
                let take = if right { (s.count + 1) / 2 } else { s.count };
                let mut picked = s;
                picked.count = take;
                let rest = s.count - take;
                self.carried = Some(picked);
                self.set(inv, kind, if rest == 0 { None } else { Some(ItemStack { count: rest, ..s }) });
            }
            (None, Some(c)) => {
                if !slot_accepts_armor(kind, &c) {
                    return dropped;
                }
                let n = if right { 1 } else { c.count };
                self.set(inv, kind, Some(ItemStack { count: n, ..c }));
                let rest = c.count - n;
                self.carried = if rest == 0 { None } else { Some(ItemStack { count: rest, ..c }) };
            }
            (Some(s), Some(c)) => {
                if !slot_accepts_armor(kind, &c) {
                    return dropped;
                }
                if s.same_item(&c) && s.is_stackable() {
                    let room = s.max_stack() - s.count;
                    let n = if right { 1.min(room) } else { c.count.min(room) };
                    self.set(inv, kind, Some(ItemStack { count: s.count + n, ..s }));
                    let rest = c.count - n;
                    self.carried = if rest == 0 { None } else { Some(ItemStack { count: rest, ..c }) };
                } else {
                    // swap
                    self.set(inv, kind, Some(c));
                    self.carried = Some(s);
                }
            }
        }
        dropped
    }

    fn click_creative(&mut self, inv: &mut Inventory, kind: SlotKind, right: bool, shift: bool) {
        match kind {
            SlotKind::Pool(i) => {
                let Some(mut st) = self.pool.get(i).copied() else { return };
                if shift {
                    st.count = st.max_stack();
                    inv.add(&mut st);
                } else if self.carried.is_some() {
                    // clicking the picker with something in hand puts it back
                    self.carried = None;
                } else {
                    st.count = if right { 1 } else { st.max_stack() };
                    self.carried = Some(st);
                }
            }
            SlotKind::Inv(i) => {
                let cur = inv.items[i];
                if shift {
                    inv.items[i] = None;
                    return;
                }
                match (cur, self.carried) {
                    (Some(s), None) => {
                        self.carried = Some(s);
                        inv.items[i] = None;
                    }
                    (None, Some(c)) => {
                        inv.items[i] = Some(c);
                        self.carried = None;
                    }
                    (Some(s), Some(c)) => {
                        if s.same_item(&c) && s.is_stackable() {
                            let n = (s.max_stack() - s.count).min(c.count);
                            inv.items[i] = Some(ItemStack { count: s.count + n, ..s });
                            let rest = c.count - n;
                            self.carried = if rest == 0 { None } else { Some(ItemStack { count: rest, ..c }) };
                        } else {
                            inv.items[i] = Some(c);
                            self.carried = Some(s);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// `moveItemStackTo` over slot indices `lo..hi` (optionally from the back). Returns whether anything moved.
    fn move_to(&mut self, inv: &mut Inventory, st: &mut ItemStack, lo: usize, hi: usize, backwards: bool) -> bool {
        let mut moved = false;
        let idxs: Vec<usize> = if backwards { (lo..hi).rev().collect() } else { (lo..hi).collect() };
        if st.is_stackable() {
            for &i in &idxs {
                if st.count == 0 {
                    break;
                }
                let k = self.slots[i].kind;
                if let Some(s) = self.get(inv, k) {
                    if s.same_item(st) && s.count < s.max_stack() {
                        let n = (s.max_stack() - s.count).min(st.count);
                        self.set(inv, k, Some(ItemStack { count: s.count + n, ..s }));
                        st.count -= n;
                        moved = true;
                    }
                }
            }
        }
        for &i in &idxs {
            if st.count == 0 {
                break;
            }
            let k = self.slots[i].kind;
            if self.get(inv, k).is_none() && slot_accepts_armor(k, st) {
                self.set(inv, k, Some(*st));
                st.count = 0;
                moved = true;
            }
        }
        moved
    }

    fn range(&self, f: impl Fn(SlotKind) -> bool) -> (usize, usize) {
        let lo = self.slots.iter().position(|s| f(s.kind)).unwrap_or(0);
        let hi = self.slots.iter().rposition(|s| f(s.kind)).map_or(0, |i| i + 1);
        (lo, hi)
    }

    /// `quickMoveStack` of `InventoryMenu` / `CraftingMenu`.
    pub fn quick_move(&mut self, inv: &mut Inventory, si: usize) {
        let kind = self.slots[si].kind;
        let (inv_lo, inv_hi) = self.range(|k| matches!(k, SlotKind::Inv(_)));
        let Some(mut st) = self.get(inv, kind) else { return };
        match kind {
            SlotKind::Result => {
                // craft repeatedly while it still fits
                let mut guard = 0;
                while let Some(mut res) = self.result {
                    let before = res.count;
                    if !self.move_to(inv, &mut res, inv_lo, inv_hi, true) || res.count == before && false {
                        break;
                    }
                    if res.count > 0 {
                        break;
                    }
                    self.consume_ingredients();
                    guard += 1;
                    if guard > 64 {
                        break;
                    }
                }
                return;
            }
            SlotKind::Inv(i) => {
                // armour goes to its slot first, otherwise main ↔ hotbar
                if let ItemKind::Armor { slot, .. } = st.kind() {
                    if self.slots.iter().any(|s| s.kind == SlotKind::Armor(slot as usize)) && inv.armor[slot as usize].is_none() {
                        inv.armor[slot as usize] = Some(ItemStack { count: 1, ..st });
                        st.count -= 1;
                        inv.items[i] = if st.count == 0 { None } else { Some(st) };
                        return;
                    }
                }
                let (lo, hi) = if self.kind == MenuKind::Chest {
                    self.range(|k| matches!(k, SlotKind::Chest(_)))
                } else if i < HOTBAR {
                    // hotbar → main inventory
                    let main: Vec<usize> = (0..self.slots.len()).filter(|&n| matches!(self.slots[n].kind, SlotKind::Inv(j) if j >= HOTBAR)).collect();
                    (main.first().copied().unwrap_or(0), main.last().map_or(0, |x| x + 1))
                } else {
                    let hot: Vec<usize> = (0..self.slots.len()).filter(|&n| matches!(self.slots[n].kind, SlotKind::Inv(j) if j < HOTBAR)).collect();
                    (hot.first().copied().unwrap_or(0), hot.last().map_or(0, |x| x + 1))
                };
                if self.kind == MenuKind::Creative {
                    inv.items[i] = None;
                    return;
                }
                self.move_to(inv, &mut st, lo, hi, false);
            }
            _ => {
                self.move_to(inv, &mut st, inv_lo, inv_hi, false);
            }
        }
        self.set(inv, kind, if st.count == 0 { None } else { Some(st) });
    }

    /// `removed()`: whatever sits in the crafting grid goes back to the inventory (or into the world).
    pub fn close(&mut self, inv: &mut Inventory) -> Vec<ItemStack> {
        let mut dropped = Vec::new();
        for s in self.craft.iter_mut() {
            if let Some(mut st) = s.take() {
                if !inv.add(&mut st) {
                    dropped.push(st);
                }
            }
        }
        if let Some(mut c) = self.carried.take() {
            if self.kind != MenuKind::Creative && !inv.add(&mut c) {
                dropped.push(c);
            }
        }
        self.result = None;
        dropped
    }
}

/// Used by tests and the creative picker to know that every craftable item exists.
pub fn craftable_count() -> usize {
    all_results().count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_world::tile_ids as t;

    fn slot_of(m: &Menu, k: SlotKind) -> usize {
        m.slots.iter().position(|s| s.kind == k).unwrap()
    }

    #[test]
    fn add_merges_then_fills_the_first_free_slot() {
        let mut inv = Inventory::new();
        let mut a = ItemStack::block(t::STONE, 60);
        assert!(inv.add(&mut a));
        let mut b = ItemStack::block(t::STONE, 10);
        assert!(inv.add(&mut b));
        assert_eq!(inv.items[0].unwrap().count, 64);
        assert_eq!(inv.items[1].unwrap().count, 6);
        let mut sword = ItemStack::new(ids::SWORD_IRON, 1);
        assert!(inv.add(&mut sword));
        assert_eq!(inv.items[2].unwrap().id, ids::SWORD_IRON);
    }

    #[test]
    fn full_inventory_keeps_the_remainder() {
        let mut inv = Inventory::new();
        for i in 0..INV_SIZE {
            inv.items[i] = Some(ItemStack::block(t::DIRT, 64));
        }
        let mut s = ItemStack::block(t::STONE, 5);
        assert!(!inv.add(&mut s));
        assert_eq!(s.count, 5);
    }

    #[test]
    fn pick_up_split_place_and_swap() {
        let mut inv = Inventory::new();
        inv.items[0] = Some(ItemStack::block(t::STONE, 10));
        inv.items[1] = Some(ItemStack::block(t::DIRT, 3));
        let mut m = Menu::inventory();
        let s0 = slot_of(&m, SlotKind::Inv(0));
        let s1 = slot_of(&m, SlotKind::Inv(1));
        let s2 = slot_of(&m, SlotKind::Inv(2));
        m.click(&mut inv, Some(s0), true, false); // right = half (rounded up)
        assert_eq!(m.carried.unwrap().count, 5);
        assert_eq!(inv.items[0].unwrap().count, 5);
        m.click(&mut inv, Some(s2), false, false); // place all in an empty slot
        assert!(m.carried.is_none());
        assert_eq!(inv.items[2].unwrap().count, 5);
        m.click(&mut inv, Some(s1), false, false); // pick up dirt
        m.click(&mut inv, Some(s0), false, false); // swap with stone
        assert_eq!(inv.items[0].unwrap().id, t::DIRT as u16);
        assert_eq!(m.carried.unwrap().id, t::STONE as u16);
    }

    #[test]
    fn crafting_planks_into_sticks_through_the_menu() {
        let mut inv = Inventory::new();
        inv.items[0] = Some(ItemStack::block(t::TREE_TRUNK, 1));
        let mut m = Menu::inventory();
        let (s0, c0) = (slot_of(&m, SlotKind::Inv(0)), slot_of(&m, SlotKind::Craft(0)));
        m.click(&mut inv, Some(s0), false, false);
        m.click(&mut inv, Some(c0), false, false);
        assert_eq!(m.result.unwrap().id, t::WOOD as u16);
        assert_eq!(m.result.unwrap().count, 4);
        let r = slot_of(&m, SlotKind::Result);
        m.click(&mut inv, Some(r), false, false);
        assert_eq!(m.carried.unwrap().count, 4);
        assert!(m.craft[0].is_none());
        assert!(m.result.is_none());
    }

    #[test]
    fn shift_click_moves_between_hotbar_and_main_and_equips_armour() {
        let mut inv = Inventory::new();
        inv.items[0] = Some(ItemStack::block(t::STONE, 3));
        inv.items[1] = Some(ItemStack::new(ids::HELMET_IRON, 1));
        let mut m = Menu::inventory();
        m.click(&mut inv, Some(slot_of(&m, SlotKind::Inv(0))), false, true);
        assert!(inv.items[0].is_none());
        assert_eq!(inv.items[9].unwrap().count, 3);
        m.click(&mut inv, Some(slot_of(&m, SlotKind::Inv(1))), false, true);
        assert!(inv.items[1].is_none());
        assert_eq!(inv.armor[0].unwrap().id, ids::HELMET_IRON);
        assert_eq!(inv.armor_value(), 2);
    }

    #[test]
    fn armour_slot_only_accepts_the_matching_piece() {
        let mut inv = Inventory::new();
        inv.items[0] = Some(ItemStack::new(ids::BOOTS_IRON, 1));
        let mut m = Menu::inventory();
        m.click(&mut inv, Some(slot_of(&m, SlotKind::Inv(0))), false, false);
        m.click(&mut inv, Some(slot_of(&m, SlotKind::Armor(0))), false, false); // helmet slot: refused
        assert!(inv.armor[0].is_none());
        m.click(&mut inv, Some(slot_of(&m, SlotKind::Armor(3))), false, false);
        assert!(inv.armor[3].is_some());
    }

    #[test]
    fn closing_returns_the_crafting_grid_and_the_carried_stack() {
        let mut inv = Inventory::new();
        let mut m = Menu::crafting();
        m.craft[4] = Some(ItemStack::block(t::STONE, 2));
        m.carried = Some(ItemStack::block(t::DIRT, 4));
        let dropped = m.close(&mut inv);
        assert!(dropped.is_empty());
        assert_eq!(inv.count_of(t::STONE as u16), 2);
        assert_eq!(inv.count_of(t::DIRT as u16), 4);
    }

    #[test]
    fn creative_pool_gives_full_stacks_and_scrolls() {
        let mut inv = Inventory::new();
        let mut m = Menu::creative();
        assert!(m.pool.len() > 100);
        m.click(&mut inv, Some(0), false, false);
        assert_eq!(m.carried.unwrap().count, m.pool[0].max_stack());
        let hot = m.slots.iter().position(|s| s.kind == SlotKind::Inv(0)).unwrap();
        m.click(&mut inv, Some(hot), false, false);
        assert!(inv.items[0].is_some());
        m.scroll_by(2);
        assert_eq!(m.scroll, 2);
        assert!(matches!(m.slots[0].kind, SlotKind::Pool(i) if i == 2 * POOL_COLS));
    }
}

#[cfg(test)]
mod pool_tests {
    use super::*;
    #[test]
    fn creative_pool_has_no_technical_blocks_and_has_the_common_ones() {
        let p = creative_pool();
        let has = |id: u16| p.iter().any(|s| s.id == id);
        assert!(has(1) && has(2) && has(5) && has(20) && has(ids::PICK_AXE_DIAMOND));
        assert!(!has(8) && !has(9) && !has(10) && !has(11) && !has(51) && !has(59));
        eprintln!("creative pool = {} entries", p.len());
    }

    #[test]
    fn chest_menu_moves_items_both_ways_and_keeps_untouched_slots() {
        use mc_level::tile_entity::ItemStack as TeStack;
        let mut contents = vec![None; 27];
        contents[0] = Some(TeStack { id: 403, count: 1, damage: 0, enchantments: vec![(16, 2)] });
        let mut m = Menu::chest(&contents);
        let mut inv = Inventory::default();
        inv.items[0] = Some(ItemStack::new(1, 10));
        // shift-click an inventory stack: it lands in the chest
        let s = m.slots.iter().position(|s| s.kind == SlotKind::Inv(0)).unwrap();
        m.click(&mut inv, Some(s), false, true);
        assert!(inv.items[0].is_none());
        assert_eq!(m.chest[1], Some(ItemStack::new(1, 10)));
        // the untouched slot keeps its enchantment data when stored back
        let back = m.chest_contents();
        assert_eq!(back[0], contents[0]);
        assert_eq!(back[1].as_ref().map(|t| (t.id, t.count)), Some((1, 10)));
        // shift-click a chest stack: it returns to the player
        let c = m.slots.iter().position(|s| s.kind == SlotKind::Chest(1)).unwrap();
        m.click(&mut inv, Some(c), false, true);
        assert!(m.chest[1].is_none() && inv.items.iter().flatten().any(|s| s.id == 1 && s.count == 10));
    }
}

