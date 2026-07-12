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

/// A stored item, as it lives in `Inventory.items` (Java: `Item`). Augmentation,
/// elemental attributes, and enchant-effect ids are later milestones — every
/// item reports "none" for those until then.
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
}

impl ItemInstance {
    fn new(object_id: i32, item_id: i32, count: i64) -> Self {
        Self { object_id, item_id, count, enchant_level: 0, custom_type1: 0, custom_type2: 0, mana_left: -1, time: 0 }
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

    fn find(&self, object_id: i32) -> Option<&ItemInstance> {
        self.items.iter().find(|i| i.object_id == object_id)
    }

    /// `PlayerInventory.addItem`: stacks onto an existing instance of the same
    /// item id if the template says stackable (stackable items are always
    /// `EtcItem`s, which `equip_item` refuses to equip, so there's no risk of
    /// merging into an equipped instance), else adds a new instance. Returns
    /// the resulting `object_id`.
    pub fn add_item(&mut self, catalog: &ItemData, object_id: i32, item_id: i32, count: i64) -> i32 {
        let stackable = catalog.get(item_id).map(|t| t.is_stackable).unwrap_or(false);
        if stackable {
            if let Some(existing) = self.items.iter_mut().find(|i| i.item_id == item_id) {
                existing.count += count;
                return existing.object_id;
            }
        }
        self.items.push(ItemInstance::new(object_id, item_id, count));
        object_id
    }

    /// Total count of an item id across all instances
    /// (`Inventory.getInventoryItemCount` narrowed to no-enchant matching —
    /// what `AbstractScript.getQuestItemsCount` calls).
    pub fn count_of(&self, item_id: i32) -> i64 {
        self.items.iter().filter(|i| i.item_id == item_id).map(|i| i.count).sum()
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
            let Some(idx) = self.items.iter().position(|i| i.item_id == item_id) else { break };
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
    pub fn equip_item(&mut self, catalog: &ItemData, object_id: i32) -> Vec<i32> {
        let Some(item) = self.find(object_id) else { return Vec::new() };
        let item_id = item.item_id;
        let Some(template) = catalog.get(item_id) else { return Vec::new() };
        if template.kind == ItemKind::Etc || !template.is_equipable() {
            return Vec::new();
        }
        let body_part = template.body_part;
        let mut changed = Vec::new();

        match body_part {
            item_data::SLOT_LR_EAR => {
                changed.extend(self.set_first_free(&[PaperdollSlot::LEar, PaperdollSlot::REar], object_id));
            }
            item_data::SLOT_LR_FINGER => {
                changed.extend(self.set_first_free(&[PaperdollSlot::LFinger, PaperdollSlot::RFinger], object_id));
            }
            item_data::SLOT_LR_HAND => {
                changed.extend(self.clear(PaperdollSlot::LHand));
                changed.extend(self.clear(PaperdollSlot::RHand));
                changed.push(self.set(PaperdollSlot::RHand, object_id));
            }
            item_data::SLOT_L_HAND => {
                // A two-handed weapon in RHand is displaced by an off-hand item.
                if let Some(rh) = self.paperdoll_item(PaperdollSlot::RHand) {
                    if catalog.get(rh.item_id).map(|t| t.body_part) == Some(item_data::SLOT_LR_HAND) {
                        changed.extend(self.clear(PaperdollSlot::RHand));
                    }
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
                if let Some(ch) = self.paperdoll_item(PaperdollSlot::Chest) {
                    if matches!(catalog.get(ch.item_id).map(|t| t.body_part), Some(item_data::SLOT_FULL_ARMOR) | Some(item_data::SLOT_ALLDRESS)) {
                        changed.extend(self.clear(PaperdollSlot::Chest));
                    }
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
        let target = if self.paperdoll[prefer[0] as usize].is_none() { prefer[0] } else { prefer[1] };
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

    /// `getPaperdollAugmentation` — always `None` (augmentation is a later
    /// milestone).
    pub fn paperdoll_augmentation(&self, _slot: PaperdollSlot) -> Option<(i32, i32)> {
        None
    }

    pub fn paperdoll_enchant_level(&self, slot: PaperdollSlot) -> i32 {
        self.paperdoll_item(slot).map_or(0, |i| i.enchant_level)
    }

    /// Sum of `count` for adena (`Inventory.getAdena`).
    pub fn adena(&self) -> i64 {
        self.items.iter().filter(|i| i.item_id == item_data::ADENA_ID).map(|i| i.count).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::item_data::ItemTemplate;

    fn armor(id: i32, body_part: i32) -> ItemTemplate {
        ItemTemplate { item_id: id, name: format!("armor{id}"), kind: ItemKind::Armor, body_part, weight: 0, is_stackable: false, type1: 0, type2: 0, is_quest_item: false }
    }

    fn weapon(id: i32, body_part: i32) -> ItemTemplate {
        ItemTemplate { item_id: id, name: format!("weapon{id}"), kind: ItemKind::Weapon, body_part, weight: 0, is_stackable: false, type1: 0, type2: 0, is_quest_item: false }
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
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Legs), 2, "plain chest must not unequip Legs");
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
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Legs), 0, "full armor clears Legs");

        // Equipping Legs again displaces the full-armor piece from Chest.
        inv.equip_item(&catalog, 2);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Legs), 2);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Chest), 0, "equipping Legs displaces full armor");
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
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::RHand), 0, "off-hand item displaces the two-handed weapon");
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
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::REar), 2, "second earring fills the free REar slot");
    }

    #[test]
    fn etc_items_are_never_equipped() {
        let catalog = ItemData::from_templates(vec![ItemTemplate {
            item_id: 57,
            name: "Adena".into(),
            kind: ItemKind::Etc,
            body_part: item_data::SLOT_NONE,
            weight: 0,
            is_stackable: true,
            type1: 0,
            type2: 0,
            is_quest_item: false,
        }]);
        let mut inv = Inventory::new();
        let oid = inv.add_item(&catalog, 1, 57, 100);
        let changed = inv.equip_item(&catalog, oid);
        assert!(changed.is_empty());
        assert_eq!(inv.paperdoll_slot_of(oid), None);
    }
}
