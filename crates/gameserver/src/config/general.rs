//! `General.ini` — port of the `GENERAL_CONFIG_FILE` block of `Config.java`,
//! grown per milestone like the other config sections.
//!
//! # Keys parsed by Java and deliberately given no field here
//!
//! Following `config::character`'s convention: a field would imply something
//! consults the key, so these are named in prose instead. Each was checked
//! against the Java tree rather than assumed.
//!
//! * **`StoryQuestRewardBuff`** — gates `Quest.giveStoryQuestReward`, a
//!   scripting API with **zero callers**: not one class in `java/` and not one
//!   script in the datapack invokes it. It is not dead the way a
//!   never-read `Config` field is — the method is a live entry point for
//!   datapack authors — but nothing on this chronicle is a story quest, so
//!   there is no reward for the buff to ride along with.
//! * **`LogAutoAnnouncements`** — assigned to a `Config` field that nothing
//!   outside `Config.java` reads.
//!
//! (`CustomTeleportTable` is dead in Java too but *does* have a field below,
//! decided when the loader cluster landed. The two treatments are not a
//! disagreement about the key: a field costs nothing and stops the next audit
//! re-deriving it, while prose keeps the unread count honest. Prefer prose.)

use commons::config::PropertiesParser;

/// `GlobalChat` / `TradeChat` — Java compares the raw string case-insensitively
/// at every use, so an unrecognised value simply matches no branch and the
/// channel goes quiet. [`ChatScope::Off`] is that state, named rather than left
/// as a stray string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChatScope {
    /// `ON` — the speaker's map region.
    #[default]
    Region,
    /// `GM` — the region, but only for a `CHAT_CONDITIONS` holder.
    GmOnly,
    /// `GLOBAL` — the whole server, behind the flood protector.
    Global,
    /// Anything else: no branch matches and nothing is sent.
    Off,
}

impl ChatScope {
    pub fn parse(raw: &str) -> Self {
        if raw.eq_ignore_ascii_case("on") {
            Self::Region
        } else if raw.eq_ignore_ascii_case("gm") {
            Self::GmOnly
        } else if raw.eq_ignore_ascii_case("global") {
            Self::Global
        } else {
            Self::Off
        }
    }
}

pub const GENERAL_CONFIG_FILE: &str = "config/General.ini";

/// The GM login-state settings applied in `EnterWorld.runImpl` plus the hero
/// aura toggle read by CharInfo/UserInfo (`Config.GM_*`).
#[derive(Debug, Clone, Default)]
pub struct GeneralConfig {
    /// `GMHeroAura`: give GMs the Hero glow on login (CharInfo/UserInfo hero
    /// byte = `isHero() || (isGM() && GMHeroAura)`).
    pub gm_hero_aura: bool,
    /// `GMStartupBuilderHide`: hide the GM on login (retail builder default).
    /// When set, Java **skips** the invul/invis/silence/diet block below.
    pub gm_startup_builder_hide: bool,
    /// `GMStartupInvulnerable`: auto-set invulnerable on login.
    pub gm_startup_invulnerable: bool,
    /// `GMStartupInvisible`: auto-set invisible on login (also applied at
    /// char-select, matching `CharacterSelect.java`).
    pub gm_startup_invisible: bool,
    /// `GMStartupSilence`: auto-enable silence (whisper block) on login.
    pub gm_startup_silence: bool,
    /// `GMStartupAutoList`: register the GM in the visible GM list on login
    /// (vs. hidden). Drives the `addGm` hidden flag.
    /// `AllowWear` (True here) — whether the try-on shop works at all.
    pub allow_wear: bool,
    /// `WearDelay` (5 s) — how long a previewed outfit stays on before the
    /// server tells the client to drop it.
    pub wear_delay: i32,
    /// `WearPrice` (10 adena) — charged **per previewed slot**, not per
    /// request.
    pub wear_price: i32,
    pub gm_startup_auto_list: bool,
    /// `GMStartupDietMode`: auto-enable diet mode (no weight overload) on login.
    pub gm_startup_diet_mode: bool,
    /// `GMGiveSpecialSkills` / `GMGiveSpecialAuraSkills`: grant the GM
    /// convenience kits at enter-world (`SkillTreeData.addSkills`). Read by
    /// `admin::flags`; the skills are session-only and filtered out of the
    /// persistence flush.
    pub gm_give_special_skills: bool,
    pub gm_give_special_aura_skills: bool,

    // --- Ground-item auto-destroy (`ItemsAutoDestroyTaskManager`) ---
    /// `AutoDestroyDroppedItemAfter` (seconds): how long a dropped ground item
    /// lies before auto-destroying. `0` = never (Java code default). Applies to
    /// NPC drops unconditionally and to player drops only when
    /// [`Self::destroy_dropped_player_item`] is set.
    pub autodestroy_item_after: u64,
    /// `AutoDestroyHerbTime` (seconds, **60** here): the same clock for herbs —
    /// items with `ex_immediate_effect`, which are consumed by walking over
    /// them. A tenth of the ordinary 600 s, because a battlefield would
    /// otherwise be carpeted in them. Gated **independently** of
    /// `AutoDestroyDroppedItemAfter`: Java schedules a herb whenever *this* is
    /// non-zero, even with the ordinary destroyer switched off.
    pub herb_auto_destroy_time: u64,
    /// `DestroyPlayerDroppedItem`: whether **player**-dropped items are subject
    /// to auto-destroy at all. Java default `false` (and the dist value) — so a
    /// player's drop persists until pickup/restart, unlike an NPC drop.
    pub destroy_dropped_player_item: bool,
    /// `DestroyEquipableItem`: extends [`Self::destroy_dropped_player_item`] to
    /// equipable player drops (weapons/armor). When false, only non-equipable
    /// player drops auto-destroy.
    pub destroy_equipable_player_item: bool,
    /// `ListOfProtectedItems`: item ids never auto-destroyed on the ground
    /// (dist ships `0`, a non-existent id ⇒ effectively empty).
    pub protected_items: Vec<i32>,
    /// `JailDisableTransaction` (dist `False`): whether a jailed character is
    /// barred from item transactions — `RequestDropItem` refuses while it is
    /// on. Java's own default is `false` too, so nothing changes unless an
    /// operator turns it on.
    pub jail_disable_transaction: bool,
    /// `JailDisableChat` (dist `True`): a jailed character without
    /// `PlayerCondOverride.CHAT_CONDITIONS` is refused chat.
    ///
    /// **Only the world-chat branch consumes this today.** Java gates two
    /// places on it: `ChatWorld.handleChat` (ported) and `Say2`'s own guard
    /// over WHISPER/SHOUT/TRADE/HERO_VOICE, which this port has never had —
    /// ported 2026-08-07 alongside the olympiad gate beside it, so both
    /// call sites now consume this key.
    pub jail_disable_chat: bool,

    /// `AllowWater` (dist `True`): whether swimming can drown you. Java gates
    /// only `Player.checkWaterState()` on it inside `revalidateZone` — the
    /// swim-speed switch in `WaterZone.onEnter` is unconditional, so turning
    /// this off makes water slow but harmless, not inert.
    pub allow_water: bool,

    // --- Quests ---
    /// `OrderQuestListByQuestId` (dist **True**) — sort the NPC's quest-choice
    /// window by quest id. Java builds a `TreeMap` keyed by id, so it also
    /// **de-duplicates**: two scripts sharing an id would collapse to one.
    pub order_quest_list_by_quest_id: bool,
    /// `AutoDeleteInvalidQuestData` (dist **False**) — what to do with a
    /// `character_quests` row naming a quest the server no longer has.
    ///
    /// Java drops it from memory either way (`q == null` → `continue`); the key
    /// only decides whether the **row** is also deleted. The port used to keep
    /// such rows in the live component and write them straight back, so a
    /// renamed or removed quest left a phantom `QuestState` that outlived every
    /// restart.
    pub auto_delete_invalid_quest_data: bool,
    /// `AltDevNoQuests` (dist **False**) — a developer switch that starts the
    /// server with **no quests registered at all**. Java skips the whole quest
    /// half of `ScriptEngineManager.executeScriptList`.
    pub alt_dev_no_quests: bool,
    /// `AltDevShowQuestsLoadInLogs` (dist **False**) — log every quest as
    /// `QuestManager` registers it, rather than only the total.
    pub alt_dev_show_quests_load_in_logs: bool,

    // --- Feature gates: subsystems an operator can switch off wholesale ---
    /// `AllowWarehouse` (dist **True**) — the private and clan warehouse
    /// bypasses (`WithdrawP`/`DepositP`/`WithdrawC`/`DepositC`). Java refuses
    /// the *bypass* rather than hiding the button, so with it off the keeper
    /// still offers the link and it does nothing.
    pub allow_warehouse: bool,
    /// `AllowRefund` (dist **True**) — the merchant refund tab. Two Java sites:
    /// `RequestSellItem` only files the sold stack into the refund list when
    /// this is on, and `Player.hasRefund()` gates the tab itself.
    pub allow_refund: bool,
    /// `AllowFishing` (dist **True**) — gates *casting*, and Java pairs it with
    /// a `ZONE_CONDITIONS` override so a GM can still fish with it off.
    pub allow_fishing: bool,
    /// `AllowBoat` (dist **True**) — whether `BoatManager` loads its docks at
    /// all. Off means no boats exist, not that they stop moving.
    pub allow_boat: bool,
    /// `BoatBroadcastRadius` (dist **20000**) — how near a dock a player must
    /// be to receive a boat's departure/arrival packets.
    pub boat_broadcast_radius: i32,
    /// `AllowCursedWeapons` (dist **True**) — whether `CursedWeaponsManager`
    /// loads. Off disables the whole subsystem: no drops, no transfers.
    pub allow_cursed_weapons: bool,
    /// `AllowDiscardItem` (dist **True**) — whether `RequestDropItem` works at
    /// all, exempting a `PlayerCondOverride.DROP_ALL_ITEMS` holder.
    pub allow_discard_item: bool,
    /// `TradeChat` (dist **`ON`**) and `GlobalChat` (dist **`ON`**) — where
    /// Trade and Shout go. Three values each, and they are not on/off:
    ///
    /// * `on` — the speaker's **map region** only.
    /// * `gm` — the same, but only for a `CHAT_CONDITIONS` holder; everyone
    ///   else falls through to the `global` test and, failing that, is dropped
    ///   silently.
    /// * `global` — the whole server, behind the global-chat flood protector.
    ///
    /// So "off" is spelled by setting something that matches no branch, and the
    /// line then vanishes with no message — Java's own shape.
    pub trade_chat: ChatScope,
    pub global_chat: ChatScope,
    /// `MinimumChatLevel` (dist **0**) — the level below which General, Shout
    /// and Whisper are refused, each with its own system message. A
    /// `CHAT_CONDITIONS` holder is exempt. Inert at 0.
    pub minimum_chat_level: i32,

    // --- Datapack `custom/` overlays and the HTML loader ---
    /// `CustomNpcData` (dist **True**) — also parse `stats/npcs/custom/`.
    ///
    /// The port read that directory nowhere, so 14 templates were missing —
    /// including the TvT event manager, which made `//event_start TvT` spawn
    /// no NPC at all.
    pub custom_npc_data: bool,
    /// `CustomSkillsLoad` (dist **True**) — `stats/skills/custom/`, one file
    /// here (`tvt_event.xml`, Ghost Walking 100000).
    pub custom_skills_load: bool,
    /// `CustomItemsLoad` (dist **True**) — `stats/items/custom/`, which this
    /// dist does not ship. Inert, and wired anyway.
    pub custom_items_load: bool,
    /// `CustomMultisellLoad` (dist **True**) — `multisell/custom/`, the
    /// `6000xx` community-board shop lists.
    pub custom_multisell_load: bool,
    /// `CustomBuyListLoad` (dist **True**) — `buylists/custom/`, the 143
    /// GM-shop lists.
    pub custom_buylist_load: bool,
    /// `CustomTeleportTable` (dist **True**) — **dead in Java**: `Config`
    /// parses it into `CUSTOM_TELEPORT_TABLE` and nothing anywhere reads that
    /// field. Given a field here so the key is accounted for and the next
    /// audit does not re-derive it; deliberately unused, like the eight dead
    /// `Character.ini` keys named in `config::character`'s header.
    pub custom_teleport_table: bool,
    /// `HtmCache` (dist **False**, Java default `true`) — whether the whole
    /// `html/` tree is parsed into memory at boot.
    ///
    /// **False is the lazy branch, and lazy is exactly what this port does.**
    /// `data::htm_cache`'s header used to frame per-interaction reading as a
    /// deliberate deviation from Java, which is true only against
    /// `HtmCache = True`; on this dist Java logs *"Cache[HTML]: Running lazy
    /// cache"* and reads the same way. So the port already implements the
    /// configured branch, and the field exists to say which branch that is —
    /// the eager one is not implemented and would change more than caching
    /// (`Npc.getHtmlPath` treats the cache as the existence oracle, so a file
    /// added after boot becomes invisible).
    pub htm_cache: bool,
    /// `CheckHtmlEncoding` (dist **True**) — warn at load when an html file is
    /// not pure ASCII. Diagnostics only; Java exempts `data/lang`.
    pub check_html_encoding: bool,
    /// `HideBypassRemoval` (dist **True**) — strip the `-h` flag from three
    /// specific bypasses as the file is read, making those links visible in
    /// the chat box instead of hidden.
    ///
    /// Safe to apply to content because the **client** consumes the flag: it
    /// strips `bypass `/`bypass -h ` before sending, and Java's
    /// `RequestBypassToServer` never sees a `-h`. So this changes what the
    /// player's chat shows, not what the server parses.
    pub hide_bypass_removal: bool,
    /// `HtmlActionCacheDebug` (dist **False**) — verbose logging inside Java's
    /// `Util` html-action cache.
    ///
    /// **No consumer: the port does not implement that cache.**
    /// `validateHtmlAction` — the registry of which bypasses a player was
    /// actually sent — is a recorded deviation in `game_loop::bypass`, which
    /// re-checks interaction distance on every route instead. There is no
    /// cache for this key to trace.
    pub html_action_cache_debug: bool,

    // --- Ground-item persistence and the item write path ---
    /// `SaveDroppedItem` (dist **False**, Java default `false`) — persist items
    /// lying on the ground to `itemsonground`, so a restart does not swallow
    /// them. Off here, which is why the table has sat empty since the baseline
    /// migration created it.
    pub save_dropped_item: bool,
    /// `SaveDroppedItemInterval` (dist **60**, Java default 60) — **minutes**
    /// between full rewrites of that table. Java multiplies by 60000 at parse
    /// time; the port keeps minutes here and converts at the scheduler, so the
    /// unit in the field matches the unit in the ini.
    pub save_dropped_item_interval_minutes: i32,
    /// `EmptyDroppedItemTableAfterLoad` (dist **False**) — truncate
    /// `itemsonground` immediately after loading it, so the rows are consumed
    /// exactly once.
    pub empty_dropped_item_table_after_load: bool,
    /// `ClearDroppedItemTable` (dist **False**) — truncate at boot when
    /// [`Self::save_dropped_item`] is **off**. Java's comment: *"may want to
    /// delete all items previously stored to avoid add old items on
    /// reactivate"* — it stops a table written during an earlier
    /// `SaveDroppedItem = True` era from resurrecting when the key is turned
    /// back on.
    pub clear_dropped_item_table: bool,
    /// `DestroyAllItems` (dist **False**, Java default `false`) — when **on**,
    /// `RequestDestroyItem` skips its whole refusal gate: undestroyable items
    /// and cursed weapons alike can be deleted. Off here, so the gate applies,
    /// and `PlayerCondOverride.DESTROY_ALL_ITEMS` exempts a holder from the
    /// undestroyable half **but not** from the cursed-weapon half.
    pub destroy_all_items: bool,
    /// `MultipleItemDrop` (dist **True**, Java default `true`) — a
    /// non-stackable item added in quantity becomes *N instances of 1* rather
    /// than one instance of N.
    ///
    /// Java's loop `break`s early when this is off, which does **not** produce
    /// one instance of N — it produces **one instance of 1, silently dropping
    /// the rest**. Ported as written; see `items::inventory`.
    pub multiple_item_drop: bool,
    /// `UpdateItemsOnCharStore` (dist **True**, Java default `false`) — whether
    /// the periodic character save also writes the inventory, warehouse and
    /// freight. The port's save is a single transaction over all of it, so this
    /// selects whether the item half is included.
    pub update_items_on_char_store: bool,
    /// `DatabaseCleanUp` (dist **True**, Java default `true`) — delete orphaned
    /// rows at boot: every child table whose owning character, clan, item or
    /// forum is gone. Java runs 50 statements over 43 tables in `IdManager`;
    /// all 43 exist in this schema.
    pub database_clean_up: bool,
    /// `LazyItemsUpdate` (dist **False**, Java default `false`) — in Java,
    /// whether an item row is written on *every* change or only when something
    /// forces it.
    ///
    /// **No consumer here, and it cannot have one as things stand.** The port
    /// is memory-first by design: item state lives in components and reaches
    /// the database through the periodic flush and the logout store, so there
    /// is no per-change write for this key to make lazy. Turning it on in Java
    /// makes that engine behave more like this one; turning it off cannot make
    /// this one behave like that. Carried so the key is accounted for rather
    /// than looking unexamined.
    pub lazy_items_update: bool,
    /// `ClanVariablesStoreInterval` (dist **15** minutes) — how often
    /// `clan_variables` is flushed.
    ///
    /// **No consumer: nothing writes a clan variable on this chronicle.** The
    /// only keys Java ever stores there are `MAX_ONLINE_MEMBERS`,
    /// `HUNTING_POINTS` and their `PREVIOUS_*` twins, all owned by the clan
    /// **reward** system (`ClanReward.xml`, Clan Unity 55168) — post-Interlude
    /// content this port does not implement. The table ships and is empty, like
    /// `itemsonground` was.
    pub clan_variables_store_interval_minutes: i32,

    /// `GMSkillRestriction` (dist **True**, Java default `false`) — whether a
    /// character holding `PlayerCondOverride.SKILL_CONDITIONS` is **still**
    /// bound by a skill's conditions.
    ///
    /// Java: `if (isFakePlayer() || (canOverrideCond(SKILL_CONDITIONS) &&
    /// !Config.GM_SKILL_RESTRICTION)) return true;` — so **True, the dist
    /// value, restricts**: the override stops exempting them. The port used to
    /// let every GM skip every skill condition unconditionally, behind a
    /// comment asserting this key was off.
    pub gm_skill_restriction: bool,
    /// `GMItemRestriction` (dist **True**, Java default `false`) — the same
    /// shape for `ItemTemplate.checkCondition` and
    /// `PlayerCondOverride.ITEM_CONDITIONS`.
    ///
    /// **No consumer here, and it is not a wiring oversight**: the port does
    /// not evaluate item conditions at all — `<cond>` is unparsed
    /// (`data::item_data`'s module header records it), and the Olympiad and
    /// event item restrictions are likewise absent. There is nothing for the
    /// override to bypass and therefore nothing for this key to re-restrict.
    /// Carried as a field anyway so the gate lands with the conditions
    /// whenever they are ported, rather than being rediscovered then.
    pub gm_item_restriction: bool,
    /// `GMTradeRestrictedItems` (dist **False**, Java default `false`) —
    /// whether an override-holder may drop, trade and store items the datapack
    /// marks untradeable or quest-bound. Four Java sites read it: the drop
    /// gate, the quest-item drop gate, `TradeStart`'s item list and
    /// `TradeList.addItem`.
    pub gm_trade_restricted_items: bool,
    /// `GMRestartFighting` (dist **True**, Java default `true`) — whether a GM
    /// may restart or log out while the attack stance is up
    /// (`Player.canLogout`). The one key in this family that *grants* rather
    /// than restricts.
    pub gm_restart_fighting: bool,
    /// `GMShowAnnouncerName` (dist **False**, Java default `false`) — append
    /// ` [GmName]` to a `//announce`. `//announce_screen` is deliberately
    /// exempt in Java, and so is the port.
    pub gm_announcer_name: bool,
    /// `GMDebugHtmlPaths` (dist **True**, Java default `true`) — send a GM the
    /// path of every HTML the server serves them (`HtmCache.getHtm`), as a
    /// plain chat line. A GM tool: it is how you find which file a dialog came
    /// from without grepping the datapack.
    pub gm_debug_html_paths: bool,
    /// `UseSuperHasteAsGMSpeed` (dist **False**, Java default `false`) —
    /// `//gmspeed <n>` forwards to `//superhaste <n>` instead of applying a
    /// run-speed multiplier. Java calls it a "rollback feature for old custom
    /// way, in order to make everyone happy".
    pub use_super_haste_as_gm_speed: bool,
    /// `DefaultAccessLevel` (dist **0**, Java default `0`) — the access level a
    /// character resolving to 0 is promoted to. Java applies it in
    /// `Player.setAccessLevel` behind a `> 0` guard, so **0 means "no
    /// promotion"** rather than "promote everyone to 0"; an operator setting it
    /// to a GM tier makes every character on the server a GM, which is why the
    /// guard is the whole feature.
    pub default_access_level: i32,
    /// `OnlyGMItemsFree` (dist **True**, Java default `true`) — a buy-list row
    /// priced at 0 is refused (and punished) for anyone who is not a GM. The
    /// port had this hard-coded to the dist's behaviour behind a comment
    /// naming the key; the field is what makes an operator's `False` mean
    /// something.
    pub only_gm_items_free: bool,
    /// `ServerGMOnly` (dist **False**, Java default `false`) — whether the
    /// server registers with the login server as GM-only at startup, and what
    /// the status returns to after a cancelled shutdown. `//server_gm_only` /
    /// `//server_all` flip it at runtime, in Java by assigning the config field
    /// itself.
    pub server_gm_only: bool,

    /// `SkillCheckEnable` (dist **True**, Java default `false`) — whether
    /// `restoreSkills` validates every row it reads out of `character_skills`
    /// against the skill trees. A row that fails is an illegal skill: it was
    /// never learnable by this class, so it arrived through a bug, a
    /// hand-edited database or an exploit.
    pub skill_check_enable: bool,
    /// `SkillCheckRemove` (dist **True**, Java default `false`) — whether a
    /// failing row is *removed* as well as reported. With it off the check is
    /// a pure audit.
    pub skill_check_remove: bool,
    /// `SkillCheckGM` (dist **False**, Java default `true`) — whether the check
    /// also applies to a character holding `PlayerCondOverride.SKILL_CONDITIONS`.
    /// Java's guard reads
    /// `(!canOverrideCond(SKILL_CONDITIONS) || Config.SKILL_CHECK_GM)`, so
    /// **False here exempts** such a character. Note the inversion: the key
    /// reads like "check GMs too", and that is exactly what it means — off, so
    /// they are skipped.
    pub skill_check_gm: bool,

    /// `EnableFallingDamage` (dist `True`) — whether a fall costs HP.
    /// Java reads it in exactly one place, `Formulas.calcFallDam`, which
    /// returns 0 when it is off; `Player.isFalling` still runs, so the
    /// position-validation suppression and the client re-grounding survive a
    /// server with damage disabled. Java's code default is `true`.
    pub enable_falling_damage: bool,

    /// `AllowManor`: whether the castle manor (seed sowing / crop harvest) runs.
    /// The dist ships `False`; the manor data + packets exist regardless so the
    /// feature works the moment an operator enables it.
    pub allow_manor: bool,
    /// `AltManorSaveAllActions`: persist every manor setup change immediately.
    /// `False` on this dist, in which case the setup is written by the periodic
    /// autosave every [`alt_manor_save_period_rate`] hours and again on
    /// shutdown — exactly Java's two branches in `CastleManorManager.load` and
    /// `Shutdown`.
    ///
    /// [`alt_manor_save_period_rate`]: General::alt_manor_save_period_rate
    pub alt_manor_save_all_actions: bool,
    /// `AltManorSavePeriodRate` (hours, dist 2) — the autosave interval used
    /// when `AltManorSaveAllActions` is off. Java schedules it at that rate
    /// both for the initial delay and the period.
    pub alt_manor_save_period_rate: i32,
    /// `AltManorRefreshTime` (hour, dist 20) — when the daily manor cycle rolls:
    /// the `APPROVED → MAINTENANCE` change (production rollover) fires at
    /// `refresh_time:refresh_min`.
    pub alt_manor_refresh_time: i32,
    /// `AltManorRefreshMin` (minute, dist 0).
    pub alt_manor_refresh_min: i32,
    /// `AltManorMaintenanceMin` (dist 6) — the maintenance window length; the
    /// `MAINTENANCE → MODIFIABLE` change fires at
    /// `refresh_time:(refresh_min + maintenance_min)`.
    pub alt_manor_maintenance_min: i32,
    /// `AltManorApproveTime` (hour, dist 4) — when the owner's edit window
    /// closes: the `MODIFIABLE → APPROVED` change fires at
    /// `approve_time:approve_min`.
    pub alt_manor_approve_time: i32,
    /// `AltManorApproveMin` (minute, dist 30).
    pub alt_manor_approve_min: i32,

    /// `AllowMail`: whether the mail/post system is available (G30). Dist and
    /// Java default both `True`.
    pub allow_mail: bool,
    /// `AllowAttachments`: whether mail may carry items and COD (G30). Java
    /// still delivers the *message* when this is off — it just strips the
    /// attachments and the payment request.
    pub allow_attachments: bool,

    /// `AllowLottery`: whether the weekly Lucky Lottery runs (G26.5). Dist ships
    /// `False`; the round engine + persistence exist regardless so it works the
    /// moment an operator enables it.
    pub allow_lottery: bool,
    /// `AltLotteryPrize` (dist 50000): the starting jackpot of a fresh round.
    pub alt_lottery_prize: i64,
    /// `AltLotteryTicketPrice` (dist 2000): adena charged per ticket.
    pub alt_lottery_ticket_price: i64,
    /// `AltLottery5NumberRate` (dist 0.6): share of the pot paid to 5-match
    /// (first-prize) winners.
    pub alt_lottery_5_number_rate: f64,
    /// `AltLottery4NumberRate` (dist 0.2): share paid to 4-match winners.
    pub alt_lottery_4_number_rate: f64,
    /// `AltLottery3NumberRate` (dist 0.2): share paid to 3-match winners.
    pub alt_lottery_3_number_rate: f64,
    /// `AltLottery2and1NumberPrize` (dist 200): flat adena for a 2-or-1 match.
    pub alt_lottery_2and1_number_prize: i64,

    /// `AllowRace`: whether the Monster Race Track runs (G26.5). Dist ships
    /// `False`; the race engine exists regardless so an operator can enable it.
    pub allow_race: bool,

    /// `AltItemAuctionEnabled`: whether the item-auction house runs (G30.5).
    /// Dist ships `True`, but `ItemAuctions.xml` is empty, so nothing auctions
    /// until an operator adds instances.
    pub alt_item_auction_enabled: bool,
    /// `AltItemAuctionExpiredAfter` (days, dist 14): how long a finished auction
    /// + its bids linger before cleanup / after which a bid can't be canceled.
    pub alt_item_auction_expired_after_days: i32,
    /// `AltItemAuctionTimeExtendsOnBid` (seconds, dist 0): the extra
    /// bid-driven ending extension past the built-in 5-/3-minute phases. `0`
    /// disables the config phases.
    pub alt_item_auction_time_extends_on_bid: i64,

    // --- World chat ---------------------------------------------------------
    // `handlers/chathandlers/ChatWorld.java`. Enabled on this dist. See the
    // chronicle caveat on [`crate::enums::ChatType::World`].
    /// `WorldChatEnabled` (dist `True`): the master gate. With it off
    /// `ChatWorld.handleChat` returns immediately — **silently**, with no
    /// notice to the speaker — and the daily point reset is skipped too.
    pub world_chat_enabled: bool,
    /// `WorldChatMinLevel` (Java default 95, dist **40**): below this the
    /// speaker is told `YOU_CAN_USE_WORLD_CHAT_FROM_LV_S1` and the line is
    /// dropped. Also zeroes the count `ExWorldChatCnt` reports.
    pub world_chat_min_level: i32,
    /// `WorldChatPointsPerDay` (dist 10): the daily quota, before the
    /// `Stat.WORLD_CHAT_POINTS` add/mul Java folds in. No skill or item on this
    /// dist grants that stat, so the config value is the whole quota here.
    pub world_chat_points_per_day: i32,
    /// `WorldChatInterval` (dist `20secs`) as **seconds**: the per-speaker
    /// reuse window. `0` disables the window entirely — Java guards both the
    /// check and the stamp with `getSeconds() > 0`.
    pub world_chat_interval_secs: i64,

    // --- Audit-record gates ------------------------------------------------
    // Which categories the never-dropped audit sink (`commons::audit`) records.
    // All ship `False`: these are operator decisions about retention and disk,
    // not features. The sink itself is always running — the gate only decides
    // whether a given category produces records.
    /// `LogChat`: record public and private chat (Java: `Say2`,
    /// `RequestSendFriendMsg`).
    pub log_chat: bool,
    /// `LogItems`: record item ownership and count changes.
    pub log_items: bool,
    /// `LogItemsSmallLog`: when [`Self::log_items`] is on, narrow it to adena
    /// and equippable items. Java treats this as an *override* rather than a
    /// filter — with it set, those items are recorded even though the broad
    /// branch is skipped.
    pub log_items_small_log: bool,
    /// `LogItemsIdsOnly`: narrow item records to [`Self::log_items_ids_list`],
    /// with the same override semantics as the small log.
    pub log_items_ids_only: bool,
    /// `LogItemsIdsList`: the ids [`Self::log_items_ids_only`] admits.
    pub log_items_ids_list: Vec<i32>,
    /// `LogItemEnchants`: record item enchant attempts and their outcome.
    pub log_item_enchants: bool,
    /// `LogSkillEnchants`: record skill enchant attempts and their outcome.
    pub log_skill_enchants: bool,
    /// `GMAudit`: record every GM command, its target and its arguments.
    pub gm_audit: bool,
    /// `DefaultPunish`: what happens to a player caught by a packet-validation
    /// guard (Java `Util.handleIllegalPlayerAction`). The dist ships `KICK`.
    pub default_punish: crate::model::punishment::IllegalActionPunishment,
    /// `DefaultPunishParam`: ban/jail duration in **seconds** for `KICKBAN` /
    /// `JAIL`. The dist ships `0`, which Java folds to one hundred years.
    pub default_punish_param: i64,
}

impl GeneralConfig {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(root, GENERAL_CONFIG_FILE))
    }

    /// Parse from an already-loaded `General.ini` (split out so tests can point
    /// at the real `dist/game` file without depending on the process cwd).
    fn from_parser(p: &PropertiesParser) -> Self {
        let d = Self::default();
        Self {
            gm_hero_aura: p.get_bool("GMHeroAura", d.gm_hero_aura),
            gm_startup_builder_hide: p.get_bool("GMStartupBuilderHide", d.gm_startup_builder_hide),
            gm_startup_invulnerable: p.get_bool("GMStartupInvulnerable", d.gm_startup_invulnerable),
            gm_startup_invisible: p.get_bool("GMStartupInvisible", d.gm_startup_invisible),
            gm_startup_silence: p.get_bool("GMStartupSilence", d.gm_startup_silence),
            allow_wear: p.get_bool("AllowWear", true),
            wear_delay: p.get_int("WearDelay", 5),
            wear_price: p.get_int("WearPrice", 10),
            gm_startup_auto_list: p.get_bool("GMStartupAutoList", d.gm_startup_auto_list),
            gm_startup_diet_mode: p.get_bool("GMStartupDietMode", d.gm_startup_diet_mode),
            gm_give_special_skills: p.get_bool("GMGiveSpecialSkills", d.gm_give_special_skills),
            gm_give_special_aura_skills: p
                .get_bool("GMGiveSpecialAuraSkills", d.gm_give_special_aura_skills),
            autodestroy_item_after: p.get_int("AutoDestroyDroppedItemAfter", 0).max(0) as u64,
            herb_auto_destroy_time: p.get_int("AutoDestroyHerbTime", 60).max(0) as u64,
            destroy_dropped_player_item: p
                .get_bool("DestroyPlayerDroppedItem", d.destroy_dropped_player_item),
            destroy_equipable_player_item: p
                .get_bool("DestroyEquipableItem", d.destroy_equipable_player_item),
            // Java parses a comma-separated id list; the dist ships a lone `0`.
            protected_items: p
                .get_string("ListOfProtectedItems", "")
                .split(',')
                .filter_map(|s| s.trim().parse::<i32>().ok())
                .collect(),
            jail_disable_transaction: p
                .get_bool("JailDisableTransaction", d.jail_disable_transaction),
            // Java's code default is `true`, not the derived `Default`'s false.
            jail_disable_chat: p.get_bool("JailDisableChat", true),
            // Java's code default is `true` (not `d.allow_water`, which the
            // derived `Default` would make `false` — the opposite meaning).
            allow_water: p.get_bool("AllowWater", true),
            // Java's code default is `true` — same trap as `allow_water`: the
            // derived `Default` would be `false`, which is the opposite.
            enable_falling_damage: p.get_bool("EnableFallingDamage", true),
            // Java's code defaults, which the dist inverts on all three:
            // `false`/`false`/`true` there, `True`/`True`/`False` here — so
            // this dist checks every character, removes what fails, and
            // exempts a `SKILL_CONDITIONS` override.
            // The GM-restriction family. Java's code defaults are all
            // `false`; this dist raises three of them.
            order_quest_list_by_quest_id: p.get_bool("OrderQuestListByQuestId", true),
            auto_delete_invalid_quest_data: p.get_bool("AutoDeleteInvalidQuestData", false),
            alt_dev_no_quests: p.get_bool("AltDevNoQuests", false),
            alt_dev_show_quests_load_in_logs: p.get_bool("AltDevShowQuestsLoadInLogs", false),
            allow_warehouse: p.get_bool("AllowWarehouse", true),
            allow_refund: p.get_bool("AllowRefund", true),
            allow_fishing: p.get_bool("AllowFishing", true),
            allow_boat: p.get_bool("AllowBoat", true),
            boat_broadcast_radius: p.get_int("BoatBroadcastRadius", 20000),
            allow_cursed_weapons: p.get_bool("AllowCursedWeapons", true),
            allow_discard_item: p.get_bool("AllowDiscardItem", true),
            trade_chat: ChatScope::parse(&p.get_string("TradeChat", "ON")),
            global_chat: ChatScope::parse(&p.get_string("GlobalChat", "ON")),
            minimum_chat_level: p.get_int("MinimumChatLevel", 0),
            custom_npc_data: p.get_bool("CustomNpcData", false),
            custom_skills_load: p.get_bool("CustomSkillsLoad", false),
            custom_items_load: p.get_bool("CustomItemsLoad", false),
            custom_multisell_load: p.get_bool("CustomMultisellLoad", false),
            custom_buylist_load: p.get_bool("CustomBuyListLoad", false),
            custom_teleport_table: p.get_bool("CustomTeleportTable", false),
            // Java's code defaults are `true` for these three.
            htm_cache: p.get_bool("HtmCache", true),
            check_html_encoding: p.get_bool("CheckHtmlEncoding", true),
            hide_bypass_removal: p.get_bool("HideBypassRemoval", true),
            html_action_cache_debug: p.get_bool("HtmlActionCacheDebug", false),
            save_dropped_item: p.get_bool("SaveDroppedItem", false),
            // Java stores this pre-multiplied into ms; kept in the ini's own
            // unit here and converted where it is scheduled.
            save_dropped_item_interval_minutes: p.get_int("SaveDroppedItemInterval", 60),
            empty_dropped_item_table_after_load: p
                .get_bool("EmptyDroppedItemTableAfterLoad", false),
            clear_dropped_item_table: p.get_bool("ClearDroppedItemTable", false),
            destroy_all_items: p.get_bool("DestroyAllItems", false),
            // Java's code default is `true` — the derived `Default` would be
            // `false`, which is the silently-lossy branch.
            multiple_item_drop: p.get_bool("MultipleItemDrop", true),
            update_items_on_char_store: p.get_bool("UpdateItemsOnCharStore", false),
            database_clean_up: p.get_bool("DatabaseCleanUp", true),
            lazy_items_update: p.get_bool("LazyItemsUpdate", false),
            clan_variables_store_interval_minutes: p.get_int("ClanVariablesStoreInterval", 15),
            gm_skill_restriction: p.get_bool("GMSkillRestriction", false),
            gm_item_restriction: p.get_bool("GMItemRestriction", false),
            gm_trade_restricted_items: p.get_bool("GMTradeRestrictedItems", false),
            // …except this one, whose Java default is `true`.
            gm_restart_fighting: p.get_bool("GMRestartFighting", true),
            gm_announcer_name: p.get_bool("GMShowAnnouncerName", false),
            gm_debug_html_paths: p.get_bool("GMDebugHtmlPaths", true),
            use_super_haste_as_gm_speed: p.get_bool("UseSuperHasteAsGMSpeed", false),
            server_gm_only: p.get_bool("ServerGMOnly", false),
            only_gm_items_free: p.get_bool("OnlyGMItemsFree", true),
            default_access_level: p.get_int("DefaultAccessLevel", 0),
            skill_check_enable: p.get_bool("SkillCheckEnable", false),
            skill_check_remove: p.get_bool("SkillCheckRemove", false),
            skill_check_gm: p.get_bool("SkillCheckGM", true),
            allow_manor: p.get_bool("AllowManor", d.allow_manor),
            alt_manor_save_all_actions: p
                .get_bool("AltManorSaveAllActions", d.alt_manor_save_all_actions),
            alt_manor_save_period_rate: p.get_int("AltManorSavePeriodRate", 2),
            alt_manor_refresh_time: p.get_int("AltManorRefreshTime", 20),
            alt_manor_refresh_min: p.get_int("AltManorRefreshMin", 0),
            alt_manor_maintenance_min: p.get_int("AltManorMaintenanceMin", 6),
            alt_manor_approve_time: p.get_int("AltManorApproveTime", 4),
            alt_manor_approve_min: p.get_int("AltManorApproveMin", 30),
            allow_mail: p.get_bool("AllowMail", true),
            allow_attachments: p.get_bool("AllowAttachments", true),
            allow_lottery: p.get_bool("AllowLottery", d.allow_lottery),
            alt_lottery_prize: p.get_int("AltLotteryPrize", 50000) as i64,
            alt_lottery_ticket_price: p.get_int("AltLotteryTicketPrice", 2000) as i64,
            alt_lottery_5_number_rate: p.get_float("AltLottery5NumberRate", 0.6) as f64,
            alt_lottery_4_number_rate: p.get_float("AltLottery4NumberRate", 0.2) as f64,
            alt_lottery_3_number_rate: p.get_float("AltLottery3NumberRate", 0.2) as f64,
            alt_lottery_2and1_number_prize: p.get_int("AltLottery2and1NumberPrize", 200) as i64,
            allow_race: p.get_bool("AllowRace", d.allow_race),
            alt_item_auction_enabled: p
                .get_bool("AltItemAuctionEnabled", d.alt_item_auction_enabled),
            alt_item_auction_expired_after_days: p.get_int("AltItemAuctionExpiredAfter", 14),
            // Java parses seconds then converts to millis for the extend amount.
            alt_item_auction_time_extends_on_bid: p.get_int("AltItemAuctionTimeExtendsOnBid", 0)
                as i64
                * 1000,
            // Java's code defaults, not `d.*`: the derived `Default` would make
            // `world_chat_enabled` false, inverting the meaning (same trap as
            // `allow_water` above). Note Java defaults the min level to 95
            // while this dist ships 40 — the shipped value is the live one.
            world_chat_enabled: p.get_bool("WorldChatEnabled", true),
            world_chat_min_level: p.get_int("WorldChatMinLevel", 95),
            world_chat_points_per_day: p.get_int("WorldChatPointsPerDay", 10),
            world_chat_interval_secs: p.get_duration_secs("WorldChatInterval", 20),
            log_chat: p.get_bool("LogChat", d.log_chat),
            log_items: p.get_bool("LogItems", d.log_items),
            log_items_small_log: p.get_bool("LogItemsSmallLog", d.log_items_small_log),
            log_items_ids_only: p.get_bool("LogItemsIdsOnly", d.log_items_ids_only),
            log_items_ids_list: p
                .get_string("LogItemsIdsList", "")
                .split(',')
                .filter_map(|s| s.trim().parse::<i32>().ok())
                .collect(),
            log_item_enchants: p.get_bool("LogItemEnchants", d.log_item_enchants),
            log_skill_enchants: p.get_bool("LogSkillEnchants", d.log_skill_enchants),
            gm_audit: p.get_bool("GMAudit", d.gm_audit),
            default_punish: crate::model::punishment::IllegalActionPunishment::find_by_name(
                &p.get_string("DefaultPunish", "KICK"),
            ),
            // Java: `0` means "one hundred years in seconds".
            default_punish_param: match p.get_int("DefaultPunishParam", 0) as i64 {
                0 => 3_155_695_200,
                v => v,
            },
        }
    }

    /// Java's `Item` gate, which is not a plain "is logging on" test: the small
    /// log and the id list are *overrides* that admit their own items even when
    /// the broad branch is off. Ported as one predicate so the four call sites
    /// cannot drift apart.
    pub fn should_log_item(&self, item_id: i32, equipable: bool) -> bool {
        const ADENA_ID: i32 = crate::data::item_data::ADENA_ID;
        (self.log_items && !self.log_items_small_log && !self.log_items_ids_only)
            || (self.log_items_small_log && (equipable || item_id == ADENA_ID))
            || (self.log_items_ids_only && self.log_items_ids_list.contains(&item_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real dist `General.ini` values (the GM block near the top of the
    /// file). Guards the key names against a config rename.
    #[test]
    fn loads_dist_gm_startup_values() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/config/General.ini"
        );
        let g = GeneralConfig::from_parser(&PropertiesParser::load(path));
        assert!(g.gm_hero_aura, "GMHeroAura=True");
        assert!(g.gm_startup_builder_hide, "GMStartupBuilderHide=True");
        assert!(g.gm_startup_invulnerable, "GMStartupInvulnerable=True");
        assert!(g.gm_startup_invisible, "GMStartupInvisible=True");
        assert!(g.gm_startup_silence, "GMStartupSilence=True");
        assert!(!g.gm_startup_auto_list, "GMStartupAutoList=False");
        assert!(!g.gm_startup_diet_mode, "GMStartupDietMode=False");
        assert!(!g.gm_give_special_skills, "GMGiveSpecialSkills=False");
        assert!(!g.allow_manor, "AllowManor=False on this dist");
        // Manor cutover times (guards the key names against a config rename).
        assert_eq!(g.alt_manor_refresh_time, 20, "AltManorRefreshTime=20");
        assert_eq!(g.alt_manor_refresh_min, 0, "AltManorRefreshMin=0");
        assert_eq!(g.alt_manor_maintenance_min, 6, "AltManorMaintenanceMin=6");
        assert_eq!(g.alt_manor_approve_time, 4, "AltManorApproveTime=4");
        assert_eq!(g.alt_manor_approve_min, 30, "AltManorApproveMin=30");
        // Lottery (G26.5): disabled on the dist, but the economics keys load.
        assert!(!g.allow_lottery, "AllowLottery=False on this dist");
        assert_eq!(g.alt_lottery_prize, 50000, "AltLotteryPrize=50000");
        assert_eq!(
            g.alt_lottery_ticket_price, 2000,
            "AltLotteryTicketPrice=2000"
        );
        // Parsed via get_float (f32) → f64, so compare with tolerance.
        assert!(
            (g.alt_lottery_5_number_rate - 0.6).abs() < 1e-6,
            "AltLottery5NumberRate=0.6"
        );
        assert_eq!(
            g.alt_lottery_2and1_number_prize, 200,
            "AltLottery2and1NumberPrize=200"
        );
        assert!(!g.allow_race, "AllowRace=False on this dist");
        assert!(
            g.alt_item_auction_enabled,
            "AltItemAuctionEnabled=True on this dist"
        );
    }

    /// The dist ground-item auto-destroy block: NPC drops decay after 600 s,
    /// but player drops are kept (`DestroyPlayerDroppedItem=False`).
    #[test]
    fn loads_dist_ground_item_autodestroy_values() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/config/General.ini"
        );
        let g = GeneralConfig::from_parser(&PropertiesParser::load(path));
        assert_eq!(
            g.autodestroy_item_after, 600,
            "AutoDestroyDroppedItemAfter=600"
        );
        assert!(
            !g.destroy_dropped_player_item,
            "DestroyPlayerDroppedItem=False"
        );
        assert!(
            !g.destroy_equipable_player_item,
            "DestroyEquipableItem=False"
        );
        assert_eq!(g.protected_items, vec![0], "ListOfProtectedItems=0");
    }
}
