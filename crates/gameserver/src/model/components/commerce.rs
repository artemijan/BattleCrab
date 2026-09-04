//! Trading state: ground items, the player-run stores, trade and warehouse
//! sessions, enchant and multisell, the recipe book.

use bevy_ecs::component::Component;

/// An item lying on the ground (Java `Item` in `ItemLocation.VOID`, tracked by
/// `ItemsOnGroundManager`). A world entity with [`super::space::Position`]/[`super::space::RegionCell`];
/// indexed in `World::ground_item_regions`. Dropped by players (`//` drop) or
/// monster death (auto-loot off), picked up via a click (`Action`).
#[derive(Component, Debug, Clone)]
pub struct GroundItem {
    pub object_id: i32,
    pub item_id: i32,
    pub count: i64,
    pub enchant: i32,
    /// Loot protection (Java `Item._ownerId` + the `ResetOwner` schedule,
    /// set by `ItemData.createItem("loot")`): while `world.tick <
    /// owner_until_tick`, only `owner_id`, their party, or — for raid drops —
    /// their command channel may pick the item up. `0`/`0` = unprotected.
    /// Expiry is lazy (checked at pickup) instead of Java's scheduled task.
    pub owner_id: i32,
    pub owner_until_tick: u64,
    /// Java `Item._dropTime`, wall-clock ms — **`-1` means protected**, i.e.
    /// never auto-destroyed (`item.setProtected(dropTime == -1)`).
    ///
    /// Only [`game_loop::ground_items`](crate::game_loop::items::ground_items)'
    /// persistence reads it: the decay itself is a scheduler entry, but a row
    /// reloaded from `itemsonground` has to know how much of its lifetime was
    /// already spent, and ticks do not survive a restart.
    pub drop_time_ms: i64,
}

/// Java `Attackable._firstCommandChannelAttacked` + `_commandChannelLastAttack`:
/// the command channel that earned raid looting rights on this boss, refreshed
/// on every hit from that channel. Expires `RaidLootRightsInterval` after the
/// last hit — lazily (checked on read) instead of Java's 10 s polling timer.
#[derive(Component, Debug, Clone, Copy)]
pub struct RaidLootRights {
    pub cc_id: u32,
    pub last_attack_tick: u64,
}

/// One line in a player's private sell store (Java `TradeItem`): the inventory
/// instance offered, how many, and the asking price per unit.
#[derive(Debug, Clone, Copy)]
pub struct StoreItem {
    pub object_id: i32,
    pub item_id: i32,
    pub count: i64,
    pub price: i64,
    pub enchant: i32,
}

/// A player's active private sell store (Java `Player._sellList` + store title).
/// Present only while the store is open; the store *type* (the CharInfo byte)
/// lives on [`Player::store_type`](crate::model::Player::store_type).
#[derive(Component, Debug, Clone, Default)]
pub struct PrivateStore {
    pub items: Vec<StoreItem>,
    pub title: String,
    /// Java `TradeList.isPackaged()` — a **package** store (`/packagesale`,
    /// `PrivateStoreType.PACKAGE_SELL`): the whole list is sold as one lot, so
    /// a buyer must take every line at once.
    pub packaged: bool,
}

/// One line of a private **buy** store: what the owner wants, how many are
/// still wanted, and what they pay each. Keyed by item id — the owner doesn't
/// hold the item yet, which is what separates this from [`StoreItem`].
#[derive(Debug, Clone, Copy)]
pub struct WantedItem {
    pub item_id: i32,
    pub count: i64,
    pub price: i64,
    pub enchant: i32,
}

/// A player's active private *buy* store (Java `Player._buyList` + store
/// title). Present only while the store is open; the store *type* byte
/// (BUY / BUY_MANAGE) lives on
/// [`Player::store_type`](crate::model::Player::store_type).
#[derive(Component, Debug, Clone, Default)]
pub struct PrivateBuyStore {
    pub items: Vec<WantedItem>,
    pub title: String,
}

/// An in-progress player-to-player trade (Java `Player._activeTradeList`).
/// Present on both partners while the trade window is open; `items` are this
/// player's offered lines (`price` unused), `confirmed` is their "OK" press.
#[derive(Component, Debug, Clone, Default)]
pub struct Trade {
    pub partner: i32,
    pub items: Vec<StoreItem>,
    pub confirmed: bool,
}

/// A pending trade *request* on the target (Java `Player._activeRequester`):
/// `from` asked to trade; cleared on answer/timeout.
#[derive(Component, Debug, Clone, Copy)]
pub struct PendingTrade {
    pub from: i32,
}

/// Which warehouse the player currently has open (Java
/// `Player._activeWarehouse`), set by the warehouse-keeper bypass. The
/// deposit/withdraw client packets carry no warehouse type, so the handlers
/// read this to route items to the right container.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ActiveWarehouse {
    #[default]
    Private,
    Clan,
    Freight,
}

/// An open enchant window (Java `EnchantItemRequest`, held as a `Player`
/// request). Present from the `EnchantScrolls` handler's `ChooseInventoryItem`
/// until the enchant completes or is cancelled. Object ids are `0` (none) until
/// the client fills them via the Ex-packet handshake.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct EnchantRequest {
    /// The scroll's inventory object id (set when the window opens).
    pub scroll_oid: i32,
    /// The item being enchanted (set by `RequestExTryToPutEnchantTargetItem`).
    pub item_oid: i32,
    /// The support item, if any (`0` = none). Support items are not yet wired.
    pub support_oid: i32,
    /// `_isProcessing` — set once `RequestEnchantItem` starts, to reject
    /// re-entrant packets mid-roll.
    pub processing: bool,
    /// Java `AbstractRequest._timestamp` — the tick of the **last window
    /// interaction** (add scroll / put target / put or remove support).
    /// `None` means the window was opened and never touched, which Java treats
    /// as cheating outright. Read only by the anti-autoenchant guard in
    /// `game_loop::enchant`.
    ///
    /// An `Option` rather than Java's `0` sentinel on purpose: Java compares
    /// wall-clock milliseconds, which are never 0, but **tick 0 is a real
    /// tick** — a server's first 100 ms would have read as "never stamped" and
    /// punished an honest player.
    pub stamped_tick: Option<u64>,
}

/// A player's active private *manufacture* store (Java `Player._manufactureItems`
/// + store title): the recipes they craft-for-hire and the adena fee each.
///   Present only while the store is open; not persisted (`StoreRecipeShopList =
/// False`). The store *type* byte (MANUFACTURE) lives on
///   [`Player::store_type`](crate::model::Player::store_type). `items` are
///   `(recipe_list_id, cost)`.
#[derive(Component, Debug, Clone, Default)]
pub struct ManufactureStore {
    pub items: Vec<(i32, i64)>,
    pub title: String,
}

/// The player's registered crafting recipes as recipe-*list* ids, split by
/// book (Java `Player._dwarvenRecipeBook` / `_commonRecipeBook`, keyed by
/// `RecipeList.getId()`). Loaded from `character_recipebook`, persisted in the
/// store transaction (the `type` column = dwarven/common, derived from
/// `RecipeData`). Player-only. Order is kept stable (Java uses a sorted map;
/// here insertion order — the wire packet carries a running 1-based slot index
/// the client keys buttons by, so consistency across resends is what matters).
#[derive(Component, Debug, Clone, Default)]
pub struct RecipeBook {
    pub dwarven: Vec<i32>,
    pub common: Vec<i32>,
}

impl RecipeBook {
    /// Whether either book holds this recipe-list id (Java `hasRecipeList`).
    pub fn contains(&self, list_id: i32) -> bool {
        self.dwarven.contains(&list_id) || self.common.contains(&list_id)
    }
}

/// The multisell list the player currently has open (Java
/// `Player._currentMultiSell` / `setMultiSell`), player-only. Presence-based:
/// added when a `MultiSellList` is sent, read/validated by `MultiSellChoose`,
/// removed on a stale/forged choose. The multipliers still come off the list
/// itself (the community-board path uses the default 1.0), but the two fields
/// `PreparedMultisellListHolder` derives *from the NPC* are latched here, so the
/// exchange charges exactly the rate the window displayed.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct ActiveMultisell {
    pub list_id: i32,
    /// Object id of the NPC the window was opened from (Java `_npcObjectId`),
    /// 0 for the npc-less community-board path. Tax is paid to its castle.
    pub npc_oid: i32,
    /// Java `PreparedMultisellListHolder.getTaxRate()` — already 0 for a list
    /// that doesn't `applyTaxes`, and for an NPC outside every tax zone.
    pub tax_rate: f64,
    /// The rows the window actually displayed, in order — Java's prepared
    /// `_entries` (+ the parallel `_itemInfos`). `MultiSellChoose`'s entry id
    /// indexes *this*, not the static list, which is what makes an
    /// inventory-only (`exc_multisell`) window addressable.
    pub rows: Vec<PreparedRow>,
}

/// One displayed multisell row: which entry of the static list it shows and,
/// for an inventory-only window, which of the player's item instances it was
/// paired with (Java `PreparedMultisellListHolder._itemInfos`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedRow {
    /// Index into `MultisellList.entries`.
    pub entry_index: usize,
    /// The paired inventory instance, `0` on a normal (non-inventory) window.
    pub item_object_id: i32,
    /// That instance's enchant level (0 when unpaired) — displayed in the
    /// window and echoed back by the client on the choose.
    pub enchant_level: i32,
}
