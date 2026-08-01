//! Port of `model/itemcontainer/Inventory` + `PlayerInventory`, scoped to G5:
//! DB-backed item instances, the paperdoll, and the equip/unequip slot
//! resolution ordinary gear exercises. Warehouse/trade/pickup/enchant/
//! augmentation arrive with later milestones.

use crate::character::ItemRow;
use crate::data::item_data::{self, ItemData, ItemKind};

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

/// A stored item, as it lives in `Inventory.items` (Java: `Item`). Elemental
/// attributes and enchant-effect ids are later milestones — every item reports
/// "none" for those until then.
#[derive(Debug, Clone, Copy)]
pub struct ItemInstance {
    pub object_id: i32,
    pub item_id: i32,
    pub count: i64,
    pub enchant_level: i32,
    pub custom_type1: i32,
    pub custom_type2: i32,
    pub mana_left: i32,
    pub time: i32,
    /// Augmentation (Java `VariationInstance` on the item): the life stone
    /// ("mineral") id and the two rolled option ids. All `0` when unaugmented.
    /// Stat effects of the options aren't applied yet (a later milestone); the
    /// ids drive `isAugmented`, the display, and cancellation.
    pub augment_mineral: i32,
    pub augment_option1: i32,
    pub augment_option2: i32,
}

impl ItemInstance {
    fn new(object_id: i32, item_id: i32, count: i64) -> Self {
        Self {
            object_id,
            item_id,
            count,
            enchant_level: 0,
            custom_type1: 0,
            custom_type2: 0,
            mana_left: -1,
            time: 0,
            augment_mineral: 0,
            augment_option1: 0,
            augment_option2: 0,
        }
    }

    /// Java `Item.isAugmented` — has a variation attached.
    pub fn is_augmented(&self) -> bool {
        self.augment_option1 != 0 || self.augment_option2 != 0
    }
}

/// What `remove_item` did to one instance — the Java `InventoryUpdate`
/// change types this slice can produce (2 = modified, 3 = removed; adds go
/// through the modified-only `inventory_update` like before). `Removed`
/// carries the final snapshot because the instance is gone from the list.
#[derive(Debug, Clone, Copy)]
pub enum ItemChange {
    Modified(ItemInstance),
    Removed(ItemInstance),
}

/// Port of `PlayerRefund`: the merchant buy-back window. Items sold to a
/// merchant land here (Java `Config.ALLOW_REFUND`, on for this dist) and can
/// be bought back at the same half-reference-price until the container
/// overflows or the player logs out. Java never persists it (`restore()` is
/// empty, `deleteMe` destroys the contents), so this component simply dies
/// with the player entity.
#[derive(Debug, Clone, Default, bevy_ecs::component::Component)]
pub struct Refund {
    items: Vec<ItemInstance>,
}

impl Refund {
    /// Java `PlayerRefund` caps the container at 12 items, destroying the
    /// oldest on overflow.
    const CAPACITY: usize = 12;

    pub fn items(&self) -> &[ItemInstance] {
        &self.items
    }

    /// `PlayerRefund.addItem`: append, then drop the oldest past capacity.
    pub fn push(&mut self, item: ItemInstance) {
        self.items.push(item);
        if self.items.len() > Self::CAPACITY {
            self.items.remove(0);
        }
    }

    /// Remove and return the entry at `index` (a `RequestRefundItem` slot).
    pub fn take(&mut self, index: usize) -> Option<ItemInstance> {
        (index < self.items.len()).then(|| self.items.remove(index))
    }
}

/// Port of `PlayerInventory`: the flat item list plus the paperdoll (indices
/// into that list by `object_id`, mirroring Java's paperdoll array referencing
/// the same `Item` objects).
#[derive(Debug, Clone, Default, bevy_ecs::component::Component)]
pub struct Inventory {
    items: Vec<ItemInstance>,
    paperdoll: [Option<i32>; PAPERDOLL_TOTAL_SLOTS],
}

impl Inventory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build from stored `items` rows (Java: `PlayerInventory.restore`). Rows
    /// with `loc == "PAPERDOLL"` populate the paperdoll at `loc_data`; every
    /// other row (currently just `"INVENTORY"`) lands in the flat item list.
    pub fn from_rows(rows: &[ItemRow]) -> Self {
        let mut inv = Self::new();
        for r in rows {
            let inst = ItemInstance {
                object_id: r.object_id,
                item_id: r.item_id,
                count: r.count,
                enchant_level: r.enchant_level,
                custom_type1: r.custom_type1,
                custom_type2: r.custom_type2,
                mana_left: r.mana_left,
                time: r.time,
                augment_mineral: r.augment_mineral,
                augment_option1: r.augment_option1,
                augment_option2: r.augment_option2,
            };
            inv.items.push(inst);
            if r.loc == "PAPERDOLL" && (r.loc_data as usize) < PAPERDOLL_TOTAL_SLOTS {
                inv.paperdoll[r.loc_data as usize] = Some(r.object_id);
            }
        }
        inv
    }

    pub fn items(&self) -> &[ItemInstance] {
        &self.items
    }

    /// Serialize the whole inventory to `items` rows for a persistence flush
    /// (`PlayerSaveData`) — the inverse of [`from_rows`](Self::from_rows). An
    /// equipped instance gets `loc="PAPERDOLL"` with `loc_data` = its paperdoll
    /// slot (the lowest slot it occupies, matching how Java stores a
    /// slot-spanning item under a single row). Plain inventory items get
    /// `loc="INVENTORY"` with `loc_data` = their running position, so the
    /// client's saved arrangement (`RequestSaveInventoryOrder` →
    /// [`apply_inventory_order`](Self::apply_inventory_order)) survives relog —
    /// `load_items` restores with `ORDER BY loc_data`.
    pub fn to_rows(&self) -> Vec<crate::character::ItemRow> {
        let mut inv_order = 0i32;
        self.items
            .iter()
            .map(|i| {
                let (loc, loc_data) =
                    match self.paperdoll.iter().position(|p| *p == Some(i.object_id)) {
                        Some(slot) => ("PAPERDOLL".to_string(), slot as i32),
                        None => {
                            let order = inv_order;
                            inv_order += 1;
                            ("INVENTORY".to_string(), order)
                        }
                    };
                crate::character::ItemRow {
                    object_id: i.object_id,
                    item_id: i.item_id,
                    count: i.count,
                    enchant_level: i.enchant_level,
                    loc,
                    loc_data,
                    custom_type1: i.custom_type1,
                    custom_type2: i.custom_type2,
                    mana_left: i.mana_left,
                    time: i.time,
                    augment_mineral: i.augment_mineral,
                    augment_option1: i.augment_option1,
                    augment_option2: i.augment_option2,
                }
            })
            .collect()
    }

    /// The item instances currently occupying a paperdoll slot (Java: the
    /// `isEquipped()` subset of `getItems()`). Each instance is returned once
    /// even if it spans two slots (e.g. full armor covering chest + legs).
    /// Used by `refresh_expertise_penalty` to scan equipped gear grades.
    pub fn equipped_items(&self) -> Vec<&ItemInstance> {
        let mut seen: Vec<i32> = Vec::new();
        let mut out = Vec::new();
        for oid in self.paperdoll.iter().flatten() {
            if seen.contains(oid) {
                continue;
            }
            seen.push(*oid);
            if let Some(item) = self.find(*oid) {
                out.push(item);
            }
        }
        out
    }

    fn find(&self, object_id: i32) -> Option<&ItemInstance> {
        self.items.iter().find(|i| i.object_id == object_id)
    }

    /// The `(item_id, count)` of the instance with this object id, if held
    /// (Java `getInventory().getItemByObjectId(objId)`).
    pub fn item_by_object_id(&self, object_id: i32) -> Option<(i32, i64)> {
        self.find(object_id).map(|i| (i.item_id, i.count))
    }

    /// `Item.setEnchantLevel` on the first item of `item_id`; `false` if absent.
    /// The enchant-scroll path is a later milestone, but a few systems already
    /// read an item's enchant level as data — quest 421 reads a Dragonflute's
    /// as its hatchling's level.
    pub fn set_item_enchant_level(&mut self, item_id: i32, level: i32) -> bool {
        match self.items.iter_mut().find(|i| i.item_id == item_id) {
            Some(item) => {
                item.enchant_level = level;
                true
            }
            None => false,
        }
    }

    /// Apply a client `RequestSaveInventoryOrder` arrangement: reorder the
    /// in-memory item list by the given `(object_id, order)` pairs so the new
    /// order persists to `items.loc_data` on the next flush (memory-first — no
    /// per-packet DB write). Items not named in `order` (e.g. equipped ones)
    /// keep their relative position after the arranged ones (stable sort).
    pub fn apply_inventory_order(&mut self, order: &[(i32, i32)]) {
        let want: std::collections::HashMap<i32, i32> = order.iter().copied().collect();
        self.items
            .sort_by_key(|i| want.get(&i.object_id).copied().unwrap_or(i32::MAX));
    }

    /// `PlayerInventory.addItem`: stacks onto an existing instance of the same
    /// item id if the template says stackable (stackable items are always
    /// `EtcItem`s, which `equip_item` refuses to equip, so there's no risk of
    /// merging into an equipped instance), else adds a new instance. Returns
    /// the resulting `object_id`.
    pub fn add_item(
        &mut self,
        catalog: &ItemData,
        object_id: i32,
        item_id: i32,
        count: i64,
    ) -> i32 {
        let stackable = catalog
            .get(item_id)
            .map(|t| t.is_stackable)
            .unwrap_or(false);
        if stackable && let Some(existing) = self.items.iter_mut().find(|i| i.item_id == item_id) {
            existing.count += count;
            return existing.object_id;
        }
        self.items
            .push(ItemInstance::new(object_id, item_id, count));
        object_id
    }

    /// Stamp a just-created item's Lucky-Lottery fields (Java `Item
    /// .setCustomType1`/`setEnchantLevel`/`setCustomType2` on a fresh 4442
    /// ticket): the round id and the two-word picked-number bitmask. No-op if the
    /// object id is gone.
    pub fn set_lotto_fields(
        &mut self,
        object_id: i32,
        custom_type1: i32,
        enchant: i32,
        custom_type2: i32,
    ) {
        if let Some(it) = self.items.iter_mut().find(|i| i.object_id == object_id) {
            it.custom_type1 = custom_type1;
            it.enchant_level = enchant;
            it.custom_type2 = custom_type2;
        }
    }

    /// Insert an item preserving its enchant level (warehouse deposit/withdraw
    /// transfers), stacking into an existing same-id stack when stackable. Unlike
    /// [`add_item`](Self::add_item) — which always starts enchant 0 — this keeps
    /// the moved instance's enchant.
    pub fn insert_instance(
        &mut self,
        catalog: &ItemData,
        object_id: i32,
        item_id: i32,
        count: i64,
        enchant: i32,
    ) {
        let stackable = catalog
            .get(item_id)
            .map(|t| t.is_stackable)
            .unwrap_or(false);
        if stackable && let Some(existing) = self.items.iter_mut().find(|i| i.item_id == item_id) {
            existing.count += count;
            return;
        }
        let mut inst = ItemInstance::new(object_id, item_id, count);
        inst.enchant_level = enchant;
        self.items.push(inst);
    }

    /// Put a complete instance back (refund buy-back): merge into an existing
    /// stack when stackable, otherwise re-add the instance as-is — keeping its
    /// object id, enchant, augment, and remaining time. Returns the resulting
    /// instance snapshot for the `InventoryUpdate`.
    pub fn restore_instance(&mut self, catalog: &ItemData, inst: ItemInstance) -> ItemInstance {
        let stackable = catalog
            .get(inst.item_id)
            .map(|t| t.is_stackable)
            .unwrap_or(false);
        if stackable
            && let Some(existing) = self.items.iter_mut().find(|i| i.item_id == inst.item_id)
        {
            existing.count += inst.count;
            return *existing;
        }
        self.items.push(inst);
        inst
    }

    /// Total count of an item id across all instances
    /// (`Inventory.getInventoryItemCount` narrowed to no-enchant matching —
    /// what `AbstractScript.getQuestItemsCount` calls).
    pub fn count_of(&self, item_id: i32) -> i64 {
        self.items
            .iter()
            .filter(|i| i.item_id == item_id)
            .map(|i| i.count)
            .sum()
    }

    /// Destroy up to `count` of an item id (negative = all, Java
    /// `takeItems`' clamp) — the first item-removal path in the codebase
    /// (quest `takeItems`; drop/trade/crystallize are later milestones).
    /// Never touches the paperdoll: quest items can't be equipped, and the
    /// game-loop wrapper (`quests::take_items`) is the only caller. Returns
    /// what happened per touched instance so the caller can mirror it to the
    /// client (`InventoryUpdate`) and the DB.
    pub fn remove_item(&mut self, item_id: i32, count: i64) -> Vec<ItemChange> {
        let mut remaining = if count < 0 { i64::MAX } else { count };
        let mut changes = Vec::new();
        while remaining > 0 {
            let Some(idx) = self.items.iter().position(|i| i.item_id == item_id) else {
                break;
            };
            if self.items[idx].count > remaining {
                self.items[idx].count -= remaining;
                changes.push(ItemChange::Modified(self.items[idx]));
                break;
            }
            let removed = self.items.remove(idx);
            remaining -= removed.count;
            self.paperdoll.iter_mut().for_each(|s| {
                if *s == Some(removed.object_id) {
                    *s = None; // defensive; see doc comment
                }
            });
            changes.push(ItemChange::Removed(removed));
        }
        changes
    }

    /// Destroy `count` of one specific instance by `object_id` (Java
    /// `destroyItem(process, objectId, count, ...)`) — unlike [`Self::remove_item`],
    /// which targets any stack of a given item id, this targets exactly the
    /// clicked instance. Used by `UseItem`'s `EtcItem` branch (e.g.
    /// `ExtractableItems`), where the client names the object id, not the
    /// item id.
    pub fn remove_by_object_id(&mut self, object_id: i32, count: i64) -> Option<ItemChange> {
        let idx = self.items.iter().position(|i| i.object_id == object_id)?;
        if self.items[idx].count > count {
            self.items[idx].count -= count;
            return Some(ItemChange::Modified(self.items[idx]));
        }
        let removed = self.items.remove(idx);
        self.paperdoll.iter_mut().for_each(|s| {
            if *s == Some(removed.object_id) {
                *s = None; // defensive; see remove_item's doc comment
            }
        });
        Some(ItemChange::Removed(removed))
    }

    /// The `PaperdollSlot` an item's `body_part` bitmask resolves to when
    /// nothing else is equipped (`Inventory.getPaperdollIndex`, single-slot
    /// cases only — the dual-slot/two-handed cases are resolved in
    /// `equip_item`).
    fn primary_slot(body_part: i32) -> Option<PaperdollSlot> {
        use item_data::*;
        Some(match body_part {
            SLOT_L_BRACELET => PaperdollSlot::LBracelet,
            SLOT_R_BRACELET => PaperdollSlot::RBracelet,
            SLOT_DECO => PaperdollSlot::Deco1,
            SLOT_CHEST | SLOT_FULL_ARMOR | SLOT_ALLDRESS => PaperdollSlot::Chest,
            SLOT_LEGS => PaperdollSlot::Legs,
            SLOT_FEET => PaperdollSlot::Feet,
            SLOT_GLOVES => PaperdollSlot::Gloves,
            SLOT_HEAD => PaperdollSlot::Head,
            SLOT_R_HAND | SLOT_LR_HAND => PaperdollSlot::RHand,
            SLOT_L_HAND => PaperdollSlot::LHand,
            SLOT_UNDERWEAR => PaperdollSlot::Under,
            SLOT_BACK => PaperdollSlot::Cloak,
            SLOT_NECK => PaperdollSlot::Neck,
            SLOT_HAIR => PaperdollSlot::Hair,
            SLOT_HAIR2 => PaperdollSlot::Hair2,
            SLOT_HAIRALL => PaperdollSlot::Hair,
            SLOT_BELT => PaperdollSlot::Belt,
            SLOT_BROOCH => PaperdollSlot::Brooch,
            _ => return None,
        })
    }

    /// Port of `PlayerInventory.equipItem`'s slot-conflict resolution, scoped
    /// to what ordinary gear (and `initialEquipment.xml`) exercises. Returns
    /// the `object_id`s whose paperdoll/unequipped state changed (for
    /// `InventoryUpdate`). No-op if `object_id` isn't a known, equipable item.
    /// Put ammunition in the left hand, bypassing the ordinary equip rules —
    /// Java's `checkAndEquipAmmunition` calls `setPaperdollItem(PAPERDOLL_LHAND,
    /// arrows)` directly rather than going through `equipItem`.
    ///
    /// Both of `equip_item`'s rules are wrong for ammunition: arrows are `Etc`
    /// items (which it refuses outright), and its `SLOT_L_HAND` branch
    /// *displaces a two-handed weapon* — which would unequip the very bow the
    /// arrows are for.
    pub fn equip_ammunition(&mut self, object_id: i32) -> Vec<i32> {
        if self.find(object_id).is_none() {
            return Vec::new();
        }
        let mut changed: Vec<i32> = self.clear(PaperdollSlot::LHand).into_iter().collect();
        changed.push(self.set(PaperdollSlot::LHand, object_id));
        changed
    }

    pub fn equip_item(&mut self, catalog: &ItemData, object_id: i32) -> Vec<i32> {
        let Some(item) = self.find(object_id) else {
            return Vec::new();
        };
        let item_id = item.item_id;
        let Some(template) = catalog.get(item_id) else {
            return Vec::new();
        };
        if template.kind == ItemKind::Etc || !template.is_equipable() {
            return Vec::new();
        }
        let body_part = template.body_part;
        let mut changed = Vec::new();

        match body_part {
            item_data::SLOT_LR_EAR => {
                changed.extend(
                    self.set_first_free(&[PaperdollSlot::LEar, PaperdollSlot::REar], object_id),
                );
            }
            item_data::SLOT_LR_FINGER => {
                changed.extend(
                    self.set_first_free(
                        &[PaperdollSlot::LFinger, PaperdollSlot::RFinger],
                        object_id,
                    ),
                );
            }
            item_data::SLOT_LR_HAND => {
                changed.extend(self.clear(PaperdollSlot::LHand));
                changed.extend(self.clear(PaperdollSlot::RHand));
                changed.push(self.set(PaperdollSlot::RHand, object_id));
            }
            item_data::SLOT_L_HAND => {
                // A two-handed weapon in RHand is displaced by an off-hand item.
                if let Some(rh) = self.paperdoll_item(PaperdollSlot::RHand)
                    && catalog.get(rh.item_id).map(|t| t.body_part) == Some(item_data::SLOT_LR_HAND)
                {
                    changed.extend(self.clear(PaperdollSlot::RHand));
                }
                changed.extend(self.clear(PaperdollSlot::LHand));
                changed.push(self.set(PaperdollSlot::LHand, object_id));
            }
            item_data::SLOT_R_HAND => {
                changed.extend(self.clear(PaperdollSlot::RHand));
                changed.push(self.set(PaperdollSlot::RHand, object_id));
            }
            item_data::SLOT_CHEST => {
                changed.extend(self.clear(PaperdollSlot::Chest));
                changed.push(self.set(PaperdollSlot::Chest, object_id));
            }
            item_data::SLOT_FULL_ARMOR => {
                changed.extend(self.clear(PaperdollSlot::Legs));
                changed.extend(self.clear(PaperdollSlot::Chest));
                changed.push(self.set(PaperdollSlot::Chest, object_id));
            }
            item_data::SLOT_ALLDRESS => {
                // Formal dress covers the whole body.
                for slot in [
                    PaperdollSlot::Legs,
                    PaperdollSlot::LHand,
                    PaperdollSlot::RHand,
                    PaperdollSlot::Head,
                    PaperdollSlot::Feet,
                    PaperdollSlot::Gloves,
                    PaperdollSlot::Chest,
                ] {
                    changed.extend(self.clear(slot));
                }
                changed.push(self.set(PaperdollSlot::Chest, object_id));
            }
            item_data::SLOT_LEGS => {
                // A full-armor piece in Chest covers Legs too; equipping Legs
                // separately displaces it.
                if let Some(ch) = self.paperdoll_item(PaperdollSlot::Chest)
                    && matches!(
                        catalog.get(ch.item_id).map(|t| t.body_part),
                        Some(item_data::SLOT_FULL_ARMOR) | Some(item_data::SLOT_ALLDRESS)
                    )
                {
                    changed.extend(self.clear(PaperdollSlot::Chest));
                }
                changed.extend(self.clear(PaperdollSlot::Legs));
                changed.push(self.set(PaperdollSlot::Legs, object_id));
            }
            item_data::SLOT_HAIR => {
                changed.extend(self.clear(PaperdollSlot::Hair2));
                changed.extend(self.clear(PaperdollSlot::Hair));
                changed.push(self.set(PaperdollSlot::Hair, object_id));
            }
            item_data::SLOT_HAIR2 => {
                changed.extend(self.clear(PaperdollSlot::Hair));
                changed.extend(self.clear(PaperdollSlot::Hair2));
                changed.push(self.set(PaperdollSlot::Hair2, object_id));
            }
            item_data::SLOT_HAIRALL => {
                changed.extend(self.clear(PaperdollSlot::Hair));
                changed.extend(self.clear(PaperdollSlot::Hair2));
                changed.push(self.set(PaperdollSlot::Hair, object_id));
            }
            other => {
                if let Some(slot) = Self::primary_slot(other) {
                    changed.extend(self.clear(slot));
                    changed.push(self.set(slot, object_id));
                }
            }
        }

        changed.sort_unstable();
        changed.dedup();
        changed
    }

    /// Port of `PlayerInventory.unEquipItemInBodySlotAndRecord`. Accepts either
    /// a `PaperdollSlot` index or a `SLOT_*` body-part bitmask (Java overloads
    /// both); `SLOT_LR_HAND`/`SLOT_R_HAND` both clear `RHand`. Returns the
    /// changed `object_id`s.
    pub fn unequip_body_part(&mut self, body_part: i32) -> Vec<i32> {
        let slot = match body_part {
            item_data::SLOT_LR_HAND | item_data::SLOT_R_HAND => PaperdollSlot::RHand,
            item_data::SLOT_L_HAND => PaperdollSlot::LHand,
            item_data::SLOT_L_EAR => PaperdollSlot::LEar,
            item_data::SLOT_R_EAR => PaperdollSlot::REar,
            item_data::SLOT_L_FINGER => PaperdollSlot::LFinger,
            item_data::SLOT_R_FINGER => PaperdollSlot::RFinger,
            item_data::SLOT_HAIR => PaperdollSlot::Hair,
            item_data::SLOT_HAIR2 => PaperdollSlot::Hair2,
            other => match Self::primary_slot(other) {
                Some(s) => s,
                None => return Vec::new(),
            },
        };
        self.clear(slot).into_iter().collect()
    }

    /// The `UseItem` click-to-unequip path (Java: `Player.useEquippableItem`
    /// resolves `Inventory.getSlotFromItem(item)` — the single-bit slot the
    /// item is *actually* occupying, read off `item.getLocationSlot()` — and
    /// only then calls `unEquipItemInBodySlotAndRecord`). Passing the item's
    /// raw template body part straight to [`Self::unequip_body_part`] instead
    /// is wrong for rings/earrings: their template body part is the combined
    /// `SLOT_LR_EAR`/`SLOT_LR_FINGER` bitmask, which matches none of that
    /// function's single-bit arms and silently no-ops. Since we already know
    /// which exact `PaperdollSlot` the object id occupies, clear it directly
    /// instead of re-deriving an ambiguous body-part value from it.
    pub fn unequip_item(&mut self, object_id: i32) -> Vec<i32> {
        match self.paperdoll_slot_of(object_id) {
            Some(idx) => self.paperdoll[idx].take().into_iter().collect(),
            None => Vec::new(),
        }
    }

    /// Unequip by wire paperdoll index (`RequestUnEquipItem`'s `_slot` is
    /// actually a body-part bitmask in Java too — kept as a thin alias so call
    /// sites read naturally).
    pub fn unequip_slot(&mut self, paperdoll_body_part: i32) -> Vec<i32> {
        self.unequip_body_part(paperdoll_body_part)
    }

    fn set(&mut self, slot: PaperdollSlot, object_id: i32) -> i32 {
        self.paperdoll[slot as usize] = Some(object_id);
        object_id
    }

    fn clear(&mut self, slot: PaperdollSlot) -> Option<i32> {
        self.paperdoll[slot as usize].take()
    }

    fn set_first_free(&mut self, prefer: &[PaperdollSlot; 2], object_id: i32) -> Vec<i32> {
        let target = if self.paperdoll[prefer[0] as usize].is_none() {
            prefer[0]
        } else {
            prefer[1]
        };
        let mut changed: Vec<i32> = self.clear(target).into_iter().collect();
        changed.push(self.set(target, object_id));
        changed
    }

    pub fn paperdoll_item(&self, slot: PaperdollSlot) -> Option<&ItemInstance> {
        self.paperdoll[slot as usize].and_then(|oid| self.find(oid))
    }

    /// The paperdoll index (`items.loc_data`) an `object_id` is equipped in,
    /// if any. Used to persist character-creation results and runtime
    /// equip/unequip changes.
    pub fn paperdoll_slot_of(&self, object_id: i32) -> Option<usize> {
        self.paperdoll.iter().position(|s| *s == Some(object_id))
    }

    /// `getPaperdollObjectId` — 0 when the slot is empty.
    pub fn paperdoll_object_id(&self, slot: PaperdollSlot) -> i32 {
        self.paperdoll_item(slot).map_or(0, |i| i.object_id)
    }

    /// `getPaperdollItemId` — 0 when the slot is empty.
    pub fn paperdoll_item_id(&self, slot: PaperdollSlot) -> i32 {
        self.paperdoll_item(slot).map_or(0, |i| i.item_id)
    }

    /// `getPaperdollItemVisualId` — always 0 (appearance stones are a later
    /// milestone).
    pub fn paperdoll_visual_id(&self, _slot: PaperdollSlot) -> i32 {
        0
    }

    /// `getPaperdollAugmentation` — the option ids of the item equipped in
    /// `slot`, or `None` when the slot is empty / unaugmented.
    pub fn paperdoll_augmentation(&self, slot: PaperdollSlot) -> Option<(i32, i32)> {
        let item = self.paperdoll_item(slot)?;
        item.is_augmented()
            .then_some((item.augment_option1, item.augment_option2))
    }

    /// The augment option ids of an item by object id, if augmented.
    pub fn augmentation_of(&self, object_id: i32) -> Option<(i32, i32)> {
        let item = self.items.iter().find(|i| i.object_id == object_id)?;
        item.is_augmented()
            .then_some((item.augment_option1, item.augment_option2))
    }

    /// Whether the item `object_id` is augmented (Java `Item.isAugmented`).
    pub fn is_augmented(&self, object_id: i32) -> bool {
        self.items
            .iter()
            .any(|i| i.object_id == object_id && i.is_augmented())
    }

    /// Attach a variation to an item (Java `Item.setAugmentation`).
    pub fn set_augmentation(&mut self, object_id: i32, mineral: i32, option1: i32, option2: i32) {
        if let Some(item) = self.items.iter_mut().find(|i| i.object_id == object_id) {
            item.augment_mineral = mineral;
            item.augment_option1 = option1;
            item.augment_option2 = option2;
        }
    }

    /// The life stone id an item was augmented with (for the cancel fee).
    pub fn augment_mineral(&self, object_id: i32) -> Option<i32> {
        self.items
            .iter()
            .find(|i| i.object_id == object_id && i.is_augmented())
            .map(|i| i.augment_mineral)
    }

    /// Remove an item's variation (Java `Item.removeAugmentation`).
    pub fn remove_augmentation(&mut self, object_id: i32) {
        if let Some(item) = self.items.iter_mut().find(|i| i.object_id == object_id) {
            item.augment_mineral = 0;
            item.augment_option1 = 0;
            item.augment_option2 = 0;
        }
    }

    pub fn paperdoll_enchant_level(&self, slot: PaperdollSlot) -> i32 {
        self.paperdoll_item(slot).map_or(0, |i| i.enchant_level)
    }

    /// Set the enchant level of the item equipped in `slot`, returning its
    /// object id (or `None` when the slot is empty). Admin `//set*` enchant.
    pub fn set_paperdoll_enchant(&mut self, slot: PaperdollSlot, level: i32) -> Option<i32> {
        let oid = self.paperdoll[slot as usize]?;
        let item = self.items.iter_mut().find(|i| i.object_id == oid)?;
        item.enchant_level = level;
        Some(oid)
    }

    /// Stamp an enchant level onto a specific item instance by object id —
    /// Java `Item.setEnchantLevel` on a freshly created item (the
    /// `Restoration`/`RestorationRandom` enchant-roll grants).
    pub fn set_item_enchant(&mut self, object_id: i32, level: i32) {
        if let Some(item) = self.items.iter_mut().find(|i| i.object_id == object_id) {
            item.enchant_level = level;
        }
    }

    /// Sum of `count` for adena (`Inventory.getAdena`).
    pub fn adena(&self) -> i64 {
        self.items
            .iter()
            .filter(|i| i.item_id == item_data::ADENA_ID)
            .map(|i| i.count)
            .sum()
    }

    /// `PlayerInventory.getNonQuestSize` — item count excluding quest items,
    /// what the ordinary inventory-slot cap (`getInventoryLimit`) is checked
    /// against, so quest rewards never crowd out bag space.
    pub fn non_quest_size(&self, catalog: &ItemData) -> usize {
        self.items
            .iter()
            .filter(|i| catalog.get(i.item_id).is_none_or(|t| !t.is_quest_item))
            .count()
    }

    /// `PlayerInventory.getQuestSize` — quest items are checked against
    /// their own separate `getQuestInventoryLimit`, never the ordinary one.
    pub fn quest_size(&self, catalog: &ItemData) -> usize {
        self.items
            .iter()
            .filter(|i| catalog.get(i.item_id).is_some_and(|t| t.is_quest_item))
            .count()
    }

    /// `Player.isInventoryUnder80(false)`: ordinary (non-quest) item count is
    /// under 80% of `normal_limit` — the gate `ExtractableItems.useItem`
    /// checks before granting reward items.
    pub fn is_under_80_percent(&self, catalog: &ItemData, normal_limit: i32) -> bool {
        (self.non_quest_size(catalog) as f64) <= (normal_limit as f64 * 0.8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::item_data::ItemTemplate;

    fn armor(id: i32, body_part: i32) -> ItemTemplate {
        ItemTemplate {
            trade_flags: Default::default(),
            time: -1,
            item_id: id,
            name: format!("armor{id}"),
            kind: ItemKind::Armor,
            crystal_type: item_data::CrystalType::None,
            crystal_count: 0,
            attack_radius: 40,
            attack_angle: 0,
            mp_consume: 0,
            reduced_mp_consume: 0,
            reduced_mp_consume_chance: 0,
            body_part,
            weight: 0,
            is_stackable: false,
            type1: 0,
            type2: 0,
            is_quest_item: false,
            is_sellable: true,
            is_freightable: false,
            price: 0,
            handler: item_data::ItemHandler::None,
            capsuled_items: Vec::new(),
            extractable_count_min: 0,
            extractable_count_max: 0,
            item_skills: Vec::new(),
            etc_item_type: crate::data::item_data::EtcItemType::Other,
            enchant_enabled: false,
            enchant_limit: 0,
            is_magic_weapon: false,
            immediate_effect: false,
            ex_immediate_effect: false,
            default_action: crate::data::item_data::ActionType::Other,
        }
    }

    fn weapon(id: i32, body_part: i32) -> ItemTemplate {
        ItemTemplate {
            trade_flags: Default::default(),
            time: -1,
            item_id: id,
            name: format!("weapon{id}"),
            kind: ItemKind::Weapon,
            crystal_type: item_data::CrystalType::None,
            crystal_count: 0,
            attack_radius: 40,
            attack_angle: 0,
            mp_consume: 0,
            reduced_mp_consume: 0,
            reduced_mp_consume_chance: 0,
            body_part,
            weight: 0,
            is_stackable: false,
            type1: 0,
            type2: 0,
            is_quest_item: false,
            is_sellable: true,
            is_freightable: false,
            price: 0,
            handler: item_data::ItemHandler::None,
            capsuled_items: Vec::new(),
            extractable_count_min: 0,
            extractable_count_max: 0,
            item_skills: Vec::new(),
            etc_item_type: crate::data::item_data::EtcItemType::Other,
            enchant_enabled: false,
            enchant_limit: 0,
            is_magic_weapon: false,
            immediate_effect: false,
            ex_immediate_effect: false,
            default_action: crate::data::item_data::ActionType::Other,
        }
    }

    #[test]
    fn plain_chest_keeps_legs_equipped() {
        let catalog = ItemData::from_templates(vec![
            armor(1, item_data::SLOT_CHEST),
            armor(2, item_data::SLOT_LEGS),
        ]);
        let mut inv = Inventory::new();
        inv.add_item(&catalog, 1, 1, 1);
        inv.add_item(&catalog, 2, 2, 1);
        inv.equip_item(&catalog, 2);
        inv.equip_item(&catalog, 1);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Chest), 1);
        assert_eq!(
            inv.paperdoll_object_id(PaperdollSlot::Legs),
            2,
            "plain chest must not unequip Legs"
        );
    }

    #[test]
    fn full_armor_clears_chest_and_legs_and_vice_versa() {
        let catalog = ItemData::from_templates(vec![
            armor(1, item_data::SLOT_CHEST),
            armor(2, item_data::SLOT_LEGS),
            armor(3, item_data::SLOT_FULL_ARMOR),
        ]);
        let mut inv = Inventory::new();
        inv.add_item(&catalog, 1, 1, 1);
        inv.add_item(&catalog, 2, 2, 1);
        inv.equip_item(&catalog, 1);
        inv.equip_item(&catalog, 2);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Chest), 1);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Legs), 2);

        inv.add_item(&catalog, 3, 3, 1);
        inv.equip_item(&catalog, 3);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Chest), 3);
        assert_eq!(
            inv.paperdoll_object_id(PaperdollSlot::Legs),
            0,
            "full armor clears Legs"
        );

        // Equipping Legs again displaces the full-armor piece from Chest.
        inv.equip_item(&catalog, 2);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Legs), 2);
        assert_eq!(
            inv.paperdoll_object_id(PaperdollSlot::Chest),
            0,
            "equipping Legs displaces full armor"
        );
    }

    #[test]
    fn two_handed_weapon_clears_both_hands_and_is_displaced_by_offhand() {
        let catalog = ItemData::from_templates(vec![
            weapon(1, item_data::SLOT_R_HAND),
            armor(2, item_data::SLOT_L_HAND), // shield
            weapon(3, item_data::SLOT_LR_HAND),
        ]);
        let mut inv = Inventory::new();
        inv.add_item(&catalog, 1, 1, 1);
        inv.add_item(&catalog, 2, 2, 1);
        inv.equip_item(&catalog, 1);
        inv.equip_item(&catalog, 2);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::RHand), 1);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::LHand), 2);

        // Equipping a two-handed weapon clears both hands, occupies RHand only.
        inv.add_item(&catalog, 3, 3, 1);
        inv.equip_item(&catalog, 3);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::RHand), 3);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::LHand), 0);

        // Equipping the shield again displaces the two-handed weapon from RHand.
        inv.equip_item(&catalog, 2);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::LHand), 2);
        assert_eq!(
            inv.paperdoll_object_id(PaperdollSlot::RHand),
            0,
            "off-hand item displaces the two-handed weapon"
        );
    }

    #[test]
    fn dual_slot_items_fill_left_then_right() {
        let catalog = ItemData::from_templates(vec![
            armor(1, item_data::SLOT_R_EAR | item_data::SLOT_L_EAR),
            armor(2, item_data::SLOT_R_EAR | item_data::SLOT_L_EAR),
        ]);
        let mut inv = Inventory::new();
        inv.add_item(&catalog, 1, 1, 1);
        inv.add_item(&catalog, 2, 2, 1);
        inv.equip_item(&catalog, 1);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::LEar), 1);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::REar), 0);

        inv.equip_item(&catalog, 2);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::LEar), 1);
        assert_eq!(
            inv.paperdoll_object_id(PaperdollSlot::REar),
            2,
            "second earring fills the free REar slot"
        );
    }

    #[test]
    fn quest_items_are_excluded_from_the_ordinary_capacity_count() {
        let catalog = ItemData::from_templates(vec![
            ItemTemplate {
                trade_flags: Default::default(),
                time: -1,
                immediate_effect: false,
                ex_immediate_effect: false,
                default_action: crate::data::item_data::ActionType::Other,
                item_id: 1,
                name: "quest_item".into(),
                kind: ItemKind::Etc,
                body_part: item_data::SLOT_NONE,
                weight: 0,
                is_stackable: false,
                type1: 0,
                type2: 0,
                is_quest_item: true,
                is_sellable: true,
                is_freightable: false,
                price: 0,
                handler: item_data::ItemHandler::None,
                crystal_type: crate::data::item_data::CrystalType::None,
                crystal_count: 0,
                attack_radius: 40,
                attack_angle: 0,
                mp_consume: 0,
                reduced_mp_consume: 0,
                reduced_mp_consume_chance: 0,
                capsuled_items: Vec::new(),
                extractable_count_min: 0,
                extractable_count_max: 0,
                item_skills: Vec::new(),
                etc_item_type: crate::data::item_data::EtcItemType::Other,
                enchant_enabled: false,
                enchant_limit: 0,
                is_magic_weapon: false,
            },
            ItemTemplate {
                trade_flags: Default::default(),
                time: -1,
                immediate_effect: false,
                ex_immediate_effect: false,
                default_action: crate::data::item_data::ActionType::Other,
                item_id: 2,
                name: "ordinary_item".into(),
                kind: ItemKind::Etc,
                body_part: item_data::SLOT_NONE,
                weight: 0,
                is_stackable: false,
                type1: 0,
                type2: 0,
                is_quest_item: false,
                is_sellable: true,
                is_freightable: false,
                price: 0,
                handler: item_data::ItemHandler::None,
                crystal_type: crate::data::item_data::CrystalType::None,
                crystal_count: 0,
                attack_radius: 40,
                attack_angle: 0,
                mp_consume: 0,
                reduced_mp_consume: 0,
                reduced_mp_consume_chance: 0,
                capsuled_items: Vec::new(),
                extractable_count_min: 0,
                extractable_count_max: 0,
                item_skills: Vec::new(),
                etc_item_type: crate::data::item_data::EtcItemType::Other,
                enchant_enabled: false,
                enchant_limit: 0,
                is_magic_weapon: false,
            },
        ]);
        let mut inv = Inventory::new();
        inv.add_item(&catalog, 100, 1, 1);
        inv.add_item(&catalog, 101, 2, 1);

        assert_eq!(inv.quest_size(&catalog), 1);
        assert_eq!(inv.non_quest_size(&catalog), 1);
        // A full quest-item bag mustn't count against the ordinary cap.
        assert!(inv.is_under_80_percent(&catalog, 2));
    }

    #[test]
    fn etc_items_are_never_equipped() {
        let catalog = ItemData::from_templates(vec![ItemTemplate {
            trade_flags: Default::default(),
            time: -1,
            immediate_effect: false,
            ex_immediate_effect: false,
            default_action: crate::data::item_data::ActionType::Other,
            item_id: 57,
            name: "Adena".into(),
            kind: ItemKind::Etc,
            body_part: item_data::SLOT_NONE,
            weight: 0,
            is_stackable: true,
            type1: 0,
            type2: 0,
            is_quest_item: false,
            is_sellable: true,
            is_freightable: false,
            price: 0,
            handler: item_data::ItemHandler::None,
            crystal_type: crate::data::item_data::CrystalType::None,
            crystal_count: 0,
            attack_radius: 40,
            attack_angle: 0,
            mp_consume: 0,
            reduced_mp_consume: 0,
            reduced_mp_consume_chance: 0,
            capsuled_items: Vec::new(),
            extractable_count_min: 0,
            extractable_count_max: 0,
            item_skills: Vec::new(),
            etc_item_type: crate::data::item_data::EtcItemType::Other,
            enchant_enabled: false,
            enchant_limit: 0,
            is_magic_weapon: false,
        }]);
        let mut inv = Inventory::new();
        let oid = inv.add_item(&catalog, 1, 57, 100);
        let changed = inv.equip_item(&catalog, oid);
        assert!(changed.is_empty());
        assert_eq!(inv.paperdoll_slot_of(oid), None);
    }
}

/// Port of the player's personal `Warehouse` (an `ItemContainer` at
/// `ItemLocation.WAREHOUSE`). A flat item list — no paperdoll — so it reuses
/// [`Inventory`]'s stacking add / remove / lookup logic. Persisted alongside the
/// inventory (its rows carry `loc="WAREHOUSE"`), loaded by [`from_rows`].
#[derive(Debug, Clone, Default, bevy_ecs::component::Component)]
pub struct Warehouse(pub Inventory);

impl Warehouse {
    /// Build from the character's `WAREHOUSE`-location item rows (non-paperdoll,
    /// so they land in the flat list).
    pub fn from_rows(rows: &[crate::character::ItemRow]) -> Self {
        Self(Inventory::from_rows(rows))
    }

    /// Serialize to `items` rows with `loc="WAREHOUSE"` (nothing is equipped in a
    /// warehouse, so [`Inventory::to_rows`] yields `INVENTORY`; remap the loc).
    pub fn to_rows(&self) -> Vec<crate::character::ItemRow> {
        let mut rows = self.0.to_rows();
        for r in &mut rows {
            r.loc = "WAREHOUSE".to_string();
            r.loc_data = 0;
        }
        rows
    }

    /// Serialize to `items` rows with `loc="CLANWH"` — the clan warehouse's
    /// persistence location (`owner_id` = clan id, bound by the DB layer).
    pub fn to_rows_clan(&self) -> Vec<crate::character::ItemRow> {
        let mut rows = self.0.to_rows();
        for r in &mut rows {
            r.loc = "CLANWH".to_string();
            r.loc_data = 0;
        }
        rows
    }

    /// Current item count (Java `ItemContainer.getSize`).
    pub fn size(&self) -> usize {
        self.0.items().len()
    }
}

/// A pet's inventory (Java `PetInventory` / `ItemLocation.PET`).
///
/// Java keys these rows by the **player-owner's** object id, not the pet's
/// (`PetInventory.getOwnerId()` returns `_owner.getOwner().getObjectId()`), so
/// they ride along with the character's items exactly like the warehouse does
/// — the pet entity is transient, the rows are not.
///
/// A consequence worth knowing rather than "fixing": because the rows carry no
/// per-pet discriminator, a player with two collars sees the *same* pet
/// inventory on both pets. That is Java's behaviour on this dist.
#[derive(Debug, Clone, Default, bevy_ecs::component::Component)]
pub struct PetInventory(pub Inventory);

impl PetInventory {
    /// Build from the character's `PET`/`PET_EQUIP` rows. `PET_EQUIP` is
    /// renamed back to `PAPERDOLL` so the shared loader restores the pet's
    /// worn slots the same way it restores a player's.
    pub fn from_rows(rows: &[crate::character::ItemRow]) -> Self {
        let rows: Vec<_> = rows
            .iter()
            .cloned()
            .map(|mut r| {
                if r.loc == "PET_EQUIP" {
                    r.loc = "PAPERDOLL".to_string();
                }
                r
            })
            .collect();
        Self(Inventory::from_rows(&rows))
    }

    /// Serialize to `items` rows: `loc="PET"` for carried items and
    /// `loc="PET_EQUIP"` for worn ones, matching Java's
    /// `PetInventory.getBaseLocation()`/`getEquipLocation()`.
    ///
    /// `Inventory::to_rows` already marks equipped rows `PAPERDOLL` with the
    /// slot in `loc_data`; remapping the name preserves the slot, so a pet's
    /// armour comes back **on** rather than in its bag.
    pub fn to_rows(&self) -> Vec<crate::character::ItemRow> {
        let mut rows = self.0.to_rows();
        for r in &mut rows {
            if r.loc == "PAPERDOLL" {
                r.loc = "PET_EQUIP".to_string();
            } else {
                r.loc = "PET".to_string();
                r.loc_data = 0;
            }
        }
        rows
    }
}

/// The character's freight (Java `PlayerFreight` / `ItemLocation.FREIGHT`) —
/// the account-package warehouse other characters send items *to*. Like
/// [`Warehouse`] it's a flat, per-owner container persisted in the player's
/// `items` rows (`loc="FREIGHT"`, `owner_id` = char id).
#[derive(Debug, Clone, Default, bevy_ecs::component::Component)]
pub struct Freight(pub Inventory);

impl Freight {
    /// Build from the character's `FREIGHT`-location item rows.
    pub fn from_rows(rows: &[crate::character::ItemRow]) -> Self {
        Self(Inventory::from_rows(rows))
    }

    /// Serialize to `items` rows with `loc="FREIGHT"`.
    pub fn to_rows(&self) -> Vec<crate::character::ItemRow> {
        let mut rows = self.0.to_rows();
        for r in &mut rows {
            r.loc = "FREIGHT".to_string();
            r.loc_data = 0;
        }
        rows
    }

    /// Current item count (Java `ItemContainer.getSize`).
    pub fn size(&self) -> usize {
        self.0.items().len()
    }
}
