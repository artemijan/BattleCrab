//! The DB thread (CONCURRENCY_MODEL §2.4). A dedicated OS thread owns the SQLite
//! pool; the game thread never blocks on the database — it sends [`DbCommand`]s
//! and drains [`DbEvent`]s each tick. Character id allocation lives here too
//! (a minimal `IdManager`).

use std::thread::JoinHandle;

use models::entity::{
    account_gsdata, account_premium, bbs_favorites, buffer_schemes, castle, castle_manor_procure,
    castle_manor_production, castle_siege_guards, character_friends, character_hennas,
    character_macroses, character_quests, character_recipebook, character_reco_bonus,
    character_shortcuts, character_skills, character_skills_save, character_subclasses,
    character_summon_skills_save, character_summons, character_variables, characters, clan_data,
    clan_privs, clan_skills, clan_subpledges, clan_wars, clanhall, clanhall_auctions_bidders,
    crests, cursed_weapons, grandboss_data, heroes, heroes_diary, item_auction, item_auction_bid,
    item_variations, items, lottery, mdt_bets, mdt_history, messages, npc_respawns, olympiad_data,
    olympiad_nobles, petition_feedback, pets, pledge_applicant, pledge_recruit,
    pledge_waiting_list, punishments, residence_functions, siege_clans,
};
use models::sea_orm::ActiveValue::{NotSet, Set, Unchanged};
use models::sea_orm::Condition;
use models::sea_orm::sea_query::{CaseStatement, Expr, OnConflict};
use models::sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};
use tracing::{error, info, warn};

use crate::character::{CharData, ItemRow};
use commons::util::now_millis;

/// First object id handed out by `IdManager` (Java `FIRST_OID`). Shared by
/// every world-object type (characters, items, …) — Java's `IdManager` is a
/// single pool, not one per type.
const FIRST_OID: i64 = 0x10000000;

/// How many object ids each `IdBlock` reservation hands the game thread.
pub const ID_BLOCK_SIZE: i64 = 5000;

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
/// `storeEffect` (`character_skills_save`), item reuse — need systems that
/// don't exist yet (subclasses, buff restore on login) and are TODO(G-later).
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
        save: PlayerSaveData,
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
    /// `Castle.setSide`/`switchSide` — persist a castle's side.
    UpdateCastleSide {
        castle_id: i32,
        side: String,
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
    /// `RequestPackageSend` to an **offline** recipient — insert the freighted
    /// items straight into their `items` rows (`loc = FREIGHT`), since there is
    /// no live `Freight` component to write through. An online recipient's
    /// component is updated instead, so the two paths never both fire.
    AddFreightItems {
        owner_id: i32,
        items: Vec<FreightItemRow>,
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
        message: crate::db::MailRow,
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
        items: Vec<crate::character::ItemRow>,
    },
    /// Replace the `loc = 'MAIL'` item rows of one message (delete-then-insert,
    /// the house style for a whole container).
    StoreMailItems {
        message_id: i32,
        owner_id: i32,
        items: Vec<crate::character::ItemRow>,
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
    },
    /// The whole `npc_respawns` table (Java `DBSpawnManager.load`), pushed
    /// unprompted at boot. See [`NpcRespawnRow`].
    NpcRespawnsLoaded { rows: Vec<NpcRespawnRow> },
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
        attachments: Vec<(i32, Vec<crate::character::ItemRow>)>,
        char_ids_by_name: Vec<(String, i32)>,
    },
    /// The active punishments (Java `PunishmentManager.load`, G31), pushed
    /// unprompted at boot. Already-expired rows are filtered out here; `next_id`
    /// seeds the game-thread id allocator past the highest loaded id.
    PunishmentsLoaded {
        next_id: i32,
        punishments: Vec<crate::model::punishment::Punishment>,
    },
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
pub type CmdRx = tokio::sync::mpsc::UnboundedReceiver<DbCommand>;
pub type EventTx = std::sync::mpsc::Sender<DbEvent>;
pub type DbEventRx = std::sync::mpsc::Receiver<DbEvent>;

/// Spawn the DB thread. It creates and owns the pool on its own runtime.
pub fn spawn(
    url: String,
    max_connections: u32,
    max_characters: i32,
    cmd_rx: CmdRx,
    event_tx: EventTx,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("db-thread".to_string())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("db thread runtime");
            rt.block_on(run(url, max_connections, max_characters, cmd_rx, event_tx));
        })
        .expect("failed to spawn db thread")
}

/// Confirms the pool actually points at a game database.
///
/// `characters` and `accounts` are the two tables the server cannot run without
/// and that no other database on the box would have together, which makes them
/// a cheap and unambiguous fingerprint.
async fn verify_schema(db: &DatabaseConnection) -> Result<(), String> {
    let mut missing = Vec::new();
    for table in ["characters", "accounts"] {
        let found = db
            .query_one_raw(models::sea_orm::Statement::from_sql_and_values(
                models::sea_orm::DatabaseBackend::Sqlite,
                "SELECT name FROM sqlite_master WHERE type='table' AND name = ?",
                [table.into()],
            ))
            .await
            .map_err(|e| format!("cannot inspect database schema: {e}"))?;
        if found.is_none() {
            missing.push(table);
        }
    }
    if missing.is_empty() {
        return Ok(());
    }
    Err(format!(
        "database is missing required table(s): {}",
        missing.join(", ")
    ))
}

async fn run(
    url: String,
    max_connections: u32,
    max_characters: i32,
    mut cmd_rx: CmdRx,
    event_tx: EventTx,
) {
    let db = match commons::db::connect(&url, max_connections).await {
        Ok(db) => db,
        Err(e) => {
            error!("DB thread: failed to open database: {e}");
            return;
        }
    };

    // `create_if_missing(true)` means a wrong path does not fail — SQLite
    // happily makes an empty database, and the server then runs against it,
    // losing every character and account silently. Catch that here rather than
    // letting it surface hours later as "no such table".
    //
    // The usual cause is a relative `URL` resolving somewhere unexpected. It is
    // resolved against the executable's directory, so the database belongs
    // beside the binary — not in the datapack, and not in whatever directory
    // the unit file happened to start the process in.
    if let Err(e) = verify_schema(&db).await {
        error!(
            "DB thread: {e}\n  URL = {url}\n  relative paths resolve next to the executable, in {}\n\
             This is not the game database. Put it beside the binary (the same file the login \
             server opens), or make the URL absolute.",
            commons::db::executable_dir().display(),
        );
        return;
    }

    let mut next_id = load_next_id(&db).await;

    // Hand the game thread its initial runtime-id block unprompted (it can't
    // ask before it knows the DB thread is up; see `DbCommand::ReserveIds`).
    let _ = event_tx.send(DbEvent::IdBlock {
        start: next_id,
        end: next_id + ID_BLOCK_SIZE,
    });
    next_id += ID_BLOCK_SIZE;

    // Premium table cache, before clans so `ClansLoaded` stays the last boot
    // event (the game loop releases the login link on it).
    let _ = event_tx.send(DbEvent::PremiumLoaded {
        entries: load_premium(&db).await,
    });

    // `SchemeBufferTable.load` — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::BufferSchemesLoaded {
        entries: load_buffer_schemes(&db).await,
    });

    // Last lottery round (Java `Lottery.startLottery`'s restore) + the drawn
    // rounds cache — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::LotteryLoaded {
        row: load_lottery(&db).await,
        draws: load_lottery_draws(&db).await,
    });

    // Monster Race history + lane bets (Java `MonsterRace` constructor) —
    // likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::MdtLoaded {
        history: load_mdt_history(&db).await,
        bets: load_mdt_bets(&db).await,
    });

    // Item auctions + bids (Java `ItemAuctionManager` boot load, G30.5) —
    // likewise unprompted, before `ClansLoaded`.
    let (next_auction_id, auctions) = load_item_auctions(&db).await;
    let _ = event_tx.send(DbEvent::ItemAuctionsLoaded {
        next_auction_id,
        auctions,
    });

    // Mail + attachments + the offline name->id table (Java `MailManager.load`
    // and `CharInfoTable`, G30) — likewise unprompted, before `ClansLoaded`.
    let (messages, attachments) = load_mail(&db).await;
    let _ = event_tx.send(DbEvent::MailLoaded {
        messages,
        attachments,
        char_ids_by_name: load_char_ids_by_name(&db).await,
    });

    // Active punishments (Java `PunishmentManager.load`, G31) — likewise
    // unprompted, before `ClansLoaded`.
    let (next_punishment_id, punishments) = load_punishments(&db).await;
    let _ = event_tx.send(DbEvent::PunishmentsLoaded {
        next_id: next_punishment_id,
        punishments,
    });

    // `FavoriteBoard` favorites cache — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::FavoritesLoaded {
        entries: load_favorites(&db).await,
    });

    // `DBSpawnManager.load` — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::NpcRespawnsLoaded {
        rows: load_npc_respawns(&db).await,
    });

    // `GrandBossManager.init` — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::GrandBossesLoaded {
        bosses: load_grandboss_data(&db).await,
    });

    // `CursedWeaponsManager.restore` — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::CursedWeaponsLoaded {
        rows: load_cursed_weapons(&db).await,
    });

    // `CastleManager.load` — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::CastlesLoaded {
        castles: load_castles(&db).await,
    });

    // `Siege.loadSiegeClan` — after castles (the game loop keys sieges off them).
    let _ = event_tx.send(DbEvent::SiegesLoaded {
        rows: load_siege_clans(&db).await,
    });

    // `CastleManorManager.loadDb` — the manor production/procure state.
    let _ = event_tx.send(DbEvent::ManorLoaded {
        production: load_manor_production(&db).await,
        procure: load_manor_procure(&db).await,
    });

    // Clan-hall ownership — overlaid onto the static hall defs at boot.
    let _ = event_tx.send(DbEvent::ClanHallsLoaded {
        rows: load_clan_hall_owners(&db).await,
    });

    // Clan-hall auction bids — restored so escrowed adena stays accounted for.
    let _ = event_tx.send(DbEvent::ClanHallBiddersLoaded {
        rows: load_clan_hall_bidders(&db).await,
    });

    // Active clan-hall function upgrades.
    let _ = event_tx.send(DbEvent::ResidenceFunctionsLoaded {
        rows: load_residence_functions(&db).await,
    });

    // `Olympiad.load` — the period/cycle row + every noble's record.
    let _ = event_tx.send(load_olympiad(&db).await);

    // `Hero.init` — the currently-crowned heroes (`played = 1`) + their diaries.
    let _ = event_tx.send(DbEvent::HeroesLoaded {
        heroes: load_heroes(&db).await,
        diary: load_hero_diary(&db).await,
    });

    // `SiegeGuardManager` — the stationed siege guards, spawned at siege start.
    let _ = event_tx.send(DbEvent::SiegeGuardsLoaded {
        guards: load_siege_guards(&db).await,
    });

    // `ClanTable`'s boot restore, likewise unprompted.
    let _ = event_tx.send(DbEvent::ClansLoaded {
        clans: load_clans(&db).await,
        wars: load_clan_wars(&db).await,
        crests: load_crests(&db).await,
        recruit_clans: load_recruit_clans(&db).await,
        recruit_waiting: load_recruit_waiting(&db).await,
        recruit_applicants: load_recruit_applicants(&db).await,
    });

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            DbCommand::LoadCharacters { client_id, account } => {
                reload(&db, &event_tx, client_id, account, true).await;
            }
            DbCommand::CreateCharacter { client_id, data } => {
                let result = create_character(&db, &mut next_id, max_characters, &data).await;
                let _ = event_tx.send(DbEvent::CharacterCreated { client_id, result });
                if result == CreateResult::Ok {
                    // Java caches the list after creation but does not re-send it.
                    reload(&db, &event_tx, client_id, data.account, false).await;
                }
            }
            DbCommand::MarkDelete {
                client_id,
                account,
                char_id,
                delete_time,
            } => {
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::Deletetime, delete_time.into())
                        .filter(characters::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
                reload(&db, &event_tx, client_id, account, true).await;
            }
            DbCommand::RestoreCharacter {
                client_id,
                account,
                char_id,
            } => {
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::Deletetime, 0.into())
                        .filter(characters::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
                reload(&db, &event_tx, client_id, account, true).await;
            }
            DbCommand::DeleteCharacter { char_id } => {
                delete_char(&db, char_id).await;
            }
            DbCommand::StoreGrandBoss { boss } => {
                warn_err(
                    grandboss_data::Entity::update_many()
                        .col_expr(grandboss_data::Column::LocX, boss.loc_x.into())
                        .col_expr(grandboss_data::Column::LocY, boss.loc_y.into())
                        .col_expr(grandboss_data::Column::LocZ, boss.loc_z.into())
                        .col_expr(grandboss_data::Column::Heading, boss.heading.into())
                        .col_expr(
                            grandboss_data::Column::RespawnTime,
                            boss.respawn_time.into(),
                        )
                        .col_expr(grandboss_data::Column::CurrentHp, boss.current_hp.into())
                        .col_expr(grandboss_data::Column::CurrentMp, boss.current_mp.into())
                        .col_expr(grandboss_data::Column::Status, boss.status.into())
                        .filter(grandboss_data::Column::BossId.eq(boss.boss_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::DeletePetRow { collar_object_id } => {
                warn_err(pets::Entity::delete_by_id(collar_object_id).exec(&db).await);
            }
            DbCommand::CountCharacters { account } => {
                let (count, del_times) = count_characters(&db, &account).await;
                let _ = event_tx.send(DbEvent::CharCount {
                    account,
                    count,
                    del_times,
                });
            }
            DbCommand::CheckNameCreatable { client_id, name } => {
                // RequestCharacterNameCreatable: NAME_ALREADY_EXISTS=2,
                // INVALID_LENGTH=3, creatable=-1 (validity was checked already).
                let result = if name_exists(&db, &name).await {
                    2
                } else if name.chars().count() > 16 {
                    3
                } else {
                    -1
                };
                let _ = event_tx.send(DbEvent::NameCreatable { client_id, result });
            }
            DbCommand::StorePlayer { save } => {
                store_player(&db, &save).await;
            }
            DbCommand::ReserveIds { count } => {
                let _ = event_tx.send(DbEvent::IdBlock {
                    start: next_id,
                    end: next_id + count,
                });
                next_id += count;
            }
            DbCommand::InsertFriendPair { a, b } => {
                // Both directions in one statement, as Java's two-row INSERT does.
                warn_err(
                    character_friends::Entity::insert_many([
                        character_friends::ActiveModel {
                            char_id: Set(a),
                            friend_id: Set(b),
                            relation: Set(0),
                            memo: NotSet,
                        },
                        character_friends::ActiveModel {
                            char_id: Set(b),
                            friend_id: Set(a),
                            relation: Set(0),
                            memo: NotSet,
                        },
                    ])
                    .on_conflict(
                        OnConflict::columns([
                            character_friends::Column::CharId,
                            character_friends::Column::FriendId,
                        ])
                        .do_nothing()
                        .to_owned(),
                    )
                    .exec_without_returning(&db)
                    .await,
                );
            }
            DbCommand::DeleteFriendPair { a, b } => {
                warn_err(
                    character_friends::Entity::delete_many()
                        .filter(
                            Condition::any()
                                .add(
                                    Condition::all()
                                        .add(character_friends::Column::CharId.eq(a))
                                        .add(character_friends::Column::FriendId.eq(b)),
                                )
                                .add(
                                    Condition::all()
                                        .add(character_friends::Column::CharId.eq(b))
                                        .add(character_friends::Column::FriendId.eq(a)),
                                ),
                        )
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::InsertClan {
                clan_id,
                name,
                leader_id,
            } => {
                warn_err(
                    clan_data::Entity::insert(clan_data::ActiveModel {
                        clan_id: Set(clan_id),
                        clan_name: Set(Some(name)),
                        clan_level: Set(Some(0)),
                        has_castle: Set(Some(0)),
                        blood_alliance_count: Set(0),
                        blood_oath_count: Set(0),
                        ally_id: Set(Some(0)),
                        ally_name: Set(None),
                        leader_id: Set(Some(leader_id)),
                        crest_id: Set(Some(0)),
                        crest_large_id: Set(Some(0)),
                        ally_crest_id: Set(Some(0)),
                        new_leader_id: Set(0),
                        ..Default::default()
                    })
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::UpdateCharClan {
                char_id,
                clan_id,
                clan_privs,
            } => {
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::Clanid, clan_id.into())
                        .col_expr(characters::Column::ClanPrivs, clan_privs.into())
                        .filter(characters::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::SaveClanSkill {
                clan_id,
                skill_id,
                skill_level,
                skill_name,
            } => {
                warn_err(
                    clan_skills::Entity::insert(clan_skills::ActiveModel {
                        clan_id: Set(clan_id),
                        skill_id: Set(skill_id),
                        skill_level: Set(skill_level),
                        skill_name: Set(Some(skill_name)),
                        sub_pledge_id: Set(-2),
                    })
                    .on_conflict(
                        OnConflict::columns([
                            clan_skills::Column::ClanId,
                            clan_skills::Column::SkillId,
                            clan_skills::Column::SubPledgeId,
                        ])
                        .update_columns([
                            clan_skills::Column::SkillLevel,
                            clan_skills::Column::SkillName,
                        ])
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeleteClanSkill { clan_id, skill_id } => {
                warn_err(
                    clan_skills::Entity::delete_many()
                        .filter(clan_skills::Column::ClanId.eq(clan_id))
                        .filter(clan_skills::Column::SkillId.eq(skill_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::StoreCursedWeapon {
                item_id,
                char_id,
                reputation,
                pk_kills,
                nb_kills,
                end_time,
            } => {
                warn_err(
                    cursed_weapons::Entity::insert(cursed_weapons::ActiveModel {
                        item_id: Set(item_id),
                        char_id: Set(char_id),
                        player_reputation: Set(Some(reputation)),
                        player_pk_kills: Set(Some(pk_kills)),
                        nb_kills: Set(Some(nb_kills)),
                        end_time: Set(end_time),
                    })
                    .on_conflict(
                        OnConflict::column(cursed_weapons::Column::ItemId)
                            .update_columns([
                                cursed_weapons::Column::CharId,
                                cursed_weapons::Column::PlayerReputation,
                                cursed_weapons::Column::PlayerPkKills,
                                cursed_weapons::Column::NbKills,
                                cursed_weapons::Column::EndTime,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::StoreNpcRespawn {
                npc_id,
                x,
                y,
                z,
                heading,
                respawn_time,
                cur_hp,
                cur_mp,
            } => {
                warn_err(
                    npc_respawns::Entity::insert(npc_respawns::ActiveModel {
                        id: Set(npc_id),
                        x: Set(x),
                        y: Set(y),
                        z: Set(z),
                        heading: Set(heading),
                        respawn_time: Set(respawn_time),
                        current_hp: Set(cur_hp),
                        current_mp: Set(cur_mp),
                    })
                    .on_conflict(
                        OnConflict::column(npc_respawns::Column::Id)
                            .update_columns([
                                npc_respawns::Column::X,
                                npc_respawns::Column::Y,
                                npc_respawns::Column::Z,
                                npc_respawns::Column::Heading,
                                npc_respawns::Column::RespawnTime,
                                npc_respawns::Column::CurrentHp,
                                npc_respawns::Column::CurrentMp,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::StoreSubClass {
                char_id,
                class_id,
                class_index,
                level,
                exp,
                sp,
            } => {
                warn_err(
                    character_subclasses::Entity::insert(character_subclasses::ActiveModel {
                        char_id: Set(char_id),
                        class_id: Set(class_id),
                        exp: Set(exp),
                        sp: Set(sp),
                        level: Set(level),
                        vitality_points: Set(0),
                        class_index: Set(class_index),
                        dual_class: Set(0),
                    })
                    .on_conflict(
                        OnConflict::columns([
                            character_subclasses::Column::CharId,
                            character_subclasses::Column::ClassId,
                        ])
                        .update_columns([
                            character_subclasses::Column::Exp,
                            character_subclasses::Column::Sp,
                            character_subclasses::Column::Level,
                            character_subclasses::Column::ClassIndex,
                        ])
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeleteNpcRespawn { npc_id } => {
                warn_err(npc_respawns::Entity::delete_by_id(npc_id).exec(&db).await);
            }
            DbCommand::RemoveCursedWeapon { item_id } => {
                warn_err(
                    cursed_weapons::Entity::delete_by_id(item_id)
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateCastleSide { castle_id, side } => {
                warn_err(
                    castle::Entity::update_many()
                        .col_expr(castle::Column::Side, side.into())
                        .filter(castle::Column::Id.eq(castle_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanLeader { clan_id, leader_id } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::LeaderId, leader_id.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanCastle { clan_id, castle_id } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::HasCastle, castle_id.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanBloodAlliance { clan_id, count } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::BloodAllianceCount, count.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateCastleTicketCount { castle_id, count } => {
                warn_err(
                    castle::Entity::update_many()
                        .col_expr(castle::Column::TicketBuyCount, count.into())
                        .filter(castle::Column::Id.eq(castle_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::AddFreightItems { owner_id, items } => {
                for it in &items {
                    warn_err(
                        items::Entity::insert(items::ActiveModel {
                            owner_id: Set(Some(owner_id)),
                            object_id: Set(it.object_id),
                            item_id: Set(Some(it.item_id)),
                            count: Set(it.count),
                            enchant_level: Set(Some(it.enchant_level)),
                            loc: Set(Some("FREIGHT".to_string())),
                            loc_data: Set(Some(0)),
                            custom_type1: Set(Some(0)),
                            custom_type2: Set(Some(0)),
                            mana_left: Set(-1),
                            time: Set(0),
                            ..Default::default()
                        })
                        .exec(&db)
                        .await,
                    );
                }
            }
            DbCommand::StoreManor {
                castle_id,
                production,
                procure,
            } => {
                warn_err(
                    castle_manor_production::Entity::delete_many()
                        .filter(castle_manor_production::Column::CastleId.eq(castle_id))
                        .exec(&db)
                        .await,
                );
                for r in &production {
                    warn_err(
                        castle_manor_production::Entity::insert(
                            castle_manor_production::ActiveModel {
                                castle_id: Set(r.castle_id),
                                seed_id: Set(r.seed_id),
                                amount: Set(r.amount as i32),
                                start_amount: Set(r.start_amount as i32),
                                price: Set(r.price as i32),
                                next_period: Set(i32::from(r.next_period)),
                            },
                        )
                        .exec(&db)
                        .await,
                    );
                }
                warn_err(
                    castle_manor_procure::Entity::delete_many()
                        .filter(castle_manor_procure::Column::CastleId.eq(castle_id))
                        .exec(&db)
                        .await,
                );
                for r in &procure {
                    warn_err(
                        castle_manor_procure::Entity::insert(castle_manor_procure::ActiveModel {
                            castle_id: Set(r.castle_id),
                            crop_id: Set(r.crop_id),
                            amount: Set(r.amount as i32),
                            start_amount: Set(r.start_amount as i32),
                            price: Set(r.price as i32),
                            reward_type: Set(r.reward_type),
                            next_period: Set(i32::from(r.next_period)),
                        })
                        .exec(&db)
                        .await,
                    );
                }
            }
            DbCommand::UpdateCastleTreasury {
                castle_id,
                treasury,
            } => {
                warn_err(
                    castle::Entity::update_many()
                        .col_expr(castle::Column::Treasury, treasury.into())
                        .filter(castle::Column::Id.eq(castle_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateCastleSiegeTime {
                castle_id,
                siege_date,
                time_registration_over,
            } => {
                // `regTimeOver` is an enum('true','false') stored as text.
                let flag = if time_registration_over {
                    "true"
                } else {
                    "false"
                };
                warn_err(
                    castle::Entity::update_many()
                        .col_expr(castle::Column::SiegeDate, siege_date.into())
                        .col_expr(castle::Column::RegTimeOver, flag.into())
                        .filter(castle::Column::Id.eq(castle_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::SaveSiegeClan {
                castle_id,
                clan_id,
                kind,
            } => {
                warn_err(
                    siege_clans::Entity::insert(siege_clans::ActiveModel {
                        clan_id: Set(clan_id),
                        castle_id: Set(castle_id),
                        r#type: Set(Some(kind)),
                        castle_owner: Set(Some(0)),
                    })
                    .on_conflict(
                        OnConflict::columns([
                            siege_clans::Column::ClanId,
                            siege_clans::Column::CastleId,
                        ])
                        .update_columns([
                            siege_clans::Column::Type,
                            siege_clans::Column::CastleOwner,
                        ])
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::RemoveSiegeClan { castle_id, clan_id } => {
                warn_err(
                    siege_clans::Entity::delete_many()
                        .filter(siege_clans::Column::CastleId.eq(castle_id))
                        .filter(siege_clans::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::SaveClanHallBid {
                hall_id,
                clan_id,
                bid,
                bid_time,
            } => {
                warn_err(
                    clanhall_auctions_bidders::Entity::insert(
                        clanhall_auctions_bidders::ActiveModel {
                            clan_hall_id: Set(hall_id),
                            clan_id: Set(clan_id),
                            bid: Set(bid),
                            bid_time: Set(bid_time),
                        },
                    )
                    .on_conflict(
                        OnConflict::columns([
                            clanhall_auctions_bidders::Column::ClanHallId,
                            clanhall_auctions_bidders::Column::ClanId,
                        ])
                        .update_columns([
                            clanhall_auctions_bidders::Column::Bid,
                            clanhall_auctions_bidders::Column::BidTime,
                        ])
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::RemoveClanHallBid { hall_id, clan_id } => {
                warn_err(
                    clanhall_auctions_bidders::Entity::delete_by_id((hall_id, clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::ClearClanHallBids { hall_id } => {
                warn_err(
                    clanhall_auctions_bidders::Entity::delete_many()
                        .filter(clanhall_auctions_bidders::Column::ClanHallId.eq(hall_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::SaveClanHall {
                id,
                owner_id,
                paid_until,
            } => {
                warn_err(
                    clanhall::Entity::insert(clanhall::ActiveModel {
                        id: Set(id),
                        owner_id: Set(owner_id),
                        paid_until: Set(paid_until),
                    })
                    .on_conflict(
                        OnConflict::column(clanhall::Column::Id)
                            .update_columns([
                                clanhall::Column::OwnerId,
                                clanhall::Column::PaidUntil,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::SaveResidenceFunction {
                residence_id,
                func_id,
                level,
                expiration,
            } => {
                warn_err(
                    residence_functions::Entity::insert(residence_functions::ActiveModel {
                        id: Set(func_id),
                        level: Set(level),
                        expiration: Set(expiration),
                        residence_id: Set(residence_id),
                    })
                    .on_conflict(
                        OnConflict::columns([
                            residence_functions::Column::Id,
                            residence_functions::Column::Level,
                            residence_functions::Column::ResidenceId,
                        ])
                        .update_column(residence_functions::Column::Expiration)
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::RemoveResidenceFunction {
                residence_id,
                func_id,
            } => {
                warn_err(
                    residence_functions::Entity::delete_many()
                        .filter(residence_functions::Column::ResidenceId.eq(residence_id))
                        .filter(residence_functions::Column::Id.eq(func_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::SaveOlympiad {
                current_cycle,
                period,
                olympiad_end,
                validation_end,
                next_weekly_change,
                nobles,
            } => {
                warn_err(
                    olympiad_data::Entity::insert(olympiad_data::ActiveModel {
                        id: Set(0),
                        current_cycle: Set(current_cycle),
                        period: Set(period),
                        olympiad_end: Set(olympiad_end),
                        validation_end: Set(validation_end),
                        next_weekly_change: Set(next_weekly_change),
                    })
                    .on_conflict(
                        OnConflict::column(olympiad_data::Column::Id)
                            .update_columns([
                                olympiad_data::Column::CurrentCycle,
                                olympiad_data::Column::Period,
                                olympiad_data::Column::OlympiadEnd,
                                olympiad_data::Column::ValidationEnd,
                                olympiad_data::Column::NextWeeklyChange,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
                for n in nobles {
                    warn_err(
                        olympiad_nobles::Entity::insert(olympiad_nobles::ActiveModel {
                            char_id: Set(n.char_id),
                            class_id: Set(n.class_id),
                            olympiad_points: Set(n.points),
                            competitions_done: Set(n.comp_done),
                            competitions_won: Set(n.comp_won),
                            competitions_lost: Set(n.comp_lost),
                            competitions_drawn: Set(n.comp_drawn),
                            competitions_done_week: Set(n.comp_done_week),
                        })
                        .on_conflict(
                            OnConflict::column(olympiad_nobles::Column::CharId)
                                .update_columns([
                                    olympiad_nobles::Column::ClassId,
                                    olympiad_nobles::Column::OlympiadPoints,
                                    olympiad_nobles::Column::CompetitionsDone,
                                    olympiad_nobles::Column::CompetitionsWon,
                                    olympiad_nobles::Column::CompetitionsLost,
                                    olympiad_nobles::Column::CompetitionsDrawn,
                                    olympiad_nobles::Column::CompetitionsDoneWeek,
                                ])
                                .to_owned(),
                        )
                        .exec(&db)
                        .await,
                    );
                }
            }
            DbCommand::SaveHeroes { heroes } => {
                // `Hero.computeNewHeroes` replaces the active crown.
                warn_err(heroes::Entity::delete_many().exec(&db).await);
                for h in heroes {
                    warn_err(
                        heroes::Entity::insert(heroes::ActiveModel {
                            char_id: Set(h.char_id),
                            class_id: Set(h.class_id),
                            count: Set(h.count),
                            played: Set(1),
                            claimed: Set("false".to_string()),
                            ..Default::default()
                        })
                        .on_conflict(
                            OnConflict::column(heroes::Column::CharId)
                                .update_columns([
                                    heroes::Column::ClassId,
                                    heroes::Column::Count,
                                    heroes::Column::Played,
                                    heroes::Column::Claimed,
                                ])
                                .to_owned(),
                        )
                        .exec(&db)
                        .await,
                    );
                }
            }
            DbCommand::SaveHeroDiary {
                char_id,
                time,
                action,
                param,
            } => {
                warn_err(
                    heroes_diary::Entity::insert(heroes_diary::ActiveModel {
                        char_id: Set(char_id),
                        time: Set(time),
                        action: Set(action),
                        param: Set(param),
                    })
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::UpdateClanLevel { clan_id, level } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::ClanLevel, level.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanReputation {
                clan_id,
                reputation,
            } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::ReputationScore, reputation.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanPenalties {
                clan_id,
                char_penalty_expiry_time,
                dissolving_expiry_time,
            } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(
                            clan_data::Column::CharPenaltyExpiryTime,
                            char_penalty_expiry_time.into(),
                        )
                        .col_expr(
                            clan_data::Column::DissolvingExpiryTime,
                            dissolving_expiry_time.into(),
                        )
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::RemoveClanMember {
                char_id,
                clan_join_expiry,
                clan_create_expiry,
            } => {
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::Clanid, 0.into())
                        .col_expr(characters::Column::Title, "".into())
                        .col_expr(characters::Column::ClanPrivs, 0.into())
                        .col_expr(
                            characters::Column::ClanJoinExpiryTime,
                            clan_join_expiry.into(),
                        )
                        .col_expr(
                            characters::Column::ClanCreateExpiryTime,
                            clan_create_expiry.into(),
                        )
                        .filter(characters::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::SaveClanRankPrivs {
                clan_id,
                rank,
                privs,
            } => {
                warn_err(
                    clan_privs::Entity::insert(clan_privs::ActiveModel {
                        clan_id: Set(clan_id),
                        rank: Set(rank),
                        party: Set(0),
                        privs: Set(privs),
                    })
                    .on_conflict(
                        OnConflict::columns([
                            clan_privs::Column::ClanId,
                            clan_privs::Column::Rank,
                            clan_privs::Column::Party,
                        ])
                        .update_column(clan_privs::Column::Privs)
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::UpdateCharPowerGrade {
                char_id,
                power_grade,
            } => {
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::PowerGrade, power_grade.into())
                        .filter(characters::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanAlly {
                clan_id,
                ally_id,
                ally_name,
                penalty_expiry,
                penalty_type,
            } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::AllyId, ally_id.into())
                        .col_expr(clan_data::Column::AllyName, ally_name.into())
                        .col_expr(
                            clan_data::Column::AllyPenaltyExpiryTime,
                            penalty_expiry.into(),
                        )
                        .col_expr(clan_data::Column::AllyPenaltyType, penalty_type.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::InsertSubPledge {
                clan_id,
                pledge_type,
                name,
                leader_id,
            } => {
                warn_err(
                    clan_subpledges::Entity::insert(clan_subpledges::ActiveModel {
                        clan_id: Set(clan_id),
                        sub_pledge_id: Set(pledge_type),
                        name: Set(Some(name)),
                        leader_id: Set(leader_id),
                    })
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::UpdateSubPledge {
                clan_id,
                pledge_type,
                name,
                leader_id,
            } => {
                warn_err(
                    clan_subpledges::Entity::update_many()
                        .col_expr(clan_subpledges::Column::LeaderId, leader_id.into())
                        .col_expr(clan_subpledges::Column::Name, name.into())
                        .filter(clan_subpledges::Column::ClanId.eq(clan_id))
                        .filter(clan_subpledges::Column::SubPledgeId.eq(pledge_type))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateCharPledgeType {
                char_id,
                pledge_type,
            } => {
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::Subpledge, pledge_type.into())
                        .filter(characters::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::InsertCrest { id, data, kind } => {
                warn_err(
                    crests::Entity::insert(crests::ActiveModel {
                        crest_id: Set(id),
                        data: Set(data),
                        r#type: Set(kind),
                    })
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeleteCrest { id } => {
                warn_err(
                    crests::Entity::delete_many()
                        .filter(crests::Column::CrestId.eq(id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanCrest { clan_id, crest_id } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::CrestId, crest_id.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanCrestLarge {
                clan_id,
                crest_large_id,
            } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::CrestLargeId, crest_large_id.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanAllyCrestSelf {
                clan_id,
                ally_crest_id,
            } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::AllyCrestId, ally_crest_id.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateAllyCrestForAlliance {
                ally_id,
                ally_crest_id,
            } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::AllyCrestId, ally_crest_id.into())
                        .filter(clan_data::Column::AllyId.eq(ally_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpsertPledgeApplicant {
                player_id,
                clan_id,
                karma,
                message,
            } => {
                warn_err(
                    pledge_applicant::Entity::insert(pledge_applicant::ActiveModel {
                        char_id: Set(player_id),
                        clan_id: Set(clan_id),
                        karma: Set(karma),
                        message: Set(message),
                    })
                    .on_conflict(
                        OnConflict::columns([
                            pledge_applicant::Column::CharId,
                            pledge_applicant::Column::ClanId,
                        ])
                        .update_columns([
                            pledge_applicant::Column::Karma,
                            pledge_applicant::Column::Message,
                        ])
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeletePledgeApplicant { player_id, clan_id } => {
                warn_err(
                    pledge_applicant::Entity::delete_by_id((player_id, clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::InsertPledgeWaiting { player_id, karma } => {
                warn_err(
                    pledge_waiting_list::Entity::insert(pledge_waiting_list::ActiveModel {
                        char_id: Set(player_id),
                        karma: Set(karma),
                    })
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeletePledgeWaiting { player_id } => {
                warn_err(
                    pledge_waiting_list::Entity::delete_many()
                        .filter(pledge_waiting_list::Column::CharId.eq(player_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::InsertPledgeRecruit {
                clan_id,
                karma,
                information,
                detailed_information,
                application_type,
                recruit_type,
            } => {
                warn_err(
                    pledge_recruit::Entity::insert(pledge_recruit::ActiveModel {
                        clan_id: Set(clan_id),
                        karma: Set(karma),
                        information: Set(information),
                        detailed_information: Set(detailed_information),
                        application_type: Set(application_type),
                        recruit_type: Set(recruit_type),
                    })
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::UpdatePledgeRecruit {
                clan_id,
                karma,
                information,
                detailed_information,
                application_type,
                recruit_type,
            } => {
                warn_err(
                    pledge_recruit::Entity::update_many()
                        .col_expr(pledge_recruit::Column::Karma, karma.into())
                        .col_expr(pledge_recruit::Column::Information, information.into())
                        .col_expr(
                            pledge_recruit::Column::DetailedInformation,
                            detailed_information.into(),
                        )
                        .col_expr(
                            pledge_recruit::Column::ApplicationType,
                            application_type.into(),
                        )
                        .col_expr(pledge_recruit::Column::RecruitType, recruit_type.into())
                        .filter(pledge_recruit::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::DeletePledgeRecruit { clan_id } => {
                warn_err(
                    pledge_recruit::Entity::delete_many()
                        .filter(pledge_recruit::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::SaveClanWar {
                attacker,
                attacked,
                attacker_kills,
                attacked_kills,
                winner,
                start_time,
                end_time,
                state,
            } => {
                // The clan-id columns are `varchar(35)`; SQLite stored the bound
                // integers as text anyway, and `load_clan_wars` parses them back.
                warn_err(
                    clan_wars::Entity::insert(clan_wars::ActiveModel {
                        clan1: Set(attacker.to_string()),
                        clan2: Set(attacked.to_string()),
                        clan1_kill: Set(attacker_kills),
                        clan2_kill: Set(attacked_kills),
                        winner_clan: Set(winner.to_string()),
                        start_time: Set(start_time),
                        end_time: Set(end_time),
                        state: Set(state),
                    })
                    .on_conflict(
                        OnConflict::columns([clan_wars::Column::Clan1, clan_wars::Column::Clan2])
                            .update_columns([
                                clan_wars::Column::Clan1Kill,
                                clan_wars::Column::Clan2Kill,
                                clan_wars::Column::WinnerClan,
                                clan_wars::Column::StartTime,
                                clan_wars::Column::EndTime,
                                clan_wars::Column::State,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeleteClanWar { clan1, clan2 } => {
                let (a, b) = (clan1.to_string(), clan2.to_string());
                warn_err(
                    clan_wars::Entity::delete_many()
                        .filter(
                            Condition::any()
                                .add(
                                    Condition::all()
                                        .add(clan_wars::Column::Clan1.eq(a.clone()))
                                        .add(clan_wars::Column::Clan2.eq(b.clone())),
                                )
                                .add(
                                    Condition::all()
                                        .add(clan_wars::Column::Clan1.eq(b))
                                        .add(clan_wars::Column::Clan2.eq(a)),
                                ),
                        )
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateClanNewLeader {
                clan_id,
                new_leader_id,
            } => {
                warn_err(
                    clan_data::Entity::update_many()
                        .col_expr(clan_data::Column::NewLeaderId, new_leader_id.into())
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::UpdateCharClanJoinExpiry { char_id, expiry } => {
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::ClanJoinExpiryTime, expiry.into())
                        .filter(characters::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::DestroyClan {
                clan_id,
                leader_id,
                leader_expiry,
            } => {
                warn_err(
                    clan_data::Entity::delete_many()
                        .filter(clan_data::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
                warn_err(
                    clan_skills::Entity::delete_many()
                        .filter(clan_skills::Column::ClanId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::Clanid, 0.into())
                        .col_expr(characters::Column::ClanPrivs, 0.into())
                        .filter(characters::Column::Clanid.eq(clan_id))
                        .exec(&db)
                        .await,
                );
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(
                            characters::Column::ClanCreateExpiryTime,
                            leader_expiry.into(),
                        )
                        .filter(characters::Column::CharId.eq(leader_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::StoreClanWarehouse { clan_id, items } => {
                warn_err(
                    items::Entity::delete_many()
                        .filter(items::Column::OwnerId.eq(clan_id))
                        .exec(&db)
                        .await,
                );
                for it in &items {
                    warn_err(
                        items::Entity::insert(items::ActiveModel {
                            owner_id: Set(Some(clan_id)),
                            object_id: Set(it.object_id),
                            item_id: Set(Some(it.item_id)),
                            count: Set(it.count),
                            enchant_level: Set(Some(it.enchant_level)),
                            loc: Set(Some(it.loc.clone())),
                            loc_data: Set(Some(it.loc_data)),
                            custom_type1: Set(Some(it.custom_type1)),
                            custom_type2: Set(Some(it.custom_type2)),
                            mana_left: Set(it.mana_left),
                            time: Set(it.time.into()),
                            ..Default::default()
                        })
                        .exec(&db)
                        .await,
                    );
                }
            }
            DbCommand::SetAccessLevel { char_id, level } => {
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::Accesslevel, level.into())
                        .filter(characters::Column::CharId.eq(char_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::StoreAccountVar {
                account_name,
                var,
                value,
            } => {
                warn_err(
                    account_gsdata::Entity::insert(account_gsdata::ActiveModel {
                        account_name: Set(account_name),
                        var: Set(var),
                        value: Set(value),
                    })
                    .on_conflict(
                        OnConflict::columns([
                            account_gsdata::Column::AccountName,
                            account_gsdata::Column::Var,
                        ])
                        .update_column(account_gsdata::Column::Value)
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::StoreCharVar {
                char_id,
                var,
                value,
            } => {
                // The table has no unique key, so replace by delete + insert
                // (Java `REMOVE_UNCLAIMED_POINTS` then `INSERT_UNCLAIMED_POINTS`).
                warn_err(
                    character_variables::Entity::delete_many()
                        .filter(character_variables::Column::CharId.eq(char_id))
                        .filter(character_variables::Column::Var.eq(var.clone()))
                        .exec(&db)
                        .await,
                );
                warn_err(
                    character_variables::Entity::insert(character_variables::ActiveModel {
                        char_id: Set(char_id),
                        var: Set(var),
                        val: Set(value),
                    })
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::StorePremium {
                account_name,
                enddate,
            } => {
                warn_err(
                    account_premium::Entity::insert(account_premium::ActiveModel {
                        account_name: Set(account_name),
                        enddate: Set(enddate),
                    })
                    .on_conflict(
                        OnConflict::column(account_premium::Column::AccountName)
                            .update_column(account_premium::Column::Enddate)
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeletePremium { account_name } => {
                warn_err(
                    account_premium::Entity::delete_by_id(account_name)
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::StoreMail { message } => {
                // The boolean-ish columns are enum('true','false') text.
                let b = |v: bool| if v { "true" } else { "false" }.to_string();
                warn_err(
                    messages::Entity::insert(messages::ActiveModel {
                        message_id: Set(message.message_id),
                        sender_id: Set(message.sender_id),
                        receiver_id: Set(message.receiver_id),
                        subject: Set(Some(message.subject.clone())),
                        content: Set(Some(message.content.clone())),
                        expiration: Set(message.expiration),
                        req_adena: Set(message.req_adena),
                        has_attachments: Set(b(message.has_attachments)),
                        is_unread: Set(b(message.unread)),
                        is_deleted_by_sender: Set(b(message.deleted_by_sender)),
                        is_deleted_by_receiver: Set(b(message.deleted_by_receiver)),
                        send_by_system: Set(message.send_by_system),
                        is_returned: Set(b(message.returned)),
                        ..Default::default()
                    })
                    .on_conflict(
                        OnConflict::column(messages::Column::MessageId)
                            .update_columns([
                                messages::Column::SenderId,
                                messages::Column::ReceiverId,
                                messages::Column::Subject,
                                messages::Column::Content,
                                messages::Column::Expiration,
                                messages::Column::ReqAdena,
                                messages::Column::HasAttachments,
                                messages::Column::IsUnread,
                                messages::Column::IsDeletedBySender,
                                messages::Column::IsDeletedByReceiver,
                                messages::Column::SendBySystem,
                                messages::Column::IsReturned,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::UpdateMailFlags {
                message_id,
                unread,
                has_attachments,
                deleted_by_sender,
                deleted_by_receiver,
            } => {
                let b = |v: bool| if v { "true" } else { "false" };
                warn_err(
                    messages::Entity::update_many()
                        .col_expr(messages::Column::IsUnread, b(unread).into())
                        .col_expr(messages::Column::HasAttachments, b(has_attachments).into())
                        .col_expr(
                            messages::Column::IsDeletedBySender,
                            b(deleted_by_sender).into(),
                        )
                        .col_expr(
                            messages::Column::IsDeletedByReceiver,
                            b(deleted_by_receiver).into(),
                        )
                        .filter(messages::Column::MessageId.eq(message_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::DeleteMail { message_id } => {
                warn_err(messages::Entity::delete_by_id(message_id).exec(&db).await);
                warn_err(
                    items::Entity::delete_many()
                        .filter(items::Column::Loc.eq("MAIL"))
                        .filter(items::Column::LocData.eq(message_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::StoreOfflineWarehouseItems { owner_id, items } => {
                for it in &items {
                    warn_err(
                        items::Entity::insert(items::ActiveModel {
                            owner_id: Set(Some(owner_id)),
                            object_id: Set(it.object_id),
                            item_id: Set(Some(it.item_id)),
                            count: Set(it.count),
                            enchant_level: Set(Some(it.enchant_level)),
                            loc: Set(Some("WAREHOUSE".to_string())),
                            loc_data: Set(Some(0)),
                            custom_type1: Set(Some(it.custom_type1)),
                            custom_type2: Set(Some(it.custom_type2)),
                            mana_left: Set(it.mana_left),
                            time: Set(it.time.into()),
                            ..Default::default()
                        })
                        .on_conflict(
                            OnConflict::column(items::Column::ObjectId)
                                .update_columns([
                                    items::Column::OwnerId,
                                    items::Column::ItemId,
                                    items::Column::Count,
                                    items::Column::EnchantLevel,
                                    items::Column::Loc,
                                    items::Column::LocData,
                                    items::Column::CustomType1,
                                    items::Column::CustomType2,
                                    items::Column::ManaLeft,
                                    items::Column::Time,
                                ])
                                .to_owned(),
                        )
                        .exec(&db)
                        .await,
                    );
                }
            }
            DbCommand::StoreMailItems {
                message_id,
                owner_id,
                items,
            } => {
                warn_err(
                    items::Entity::delete_many()
                        .filter(items::Column::Loc.eq("MAIL"))
                        .filter(items::Column::LocData.eq(message_id))
                        .exec(&db)
                        .await,
                );
                for it in &items {
                    warn_err(
                        items::Entity::insert(items::ActiveModel {
                            owner_id: Set(Some(owner_id)),
                            object_id: Set(it.object_id),
                            item_id: Set(Some(it.item_id)),
                            count: Set(it.count),
                            enchant_level: Set(Some(it.enchant_level)),
                            loc: Set(Some("MAIL".to_string())),
                            loc_data: Set(Some(message_id)),
                            custom_type1: Set(Some(it.custom_type1)),
                            custom_type2: Set(Some(it.custom_type2)),
                            mana_left: Set(it.mana_left),
                            time: Set(it.time.into()),
                            ..Default::default()
                        })
                        .exec(&db)
                        .await,
                    );
                }
            }
            DbCommand::StoreLottery {
                idnr,
                enddate,
                prize,
            } => {
                warn_err(
                    lottery::Entity::insert(lottery::ActiveModel {
                        id: Set(1),
                        idnr: Set(idnr),
                        enddate: Set(enddate),
                        prize: Set(prize),
                        newprize: Set(prize),
                        ..Default::default()
                    })
                    .on_conflict(
                        OnConflict::columns([lottery::Column::Id, lottery::Column::Idnr])
                            .update_columns([
                                lottery::Column::Enddate,
                                lottery::Column::Prize,
                                lottery::Column::Newprize,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::FinishLottery {
                idnr,
                prize,
                newprize,
                number1,
                number2,
                prize1,
                prize2,
                prize3,
            } => {
                warn_err(
                    lottery::Entity::update_many()
                        .col_expr(lottery::Column::Finished, 1.into())
                        .col_expr(lottery::Column::Prize, prize.into())
                        .col_expr(lottery::Column::Newprize, newprize.into())
                        .col_expr(lottery::Column::Number1, number1.into())
                        .col_expr(lottery::Column::Number2, number2.into())
                        .col_expr(lottery::Column::Prize1, prize1.into())
                        .col_expr(lottery::Column::Prize2, prize2.into())
                        .col_expr(lottery::Column::Prize3, prize3.into())
                        .filter(lottery::Column::Id.eq(1))
                        .filter(lottery::Column::Idnr.eq(idnr))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::IncreaseLotteryPrize { idnr, prize } => {
                warn_err(
                    lottery::Entity::update_many()
                        .col_expr(lottery::Column::Prize, prize.into())
                        .col_expr(lottery::Column::Newprize, prize.into())
                        .filter(lottery::Column::Id.eq(1))
                        .filter(lottery::Column::Idnr.eq(idnr))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::LoadLotteryTickets { round } => {
                // Lottery tickets are ordinary items (id 4442) whose
                // `custom_type1` is the round they were bought in.
                let rows = items::Entity::find()
                    .filter(items::Column::ItemId.eq(4442))
                    .filter(items::Column::CustomType1.eq(round))
                    .all(&db)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|r| {
                        (
                            r.object_id,
                            r.enchant_level.unwrap_or(0),
                            r.custom_type2.unwrap_or(0),
                        )
                    })
                    .collect();
                let _ = event_tx.send(DbEvent::LotteryTicketsLoaded { round, rows });
            }
            DbCommand::SaveMdtHistory {
                race_id,
                first,
                second,
                odd_rate,
            } => {
                warn_err(
                    mdt_history::Entity::insert(mdt_history::ActiveModel {
                        race_id: Set(race_id),
                        first: Set(Some(first)),
                        second: Set(Some(second)),
                        odd_rate: Set(Some(odd_rate)),
                    })
                    .on_conflict(
                        OnConflict::column(mdt_history::Column::RaceId)
                            .update_columns([
                                mdt_history::Column::First,
                                mdt_history::Column::Second,
                                mdt_history::Column::OddRate,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::SaveMdtBet { lane, bet } => {
                warn_err(
                    mdt_bets::Entity::insert(mdt_bets::ActiveModel {
                        lane_id: Set(lane),
                        bet: Set(Some(bet)),
                    })
                    .on_conflict(
                        OnConflict::column(mdt_bets::Column::LaneId)
                            .update_column(mdt_bets::Column::Bet)
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::ClearMdtBets => {
                warn_err(
                    mdt_bets::Entity::update_many()
                        .col_expr(mdt_bets::Column::Bet, 0.into())
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::StoreItemAuction {
                auction_id,
                instance_id,
                auction_item_id,
                starting_time,
                ending_time,
                state_id,
            } => {
                warn_err(
                    item_auction::Entity::insert(item_auction::ActiveModel {
                        auction_id: Set(auction_id),
                        instance_id: Set(instance_id),
                        auction_item_id: Set(auction_item_id),
                        starting_time: Set(starting_time),
                        ending_time: Set(ending_time),
                        auction_state_id: Set(state_id.into()),
                    })
                    .on_conflict(
                        OnConflict::column(item_auction::Column::AuctionId)
                            .update_columns([
                                item_auction::Column::InstanceId,
                                item_auction::Column::AuctionItemId,
                                item_auction::Column::StartingTime,
                                item_auction::Column::EndingTime,
                                item_auction::Column::AuctionStateId,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::StoreItemAuctionBid {
                auction_id,
                player_obj_id,
                bid,
            } => {
                warn_err(
                    item_auction_bid::Entity::insert(item_auction_bid::ActiveModel {
                        auction_id: Set(auction_id),
                        player_obj_id: Set(player_obj_id),
                        player_bid: Set(bid),
                    })
                    .on_conflict(
                        OnConflict::columns([
                            item_auction_bid::Column::AuctionId,
                            item_auction_bid::Column::PlayerObjId,
                        ])
                        .update_column(item_auction_bid::Column::PlayerBid)
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeleteItemAuctionBid {
                auction_id,
                player_obj_id,
            } => {
                warn_err(
                    item_auction_bid::Entity::delete_by_id((auction_id, player_obj_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::DeleteItemAuction { auction_id } => {
                warn_err(
                    item_auction::Entity::delete_by_id(auction_id)
                        .exec(&db)
                        .await,
                );
                warn_err(
                    item_auction_bid::Entity::delete_many()
                        .filter(item_auction_bid::Column::AuctionId.eq(auction_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::StorePunishment {
                id,
                key,
                affect,
                ptype,
                expiration,
                reason,
                punished_by,
            } => {
                warn_err(
                    punishments::Entity::insert(punishments::ActiveModel {
                        id: Set(id),
                        key: Set(key),
                        affect: Set(affect),
                        r#type: Set(ptype),
                        expiration: Set(expiration),
                        reason: Set(reason),
                        punished_by: Set(punished_by),
                    })
                    .on_conflict(
                        OnConflict::column(punishments::Column::Id)
                            .update_columns([
                                punishments::Column::Key,
                                punishments::Column::Affect,
                                punishments::Column::Type,
                                punishments::Column::Expiration,
                                punishments::Column::Reason,
                                punishments::Column::PunishedBy,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeletePunishment { id } => {
                warn_err(punishments::Entity::delete_by_id(id).exec(&db).await);
            }
            DbCommand::StorePetitionFeedback {
                char_name,
                gm_name,
                rate,
                message,
                date,
            } => {
                warn_err(
                    petition_feedback::Entity::insert(petition_feedback::ActiveModel {
                        char_name: Set(char_name),
                        gm_name: Set(gm_name),
                        rate: Set(rate),
                        message: Set(message),
                        date: Set(date),
                    })
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::StoreOfflineWarehouseItem {
                owner_id,
                object_id,
                item_id,
                count,
                enchant,
            } => {
                warn_err(
                    items::Entity::insert(items::ActiveModel {
                        owner_id: Set(Some(owner_id)),
                        object_id: Set(object_id),
                        item_id: Set(Some(item_id)),
                        count: Set(count),
                        enchant_level: Set(Some(enchant)),
                        loc: Set(Some("WAREHOUSE".to_string())),
                        loc_data: Set(Some(0)),
                        ..Default::default()
                    })
                    .on_conflict(
                        OnConflict::column(items::Column::ObjectId)
                            .update_columns([
                                items::Column::OwnerId,
                                items::Column::ItemId,
                                items::Column::Count,
                                items::Column::EnchantLevel,
                                items::Column::Loc,
                                items::Column::LocData,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::StoreBufferScheme {
                object_id,
                scheme_name,
                skills,
            } => {
                warn_err(
                    buffer_schemes::Entity::insert(buffer_schemes::ActiveModel {
                        object_id: Set(object_id),
                        scheme_name: Set(scheme_name),
                        skills: Set(skills),
                    })
                    .on_conflict(
                        OnConflict::columns([
                            buffer_schemes::Column::ObjectId,
                            buffer_schemes::Column::SchemeName,
                        ])
                        .update_column(buffer_schemes::Column::Skills)
                        .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeleteBufferScheme {
                object_id,
                scheme_name,
            } => {
                warn_err(
                    buffer_schemes::Entity::delete_by_id((object_id, scheme_name))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::StoreFavorite {
                fav_id,
                player_id,
                title,
                bypass,
                add_date,
            } => {
                warn_err(
                    bbs_favorites::Entity::insert(bbs_favorites::ActiveModel {
                        fav_id: Set(fav_id),
                        player_id: Set(player_id),
                        fav_title: Set(title),
                        fav_bypass: Set(bypass),
                        fav_add_date: Set(add_date),
                    })
                    .on_conflict(
                        OnConflict::column(bbs_favorites::Column::FavId)
                            .update_columns([
                                bbs_favorites::Column::PlayerId,
                                bbs_favorites::Column::FavTitle,
                                bbs_favorites::Column::FavBypass,
                                bbs_favorites::Column::FavAddDate,
                            ])
                            .to_owned(),
                    )
                    .exec(&db)
                    .await,
                );
            }
            DbCommand::DeleteFavorite { player_id, fav_id } => {
                warn_err(
                    bbs_favorites::Entity::delete_many()
                        .filter(bbs_favorites::Column::PlayerId.eq(player_id))
                        .filter(bbs_favorites::Column::FavId.eq(fav_id))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::ResetRecommends => {
                // Java `DailyTaskManager.resetRecommends`: rec_left → 0 for
                // everyone; rec_have → 0 for those at/under 20, else -20.
                warn_err(
                    character_reco_bonus::Entity::update_many()
                        .col_expr(character_reco_bonus::Column::RecLeft, 0.into())
                        .col_expr(character_reco_bonus::Column::RecHave, 0.into())
                        .filter(character_reco_bonus::Column::RecHave.lte(20))
                        .exec(&db)
                        .await,
                );
                // `ExprTrait` is imported here rather than at module scope: it
                // adds `min`/`max`/`add` to *every* type, which shadows the
                // `Ord` ones everywhere else in this file.
                use models::sea_orm::sea_query::ExprTrait as _;
                warn_err(
                    character_reco_bonus::Entity::update_many()
                        .col_expr(character_reco_bonus::Column::RecLeft, 0.into())
                        .col_expr(
                            character_reco_bonus::Column::RecHave,
                            Expr::col(character_reco_bonus::Column::RecHave).sub(20),
                        )
                        .filter(character_reco_bonus::Column::RecHave.gt(20))
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::ResetVitality { weekly } => {
                // Java `resetVitalityDaily`/`resetVitalityWeekly` — both the
                // `characters` and `character_subclasses` rows. `MAX/4` is added
                // uncapped (as Java does); the read-side clamp hides any overflow.
                const MAX: i32 = 140_000;
                // `ExprTrait` is imported here rather than at module scope: it
                // adds `min`/`max`/`add` to *every* type, which shadows the
                // `Ord` ones everywhere else in this file.
                use models::sea_orm::sea_query::ExprTrait as _;
                // Daily adds a quarter of the cap unless the pool is already
                // full; weekly refills it outright.
                fn refill<C: models::sea_orm::ColumnTrait>(col: C, weekly: bool) -> Expr {
                    if weekly {
                        Expr::value(MAX)
                    } else {
                        CaseStatement::new()
                            .case(Expr::col(col).eq(MAX), Expr::col(col))
                            .finally(Expr::col(col).add(MAX / 4))
                            .into()
                    }
                }
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(
                            characters::Column::VitalityPoints,
                            refill(characters::Column::VitalityPoints, weekly),
                        )
                        .exec(&db)
                        .await,
                );
                warn_err(
                    character_subclasses::Entity::update_many()
                        .col_expr(
                            character_subclasses::Column::VitalityPoints,
                            refill(character_subclasses::Column::VitalityPoints, weekly),
                        )
                        .exec(&db)
                        .await,
                );
            }
            DbCommand::RepairCharacter { char_name } => {
                // Java `AdminRepairChar`, verbatim. Best-effort: each statement
                // is independent, keyed by name / resolved id.
                warn_err(
                    characters::Entity::update_many()
                        .col_expr(characters::Column::X, (-84318).into())
                        .col_expr(characters::Column::Y, 244579.into())
                        .col_expr(characters::Column::Z, (-3730).into())
                        .filter(characters::Column::CharName.eq(&char_name))
                        .exec(&db)
                        .await,
                );
                let obj_id = characters::Entity::find()
                    .filter(characters::Column::CharName.eq(&char_name))
                    .one(&db)
                    .await
                    .ok()
                    .flatten()
                    .map(|c| c.char_id);
                if let Some(obj_id) = obj_id {
                    warn_err(
                        character_shortcuts::Entity::delete_many()
                            .filter(character_shortcuts::Column::CharId.eq(obj_id))
                            .exec(&db)
                            .await,
                    );
                    warn_err(
                        items::Entity::update_many()
                            .col_expr(items::Column::Loc, "INVENTORY".into())
                            .filter(items::Column::OwnerId.eq(obj_id))
                            .exec(&db)
                            .await,
                    );
                }
            }
            DbCommand::Shutdown => break,
        }
    }

    let _ = db.close().await;
    info!("DB thread: stopped.");
}

async fn reload(
    db: &DatabaseConnection,
    event_tx: &EventTx,
    client_id: u32,
    account: String,
    send_list: bool,
) {
    let chars = load_characters(db, &account).await;
    let _ = event_tx.send(DbEvent::CharactersLoaded {
        client_id,
        account,
        chars,
        send_list,
    });
}

/// Best-effort read of one `account_gsdata` variable (Java
/// `AccountVariables.restoreMe`). Returns `None` on a missing row or any error
/// (e.g. the table absent in a minimal test schema), mirroring Java's
/// catch-and-default-empty behaviour.
async fn load_account_var(db: &DatabaseConnection, account: &str, var: &str) -> Option<String> {
    account_gsdata::Entity::find_by_id((account.to_string(), var.to_string()))
        .one(db)
        .await
        .ok()
        .flatten()
        .map(|row| row.value)
}

/// Best-effort boot load of the whole `account_premium` table (Java
/// `PremiumManager` has no table-wide load; this port caches all rows so the
/// admin `//premium_*` commands work for offline accounts). Missing table → empty.
async fn load_premium(db: &DatabaseConnection) -> Vec<(String, i64)> {
    account_premium::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| (row.account_name.to_lowercase(), row.enddate))
        .collect()
}

/// The most recent lottery round (Java `Lottery.SELECT_LAST_LOTTERY`). `None`
/// when the table is empty or unavailable.
async fn load_lottery(db: &DatabaseConnection) -> Option<crate::model::lottery::LotteryRow> {
    let row = lottery::Entity::find()
        .filter(lottery::Column::Id.eq(1))
        .order_by_desc(lottery::Column::Idnr)
        .one(db)
        .await
        .ok()
        .flatten()?;
    Some(crate::model::lottery::LotteryRow {
        idnr: row.idnr,
        prize: row.prize,
        newprize: row.newprize,
        enddate: row.enddate,
        finished: row.finished == 1,
    })
}

/// Every finished lottery round's draw result (Java re-queries per
/// `checkTicket`; loaded once at boot into the game-thread cache).
async fn load_lottery_draws(
    db: &DatabaseConnection,
) -> Vec<(i32, crate::model::lottery::DrawnRound)> {
    lottery::Entity::find()
        .filter(lottery::Column::Id.eq(1))
        .filter(lottery::Column::Finished.eq(1))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| {
            (
                row.idnr,
                crate::model::lottery::DrawnRound {
                    number1: row.number1,
                    number2: row.number2,
                    prize1: row.prize1,
                    prize2: row.prize2,
                    prize3: row.prize3,
                },
            )
        })
        .collect()
}

/// Every Monster Race history record, oldest first (Java `MonsterRace
/// .loadHistory` — also fixes the current race number by the row count).
async fn load_mdt_history(db: &DatabaseConnection) -> Vec<crate::model::monster_race::HistoryInfo> {
    mdt_history::Entity::find()
        .order_by_asc(mdt_history::Column::RaceId)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| crate::model::monster_race::HistoryInfo {
            race_id: row.race_id,
            first: row.first.unwrap_or(0),
            second: row.second.unwrap_or(0),
            odd_rate: row.odd_rate.unwrap_or(0.0),
        })
        .collect()
}

/// The current lane bets (Java `MonsterRace.loadBets`): `(lane_id, bet)`.
async fn load_mdt_bets(db: &DatabaseConnection) -> Vec<(i32, i64)> {
    mdt_bets::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| (row.lane_id, row.bet.unwrap_or(0)))
        .collect()
}

/// Every persisted item auction + its bids, plus the next auction id (Java
/// `ItemAuctionManager` boot load: `MAX(auctionId)+1` and each instance's
/// `loadAuction`). Empty on this dist.
/// Java `MailManager.load` + the `loc = 'MAIL'` item rows, in one pass.
/// Tolerates the tables being absent (a minimal test schema has neither).
async fn load_mail(
    db: &DatabaseConnection,
) -> (
    Vec<crate::model::mail::Message>,
    Vec<(i32, Vec<crate::character::ItemRow>)>,
) {
    use crate::model::mail::{MailType, Message};

    // The flag columns are enum('true','false') text; older rows may carry '1'.
    let truthy = |v: &str| v.eq_ignore_ascii_case("true") || v == "1";
    let messages = messages::Entity::find()
        .order_by_asc(messages::Column::Expiration)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| Message {
            id: r.message_id,
            sender_id: r.sender_id,
            receiver_id: r.receiver_id,
            subject: r.subject.unwrap_or_default(),
            content: r.content.unwrap_or_default(),
            expiration: r.expiration,
            req_adena: r.req_adena,
            has_attachments: truthy(&r.has_attachments),
            unread: truthy(&r.is_unread),
            deleted_by_sender: truthy(&r.is_deleted_by_sender),
            deleted_by_receiver: truthy(&r.is_deleted_by_receiver),
            mail_type: MailType::from_id(r.send_by_system),
            returned: truthy(&r.is_returned),
        })
        .collect();

    let mut by_message: std::collections::HashMap<i32, Vec<crate::character::ItemRow>> =
        std::collections::HashMap::new();
    for r in items::Entity::find()
        .filter(items::Column::Loc.eq("MAIL"))
        .all(db)
        .await
        .unwrap_or_default()
    {
        // Attachments hang off the message through `loc_data`.
        let message_id = r.loc_data.unwrap_or(0);
        by_message
            .entry(message_id)
            .or_default()
            .push(crate::character::ItemRow {
                object_id: r.object_id,
                item_id: r.item_id.unwrap_or(0),
                count: r.count,
                enchant_level: r.enchant_level.unwrap_or(0),
                loc: "MAIL".to_string(),
                loc_data: message_id,
                custom_type1: r.custom_type1.unwrap_or(0),
                custom_type2: r.custom_type2.unwrap_or(0),
                mana_left: r.mana_left,
                time: r.time as i32,
                augment_mineral: 0,
                augment_option1: 0,
                augment_option2: 0,
            });
    }
    (messages, by_message.into_iter().collect())
}

/// Java `CharInfoTable` — the offline character name -> id table. Mail is
/// addressed by name to characters who need not be online; nothing else in the
/// port needs this, so it is loaded once and maintained on creation/deletion.
async fn load_char_ids_by_name(db: &DatabaseConnection) -> Vec<(String, i32)> {
    characters::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| (row.char_name.to_lowercase(), row.char_id))
        .collect()
}

async fn load_item_auctions(
    db: &DatabaseConnection,
) -> (i32, Vec<crate::model::item_auction::ItemAuction>) {
    use crate::model::item_auction::{AuctionState, ItemAuction, ItemAuctionBid};

    let mut auctions: Vec<ItemAuction> = item_auction::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|row| {
            let state = AuctionState::from_state_id(row.auction_state_id as i8)?;
            Some(ItemAuction::new(
                row.auction_id,
                row.instance_id,
                row.auction_item_id,
                row.starting_time,
                row.ending_time,
                state,
            ))
        })
        .collect();

    // Attach each auction's bids.
    for bid in item_auction_bid::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
    {
        if let Some(a) = auctions.iter_mut().find(|a| a.auction_id == bid.auction_id) {
            a.bids.push(ItemAuctionBid {
                player_obj_id: bid.player_obj_id,
                last_bid: bid.player_bid,
            });
        }
    }

    let next_id = auctions.iter().map(|a| a.auction_id).max().unwrap_or(0) + 1;
    (next_id, auctions)
}

/// `PunishmentManager.load` (G31): every active punishment, minus the rows that
/// have already expired (Java skips them, counting them as "expired"). Returns
/// `(next_id, rows)` — `next_id` seeds the game-thread id allocator. Fail-open
/// (empty) if the table is absent, like a minimal test schema.
async fn load_punishments(
    db: &DatabaseConnection,
) -> (i32, Vec<crate::model::punishment::Punishment>) {
    use crate::model::punishment::{Punishment, PunishmentAffect, PunishmentType};

    let now = commons::util::now_millis();
    let all = punishments::Entity::find()
        .all(db)
        .await
        .unwrap_or_default();
    let rows: Vec<Punishment> = all
        .iter()
        .filter_map(|row| {
            let affect = PunishmentAffect::from_name(&row.affect)?;
            let ptype = PunishmentType::from_name(&row.r#type)?;
            // Java's `load` skips already-expired rows.
            if row.expiration > 0 && now > row.expiration {
                return None;
            }
            Some(Punishment {
                id: row.id,
                key: row.key.clone(),
                affect,
                ptype,
                expiration: row.expiration,
                reason: row.reason.clone(),
                punished_by: row.punished_by.clone(),
            })
        })
        .collect();

    // The id allocator must clear *every* persisted id, not just the still-active
    // ones — an expired row we filtered out above may still own the max id until
    // the operator purges it, and reusing that id would collide on INSERT.
    // `all` (not `rows`) on purpose: an expired row we filtered out still owns
    // its id.
    let loaded_max = all.iter().map(|row| row.id).max().unwrap_or(0);
    let next_id = (loaded_max + 1).max(1);
    (next_id, rows)
}

/// One `npc_respawns` row — a raid boss's persisted state.
#[derive(Debug, Clone, Copy)]
pub struct NpcRespawnRow {
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// Absolute unix millis the boss is due back, or 0 when it's alive (Java
    /// stores 0 for a living boss and the due time for a dead one).
    pub respawn_time: i64,
    pub cur_hp: f64,
    pub cur_mp: f64,
}

/// `RESTORE_CHAR_SUBCLASSES` — a character's subclass slots.
async fn load_subclasses(db: &DatabaseConnection, char_id: i32) -> Vec<crate::model::SubClass> {
    character_subclasses::Entity::find()
        .filter(character_subclasses::Column::CharId.eq(char_id))
        .order_by_asc(character_subclasses::Column::ClassIndex)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| crate::model::SubClass {
            class_id: row.class_id,
            class_index: row.class_index,
            level: row.level,
            exp: row.exp,
            sp: row.sp,
        })
        .collect()
}

/// Boot load of the whole `npc_respawns` table (Java `DBSpawnManager.load`).
/// Missing table → empty, like the other boot loads.
async fn load_npc_respawns(db: &DatabaseConnection) -> Vec<NpcRespawnRow> {
    npc_respawns::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| NpcRespawnRow {
            npc_id: row.id,
            x: row.x,
            y: row.y,
            z: row.z,
            heading: row.heading,
            respawn_time: row.respawn_time,
            cur_hp: row.current_hp,
            cur_mp: row.current_mp,
        })
        .collect()
}

/// Boot load of the whole `buffer_schemes` table (Java `SchemeBufferTable.load`).
/// `skills` is stored comma-joined; parse it here, drop empties. Availability
/// filtering (skills still in the buffer table) happens on the game thread,
/// where the datapack lives. Missing table → empty.
async fn load_buffer_schemes(db: &DatabaseConnection) -> Vec<(i32, String, Vec<i32>)> {
    buffer_schemes::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| {
            let skills = row
                .skills
                .split(',')
                .filter_map(|s| s.trim().parse::<i32>().ok())
                .collect();
            (row.object_id, row.scheme_name, skills)
        })
        .collect()
}

/// Boot load of the whole `bbs_favorites` table (Java `FavoriteBoard` loads it
/// per-player on `_bbsgetfav`; this port caches all rows at boot like the
/// buffer schemes). `ORDER BY favAddDate DESC` matches Java's list order.
/// Missing table → empty.
async fn load_favorites(db: &DatabaseConnection) -> Vec<(i32, i32, String, String, String)> {
    bbs_favorites::Entity::find()
        .order_by_desc(bbs_favorites::Column::FavAddDate)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| {
            (
                row.player_id,
                row.fav_id,
                row.fav_title,
                row.fav_bypass,
                row.fav_add_date,
            )
        })
        .collect()
}

/// Java's `IdManager` hands out ids from a single pool shared by every
/// world-object type, so the next free id must clear the high-water mark of
/// every table that stores one — not just `characters` (a fresh id here that
/// collides with an existing `items.object_id` fails its INSERT silently).
async fn load_next_id(db: &DatabaseConnection) -> i64 {
    let max_char = characters::Entity::find()
        .select_only()
        .column_as(characters::Column::CharId.max(), "m")
        .into_tuple::<Option<i64>>()
        .one(db)
        .await
        .ok()
        .flatten()
        .flatten()
        .unwrap_or(0);
    let max_item = items::Entity::find()
        .select_only()
        .column_as(items::Column::ObjectId.max(), "m")
        .into_tuple::<Option<i64>>()
        .one(db)
        .await
        .ok()
        .flatten()
        .flatten()
        .unwrap_or(0);
    (max_char.max(max_item) + 1).max(FIRST_OID)
}

/// `loadCharacterSelectInfo`: rows for an account, expired deletions purged.
async fn load_characters(db: &DatabaseConnection, account: &str) -> Vec<CharData> {
    let rows = match characters::Entity::find()
        .filter(characters::Column::AccountName.eq(account))
        .order_by_asc(characters::Column::CreateDate)
        .all(db)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!("DB thread: load_characters failed: {e}");
            return Vec::new();
        }
    };

    // Account-scoped prime (NCoin) balance — same for every char on the
    // account. Best-effort: absent table/row → 0 (Java `restoreMe` catch).
    let prime_points = load_account_var(db, account, "PRIME_POINTS")
        .await
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);

    let now = now_millis();
    let mut out = Vec::new();
    for (slot, row) in rows.iter().enumerate() {
        let delete_time = row.deletetime;
        let object_id = row.char_id;
        if delete_time > 0 && now > delete_time {
            delete_char(db, object_id).await; // restoreChar: purge expired
            continue;
        }
        let items = load_items(db, object_id).await;
        let skills_by_index = load_skills(db, object_id).await;
        let subclasses = load_subclasses(db, object_id).await;
        let class_id_now = row.classid.unwrap_or(0);
        // Java keeps the *active* class in `characters.classid`; the index is
        // whichever subclass slot carries it (0 when it's the base class).
        let active_index = subclasses
            .iter()
            .find(|s| s.class_id == class_id_now)
            .map(|s| s.class_index)
            .unwrap_or(0);
        let hennas_by_index = load_hennas(db, object_id).await;
        let recipe_book = load_recipe_book(db, object_id).await;
        let variables = load_variables(db, object_id).await;
        let pets = load_pets(db, object_id).await;
        let summons = load_summons(db, object_id).await;
        let shortcuts_by_index = load_shortcuts(db, object_id).await;
        let macros = load_macros(db, object_id).await;
        let friends = load_friends(db, object_id).await;
        let quests = load_quests(db, object_id).await;
        let skill_reuses = load_skill_reuses(db, object_id, active_index).await;
        let skill_buffs = load_skill_buffs(db, object_id, active_index).await;
        let (rec_have, rec_left) = load_reco_bonus(db, object_id).await;
        out.push(CharData {
            object_id,
            name: row.char_name.clone(),
            account_name: row.account_name.clone().unwrap_or_default(),
            level: row.level.unwrap_or(0),
            max_hp: row.max_hp.unwrap_or(0),
            cur_hp: row.cur_hp.map(f64::from).unwrap_or(0.0),
            max_mp: row.max_mp.unwrap_or(0),
            cur_mp: row.cur_mp.map(f64::from).unwrap_or(0.0),
            face: row.face.unwrap_or(0),
            hair_style: row.hair_style.unwrap_or(0),
            hair_color: row.hair_color.unwrap_or(0),
            sex: row.sex.unwrap_or(0),
            x: row.x.unwrap_or(0),
            y: row.y.unwrap_or(0),
            z: row.z.unwrap_or(0),
            exp: row.exp.unwrap_or(0),
            sp: row.sp,
            reputation: row.reputation.unwrap_or(0),
            pk_kills: row.pkkills.unwrap_or(0),
            raidboss_points: row.raidboss_points,
            pvp_kills: row.pvpkills.unwrap_or(0),
            rec_have,
            rec_left,
            clan_id: row.clanid.unwrap_or(0),
            clan_privs: row.clan_privs.unwrap_or(0),
            clan_create_expiry_time: row.clan_create_expiry_time,
            clan_join_expiry_time: row.clan_join_expiry_time,
            create_date: row.create_date.clone(),
            power_grade: row.power_grade.unwrap_or(0),
            pledge_type: row.subpledge,
            race: row.race.unwrap_or(0),
            class_id: class_id_now,
            base_class_id: row.base_class,
            delete_time,
            last_access: row.last_access,
            vitality_points: row.vitality_points,
            pccafe_points: row.pccafe_points,
            prime_points,
            access_level: row.accesslevel.unwrap_or(0),
            noble: row.nobless == 1,
            subclasses,
            char_slot: slot as i32,
            items,
            // The active class index is whichever subclass row matches the
            // `characters.classid` we just loaded; base class → 0.
            skills: skills_by_index
                .get(&active_index)
                .cloned()
                .unwrap_or_default(),
            skills_by_index,
            hennas: hennas_by_index
                .get(&active_index)
                .cloned()
                .unwrap_or_default(),
            hennas_by_index,
            recipe_book,
            variables,
            pets,
            summons,
            shortcuts: shortcuts_by_index
                .get(&active_index)
                .cloned()
                .unwrap_or_default(),
            shortcuts_by_index,
            macros,
            friends,
            quests,
            skill_reuses,
            skill_buffs,
        });
    }
    // Characters marked for deletion are listed last in the lobby; the stable
    // sort keeps createDate order within each group. Slots are the list
    // positions the client will send back, so renumber after sorting.
    out.sort_by_key(|c| c.delete_time > 0);
    for (slot, c) in out.iter_mut().enumerate() {
        c.char_slot = slot as i32;
    }
    out
}

/// A character's `character_skills` rows (Java: `Player.restoreSkills`,
/// called for every row shown in `CharSelectionInfo` — same treatment as
/// `load_items`).
async fn load_skills(
    db: &DatabaseConnection,
    owner_id: i32,
) -> std::collections::HashMap<i32, Vec<(i32, i32, i32)>> {
    let rows = character_skills::Entity::find()
        .filter(character_skills::Column::CharId.eq(owner_id))
        .all(db)
        .await
        .unwrap_or_default();
    let mut out: std::collections::HashMap<i32, Vec<(i32, i32, i32)>> =
        std::collections::HashMap::new();
    for row in rows {
        out.entry(row.class_index).or_default().push((
            row.skill_id,
            row.skill_level,
            row.skill_sub_level,
        ));
    }
    out
}

/// A character's `character_hennas` rows (Java `Player.restoreHenna`) as
/// `(slot, symbol_id)`. `class_index = 0` — no subclasses on this dist.
async fn load_hennas(
    db: &DatabaseConnection,
    owner_id: i32,
) -> std::collections::HashMap<i32, Vec<(i32, i32)>> {
    let rows = character_hennas::Entity::find()
        .filter(character_hennas::Column::CharId.eq(owner_id))
        .all(db)
        .await
        .unwrap_or_default();
    let mut out: std::collections::HashMap<i32, Vec<(i32, i32)>> = std::collections::HashMap::new();
    for row in rows {
        let (slot, sym) = (row.slot, row.symbol_id.unwrap_or(0));
        if (1..=3).contains(&slot) && sym != 0 {
            out.entry(row.class_index).or_default().push((slot, sym));
        }
    }
    out
}

/// A character's `character_recipebook` rows (Java `Player.restoreRecipeBook`)
/// as recipe-*list* ids. The dwarven/common split (the `type` column) is
/// re-derived from `RecipeData` on the game thread, so the DB layer just
/// returns the ids. `classIndex = 0` — no subclasses on this dist.
async fn load_recipe_book(db: &DatabaseConnection, owner_id: i32) -> Vec<i32> {
    character_recipebook::Entity::find()
        .filter(character_recipebook::Column::CharId.eq(owner_id))
        .filter(character_recipebook::Column::ClassIndex.eq(0))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| row.id as i32)
        .collect()
}

/// A character's `character_variables` rows (Java `PlayerVariables.restoreMe`)
/// as `(var, val)` pairs. Values stay strings — the component parses on read,
/// like Java's `StatSet` getters.
async fn load_variables(db: &DatabaseConnection, owner_id: i32) -> Vec<(String, String)> {
    character_variables::Entity::find()
        .filter(character_variables::Column::CharId.eq(owner_id))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| (row.var, row.val))
        .collect()
}

/// Every pet this character owns (Java `Pet.restore`, hoisted from per-summon
/// to per-login — see `PlayerPets`). Java reads one row by collar object id at
/// summon time; loading the whole set here keeps the summon path off the DB
/// thread and costs one extra query per login.
/// The servitor this character had out at logout, if any (Java
/// `CharSummonTable.LOAD_SUMMON`).
async fn load_summons(db: &DatabaseConnection, owner_id: i32) -> Vec<SummonRow> {
    let rows = character_summons::Entity::find()
        .filter(character_summons::Column::OwnerId.eq(owner_id))
        .all(db)
        .await
        .unwrap_or_default();
    let mut out = Vec::new();
    for row in rows {
        out.push(SummonRow {
            summon_skill_id: row.summon_skill_id,
            cur_hp: row.cur_hp.unwrap_or(0),
            cur_mp: row.cur_mp.unwrap_or(0),
            remaining_secs: row.time,
            buffs: load_summon_buffs(db, owner_id, row.summon_skill_id).await,
        });
    }
    out
}

/// A servitor's saved buffs (Java `Servitor.RESTORE_SKILL_SAVE`), ordered by
/// `buff_index` so they come back in the order they were applied — which
/// matters for the buff-slot cap.
async fn load_summon_buffs(
    db: &DatabaseConnection,
    owner_id: i32,
    summon_skill_id: i32,
) -> Vec<SkillBuffRow> {
    character_summon_skills_save::Entity::find()
        .filter(character_summon_skills_save::Column::OwnerId.eq(owner_id))
        .filter(character_summon_skills_save::Column::OwnerClassIndex.eq(0))
        .filter(character_summon_skills_save::Column::SummonSkillId.eq(summon_skill_id))
        .order_by_asc(character_summon_skills_save::Column::BuffIndex)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| SkillBuffRow {
            skill_id: row.skill_id,
            skill_level: row.skill_level,
            remaining_time_secs: row.remaining_time,
        })
        .collect()
}

async fn load_pets(db: &DatabaseConnection, owner_id: i32) -> Vec<PetRow> {
    pets::Entity::find()
        .filter(pets::Column::OwnerId.eq(owner_id))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| PetRow {
            collar_object_id: row.item_obj_id,
            name: row.name.unwrap_or_default(),
            level: row.level,
            cur_hp: row.cur_hp.map(f64::from).unwrap_or(0.0),
            cur_mp: row.cur_mp.map(f64::from).unwrap_or(0.0),
            exp: row.exp.unwrap_or(0),
            sp: row.sp.unwrap_or(0),
            fed: row.fed.unwrap_or(0),
            restore: row.restore == "true",
        })
        .collect()
}

/// A character's `character_skills_save` reuse rows for the **active** class
/// index (Java `restoreEffects`, `restore_type = 1` half). Already-expired rows (`systime <= now`) are
/// dropped here; the survivors carry the absolute `systime` and the game side
/// converts it to a game tick when the character enters the world. Buff rows
/// (restore_type 0) are loaded separately by [`load_skill_buffs`].
async fn load_skill_reuses(
    db: &DatabaseConnection,
    owner_id: i32,
    class_index: i32,
) -> Vec<SkillReuseRow> {
    let now = now_millis();
    character_skills_save::Entity::find()
        .filter(character_skills_save::Column::CharId.eq(owner_id))
        .filter(character_skills_save::Column::ClassIndex.eq(class_index))
        .filter(character_skills_save::Column::RestoreType.eq(1))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|row| {
            (row.systime > now).then_some(SkillReuseRow {
                reuse_key: row.skill_id,
                skill_level: row.skill_level,
                reuse_delay: row.reuse_delay,
                systime_ms: row.systime,
            })
        })
        .collect()
}

/// A character's `character_skills_save` **buff** rows for the **active** class
/// index (Java `restoreEffects`, `restore_type = 0` half), in `buff_index`
/// order so the buff bar comes back in the order it was stored.
///
/// No expiry filtering happens here, unlike [`load_skill_reuses`]: a buff's
/// `remaining_time` is relative and its countdown is frozen while the character
/// is offline, so there is no elapsed time to compare against. Rows with a
/// non-positive remaining time are dropped since they'd restore an
/// already-dead buff.
async fn load_skill_buffs(
    db: &DatabaseConnection,
    owner_id: i32,
    class_index: i32,
) -> Vec<SkillBuffRow> {
    character_skills_save::Entity::find()
        .filter(character_skills_save::Column::CharId.eq(owner_id))
        .filter(character_skills_save::Column::ClassIndex.eq(class_index))
        .filter(character_skills_save::Column::RestoreType.eq(0))
        .order_by_asc(character_skills_save::Column::BuffIndex)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|row| {
            (row.remaining_time > 0).then_some(SkillBuffRow {
                skill_id: row.skill_id,
                skill_level: row.skill_level,
                remaining_time_secs: row.remaining_time,
            })
        })
        .collect()
}

/// A character's recommendation counters (Java `Player.loadRecommendations`).
/// Returns `(rec_have, rec_left)`; `(0, 0)` when the row is absent, matching
/// Java's field defaults for a character whose `character_reco_bonus` row
/// hasn't been written yet.
async fn load_reco_bonus(db: &DatabaseConnection, owner_id: i32) -> (i32, i32) {
    match character_reco_bonus::Entity::find_by_id(owner_id)
        .one(db)
        .await
    {
        Ok(Some(row)) => (row.rec_have, row.rec_left),
        _ => (0, 0),
    }
}

/// A character's `character_shortcuts` rows (Java `ShortCuts.restoreMe` —
/// the inventory verification half runs on the game thread, in
/// `Player::from_char`). `characterType` isn't stored; restore hardcodes 1
/// like Java. `shared_reuse_group` starts at the -1 default; `from_char`
/// fills it for EtcItem shortcuts.
async fn load_shortcuts(
    db: &DatabaseConnection,
    owner_id: i32,
) -> std::collections::HashMap<i32, Vec<crate::model::shortcut::Shortcut>> {
    let rows = character_shortcuts::Entity::find()
        .filter(character_shortcuts::Column::CharId.eq(owner_id))
        .all(db)
        .await
        .unwrap_or_default();
    let mut out: std::collections::HashMap<i32, Vec<crate::model::shortcut::Shortcut>> =
        std::collections::HashMap::new();
    for row in rows {
        out.entry(row.class_index)
            .or_default()
            .push(crate::model::shortcut::Shortcut {
                slot: row.slot,
                page: row.page,
                kind: crate::model::shortcut::ShortcutType::from_ordinal(row.r#type.unwrap_or(0)),
                id: row.shortcut_id.unwrap_or(0) as i32,
                level: row.level.unwrap_or(0),
                character_type: 1,
                shared_reuse_group: -1,
            });
    }
    out
}

/// A character's `character_friends` rows joined with each friend's
/// character row — the name/level/class snapshot Java reads through
/// `CharInfoTable` on demand (`relation`/`memo` unused).
async fn load_friends(db: &DatabaseConnection, owner_id: i32) -> Vec<crate::character::FriendInfo> {
    // The join is two reads instead of one: `character_friends` declares no
    // foreign key, so there is no relation to traverse — and a friend list is a
    // handful of rows.
    let ids: Vec<i32> = character_friends::Entity::find()
        .filter(character_friends::Column::CharId.eq(owner_id))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| row.friend_id)
        .collect();
    if ids.is_empty() {
        return Vec::new();
    }
    characters::Entity::find()
        .filter(characters::Column::CharId.is_in(ids))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| crate::character::FriendInfo {
            char_id: row.char_id,
            name: row.char_name,
            level: row.level.unwrap_or(0),
            class_id: row.classid.unwrap_or(0),
        })
        .collect()
}

/// A character's `character_quests` rows grouped by quest name (Java
/// `Quest.playerEnter`): the `<state>` rows define which quests exist, the
/// remaining rows fill each one's variable map. Vars for a quest without a
/// state row are orphans — Java warns (or deletes with
/// `AUTODELETE_INVALID_QUEST_DATA`); we drop them from the load.
async fn load_quests(
    db: &DatabaseConnection,
    owner_id: i32,
) -> std::collections::HashMap<String, crate::model::quest::QuestState> {
    use crate::model::quest::{QuestState, STATE_VAR, state};
    let rows = character_quests::Entity::find()
        .filter(character_quests::Column::CharId.eq(owner_id))
        .all(db)
        .await
        .unwrap_or_default();
    let mut out: std::collections::HashMap<String, QuestState> = std::collections::HashMap::new();
    for row in rows.iter().filter(|r| r.var == STATE_VAR) {
        out.insert(
            row.name.clone(),
            QuestState {
                state: state::from_name(row.value.as_deref().unwrap_or_default()),
                ..Default::default()
            },
        );
    }
    for row in rows.iter().filter(|r| r.var != STATE_VAR) {
        if let Some(qs) = out.get_mut(&row.name) {
            qs.vars
                .insert(row.var.clone(), row.value.clone().unwrap_or_default());
        }
    }
    out
}

/// `GrandBossManager.init`: every `grandboss_data` row. The NPC-template
/// filter (`NpcData.getTemplate != null`) runs on the game thread, which owns
/// the datapack; here we just read the table.
/// `Olympiad.load` — the single `olympiad_data` row (defaults if absent: cycle
/// 1, period 0) plus every `olympiad_nobles` record.
async fn load_olympiad(db: &DatabaseConnection) -> DbEvent {
    let data = olympiad_data::Entity::find()
        .filter(olympiad_data::Column::Id.eq(0))
        .one(db)
        .await
        .ok()
        .flatten();
    let (current_cycle, period, olympiad_end, validation_end, next_weekly_change) = match &data {
        Some(r) => (
            r.current_cycle,
            r.period,
            r.olympiad_end,
            r.validation_end,
            r.next_weekly_change,
        ),
        // Java's defaults for a database with no olympiad row yet.
        None => (1, 0, 0, 0, 0),
    };
    let nobles = olympiad_nobles::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| OlympiadNobleRow {
            char_id: r.char_id,
            class_id: r.class_id,
            points: r.olympiad_points,
            comp_done: r.competitions_done,
            comp_won: r.competitions_won,
            comp_lost: r.competitions_lost,
            comp_drawn: r.competitions_drawn,
            comp_done_week: r.competitions_done_week,
        })
        .collect();
    DbEvent::OlympiadLoaded {
        current_cycle,
        period,
        olympiad_end,
        validation_end,
        next_weekly_change,
        nobles,
    }
}

/// `Hero.init` — the currently-crowned heroes (`heroes` rows with `played = 1`).
async fn load_heroes(db: &DatabaseConnection) -> Vec<HeroRow> {
    // The name/clan half of the row lives on `characters`; Java reads it
    // through `CharInfoTable` for the same reason there is no FK to follow.
    let heroes = heroes::Entity::find()
        .filter(heroes::Column::Played.eq(1))
        .all(db)
        .await
        .unwrap_or_default();
    if heroes.is_empty() {
        return Vec::new();
    }
    let chars = characters::Entity::find()
        .filter(characters::Column::CharId.is_in(heroes.iter().map(|h| h.char_id)))
        .all(db)
        .await
        .unwrap_or_default();
    heroes
        .into_iter()
        .map(|h| {
            let c = chars.iter().find(|c| c.char_id == h.char_id);
            HeroRow {
                char_id: h.char_id,
                class_id: h.class_id,
                count: h.count,
                name: c.map(|c| c.char_name.clone()).unwrap_or_default(),
                clan_id: c.and_then(|c| c.clanid).unwrap_or(0),
                message: h.message,
            }
        })
        .collect()
}

/// Every hero-diary entry (Java `Hero.loadDiary` per hero, batched here into one
/// query), oldest first: `(charId, time, action, param)`.
async fn load_hero_diary(db: &DatabaseConnection) -> Vec<(i32, i64, i8, i32)> {
    heroes_diary::Entity::find()
        .order_by_asc(heroes_diary::Column::Time)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| (r.char_id, r.time, r.action as i8, r.param))
        .collect()
}

async fn load_grandboss_data(db: &DatabaseConnection) -> Vec<crate::model::grand_boss::GrandBoss> {
    grandboss_data::Entity::find()
        .order_by_asc(grandboss_data::Column::BossId)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| crate::model::grand_boss::GrandBoss {
            boss_id: r.boss_id,
            loc_x: r.loc_x,
            loc_y: r.loc_y,
            loc_z: r.loc_z,
            heading: r.heading,
            respawn_time: r.respawn_time,
            current_hp: r.current_hp,
            current_mp: r.current_mp,
            status: r.status,
        })
        .collect()
}

/// `CursedWeaponsManager.restore`: every `cursed_weapons` state row.
async fn load_cursed_weapons(db: &DatabaseConnection) -> Vec<CursedWeaponRow> {
    cursed_weapons::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| CursedWeaponRow {
            item_id: r.item_id,
            char_id: r.char_id,
            player_reputation: r.player_reputation.unwrap_or(0),
            player_pk_kills: r.player_pk_kills.unwrap_or(0),
            nb_kills: r.nb_kills.unwrap_or(0),
            end_time: r.end_time,
        })
        .collect()
}

/// `ClanTable`'s boot restore: every `clan_data` row + its member roster
/// from `characters WHERE clanid=?` (Java `Clan.restore`).
/// The stationed siege guards (`castle_siege_guards WHERE isHired=0`) — the
/// non-mercenary garrison spawned at siege start.
async fn load_siege_guards(db: &DatabaseConnection) -> Vec<(i32, crate::model::siege::SiegeSpawn)> {
    castle_siege_guards::Entity::find()
        .filter(castle_siege_guards::Column::IsHired.eq(0))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            (
                r.castle_id,
                crate::model::siege::SiegeSpawn {
                    npc_id: r.npc_id,
                    x: r.x,
                    y: r.y,
                    z: r.z,
                    heading: r.heading,
                },
            )
        })
        .collect()
}

/// `Siege.loadSiegeClan`: every `siege_clans` row.
async fn load_siege_clans(db: &DatabaseConnection) -> Vec<SiegeClanRow> {
    siege_clans::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| SiegeClanRow {
            castle_id: r.castle_id,
            clan_id: r.clan_id,
            kind: r.r#type.unwrap_or(0),
        })
        .collect()
}

/// `CastleManorManager.loadDb`: the `castle_manor_production` rows (seeds the
/// manor sells). Missing table → empty (the manor is simply unset).
async fn load_manor_production(db: &DatabaseConnection) -> Vec<ManorProductionRow> {
    castle_manor_production::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| ManorProductionRow {
            castle_id: r.castle_id,
            seed_id: r.seed_id,
            amount: r.amount.into(),
            start_amount: r.start_amount.into(),
            price: r.price.into(),
            next_period: r.next_period != 0,
        })
        .collect()
}

/// `CastleManorManager.loadDb`: the `castle_manor_procure` rows (crops the manor
/// buys). Missing table → empty.
async fn load_manor_procure(db: &DatabaseConnection) -> Vec<ManorProcureRow> {
    castle_manor_procure::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| ManorProcureRow {
            castle_id: r.castle_id,
            crop_id: r.crop_id,
            amount: r.amount.into(),
            start_amount: r.start_amount.into(),
            price: r.price.into(),
            reward_type: r.reward_type,
            next_period: r.next_period != 0,
        })
        .collect()
}

/// The `clanhall` table — persisted hall ownership (id → owner/paidUntil).
async fn load_clan_hall_owners(db: &DatabaseConnection) -> Vec<ClanHallRow> {
    clanhall::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| ClanHallRow {
            id: r.id,
            owner_id: r.owner_id,
            paid_until: r.paid_until,
        })
        .collect()
}

/// The `clanhall_auctions_bidders` table — the live auction bids.
async fn load_clan_hall_bidders(db: &DatabaseConnection) -> Vec<ClanHallBidRow> {
    clanhall_auctions_bidders::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| ClanHallBidRow {
            hall_id: r.clan_hall_id,
            clan_id: r.clan_id,
            bid: r.bid,
            bid_time: r.bid_time,
        })
        .collect()
}

/// The `residence_functions` table — active hall function upgrades.
async fn load_residence_functions(db: &DatabaseConnection) -> Vec<ResidenceFunctionRow> {
    residence_functions::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| ResidenceFunctionRow {
            residence_id: r.residence_id,
            func_id: r.id,
            level: r.level,
            expiration: r.expiration,
        })
        .collect()
}

/// `CastleManager.load`: every `castle` row (id/name/side).
async fn load_castles(db: &DatabaseConnection) -> Vec<crate::model::castle::Castle> {
    castle::Entity::find()
        .order_by_asc(castle::Column::Id)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| crate::model::castle::Castle {
            id: r.id,
            name: r.name,
            side: crate::model::castle::CastleSide::from_str(&r.side).unwrap_or_default(),
            ticket_buy_count: r.ticket_buy_count,
            // `regTimeOver` is an enum('true','false'); default (missing) is true.
            time_registration_over: r.reg_time_over != "false",
            siege_date: r.siege_date,
            treasury: r.treasury,
        })
        .collect()
}

async fn load_clans(db: &DatabaseConnection) -> Vec<crate::model::clan::Clan> {
    let clan_rows = clan_data::Entity::find().all(db).await.unwrap_or_default();
    let mut out = Vec::with_capacity(clan_rows.len());
    for row in &clan_rows {
        let clan_id = row.clan_id;
        let member_rows = characters::Entity::find()
            .filter(characters::Column::Clanid.eq(clan_id))
            .all(db)
            .await
            .unwrap_or_default();
        // Clan warehouse contents (`owner_id = clan_id`, `loc = "CLANWH"`).
        let wh_rows = load_items(db, clan_id).await;
        // Clan skills (Java `Clan.restoreSkills`) — the main-pledge set
        // (`sub_pledge_id = -2`); sub-unit skills aren't modelled, so other
        // sub_pledge ids are ignored.
        let skills = clan_skills::Entity::find()
            .filter(clan_skills::Column::ClanId.eq(clan_id))
            .filter(clan_skills::Column::SubPledgeId.is_in([-2, 0]))
            .all(db)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|s| (s.skill_id, s.skill_level))
            .collect();
        // Rank → privilege-mask rows (Java `restoreRankPrivs`; rank -1 skipped).
        let rank_privs = clan_privs::Entity::find()
            .filter(clan_privs::Column::ClanId.eq(clan_id))
            .all(db)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r.rank, r.privs))
            .filter(|&(rank, _)| rank != -1)
            .collect();
        // Sub-pledges (Java `Clan.restoreSubPledges`).
        let sub_pledges: std::collections::HashMap<i32, crate::model::clan::SubPledge> =
            clan_subpledges::Entity::find()
                .filter(clan_subpledges::Column::ClanId.eq(clan_id))
                .all(db)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|r| {
                    (
                        r.sub_pledge_id,
                        crate::model::clan::SubPledge {
                            id: r.sub_pledge_id,
                            name: r.name.unwrap_or_default(),
                            leader_id: r.leader_id,
                        },
                    )
                })
                .collect();
        out.push(crate::model::clan::Clan {
            id: clan_id,
            name: row.clan_name.clone().unwrap_or_default(),
            leader_id: row.leader_id.unwrap_or(0),
            level: row.clan_level.unwrap_or(0),
            reputation_score: row.reputation_score,
            castle_id: row.has_castle.unwrap_or(0),
            blood_alliance_count: row.blood_alliance_count,
            char_penalty_expiry_time: row.char_penalty_expiry_time,
            dissolving_expiry_time: row.dissolving_expiry_time,
            rank_privs,
            new_leader_id: row.new_leader_id,
            sub_pledges,
            ally_id: row.ally_id.unwrap_or(0),
            ally_name: row.ally_name.clone().unwrap_or_default(),
            ally_penalty_expiry_time: row.ally_penalty_expiry_time,
            ally_penalty_type: row.ally_penalty_type,
            crest_id: row.crest_id.unwrap_or(0),
            crest_large_id: row.crest_large_id.unwrap_or(0),
            ally_crest_id: row.ally_crest_id.unwrap_or(0),
            skills,
            warehouse: crate::model::inventory::Warehouse::from_rows(&wh_rows),
            members: member_rows
                .into_iter()
                .map(|m| crate::model::clan::ClanMember {
                    char_id: m.char_id,
                    name: m.char_name,
                    level: m.level.unwrap_or(0),
                    class_id: m.classid.unwrap_or(0),
                    sex: m.sex.unwrap_or(0),
                    race: m.race.unwrap_or(0),
                    power_grade: m.power_grade.unwrap_or(0),
                    title: m.title.unwrap_or_default(),
                    pledge_type: m.subpledge,
                })
                .collect(),
        });
    }
    out
}

/// A character's `character_macroses` rows (Java `MacroList.restoreMe`),
/// commands decoded from the `type,d1,d2[,cmd];…` column encoding.
async fn load_macros(db: &DatabaseConnection, owner_id: i32) -> Vec<crate::model::shortcut::Macro> {
    character_macroses::Entity::find()
        .filter(character_macroses::Column::CharId.eq(owner_id))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| crate::model::shortcut::Macro {
            id: r.id,
            icon: r.icon.unwrap_or(0),
            name: r.name.unwrap_or_default(),
            descr: r.descr.unwrap_or_default(),
            acronym: r.acronym.unwrap_or_default(),
            commands: crate::model::shortcut::decode_commands(
                r.commands.as_deref().unwrap_or_default(),
            ),
        })
        .collect()
}

async fn upsert_shortcut(
    db: &DatabaseConnection,
    char_id: i32,
    slot: i32,
    page: i32,
    kind: i32,
    shortcut_id: i32,
    level: i32,
) {
    let row = character_shortcuts::ActiveModel {
        char_id: Set(char_id),
        slot: Set(slot),
        page: Set(page),
        r#type: Set(Some(kind)),
        shortcut_id: Set(Some(shortcut_id.into())),
        level: Set(Some(level)),
        sub_level: Set(0),
        class_index: Set(0),
    };
    let res = character_shortcuts::Entity::insert(row)
        .on_conflict(
            OnConflict::columns([
                character_shortcuts::Column::CharId,
                character_shortcuts::Column::Slot,
                character_shortcuts::Column::Page,
                character_shortcuts::Column::ClassIndex,
            ])
            .update_columns([
                character_shortcuts::Column::Type,
                character_shortcuts::Column::ShortcutId,
                character_shortcuts::Column::Level,
            ])
            .to_owned(),
        )
        .exec(db)
        .await;
    if let Err(e) = res {
        warn!("DB thread: upsert_shortcut failed: {e}");
    }
}

async fn upsert_macro(db: &DatabaseConnection, char_id: i32, m: &crate::model::shortcut::Macro) {
    let row = character_macroses::ActiveModel {
        char_id: Set(char_id),
        id: Set(m.id),
        icon: Set(Some(m.icon)),
        name: Set(Some(m.name.clone())),
        descr: Set(Some(m.descr.clone())),
        acronym: Set(Some(m.acronym.clone())),
        commands: Set(Some(crate::model::shortcut::encode_commands(&m.commands))),
    };
    let res = character_macroses::Entity::insert(row)
        .on_conflict(
            OnConflict::columns([
                character_macroses::Column::CharId,
                character_macroses::Column::Id,
            ])
            .update_columns([
                character_macroses::Column::Icon,
                character_macroses::Column::Name,
                character_macroses::Column::Descr,
                character_macroses::Column::Acronym,
                character_macroses::Column::Commands,
            ])
            .to_owned(),
        )
        .exec(db)
        .await;
    if let Err(e) = res {
        warn!("DB thread: upsert_macro failed: {e}");
    }
}

/// A character's `items` rows (Java: `PlayerInventory.restore`, called for
/// every row shown in `CharSelectionInfo`, not just the entered character).
async fn load_items(db: &DatabaseConnection, owner_id: i32) -> Vec<ItemRow> {
    // Java `PlayerInventory.restore` orders by `loc_data` so a client's saved
    // inventory arrangement (`RequestSaveInventoryOrder`) survives relog.
    let rows = items::Entity::find()
        .filter(items::Column::OwnerId.eq(owner_id))
        .order_by_asc(items::Column::LocData)
        .all(db)
        .await
        .unwrap_or_default();
    // Augmentations (Java `Item.restoreAttributes`): object_id → (mineral, o1, o2).
    let variations: std::collections::HashMap<i32, (i32, i32, i32)> =
        item_variations::Entity::find()
            .filter(item_variations::Column::ItemId.is_in(rows.iter().map(|r| r.object_id)))
            .all(db)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r.item_id, (r.mineral_id, r.option1, r.option2)))
            .collect();
    rows.into_iter()
        .map(|r| {
            let (augment_mineral, augment_option1, augment_option2) =
                variations.get(&r.object_id).copied().unwrap_or((0, 0, 0));
            ItemRow {
                object_id: r.object_id,
                item_id: r.item_id.unwrap_or(0),
                count: r.count,
                enchant_level: r.enchant_level.unwrap_or(0),
                loc: r.loc.unwrap_or_default(),
                loc_data: r.loc_data.unwrap_or(0),
                custom_type1: r.custom_type1.unwrap_or(0),
                custom_type2: r.custom_type2.unwrap_or(0),
                mana_left: r.mana_left,
                time: r.time as i32,
                augment_mineral,
                augment_option1,
                augment_option2,
            }
        })
        .collect()
}

/// Case-insensitive character-name existence check (`getIdByName`).
async fn name_exists(db: &DatabaseConnection, name: &str) -> bool {
    // `COLLATE NOCASE` is the point of this query — two characters may not
    // differ only by case — and sea-query cannot attach a collation, so the
    // comparison stays a bound custom expression.
    characters::Entity::find()
        .filter(models::sea_orm::sea_query::Expr::cust_with_values(
            "char_name = ? COLLATE NOCASE",
            [name],
        ))
        .count(db)
        .await
        .unwrap_or(0)
        > 0
}

/// `characters.createDate` is a `date` column SQLite fills with `date('now')`;
/// the entity carries it as text, so the value is formatted here.
fn today() -> String {
    commons::util::format_date(commons::util::now_millis())
}

/// Runs an insert that the caller treats as best-effort, logging a failure the
/// way the old `exec` helper did.
async fn insert_or_warn<A: models::sea_orm::ActiveModelTrait>(
    db: &DatabaseConnection,
    insert: models::sea_orm::Insert<A>,
) {
    if let Err(e) = insert.exec(db).await {
        warn!("DB thread: insert failed: {e}");
    }
}

/// Logs a failed fire-and-forget write, the way the old `exec` helper did.
///
/// The DB thread must not stop for one bad statement: the game thread has
/// already applied the change in memory and is not waiting for a reply.
fn warn_err<T>(res: Result<T, DbErr>) {
    if let Err(e) = res {
        warn!("DB thread: query failed: {e}");
    }
}

async fn create_character(
    db: &DatabaseConnection,
    next_id: &mut i64,
    max_characters: i32,
    data: &NewCharacter,
) -> CreateResult {
    if name_exists(db, &data.name).await {
        return CreateResult::NameExists;
    }
    let count = characters::Entity::find()
        .filter(characters::Column::AccountName.eq(&data.account))
        .count(db)
        .await
        .unwrap_or(0) as i64;
    if max_characters > 0 && count >= max_characters as i64 {
        return CreateResult::TooMany;
    }

    let char_id = *next_id;
    *next_id += 1;
    // Columns the template does not set keep their DDL defaults, exactly as the
    // old INSERT's column list did. `createDate` is SQLite's `date('now')`.
    let row = characters::ActiveModel {
        account_name: Set(Some(data.account.clone())),
        char_id: Set(char_id as i32),
        char_name: Set(data.name.clone()),
        level: Set(Some(1)),
        max_hp: Set(Some(data.max_hp)),
        cur_hp: Set(Some(f64::from(data.max_hp).into())),
        max_cp: Set(Some(0)),
        cur_cp: Set(Some(0.0.into())),
        max_mp: Set(Some(data.max_mp)),
        cur_mp: Set(Some(f64::from(data.max_mp).into())),
        face: Set(Some(data.face)),
        hair_style: Set(Some(data.hair_style)),
        hair_color: Set(Some(data.hair_color)),
        sex: Set(Some(data.sex)),
        heading: Set(Some(0)),
        x: Set(Some(data.x)),
        y: Set(Some(data.y)),
        z: Set(Some(data.z)),
        exp: Set(Some(0)),
        sp: Set(0),
        reputation: Set(Some(0)),
        race: Set(Some(data.race)),
        classid: Set(Some(data.class_id)),
        base_class: Set(data.class_id),
        deletetime: Set(0),
        title: Set(Some(String::new())),
        accesslevel: Set(Some(0)),
        online: Set(Some(0)),
        char_slot: Set(Some(count as i32)),
        last_access: Set(now_millis()),
        create_date: Set(today()),
        vitality_points: Set(data.vitality_points),
        ..Default::default()
    };
    if let Err(e) = characters::Entity::insert(row).exec(db).await {
        error!("DB thread: character insert failed: {e}");
        return CreateResult::Fail;
    }

    // Seed the recommendation row: Java `Player.create` grants rec_left=20,
    // persisted to `character_reco_bonus` when the freshly-created character
    // disconnects back to the lobby.
    insert_or_warn(
        db,
        character_reco_bonus::Entity::insert(character_reco_bonus::ActiveModel {
            char_id: Set(char_id as i32),
            rec_have: Set(0),
            rec_left: Set(20),
            time_left: Set(0),
        }),
    )
    .await;

    // Initial skills (character_skills).
    for (skill_id, skill_level) in &data.skills {
        insert_or_warn(
            db,
            character_skills::Entity::insert(character_skills::ActiveModel {
                char_id: Set(char_id as i32),
                skill_id: Set(*skill_id),
                skill_level: Set(*skill_level),
                skill_sub_level: Set(0),
                class_index: Set(0),
            }),
        )
        .await;
    }

    // Initial equipment + starting adena. The item_id → object_id map feeds
    // ITEM shortcut resolution below (first occurrence wins, like Java
    // `getItemByItemId`).
    let mut item_object_ids: std::collections::HashMap<i32, i64> = std::collections::HashMap::new();
    for item in &data.items {
        let item_object_id = *next_id;
        *next_id += 1;
        item_object_ids
            .entry(item.item_id)
            .or_insert(item_object_id);
        let (loc, loc_data) = match item.paperdoll_index {
            Some(slot) => ("PAPERDOLL", slot as i32),
            None => ("INVENTORY", 0),
        };
        insert_or_warn(
            db,
            items::Entity::insert(items::ActiveModel {
                owner_id: Set(Some(char_id as i32)),
                object_id: Set(item_object_id as i32),
                item_id: Set(Some(item.item_id)),
                count: Set(item.count),
                enchant_level: Set(Some(0)),
                loc: Set(Some(loc.to_string())),
                loc_data: Set(Some(loc_data)),
                custom_type1: Set(Some(0)),
                custom_type2: Set(Some(0)),
                mana_left: Set(-1),
                time: Set(0),
                ..Default::default()
            }),
        )
        .await;
    }

    // Initial shortcuts + macro presets (`InitialShortcutData.
    // registerAllShortcuts` — persistence only; there's no in-world session to
    // echo packets to at creation).
    for sc in &data.shortcuts {
        let shortcut_id = if sc.kind == crate::model::shortcut::ShortcutType::Item {
            // ITEM entries reference an item id; skip ones the new character
            // didn't actually receive (Java `continue`s).
            match item_object_ids.get(&sc.id) {
                Some(&object_id) => object_id as i32,
                None => continue,
            }
        } else {
            sc.id
        };
        upsert_shortcut(
            db,
            char_id as i32,
            sc.slot,
            sc.page,
            sc.kind.ordinal(),
            shortcut_id,
            sc.level,
        )
        .await;
    }
    for m in &data.macros {
        upsert_macro(db, char_id as i32, m).await;
    }
    info!(
        "Created character '{}' ({}) for account {} with {} initial skill(s), {} item(s).",
        data.name,
        char_id,
        data.account,
        data.skills.len(),
        data.items.len()
    );
    CreateResult::Ok
}

/// Java `Player.storeCharBase` (narrowed to the tracked columns, see
/// [`PlayerSnapshot`]) + `updateOnlineStatus` — the character leaves the world,
/// so `online=0` and `lastAccess=now` in the same write.
/// Flush a whole player to the database in one transaction — the only path that
/// writes character-owned gameplay state (memory-first model). Reconciles the
/// `characters` row plus every child table (items, skills, shortcuts, macros,
/// quests) so a single flush captures pickups, drops, equips, skill changes,
/// shortcut/macro edits and quest progress made since the last flush. Child
/// tables are rewritten wholesale (delete-this-owner + re-insert), which is
/// atomic inside the transaction and doubles as the delete path — anything no
/// longer in memory is gone from the DB after the flush. On any error the
/// transaction is dropped (rolled back) and logged, leaving the last good save
/// intact.
async fn store_player(db: &DatabaseConnection, s: &PlayerSaveData) {
    if let Err(e) = store_player_tx(db, s).await {
        error!(
            "store_player: flush for char {} failed (rolled back): {e}",
            s.base.object_id
        );
    }
}

async fn store_player_tx(db: &DatabaseConnection, s: &PlayerSaveData) -> Result<(), DbErr> {
    let b = &s.base;
    let char_id = b.object_id;
    let tx = db.begin().await?;

    // characters row (Java storeCharBase). `online` stays 0: the port never
    // sets it to 1, and char-select doesn't read it — a periodic save of an
    // online player must not diverge from that. Columns left `NotSet` keep
    // their stored values, which is what the old UPDATE's column list did.
    characters::ActiveModel {
        char_id: Unchanged(char_id),
        level: Set(Some(b.level)),
        max_hp: Set(Some(b.max_hp)),
        cur_hp: Set(Some(b.cur_hp.into())),
        max_cp: Set(Some(b.max_cp)),
        cur_cp: Set(Some(b.cur_cp.into())),
        max_mp: Set(Some(b.max_mp)),
        cur_mp: Set(Some(b.cur_mp.into())),
        face: Set(Some(b.face)),
        hair_style: Set(Some(b.hair_style)),
        hair_color: Set(Some(b.hair_color)),
        sex: Set(Some(b.sex)),
        heading: Set(Some(b.heading)),
        x: Set(Some(b.x)),
        y: Set(Some(b.y)),
        z: Set(Some(b.z)),
        exp: Set(Some(b.exp)),
        sp: Set(b.sp),
        reputation: Set(Some(b.reputation)),
        pvpkills: Set(Some(b.pvp_kills)),
        pkkills: Set(Some(b.pk_kills)),
        raidboss_points: Set(b.raidboss_points),
        race: Set(Some(b.race)),
        classid: Set(Some(b.class_id)),
        base_class: Set(b.base_class_id),
        vitality_points: Set(b.vitality_points),
        pccafe_points: Set(b.pccafe_points),
        nobless: Set(if b.noble { 1 } else { 0 }),
        online: Set(Some(0)),
        last_access: Set(now_millis()),
        ..Default::default()
    }
    .update(&tx)
    .await?;

    // character_reco_bonus (Java `Player.storeRecommendations`). `time_left` is
    // always 0 — the reco bonus timer isn't used in Interlude Classic.
    character_reco_bonus::Entity::insert(character_reco_bonus::ActiveModel {
        char_id: Set(char_id),
        rec_have: Set(b.rec_have),
        rec_left: Set(b.rec_left),
        time_left: Set(0),
    })
    .on_conflict(
        OnConflict::column(character_reco_bonus::Column::CharId)
            .update_columns([
                character_reco_bonus::Column::RecHave,
                character_reco_bonus::Column::RecLeft,
                character_reco_bonus::Column::TimeLeft,
            ])
            .to_owned(),
    )
    .exec(&tx)
    .await?;

    // items (inventory + equipped): `Inventory::to_rows` is the whole owned set.
    items::Entity::delete_many()
        .filter(items::Column::OwnerId.eq(char_id))
        .exec(&tx)
        .await?;
    for it in &s.items {
        items::Entity::insert(items::ActiveModel {
            owner_id: Set(Some(char_id)),
            object_id: Set(it.object_id),
            item_id: Set(Some(it.item_id)),
            count: Set(it.count),
            enchant_level: Set(Some(it.enchant_level)),
            loc: Set(Some(it.loc.clone())),
            loc_data: Set(Some(it.loc_data)),
            custom_type1: Set(Some(it.custom_type1)),
            custom_type2: Set(Some(it.custom_type2)),
            mana_left: Set(it.mana_left),
            time: Set(it.time.into()),
            ..Default::default()
        })
        .exec(&tx)
        .await?;
    }

    // Augmentations, keyed to the item rows just written (the old statement
    // sub-selected the same set).
    item_variations::Entity::delete_many()
        .filter(item_variations::Column::ItemId.is_in(s.items.iter().map(|it| it.object_id)))
        .exec(&tx)
        .await?;
    for it in s
        .items
        .iter()
        .filter(|it| it.augment_option1 != 0 || it.augment_option2 != 0)
    {
        item_variations::Entity::insert(item_variations::ActiveModel {
            item_id: Set(it.object_id),
            mineral_id: Set(it.augment_mineral),
            option1: Set(it.augment_option1),
            option2: Set(it.augment_option2),
        })
        .exec(&tx)
        .await?;
    }

    character_skills::Entity::delete_many()
        .filter(character_skills::Column::CharId.eq(char_id))
        .exec(&tx)
        .await?;

    character_hennas::Entity::delete_many()
        .filter(character_hennas::Column::CharId.eq(char_id))
        .exec(&tx)
        .await?;
    let mut henna_idx: Vec<(i32, &Vec<(i32, i32)>)> =
        s.hennas_by_index.iter().map(|(i, v)| (*i, v)).collect();
    henna_idx.push((s.class_index, &s.hennas));
    for (class_index, hennas) in henna_idx {
        for (slot, symbol_id) in hennas {
            character_hennas::Entity::insert(character_hennas::ActiveModel {
                char_id: Set(char_id),
                symbol_id: Set(Some(*symbol_id)),
                slot: Set(*slot),
                class_index: Set(class_index),
            })
            .on_conflict(
                OnConflict::columns([
                    character_hennas::Column::CharId,
                    character_hennas::Column::Slot,
                    character_hennas::Column::ClassIndex,
                ])
                .update_column(character_hennas::Column::SymbolId)
                .to_owned(),
            )
            .exec(&tx)
            .await?;
        }
    }
    let mut per_index: Vec<(i32, &Vec<(i32, i32, i32)>)> =
        s.skills_by_index.iter().map(|(i, v)| (*i, v)).collect();
    per_index.push((s.class_index, &s.skills));
    for (class_index, skills) in per_index {
        for (skill_id, level, sub_level) in skills {
            character_skills::Entity::insert(character_skills::ActiveModel {
                char_id: Set(char_id),
                skill_id: Set(*skill_id),
                skill_level: Set(*level),
                skill_sub_level: Set(*sub_level),
                class_index: Set(class_index),
            })
            .on_conflict(
                OnConflict::columns([
                    character_skills::Column::CharId,
                    character_skills::Column::SkillId,
                    character_skills::Column::ClassIndex,
                ])
                .update_columns([
                    character_skills::Column::SkillLevel,
                    character_skills::Column::SkillSubLevel,
                ])
                .to_owned(),
            )
            .exec(&tx)
            .await?;
        }
    }

    character_recipebook::Entity::delete_many()
        .filter(character_recipebook::Column::CharId.eq(char_id))
        .filter(character_recipebook::Column::ClassIndex.eq(0))
        .exec(&tx)
        .await?;
    for (list_id, is_dwarven) in &s.recipe_book {
        character_recipebook::Entity::insert(character_recipebook::ActiveModel {
            char_id: Set(char_id),
            id: Set((*list_id).into()),
            class_index: Set(0),
            r#type: Set(if *is_dwarven { 1 } else { 0 }),
        })
        .exec(&tx)
        .await?;
    }

    character_variables::Entity::delete_many()
        .filter(character_variables::Column::CharId.eq(char_id))
        .exec(&tx)
        .await?;
    for (var, val) in &s.variables {
        character_variables::Entity::insert(character_variables::ActiveModel {
            char_id: Set(char_id),
            var: Set(var.clone()),
            val: Set(val.clone()),
        })
        .exec(&tx)
        .await?;
    }

    for pet in &s.pets {
        pets::Entity::insert(pets::ActiveModel {
            item_obj_id: Set(pet.collar_object_id),
            name: Set(Some(pet.name.clone())),
            level: Set(pet.level),
            cur_hp: Set(Some(pet.cur_hp.into())),
            cur_mp: Set(Some(pet.cur_mp.into())),
            exp: Set(Some(pet.exp)),
            sp: Set(Some(pet.sp)),
            fed: Set(Some(pet.fed)),
            owner_id: Set(char_id),
            restore: Set(if pet.restore { "true" } else { "false" }.to_string()),
        })
        .on_conflict(
            OnConflict::column(pets::Column::ItemObjId)
                .update_columns([
                    pets::Column::Name,
                    pets::Column::Level,
                    pets::Column::CurHp,
                    pets::Column::CurMp,
                    pets::Column::Exp,
                    pets::Column::Sp,
                    pets::Column::Fed,
                    pets::Column::OwnerId,
                    pets::Column::Restore,
                ])
                .to_owned(),
        )
        .exec(&tx)
        .await?;
    }

    // Summons are best-effort, as they were before: a servitor that fails to
    // persist costs a resummon, and must not roll back the character save.
    let _ = character_summons::Entity::delete_many()
        .filter(character_summons::Column::OwnerId.eq(char_id))
        .exec(&tx)
        .await;
    for summon in &s.summons {
        let _ = character_summons::Entity::insert(character_summons::ActiveModel {
            owner_id: Set(char_id),
            summon_id: Set(0),
            summon_skill_id: Set(summon.summon_skill_id),
            cur_hp: Set(Some(summon.cur_hp)),
            cur_mp: Set(Some(summon.cur_mp)),
            time: Set(summon.remaining_secs),
        })
        .exec(&tx)
        .await;
        let _ = character_summon_skills_save::Entity::delete_many()
            .filter(character_summon_skills_save::Column::OwnerId.eq(char_id))
            .filter(character_summon_skills_save::Column::OwnerClassIndex.eq(0))
            .filter(character_summon_skills_save::Column::SummonSkillId.eq(summon.summon_skill_id))
            .exec(&tx)
            .await;
        for (i, buff) in summon.buffs.iter().enumerate() {
            let _ = character_summon_skills_save::Entity::insert(
                character_summon_skills_save::ActiveModel {
                    owner_id: Set(char_id),
                    owner_class_index: Set(0),
                    summon_skill_id: Set(summon.summon_skill_id),
                    skill_id: Set(buff.skill_id),
                    skill_level: Set(buff.skill_level),
                    skill_sub_level: Set(0),
                    remaining_time: Set(buff.remaining_time_secs),
                    buff_index: Set(i as i32),
                },
            )
            .exec(&tx)
            .await;
        }
    }

    character_shortcuts::Entity::delete_many()
        .filter(character_shortcuts::Column::CharId.eq(char_id))
        .exec(&tx)
        .await?;
    let mut sc_idx: Vec<(i32, &Vec<crate::model::shortcut::Shortcut>)> =
        s.shortcuts_by_index.iter().map(|(i, v)| (*i, v)).collect();
    sc_idx.push((s.class_index, &s.shortcuts));
    for (class_index, shortcuts) in sc_idx {
        for sc in shortcuts {
            character_shortcuts::Entity::insert(character_shortcuts::ActiveModel {
                char_id: Set(char_id),
                slot: Set(sc.slot),
                page: Set(sc.page),
                r#type: Set(Some(sc.kind.ordinal())),
                shortcut_id: Set(Some(sc.id.into())),
                level: Set(Some(sc.level)),
                sub_level: Set(0),
                class_index: Set(class_index),
            })
            .on_conflict(
                OnConflict::columns([
                    character_shortcuts::Column::CharId,
                    character_shortcuts::Column::Slot,
                    character_shortcuts::Column::Page,
                    character_shortcuts::Column::ClassIndex,
                ])
                .update_columns([
                    character_shortcuts::Column::Type,
                    character_shortcuts::Column::ShortcutId,
                    character_shortcuts::Column::Level,
                ])
                .to_owned(),
            )
            .exec(&tx)
            .await?;
        }
    }

    character_macroses::Entity::delete_many()
        .filter(character_macroses::Column::CharId.eq(char_id))
        .exec(&tx)
        .await?;
    for m in &s.macros {
        character_macroses::Entity::insert(character_macroses::ActiveModel {
            char_id: Set(char_id),
            id: Set(m.id),
            icon: Set(Some(m.icon)),
            name: Set(Some(m.name.clone())),
            descr: Set(Some(m.descr.clone())),
            acronym: Set(Some(m.acronym.clone())),
            commands: Set(Some(crate::model::shortcut::encode_commands(&m.commands))),
        })
        .exec(&tx)
        .await?;
    }

    character_quests::Entity::delete_many()
        .filter(character_quests::Column::CharId.eq(char_id))
        .exec(&tx)
        .await?;
    for (name, qs) in &s.quests {
        use crate::model::quest::{STATE_VAR, state};
        if qs.state == state::CREATED && qs.vars.is_empty() {
            continue;
        }
        character_quests::Entity::insert(character_quests::ActiveModel {
            char_id: Set(char_id),
            name: Set(name.clone()),
            var: Set(STATE_VAR.to_string()),
            value: Set(Some(state::name(qs.state).to_string())),
        })
        .exec(&tx)
        .await?;
        for (var, value) in &qs.vars {
            character_quests::Entity::insert(character_quests::ActiveModel {
                char_id: Set(char_id),
                name: Set(name.clone()),
                var: Set(var.clone()),
                value: Set(Some(value.clone())),
            })
            .exec(&tx)
            .await?;
        }
    }

    character_skills_save::Entity::delete_many()
        .filter(character_skills_save::Column::CharId.eq(char_id))
        .filter(character_skills_save::Column::ClassIndex.eq(s.class_index))
        .exec(&tx)
        .await?;
    for (i, b) in s.skill_buffs.iter().enumerate() {
        character_skills_save::Entity::insert(character_skills_save::ActiveModel {
            char_id: Set(char_id),
            skill_id: Set(b.skill_id),
            skill_level: Set(b.skill_level),
            skill_sub_level: Set(0),
            remaining_time: Set(b.remaining_time_secs),
            reuse_delay: Set(0),
            systime: Set(0),
            restore_type: Set(0),
            class_index: Set(s.class_index),
            buff_index: Set(i as i32 + 1),
        })
        .exec(&tx)
        .await?;
    }
    let buff_rows = s.skill_buffs.len() as i32;
    for (i, r) in s.skill_reuses.iter().enumerate() {
        character_skills_save::Entity::insert(character_skills_save::ActiveModel {
            char_id: Set(char_id),
            skill_id: Set(r.reuse_key),
            skill_level: Set(r.skill_level),
            skill_sub_level: Set(0),
            remaining_time: Set(-1),
            reuse_delay: Set(r.reuse_delay),
            systime: Set(r.systime_ms),
            restore_type: Set(1),
            class_index: Set(s.class_index),
            buff_index: Set(buff_rows + i as i32 + 1),
        })
        .exec(&tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Character count + pending-deletion timestamps for the login server's
/// `ReplyCharacters` (Java `LoginServerThread.getCharsOnServer`). Mirrors
/// [`load_characters`]: a character whose deletion timer has **expired** is
/// purged and excluded, so the login server-select count never exceeds the
/// char-select list the client sees on entry (the port has no separate global
/// expired-char sweep, so counting raw rows would over-report).
async fn count_characters(db: &DatabaseConnection, account: &str) -> (u8, Vec<i64>) {
    let rows = characters::Entity::find()
        .filter(characters::Column::AccountName.eq(account))
        .all(db)
        .await
        .unwrap_or_default();
    let now = now_millis();
    let mut count: u8 = 0;
    let mut del_times = Vec::new();
    for row in &rows {
        if row.deletetime > 0 && now > row.deletetime {
            delete_char(db, row.char_id).await; // restoreChar: purge expired
            continue;
        }
        count += 1;
        if row.deletetime != 0 {
            del_times.push(row.deletetime); // still counting down toward deletion
        }
    }
    (count, del_times)
}

async fn delete_char(db: &DatabaseConnection, char_id: i32) {
    if let Err(e) = characters::Entity::delete_by_id(char_id).exec(db).await {
        warn!("DB thread: delete_char failed: {e}");
    }
}

/// `ClanTable.restoreClanWars` — the `clan_wars` table (ids in the varchar
/// columns, as Java writes them).
async fn load_clan_wars(db: &DatabaseConnection) -> Vec<crate::model::clan::ClanWar> {
    // `clan1`/`clan2`/`winnerClan` are `varchar(35)` holding clan ids, so the
    // stored values are text. Java reads them with `rset.getInt`, which coerces;
    // the parse here is that coercion. (The pre-ORM code asked sqlx for an i64
    // and silently got 0 for every row, which made every restored war a war
    // between clan 0 and clan 0.)
    fn id(raw: &str) -> i32 {
        raw.trim().parse().unwrap_or(0)
    }
    clan_wars::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| crate::model::clan::ClanWar {
            attacker_id: id(&r.clan1),
            attacked_id: id(&r.clan2),
            attacker_kills: r.clan1_kill,
            attacked_kills: r.clan2_kill,
            winner_id: id(&r.winner_clan),
            start_time: r.start_time,
            end_time: r.end_time,
            state: crate::model::clan::ClanWarState::from_i32(r.state),
        })
        .collect()
}

/// `CrestTable.load` — every stored crest bitmap (`crests` table).
async fn load_crests(db: &DatabaseConnection) -> Vec<crate::model::clan::Crest> {
    crests::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| crate::model::clan::Crest {
            id: r.crest_id,
            data: r.data,
            kind: r.r#type,
        })
        .collect()
}

/// `ClanEntryManager.load`'s `pledge_recruit` half (the boot-time removal of
/// entries for clans that no longer exist is done by the caller, which
/// already has the loaded clan set).
async fn load_recruit_clans(
    db: &DatabaseConnection,
) -> Vec<crate::model::clan_entry::PledgeRecruitInfo> {
    pledge_recruit::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| crate::model::clan_entry::PledgeRecruitInfo {
            clan_id: r.clan_id,
            karma: r.karma,
            information: r.information,
            detailed_information: r.detailed_information,
            application_type: r.application_type,
            recruit_type: r.recruit_type,
        })
        .collect()
}

/// `ClanEntryManager.load`'s `pledge_waiting_list` half (joined with
/// `characters` for the display fields, as Java's own query does).
async fn load_recruit_waiting(
    db: &DatabaseConnection,
) -> Vec<crate::model::clan_entry::PledgeWaitingInfo> {
    let waiting = pledge_waiting_list::Entity::find()
        .all(db)
        .await
        .unwrap_or_default();
    if waiting.is_empty() {
        return Vec::new();
    }
    // Java's query LEFT JOINs `characters` for the display fields; an applicant
    // whose character is gone keeps the row with empty values.
    let chars = characters::Entity::find()
        .filter(characters::Column::CharId.is_in(waiting.iter().map(|w| w.char_id)))
        .all(db)
        .await
        .unwrap_or_default();
    waiting
        .into_iter()
        .map(|w| {
            let c = chars.iter().find(|c| c.char_id == w.char_id);
            crate::model::clan_entry::PledgeWaitingInfo {
                player_id: w.char_id,
                level: c.and_then(|c| c.level).unwrap_or(0),
                karma: w.karma,
                class_id: c.map(|c| c.base_class).unwrap_or(0),
                name: c.map(|c| c.char_name.clone()).unwrap_or_default(),
            }
        })
        .collect()
}

/// `ClanEntryManager.load`'s `pledge_applicant` half.
async fn load_recruit_applicants(
    db: &DatabaseConnection,
) -> Vec<crate::model::clan_entry::PledgeApplicantInfo> {
    let applicants = pledge_applicant::Entity::find()
        .all(db)
        .await
        .unwrap_or_default();
    if applicants.is_empty() {
        return Vec::new();
    }
    let chars = characters::Entity::find()
        .filter(characters::Column::CharId.is_in(applicants.iter().map(|a| a.char_id)))
        .all(db)
        .await
        .unwrap_or_default();
    applicants
        .into_iter()
        .map(|a| {
            let c = chars.iter().find(|c| c.char_id == a.char_id);
            crate::model::clan_entry::PledgeApplicantInfo {
                player_id: a.char_id,
                name: c.map(|c| c.char_name.clone()).unwrap_or_default(),
                level: c.and_then(|c| c.level).unwrap_or(0),
                karma: a.karma,
                clan_id: a.clan_id,
                message: a.message,
            }
        })
        .collect()
}
