//! Minimal tile-entity model: just what world generation produces (chest contents, spawner entity).

/// `ItemInstance` as far as generation needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemStack {
    /// Full item id (256 + the constructor id for items).
    pub id: i32,
    pub count: i32,
    pub damage: i32,
    /// Stored enchantments `(enchantment id, level)` (enchanted books).
    pub enchantments: Vec<(u8, u8)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TileEntity {
    /// `ChestTileEntity` (27 slots).
    Chest { items: Vec<Option<ItemStack>> },
    /// `MobSpawnerTileEntity`: the spawner's entity id.
    Spawner { entity_id: String },
    /// `DispenserTileEntity` (9 slots).
    Dispenser { items: Vec<Option<ItemStack>> },
    /// `SignTileEntity`: four lines of text.
    Sign { lines: [String; 4] },
}

pub const CHEST_SIZE: usize = 27;
pub const DISPENSER_SIZE: usize = 9;

impl TileEntity {
    pub fn new_chest() -> TileEntity {
        TileEntity::Chest { items: vec![None; CHEST_SIZE] }
    }
}
