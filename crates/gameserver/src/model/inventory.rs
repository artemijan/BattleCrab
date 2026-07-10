//! Port of `model/itemcontainer/Inventory` — for now just the paperdoll slot
//! ids and an empty paperdoll, so packets read equipment through the real API
//! instead of hardcoding zero-runs. Items, equip/unequip rules, and DB-backed
//! contents arrive with G6.

/// Port of the `Inventory.PAPERDOLL_*` constants — the 32 equipment-slot
/// indices of the paperdoll array. The numeric values are storage/DB indices
/// (`items.loc_data`), **not** the client wire order; packets order slots via
/// `InventorySlot` / `PAPERDOLL_ORDER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum PaperdollSlot {
    Under = 0,
    Head = 1,
    Hair = 2,
    Hair2 = 3,
    Neck = 4,
    RHand = 5,
    Chest = 6,
    LHand = 7,
    REar = 8,
    LEar = 9,
    Gloves = 10,
    Legs = 11,
    Feet = 12,
    RFinger = 13,
    LFinger = 14,
    LBracelet = 15,
    RBracelet = 16,
    Deco1 = 17,
    Deco2 = 18,
    Deco3 = 19,
    Deco4 = 20,
    Deco5 = 21,
    Deco6 = 22,
    Cloak = 23,
    Belt = 24,
    Brooch = 25,
    BroochJewel1 = 26,
    BroochJewel2 = 27,
    BroochJewel3 = 28,
    BroochJewel4 = 29,
    BroochJewel5 = 30,
    BroochJewel6 = 31,
}

/// `Inventory.PAPERDOLL_TOTALSLOTS`.
pub const PAPERDOLL_TOTAL_SLOTS: usize = 32;

/// An equipped item as the paperdoll packets see it. Stand-in for `Item` +
/// `VariationInstance` until the item system lands (G6).
#[derive(Debug, Clone, Copy)]
pub struct PaperdollItem {
    pub object_id: i32,
    pub item_id: i32,
    pub visual_id: i32,
    /// Augmentation (option1 id, option2 id), Java `VariationInstance`.
    pub augmentation: Option<(i32, i32)>,
}

/// Port of `PlayerInventory`'s paperdoll accessors. Every slot is empty until
/// G6 loads real items; the getters mirror Java's zero-for-empty behavior.
#[derive(Debug, Clone, Default)]
pub struct Inventory {
    paperdoll: [Option<PaperdollItem>; PAPERDOLL_TOTAL_SLOTS],
}

impl Inventory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn paperdoll_item(&self, slot: PaperdollSlot) -> Option<&PaperdollItem> {
        self.paperdoll[slot as usize].as_ref()
    }

    /// `getPaperdollObjectId` — 0 when the slot is empty.
    pub fn paperdoll_object_id(&self, slot: PaperdollSlot) -> i32 {
        self.paperdoll_item(slot).map_or(0, |i| i.object_id)
    }

    /// `getPaperdollItemId` — 0 when the slot is empty.
    pub fn paperdoll_item_id(&self, slot: PaperdollSlot) -> i32 {
        self.paperdoll_item(slot).map_or(0, |i| i.item_id)
    }

    /// `getPaperdollItemVisualId` — 0 when the slot is empty.
    pub fn paperdoll_visual_id(&self, slot: PaperdollSlot) -> i32 {
        self.paperdoll_item(slot).map_or(0, |i| i.visual_id)
    }

    /// `getPaperdollAugmentation` — the (option1, option2) ids, if any.
    pub fn paperdoll_augmentation(&self, slot: PaperdollSlot) -> Option<(i32, i32)> {
        self.paperdoll_item(slot).and_then(|i| i.augmentation)
    }
}
