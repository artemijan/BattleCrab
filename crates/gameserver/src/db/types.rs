use super::*;

/// A starting item, already slot-resolved by the game thread (see
/// `game_loop::handle_character_create`) so the DB thread just persists rows.
#[derive(Debug, Clone)]
pub struct NewItem {
    pub item_id: i32,
    pub count: i64,
    /// `Some(paperdoll_index)` if equipped, `None` for a plain inventory item.
    pub paperdoll_index: Option<usize>,
}

/// An initial shortcut to persist at creation (`InitialShortcutData.
/// registerAllShortcuts`), already filtered by the game thread (unknown
/// skills / missing macro presets dropped). For `ShortcutType::Item` the `id`
/// is still the *item id* — the DB thread resolves it to the freshly created
/// item's object id (the game thread never learns those).
#[derive(Debug, Clone, Copy)]
pub struct NewShortcut {
    pub slot: i32,
    pub page: i32,
    pub kind: crate::model::shortcut::ShortcutType,
    pub id: i32,
    pub level: i32,
}

/// A validated character to insert (built by the game thread from the template).
#[derive(Debug, Clone)]
pub struct NewCharacter {
    pub account: String,
    pub name: String,
    pub race: i32,
    pub class_id: i32,
    pub sex: i32,
    pub face: i32,
    pub hair_style: i32,
    pub hair_color: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub max_hp: i32,
    pub max_mp: i32,
    /// Initial `(skill_id, skill_level)` from the class skill tree.
    pub skills: Vec<(i32, i32)>,
    /// Initial equipment + starting adena, pre-resolved by the game thread.
    pub items: Vec<NewItem>,
    /// Initial panel shortcuts (`initialShortcuts.xml`, global + class pages).
    pub shortcuts: Vec<NewShortcut>,
    /// Macro presets referenced by MACRO shortcuts above.
    pub macros: Vec<crate::model::shortcut::Macro>,
    /// `CharacterCreate`: `min(StartingVitalityPoints, MAX_VITALITY_POINTS)`
    /// when `EnableVitality`, else the column default (0). Resolved on the game
    /// thread, which owns the config.
    pub vitality_points: i32,
}

/// The persistable slice of a `Player`, snapshotted on the game thread when the
/// character leaves the world (restart / logout / disconnect) — Java
/// `Disconnection.storeMe().deleteMe()`. Covers the `storeCharBase` columns the
/// Rust `Player` actually tracks; the rest (clan, title, online time, faction,
/// …) keep their stored values. Java's companion stores — `storeCharSub`,
/// `storeEffect` (`character_skills_save`), item reuse — write through their
/// own paths rather than this one: subclasses landed with G17
/// (`character_subclasses`) and buff restore on login with G19's relative
/// `remaining_time` rows, both flushed where they are mutated.
/// Items and learned skills are already persisted at mutation time.
#[derive(Debug, Clone)]
pub struct PlayerSnapshot {
    pub object_id: i32,
    pub level: i32,
    pub max_hp: i32,
    pub cur_hp: f64,
    pub max_cp: i32,
    pub cur_cp: f64,
    pub max_mp: i32,
    pub cur_mp: f64,
    pub face: i32,
    pub hair_style: i32,
    pub hair_color: i32,
    pub sex: i32,
    pub heading: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub exp: i64,
    pub sp: i64,
    pub reputation: i32,
    pub pvp_kills: i32,
    pub pk_kills: i32,
    pub raidboss_points: i32,
    pub rec_have: i32,
    pub rec_left: i32,
    pub race: i32,
    pub class_id: i32,
    pub base_class_id: i32,
    pub vitality_points: i32,
    pub pccafe_points: i32,
    /// `characters.nobless` — Olympiad nobless, toggled by `//setnoble`.
    pub noble: bool,
}

impl PlayerSnapshot {
    pub fn of(
        p: &crate::model::Player,
        pos: &crate::model::components::Position,
        vitals: &crate::model::components::Vitals,
        pvitals: &crate::model::components::PlayerVitals,
    ) -> Self {
        Self {
            object_id: p.object_id,
            level: p.level,
            max_hp: vitals.max_hp,
            cur_hp: vitals.cur_hp,
            max_cp: pvitals.max_cp,
            cur_cp: pvitals.cur_cp,
            max_mp: vitals.max_mp,
            cur_mp: vitals.cur_mp,
            face: p.face,
            hair_style: p.hair_style,
            hair_color: p.hair_color,
            sex: p.is_female as i32,
            heading: pos.heading,
            x: pos.x,
            y: pos.y,
            z: pos.z,
            exp: p.exp,
            sp: p.sp,
            reputation: p.reputation,
            pvp_kills: p.pvp_kills,
            pk_kills: p.pk_kills,
            raidboss_points: p.raidboss_points,
            rec_have: p.rec_have,
            rec_left: p.rec_left,
            race: p.race,
            class_id: p.class_id,
            base_class_id: p.base_class_id,
            vitality_points: p.vitality_points,
            pccafe_points: p.pccafe_points,
            noble: p.is_noble,
        }
    }
}

/// The full persistable state of an online player, gathered on the game thread
/// and flushed by the DB thread in one transaction (`store_player`). Built by
/// `game_loop::net::build_save_data` at the four flush points — the staggered
/// periodic autosave, logout, class-transfer, and shutdown save-all. Between
/// flushes, gameplay mutations (equip, loot, skill learn, shortcuts, quests)
/// touch only in-memory ECS components; nothing is written on the packet path,
/// so no client packet can drive database load (the memory-first model — Java
/// `Player.store()` gathers the same data, but Java also writes eagerly on many
/// actions, which is exactly what this port deliberately does not do).
#[derive(Debug, Clone)]
pub struct PlayerSaveData {
    /// The `characters` row (level/exp/vitals/position/appearance).
    pub base: PlayerSnapshot,
    /// Every item the character owns — inventory + equipped — serialized from
    /// the `Inventory` component (`Inventory::to_rows`). The DB thread deletes
    /// any `items` row for this owner not present here, so this is the whole
    /// authoritative set, covering pickups, drops, stack changes and equips.
    pub items: Vec<ItemRow>,
    /// Learned skills as `(skill_id, skill_level, skill_sub_level)` for the
    /// **active** class index (see [`Self::class_index`]).
    pub skills: Vec<(i32, i32, i32)>,
    /// The *inactive* class indices' books (G17 subclasses), so a slot keeps
    /// what it learned while it was active.
    pub skills_by_index: std::collections::HashMap<i32, Vec<(i32, i32, i32)>>,
    /// Inactive indices' worn hennas.
    pub hennas_by_index: std::collections::HashMap<i32, Vec<(i32, i32)>>,
    /// Inactive indices' shortcut bars.
    pub shortcuts_by_index: std::collections::HashMap<i32, Vec<crate::model::shortcut::Shortcut>>,
    /// Which class index [`Self::skills`] belongs to.
    pub class_index: i32,
    /// Worn henna dyes as `(slot 1-3, symbol_id)` (class_index 0).
    pub hennas: Vec<(i32, i32)>,
    /// Registered recipes as `(recipe_list_id, is_dwarven)` — the `type` column
    /// (1 dwarven / 0 common) is derived on the game thread from `RecipeData`.
    pub recipe_book: Vec<(i32, bool)>,
    /// Panel/hotbar shortcuts (`Shortcuts` component).
    pub shortcuts: Vec<crate::model::shortcut::Shortcut>,
    /// Macro definitions (`Macros` component).
    pub macros: Vec<crate::model::shortcut::Macro>,
    /// Quest states + vars (`Quests` component), keyed by quest name.
    pub quests: std::collections::HashMap<String, crate::model::quest::QuestState>,
    /// Live skill reuse cooldowns (`Reuses` component) as `character_skills_save`
    /// rows — empty when `StoreSkillCooltime` is off. See [`SkillReuseRow`].
    pub skill_reuses: Vec<SkillReuseRow>,
    /// Active buffs (`Buffs` component) as `character_skills_save` rows —
    /// empty when `StoreSkillCooltime` is off. See [`SkillBuffRow`].
    pub skill_buffs: Vec<SkillBuffRow>,
    /// `character_variables` rows (`PlayerVariables` component) as `(var, val)`.
    pub variables: Vec<(String, String)>,
    /// Every `pets` row this character owns (`PlayerPets` component), including
    /// the currently-summoned pet, whose live state is folded in before the
    /// save. Upserted row by row — **never** deleted as a set, because a row is
    /// keyed by a collar this character may trade away rather than by the
    /// character (Java writes one pet at a time for the same reason).
    pub pets: Vec<PetRow>,
    /// `character_summons` rows — at most one on this dist. Replaced as a set
    /// (unlike `pets`), because a servitor row is keyed by its **owner** and
    /// so cannot be traded away.
    pub summons: Vec<SummonRow>,
}

/// One `character_summons` row — a servitor that was out when its owner logged
/// off, so the next login brings it back.
///
/// Unlike a pet, a servitor has no collar and no identity of its own: it is
/// recreated by **re-casting the skill that summoned it** (Java's restore is
/// literally `skill.applyEffects(player, player)`), then having its saved
/// vitals and remaining lifetime stamped back on.
#[derive(Debug, Clone)]
pub struct SummonRow {
    /// `summonSkillId` — the skill to re-cast. The player's *current* level of
    /// it is used, as Java does, so a servitor restored after a level-up comes
    /// back at the stronger tier.
    pub summon_skill_id: i32,
    pub cur_hp: i32,
    pub cur_mp: i32,
    /// `time` — seconds of lifetime left, so a servitor does not get a fresh
    /// full duration for free by relogging.
    pub remaining_secs: i32,
    /// The servitor's **own** buffs (`character_summon_skills_save`), so a
    /// Summoner's investment in their servitor is not wiped by a relog.
    /// Reuses [`SkillBuffRow`]: same relative-remaining-time semantics as the
    /// player's own buffs, frozen while offline.
    pub buffs: Vec<SkillBuffRow>,
}

/// One `pets` row — a pet's saved state, keyed by the **object id of its
/// collar** (`item_obj_id`), which is what makes two collars of the same kind
/// two different pets.
///
#[derive(Debug, Clone)]
pub struct PetRow {
    pub collar_object_id: i32,
    pub name: String,
    pub level: i32,
    pub cur_hp: f64,
    pub cur_mp: f64,
    pub exp: i64,
    pub sp: i64,
    pub fed: i32,
    /// Java's `restore` column — "True restores pet on login". Set when the
    /// pet was **out** at logout, so the next login brings it back
    /// (`CharSummonTable.INIT_PET` reads exactly `restore = 'true'`).
    pub restore: bool,
}

/// One `character_skills_save` reuse row (Java `Player.storeEffect`'s
/// `restore_type = 1` half). `systime_ms` is the **absolute** wall-clock end
/// time (Java `TimeStamp.getStamp()`), so cooldowns decay by real elapsed time
/// across a relog/restart; the game side converts it to/from a game tick.
#[derive(Debug, Clone, Copy)]
pub struct SkillReuseRow {
    /// The reuse-map key (Java `getReuseHashCode()`): the reuse group id, or the
    /// skill id when the skill has no group. Stored in the `skill_id` column —
    /// Java-schema-compatible for the (common) ungrouped case, and the value the
    /// `Reuses` map is re-keyed by on restore.
    pub reuse_key: i32,
    pub skill_level: i32,
    /// Full reuse duration ms (`reuse_delay` column / `SkillReuse::total_ms`).
    pub reuse_delay: i32,
    /// Absolute wall-clock instant the cooldown ends (`systime` column, ms).
    pub systime_ms: i64,
}

/// One `character_skills_save` **buff** row (Java `Player.storeEffect`'s
/// `restore_type = 0` half). Unlike a reuse row this stores a *relative*
/// `remaining_time` (seconds left at logout), never an absolute instant:
/// Java's `restoreEffects` feeds it straight back into
/// `skill.applyEffects(this, this, false, remainingTime)`, so a buff's
/// countdown is **frozen while the character is offline**. Buffs deliberately
/// do not decay across the offline gap the way cooldowns (which carry an
/// absolute `systime`) do.
#[derive(Debug, Clone, Copy)]
pub struct SkillBuffRow {
    pub skill_id: i32,
    pub skill_level: i32,
    /// Seconds of buff time left (`remaining_time` column, Java
    /// `BuffInfo.getTime()`).
    pub remaining_time_secs: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateResult {
    Ok,
    NameExists,
    TooMany,
    Fail,
}

/// Game thread → DB thread.
pub enum DbCommand {
    LoadCharacters {
        client_id: u32,
        account: String,
    },
    CreateCharacter {
        client_id: u32,
        data: NewCharacter,
    },
    MarkDelete {
        client_id: u32,
        account: String,
        char_id: i32,
        delete_time: i64,
    },
    RestoreCharacter {
        client_id: u32,
        account: String,
        char_id: i32,
    },
    /// Fire-and-forget hard delete (expired characters).
    DeleteCharacter {
        char_id: i32,
    },
    /// Fire-and-forget delete of a `pets` row whose collar was destroyed (Java
    /// `RequestDestroyItem`). Object ids are recycled, so leaving the row would
    /// let a future item inherit a stale pet.
    DeletePetRow {
        collar_object_id: i32,
    },
    /// Write a grand boss's state back (Java `GrandBossManager.setStatus` +
    /// `setStatSet`, which both hit `grandboss_data`). Sent when a boss dies
    /// and when it respawns, so the respawn window survives a restart —
    /// **the point of the table**.
    StoreGrandBoss {
        boss: crate::model::grand_boss::GrandBoss,
    },
    /// Char count + deletion times for the login server's `ReplyCharacters`.
    CountCharacters {
        account: String,
    },
    /// Name availability check for `RequestCharacterNameCreatable` (name already
    /// passed the game thread's validity checks).
    CheckNameCreatable {
        client_id: u32,
        name: String,
    },
    /// Flush a player's full state to the DB (`store_player`) — the memory-first
    /// model's only character-write path, sent by the staggered periodic
    /// autosave, on logout (`Disconnection.storeMe().deleteMe()`), on
    /// class-transfer, and by the shutdown save-all. Ordered before any
    /// following `LoadCharacters` on this channel, so a restart's re-sent list
    /// already reflects the save.
    StorePlayer {
        /// Boxed: this is the one large variant, and every *other* `DbCommand`
        /// queued on the channel would otherwise be padded to its size. A
        /// player save already writes to SQLite, so one allocation on the way
        /// there does not register.
        save: Box<PlayerSaveData>,
    },
    /// Reserve a block of object ids for the game thread (Java `IdManager`
    /// semantics without a cross-thread round trip per item — the DB thread
    /// owns the counter, the game thread allocates out of its block and asks
    /// for another when it runs low). Replied with `DbEvent::IdBlock`.
    ReserveIds {
        count: i64,
    },
    /// Fire-and-forget friendship insert — both directions in one statement
    /// (Java `RequestAnswerFriendInvite`'s two-row INSERT). Kept immediate:
    /// needs a consenting second player, so it's not a packet-flood surface.
    InsertFriendPair {
        a: i32,
        b: i32,
    },
    /// Fire-and-forget friendship delete, both directions (`RequestFriendDel`).
    DeleteFriendPair {
        a: i32,
        b: i32,
    },
    /// Fire-and-forget block insert (Java `BlockList.updateInDB`, add branch) —
    /// **one** row in **one** direction at `relation = 1`. Blocking is not
    /// mutual: the target is never told to block back, and their own list is
    /// untouched.
    InsertBlock {
        owner: i32,
        target: i32,
    },
    /// Fire-and-forget block delete (`BlockList.updateInDB`, remove branch).
    /// Relation-scoped, so it can never take a friendship row with it.
    DeleteBlock {
        owner: i32,
        target: i32,
    },
    /// Fire-and-forget `Clan.store()` — the 13-column `clan_data` INSERT
    /// with everything but id/name/leader at Java's defaults.
    InsertClan {
        clan_id: i32,
        name: String,
        leader_id: i32,
    },
    /// Fire-and-forget clan-membership update on a character
    /// (`ClanTable.createClan` side effects; `StorePlayer`'s UPDATE doesn't
    /// touch these columns).
    UpdateCharClan {
        char_id: i32,
        clan_id: i32,
        clan_privs: i32,
    },
    /// `CursedWeapon.saveData` — upsert the weapon's wielder state row.
    StoreCursedWeapon {
        item_id: i32,
        char_id: i32,
        reputation: i32,
        pk_kills: i32,
        nb_kills: i32,
        end_time: i64,
    },
    /// `DBSpawnManager.updateStatus`/`addNewSpawn` — upsert a raid boss's
    /// `npc_respawns` row (live HP/MP while alive, or the pending respawn time
    /// while dead). Keyed on npc id, matching Java's PRIMARY KEY.
    StoreNpcRespawn {
        npc_id: i32,
        x: i32,
        y: i32,
        z: i32,
        heading: i32,
        respawn_time: i64,
        cur_hp: f64,
        cur_mp: f64,
    },
    /// `ADD_CHAR_SUBCLASS` / `UPDATE_CHAR_SUBCLASS` — upsert one subclass slot.
    /// Keyed on `(charId, class_id)` like Java's primary key.
    /// `Clan.storeNotice` — upsert one `clan_notices` row (the board's clan
    /// notice edit / enable / disable).
    SaveClanNotice {
        clan_id: i32,
        enabled: bool,
        notice: String,
    },
    /// `Player.modifySubClass`'s delete block: drop one slot's
    /// `character_subclasses` row (keyed by the old class id) and every
    /// per-index row — skills, hennas, shortcuts, skill reuses — for that
    /// `class_index`. The replacement class is stored by the normal slot
    /// upsert that follows.
    WipeSubclassSlot {
        char_id: i32,
        class_index: i32,
        old_class_id: i32,
    },
    StoreSubClass {
        char_id: i32,
        class_id: i32,
        class_index: i32,
        level: i32,
        exp: i64,
        sp: i64,
    },
    /// `DBSpawnManager.deleteSpawn` — drop a raid boss's respawn row.
    DeleteNpcRespawn {
        npc_id: i32,
    },
    /// `CursedWeaponsManager.removeFromDb` — drop the weapon's state row.
    RemoveCursedWeapon {
        item_id: i32,
    },
    /// `CursedWeapon.endOfLife`'s **offline** branch — the wielder isn't logged
    /// in, so their restore is done straight in the database: delete the weapon
    /// item and put back the reputation/pk-kills the curse overwrote. Java's
    /// two statements (`DELETE FROM items …` + `UPDATE characters SET
    /// reputation, pkkills …`) plus `skill_ids`, which has no Java counterpart:
    /// Java grants the cursed/transform skills with `addSkill(…, false)` so
    /// they never reach the DB, while this port persists the whole `SkillBook`.
    RestoreOfflineCursedOwner {
        char_id: i32,
        item_id: i32,
        reputation: i32,
        pk_kills: i32,
        skill_ids: Vec<i32>,
    },
    /// `Castle.setSide`/`switchSide` — persist a castle's side.
    UpdateCastleSide {
        castle_id: i32,
        side: String,
    },
    /// `Castle.setShowNpcCrest` — ownership changes reset the crest display.
    UpdateCastleShowNpcCrest {
        castle_id: i32,
        show: bool,
    },
    /// Castle ownership on the clan side (`Castle.setOwner`/`removeOwner` →
    /// `clan_data.hasCastle`).
    /// `//clan_changeleader` — persist a forced leader swap (`clan_data.leader_id`).
    UpdateClanLeader {
        clan_id: i32,
        leader_id: i32,
    },
    UpdateClanCastle {
        clan_id: i32,
        castle_id: i32,
    },
    /// `Clan.updateBloodAllianceCountInDB` — persist the blood-alliance count.
    UpdateClanBloodAlliance {
        clan_id: i32,
        count: i32,
    },
    /// `Castle.setTicketBuyCount` — persist the mercenary ticket-buy count.
    UpdateCastleTicketCount {
        castle_id: i32,
        count: i32,
    },
    /// `Product.save()` — upsert the remaining stock and restock deadline of
    /// one limited-stock buy-list line. Written on every sale and every
    /// restock, which is Java's rate too.
    SaveBuyListStock {
        list_id: i32,
        item_id: i32,
        count: i64,
        next_restock_time: i64,
    },
    /// `SiegeGuardManager.addTicket` — a posted mercenary, written as a
    /// `castle_siege_guards` row with `isHired = 1`.
    AddHiredSiegeGuard {
        castle_id: i32,
        npc_id: i32,
        x: i32,
        y: i32,
        z: i32,
        heading: i32,
    },
    /// `SiegeGuardManager.removeSiegeGuard` — one posting undone, matched the
    /// way Java matches it: npc id **and** exact position.
    RemoveHiredSiegeGuard {
        npc_id: i32,
        x: i32,
        y: i32,
        z: i32,
    },
    /// `SiegeGuardManager.removeSiegeGuards(castle)` — every posting cleared,
    /// which is what a change of ownership does.
    ClearHiredSiegeGuards {
        castle_id: i32,
    },
    /// `RequestPackageSend` to an **offline** recipient — insert the freighted
    /// items straight into their `items` rows (`loc = FREIGHT`), since there is
    /// no live `Freight` component to write through. An online recipient's
    /// component is updated instead, so the two paths never both fire.
    AddFreightItems {
        owner_id: i32,
        items: Vec<FreightItemRow>,
    },
    /// `OfflineTraderTable.onTransaction(trader, false, true)` — rewrite one
    /// unattended shop's two tables (status row + its item lines). Sent when a
    /// shop goes offline, after every transaction against it (realtime mode),
    /// and by the shutdown sweep.
    StoreOfflineTrader {
        char_id: i32,
        /// Java `Player.getOfflineStartTime()`.
        time: i64,
        /// `PrivateStoreType.getId()`.
        store_type: i32,
        title: String,
        /// `(item, count, price)` — `item` is the *object* id for a sell store
        /// and the *item* id for a buy store (Java writes both into the same
        /// column), or the recipe id for a manufacture store.
        items: Vec<(i32, i64, i64)>,
    },
    /// `OfflineTraderTable.removeTrader` / `onTransaction(trader, true, …)` —
    /// drop one character's offline-shop rows (sold out, logged back in).
    ClearOfflineTrader {
        char_id: i32,
    },
    /// `CastleManorManager.storeMe` for one castle — replace both manor tables'
    /// rows for it (all four period lists) in one shot. Java rewrites every
    /// castle's rows after the daily rollover; the port stores the castle it
    /// just rolled, which is the same end state with a narrower delete.
    StoreManor {
        castle_id: i32,
        production: Vec<ManorProductionRow>,
        procure: Vec<ManorProcureRow>,
    },
    /// `Castle.addToTreasuryNoTax` — persist the castle vault. Java writes the
    /// row on every change (tax income, manor seed sale, chamberlain deposit or
    /// withdrawal), so this is sent from each of those paths.
    UpdateCastleTreasury {
        castle_id: i32,
        treasury: i64,
    },
    /// `Siege.saveSiegeDate` — persist the owner-chosen siege time + that the
    /// time-registration window has closed.
    UpdateCastleSiegeTime {
        castle_id: i32,
        siege_date: i64,
        time_registration_over: bool,
        /// `castle.regTimeEnd` — only written when the hour-picking window is
        /// (re)stamped at siege end; `None` leaves the stored value alone, the
        /// way Java's `saveSiegeDate` touches only the columns it owns.
        siege_time_registration_end: Option<i64>,
    },
    /// `GlobalVariablesManager.set` — upsert one `global_variables` row.
    ///
    /// Java batches the whole map on a 30-minute `onSave` timer plus shutdown;
    /// this port writes each change through, matching how every other piece of
    /// small global state here is persisted (and removing the "lost the last
    /// 30 minutes on a crash" window).
    SaveGlobalVariable {
        var: String,
        value: String,
    },
    /// `Siege.saveSiegeClan` — register a clan for a castle's siege.
    SaveSiegeClan {
        castle_id: i32,
        clan_id: i32,
        kind: i32,
    },
    /// `Siege.removeSiegeClan` — drop a clan's `siege_clans` row.
    RemoveSiegeClan {
        castle_id: i32,
        clan_id: i32,
    },
    /// `ClanHallAuction.addBid` — upsert a `clanhall_auctions_bidders` row.
    SaveClanHallBid {
        hall_id: i32,
        clan_id: i32,
        bid: i64,
        bid_time: i64,
    },
    /// `ClanHallAuction.removeBid` — drop one clan's bid row.
    RemoveClanHallBid {
        hall_id: i32,
        clan_id: i32,
    },
    /// `finalizeAuctions` — clear every bid row for a hall.
    ClearClanHallBids {
        hall_id: i32,
    },
    /// `ClanHall.updateDB` — upsert a hall's ownership row (`clanhall`).
    SaveClanHall {
        id: i32,
        owner_id: i32,
        paid_until: i64,
    },
    /// `ClanHall.addFunction` — upsert a `residence_functions` row.
    SaveResidenceFunction {
        residence_id: i32,
        func_id: i32,
        level: i32,
        expiration: i64,
    },
    /// `ClanHall.removeFunction` — drop a function's `residence_functions` row.
    RemoveResidenceFunction {
        residence_id: i32,
        func_id: i32,
    },
    /// `Olympiad.saveOlympiadStatus` + `saveNobleData` — upsert the single
    /// `olympiad_data` row and every `olympiad_nobles` record.
    SaveOlympiad {
        current_cycle: i32,
        period: i32,
        olympiad_end: i64,
        validation_end: i64,
        next_weekly_change: i64,
        nobles: Vec<OlympiadNobleRow>,
    },
    /// `Hero.computeNewHeroes` — replace the `heroes` table with the new crown
    /// (all `played = 1`, `claimed = 'false'`).
    SaveHeroes {
        heroes: Vec<HeroRow>,
    },
    /// `Hero.claimHero`'s persistence half (Java re-runs `updateHeroes`, which
    /// rewrites the row): flip `heroes.claimed` for one crowned character.
    ClaimHero {
        char_id: i32,
    },
    /// `Olympiad.updateMonthlyData` — replace `olympiad_nobles_eom` with a copy
    /// of the live `olympiad_nobles`, run at the round end right after the
    /// nobles themselves are saved (so it must stay ordered behind
    /// `SaveOlympiad` on this channel).
    SnapshotOlympiadEom,
    /// `Hero.setDiaryData` — append a `heroes_diary` row (a noble's notable
    /// action, e.g. `ACTION_CASTLE_TAKEN`).
    SaveHeroDiary {
        char_id: i32,
        time: i64,
        action: i32,
        param: i32,
    },
    /// Fire-and-forget clan level persist (`Clan.changeLevel`'s single UPDATE).
    UpdateClanLevel {
        clan_id: i32,
        level: i32,
    },
    /// `Clan.addNewSkill` — upsert a learned clan skill (`sub_pledge_id = -2`,
    /// the main pledge). Keyed on `(clan_id, skill_id)`, so a re-grant at a
    /// higher level replaces the row.
    SaveClanSkill {
        clan_id: i32,
        skill_id: i32,
        skill_level: i32,
        skill_name: String,
    },
    /// Delete one learned clan skill row (`sub_pledge_id = -2`). Used to purge a
    /// residence skill wrongly stored on the clan by a pre-fix `//give_clan_skills`.
    DeleteClanSkill {
        clan_id: i32,
        skill_id: i32,
    },
    /// Fire-and-forget clan reputation persist (`Clan.setReputationScore`, which
    /// Java writes via `updateClanScoreInDb`).
    UpdateClanReputation {
        clan_id: i32,
        reputation: i32,
    },
    /// `RequestOustPledgeMember` / village-master dissolve/recover — the two
    /// clan-side penalty stamps (`Clan.updateClanInDB`, narrowed).
    UpdateClanPenalties {
        clan_id: i32,
        char_penalty_expiry_time: i64,
        dissolving_expiry_time: i64,
    },
    /// A member left/was ousted (`Clan.removeClanMember` →
    /// `removeMemberInDatabase`): reset the character's clan columns and stamp
    /// the rejoin penalty (+ recreate penalty when the ex-member led the clan).
    RemoveClanMember {
        char_id: i32,
        clan_join_expiry: i64,
        clan_create_expiry: i64,
    },
    /// `Player.setClanJoinExpiryTime` persisted alone (invite accepted zeroes
    /// it; `characters.clan_join_expiry_time`).
    UpdateCharClanJoinExpiry {
        char_id: i32,
        expiry: i64,
    },
    /// `Clan.setRankPrivs` — upsert one `clan_privs` row (party is always 0).
    SaveClanRankPrivs {
        clan_id: i32,
        rank: i32,
        privs: i32,
    },
    /// `ClanMember.updatePowerGrade` — persist a member's rank.
    UpdateCharPowerGrade {
        char_id: i32,
        power_grade: i32,
    },
    /// `Clan.setNewLeaderId(id, true)` — the pending delegated leader transfer.
    UpdateClanNewLeader {
        clan_id: i32,
        new_leader_id: i32,
    },
    /// `ClanEntryManager.addPlayerApplicationToClan` — upsert one applicant row.
    UpsertPledgeApplicant {
        player_id: i32,
        clan_id: i32,
        karma: i32,
        message: String,
    },
    /// `ClanEntryManager.removePlayerApplication`.
    DeletePledgeApplicant {
        player_id: i32,
        clan_id: i32,
    },
    /// `ClanEntryManager.addToWaitingList`.
    InsertPledgeWaiting {
        player_id: i32,
        karma: i32,
    },
    /// `ClanEntryManager.removeFromWaitingList`.
    DeletePledgeWaiting {
        player_id: i32,
    },
    /// `ClanEntryManager.addToClanList`.
    InsertPledgeRecruit {
        clan_id: i32,
        karma: i32,
        information: String,
        detailed_information: String,
        application_type: i32,
        recruit_type: i32,
    },
    /// `ClanEntryManager.updateClanList`.
    UpdatePledgeRecruit {
        clan_id: i32,
        karma: i32,
        information: String,
        detailed_information: String,
        application_type: i32,
        recruit_type: i32,
    },
    /// `ClanEntryManager.removeFromClanList`.
    DeletePledgeRecruit {
        clan_id: i32,
    },
    /// `CrestTable.createCrest` — insert a new stored bitmap.
    InsertCrest {
        id: i32,
        data: Vec<u8>,
        kind: i32,
    },
    /// `CrestTable.removeCrest` (skipped by the caller when `id` is the most
    /// recently allocated one — Java never reuses the last id).
    DeleteCrest {
        id: i32,
    },
    /// `Clan.changeClanCrest` — the small pledge crest column.
    UpdateClanCrest {
        clan_id: i32,
        crest_id: i32,
    },
    /// `Clan.changeLargeCrest` — the large pledge crest column.
    UpdateClanCrestLarge {
        clan_id: i32,
        crest_large_id: i32,
    },
    /// `Clan.changeAllyCrest(id, onlyThisClan=true)` — one clan's own row
    /// (a member who just joined/left inherits the leader's crest id this way).
    UpdateClanAllyCrestSelf {
        clan_id: i32,
        ally_crest_id: i32,
    },
    /// `Clan.changeAllyCrest(id, onlyThisClan=false)` — every clan in the
    /// alliance at once (`WHERE ally_id=?`), the leader's own registration path.
    UpdateAllyCrestForAlliance {
        ally_id: i32,
        ally_crest_id: i32,
    },
    /// The ally half of `Clan.updateClanInDB` — membership + penalty stamps.
    UpdateClanAlly {
        clan_id: i32,
        ally_id: i32,
        ally_name: String,
        penalty_expiry: i64,
        penalty_type: i32,
    },
    /// `Clan.createSubPledge`'s insert — a new academy/royal/knight unit.
    InsertSubPledge {
        clan_id: i32,
        pledge_type: i32,
        name: String,
        leader_id: i32,
    },
    /// `Clan.updateSubPledgeInDB` — rename and/or leader reassignment.
    UpdateSubPledge {
        clan_id: i32,
        pledge_type: i32,
        name: String,
        leader_id: i32,
    },
    /// `ClanMember.updatePowerGrade`'s sibling for pledge type — persisted
    /// whenever a member's sub-unit membership changes (join/reorganize/
    /// leave-clears-captaincy).
    UpdateCharPledgeType {
        char_id: i32,
        pledge_type: i32,
    },
    /// `characters.lvl_joined_academy` — set when a character joins a clan
    /// academy, cleared on graduation and on leaving the clan. It is what makes
    /// `isAcademyMember()` true, so it cannot ride the periodic autosave.
    UpdateCharAcademyLevel {
        char_id: i32,
        lvl_joined_academy: i32,
    },
    /// `ClanMember.saveApprenticeAndSponsor` — both columns in one UPDATE.
    /// Java writes this **even when the member is online**, "since both must
    /// match", so the port keeps it a direct write too.
    UpdateCharApprenticeSponsor {
        char_id: i32,
        apprentice: i32,
        sponsor: i32,
    },
    /// `ClanTable.storeClanWars` — upsert one `clan_wars` row (ids, despite the
    /// varchar columns — Java binds ints too).
    SaveClanWar {
        attacker: i32,
        attacked: i32,
        attacker_kills: i32,
        attacked_kills: i32,
        winner: i32,
        start_time: i64,
        end_time: i64,
        state: i32,
    },
    /// `ClanTable.deleteClanWars` — drop the war row. Java deletes only the
    /// `(clan1, clan2)` order it was called with (a surrender can miss the row
    /// until the next boot's cleanup); both orders are deleted here — the same
    /// eventual outcome without the stale row.
    DeleteClanWar {
        clan1: i32,
        clan2: i32,
    },
    /// `ClanTable.destroyClan` — delete the `clan_data` row and reset every
    /// member's `characters` clan columns (online *and* offline, since the
    /// memory-first autosave never touches those columns). `leader_id` also gets
    /// the 10-day recreate cooldown stamped at `leader_expiry` (Java sets it on
    /// the leader during `removeClanMember`).
    DestroyClan {
        clan_id: i32,
        leader_id: i32,
        leader_expiry: i64,
    },
    /// Fire-and-forget clan-warehouse flush — delete every `owner_id = clan_id`
    /// item row (`loc = "CLANWH"`) and reinsert the current set (the same
    /// delete-then-reinsert the player item save uses).
    StoreClanWarehouse {
        clan_id: i32,
        items: Vec<ItemRow>,
    },
    /// Java `Player.setAccessLevel(updateInDb=true)` — persist a GM access-level
    /// change immediately (the memory-first autosave doesn't carry accesslevel).
    SetAccessLevel {
        char_id: i32,
        level: i32,
    },
    /// Upsert one `account_gsdata` row (Java `AccountVariables.storeMe`,
    /// write-through). Used by `//primepoints` for the account-scoped
    /// "PRIME_POINTS" variable.
    StoreAccountVar {
        account_name: String,
        var: String,
        value: String,
    },
    /// Upsert a single `character_variables` row for a (possibly offline)
    /// character — a targeted replace (delete + insert), unlike the wholesale
    /// rewrite in the player flush. Used to bank Olympiad trade points for
    /// offline nobles.
    StoreCharVar {
        char_id: i32,
        var: String,
        value: String,
    },
    /// Upsert / delete an `account_premium` row (Java `PremiumManager`
    /// UPDATE/DELETE). Used by `//premium_*`.
    StorePremium {
        account_name: String,
        enddate: i64,
    },
    DeletePremium {
        account_name: String,
    },
    /// Insert a fresh Lucky Lottery round (Java `Lottery.INSERT_LOTTERY`, G26.5).
    /// `newprize` starts equal to `prize`.
    /// Persist a whole mail message (Java `Message.getStatement` INSERT) —
    /// G30. Attachments are separate `items` rows, written by `StoreMailItems`.
    StoreMail {
        message: MailRow,
    },
    /// Flip one of the message's boolean columns (Java's four one-column
    /// UPDATEs on `messages`), or delete the row when `delete` is set.
    UpdateMailFlags {
        message_id: i32,
        unread: bool,
        has_attachments: bool,
        deleted_by_sender: bool,
        deleted_by_receiver: bool,
    },
    /// Java `MailManager.deleteMessageInDb` — drop the row *and* any attachment
    /// item rows still parked on it.
    DeleteMail {
        message_id: i32,
    },
    /// Park items in an **offline** character's warehouse (G30) — how expired
    /// mail attachments get back to a sender who is not logged in. Additive:
    /// the owner's other warehouse rows are untouched.
    StoreOfflineWarehouseItems {
        owner_id: i32,
        items: Vec<ItemRow>,
    },
    /// Replace the `loc = 'MAIL'` item rows of one message (delete-then-insert,
    /// the house style for a whole container).
    StoreMailItems {
        message_id: i32,
        owner_id: i32,
        items: Vec<ItemRow>,
    },
    StoreLottery {
        idnr: i32,
        enddate: i64,
        prize: i64,
    },
    /// Mark a lottery round drawn (Java `Lottery.UPDATE_LOTTERY`): winning
    /// numbers + per-tier prizes + the pot carried to the next round.
    FinishLottery {
        idnr: i32,
        prize: i64,
        newprize: i64,
        number1: i32,
        number2: i32,
        prize1: i64,
        prize2: i64,
        prize3: i64,
    },
    /// Grow the current round's pot after a ticket sale (Java
    /// `Lottery.UPDATE_PRICE`).
    IncreaseLotteryPrize {
        idnr: i32,
        prize: i64,
    },
    /// Query the sold tickets of a round for the draw (Java
    /// `Lottery.SELECT_LOTTERY_ITEM`): the persisted item 4442 rows. Replies with
    /// [`DbEvent::LotteryTicketsLoaded`].
    /// `CustomMailManager`'s poll: read every pending `custom_mail` row. The
    /// rows an operator (or a web shop) writes straight into the table are the
    /// whole interface — the game server only ever reads and deletes them.
    LoadCustomMail,
    /// Delete one delivered `custom_mail` row, keyed as Java keys it: the
    /// `(date, receiver)` pair, which is the table's composite primary key.
    DeleteCustomMail {
        date: String,
        receiver: i32,
    },
    LoadLotteryTickets {
        round: i32,
    },
    /// Upsert a finished Monster Race result (Java `MonsterRace.saveHistory`).
    SaveMdtHistory {
        race_id: i32,
        first: i32,
        second: i32,
        odd_rate: f64,
    },
    /// Upsert a lane's pooled bet (Java `MonsterRace.saveBet`).
    SaveMdtBet {
        lane: i32,
        bet: i64,
    },
    /// Zero every lane's bet after a race (Java `MonsterRace.clearBets`).
    ClearMdtBets,
    /// Upsert an item auction's row (Java `ItemAuction.storeMe`, G30.5).
    StoreItemAuction {
        auction_id: i32,
        instance_id: i32,
        auction_item_id: i32,
        starting_time: i64,
        ending_time: i64,
        state_id: i8,
    },
    /// Upsert one player's bid on an auction (Java `updatePlayerBid`, insert).
    StoreItemAuctionBid {
        auction_id: i32,
        player_obj_id: i32,
        bid: i64,
    },
    /// Delete one player's bid (Java `updatePlayerBid`, delete branch).
    DeleteItemAuctionBid {
        auction_id: i32,
        player_obj_id: i32,
    },
    /// Delete an auction + all its bids (Java `ItemAuctionManager.deleteAuction`).
    DeleteItemAuction {
        auction_id: i32,
    },
    /// Insert a punishment row (Java `PunishmentTask.onStart`'s INSERT, G31).
    /// The `id` is allocated on the game thread so the row can be deleted by id
    /// without waiting for a generated-key round-trip.
    /// Java `BotReportTable.saveReportedCharData` — clear the table and
    /// rewrite every (bot, reporter, time) row. Shutdown only, like Java.
    StoreBotReports {
        rows: Vec<(i32, i32, i64)>,
    },
    StorePunishment {
        id: i32,
        key: String,
        affect: String,
        ptype: String,
        expiration: i64,
        reason: String,
        punished_by: String,
    },
    /// Delete a punishment row by id (Java's `onEnd` expires the row in place;
    /// this port removes it — behaviourally identical since the load skips
    /// expired rows anyway, and nothing reads a dead punishment).
    DeletePunishment {
        id: i32,
    },
    /// Insert a petition-feedback row (Java `RequestPetitionFeedback`, G31) — the
    /// only petition state that persists. `rate` is 0-4.
    StorePetitionFeedback {
        char_name: String,
        gm_name: String,
        rate: i32,
        message: String,
        date: i64,
    },
    /// Insert a won auction item into an **offline** winner's warehouse (Java
    /// `onAuctionFinished`'s offline branch: set owner + WAREHOUSE loc +
    /// `updateDatabase`). Online winners get it added to their live component.
    StoreOfflineWarehouseItem {
        owner_id: i32,
        object_id: i32,
        item_id: i32,
        count: i64,
        enchant: i32,
    },
    /// Upsert / delete a `buffer_schemes` row (Java `SchemeBufferTable`; Java
    /// bulk-rewrites the table at shutdown, this port write-throughs per change).
    /// `skills` is the comma-joined skill-id list. Used by the community board's
    /// `_bbs_buff_scheme_create`/`_delete`.
    StoreBufferScheme {
        object_id: i32,
        scheme_name: String,
        skills: String,
    },
    DeleteBufferScheme {
        object_id: i32,
        scheme_name: String,
    },
    /// Insert / delete a `bbs_favorites` row (Java `FavoriteBoard`
    /// ADD_FAVORITE / DELETE_FAVORITE). `fav_id` is allocated on the game thread
    /// (the table-wide AUTOINCREMENT PK, written explicitly here) so the
    /// memory-first mirror carries it immediately. Used by the community board's
    /// `bbs_add_fav`/`_bbsdelfav_`.
    StoreFavorite {
        fav_id: i32,
        player_id: i32,
        title: String,
        bypass: String,
        add_date: String,
    },
    DeleteFavorite {
        player_id: i32,
        fav_id: i32,
    },
    /// Fire-and-forget daily recommendation reset for offline characters
    /// (Java `DailyTaskManager.resetRecommends`'s two UPDATE statements).
    /// Online players are reset in memory on the game thread; their rows get
    /// rewritten from memory by the next autosave, so this only needs to fix
    /// the offline population.
    ResetRecommends,
    /// Fire-and-forget daily world-chat quota reset (Java
    /// `DailyTaskManager.resetWorldChatPoints`'s single UPDATE).
    ///
    /// Java's statement carries **no character filter** — it zeroes the
    /// `WORLD_CHAT_USED` row of every character that has one, online included.
    /// Harmless there and here: the online half runs straight after and writes
    /// the same 0 into memory.
    ResetWorldChatPoints,
    /// The offline-population vitality refill (Java `DailyTaskManager
    /// .resetVitalityDaily`/`resetVitalityWeekly`, G33): `weekly` sets the pool
    /// to max, otherwise adds `MAX/4`. Online players are refilled in memory on
    /// the game thread (rewritten by the next autosave); this fixes the offline
    /// rows in both `characters` and `character_subclasses`.
    ResetVitality {
        weekly: bool,
    },
    /// `AdminRepairChar` (Java `//repair`/`//restore`, G33): unstick a broken
    /// **offline** character by name — teleport to a safe spot (Giran), clear
    /// its shortcuts (a corrupt one crashes the client), and move every item
    /// back to the inventory (un-equip whatever is wedged). Keyed by name; a
    /// no-op if the name doesn't exist.
    RepairCharacter {
        char_name: String,
    },
    Shutdown,
}

/// DB thread → game thread (drained in tick step 2).
pub enum DbEvent {
    /// `GlobalVariablesManager.restoreMe()` — the whole table, at boot.
    GlobalVariablesLoaded { entries: Vec<(String, String)> },
    /// `send_list` = push a fresh `CharSelectionInfo` to the client (login,
    /// delete, restore). After character creation it is false — Java only caches
    /// the list (`setCharSelection`) and does not re-send it.
    CharactersLoaded {
        client_id: u32,
        account: String,
        chars: Vec<CharData>,
        send_list: bool,
    },
    CharacterCreated {
        client_id: u32,
        result: CreateResult,
    },
    CharCount {
        account: String,
        count: u8,
        del_times: Vec<i64>,
    },
    /// `ExIsCharNameCreatable` result: -1 = creatable, else a failure code.
    NameCreatable { client_id: u32, result: i32 },
    /// A reserved object-id block `[start, end)` for the game thread's
    /// runtime allocations (loot items). One is pushed unprompted at boot.
    IdBlock { start: i64, end: i64 },
    /// The full clan table (`ClanTable` boot load), pushed unprompted at
    /// boot like the first `IdBlock`.
    ClansLoaded {
        clans: Vec<crate::model::clan::Clan>,
        wars: Vec<crate::model::clan::ClanWar>,
        crests: Vec<crate::model::clan::Crest>,
        recruit_clans: Vec<crate::model::clan_entry::PledgeRecruitInfo>,
        recruit_waiting: Vec<crate::model::clan_entry::PledgeWaitingInfo>,
        recruit_applicants: Vec<crate::model::clan_entry::PledgeApplicantInfo>,
        /// `clan_notices` rows as `clan_id → (enabled, notice)`.
        notices: Vec<(i32, bool, String)>,
    },
    /// The whole `npc_respawns` table (Java `DBSpawnManager.load`), pushed
    /// unprompted at boot. See [`NpcRespawnRow`].
    NpcRespawnsLoaded { rows: Vec<NpcRespawnRow> },
    /// `OfflineTraderTable.restoreOfflineTraders`' two queries, already joined:
    /// every stored shop with its full character and its item lines. Pushed
    /// unprompted at boot (before `ClansLoaded`), like the other restores; the
    /// game thread applies the config gates and `OfflineMaxDays`.
    OfflineTradersLoaded { traders: Vec<OfflineTraderRow> },
    /// The whole `account_premium` table (Java `PremiumManager` cache),
    /// pushed unprompted at boot. `(account_name lowercase, enddate millis)`.
    PremiumLoaded { entries: Vec<(String, i64)> },
    /// The most recent `lottery` row (Java `Lottery.SELECT_LAST_LOTTERY`) for the
    /// lifecycle, plus every finished round's draw result (round id →
    /// [`DrawnRound`](crate::model::lottery::DrawnRound)) for offline prize
    /// claim. Pushed unprompted at boot; `row` is `None` on a first-ever boot.
    LotteryLoaded {
        row: Option<crate::model::lottery::LotteryRow>,
        draws: Vec<(i32, crate::model::lottery::DrawnRound)>,
    },
    /// The persisted (offline) sold tickets of `round` for a draw — the reply to
    /// [`DbCommand::LoadCustomMail`] — the pending rows, in table order.
    CustomMailLoaded { rows: Vec<CustomMailRow> },
    /// [`DbCommand::LoadLotteryTickets`]. `(object_id, enchant, custom_type2)`
    /// per ticket item 4442; the draw dedupes these against online inventories.
    LotteryTicketsLoaded {
        round: i32,
        rows: Vec<(i32, i32, i32)>,
    },
    /// The Monster Race history + current lane bets (Java `MonsterRace
    /// .loadHistory`/`loadBets`), pushed unprompted at boot.
    MdtLoaded {
        history: Vec<crate::model::monster_race::HistoryInfo>,
        bets: Vec<(i32, i64)>,
    },
    /// The persisted item auctions + their bids + the next auction id (Java
    /// `ItemAuctionManager` boot load, G30.5), pushed unprompted at boot.
    ItemAuctionsLoaded {
        next_auction_id: i32,
        auctions: Vec<crate::model::item_auction::ItemAuction>,
    },
    /// Every mail message + its attachments, and the offline character
    /// name -> id table mail needs to address them (Java `MailManager.load`
    /// + `CharInfoTable`, G30). Pushed unprompted at boot.
    MailLoaded {
        messages: Vec<crate::model::mail::Message>,
        attachments: Vec<(i32, Vec<ItemRow>)>,
        char_ids_by_name: Vec<(String, i32)>,
        /// Every character's ignore list (Java `BlockList`). Rides this event
        /// because it is wanted at the same moment and for the same reason as
        /// the name table: mail must be filtered against an addressee who need
        /// not be online.
        block_lists: Vec<(i32, std::collections::HashSet<i32>)>,
    },
    /// The active punishments (Java `PunishmentManager.load`, G31), pushed
    /// unprompted at boot. Already-expired rows are filtered out here; `next_id`
    /// seeds the game-thread id allocator past the highest loaded id.
    PunishmentsLoaded {
        next_id: i32,
        punishments: Vec<crate::model::punishment::Punishment>,
    },
    /// The whole `bot_reported_char_data` table (Java
    /// `BotReportTable.loadReportedCharData`), pushed unprompted at boot as
    /// `(bot_id, reporter_id, report_date)`.
    BotReportsLoaded { rows: Vec<(i32, i32, i64)> },
    /// The whole `buffer_schemes` table (Java `SchemeBufferTable.load`), pushed
    /// unprompted at boot. `(object_id, scheme_name, skill_ids)`; skills not in
    /// the available-buff table are filtered on the game thread.
    BufferSchemesLoaded {
        entries: Vec<(i32, String, Vec<i32>)>,
    },
    /// The whole `bbs_favorites` table (Java `FavoriteBoard` loads per-player on
    /// demand; this port caches all rows at boot like `buffer_schemes`), pushed
    /// unprompted at boot. `(player_id, fav_id, title, bypass, add_date)`,
    /// newest first.
    FavoritesLoaded {
        entries: Vec<(i32, i32, String, String, String)>,
    },
    /// The `grandboss_data` table (Java `GrandBossManager.init`), pushed
    /// unprompted at boot. Filtered to known NPC templates on the game thread.
    GrandBossesLoaded {
        bosses: Vec<crate::model::grand_boss::GrandBoss>,
    },
    /// The `cursed_weapons` state table (Java `CursedWeaponsManager.restore`),
    /// pushed unprompted at boot; overlaid onto the XML config on the game thread.
    CursedWeaponsLoaded { rows: Vec<CursedWeaponRow> },
    /// The `castle` table (Java `CastleManager.load`), pushed unprompted at boot.
    CastlesLoaded {
        castles: Vec<crate::model::castle::Castle>,
    },
    /// The `siege_clans` table (Java `Siege.loadSiegeClan`), pushed unprompted at
    /// boot after `CastlesLoaded`. Grouped into per-castle sieges on the game thread.
    SiegesLoaded { rows: Vec<SiegeClanRow> },
    /// The `clanhall` table (Java `ClanHall` ownership load) — id → owner/paidUntil.
    /// Overlaid onto the static hall defs on the game thread.
    ClanHallsLoaded { rows: Vec<ClanHallRow> },
    /// The `clanhall_auctions_bidders` table (Java `ClanHallAuction.loadBidder`) —
    /// the live bids per hall, restored at boot.
    ClanHallBiddersLoaded { rows: Vec<ClanHallBidRow> },
    /// The `residence_functions` table — active hall function upgrades, restored
    /// at boot.
    ResidenceFunctionsLoaded { rows: Vec<ResidenceFunctionRow> },
    /// `olympiad_data` (the single id=0 row) + all `olympiad_nobles`
    /// (Java `Olympiad.load`), loaded once at boot.
    OlympiadLoaded {
        current_cycle: i32,
        period: i32,
        olympiad_end: i64,
        validation_end: i64,
        next_weekly_change: i64,
        nobles: Vec<OlympiadNobleRow>,
        /// The last completed cycle's snapshot (`olympiad_nobles_eom`) — what
        /// the Olympiad Manager's class leaderboard shows.
        eom: Vec<OlympiadEomRow>,
    },
    /// The current heroes (`heroes` rows with `played = 1`) + every hero-diary
    /// entry (`heroes_diary`, `(charId, time, action, param)`), loaded at boot.
    HeroesLoaded {
        heroes: Vec<HeroRow>,
        diary: Vec<(i32, i64, i8, i32)>,
    },
    /// The `castle_siege_guards` table (the stationed garrison, `isHired=0`),
    /// pushed unprompted at boot. `(castle_id, spawn)`; grouped by castle on the
    /// game thread.
    SiegeGuardsLoaded {
        guards: Vec<(i32, crate::model::siege::SiegeSpawn)>,
    },
    /// The same table's `isHired = 1` rows — the mercenaries the owning clans
    /// posted between sieges. Pushed unprompted at boot beside the garrison.
    MercenariesLoaded {
        guards: Vec<(i32, crate::model::siege::SiegeSpawn)>,
    },
    /// The `buylists` table — the remaining stock of every limited-stock
    /// product that has been sold since its last restock. `BuyListData.load`
    /// reads it right after parsing the XML; pushed unprompted at boot here.
    /// `(list_id, item_id, count, next_restock_time)`.
    BuyListStockLoaded { rows: Vec<(i32, i32, i64, i64)> },
    /// The `castle_manor_production` + `castle_manor_procure` tables (Java
    /// `CastleManorManager.loadDb`), pushed unprompted at boot. Filtered to
    /// known seeds/crops and grouped by castle/period on the game thread.
    ManorLoaded {
        production: Vec<ManorProductionRow>,
        procure: Vec<ManorProcureRow>,
    },
}

/// The `messages` columns, flattened for binding. Booleans go to the DB as the
/// strings `'true'`/`'false'` exactly like Java (the column is an enum there).
#[derive(Debug, Clone)]
pub struct MailRow {
    pub message_id: i32,
    pub sender_id: i32,
    pub receiver_id: i32,
    pub subject: String,
    pub content: String,
    pub expiration: i64,
    pub req_adena: i64,
    pub has_attachments: bool,
    pub unread: bool,
    pub deleted_by_sender: bool,
    pub deleted_by_receiver: bool,
    pub send_by_system: i32,
    pub returned: bool,
}

/// One freighted item destined for an **offline** character's `items` rows
/// (`loc = FREIGHT`) — the cross-character package send.
#[derive(Debug, Clone)]
pub struct FreightItemRow {
    pub object_id: i32,
    pub item_id: i32,
    pub count: i64,
    pub enchant_level: i32,
}

/// One `castle_manor_production` row — a seed the manor sells.
#[derive(Debug, Clone)]
pub struct ManorProductionRow {
    pub castle_id: i32,
    pub seed_id: i32,
    pub amount: i64,
    pub start_amount: i64,
    pub price: i64,
    pub next_period: bool,
}

/// One `castle_manor_procure` row — a crop the manor buys back.
#[derive(Debug, Clone)]
pub struct ManorProcureRow {
    pub castle_id: i32,
    pub crop_id: i32,
    pub amount: i64,
    pub start_amount: i64,
    pub price: i64,
    pub reward_type: i32,
    pub next_period: bool,
}

/// One `siege_clans` row — a clan registered for a castle's siege.
#[derive(Debug, Clone)]
pub struct SiegeClanRow {
    pub castle_id: i32,
    pub clan_id: i32,
    pub kind: i32,
}

/// One `clanhall` row — a hall's persisted ownership.
#[derive(Debug, Clone)]
pub struct ClanHallRow {
    pub id: i32,
    pub owner_id: i32,
    pub paid_until: i64,
}

/// One `clanhall_auctions_bidders` row — a clan's standing bid.
#[derive(Debug, Clone)]
pub struct ClanHallBidRow {
    pub hall_id: i32,
    pub clan_id: i32,
    pub bid: i64,
    pub bid_time: i64,
}

/// One `residence_functions` row — an active hall function upgrade.
#[derive(Debug, Clone)]
pub struct ResidenceFunctionRow {
    pub residence_id: i32,
    pub func_id: i32,
    pub level: i32,
    pub expiration: i64,
}

/// One `olympiad_nobles` row — a noble's persisted Olympiad record.
#[derive(Debug, Clone)]
pub struct OlympiadNobleRow {
    pub char_id: i32,
    pub class_id: i32,
    pub points: i32,
    pub comp_done: i32,
    pub comp_won: i32,
    pub comp_lost: i32,
    pub comp_drawn: i32,
    pub comp_done_week: i32,
}

/// One pending `custom_mail` row — the table an operator writes into to hand a
/// character mail from outside the game.
#[derive(Debug, Clone)]
pub struct CustomMailRow {
    /// The timestamp column, half of the composite key. Kept as the string the
    /// DB returns so the delete matches byte-for-byte.
    pub date: String,
    pub receiver: i32,
    pub subject: String,
    pub message: String,
    /// `itemId count enchant;itemId count;itemId…` — see `parse_item_list`.
    pub items: String,
}

/// One `olympiad_nobles_eom` row — the end-of-cycle snapshot the Grand Olympiad
/// Manager's class leaderboard reads (`AltOlyShowMonthlyWinners = True` here, so
/// the board shows the *last completed* cycle rather than the live one). `name`
/// comes from the `characters` join Java's query does.
#[derive(Debug, Clone)]
pub struct OlympiadEomRow {
    pub class_id: i32,
    pub name: String,
    pub points: i32,
    pub comp_done: i32,
    pub comp_won: i32,
}

/// One `heroes` row (`played = 1`) — a currently-crowned hero.
#[derive(Debug, Clone)]
pub struct HeroRow {
    pub char_id: i32,
    pub class_id: i32,
    /// How many times this character has been a hero (Java `count`).
    pub count: i32,
    /// The hero's character name + clan id (for the `ExHeroList` display),
    /// resolved via a join at load and from the noble/player at crown time. Not
    /// persisted — the `heroes` table has no such columns.
    pub name: String,
    pub clan_id: i32,
    /// The hero's words (`heroes.message`), shown atop the hero diary window.
    pub message: String,
    /// `heroes.claimed` (a `'true'`/`'false'` string column): whether the crowned
    /// character has collected the status at the Monument of Heroes. Java's
    /// `Hero.isHero` — the predicate that grants hero status at login — is
    /// crowned **and** claimed; `isUnclaimedHero` is crowned and not.
    pub claimed: bool,
}

/// One `cursed_weapons` row — the persisted wielder state of a cursed weapon.
#[derive(Debug, Clone)]
pub struct CursedWeaponRow {
    pub item_id: i32,
    pub char_id: i32,
    pub player_reputation: i32,
    pub player_pk_kills: i32,
    pub nb_kills: i32,
    pub end_time: i64,
}

pub type CmdTx = tokio::sync::mpsc::UnboundedSender<DbCommand>;

#[cfg(test)]
mod size_guards {
    use super::*;

    /// Every [`DbCommand`] queued on the channel costs the size of the largest
    /// variant, so an id reservation used to occupy `StorePlayer`'s 608 B.
    /// Boxing that one field brought the enum to 200 B.
    #[test]
    fn db_command_stays_small() {
        let size = size_of::<DbCommand>();
        assert!(
            size <= 256,
            "DbCommand grew to {size} B — every queued command, however small, \
             pays this. Box the new large field."
        );
    }
}
