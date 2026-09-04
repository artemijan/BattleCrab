//! `DbCommand` — the game→DB half of the thread protocol. One variant per
//! write the game thread asks for; `db::commands` has the arm that runs it.

use super::super::ItemRow;
use super::rows::{
    BirthdayDay, FreightItemRow, GroundItemRow, HeroRow, MailRow, ManorProcureRow,
    ManorProductionRow, OlympiadNobleRow,
};
use super::save::{NewCharacter, PlayerSaveData};

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
    /// `ItemsOnGroundManager.run()` — truncate `itemsonground` and rewrite it
    /// from the live set. Java's periodic task does exactly this (empty, then
    /// insert), which is why it is one command rather than a diff.
    ///
    /// Cursed weapons are filtered on the game thread, matching Java's
    /// `isCursed` skip: `CursedWeaponsManager` owns their row and would
    /// otherwise double-save them.
    StoreGroundItems {
        items: Vec<GroundItemRow>,
    },
    /// `ItemsOnGroundManager.emptyTable()` on its own — the boot path for
    /// `!SaveDroppedItem && ClearDroppedItemTable`, and for
    /// `EmptyDroppedItemTableAfterLoad`.
    ClearGroundItems,
    /// `Config.AUTODELETE_INVALID_QUEST_DATA`'s half of
    /// `Quest.restoreQuestStates`: delete the `character_quests` rows naming a
    /// quest the server no longer has. Java deletes the state row and the
    /// variable rows with two statements over the same name.
    DeleteQuestRows {
        char_id: i32,
        quest_names: Vec<String>,
    },
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
    /// [`super::event::DbEvent::LotteryTicketsLoaded`].
    /// `CustomMailManager`'s poll: read every pending `custom_mail` row. The
    /// rows an operator (or a web shop) writes straight into the table are the
    /// whole interface — the game server only ever reads and deletes them.
    LoadCustomMail,
    /// `TaskBirthday`'s query: every character whose `create_date` ends in one
    /// of these `MM-DD` day keys, with the year that day belongs to (the task
    /// catches up a day at a time, so a run can carry several). Replies with
    /// [`super::event::DbEvent::BirthdaysLoaded`].
    LoadBirthdays {
        days: Vec<BirthdayDay>,
    },
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
