//! Port of `model/itemcontainer/Inventory` + `PlayerInventory`, scoped to G5:
//! DB-backed item instances, the paperdoll, and the equip/unequip slot
//! resolution ordinary gear exercises. Warehouse/trade/pickup/enchant/
//! augmentation arrive with later milestones.

use crate::data::item_data::kinds::ItemKind;
use crate::data::item_data::{self, ItemData};
use crate::db::ItemRow;

/// Port of the `Inventory.PAPERDOLL_*` constants — the 37 equipment-slot
/// indices of the paperdoll array. The numeric values are storage/DB indices
/// (`items.loc_data`), **not** the client wire order; packets order slots via
/// `InventorySlot` / `PAPERDOLL_ORDER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    Agathion1 = 17,
    Agathion2 = 18,
    Agathion3 = 19,
    Agathion4 = 20,
    Agathion5 = 21,
    Deco1 = 22,
    Deco2 = 23,
    Deco3 = 24,
    Deco4 = 25,
    Deco5 = 26,
    Deco6 = 27,
    Cloak = 28,
    Belt = 29,
    Brooch = 30,
    BroochJewel1 = 31,
    BroochJewel2 = 32,
    BroochJewel3 = 33,
    BroochJewel4 = 34,
    BroochJewel5 = 35,
    BroochJewel6 = 36,
}

impl PaperdollSlot {
    /// Every slot, in index order.
    pub const ALL: [PaperdollSlot; PAPERDOLL_TOTAL_SLOTS] = {
        use PaperdollSlot::*;
        [
            Under,
            Head,
            Hair,
            Hair2,
            Neck,
            RHand,
            Chest,
            LHand,
            REar,
            LEar,
            Gloves,
            Legs,
            Feet,
            RFinger,
            LFinger,
            LBracelet,
            RBracelet,
            Agathion1,
            Agathion2,
            Agathion3,
            Agathion4,
            Agathion5,
            Deco1,
            Deco2,
            Deco3,
            Deco4,
            Deco5,
            Deco6,
            Cloak,
            Belt,
            Brooch,
            BroochJewel1,
            BroochJewel2,
            BroochJewel3,
            BroochJewel4,
            BroochJewel5,
            BroochJewel6,
        ]
    };

    /// The slot at a paperdoll index (`items.loc_data`), if it is one.
    pub fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }
}

/// Wedding Bouquet — the one item `Inventory.equipItem` lets past formal wear.
const WEDDING_BOUQUET_ITEM_ID: i32 = 21163;

/// `Inventory.PAPERDOLL_TOTALSLOTS`.
pub const PAPERDOLL_TOTAL_SLOTS: usize = 37;

/// How many talisman, brooch-jewel and agathion slots are open — Java's
/// `PlayerStat.getTalismanSlots` / `getBroochJewelSlots` / `getAgathionSlots`,
/// which `Inventory.equipItem` reads to place those three multi-slot body
/// parts. Resolved by the caller, because the answer depends on the owner
/// rather than on the bag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenSlots {
    pub talisman: usize,
    pub brooch_jewel: usize,
    pub agathion: usize,
}

impl OpenSlots {
    /// Before `hasEnteredWorld`, `PlayerStat` reports the maximum of each
    /// (6/6/5), so character creation and restore can place anything.
    pub const BEFORE_ENTER_WORLD: Self = Self {
        talisman: 6,
        brooch_jewel: 6,
        agathion: 5,
    };
    /// `PlayerStat`'s three getters for a player: the maximum until
    /// `EnterWorld` has finished, then `(int) getValue(stat, 0)` of each.
    pub fn of_player(
        entered_world: bool,
        mods: &crate::model::components::stats::StatModifiers,
    ) -> Self {
        use crate::model::stats::Stat;
        if !entered_world {
            return Self::BEFORE_ENTER_WORLD;
        }
        let count = |v: f64| usize::try_from(v as i32).unwrap_or(0);
        let stat = |s| count(crate::model::stat_finalize::finalize(mods, s, 0.0));
        Self {
            // A counter rather than a stat in Java: no multiplier or
            // `//setparam` override reaches it.
            talisman: count(mods.add.get(&Stat::TalismanSlots).copied().unwrap_or(0.0)),
            brooch_jewel: stat(Stat::BroochJewels),
            agathion: stat(Stat::AgathionSlots),
        }
    }

    /// A pet's bag: Java would call `asPlayer()` on a pet and fail, but a pet
    /// never reaches these body parts — none of its gear has them.
    pub const NONE: Self = Self {
        talisman: 0,
        brooch_jewel: 0,
        agathion: 0,
    };
}

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

    /// A standalone instance with an enchant level, defaulted exactly like
    /// [`Self::new`]. For the transient items that are serialized into a
    /// packet without ever entering a container — a store row, a trade offer,
    /// an auction's advertised reward — where the alternative is spelling the
    /// eight zeroed fields out at the call site.
    pub(crate) fn detached(object_id: i32, item_id: i32, count: i64, enchant: i32) -> Self {
        Self {
            enchant_level: enchant,
            ..Self::new(object_id, item_id, count)
        }
    }

    /// Java `Item.isAugmented` — has a variation attached.
    pub fn is_augmented(&self) -> bool {
        self.augment_option1 != 0 || self.augment_option2 != 0
    }
}

/// One inventory mutation, as the Java `InventoryUpdate` change types
/// (1 = added, 2 = modified, 3 = removed). The variant is decided at the
/// mutation site, where the information lives: `addItem` knows whether it
/// minted a fresh instance (`Added` — the client must create the slot) or
/// grew an existing stack (`Modified`); `remove_item` knows whether the
/// stack survived. Every variant carries a snapshot taken *after* the
/// mutation — for `Removed` because the instance is gone from the list,
/// for the others so the packet shows the post-change state.
#[derive(Debug, Clone, Copy)]
pub enum ItemChange {
    Added(ItemInstance),
    Modified(ItemInstance),
    Removed(ItemInstance),
}

impl ItemChange {
    /// The snapshot inside, whichever way the instance changed.
    pub fn instance(&self) -> &ItemInstance {
        match self {
            ItemChange::Added(i) | ItemChange::Modified(i) | ItemChange::Removed(i) => i,
        }
    }
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
#[derive(Debug, Clone, bevy_ecs::component::Component)]
pub struct Inventory {
    items: Vec<ItemInstance>,
    paperdoll: [Option<i32>; PAPERDOLL_TOTAL_SLOTS],
    /// Item losses awaiting an audit record, as `(item_id, count_removed)`.
    ///
    /// Java records ownership changes down in `Item.setOwnerId`/`changeCount`,
    /// which every path funnels through. This component cannot do that — it has
    /// no access to `World`, so it cannot reach the config gate, the item names
    /// or the owning player. Rather than route all ~43 callers of
    /// [`Self::remove_item`] through a `World`-aware helper (borrow surgery at
    /// every site, for an audit feature), the removal is *noted* here and
    /// [`crate::game_loop::character::inventory::drain_item_audit`] turns it
    /// into a record on the next tick, where the context exists.
    ///
    /// Only ever appended by the removal methods and drained by that function,
    /// so an inventory nobody drains (mail attachments, which never call the
    /// removal methods) cannot accumulate.
    pending_audit: Vec<(i32, i64)>,
    /// Memoized `weight::total_load` so the periodic weight sweep only walks
    /// inventories that changed. `false` (the default — a fresh or restored
    /// inventory is unsettled) until the sweep computes and stores the load;
    /// every count-mutating method clears it again. The funnel is closed:
    /// `items` is private and only the five methods below change counts, and
    /// the sweep re-derives and asserts the cache in debug builds.
    load_settled: bool,
    cached_load: i64,
}

// By hand: `Default` stops at 32-element arrays.
impl Default for Inventory {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            paperdoll: [None; PAPERDOLL_TOTAL_SLOTS],
            pending_audit: Vec::new(),
            load_settled: false,
            cached_load: 0,
        }
    }
}

impl Inventory {
    pub fn new() -> Self {
        Self::default()
    }

    /// The memoized carried weight, `None` while a count change (or a fresh
    /// restore) has it unsettled — the caller recomputes and [`Self::settle_load`]s.
    pub fn cached_load(&self) -> Option<i64> {
        self.load_settled.then_some(self.cached_load)
    }

    pub fn settle_load(&mut self, load: i64) {
        self.cached_load = load;
        self.load_settled = true;
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

    /// Java `PlayerInventory.getAvailableItems` — every **unequipped** instance
    /// paired with its template, the shape each "pick items out of your bag"
    /// window starts from (warehouse deposit, freight, mail attachments, trade,
    /// private store).
    ///
    /// Equipped gear is excluded because Java's own accessor is
    /// `!item.isEquipped()`; an instance whose `item_id` has no template is
    /// dropped rather than shown untyped. Callers layer their own predicate on
    /// top (tradable, freightable, priced, …) — that part genuinely differs per
    /// window, the two filters here did not.
    pub fn unequipped_with_templates<'a>(
        &'a self,
        data: &'a ItemData,
    ) -> impl Iterator<Item = (&'a ItemInstance, &'a item_data::template::ItemTemplate)> {
        self.items
            .iter()
            .filter(|it| self.paperdoll_slot_of(it.object_id).is_none())
            .filter_map(|it| data.get(it.item_id).map(|t| (it, t)))
    }

    /// Java `Inventory.getItemByItemId` — the first instance of `item_id`, or
    /// `None` when the bag holds none.
    ///
    /// For a stackable this is *the* stack; for a non-stackable it is whichever
    /// copy sorts first, which is what every caller wanted when it open-coded
    /// `items().iter().find(|i| i.item_id == id)`.
    pub fn first_of_item(&self, item_id: i32) -> Option<&ItemInstance> {
        self.items.iter().find(|i| i.item_id == item_id)
    }

    /// Java `Inventory.getItemByObjectId` — one specific instance in this bag,
    /// or `None` when it isn't here.
    ///
    /// The object-id counterpart of [`first_of_item`](Self::first_of_item), and
    /// the shape every packet handler needs: the client names the instance it
    /// acted on, and "not in this inventory" is the same refusal as "no such
    /// item".
    pub fn by_object_id(&self, object_id: i32) -> Option<&ItemInstance> {
        self.items.iter().find(|i| i.object_id == object_id)
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
    pub fn to_rows(&self) -> Vec<ItemRow> {
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
                ItemRow {
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
        self.load_settled = false;
        let stackable = catalog
            .get(item_id)
            .map(|t| t.is_stackable)
            .unwrap_or(false);
        if stackable && let Some(existing) = self.items.iter_mut().find(|i| i.item_id == item_id) {
            existing.count += count;
            return existing.object_id;
        }
        let mut inst = ItemInstance::new(object_id, item_id, count);
        // Java's `Item` constructors stamp `_mana = _itemTemplate.getDuration()`
        // on every freshly created instance — that (and only that) is what makes
        // an item a shadow item. `-1` for everything else.
        inst.mana_left = catalog.get(item_id).map_or(-1, |t| t.duration);
        self.items.push(inst);
        object_id
    }

    /// Write back a shadow item's remaining mana (Java `Item._mana` after
    /// `decreaseMana`). No-op if the object id is gone. See
    /// [`crate::game_loop::items::item_mana`].
    pub fn set_mana_left(&mut self, object_id: i32, mana_left: i32) {
        if let Some(it) = self.items.iter_mut().find(|i| i.object_id == object_id) {
            it.mana_left = mana_left;
        }
    }

    /// Stamp an enchant level onto an instance already in the bag. No-op if the
    /// object id is gone.
    ///
    /// Needed because the ordinary add path always creates at `+0`: a ground
    /// pickup has to re-apply the level the dropped instance carried, which
    /// Java gets for free by moving the instance instead of minting one.
    pub fn set_enchant_level(&mut self, object_id: i32, level: i32) {
        if let Some(it) = self.items.iter_mut().find(|i| i.object_id == object_id) {
            it.enchant_level = level;
        }
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
    ///
    /// `mana` is the moved instance's remaining shadow-item mana. Java moves the
    /// same `Item` object between containers, so its `_mana` rides along; here
    /// the instance is rebuilt, so the caller has to carry it. Pass `-1` when
    /// the item is genuinely new (server-generated adena, mail payouts) —
    /// re-deriving it from the template instead would **refill** a worn shadow
    /// weapon on every private-warehouse round trip, and the private warehouse
    /// is the one container that accepts them (`Item.isDepositable`:
    /// `isPrivateWareHouse || (isTradeable() && !isShadowItem())`).
    pub fn insert_instance(
        &mut self,
        catalog: &ItemData,
        object_id: i32,
        item_id: i32,
        count: i64,
        enchant: i32,
        mana: i32,
    ) {
        self.load_settled = false;
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
        inst.mana_left = mana;
        self.items.push(inst);
    }

    /// Put a complete instance back (refund buy-back): merge into an existing
    /// stack when stackable, otherwise re-add the instance as-is — keeping its
    /// object id, enchant, augment, and remaining time. Returns the resulting
    /// instance snapshot for the `InventoryUpdate`.
    pub fn restore_instance(&mut self, catalog: &ItemData, inst: ItemInstance) -> ItemInstance {
        self.load_settled = false;
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
    /// `takeItems`' clamp). Returns what happened per touched instance so the
    /// caller can mirror it to the client (`InventoryUpdate`) and the DB.
    ///
    /// A destroyed instance is lifted off the paperdoll first, mirroring
    /// Java's `Inventory.removeItem` ("Unequip item if equiped"). Worn items
    /// *do* reach this path — a registered quest item can be equipment (Q229
    /// `Test of Witchcraft` registers the Sword of Seal, a weapon, which
    /// `exitQuest` then destroys while it is still in the player's hand). The
    /// clearing alone is not enough: Java's `setPaperdollItem` also drops the
    /// item's bonuses, recalculates stats and pushes `ExUserInfoEquipSlot`,
    /// none of which a plain data component can do. Snapshot
    /// [`Self::equipped_object_ids`] before calling and hand what this
    /// unequipped to `game_loop::items::finish_equipped_item_destroyed`.
    pub fn remove_item(&mut self, item_id: i32, count: i64) -> Vec<ItemChange> {
        self.load_settled = false;
        let mut remaining = if count < 0 { i64::MAX } else { count };
        // The *actual* amount that left, which is not `count`: a request for
        // more than the player holds removes only what is there, and `count < 0`
        // means "all of it". An audit record has to state what really moved.
        let mut removed_total: i64 = 0;
        let mut changes = Vec::new();
        while remaining > 0 {
            let Some(idx) = self.items.iter().position(|i| i.item_id == item_id) else {
                break;
            };
            if self.items[idx].count > remaining {
                self.items[idx].count -= remaining;
                removed_total += remaining;
                changes.push(ItemChange::Modified(self.items[idx]));
                break;
            }
            let removed = self.items.remove(idx);
            remaining -= removed.count;
            removed_total += removed.count;
            self.paperdoll.iter_mut().for_each(|s| {
                if *s == Some(removed.object_id) {
                    *s = None; // Java `Inventory.removeItem`; see doc comment
                }
            });
            changes.push(ItemChange::Removed(removed));
        }
        if removed_total > 0 {
            self.pending_audit.push((item_id, removed_total));
        }
        changes
    }

    /// Takes the noted losses, leaving the buffer empty. Called by
    /// [`crate::game_loop::character::inventory::drain_item_audit`] — see
    /// [`Self::pending_audit`].
    pub fn take_pending_audit(&mut self) -> Vec<(i32, i64)> {
        std::mem::take(&mut self.pending_audit)
    }

    /// Whether anything is waiting, so the per-tick drain can skip the common
    /// case without allocating.
    pub fn has_pending_audit(&self) -> bool {
        !self.pending_audit.is_empty()
    }

    /// Destroy `count` of one specific instance by `object_id` (Java
    /// `destroyItem(process, objectId, count, ...)`) — unlike [`Self::remove_item`],
    /// which targets any stack of a given item id, this targets exactly the
    /// clicked instance. Used by `UseItem`'s `EtcItem` branch (e.g.
    /// `ExtractableItems`), where the client names the object id, not the
    /// item id.
    pub fn remove_by_object_id(&mut self, object_id: i32, count: i64) -> Option<ItemChange> {
        self.load_settled = false;
        let idx = self.items.iter().position(|i| i.object_id == object_id)?;
        let item_id = self.items[idx].item_id;
        if self.items[idx].count > count {
            self.items[idx].count -= count;
            // A partial take: `count` is bounded by the stack above, so here it
            // really is the amount that left.
            self.pending_audit.push((item_id, count));
            return Some(ItemChange::Modified(self.items[idx]));
        }
        let removed = self.items.remove(idx);
        self.pending_audit.push((item_id, removed.count));
        self.paperdoll.iter_mut().for_each(|s| {
            if *s == Some(removed.object_id) {
                *s = None; // defensive; see remove_item's doc comment
            }
        });
        Some(ItemChange::Removed(removed))
    }

    /// `BodyPart.getPaperdollSlot` — the slot a body part names directly.
    /// `None` for the parts that name none: the paired ones (`rear;lear`,
    /// `rfinger;lfinger`), `lrhand`, `fullarmor`, `alldress`, `hairall`, the
    /// pet parts and `none`.
    ///
    /// Java maps `none` to `PAPERDOLL_TOTALSLOTS`, one past the array; the
    /// only caller that does not first rule out a slotless item is the try-on
    /// preview, which handles that case itself.
    pub(crate) fn body_part_slot(body_part: i64) -> Option<PaperdollSlot> {
        use item_data::*;
        Some(match body_part {
            SLOT_UNDERWEAR => PaperdollSlot::Under,
            SLOT_R_EAR => PaperdollSlot::REar,
            SLOT_L_EAR => PaperdollSlot::LEar,
            SLOT_NECK => PaperdollSlot::Neck,
            SLOT_R_FINGER => PaperdollSlot::RFinger,
            SLOT_L_FINGER => PaperdollSlot::LFinger,
            SLOT_HEAD => PaperdollSlot::Head,
            SLOT_R_HAND => PaperdollSlot::RHand,
            SLOT_L_HAND => PaperdollSlot::LHand,
            SLOT_GLOVES => PaperdollSlot::Gloves,
            SLOT_CHEST => PaperdollSlot::Chest,
            SLOT_LEGS => PaperdollSlot::Legs,
            SLOT_FEET => PaperdollSlot::Feet,
            SLOT_BACK => PaperdollSlot::Cloak,
            SLOT_HAIR => PaperdollSlot::Hair,
            SLOT_HAIR2 => PaperdollSlot::Hair2,
            SLOT_R_BRACELET => PaperdollSlot::RBracelet,
            SLOT_L_BRACELET => PaperdollSlot::LBracelet,
            SLOT_DECO => PaperdollSlot::Deco1,
            SLOT_BELT => PaperdollSlot::Belt,
            SLOT_BROOCH => PaperdollSlot::Brooch,
            SLOT_BROOCH_JEWEL => PaperdollSlot::BroochJewel1,
            SLOT_AGATHION => PaperdollSlot::Agathion1,
            _ => return None,
        })
    }

    /// `BodyPart.getPaperdollIndex` — [`Self::body_part_slot`] plus the parts
    /// that borrow another's slot: `lrhand` the right hand, `fullarmor` and
    /// `alldress` the chest, `hairall` the hair.
    pub(crate) fn body_part_index(body_part: i64) -> Option<PaperdollSlot> {
        use item_data::*;
        match body_part {
            SLOT_LR_HAND => Some(PaperdollSlot::RHand),
            SLOT_FULL_ARMOR | SLOT_ALLDRESS => Some(PaperdollSlot::Chest),
            SLOT_HAIRALL => Some(PaperdollSlot::Hair),
            other => Self::body_part_slot(other),
        }
    }

    /// `BodyPart.fromPaperdollSlot` — the body part a paperdoll index belongs
    /// to. The multi-slot runs collapse onto their part: every agathion,
    /// talisman and brooch-jewel slot answers `agathion`, `deco1` and
    /// `brooch_jewel`, whose own slot is the *first* of the run.
    pub(crate) fn body_part_of_slot(slot: PaperdollSlot) -> i64 {
        use PaperdollSlot as P;
        use item_data::*;
        match slot {
            P::Under => SLOT_UNDERWEAR,
            P::Head => SLOT_HEAD,
            P::Hair => SLOT_HAIR,
            P::Hair2 => SLOT_HAIR2,
            P::Neck => SLOT_NECK,
            P::RHand => SLOT_R_HAND,
            P::Chest => SLOT_CHEST,
            P::LHand => SLOT_L_HAND,
            P::REar => SLOT_R_EAR,
            P::LEar => SLOT_L_EAR,
            P::Gloves => SLOT_GLOVES,
            P::Legs => SLOT_LEGS,
            P::Feet => SLOT_FEET,
            P::RFinger => SLOT_R_FINGER,
            P::LFinger => SLOT_L_FINGER,
            P::LBracelet => SLOT_L_BRACELET,
            P::RBracelet => SLOT_R_BRACELET,
            P::Agathion1 | P::Agathion2 | P::Agathion3 | P::Agathion4 | P::Agathion5 => {
                SLOT_AGATHION
            }
            P::Deco1 | P::Deco2 | P::Deco3 | P::Deco4 | P::Deco5 | P::Deco6 => SLOT_DECO,
            P::Cloak => SLOT_BACK,
            P::Belt => SLOT_BELT,
            P::Brooch => SLOT_BROOCH,
            P::BroochJewel1
            | P::BroochJewel2
            | P::BroochJewel3
            | P::BroochJewel4
            | P::BroochJewel5
            | P::BroochJewel6 => SLOT_BROOCH_JEWEL,
        }
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

    pub fn equip_item(&mut self, catalog: &ItemData, object_id: i32, open: OpenSlots) -> Vec<i32> {
        use PaperdollSlot as P;
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
        let body_part_in = |inv: &Self, slot: P| {
            inv.paperdoll_item(slot)
                .and_then(|i| catalog.get(i.item_id))
                .map(|t| t.body_part)
        };

        // "Check if player is using Formal Wear and item isn't Wedding Bouquet.
        // Only chest target can pass this."
        if item_id != WEDDING_BOUQUET_ITEM_ID
            && body_part_in(self, P::Chest) == Some(item_data::SLOT_ALLDRESS)
            && matches!(
                body_part,
                item_data::SLOT_LR_HAND
                    | item_data::SLOT_L_HAND
                    | item_data::SLOT_R_HAND
                    | item_data::SLOT_LEGS
                    | item_data::SLOT_FEET
                    | item_data::SLOT_GLOVES
                    | item_data::SLOT_HEAD
            )
        {
            return Vec::new();
        }

        let mut changed = Vec::new();
        match body_part {
            item_data::SLOT_LR_HAND => {
                changed.extend(self.clear(P::LHand));
                changed.extend(self.put(P::RHand, object_id));
            }
            item_data::SLOT_L_HAND => {
                // A two-handed weapon is displaced by an off-hand item. Java
                // spares a bow from its arrows (and crossbow/bolts, rod/lure),
                // but those are `Etc` items and never get this far: arrows go
                // through `equip_ammunition`.
                if body_part_in(self, P::RHand) == Some(item_data::SLOT_LR_HAND) {
                    changed.extend(self.clear(P::RHand));
                }
                changed.extend(self.put(P::LHand, object_id));
            }
            item_data::SLOT_L_EAR | item_data::SLOT_R_EAR | item_data::SLOT_LR_EAR => {
                changed.extend(self.put_pair(P::LEar, P::REar, object_id));
            }
            item_data::SLOT_L_FINGER | item_data::SLOT_R_FINGER | item_data::SLOT_LR_FINGER => {
                changed.extend(self.put_pair(P::LFinger, P::RFinger, object_id));
            }
            item_data::SLOT_FULL_ARMOR => {
                changed.extend(self.clear(P::Legs));
                changed.extend(self.put(P::Chest, object_id));
            }
            item_data::SLOT_LEGS => {
                if body_part_in(self, P::Chest) == Some(item_data::SLOT_FULL_ARMOR) {
                    changed.extend(self.clear(P::Chest));
                }
                changed.extend(self.put(P::Legs, object_id));
            }
            item_data::SLOT_HAIR => {
                // A hair-all piece in Hair covers Hair2 as well.
                if body_part_in(self, P::Hair) == Some(item_data::SLOT_HAIRALL) {
                    changed.extend(self.clear(P::Hair2));
                } else {
                    changed.extend(self.clear(P::Hair));
                }
                changed.extend(self.put(P::Hair, object_id));
            }
            item_data::SLOT_HAIR2 => {
                if body_part_in(self, P::Hair) == Some(item_data::SLOT_HAIRALL) {
                    changed.extend(self.clear(P::Hair));
                } else {
                    changed.extend(self.clear(P::Hair2));
                }
                changed.extend(self.put(P::Hair2, object_id));
            }
            item_data::SLOT_HAIRALL => {
                changed.extend(self.clear(P::Hair2));
                changed.extend(self.put(P::Hair, object_id));
            }
            item_data::SLOT_ALLDRESS => {
                for slot in [P::Legs, P::LHand, P::RHand, P::Head, P::Feet, P::Gloves] {
                    changed.extend(self.clear(slot));
                }
                changed.extend(self.put(P::Chest, object_id));
            }
            item_data::SLOT_DECO => {
                changed.extend(self.put_in_run(P::Deco1, open.talisman, item_id, object_id));
            }
            item_data::SLOT_BROOCH_JEWEL => {
                changed.extend(self.put_in_run(
                    P::BroochJewel1,
                    open.brooch_jewel,
                    item_id,
                    object_id,
                ));
            }
            item_data::SLOT_AGATHION => {
                changed.extend(self.put_in_run(P::Agathion1, open.agathion, item_id, object_id));
            }
            other => {
                // Every remaining part goes in its own slot; Java logs
                // "Unknown body slot" for the pet parts and equips nothing.
                if let Some(slot) = Self::body_part_slot(other) {
                    changed.extend(self.put(slot, object_id));
                }
            }
        }

        changed.sort_unstable();
        changed.dedup();
        changed
    }

    /// Port of `Inventory.unEquipItemInBodySlot`: clear the slot
    /// [`Self::body_part_index`] names (`hairall` clears Hair first, which is
    /// that same slot). Returns the changed `object_id`s.
    pub fn unequip_body_part(&mut self, body_part: i64) -> Vec<i32> {
        if body_part == item_data::SLOT_HAIRALL {
            self.clear(PaperdollSlot::Hair);
        }
        match Self::body_part_index(body_part) {
            Some(slot) => self.clear(slot).into_iter().collect(),
            None => Vec::new(),
        }
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

    fn set(&mut self, slot: PaperdollSlot, object_id: i32) -> i32 {
        self.paperdoll[slot as usize] = Some(object_id);
        object_id
    }

    /// `setPaperdollItem(slot, item)`: the item goes in, and whatever sat
    /// there comes out. Both are changes.
    fn put(&mut self, slot: PaperdollSlot, object_id: i32) -> Vec<i32> {
        let mut changed: Vec<i32> = self.clear(slot).into_iter().collect();
        changed.push(self.set(slot, object_id));
        changed
    }

    /// The ear and finger pairs: the left slot if free, else the right one,
    /// else the left one again.
    fn put_pair(&mut self, left: PaperdollSlot, right: PaperdollSlot, object_id: i32) -> Vec<i32> {
        let target = if self.paperdoll[left as usize].is_none() {
            left
        } else if self.paperdoll[right as usize].is_none() {
            right
        } else {
            left
        };
        self.put(target, object_id)
    }

    /// `equipTalisman` / `equipBroochJewel` / `equipAgathion`: within the
    /// first `open` slots of the run, replace a copy of the same item if one
    /// is worn, else take the first free slot. A full run equips nothing.
    fn put_in_run(
        &mut self,
        first: PaperdollSlot,
        open: usize,
        item_id: i32,
        object_id: i32,
    ) -> Vec<i32> {
        let run = first as usize..first as usize + open;
        let same = run.clone().find(|&i| {
            self.paperdoll[i]
                .and_then(|oid| self.find(oid))
                .is_some_and(|w| w.item_id == item_id)
        });
        let free = || run.clone().find(|&i| self.paperdoll[i].is_none());
        match same.or_else(free).and_then(PaperdollSlot::from_index) {
            Some(slot) => self.put(slot, object_id),
            None => Vec::new(),
        }
    }

    fn clear(&mut self, slot: PaperdollSlot) -> Option<i32> {
        self.paperdoll[slot as usize].take()
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

    /// Every object id currently sitting in a paperdoll slot. Snapshot this
    /// *before* a destroy to learn which instances the removal unequipped —
    /// see [`Self::remove_item`] for why that has to be driven by hand here
    /// when Java gets it for free inside `Inventory.removeItem`.
    pub fn equipped_object_ids(&self) -> Vec<i32> {
        self.paperdoll.iter().flatten().copied().collect()
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
    use crate::data::item_data::template::ItemTemplate;

    fn armor(id: i32, body_part: i64) -> ItemTemplate {
        ItemTemplate {
            item_id: id,
            name: format!("armor{id}"),
            kind: ItemKind::Armor,
            body_part,
            is_stackable: false,
            is_quest_item: false,
            ..ItemTemplate::for_test()
        }
    }

    fn weapon(id: i32, body_part: i64) -> ItemTemplate {
        ItemTemplate {
            item_id: id,
            name: format!("weapon{id}"),
            kind: ItemKind::Weapon,
            body_part,
            is_stackable: false,
            is_quest_item: false,
            ..ItemTemplate::for_test()
        }
    }

    /// The audit note must state what actually left, not what was asked for.
    /// Java records the real count change; a record saying 500 adena moved when
    /// only 100 existed would be worse than no record.
    #[test]
    fn the_pending_audit_note_is_what_actually_left() {
        let catalog = ItemData::from_templates(vec![armor(1, item_data::SLOT_CHEST)]);
        let mut inv = Inventory::new();
        inv.add_item(&catalog, 1, 1, 1);
        assert!(!inv.has_pending_audit(), "adding must not note a loss");

        // Ask for more than is held.
        inv.remove_item(1, 50);
        assert_eq!(inv.take_pending_audit(), vec![(1, 1)]);
        // Draining empties it, so the next tick cannot re-record the same loss.
        assert!(!inv.has_pending_audit());
    }

    /// `count < 0` is the "remove all of it" convention; the note has to
    /// resolve that to the real amount rather than passing the sentinel on.
    #[test]
    fn removing_all_notes_the_real_amount_not_the_sentinel() {
        let catalog = ItemData::from_templates(vec![armor(1, item_data::SLOT_CHEST)]);
        let mut inv = Inventory::new();
        inv.add_item(&catalog, 1, 1, 1);
        inv.add_item(&catalog, 2, 1, 1);
        inv.remove_item(1, -1);
        let noted = inv.take_pending_audit();
        assert_eq!(
            noted,
            vec![(1, 2)],
            "expected both instances, got {noted:?}"
        );
    }

    /// A removal that matches nothing must stay silent — an audit file full of
    /// zero-count entries is how a real one stops being read.
    #[test]
    fn a_removal_that_finds_nothing_notes_nothing() {
        let mut inv = Inventory::new();
        inv.remove_item(1, 5);
        assert!(!inv.has_pending_audit());
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
        inv.equip_item(&catalog, 2, OpenSlots::BEFORE_ENTER_WORLD);
        inv.equip_item(&catalog, 1, OpenSlots::BEFORE_ENTER_WORLD);
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
        inv.equip_item(&catalog, 1, OpenSlots::BEFORE_ENTER_WORLD);
        inv.equip_item(&catalog, 2, OpenSlots::BEFORE_ENTER_WORLD);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Chest), 1);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Legs), 2);

        inv.add_item(&catalog, 3, 3, 1);
        inv.equip_item(&catalog, 3, OpenSlots::BEFORE_ENTER_WORLD);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Chest), 3);
        assert_eq!(
            inv.paperdoll_object_id(PaperdollSlot::Legs),
            0,
            "full armor clears Legs"
        );

        // Equipping Legs again displaces the full-armor piece from Chest.
        inv.equip_item(&catalog, 2, OpenSlots::BEFORE_ENTER_WORLD);
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
        inv.equip_item(&catalog, 1, OpenSlots::BEFORE_ENTER_WORLD);
        inv.equip_item(&catalog, 2, OpenSlots::BEFORE_ENTER_WORLD);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::RHand), 1);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::LHand), 2);

        // Equipping a two-handed weapon clears both hands, occupies RHand only.
        inv.add_item(&catalog, 3, 3, 1);
        inv.equip_item(&catalog, 3, OpenSlots::BEFORE_ENTER_WORLD);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::RHand), 3);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::LHand), 0);

        // Equipping the shield again displaces the two-handed weapon from RHand.
        inv.equip_item(&catalog, 2, OpenSlots::BEFORE_ENTER_WORLD);
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
        inv.equip_item(&catalog, 1, OpenSlots::BEFORE_ENTER_WORLD);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::LEar), 1);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::REar), 0);

        inv.equip_item(&catalog, 2, OpenSlots::BEFORE_ENTER_WORLD);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::LEar), 1);
        assert_eq!(
            inv.paperdoll_object_id(PaperdollSlot::REar),
            2,
            "second earring fills the free REar slot"
        );
    }

    /// Java's pair rule: left, then right, then the left one again.
    #[test]
    fn a_third_earring_replaces_the_left_one() {
        let ear = item_data::SLOT_LR_EAR;
        let catalog = ItemData::from_templates(vec![armor(1, ear), armor(2, ear), armor(3, ear)]);
        let mut inv = Inventory::new();
        for oid in 1..=3 {
            inv.add_item(&catalog, oid, oid, 1);
        }
        inv.equip_item(&catalog, 1, OpenSlots::BEFORE_ENTER_WORLD);
        inv.equip_item(&catalog, 2, OpenSlots::BEFORE_ENTER_WORLD);
        let changed = inv.equip_item(&catalog, 3, OpenSlots::BEFORE_ENTER_WORLD);
        assert_eq!(changed, [1, 3]);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::LEar), 3);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::REar), 2);
    }

    /// Agathions, talismans and brooch jewels fill their run up to the open
    /// count, swap a copy of the same item in place, and do nothing when the
    /// run is full or closed.
    #[test]
    fn multi_slot_parts_fill_only_their_open_slots() {
        let catalog = ItemData::from_templates(vec![
            armor(10, item_data::SLOT_AGATHION),
            armor(11, item_data::SLOT_AGATHION),
            armor(12, item_data::SLOT_AGATHION),
            armor(20, item_data::SLOT_DECO),
        ]);
        let mut inv = Inventory::new();
        for (oid, id) in [(1, 10), (2, 11), (3, 12), (4, 10), (5, 20)] {
            inv.add_item(&catalog, oid, id, 1);
        }
        let two = OpenSlots {
            talisman: 0,
            brooch_jewel: 0,
            agathion: 2,
        };
        inv.equip_item(&catalog, 1, two);
        inv.equip_item(&catalog, 2, two);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Agathion1), 1);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Agathion2), 2);
        assert!(
            inv.equip_item(&catalog, 3, two).is_empty(),
            "the run is full"
        );
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Agathion3), 0);
        // A second copy of item 10 takes the first one's slot.
        assert_eq!(inv.equip_item(&catalog, 4, two), [1, 4]);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Agathion1), 4);
        assert!(
            inv.equip_item(&catalog, 5, two).is_empty(),
            "no talisman slot is open"
        );
    }

    #[test]
    fn a_hair_accessory_keeps_the_other_hair_slot() {
        let catalog = ItemData::from_templates(vec![
            armor(1, item_data::SLOT_HAIR2),
            armor(2, item_data::SLOT_HAIR),
            armor(3, item_data::SLOT_HAIRALL),
            armor(4, item_data::SLOT_HAIR2),
        ]);
        let mut inv = Inventory::new();
        for oid in 1..=4 {
            inv.add_item(&catalog, oid, oid, 1);
        }
        let open = OpenSlots::BEFORE_ENTER_WORLD;
        inv.equip_item(&catalog, 1, open);
        inv.equip_item(&catalog, 2, open);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Hair2), 1);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Hair), 2);
        // A hair-all piece covers both; a hair2 accessory then displaces it.
        inv.equip_item(&catalog, 3, open);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Hair), 3);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Hair2), 0);
        inv.equip_item(&catalog, 4, open);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Hair), 0);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Hair2), 4);
    }

    #[test]
    fn formal_wear_refuses_everything_but_the_chest_and_the_bouquet() {
        let catalog = ItemData::from_templates(vec![
            armor(1, item_data::SLOT_ALLDRESS),
            armor(2, item_data::SLOT_GLOVES),
            weapon(WEDDING_BOUQUET_ITEM_ID, item_data::SLOT_R_HAND),
            armor(3, item_data::SLOT_NECK),
        ]);
        let mut inv = Inventory::new();
        inv.add_item(&catalog, 1, 1, 1);
        inv.add_item(&catalog, 2, 2, 1);
        inv.add_item(&catalog, 3, WEDDING_BOUQUET_ITEM_ID, 1);
        inv.add_item(&catalog, 4, 3, 1);
        let open = OpenSlots::BEFORE_ENTER_WORLD;
        inv.equip_item(&catalog, 1, open);
        assert!(inv.equip_item(&catalog, 2, open).is_empty());
        assert_eq!(inv.equip_item(&catalog, 3, open), [3]);
        assert_eq!(
            inv.equip_item(&catalog, 4, open),
            [4],
            "jewellery is not covered"
        );
    }

    /// `fromPaperdollSlot` folds each multi-slot run onto its first slot, so
    /// a body-part unequip by any of them clears the first.
    #[test]
    fn a_run_slot_unequips_the_first_of_its_run() {
        assert_eq!(
            Inventory::body_part_of_slot(PaperdollSlot::Deco3),
            item_data::SLOT_DECO
        );
        let catalog = ItemData::from_templates(vec![
            armor(1, item_data::SLOT_DECO),
            armor(2, item_data::SLOT_DECO),
        ]);
        let mut inv = Inventory::new();
        inv.add_item(&catalog, 1, 1, 1);
        inv.add_item(&catalog, 2, 2, 1);
        inv.equip_item(&catalog, 1, OpenSlots::BEFORE_ENTER_WORLD);
        inv.equip_item(&catalog, 2, OpenSlots::BEFORE_ENTER_WORLD);
        let part = Inventory::body_part_of_slot(PaperdollSlot::Deco2);
        assert_eq!(inv.unequip_body_part(part), [1]);
        assert_eq!(inv.paperdoll_object_id(PaperdollSlot::Deco2), 2);
    }

    #[test]
    fn slot_counts_are_the_maximum_until_the_world_is_entered() {
        use crate::model::components::stats::StatModifiers;
        use crate::model::stats::Stat;

        let mut mods = StatModifiers::default();
        assert_eq!(
            OpenSlots::of_player(false, &mods),
            OpenSlots::BEFORE_ENTER_WORLD
        );
        assert_eq!(OpenSlots::of_player(true, &mods), OpenSlots::NONE);
        mods.add.insert(Stat::TalismanSlots, 2.0);
        mods.add.insert(Stat::AgathionSlots, 3.0);
        mods.mul.insert(Stat::TalismanSlots, 4.0);
        assert_eq!(
            OpenSlots::of_player(true, &mods),
            OpenSlots {
                talisman: 2,
                brooch_jewel: 0,
                agathion: 3
            },
            "the talisman counter ignores multipliers"
        );
    }

    #[test]
    fn quest_items_are_excluded_from_the_ordinary_capacity_count() {
        let catalog = ItemData::from_templates(vec![
            ItemTemplate {
                item_id: 1,
                name: "quest_item".into(),
                kind: ItemKind::Etc,
                body_part: item_data::SLOT_NONE,
                is_stackable: false,
                is_quest_item: true,
                ..ItemTemplate::for_test()
            },
            ItemTemplate {
                item_id: 2,
                name: "ordinary_item".into(),
                kind: ItemKind::Etc,
                body_part: item_data::SLOT_NONE,
                is_stackable: false,
                is_quest_item: false,
                ..ItemTemplate::for_test()
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
            item_id: 57,
            name: "Adena".into(),
            kind: ItemKind::Etc,
            body_part: item_data::SLOT_NONE,
            is_stackable: true,
            is_quest_item: false,
            ..ItemTemplate::for_test()
        }]);
        let mut inv = Inventory::new();
        let oid = inv.add_item(&catalog, 1, 57, 100);
        let changed = inv.equip_item(&catalog, oid, OpenSlots::BEFORE_ENTER_WORLD);
        assert!(changed.is_empty());
        assert_eq!(inv.paperdoll_slot_of(oid), None);
    }

    /// `remove_item` lifts a destroyed instance off the paperdoll by itself —
    /// Java `Inventory.removeItem`'s "Unequip item if equiped".
    ///
    /// Call sites used to run `unequip_item` first, guarded by
    /// `paperdoll_slot_of(..).is_some()`. Both halves were dead: the guard
    /// repeated the lookup `unequip_item` does internally, and `unequip_item`
    /// repeated the clearing `remove_item` does one line later — less
    /// thoroughly, since it frees only the first matching slot while the
    /// removal frees every slot the instance occupies. This pins the behaviour
    /// those call sites now lean on.
    #[test]
    fn removing_an_item_frees_its_paperdoll_slot() {
        let catalog = ItemData::from_templates(vec![weapon(1, item_data::SLOT_R_HAND)]);
        let mut inv = Inventory::new();
        let oid = inv.add_item(&catalog, 100, 1, 1);
        inv.equip_item(&catalog, oid, OpenSlots::BEFORE_ENTER_WORLD);
        assert!(inv.paperdoll_slot_of(oid).is_some(), "equipped first");

        inv.remove_item(1, 1);

        assert_eq!(
            inv.paperdoll_slot_of(oid),
            None,
            "the destroyed instance is off the paperdoll"
        );
        assert!(inv.first_of_item(1).is_none(), "and gone from the bag");
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
    pub fn from_rows(rows: &[ItemRow]) -> Self {
        Self(Inventory::from_rows(rows))
    }

    /// Serialize to `items` rows with `loc="WAREHOUSE"` (nothing is equipped in a
    /// warehouse, so [`Inventory::to_rows`] yields `INVENTORY`; remap the loc).
    pub fn to_rows(&self) -> Vec<ItemRow> {
        let mut rows = self.0.to_rows();
        for r in &mut rows {
            r.loc = "WAREHOUSE".to_string();
            r.loc_data = 0;
        }
        rows
    }

    /// Serialize to `items` rows with `loc="CLANWH"` — the clan warehouse's
    /// persistence location (`owner_id` = clan id, bound by the DB layer).
    pub fn to_rows_clan(&self) -> Vec<ItemRow> {
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
    pub fn from_rows(rows: &[ItemRow]) -> Self {
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
    pub fn to_rows(&self) -> Vec<ItemRow> {
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
    pub fn from_rows(rows: &[ItemRow]) -> Self {
        Self(Inventory::from_rows(rows))
    }

    /// Serialize to `items` rows with `loc="FREIGHT"`.
    pub fn to_rows(&self) -> Vec<ItemRow> {
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
