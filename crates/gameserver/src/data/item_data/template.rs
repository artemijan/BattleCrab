//! `ItemTemplate` — one row of the item catalogue — and the sub-structs it
//! carries: the equipped stat bonuses, the trade/drop flags, and the
//! `<capsuled_items>` list an extractable rolls against.

use super::SLOT_NONE;
use super::kinds::{ActionType, CrystalType, EtcItemType, ItemHandler, ItemKind};
use crate::data::item_cond::ItemCondition;
use crate::model::stats::Stat;

/// One `<capsuled_items><item .../></capsuled_items>` entry (Java
/// `ExtractableProduct`). `chance` is pre-scaled the same way Java's
/// constructor does (`(int) (chance * 1000)`), so it compares directly
/// against a `World::roll(100_000)` draw. `minEnchant`/`maxEnchant` are not
/// parsed — none of the currently-loaded extractable items set them, and
/// applying an enchant level to a freshly granted item needs an `Inventory`
/// setter that doesn't exist yet.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct CapsuledItem {
    pub item_id: i32,
    pub min: i64,
    pub max: i64,
    pub chance: i32,
}

/// Parsed `<stats>` block of an equipable item (Java `ItemTemplate`'s
/// `_funcTemplates`, all `FuncAdd`). Kept in a side-map on [`super::ItemData`] rather
/// than on [`ItemTemplate`] so the (many) template literals stay untouched.
/// The stats engine distinguishes two application rules when the item is worn
/// (see `Player::recalculate_stats`), matching the Java stat finalizers:
///   * **weapon-replace** (`calcWeaponBaseValue`): the equipped weapon's
///     `pAtk`/`mAtk`/`pAtkSpd`/`rCrit`/`mCritRate` value *replaces* the wearer's
///     naked class base before the STR/level multipliers apply;
///   * **sum-add** (`calcWeaponPlusBaseValue` / paperdoll loop): `pDef`/`mDef`/
///     `accCombat`/`accMagic`/`rEvas`/`mEvas`/`maxHp`/`maxMp` are summed across
///     every equipped piece and added on top of the computed base.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ItemStats {
    /// `<stat type="..">` entries mapped to an engine [`Stat`], in document
    /// order. Types the engine doesn't compute yet (elemental power/res,
    /// `sDef`, `rShld`, …) are dropped during parse.
    pub bonuses: Vec<(Stat, f64)>,
    /// `pAtkRange` — a weapon-only template constant (not a `Stat`); replaces
    /// `CombatStats.atk_range` while the weapon is equipped.
    pub atk_range: Option<i32>,
    /// `randomDamage` — weapon damage spread; replaces `CombatStats.random_dmg`
    /// (class templates all declare 10) while the weapon is equipped.
    pub random_damage: Option<i32>,
    /// `sDef` — a shield's block defence (added to the wearer's pDef on a
    /// successful shield block, Java `getShldDef`). Shield-only.
    pub shield_def: Option<i32>,
    /// `rShld` — a shield's base block *rate* (percent, before the CON bonus),
    /// Java `Stat.SHIELD_DEFENCE_RATE`. Shield-only.
    pub shield_rate: Option<i32>,
}

/// The datapack's per-item transfer restrictions (Java `ItemTemplate`'s
/// `_dropable` / `_tradeable` / `_destroyable` / `_depositable`). Every flag
/// defaults to **true** in Java — `set.getBoolean("is_dropable", true)` — so an
/// item is fully transferable unless its XML says otherwise; `Default` here
/// mirrors that, which also keeps `..Default::default()` test fixtures
/// permissive.
///
/// Time-limited reward boxes such as *Mage Class Equipment Set (10-day)*
/// (15195) declare all three of `is_tradable`/`is_dropable`/`is_sellable` as
/// `false`: they may only be used, warehoused or destroyed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TradeFlags {
    /// `<set name="is_dropable">` — may be dropped on the ground
    /// (`RequestDropItem`) and scattered by `Player.onDieDropItem`.
    pub dropable: bool,
    /// `<set name="is_tradable">` — may change owner: player trade, private
    /// store (buy and sell), mail attachment.
    pub tradable: bool,
    /// `<set name="is_destroyable">` — may be destroyed (`RequestDestroyItem`).
    pub destroyable: bool,
    /// `<set name="is_depositable">` — may be put in a warehouse. Java forces
    /// this to `!is_questitem` for quest items; for everything else it reads
    /// the tag (default true). Note `Item.isDepositable(isPrivateWareHouse)`
    /// only additionally demands tradability for the *clan* warehouse and
    /// freight — a private warehouse takes untradable items, which is why a
    /// bound reward box can still be stored.
    pub depositable: bool,
}

impl Default for TradeFlags {
    fn default() -> Self {
        Self {
            dropable: true,
            tradable: true,
            destroyable: true,
            depositable: true,
        }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ItemTemplate {
    pub item_id: i32,
    pub name: String,
    pub kind: ItemKind,
    /// `<set name="crystal_type"/>` — the item's grade, `None` when undeclared.
    /// Drives the expertise/grade penalty (`refresh_expertise_penalty`).
    pub crystal_type: CrystalType,
    /// `<set name="crystal_count"/>` — crystals yielded on crystallization (0 =
    /// not crystallizable).
    pub crystal_count: i32,
    /// `<set name="damage_range" val="0;0;radius;angle"/>` — the weapon's
    /// melee sweep geometry (Java `Weapon.getBaseAttackRadius/Angle`). A
    /// polearm reaches 66 with a 120° arc; a sword 40/120. Java's fallback when
    /// the tag is absent or malformed is radius 40, angle **0** — an angle of 0
    /// makes the multi-target sweep hit nothing, which is the intended
    /// "single-target weapon" behaviour.
    pub attack_radius: i32,
    pub attack_angle: i32,
    /// `<set name="mp_consume"/>` — MP a **ranged** weapon spends per shot
    /// (Short Bow 13 → 1). 0 for everything else.
    pub mp_consume: i32,
    /// `<set name="reduced_mp_consume"/>` + `reduced_mp_consume_chance` — some
    /// bows roll a cheaper shot. Absent on this dist's bows; ported for
    /// faithfulness (Java: `Rnd.get(100) < chance` swaps in the cheaper cost).
    pub reduced_mp_consume: i32,
    pub reduced_mp_consume_chance: i32,
    pub body_part: i32,
    pub weight: i32,
    pub is_stackable: bool,
    /// `<set name="is_infinite">` (Java `EtcItem._infinite`) — ammunition that
    /// is never spent. `PlayerInventory.reduceArrowCount` returns before the
    /// decrement for these, so an infinite quiver lasts forever. This dist
    /// ships 14 of them: arrows 32249-32255 and bolts 32256-32262.
    pub is_infinite: bool,
    pub type1: i32,
    pub type2: i32,
    pub is_quest_item: bool,
    /// `<set name="is_sellable">` (Java `ItemTemplate._sellable`, default
    /// **true**) — whether a merchant buys the item; gates both the sell-tab
    /// listing and `RequestSellItem`. Note the derived `Default` yields
    /// `false` — test fixtures built with `..Default::default()` are
    /// non-sellable unless they say otherwise.
    pub is_sellable: bool,
    /// `<set name="is_freightable">` (Java `ItemTemplate._freightable`, default
    /// **false**) — whether the item may be sent to another character on the
    /// account through the freight (`RequestPackageSend`). 3457 items declare
    /// it on this dist; everything else is refused.
    pub is_freightable: bool,
    /// `is_dropable` / `is_tradable` / `is_destroyable` / `is_depositable` —
    /// see [`TradeFlags`]. Grouped into a sub-struct so the derived `Default`
    /// on `ItemTemplate` yields Java's permissive defaults (everything allowed)
    /// instead of `false` (everything forbidden).
    pub trade_flags: TradeFlags,
    /// `<set name="time">` — lifetime in minutes for a time-limited item
    /// (Java `ItemTemplate.getTime()`, `-1`/absent = permanent). Only the
    /// "is this time-limited at all" question is modelled: expiry itself is not
    /// ported yet, but Java's `Player.onDieDropItem` refuses to scatter such
    /// items and that guard is honoured.
    /// SKIP(off-chronicle): actual expiry (`Item.scheduleLifeTimeTask`) is not
    /// ported, and this is a decision rather than a deferral. All **3230**
    /// items on this dist declaring a positive `time` sit in the 10015-47923
    /// band — every one post-Interlude content, the lowest being Prison Gate
    /// Key (10015). Nothing reachable on this chronicle expires, so the timer
    /// would be machinery with no consumer.
    ///
    /// Re-verified 2026-08-05 against the datapack, not against this prose:
    /// 3230 items, lowest id exactly 10015, none below it. Re-run that check
    /// before porting the timer — the claim is about the *data*, so new data
    /// is what would overturn it.
    pub time: i32,
    /// `<set name="duration">` — a **shadow item**'s starting mana, in
    /// minutes of wear (Java `ItemTemplate.getDuration()`, `-1`/absent =
    /// not a shadow item). Every freshly created instance starts at this
    /// value (Java's `Item` constructors: `_mana = _itemTemplate
    /// .getDuration()`), and `Item.isShadowItem()` is simply `mana >= 0`.
    /// 1353 items declare it on this dist, but inside the Interlude id range
    /// they are the 238 `Shadow Item: …` weapons (8821+, 90 or 300 minutes)
    /// plus the talismans; the rest are later-chronicle ids nothing here can
    /// hand out. See [`crate::game_loop::items::item_mana`].
    pub duration: i32,
    /// `<set name="price">` — the reference price (sell value = half of it;
    /// the `CorrectPrices` buylist floor uses it too). 0 when undeclared.
    pub price: i64,
    /// `<set name="handler">`, resolved to a typed dispatch target.
    pub handler: ItemHandler,
    /// `<capsuled_items>` children, in document order (`ExtractableItems`
    /// rolls each entry independently against its `chance`).
    pub capsuled_items: Vec<CapsuledItem>,
    /// `<set name="extractableCountMin">` — 0 (the common case) means "no
    /// minimum, one pass over the list is enough"; > 0 means keep re-rolling
    /// the whole list until at least this many distinct entries have hit
    /// (Java `ExtractableItems.useItem`'s `while` loop — used by "pick one of
    /// N" reward boxes).
    pub extractable_count_min: i32,
    /// `<set name="extractableCountMax">` — 0 (the common case) means "no
    /// cap, grant every entry that hits"; `ExtractableItems` stops rolling
    /// once this many entries have been granted.
    pub extractable_count_max: i32,
    /// `<skills><skill id=".." level=".." /></skills>` (Java
    /// `EtcItem._skills`, read by `ItemSkillsTemplate.useItem` via
    /// `getSkills(ItemSkillType.NORMAL)`) — `(skill_id, skill_level)` pairs,
    /// in document order.
    pub item_skills: Vec<(i32, i32)>,
    /// `<set name="etcitem_type">`, narrowed to enchant classification.
    pub etc_item_type: EtcItemType,
    /// `<set name="enchant_enabled">` (Java `_enchantable`) — whether this item
    /// can be enchanted at all.
    pub enchant_enabled: bool,
    /// `<set name="enchant_limit">` (Java `_enchantLimit`, 0 = no limit) — the
    /// cap an enchant scroll may not push past.
    pub enchant_limit: i32,
    /// `<set name="is_magic_weapon">` (Java `Weapon._isMagicWeapon`; false for
    /// non-weapons) — splits the fighter/mage enchant rate groups.
    pub is_magic_weapon: bool,
    /// `<set name="immediate_effect">` / `<set name="ex_immediate_effect">`
    /// (Java `ItemTemplate.hasImmediateEffect`/`hasExImmediateEffect`, both
    /// default false). Either one makes `ItemSkillsTemplate` fire the item's
    /// skills instantly instead of casting them; `immediate_effect` alone
    /// also feeds `checkConsume`.
    pub immediate_effect: bool,
    pub ex_immediate_effect: bool,
    /// `<set name="default_action">` — only the values `checkConsume`
    /// distinguishes are kept (see [`ActionType`]).
    pub default_action: ActionType,
    /// `<cond>` blocks, in document order (Java `ItemTemplate._preConditions`).
    /// ANDed at evaluation, each with its own refusal message — see
    /// [`crate::data::item_cond`].
    pub pre_conditions: Vec<ItemCondition>,
    /// `<set name="is_oly_restricted">` (Java `_isOlyRestricted`) — barred from
    /// an Olympiad match. Java ORs this with `Config.LIST_OLY_RESTRICTED_ITEMS`
    /// at the call site, which is empty on this dist.
    pub is_oly_restricted: bool,
    /// `<set name="is_event_restricted">` (Java `_isEventRestricted`) — barred
    /// while the holder is on an event.
    pub is_event_restricted: bool,
    /// `<set name="for_npc">` (Java `_forNpc`, 508 items here) — the item may
    /// be handed to a pet at all. `RequestPetUseItem` refuses anything else
    /// before it reaches the conditions.
    pub for_npc: bool,
}

#[cfg(test)]
impl ItemTemplate {
    /// A blank template carrying **Java's** field defaults, for test fixtures.
    ///
    /// The derived `Default` is a zero-fill, which disagrees with Java in four
    /// places. Every hand-written fixture in the tree used to correct them by
    /// spelling out all ~35 fields; build on this instead and list only what
    /// the test is actually about.
    ///
    /// | field | derived `Default` | Java |
    /// |---|---|---|
    /// | `time` / `duration` | `0` | `-1` (permanent) |
    /// | `attack_radius` | `0` | `40` (single-target melee reach) |
    /// | `is_sellable` | `false` | `true` |
    pub(crate) fn for_test() -> Self {
        Self {
            time: -1,
            duration: -1,
            attack_radius: 40,
            is_sellable: true,
            ..Default::default()
        }
    }
}

impl ItemTemplate {
    /// `ItemTemplate.isEquipable`: has a body part and isn't an `EtcItem`.
    pub fn is_equipable(&self) -> bool {
        self.kind != ItemKind::Etc && self.body_part != SLOT_NONE
    }

    /// `ItemTemplate.isEnchantable` — narrowed to the `enchant_enabled` flag
    /// (the `Config.ENCHANT_BLACKLIST` check isn't modelled).
    pub fn is_enchantable(&self) -> bool {
        self.enchant_enabled
    }

    /// `Item.isDropable` — droppable on the ground and by a death drop.
    /// (Java also refuses augmented / visual-transformed items; neither is a
    /// state this port carries on the ground-drop path.)
    pub fn is_dropable(&self) -> bool {
        self.trade_flags.dropable
    }

    /// `Item.isTradeable` — may change owner (trade, private store, mail).
    pub fn is_tradable(&self) -> bool {
        self.trade_flags.tradable
    }

    /// `Item.isDestroyable` — may be destroyed by the player.
    pub fn is_destroyable(&self) -> bool {
        self.trade_flags.destroyable
    }

    /// `Item.isDepositable(isPrivateWareHouse)` minus the equipped check (the
    /// caller owns that, since it needs the inventory). A private warehouse
    /// accepts any depositable item; the clan warehouse and freight also
    /// require tradability.
    pub fn is_depositable(&self, private_warehouse: bool) -> bool {
        self.trade_flags.depositable && (private_warehouse || self.is_tradable())
    }

    /// `Item.isTimeLimitedItem` — the template declares a `time` lifetime, so
    /// the item expires. Java refuses to scatter such items on death.
    pub fn is_time_limited(&self) -> bool {
        self.time > 0
    }

    /// What a merchant pays for one of these: half the reference price (Java
    /// `getReferencePrice() / 2`, spelled out at every use site there). Also
    /// the `Config.CORRECT_PRICES` floor a buy-list product may not undercut.
    ///
    /// Per **unit** — a stack costs `sell_price() * count`, halving before the
    /// multiply, which is what Java does and what the odd-priced items round
    /// against.
    pub fn sell_price(&self) -> i64 {
        self.price / 2
    }
}
