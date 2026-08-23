//! Port of `gameserver/model` — the game domain. G4 introduces the composed
//! `Player` (challenge #1: composition over inheritance) with just enough state
//! to enter the world and display correctly. Inventory, skills, effects, and the
//! full stat pipeline arrive in later milestones.

pub mod boat;
pub mod castle;
pub mod clan;
pub mod clan_entry;
pub mod clan_hall;
pub mod command_channel;
pub mod components;
pub mod cursed_weapon;
pub mod door;
pub mod enchant_bonus;
pub mod event;
pub mod formulas;
pub mod grand_boss;
pub mod instance;
pub mod inventory;
pub mod item_auction;
pub mod lottery;
pub mod mail;
pub mod manor;
pub mod matching_room;
pub mod mob_group;
pub mod monster_race;
pub mod movement;
pub mod npc;
pub mod olympiad;
pub mod party;
pub mod petition;
pub mod punishment;
pub mod quest;
pub mod shortcut;
pub mod siege;
pub mod skill;
pub mod static_object;
pub mod stats;

use crate::character::CharData;
use crate::data::GameData;
use crate::data::admin_data::AccessLevel;
use crate::data::player_template::PlayerTemplate;

/// Client-default name/title colors for a normal (level-0) player, matching a
/// real UserInfo capture. See [`Player::name_color`].
pub const DEFAULT_NAME_COLOR: i32 = 0x00FF_FFFF;
pub const DEFAULT_TITLE_COLOR: i32 = 0x00FF_FF77;

/// `PlayerStat.MAX_VITALITY_POINTS` / `MIN_VITALITY_POINTS` — the bounds every
/// vitality read and write clamps to. Lives here (rather than in
/// `game_loop::vitality`) because both the config loader and the stat code need
/// them.
pub const MAX_VITALITY_POINTS: i32 = 140_000;
pub const MIN_VITALITY_POINTS: i32 = 0;
use components::{
    AttackState, BaseStats, Buffs, ClientPos, Collision, CombatStats, Macros, PlayerVitals,
    Position, RegionCell, Reuses, Shortcuts, SkillBook, Speeds, StatModifiers, TargetRef, Vitals,
};
use inventory::Inventory;
use skill::{ActiveBuff, BuffSlot, StatModifierEffect};
use stats::{BaseStat, Stat, StatModifierType};

/// Java `SkillCaster`'s per-cast state, one NORMAL casting slot (no dual
/// casting in Interlude). Owned by the casting `Player`; the scheduler's
/// phase tasks carry `seq` and no-op when it no longer matches (see
/// `Scheduler`'s dead-id contract) — that mismatch is how an aborted cast
/// "cancels" its already-queued tasks without touching the heap.
#[derive(Debug, Clone)]
pub struct CastState {
    pub skill_id: i32,
    pub skill_level: i32,
    /// The enchant sub-level the cast was started with (0 = plain) — the
    /// launch/finish/channeling phases re-resolve through it so an enchanted
    /// cast lands its enchanted effects (PLAN_G19_SKILL_ENCHANT.md).
    pub skill_sub_level: i32,
    /// Aiming target snapshotted at cast start (Java `SkillCaster._target`).
    pub target_object_id: i32,
    /// Generation counter from `Player.cast_seq`.
    pub seq: u64,
    /// Java `canAbortCast()`: a cast can only be aborted before `launchSkill`
    /// resolves its targets.
    pub launched: bool,
    /// `SkillCaster._cancelTime`/`_coolTime` (ms), fixed at cast start so a
    /// mid-cast stat change can't shift the already-announced timing.
    pub cancel_ms: i32,
    pub cool_ms: i32,
    /// Java `SkillCaster._item` — the inventory instance whose item-skill
    /// started this cast, when that item is a `SKILL_REDUCE_ON_SKILL_SUCCESS`
    /// one (spent by `finishSkill` if the cast lands, not at use). `0`
    /// otherwise, which is every other cast.
    pub trigger_item_object_id: i32,
}

/// The player's current AI intention beyond standing/moving (Java
/// `CtrlIntention` narrowed to what exists). `Attack` keeps auto-attacking
/// (and walking into range of) the target until it dies, the player cancels
/// (Esc / move click), or the player dies — `PlayerAI.thinkAttack`'s loop.
/// `Cast` walks into cast range of the snapshotted target and then casts —
/// `PlayerAI.thinkCast` → `maybeMoveToPawn`. `Interact` walks into an NPC's
/// talk range and then re-runs the interact click — `PlayerAI.thinkInteract`
/// → `maybeMoveToPawn` → `Player.doInteract` re-dispatching `onAction`.
/// `PickUp` walks to a ground item and lifts it once inside reach —
/// `PlayerAI.thinkPickUp` → `maybeMoveToPawn(target, 36)` →
/// `Player.doPickupItem`. All four are driven from the combat tick system and
/// cleared by the same cancel paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerIntent {
    Attack {
        target_object_id: i32,
    },
    Cast {
        skill_id: i32,
        ctrl: bool,
        shift: bool,
        target_object_id: i32,
    },
    Interact {
        target_object_id: i32,
    },
    /// Java `AI_INTENTION_PICK_UP`. The object id is the ground item's, held
    /// the same way `AbstractAI.setTarget` holds it — an AI-local field, not
    /// the player's real target (that `setTarget` is the AI's, which only
    /// assigns `_target` and sends no packet).
    PickUp {
        item_object_id: i32,
    },
}

/// One live cooldown (Java `TimeStamp`, trimmed): `SkillCoolTime` reports the
/// map key (reuse group or skill id) plus the level it was cast at, so the
/// level rides along here instead of being re-looked-up from `skills`.
#[derive(Debug, Clone, Copy)]
pub struct SkillReuse {
    pub skill_level: i32,
    /// Absolute tick the cooldown ends at.
    pub until_tick: u64,
    /// Full reuse duration in ms (Java `TimeStamp.getReuse()`).
    pub total_ms: i32,
}

/// Java `SubClassHolder` — one subclass slot's saved progress.
/// A pending resurrection proposal (Java's `_revive*` fields).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReviveRequest {
    pub reviver: i32,
    pub restore_percent: f64,
    pub hp_percent: i32,
    pub mp_percent: i32,
    pub cp_percent: i32,
    /// Java `_revivePet` — the proposal targets the player's **pet**, not the
    /// player. The dialog still goes to the owner, which is why one field on
    /// the player carries both cases.
    pub is_pet: bool,
}

/// Java's `SummonRequestHolder` — a pending Summon Friend prompt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SummonRequest {
    pub summoner_object_id: i32,
    /// Where the summoner stood **when the prompt was sent**. Java stores a
    /// `Location`, not the summoner, so walking away during the 30 s window
    /// still pulls the target to the cast site rather than to the new one.
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubClass {
    pub class_id: i32,
    /// 1..=`MaxSubclass`; 0 is reserved for the base class.
    pub class_index: i32,
    pub level: i32,
    pub exp: i64,
    pub sp: i64,
}

/// The player residual core component: identity, class/appearance,
/// progression counters, and the few flags nothing sweeps — everything
/// system-shaped lives in the extracted components (`model/components.rs`;
/// PLAN_ECS_STAGE2 §2). Owned by the `World` object registry once in game;
/// the `InGame` session links to it by `object_id`.
#[derive(Debug, Clone, bevy_ecs::component::Component)]
pub struct Player {
    pub object_id: i32,
    pub name: String,
    pub account: String,
    pub title: String,

    /// `characters.accesslevel` — GM tier, indexing the [`AdminData`] table
    /// (0 = normal player). Drives [`Player::is_gm`], admin-command gating, and
    /// the name/title colors below.
    pub access_level: i32,
    /// Name/title colors resolved from the access level at load (Java sets
    /// these on `_appearance` in `setAccessLevel`). A level-0 player keeps the
    /// client defaults ([`DEFAULT_NAME_COLOR`]/[`DEFAULT_TITLE_COLOR`]) — the
    /// datapack's `User` row (`ECF9A2` title) is a Mobius quirk the retail
    /// client does not send, and a real UserInfo capture uses the defaults.
    pub name_color: i32,
    pub title_color: i32,
    /// Hero glow shown in CharInfo/UserInfo: `isHero() || (isGM() &&
    /// GMHeroAura)`. Recomputed whenever [`is_hero`](Self::is_hero) changes.
    pub hero_aura: bool,
    /// Java `Player._noble` — Olympiad nobless. Grants the noble skill tree,
    /// unlocks the noblesse teleport lists and Advanced Headquarters (326).
    pub is_noble: bool,
    /// Java `Player._classIndex` — 0 is the base class, 1..=`MaxSubclass` are
    /// the subclass slots. Everything the character *is* right now (class_id,
    /// level, exp, sp, learned skills) belongs to this slot.
    pub class_index: i32,
    /// Java `Player.getSubClasses()`, keyed by class index. The *inactive*
    /// slots: the active one's progress lives in the ordinary `level`/`exp`/
    /// `sp` fields and is written back on switch.
    pub subclasses: Vec<SubClass>,
    /// The base class's banked progress while a subclass is active. Java keeps
    /// the base row in `characters` and only ever writes the *active* class
    /// there; the port stashes it here so a switch back restores it without a
    /// DB round-trip.
    /// The *inactive* class indices' learned skills. The active index's book
    /// lives in the `SkillBook` component; a switch moves it here and takes the
    /// target's out.
    pub skills_by_index: std::collections::HashMap<i32, Vec<(i32, i32, i32)>>,
    /// Java `Creature._team` (`//setteam` — 0 none, 1 blue, 2 red): the aura
    /// circle color in UserInfo/CharInfo. Transient, like Java (not persisted).
    pub team: u8,
    /// Java `Player._isOnEvent` — the player is *inside* a running event's
    /// arena (a TvT fight, G28). Gates cancel-registration, targeting and
    /// several event checks. Transient.
    pub on_event: bool,
    /// Java `Player._isRegisteredOnEvent` — the player has signed up for an
    /// event but the fight hasn't teleported them in yet (TvT registration
    /// window, G28). Blocks registering for a second event. Transient.
    pub registered_on_event: bool,
    /// Inactive indices' worn hennas — dyes are per-subclass.
    pub hennas_by_index: std::collections::HashMap<i32, Vec<(i32, i32)>>,
    /// Inactive indices' shortcut bars.
    pub shortcuts_by_index: std::collections::HashMap<i32, Vec<shortcut::Shortcut>>,
    pub base_level: i32,
    pub base_exp: i64,
    pub base_sp: i64,
    /// Java `Player._hero`. Set by the olympiad's period-end crowning
    /// (`olympiad::crown` → `admin::hero::set_hero`, G25) and cleared when the
    /// next period starts; a fresh session loads whatever was persisted.
    /// `//sethero` toggles it by hand (grant/remove the hero skill tree +
    /// refresh the aura).
    pub is_hero: bool,
    /// Java `Player._trueHero` — a *second*, independent hero flag with its own
    /// `100 : 0` byte in both `CharInfo` and `UserInfo`, separate from the
    /// `hero_aura` byte above. Java only ever sets it from `//settruehero`
    /// (`AdminAdmin`) and never persists it, so it is transient here too.
    /// The port had the byte hard-coded to 0, which made the flag untestable.
    pub true_hero: bool,
    /// Java `Player._teleportType` — the GM click-to-move latch armed from the
    /// "Additional Movement Options" window (`//instant_move`, `//teleto
    /// sayune|charge|end`). Consumed by `MoveBackwardToLocation`. Transient,
    /// exactly like Java (a field, no DB column).
    pub tele_mode: crate::enums::AdminTeleportType,
    /// Java `Player._blinkActive`, set by every `FlyToLocation` sent for a
    /// player (the packet's own constructor does it) and consumed by the next
    /// `ValidatePosition`: it skips the out-of-sync snap once, so the slide the
    /// server just performed is not immediately reverted to the position the
    /// client is still reporting from before the fly.
    pub blink_active: bool,
    /// Java `Player._fallingTimestamp` — the tick until which further
    /// `ValidatePosition` reports are ignored, so a fall in progress cannot be
    /// "corrected" into a jump. Armed by `setFalling()` for
    /// `FALLING_VALIDATION_DELAY` (1 s) on every report that continues the
    /// fall, and cleared the moment a report lands within the safe height or
    /// over ungeodata'd ground. `0` = not falling.
    pub falling_until_tick: u64,

    pub level: i32,
    pub class_id: i32,
    pub base_class_id: i32,
    pub race: i32,
    pub is_female: bool,

    // Extracted components on the same entity (stage 2 —
    // `model/components.rs`): Position/RegionCell (phase 1); Vitals (+ CP in
    // PlayerVitals), BaseStats, Speeds, Collision (phase 2).
    pub exp: i64,
    pub sp: i64,
    pub reputation: i32,
    pub pk_kills: i32,
    pub pvp_kills: i32,
    /// `characters.raidbossPoints` — accumulated raid points. Awarded on a
    /// raid boss kill and spent on clan reputation; a pure counter here.
    pub raidboss_points: i32,
    /// Java `Player._cursedWeaponEquippedId` — the cursed weapon item id the
    /// player currently wields (0 = none). Set by `CursedWeapon.activate`,
    /// cleared by `endOfLife`; suppresses karma decay and gates un-equip.
    pub cursed_weapon_equipped_id: i32,
    /// Java `Player._charges` — the warrior "Force" resource (Sonic Focus →
    /// Sonic Blaster/Buster, and the Orc/Dark Elf Force Burst/Storm/Blaster
    /// equivalents). Transient, never persisted (matches Java: an
    /// `AtomicInteger`, not a DB column). Java clears it after ten minutes of
    /// inactivity, which `charges_seq` implements — see there.
    pub charges: i32,
    /// Which charge-decay timer is the live one.
    ///
    /// Java restarts a cancellable `ResetChargesTask` on every gain and every
    /// partial spend, and cancels it outright when the pool hits 0. The port's
    /// scheduler cannot cancel, so each gain/spend bumps this counter and
    /// schedules a fresh task carrying the new value; a task whose value no
    /// longer matches is stale and does nothing. Bumping on the spend-to-zero
    /// case is what stands in for Java's `stopChargeTask`.
    ///
    /// Transient, like `charges` itself.
    pub charges_seq: u64,
    /// `PlayerStat._vitalityPoints` — always clamped to
    /// [`MIN_VITALITY_POINTS`]..=[`MAX_VITALITY_POINTS`]. Persisted in
    /// `characters.vitality_points`; consumed on monster kills and spent as an
    /// exp/sp multiplier (see `game_loop::vitality`).
    pub vitality_points: i32,
    /// `characters.pccafe_points` — PC-cafe loyalty points (`//pccafepoints`).
    pub pccafe_points: i32,
    /// Account-scoped NCoin balance (`account_gsdata` "PRIME_POINTS",
    /// `//primepoints`). Mirror of the account var, loaded at enter-world.
    pub prime_points: i32,
    pub fame: i32,
    /// Recommendations received / left to give (Java `Player.getRecomHave` /
    /// `getRecomLeft`). Persisted in the `character_reco_bonus` table — loaded
    /// in `from_char`, flushed by the memory-first autosave.
    pub rec_have: i32,
    pub rec_left: i32,
    /// Java `Player._recoTwoHoursGiven` — a transient, per-session flag (never
    /// persisted, always `false` at login) that makes the first `RecoGiveTask`
    /// firing hand out 10 recommendations instead of 1.
    pub reco_two_hours_given: bool,
    /// Guard for the self-rescheduling `RecoGiveTask` (Java's per-player
    /// `scheduleAtFixedRate` future): a fresh value is stamped at enter-world so
    /// a stale task left over from a previous session no-ops. See
    /// `World::next_reco_give_seq`.
    pub reco_give_seq: u64,
    /// The same guard for the retail-like `PcCafeReward` task
    /// (`game_loop::pc_cafe`). Re-stamped by every `run`, so an earlier
    /// schedule goes stale instead of stacking a second payout timer.
    pub pc_cafe_seq: u64,

    // Clan membership (G11 — creation/display slice). The `Clan` itself
    // lives in `World.clans`; these are the per-player fields the
    // UserInfo/CharInfo builders write. `clan_leader` is fixed up at
    // enter-world from the live table (and by `create_clan`).
    pub clan_id: i32,
    pub clan_privs: i32,
    pub clan_leader: bool,
    /// `ClanMember.calculatePledgeClass` result, denormalized here so the
    /// UserInfo/CharInfo builders (store-only, no `World.clans` access) can
    /// write it. Drives the on-head clan-rank crown; recomputed alongside
    /// `clan_leader` whenever clan membership or level changes.
    pub pledge_class: u8,
    /// `characters.clan_create_expiry_time` — the 10-day recreate cooldown.
    pub clan_create_expiry_time: i64,
    /// `characters.clan_join_expiry_time` — the 1-day rejoin penalty after
    /// leaving/being ousted from a clan (`Player.getClanJoinExpiryTime`).
    pub clan_join_expiry_time: i64,
    /// `characters.create_date` (`YYYY-MM-DD`) — Java `Player.getCreateDate()`,
    /// shown by `/mybirthday`.
    pub create_date: String,
    /// Java `Player._powerGrade` — the clan rank (1 leader … 9 academy);
    /// fixed up at enter-world (leader → 1, unset → 5) alongside `clan_privs`.
    pub power_grade: i32,
    /// Java `Player.getAllyId()` (via the clan) — denormalized here so the
    /// store-only UserInfo/CharInfo builders can write it; synced at
    /// enter-world and on every ally change.
    pub ally_id: i32,
    /// Java `Player._siegeState` — **1 attacker, 2 defender, 0 uninvolved**.
    /// Set for every online member of a registered clan when a siege starts
    /// (`Siege.updatePlayerSiegeStateFlags`) and cleared when it ends. Drives
    /// the `RelationChanged` siege bits (INSIEGE / ENEMY-vs-ALLY / ATTACKER).
    ///
    /// Note it is a *clan* property projected onto the member, and it is not
    /// the same test as "standing in the siege zone" — a registered attacker
    /// carries state 1 across the whole world while their siege runs.
    pub siege_state: u8,
    /// Java `Player._siegeSide` — the residence id of the siege the member is
    /// registered for, so two simultaneous sieges don't bleed into each other.
    pub siege_side: i32,
    /// Java `Player._pledgeType` — 0 main pledge, -1 academy, 100/200 royal
    /// guard, 1001/1002/2001/2002 knight order. Drives `pledge_class_of` and
    /// the sub-pledge member caps.
    pub pledge_type: i32,
    /// Java `Player._lvlJoinedAcademy` (`characters.lvl_joined_academy`) — the
    /// level the character was at when it joined a clan academy, and the only
    /// thing that makes it *an academy member* (`isAcademyMember()` is
    /// `> 0`). The graduation reputation reward scales off it, so it must be
    /// the joining level and not the current one.
    pub lvl_joined_academy: i32,
    /// Java `Player._apprentice` / `_sponsor` (`characters.apprentice` /
    /// `sponsor`) — the two ends of an academy mentorship, each holding the
    /// other's object id. A sponsor is a full member; an apprentice is the
    /// academy member they took on.
    pub apprentice: i32,
    pub sponsor: i32,
    /// Java `clan.getCrestId()`/`getAllyCrestId()`, denormalized like
    /// `ally_id` so the store-only UserInfo/CharInfo builders can write them;
    /// synced at enter-world and whenever a crest changes.
    pub clan_crest_id: i32,
    /// `Clan.getCrestLargeId()` — the **large** crest, shown in the clan
    /// window. Mirrored onto the player alongside `clan_crest_id` because the
    /// `UserInfo` builder has no access to `World.clans`.
    pub clan_crest_large_id: i32,
    pub ally_crest_id: i32,

    pub face: i32,
    pub hair_style: i32,
    pub hair_color: i32,

    // Computed combat stats live in the `CombatStats` component; swing/
    // stance timing in `AttackState` (stage 2 phase 4).

    // Inventory / SkillBook / Buffs / StatModifiers / Reuses / TargetRef /
    // ClientPos are components on the same entity (stage 2 phase 5); the
    // in-flight move / cast / attack intention are the presence-based
    // `Movement`/`Casting`/`Intent` components (phase 3).
    /// Monotonic cast-generation counter, bumped every `startCasting` — the
    /// in-flight cast itself is the presence-based `Casting` component
    /// (stage 2 phase 3); this counter must survive across casts for the
    /// scheduler's stale-task no-op contract.
    pub cast_seq: u64,

    // --- Combat state (G9) ---
    /// `Player._reviveRequested`-ish: die → "to village" → teleport →
    /// revive on `Appearing` (Java `setPendingRevive` → `onTeleported`).
    pub pending_revive: bool,
    /// XP lost to the most recent death — Java's `expBeforeDeath - getExp()`,
    /// recorded here rather than the pre-death total because that difference is
    /// the only thing a resurrection reads. Cleared on revive.
    pub lost_exp_on_death: i64,
    /// A resurrection proposal awaiting this player's `ConfirmDlg` answer —
    /// Java's `_reviveRequested`/`_revivePower`/`_revive*Percent`/`_revivePet`
    /// block. The dialog always goes to the **player**, even when what is
    /// being resurrected is their pet.
    pub revive_request: Option<ReviveRequest>,
    /// Java's `SummonRequestHolder` script, stashed on the player a Summon
    /// Friend prompt was sent to. Holds the summoner's id and the destination
    /// captured **when the prompt was sent** — Java stores a `Location`, so a
    /// summoner who walks away during the 30 s window still pulls the target
    /// to where they cast, not to where they now stand.
    pub summon_request: Option<SummonRequest>,
    /// The collar item object id a pet summon is about to consume — Java's
    /// `PetItemHolder`, which `SummonItems` attaches to the player as a script
    /// and `SummonPet.instant` pulls back out with `removeScript`.
    ///
    /// It exists because the effect never receives the item: the item-use path
    /// and the effect are separated by the whole cast pipeline, so the item
    /// identity has to be parked somewhere in between. Taken (not copied) by
    /// the effect, so a stale collar can never summon a second pet.
    pub pending_pet_collar: Option<i32>,
    /// The mercenary ticket awaiting its `ConfirmDlg` answer (Java's
    /// `MercTicket._items` map, keyed by player). Object id, not item id: the
    /// answer destroys that exact stack entry.
    pub pending_mercenary_ticket: Option<i32>,
    /// Java `Creature._isTeleporting`: position pushed server-side, waiting
    /// for the client's `Appearing`.
    pub teleporting: bool,
    /// Java `Player.isJailed()` (G31): whether a JAIL punishment currently
    /// applies to this character (by char id, account, or IP). Cached at
    /// login/apply and cleared on release; the JailZone keep-in reads it.
    /// Not persisted — re-derived from `PunishmentManager` on enter-world.
    pub jailed: bool,
    /// Java `Player._waitTypeSitting` — the character is seated. Sitting is a
    /// **two-step** state on both ends: `sitDown` flips this immediately and
    /// blocks actions for 2.5 s while the animation plays, while `standUp`
    /// broadcasts first and only clears the flag 2.5 s later. So "seated" and
    /// "can act" are not the same predicate, and the regen bonus follows this
    /// flag rather than the block.
    pub sitting: bool,
    /// Java `Player._isSellingBuffs` — this character's buff shop is open. It
    /// rides the `PACKAGE_SELL` private-store type (so other clients render the
    /// shop label) but has its own list and its own bypasses.
    pub selling_buffs: bool,
    /// Java `Player._sellingBuffs` — the shop's `(skill id, price)` lines.
    /// Transient: Java never persists them, so a relog empties the shop.
    pub sell_buff_list: Vec<(i32, i64)>,
    /// Java `Player._lastPetitionGmName`: the GM who last handled this player's
    /// petition, set when a consultation starts. The feedback packet
    /// (`RequestPetitionFeedback`) needs it to attribute the rating. Transient.
    pub last_petition_gm_name: Option<String>,
    /// Java `Player._snoopListener`: GM object ids currently eavesdropping on
    /// this player's chat (`//snoop`). Each of this player's outgoing chat lines
    /// is mirrored to them via a `Snoop` packet. Transient (offline listeners
    /// are skipped at send time).
    pub snoop_listeners: Vec<i32>,
    /// Java `Player._snoopedPlayer`: the players this (GM) character is snooping
    /// — kept so the relationship can be torn down. Transient.
    pub snooped: Vec<i32>,
    /// `AdminData._gmList`'s value half — whether this GM is hidden from the
    /// `/gmlist` a **non-GM** player runs. Set once at enter-world by
    /// `admin::flags::register_gm`; nothing in this Java build ever flips it
    /// afterwards (`showGm`/`hideGm` have no callers, and `//gmliston` only
    /// prints a message). **True for every GM on this dist**, because it is
    /// `!GMStartupAutoList || …` and `GMStartupAutoList = False`.
    pub gm_hidden: bool,
    /// `Player._questZoneId` (default -1): the quest zone the client last
    /// selected (`ExSendSelectedQuestZoneID`), read by quest teleports
    /// (`TeleportHolder`). Transient — not persisted.
    pub quest_zone_id: i32,

    /// `Creature._chargedShots` — a bitset of [`ShotType`] masks currently
    /// charged on the equipped weapon (consumed by the next attack/cast for a
    /// damage bonus). Transient.
    pub charged_shots: u8,
    /// `Player._activeSoulShots` — item ids the player toggled for automatic
    /// use (`RequestAutoSoulShot`); re-charged before each attack/cast. Not
    /// persisted in this slice (Java saves them to `character_variables`).
    pub auto_shots: Vec<i32>,

    /// `Player._mountType` ordinal (Java `MountType`): 0 none, 1 strider,
    /// 2 wyvern, 3 great wolf. Drives the mount byte in UserInfo/CharInfo and
    /// the `Ride` broadcast. Transient (admin `//ride*`), not persisted.
    pub mount_type: u8,
    /// `Player._mountNpcId` — the ridden creature's npc id (0 when unmounted).
    /// CharInfo/`Ride` send it as `+ 1_000_000`.
    pub mount_npc_id: i32,
    /// `Player._mountLevel` — the mount's level: the *pet's* level when
    /// mounting an owned strider, the *rider's* level for a wyvern
    /// (`mount(npcId, …)` → `setMount(npcId, getLevel())`). Selects the
    /// `speed_on_ride` row and drives the "-50% when 10+ levels above you"
    /// speed penalty in `recalculate_stats`.
    pub mount_level: i32,
    /// `Player._curFeed` — the ridden creature's food gauge, drained every 10 s
    /// by the mount feed task and refilled by using its food item while
    /// mounted (the `Feed` effect's `ride`/`wyvern` params). Hitting zero
    /// force-dismounts. Transient, like the rest of the mount state.
    pub mount_feed: i32,
    /// Java `Player._controlItemId`/`_mountObjectID` — the collar object id of
    /// the pet being ridden, so a dismount can write the drained feed gauge
    /// back onto its `pets` row (`storePetFood`). 0 for wyverns and admin
    /// mounts, which have no collar.
    pub mount_collar_object_id: i32,
    /// A `BroadcastCharInfo` task is already scheduled (Java
    /// `_broadcastCharInfoTask != null`) — further `broadcastUserInfo` calls
    /// inside the 50 ms window coalesce into it.
    pub char_info_pending: bool,

    /// `Player.getTradeRefusal()` — `//tradeoff`: refuse incoming trade
    /// requests. Transient.
    pub trade_refusal: bool,
    /// `PlayerCondOverride` bitmask (`//exceptions`). Bit N = ordinal N of
    /// Java's enum; `SEE_ALL_PLAYERS` (13) is consumed by the visibility
    /// describe path. Transient (Java re-grants per access level on login).
    pub cond_overrides: u64,
    /// `Player._transformation` id (0 = not transformed). Drives the transform
    /// speed/collision override in `recalculate_stats` and the untransform
    /// logic (`AdminRide` checks the id). Transient (admin `//transform`).
    pub transform_id: i32,
    /// The transform's display id (Java `getTransformationDisplayId()`) — the
    /// model shown in UserInfo/CharInfo. Equals `transform_id` on this dist (no
    /// template overrides `displayId`); 0 when not transformed.
    pub transform_display_id: i32,

    /// `Player._privateStoreType` (Java `PrivateStoreType`): 0 none, 1 sell,
    /// 3 buy, … The CharInfo/UserInfo store byte; a non-zero value makes the
    /// client sit the character with the store title above it. The sell list
    /// itself lives in the `PrivateStore` component.
    pub store_type: u8,

    /// Java `Player._spawnProtectEndTime`, as an absolute world tick (0 = not
    /// protected). While it is in the future the character is **ignored by
    /// aggressive monsters** — not invulnerable; see
    /// [`crate::config::character::CharacterConfig::player_spawn_protection`].
    /// Cleared by the first deliberate action, which is Java's
    /// `Player.onActionRequest`.
    pub spawn_protect_end_tick: u64,
}

/// Port of `enums/ShotType`, narrowed to the kinds this slice charges. The
/// mask is `1 << ordinal`, matching Java's `ShotType._mask` so a single `u8`
/// on [`Player::charged_shots`] mirrors `Creature._chargedShots`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotType {
    Soulshots = 0,
    Spiritshots = 1,
    BlessedSpiritshots = 3,
    /// `FISH_SOULSHOTS` (ordinal 4) — doubles the fishing win chance (G32).
    FishSoulshots = 4,
}

impl ShotType {
    /// `ShotType.getMask()` (`1 << ordinal`).
    pub fn mask(self) -> u8 {
        1 << (self as u8)
    }
}

impl Player {
    /// Java `PlayerAppearance.getVisibleClanId()` and its crest/ally siblings:
    /// while a cursed weapon is held the wielder shows **no pledge at all** —
    /// clan id, both crests, ally id and ally crest all report 0. The demon is
    /// deliberately untraceable to their clan for as long as they carry it.
    pub fn visible_clan_id(&self) -> i32 {
        if self.cursed_weapon_equipped_id != 0 {
            0
        } else {
            self.clan_id
        }
    }

    pub fn visible_clan_crest_id(&self) -> i32 {
        if self.cursed_weapon_equipped_id != 0 {
            0
        } else {
            self.clan_crest_id
        }
    }

    pub fn visible_clan_crest_large_id(&self) -> i32 {
        if self.cursed_weapon_equipped_id != 0 {
            0
        } else {
            self.clan_crest_large_id
        }
    }

    pub fn visible_ally_id(&self) -> i32 {
        if self.cursed_weapon_equipped_id != 0 {
            0
        } else {
            self.ally_id
        }
    }

    pub fn visible_ally_crest_id(&self) -> i32 {
        if self.cursed_weapon_equipped_id != 0 {
            0
        } else {
            self.ally_crest_id
        }
    }

    /// Java `Player.isGM()` — `getAccessLevel().isGm()`.
    pub fn is_gm(&self, data: &GameData) -> bool {
        data.admin.is_gm(self.access_level)
    }

    /// The player's resolved [`AccessLevel`] (Java `Player.getAccessLevel()`).
    pub fn access_level_def<'a>(&self, data: &'a GameData) -> &'a AccessLevel {
        data.admin.access_level(self.access_level)
    }

    /// `Creature.isChargedShot(type)`.
    pub fn is_charged_shot(&self, shot: ShotType) -> bool {
        (self.charged_shots & shot.mask()) != 0
    }

    /// `Creature.chargeShot(type)` — returns whether the flag flipped on.
    pub fn charge_shot(&mut self, shot: ShotType) -> bool {
        let was = self.is_charged_shot(shot);
        self.charged_shots |= shot.mask();
        !was
    }

    /// `Creature.unchargeShot(type)` — returns whether the flag flipped off.
    pub fn uncharge_shot(&mut self, shot: ShotType) -> bool {
        let was = self.is_charged_shot(shot);
        self.charged_shots &= !shot.mask();
        was
    }

    /// `Player.isMounted()`.
    pub fn is_mounted(&self) -> bool {
        self.mount_type != 0
    }

    /// Java `canOverrideCond(PlayerCondOverride)` — bit = the enum ordinal.
    pub fn can_override_cond(&self, ordinal: u8) -> bool {
        self.cond_overrides & (1u64 << ordinal) != 0
    }

    /// `Creature.isFlying()` for a player. Java flips an explicit `_isFlying`
    /// in `setMount` — true exactly for `MountType.WYVERN` — so the port
    /// derives it from the mount type and the flag can't drift. (Gracia
    /// flying *transformations* would also set it; none exist on Interlude.)
    pub fn is_flying(&self) -> bool {
        self.mount_type == 2
    }
}

/// The player's full component set, together *outside* the ECS world — the
/// boundary DTO of PLAN_ECS_STAGE2 §3. Built by `from_char`, held by the
/// `Entering` session state until `EnterWorld` spawns it (`spawn_into`);
/// persistence gathers its own view (`PlayerSnapshot`) from components.
#[derive(Debug, Clone)]
pub struct PlayerData {
    pub player: Player,
    pub position: Position,
    pub region: RegionCell,
    pub vitals: Vitals,
    pub player_vitals: PlayerVitals,
    pub base_stats: BaseStats,
    pub speeds: Speeds,
    pub collision: Collision,
    pub combat: CombatStats,
    /// Active buffs/debuffs. At character load this holds only the restored
    /// armor-conditioned passives (`conditioned_passive_buffs`); timed buffs
    /// aren't persisted yet.
    pub buffs: Buffs,
    /// Modifier maps folded from `buffs` — kept in sync so a spawned player's
    /// stats already include its passives.
    pub stat_modifiers: StatModifiers,
    pub inventory: Inventory,
    pub warehouse: inventory::Warehouse,
    pub freight: inventory::Freight,
    pub skills: SkillBook,
    /// Enchant sub-levels per skill id (`character_skills.skill_sub_level`).
    pub skill_enchants: components::SkillEnchants,
    /// Worn henna dyes (`character_hennas`); their stat bonus is already folded
    /// into `base_stats`.
    pub henna: components::HennaSlots,
    /// Registered crafting recipes (`character_recipebook`), split by book.
    pub recipe_book: components::RecipeBook,
    /// `character_variables` key/value store (Java `PlayerVariables`).
    pub variables: components::PlayerVariables,
    /// Saved pet rows, keyed by collar object id (Java's `pets` table).
    pub pets: components::PlayerPets,
    /// The servitor that was out at logout (`character_summons`).
    pub summons: components::PlayerSummons,
    /// Items held by the player's pet (Java `PetInventory`, `loc="PET"`).
    pub pet_inventory: inventory::PetInventory,
    pub shortcuts: Shortcuts,
    pub macros: Macros,
    pub friends: components::Friends,
    pub quests: components::Quests,
    /// Live skill-reuse cooldowns. Empty out of `from_char`; the real select
    /// path fills it from the DB via [`PlayerData::restore_reuses`].
    pub reuses: Reuses,
    /// Persisted buffs waiting to be re-applied. Unlike every other field here
    /// this is *not* a component: a buff can only be applied to a character
    /// that is already in the world (it drives stats, the expiry scheduler and
    /// client packets), so enter-world takes these rows just before
    /// [`PlayerData::spawn_into`] and hands them to
    /// `skills::effects::restore_persisted_buffs` once the entity exists.
    pub pending_buffs: Vec<crate::db::SkillBuffRow>,
    /// `character_skills` rows this character was not entitled to — Java's
    /// `restoreSkills` skill check (`SkillCheckEnable`).
    ///
    /// Not a component: the finding belongs to the *load*, not to the
    /// character. `from_char` runs against `&GameData` alone and has no world
    /// to broadcast into and no audit sink, so it records what it found and the
    /// login path ([`game_loop::lobby`](crate::game_loop::lobby)) reports it —
    /// the same split `pending_buffs` above uses for the same reason.
    ///
    /// Whether the skills were also *removed* from [`Self::skills`] depends on
    /// `SkillCheckRemove`; this list is populated either way, because the audit
    /// half of the feature is the half that works with removal off.
    pub illegal_skills: Vec<(i32, i32)>,
}

/// Java `Player.restoreSkills`' skill check, factored out of
/// [`Player::from_char`] so it can be read (and tested) as the one thing it is.
///
/// Java's guard is
/// `SKILL_CHECK_ENABLE && (!canOverrideCond(SKILL_CONDITIONS) || SKILL_CHECK_GM)
/// && !isSkillAllowed(...)`, evaluated per restored row. The middle clause is
/// the one worth reading twice: with `SkillCheckGM = False` — this dist — a
/// character holding the `SKILL_CONDITIONS` override is **skipped entirely**,
/// so the key named "check GMs" turns checking GMs off.
///
/// Returns the book to keep and everything that failed. Failures are reported
/// whether or not `SkillCheckRemove` takes them out: an operator running the
/// check as a pure audit still wants the list.
fn check_restored_skills(
    data: &GameData,
    c: &CharData,
    cond_overrides: u64,
    skills: SkillBook,
) -> (SkillBook, Vec<(i32, i32)>) {
    if !data.skill_check.enable {
        return (skills, Vec::new());
    }
    // `canOverrideCond(PlayerCondOverride.SKILL_CONDITIONS)`.
    let overrides_skill_conditions =
        cond_overrides & (1u64 << crate::game_loop::admin::SKILL_CONDITIONS_ORDINAL) != 0;
    if overrides_skill_conditions && !data.skill_check.gm {
        return (skills, Vec::new());
    }
    let is_gm = data.admin.is_gm(
        data.admin
            .effective_access_level(c.access_level, data.default_access_level),
    );
    let race = crate::enums::Race::from_ordinal(c.race);
    let mut illegal: Vec<(i32, i32)> = Vec::new();
    let mut kept = SkillBook::default();
    for (&id, &level) in &skills.0 {
        // Java's first arm, `skill.isExcludedFromCheck()`, reads the *skill*
        // rather than any tree — the datapack's own opt-out for skills learned
        // by routes that are not class trees (the subclass certifications).
        let excluded = data
            .skill_data
            .get(id, level)
            .is_some_and(|s| s.excluded_from_check);
        let max_level = data.skill_data.max_level(id);
        if excluded
            || data
                .skill_trees
                .is_skill_allowed(c.class_id, race, is_gm, id, level, max_level)
        {
            kept.0.insert(id, level);
            continue;
        }
        illegal.push((id, level));
    }
    illegal.sort_unstable();
    // `Config.SKILL_CHECK_REMOVE` — off, the check is an audit and the book is
    // returned untouched.
    if data.skill_check.remove {
        (kept, illegal)
    } else {
        (skills, illegal)
    }
}

impl PlayerData {
    /// Java `restoreEffects` (skill-reuse half): rebuild the live cooldown map
    /// from the `character_skills_save` rows the DB loaded. Each row's absolute
    /// `systime_ms` becomes an `until_tick` off the current game tick, using the
    /// real remaining time (`systime - now`), so a cooldown decays across the
    /// offline gap. Rows already expired at restore are skipped.
    pub fn restore_reuses(&mut self, c: &CharData, now_tick: u64, now_wallclock_ms: i64) {
        for r in &c.skill_reuses {
            let remaining_ms = r.systime_ms - now_wallclock_ms;
            if remaining_ms <= 0 {
                continue;
            }
            self.reuses.0.insert(
                r.reuse_key,
                SkillReuse {
                    skill_level: r.skill_level,
                    until_tick: now_tick + crate::scheduler::ms_to_ticks(remaining_ms),
                    total_ms: r.reuse_delay,
                },
            );
        }
    }

    /// Java `restoreEffects` (buff half), staging step: carry the loaded buff
    /// rows on the bundle so enter-world can re-apply them after the character
    /// spawns. No time arithmetic happens here — unlike a cooldown, a buff's
    /// stored `remaining_time` is relative and its countdown does not advance
    /// while the character is offline, so the value is used verbatim.
    pub fn restore_buffs(&mut self, c: &CharData) {
        self.pending_buffs = c.skill_buffs.clone();
    }

    /// Spawn into the world registry (Java `World.addObject` at EnterWorld).
    ///
    /// Takes the whole `World`, not just the store, so the new player lands in
    /// `World.player_regions` in the same step it lands in the ECS — an entity
    /// spawned without its index entry is invisible to every broadcast.
    pub fn spawn_into(self, world: &mut crate::world::World) {
        let object_id = self.player.object_id;
        let region = self.region.0;
        world.objects.spawn(
            self.player.object_id,
            (
                self.player,
                (
                    self.position,
                    self.region,
                    self.vitals,
                    self.player_vitals,
                    self.base_stats,
                    self.speeds,
                    self.collision,
                    self.combat,
                ),
                (
                    self.inventory,
                    self.skills,
                    self.shortcuts,
                    self.macros,
                    self.friends,
                    self.quests,
                    AttackState::default(),
                    TargetRef::default(),
                    ClientPos::default(),
                    self.buffs,
                    self.stat_modifiers,
                    self.reuses,
                    components::ZoneFlags::default(),
                    components::ExpertisePenalty::default(),
                    components::PvpState::default(),
                ),
                (
                    self.warehouse,
                    self.freight,
                    components::ClanSkills::default(),
                    components::OptionSkills::default(),
                    components::OptionTriggers::default(),
                    self.skill_enchants,
                    self.henna,
                    self.recipe_book,
                    self.variables,
                    self.pets,
                    self.pet_inventory,
                    self.summons,
                ),
            ),
        );
        world.index_player(object_id, region);
    }
}

/// Borrowed view of a player's full component set — the read-side
/// counterpart of `PlayerData`, so packet builders take one argument
/// instead of seven. Build with `PlayerView::of` (in-world player) or
/// `PlayerData::view` (pre-spawn, enter-world path).
pub struct PlayerView<'a> {
    pub p: &'a Player,
    pub pos: &'a Position,
    pub vitals: &'a Vitals,
    pub pvitals: &'a PlayerVitals,
    pub base: &'a BaseStats,
    pub speeds: &'a Speeds,
    pub collision: &'a Collision,
    pub combat: &'a CombatStats,
    pub inventory: &'a Inventory,
    /// Runtime PvP flag (0/1/2) for the SOCIAL block; 0 pre-spawn.
    pub pvp_flag: u8,
    /// `Player.isInMatchingRoom()` for the CLAN block (G30).
    pub in_matching_room: bool,
    /// Passive-skill stat modifiers, for storage-capacity finalizers
    /// (`Stat::InventoryNormal`/`StoragePrivate`/`TradeSell`/`TradeBuy`) that
    /// packet builders need but that don't have their own finalized field.
    pub mods: &'a StatModifiers,
    /// The learned-skill map, for the STATUS block's craft byte
    /// (`hasDwarvenCraft() || getSkillLevel(248) > 0`).
    pub skills: &'a SkillBook,
    /// `isCursedWeaponEquipped() ? CursedWeaponsManager.getLevel(id) : 0` — the
    /// INVENTORY_LIMIT block's trailing byte. Resolved by the caller because
    /// the level lives on `World.cursed_weapons`, not on the player.
    pub cursed_weapon_level: u8,
    /// `insideZone(ZoneId.WATER)` — the first arm of the MOVEMENTS byte both
    /// `UserInfo` and `CharInfo` write (`insideZone(WATER) ? 1 :
    /// isFlyingMounted() ? 2 : 0`). Resolved by the caller for the same reason
    /// as [`Self::cursed_weapon_level`]: it is a *zone* question, and zones
    /// live on `World`, not on the entity.
    ///
    /// `false` from the entity-store-only [`Self::of`], which is why every
    /// packet builder must go through [`Self::of_world`].
    pub in_water: bool,
}

impl<'a> PlayerView<'a> {
    pub fn of(objects: &'a crate::store::EntityStore, object_id: i32) -> Option<Self> {
        Some(Self {
            p: objects.get_component::<Player>(&object_id)?,
            pos: objects.get_component::<Position>(&object_id)?,
            vitals: objects.get_component::<Vitals>(&object_id)?,
            pvitals: objects.get_component::<PlayerVitals>(&object_id)?,
            base: objects.get_component::<BaseStats>(&object_id)?,
            speeds: objects.get_component::<Speeds>(&object_id)?,
            collision: objects.get_component::<Collision>(&object_id)?,
            combat: objects.get_component::<CombatStats>(&object_id)?,
            inventory: objects.get_component::<Inventory>(&object_id)?,
            pvp_flag: objects
                .get_component::<components::PvpState>(&object_id)
                .map_or(0, |s| s.flag),
            in_matching_room: objects.has_component::<components::InMatchingRoom>(&object_id),
            // Zones live on `World`; `of_world` fills this in.
            in_water: false,
            mods: objects.get_component::<StatModifiers>(&object_id)?,
            skills: objects.get_component::<SkillBook>(&object_id)?,
            cursed_weapon_level: 0,
        })
    }

    /// [`Self::of`] plus the fields that need the world rather than the entity
    /// store: the cursed-weapon stage (`World.cursed_weapons`) and the
    /// water-zone flag (`World.data.zone_data`).
    ///
    /// Every `UserInfo` builder goes through this; `of` alone would silently
    /// report an unwielded weapon.
    pub fn of_world(world: &'a crate::world::World, object_id: i32) -> Option<Self> {
        let mut v = Self::of(&world.objects, object_id)?;
        let equipped = v.p.cursed_weapon_equipped_id;
        if equipped != 0 {
            v.cursed_weapon_level = world
                .cursed_weapons
                .iter()
                .find(|c| c.item_id == equipped)
                .map(|c| c.level() as u8)
                .unwrap_or(0);
        }
        v.in_water = crate::game_loop::position::is_in_water(world, object_id);
        Some(v)
    }
}

impl PlayerData {
    pub fn view(&self) -> PlayerView<'_> {
        PlayerView {
            p: &self.player,
            pos: &self.position,
            vitals: &self.vitals,
            pvitals: &self.player_vitals,
            base: &self.base_stats,
            speeds: &self.speeds,
            collision: &self.collision,
            combat: &self.combat,
            inventory: &self.inventory,
            pvp_flag: 0,
            in_matching_room: false,
            // No world yet at bundle-build time; the enter-world UserInfo goes
            // through `of_world`.
            in_water: false,
            mods: &self.stat_modifiers,
            skills: &self.skills,
            cursed_weapon_level: 0,
        }
    }
}

/// Equipped-gear contributions to the combat finalizers, summarized from the
/// paperdoll once per recompute — Java re-reads item `getStats(...)` inside
/// each finalizer, but the numbers are the same. Two families, matching the
/// Java stat finalizers (see [`crate::data::item_data::ItemStats`]):
///   * **weapon-replace** bases (`None` ⇒ fall back to the class template
///     base) — `calcWeaponBaseValue`, the equipped weapon only;
///   * **sum-add** contributions (0.0 when nothing equipped adds them) —
///     summed across every equipped piece.
struct EquippedBonuses {
    weapon_p_atk: Option<f64>,
    weapon_m_atk: Option<f64>,
    weapon_p_atk_spd: Option<f64>,
    weapon_crit: Option<f64>,
    weapon_m_crit: Option<f64>,
    weapon_atk_range: Option<i32>,
    weapon_random_dmg: Option<i32>,
    p_def: f64,
    m_def: f64,
    accuracy: f64,
    magic_accuracy: f64,
    evasion: f64,
    magic_evasion: f64,
    /// Sum of `getBaseDefBySlot` over the *occupied* pDef/mDef slots — the naked
    /// slot defenses the P/MDefenseFinalizer subtracts so worn gear replaces
    /// (not stacks on) the class base. See the finalizer loops in Java's
    /// `PDefenseFinalizer`/`MDefenseFinalizer`.
    p_def_slot_sub: f64,
    m_def_slot_sub: f64,
    /// `calcEnchantedItemBonus` per stat — the extra attack/defence an
    /// **enchanted** piece contributes on top of its own declared value. Java
    /// folds each into its finalizer *before* the stat bonus and level mod, so
    /// they are carried separately rather than merged into `p_def`/`weapon_p_atk`.
    enchant_p_atk: f64,
    enchant_m_atk: f64,
    enchant_p_def: f64,
    enchant_m_def: f64,
    /// `ShotsBonusFinalizer` — `1 + enchantLevel·0.003` off the equipped weapon.
    shots_bonus: f64,
}

/// Hand-written rather than derived: `shots_bonus`'s identity is **1**, and a
/// derived `0.0` would silently delete every soulshot's damage bonus.
impl Default for EquippedBonuses {
    fn default() -> Self {
        Self {
            weapon_p_atk: None,
            weapon_m_atk: None,
            weapon_p_atk_spd: None,
            weapon_crit: None,
            weapon_m_crit: None,
            weapon_atk_range: None,
            weapon_random_dmg: None,
            p_def: 0.0,
            m_def: 0.0,
            accuracy: 0.0,
            magic_accuracy: 0.0,
            evasion: 0.0,
            magic_evasion: 0.0,
            p_def_slot_sub: 0.0,
            m_def_slot_sub: 0.0,
            enchant_p_atk: 0.0,
            enchant_m_atk: 0.0,
            enchant_p_def: 0.0,
            enchant_m_def: 0.0,
            shots_bonus: 1.0,
        }
    }
}

impl EquippedBonuses {
    fn from_inventory(inventory: &Inventory, data: &GameData, t: &PlayerTemplate) -> Self {
        use crate::model::inventory::PaperdollSlot;
        let mut eq = EquippedBonuses::default();

        // P/MDefenseFinalizer's slot loops: for every occupied armor slot,
        // subtract the class template's naked defense for that slot. The pDef
        // legs slot also counts when a full-armor chest covers the legs (its
        // `isPaperdollSlotEmpty(LEGS) || (CHEST is FULL_ARMOR)` guard).
        let occupied = |slot: PaperdollSlot| inventory.paperdoll_item(slot).is_some();
        let chest_is_full_armor = inventory
            .paperdoll_item(PaperdollSlot::Chest)
            .and_then(|it| data.item_data.get(it.item_id))
            .map(|tpl| tpl.body_part == crate::data::item_data::SLOT_FULL_ARMOR)
            .unwrap_or(false);
        for slot in [
            PaperdollSlot::Chest,
            PaperdollSlot::Head,
            PaperdollSlot::Feet,
            PaperdollSlot::Gloves,
            PaperdollSlot::Under,
            PaperdollSlot::Cloak,
            PaperdollSlot::Hair,
        ] {
            if occupied(slot) {
                eq.p_def_slot_sub += t.base_def_by_slot(slot as usize) as f64;
            }
        }
        if occupied(PaperdollSlot::Legs) || chest_is_full_armor {
            eq.p_def_slot_sub += t.base_def_by_slot(PaperdollSlot::Legs as usize) as f64;
        }
        for slot in [
            PaperdollSlot::LFinger,
            PaperdollSlot::RFinger,
            PaperdollSlot::LEar,
            PaperdollSlot::REar,
            PaperdollSlot::Neck,
        ] {
            if occupied(slot) {
                eq.m_def_slot_sub += t.base_def_by_slot(slot as usize) as f64;
            }
        }

        // `ShotsBonusFinalizer`: `1 + enchantLevel·0.003`, read off the active
        // weapon instance. Java's `getActiveWeaponInstance()` is the right hand.
        if let Some(weapon) = inventory.paperdoll_item(PaperdollSlot::RHand) {
            eq.shots_bonus = crate::model::enchant_bonus::shots_bonus(weapon.enchant_level);
        }

        // `calcEnchantedItemBonus`, run once over the paperdoll instead of once
        // per finalizer: Java calls it from `PAttackFinalizer`,
        // `MAttackFinalizer`, `PDefenseFinalizer` and `MDefenseFinalizer`, each
        // asking about its own stat, and the per-item gate differs by stat only
        // through `enchant_bonus_applies`.
        for item in inventory
            .equipped_items()
            .into_iter()
            .filter(|i| i.enchant_level > 0)
        {
            let Some(tpl) = data.item_data.get(item.item_id) else {
                continue;
            };
            let declares = |stat: Stat| {
                data.item_data
                    .item_stats(item.item_id)
                    .is_some_and(|st| st.bonuses.iter().any(|&(s, v)| s == stat && v > 0.0))
            };
            let body_part = tpl.body_part;
            use crate::model::enchant_bonus::{
                enchant_bonus_applies, enchant_def_bonus, enchant_m_atk_bonus, enchant_p_atk_bonus,
            };
            // `stat == PHYSICAL_ATTACK && equippedItem.isWeapon()` — the extra
            // weapon test Java applies only on this arm.
            if tpl.kind == crate::data::item_data::ItemKind::Weapon
                && enchant_bonus_applies(body_part, declares(Stat::PhysicalAttack), false)
            {
                eq.enchant_p_atk += enchant_p_atk_bonus(
                    tpl.crystal_type,
                    body_part,
                    data.item_data.weapon_type(item.item_id),
                    item.enchant_level,
                );
            }
            if enchant_bonus_applies(body_part, declares(Stat::MagicalAttack), false) {
                eq.enchant_m_atk += enchant_m_atk_bonus(tpl.crystal_type, item.enchant_level);
            }
            if enchant_bonus_applies(body_part, declares(Stat::PhysicalDefence), true) {
                eq.enchant_p_def += enchant_def_bonus(tpl.crystal_type, item.enchant_level);
            }
            if enchant_bonus_applies(body_part, declares(Stat::MagicalDefence), true) {
                eq.enchant_m_def += enchant_def_bonus(tpl.crystal_type, item.enchant_level);
            }
        }

        // Weapon-replace stats come from the right-hand slot only (Java
        // `calcWeaponBaseValue`); a two-handed weapon also lives in RHand.
        if let Some(weapon) = inventory.paperdoll_item(PaperdollSlot::RHand)
            && let Some(stats) = data.item_data.item_stats(weapon.item_id)
        {
            for &(stat, val) in &stats.bonuses {
                match stat {
                    Stat::PhysicalAttack => eq.weapon_p_atk = Some(val),
                    Stat::MagicalAttack => eq.weapon_m_atk = Some(val),
                    Stat::PhysicalAttackSpeed => eq.weapon_p_atk_spd = Some(val),
                    Stat::CriticalRate => eq.weapon_crit = Some(val),
                    Stat::MagicCriticalRate => eq.weapon_m_crit = Some(val),
                    _ => {}
                }
            }
            eq.weapon_atk_range = stats.atk_range;
            eq.weapon_random_dmg = stats.random_damage;
        }

        // Sum-add stats are summed across every equipped piece (Java's
        // finalizer paperdoll loop / `calcWeaponPlusBaseValue`). `accCombat`
        // lives on weapons too, so this deliberately includes the weapon.
        for item in inventory.equipped_items() {
            let Some(stats) = data.item_data.item_stats(item.item_id) else {
                continue;
            };
            for &(stat, val) in &stats.bonuses {
                match stat {
                    Stat::PhysicalDefence => eq.p_def += val,
                    Stat::MagicalDefence => eq.m_def += val,
                    Stat::AccuracyCombat => eq.accuracy += val,
                    Stat::AccuracyMagic => eq.magic_accuracy += val,
                    Stat::EvasionRate => eq.evasion += val,
                    Stat::MagicEvasionRate => eq.magic_evasion += val,
                    // maxHp/maxMp item bonuses are folded in by
                    // `calc_max_hp`/`calc_max_mp` themselves
                    // (`equipped_stat_sum`) — adding them here would count
                    // them twice.
                    _ => {}
                }
            }
        }
        eq
    }
}

impl Player {
    /// Build a `Player` (+ its extracted components, as a `PlayerData`)
    /// from a stored character row + its class template.
    /// Max HP/MP/CP are recomputed (not read from the DB) so they display
    /// correctly; current HP/MP/CP come from the row, clamped to the max.
    pub fn from_char(data: &GameData, c: &CharData) -> PlayerData {
        // Java `Player.setAccessLevel`, called from `restore`: `DefaultAccessLevel`
        // promotes a level-0 character. 0 on this dist, so it is the identity —
        // but every access-level read below has to see the promoted value, not
        // the stored one, or an operator who sets the key gets GMs whose
        // condition overrides and skill-check exemption do not match their tier.
        let access_level = data
            .admin
            .effective_access_level(c.access_level, data.default_access_level);
        let cond_overrides = if data.admin.is_gm(access_level) {
            crate::game_loop::admin::all_exceptions_mask()
        } else {
            0
        };
        // The active class's template (base classes only in G4).
        let t = data
            .player_templates
            .get_or_base(c.class_id, c.base_class_id)
            .cloned()
            .unwrap_or_default();

        // Split stored items by location: warehouse / freight rows go to their
        // own containers, everything else (inventory + paperdoll) to inventory.
        let (wh_rows, rest): (Vec<_>, Vec<_>) =
            c.items.iter().cloned().partition(|r| r.loc == "WAREHOUSE");
        let (freight_rows, rest): (Vec<_>, Vec<_>) =
            rest.into_iter().partition(|r| r.loc == "FREIGHT");
        // Pet-held items (Java `ItemLocation.PET`) are stored against the
        // *player's* owner id, so they arrive in the same batch.
        let (pet_rows, inv_rows): (Vec<_>, Vec<_>) = rest
            .into_iter()
            .partition(|r| r.loc == "PET" || r.loc == "PET_EQUIP");
        let warehouse = inventory::Warehouse::from_rows(&wh_rows);
        let freight = inventory::Freight::from_rows(&freight_rows);
        let pet_inventory = inventory::PetInventory::from_rows(&pet_rows);

        // Built early so equipped gear feeds every finalizer below — max HP/MP
        // (item +MP jewelry) as well as the combat recompute further down.
        let inventory = Inventory::from_rows(&inv_rows);
        // No buffs are active at load; the enter-world clan-skill / passive
        // pass recomputes these through `recompute_max_vitals` once buffs land.
        let no_mods = StatModifiers::default();
        let max_hp = calc_max_hp(data, &t, c.level, Some(&inventory), &no_mods);
        let max_mp = calc_max_mp(data, &t, c.level, Some(&inventory), &no_mods);
        let max_cp = calc_max_cp(data, &t, c.level, &no_mods);

        // Restored henna dyes (Java `restoreHenna`): their base-stat bonuses are
        // folded straight into `BaseStats` — henna is a permanent base modifier,
        // exactly like the template, so every downstream reader (finalizers,
        // UserInfo STR/…) picks it up with no special-casing.
        let mut henna_slots = [None; 3];
        for &(slot, dye_id) in &c.hennas {
            if (1..=3).contains(&slot) {
                henna_slots[(slot - 1) as usize] = Some(dye_id);
            }
        }
        let henna = components::HennaSlots(henna_slots);
        let hs = data.hennas.stat_sums(&henna_slots);
        // A complete worn armor set adds flat base stats exactly as a dye does
        // (Java `BaseStatFinalizer`). Folded in here so the enter-world
        // `UserInfo` already carries them; `compose_base_stats` is the same
        // sum for every later recompute.
        let sets = crate::game_loop::armor_sets::set_stat_sums_for(&data.armor_sets, &inventory);
        let base_stats = BaseStats {
            str_: t.base_str + hs.str_ + sets.str_ as i32,
            dex: t.base_dex + hs.dex + sets.dex as i32,
            con: t.base_con + hs.con + sets.con as i32,
            int_: t.base_int + hs.int_ + sets.int_ as i32,
            wit: t.base_wit + hs.wit + sets.wit as i32,
            men: t.base_men + hs.men + sets.men as i32,
        };
        let mut vitals = Vitals {
            max_hp: max_hp as i32,
            cur_hp: c.cur_hp.min(max_hp),
            max_mp: max_mp as i32,
            cur_mp: c.cur_mp.min(max_mp),
            dead: c.cur_hp < 0.5,
        };
        // Java `Player.restore`: `setCurrentCp(currentCp)` replays the stored
        // `curCp`, clamped to the freshly computed max — the same treatment
        // `curHp`/`curMp` get just above.
        let mut player_vitals = PlayerVitals {
            max_cp: max_cp as i32,
            cur_cp: c.cur_cp.min(max_cp),
        };
        let mut speeds = Speeds {
            run_spd: t.base_run_spd as f64,
            walk_spd: t.base_walk_spd as f64,
            swim_run_spd: t.base_swim_run_spd as f64,
            swim_walk_spd: t.base_swim_walk_spd as f64,
            move_multiplier: 1.0,
            base_run_spd: t.base_run_spd as f64,
            base_walk_spd: t.base_walk_spd as f64,
            base_swim_run_spd: t.base_swim_run_spd as f64,
            base_swim_walk_spd: t.base_swim_walk_spd as f64,
            running: true,
            swimming: false,
            swamp_multiplier: 1.0,
        };
        // Java `PlayerTemplate.getCollisionRadius()` picks the box by
        // `appearance.isFemale()`; the two differ for every class on this dist.
        let (radius, height) = t.collision(c.sex != 0);
        let collision = Collision { radius, height };
        // Java `setAccessLevel` folds the tier's name/title color into the
        // appearance; a level-0 player keeps the client defaults (see
        // `Player::name_color`).
        let access = data.admin.access_level(c.access_level);
        let (name_color, title_color) = if c.access_level != 0 {
            (access.name_color, access.title_color)
        } else {
            (DEFAULT_NAME_COLOR, DEFAULT_TITLE_COLOR)
        };
        // Java `CharInfo`/`UserInfo`: hero glow = `isHero() || (isGM() &&
        // GM_HERO_AURA)`. `isHero()` is set by the olympiad's crowning (G25)
        // and by `//sethero`; either recomputes this.
        let hero_aura = access.is_gm && data.gm.hero_aura;
        let p = Player {
            object_id: c.object_id,
            name: c.name.clone(),
            account: c.account_name.clone(),
            title: String::new(),
            access_level,
            name_color,
            title_color,
            hero_aura,
            is_noble: c.noble,
            class_index: c
                .subclasses
                .iter()
                .find(|s| s.class_id == c.class_id)
                .map(|s| s.class_index)
                .unwrap_or(0),
            subclasses: c.subclasses.clone(),
            skills_by_index: c.skills_by_index.clone(),
            team: 0,
            on_event: false,
            registered_on_event: false,
            hennas_by_index: c.hennas_by_index.clone(),
            shortcuts_by_index: c.shortcuts_by_index.clone(),
            base_level: c.level,
            base_exp: c.exp,
            base_sp: c.sp,
            is_hero: false,
            true_hero: false,
            tele_mode: crate::enums::AdminTeleportType::Normal,
            blink_active: false,
            falling_until_tick: 0,
            level: c.level,
            class_id: c.class_id,
            base_class_id: c.base_class_id,
            race: c.race,
            is_female: c.sex != 0,
            exp: c.exp,
            sp: c.sp,
            reputation: c.reputation,
            pk_kills: c.pk_kills,
            raidboss_points: c.raidboss_points,
            pvp_kills: c.pvp_kills,
            // A fresh session starts unowned; `cursed_weapon::on_enter_world`
            // (G28) restores it for a player who logged out still cursed.
            cursed_weapon_equipped_id: 0,
            charges: 0,
            charges_seq: 0,
            vitality_points: c.vitality_points,
            pccafe_points: c.pccafe_points,
            prime_points: c.prime_points,
            fame: 0,
            // `character_reco_bonus` row (Java `Player.loadRecommendations`).
            // A new character's row is seeded with rec_left=20 at creation
            // (`Player.create` → `setRecomLeft(20)`); `db::load_reco_bonus`
            // returns those two values (or 0/0 when the row is absent).
            rec_have: c.rec_have,
            rec_left: c.rec_left,
            reco_two_hours_given: false,
            reco_give_seq: 0,
            pc_cafe_seq: 0,
            clan_id: c.clan_id,
            clan_privs: c.clan_privs,
            clan_leader: false, // fixed up at enter-world from World.clans
            pledge_class: 0,    // recomputed with clan_leader from World.clans
            clan_create_expiry_time: c.clan_create_expiry_time,
            clan_join_expiry_time: c.clan_join_expiry_time,
            create_date: c.create_date.clone(),
            power_grade: c.power_grade,
            ally_id: 0, // synced from the clan at enter-world
            siege_state: 0,
            siege_side: 0,
            pledge_type: c.pledge_type,
            lvl_joined_academy: c.lvl_joined_academy,
            apprentice: c.apprentice,
            sponsor: c.sponsor,
            clan_crest_id: 0, // synced from the clan at enter-world
            clan_crest_large_id: 0,
            ally_crest_id: 0, // synced from the clan at enter-world
            face: c.face,
            hair_style: c.hair_style,
            hair_color: c.hair_color,
            cast_seq: 0,
            pending_revive: false,
            lost_exp_on_death: c.lost_exp_on_death,
            revive_request: None,
            summon_request: None,
            pending_pet_collar: None,
            pending_mercenary_ticket: None,
            teleporting: false,
            jailed: false,
            sitting: false,
            selling_buffs: false,
            sell_buff_list: Vec::new(),
            last_petition_gm_name: None,
            snoop_listeners: Vec::new(),
            snooped: Vec::new(),
            gm_hidden: false,
            quest_zone_id: -1,
            charged_shots: 0,
            auto_shots: Vec::new(),
            mount_type: 0,
            mount_npc_id: 0,
            mount_level: 0,
            mount_feed: 0,
            mount_collar_object_id: 0,
            char_info_pending: false,
            trade_refusal: false,
            // Java `Player.restore`: `if (player.isGM())
            // setOverrideCond(variables.getLong(COND_OVERRIDE_KEY,
            // PlayerCondOverride.getAllExceptionsMask()))` — a GM who has never
            // touched `//set_exception` overrides **everything** by default.
            // The port used to start every character at 0, which left
            // `//exceptions` showing a GM as overriding nothing and made
            // `SkillCheckGM` unreachable (nothing ever held the override at
            // load, so the key it gates could not matter). The variable itself
            // is still not persisted here, so this is the default arm only.
            cond_overrides: if data.admin.is_gm(c.access_level) {
                crate::game_loop::admin::all_exceptions_mask()
            } else {
                0
            },
            transform_id: 0,
            transform_display_id: 0,
            store_type: 0,
            spawn_protect_end_tick: 0,
        };
        // Filled in by `recalculate_stats` (incl. atk_range/random_dmg, which it
        // sets from the equipped weapon or the class template).
        let mut combat = CombatStats::default();
        let mut mods = StatModifiers::default();
        let mut buffs = Buffs::default();
        p.recalculate_stats(
            data,
            &base_stats,
            &mods,
            &inventory,
            &mut speeds,
            &mut combat,
        );
        // Java `restoreCharData` → `addSkill`: fold the character's known
        // armor-conditioned passives (Spellcraft/Magician's Movement) into the
        // stat maps now, so the enter-world `UserInfo` burst already carries them
        // (no separate post-spawn resend). Timed buffs aren't restored yet.
        // Transform-granted skills are session-only and are filtered out of
        // every flush (`net::build_save_data`), but rows written before that
        // filter existed can still be in the DB — drop them here too, since a
        // fresh login is never transformed (Dissonance 5437's Accuracy -50
        // otherwise follows the character across relogs).
        let skills = SkillBook(
            c.skills
                .iter()
                // Armor-set skills are session-only for the same reason and
                // are dropped here too; the worn set re-grants them a few lines
                // below, so a character who logs in wearing one keeps the bonus
                // while one who logged out and sold the set does not.
                .filter(|&&(id, _, _)| {
                    !data.transforms.is_transform_skill(id)
                        && !data.armor_sets.is_armor_set_skill(id)
                })
                .map(|&(id, lvl, _)| (id, lvl))
                .collect(),
        );
        // Java `restoreSkills`' skill check. It runs over the rows read **from
        // the database**, which is the whole reason it sits here and not after
        // the derived grants below: Java iterates its `ResultSet`, not the
        // finished `_skills` map, so a skill that is granted rather than stored
        // is never a candidate. Check the book instead and the armour-set and
        // noble grants — which are in no allow-list arm, correctly — get eaten
        // the moment they are added.
        let (skills, illegal_skills) = check_restored_skills(data, c, cond_overrides, skills);

        // Re-grant whatever the gear the character logged out wearing entitles
        // them to. The rows themselves were just filtered out above, so this is
        // the only thing that puts a set bonus back — without it a relog would
        // silently drop every armor-set passive, which is precisely how the
        // augment options regressed before they were re-derived here too.
        let mut skills = skills;
        for (id, level) in
            crate::game_loop::armor_sets::granted_skills_for(&data.armor_sets, &inventory)
        {
            skills.0.insert(id, level);
        }
        // Java `Player.restore`: `player.setNoble(rset.getInt("nobless") == 1)`,
        // whose `setNoble(true)` grants the noble tree with
        // `addSkill(skill, false)` — **granted from the column, never
        // persisted**. The port had it the other way round: nothing re-granted
        // at load and the skills survived only as `character_skills` rows, so a
        // nobless who was stripped of it kept every skill, and the rows are
        // exactly what `is_skill_allowed` is built to reject. Deriving them
        // here is what lets the check remove the rows without taking the
        // skills with them.
        if c.noble {
            for &(id, level) in data.skill_trees.noble_skills() {
                skills.0.insert(id, level);
            }
        }

        // The enchant sub-levels ride the same rows (PLAN_G19_SKILL_ENCHANT.md).
        let skill_enchants = components::SkillEnchants(
            c.skills
                .iter()
                .filter(|&&(_, _, sub)| sub > 0)
                .map(|&(id, _, sub)| (id, sub))
                .collect(),
        );
        // Java `restoreRecipeBook`: classify each stored recipe-list id into the
        // dwarven/common book by its `RecipeList.isDwarvenRecipe()`; ids with no
        // matching recipe are dropped (Java's `recipe == null` continue).
        let mut recipe_book = components::RecipeBook::default();
        for &list_id in &c.recipe_book {
            match data.recipes.get(list_id) {
                Some(r) if r.is_dwarven => recipe_book.dwarven.push(list_id),
                Some(_) => recipe_book.common.push(list_id),
                None => {}
            }
        }
        // The HP-conditioned passives (Final Frenzy 290, Final Fortress 291)
        // are evaluated against the *stored* HP, so a character who logs out
        // below 30 % logs back in with the bonus already up — which is what
        // Java's first `recalculateStats` after `restore` does.
        for buff in conditioned_passive_buffs(
            data,
            &skills,
            &inventory,
            hp_percent_of(vitals.cur_hp, vitals.max_hp),
        ) {
            p.apply_buff(
                data,
                &base_stats,
                &mut mods,
                &inventory,
                &mut buffs,
                &mut speeds,
                &mut combat,
                buff,
            );
        }
        // Those passive skills can carry MaxHp/MaxMp/MaxCp modifiers (e.g. a
        // mystic's MP passives, which drive most of an Archmage's MP pool). They
        // land in `mods` above, but the vitals were computed before the passive
        // pass — recompute them now so the enter-world `UserInfo` carries the
        // boosted maxima. Java's Max{Hp,Mp,Cp}Finalizer run inside the same
        // `recalculateStats`; keep current values (clamp only on shrink).
        vitals.max_hp = calc_max_hp(data, &t, c.level, Some(&inventory), &mods) as i32;
        vitals.max_mp = calc_max_mp(data, &t, c.level, Some(&inventory), &mods) as i32;
        player_vitals.max_cp = calc_max_cp(data, &t, c.level, &mods) as i32;
        vitals.cur_hp = vitals.cur_hp.min(vitals.max_hp as f64);
        vitals.cur_mp = vitals.cur_mp.min(vitals.max_mp as f64);
        player_vitals.cur_cp = player_vitals.cur_cp.min(player_vitals.max_cp as f64);

        // `ShortCuts.restoreMe`'s verification tail: ITEM shortcuts whose
        // object id left the inventory are dropped here, so they never reach the
        // bundle and the next persistence flush's reconcile removes their rows
        // (memory-first — no per-select `DeleteShortcut`; see
        // `stale_item_shortcuts`). Surviving *EtcItem* shortcuts pick up the
        // template's shared reuse group (weapons/armor keep -1 on restore — a
        // Java quirk kept as-is).
        let shortcuts = c
            .shortcuts
            .iter()
            .filter(|sc| {
                sc.kind != shortcut::ShortcutType::Item
                    || c.items.iter().any(|i| i.object_id == sc.id)
            })
            .map(|sc| {
                let mut sc = *sc;
                if sc.kind == shortcut::ShortcutType::Item {
                    let is_etc = c
                        .items
                        .iter()
                        .find(|i| i.object_id == sc.id)
                        .and_then(|i| data.item_data.get(i.item_id))
                        .is_some_and(|t| t.kind == crate::data::item_data::ItemKind::Etc);
                    if is_etc {
                        // `shared_reuse_group` template default (never set in
                        // this dist's item XMLs).
                        sc.shared_reuse_group = 0;
                    }
                }
                sc
            })
            .collect();

        PlayerData {
            player: p,
            position: Position {
                x: c.x,
                y: c.y,
                z: c.z,
                heading: 0,
            },
            region: RegionCell(crate::world::region_of(c.x, c.y)),
            vitals,
            player_vitals,
            base_stats,
            speeds,
            collision,
            combat,
            buffs,
            stat_modifiers: mods,
            inventory,
            warehouse,
            freight,
            skills,
            skill_enchants,
            henna,
            recipe_book,
            variables: components::PlayerVariables(c.variables.iter().cloned().collect()),
            pets: components::PlayerPets(
                c.pets
                    .iter()
                    .map(|p| (p.collar_object_id, p.clone()))
                    .collect(),
            ),
            summons: components::PlayerSummons(c.summons.clone()),
            pet_inventory,
            shortcuts: Shortcuts::from_list(shortcuts),
            macros: Macros::from_list(c.macros.clone()),
            friends: components::Friends(c.friends.clone()),
            quests: components::Quests(c.quests.clone()),
            // Filled by the select path via `restore_reuses` (needs the game
            // tick); empty here keeps the many test callers unchanged.
            reuses: Reuses::default(),
            // Likewise filled by the select path, via `restore_buffs`.
            pending_buffs: Vec::new(),
            illegal_skills,
        }
    }

    /// The ITEM shortcuts `from_char` will prune (object id no longer in the
    /// inventory) — the character-select handler deletes their DB rows, the
    /// `deleteShortCutFromDb` half of `ShortCuts.restoreMe`'s verification.
    pub fn stale_item_shortcuts(c: &CharData) -> Vec<(i32, i32)> {
        c.shortcuts
            .iter()
            .filter(|sc| {
                sc.kind == shortcut::ShortcutType::Item
                    && !c.items.iter().any(|i| i.object_id == sc.id)
            })
            .map(|sc| (sc.slot, sc.page))
            .collect()
    }

    /// Java `CreatureStat.recalculateStats` narrowed to the combat stats G6
    /// computes. Re-derives from the class template's base values (not from
    /// `self`, so it's idempotent) × `BaseStat` bonus × level mod, folds in the
    /// equipped gear's `<stats>` contributions, then `stats_add`/`stats_mul`
    /// (buffs). Call after any level/buff/gear change. Gear applies in two
    /// ways, matching the Java finalizers (see [`EquippedBonuses`]): the
    /// weapon's pAtk/mAtk/atk-speed/crit *replace* the naked class base before
    /// the STR/level multipliers; armor/jewel pDef/mDef/accuracy/evasion are
    /// *summed* on top. maxHp/maxMp gear bonuses are **not** missing: they are
    /// computed on a separate path, `calc_max_hp`/`calc_max_mp`, which folds the
    /// same `equipped_stat_sum` for `Stat::MaxHp`/`MaxMp`.
    pub fn recalculate_stats(
        &self,
        data: &GameData,
        base: &BaseStats,
        mods: &StatModifiers,
        inventory: &Inventory,
        speeds: &mut Speeds,
        combat: &mut CombatStats,
    ) {
        let t = data
            .player_templates
            .get_or_base(self.class_id, self.base_class_id)
            .cloned()
            .unwrap_or_default();
        let eq = EquippedBonuses::from_inventory(inventory, data, &t);
        let level_mod = (self.level as f64 + 89.0) / 100.0;
        let sb = &data.stat_bonus;
        let str_bonus = sb.bonus(BaseStat::Str, base.str_);
        let dex_bonus = sb.bonus(BaseStat::Dex, base.dex);
        let int_bonus = sb.bonus(BaseStat::Int, base.int_);
        let wit_bonus = sb.bonus(BaseStat::Wit, base.wit);
        // Java's stat display getters (`getPAtk`/`getPDef`/…) return `(int)
        // getValue()` — a truncation toward zero, *not* a round. The engine
        // stores the finalized double and the packet layer truncates (`as i32`
        // in `user_info`), so nothing here rounds; the `as i32`/`.trunc()`
        // casts below match Java's display exactly.

        // Java `IStatFunction.calcWeaponBaseValue`: a transform's `<base>` block
        // stands in for the equipped weapon — but only for the forms the weapon
        // branch *excludes*. That `else if` fires when the player is
        // untransformed **or** the form is `COMBAT`/`MODE_CHANGE`, and it
        // overwrites whatever the transform contributed; so a COMBAT form keeps
        // swinging its real weapon, and every other form (NON_COMBAT,
        // RIDING_MODE, PURE_STAT, FLYING, CURSED) fights with the template's
        // numbers instead. `None` here means "weapon rules apply as usual".
        //
        // Live on both transforms a player can actually enter on this dist —
        // 105 (NON_COMBAT) and 20008 (RIDING_MODE); see the reachability census
        // in `data::transform_data`.
        let tf_base = (self.transform_id != 0)
            .then(|| data.transforms.get(self.transform_id))
            .flatten()
            .filter(|tf| !tf.kind.weapon_overrides_base())
            .and_then(|tf| tf.template(self.is_female).base.as_ref());
        // Each field falls back to the *class* template value, never to the
        // weapon's: Java's `getStats(stat, baseTemplateValue)` hands back the
        // template default for any key the transform doesn't set.
        let tf_or = |tf: Option<f64>, weapon: Option<f64>, class_base: f64| {
            if tf_base.is_some() { tf } else { weapon }.unwrap_or(class_base)
        };

        // PAttackFinalizer / MAttackFinalizer: the equipped weapon's pAtk/mAtk
        // replaces the naked base (`calcWeaponBaseValue`) before STR/level.
        let p_atk_base = tf_or(
            tf_base.and_then(|b| b.p_atk),
            eq.weapon_p_atk,
            t.base_p_atk as f64,
        );
        let m_atk_base = tf_or(
            tf_base.and_then(|b| b.m_atk),
            eq.weapon_m_atk,
            t.base_m_atk as f64,
        );
        let caps = &data.combat_caps;
        // Every max cap below goes through Java's `validateValue`, which skips
        // the ceiling for creatures with the MAX_STATS_VALUE cond override —
        // granted to GMs on login (Player.restore). Floors still apply.
        let cap = |max: f64| if self.is_gm(data) { f64::MAX } else { max };
        // Java adds `calcEnchantedItemBonus` to the weapon base *before* the
        // stat bonus and level mod, so an enchant on a level-80 character is
        // worth far more than the flat table suggests.
        combat.p_atk = finalize(
            mods,
            Stat::PhysicalAttack,
            (p_atk_base + eq.enchant_p_atk) * str_bonus * level_mod,
        )
        .clamp(0.0, cap(caps.max_p_atk));
        combat.m_atk = finalize(
            mods,
            Stat::MagicalAttack,
            (m_atk_base + eq.enchant_m_atk) * (int_bonus * level_mod).powf(2.2072),
        )
        .clamp(0.0, cap(caps.max_m_atk));

        // P/MDefenseFinalizer: (naked base + summed gear def − the naked defense
        // of every occupied slot) × levelMod (mDef also × MEN bonus), then the
        // `defaultValue` mul(≥0.5)/add and the `base × 0.2` floor.
        let p_def_pre =
            (t.base_p_def as f64 + eq.enchant_p_def + eq.p_def - eq.p_def_slot_sub) * level_mod;
        combat.p_def = finalize_def(
            mods,
            Stat::PhysicalDefence,
            p_def_pre,
            t.base_p_def as f64 * 0.2,
        );
        let men_bonus = if base.men > 0 {
            sb.bonus(BaseStat::Men, base.men)
        } else {
            1.0
        };
        let m_def_pre = (t.base_m_def as f64 + eq.enchant_m_def + eq.m_def - eq.m_def_slot_sub)
            * men_bonus
            * level_mod;
        combat.m_def = finalize_def(
            mods,
            Stat::MagicalDefence,
            m_def_pre,
            t.base_m_def as f64 * 0.2,
        );

        // P/MAttackSpeedFinalizer: weapon replaces base; `mul` floors at 0.7.
        // `<base attackSpeed=…>` feeds `Stat.PHYSICAL_ATTACK_SPEED` only —
        // no transform block sets a magic attack speed, so `m_atk_spd` below
        // keeps the class base under every form.
        let p_atk_spd_base = tf_or(
            tf_base.and_then(|b| b.attack_speed),
            eq.weapon_p_atk_spd,
            t.base_p_atk_spd as f64,
        );
        combat.p_atk_spd =
            finalize_speed(mods, Stat::PhysicalAttackSpeed, p_atk_spd_base * dex_bonus)
                .clamp(1.0, cap(caps.max_p_atk_speed)) as i32;
        combat.m_atk_spd = finalize_speed(
            mods,
            Stat::MagicAttackSpeed,
            t.base_m_atk_spd as f64 * wit_bonus,
        )
        .clamp(1.0, cap(caps.max_m_atk_speed)) as i32;

        // P/MCritRateFinalizer (in per-mille, ×10): weapon replaces base crit.
        // Only the *physical* rate goes through `calcWeaponBaseValue`; Java's
        // `MCritRateFinalizer` uses `calcWeaponPlusBaseValue`, which a
        // transform's `<base>` never contributes a MAGIC_CRITICAL_RATE key to.
        let crit_base = tf_or(
            tf_base.and_then(|b| b.crit_rate),
            eq.weapon_crit,
            t.base_crit_rate as f64,
        );
        let m_crit_base = eq.weapon_m_crit.unwrap_or(t.base_m_crit_rate as f64);
        combat.crit_hit = finalize(mods, Stat::CriticalRate, crit_base * dex_bonus * 10.0)
            .clamp(0.0, cap(caps.max_p_crit_rate));
        combat.m_crit_hit = finalize(
            mods,
            Stat::MagicCriticalRate,
            m_crit_base * wit_bonus * 10.0,
        )
        .clamp(0.0, cap(caps.max_m_crit_rate));

        // P/MAccuracyFinalizer, P/MEvasionRateFinalizer. Gear accuracy/evasion
        // sums add on top (`calcWeaponPlusBaseValue`). `as i32` truncates toward
        // zero, matching Java's `(int)` display getter. The high-level +N steps
        // above level 69 apply only to the *physical* P{Accuracy,EvasionRate}
        // finalizers for players (the M-variants for players have no steps).
        let level = self.level as f64;
        // High-level bonus steps from P{Accuracy,EvasionRate}Finalizer: at lv 80
        // this sums to +12 (11 for >69, +1 for >77).
        let hi_level_step = |lvl: i32| -> f64 {
            let mut b = 0.0;
            if lvl > 69 {
                b += (lvl - 69) as f64;
            }
            if lvl > 77 {
                b += 1.0;
            }
            if lvl > 80 {
                b += 2.0;
            }
            if lvl > 87 {
                b += 2.0;
            }
            if lvl > 92 {
                b += 1.0;
            }
            if lvl > 97 {
                b += 1.0;
            }
            b
        };
        let acc_ev_step = hi_level_step(self.level);
        combat.accuracy = finalize(
            mods,
            Stat::AccuracyCombat,
            (base.dex as f64).sqrt() * 5.0 + level + acc_ev_step + eq.accuracy,
        ) as i32;
        combat.magic_accuracy = finalize(
            mods,
            Stat::AccuracyMagic,
            (base.wit as f64).sqrt() * 3.0 + level * 2.0 + eq.magic_accuracy,
        ) as i32;
        // `PEvasionRateFinalizer` ends on `validateValue(…, Double.NEGATIVE_INFINITY,
        // MAX_EVASION)` — a **ceiling only**. Evasion is allowed to go negative,
        // and 309 skills on this dist carry a `PhysicalEvasion` effect reaching
        // −60, which is more than a low-level character's whole base; flooring
        // it at 0 would hand them evasion they should not have.
        combat.evasion = finalize(
            mods,
            Stat::EvasionRate,
            (base.dex as f64).sqrt() * 5.0 + level + acc_ev_step + eq.evasion,
        )
        .min(cap(caps.max_evasion)) as i32;
        // `MEvasionRateFinalizer` runs the **same** `validateValue` ceiling as its
        // physical twin — `MAX_EVASION` (250 here), which a level-80 caster's
        // `sqrt(WIT)·3 + level·2` base can pass once buffs pile on.
        combat.magic_evasion = finalize(
            mods,
            Stat::MagicEvasionRate,
            (base.wit as f64).sqrt() * 3.0 + level * 2.0 + eq.magic_evasion,
        )
        .min(cap(caps.max_evasion)) as i32;

        // Weapon range / damage spread replace the class template constants
        // while a weapon is equipped (`PRangeFinalizer` / `RandomDamageFinalizer`).
        // `PRangeFinalizer` is a plain `defaultValue(base*mul+add)` finalizer —
        // Archery 431/Long Shot 113/Rapid Fire 413/Snipe 972 (`PhysicalAttackRange`,
        // all `<weaponType>BOW</weaponType>`-conditioned) previously had no stat
        // to land on here at all.
        combat.atk_range = finalize(
            mods,
            Stat::PhysicalAttackRange,
            tf_or(
                tf_base.and_then(|b| b.attack_range),
                eq.weapon_atk_range.map(|r| r as f64),
                t.base_atk_range as f64,
            ),
        ) as i32;
        // `RandomDamageFinalizer` is `calcWeaponBaseValue` too, so a
        // transform's `randomDamage` replaces the weapon's spread the same way.
        // The bare `10` is the class-template stand-in Java reads from the
        // player template's own RANDOM_DAMAGE default.
        combat.random_dmg = tf_or(
            tf_base.and_then(|b| b.random_damage),
            eq.weapon_random_dmg.map(|d| d as f64),
            10.0,
        ) as i32;
        // `ShotsBonusFinalizer`. Nothing on this dist declares a `shotBonus`
        // stat modifier, so `Stat.defaultValue`'s mul/add pair is the identity
        // and the weapon enchant is the whole of it.
        combat.shots_bonus_add = eq.shots_bonus - 1.0;

        // SpeedFinalizer: every player speed stat gets `Config.RUN_SPD_BOOST`
        // added in `getBaseSpeed` (35 on this dist — see `CombatCaps`).
        // Buffs (Speed effect) apply through the add/mul maps like the combat
        // stats above; stored as f64 (Speeds is shared with NPCs, whose
        // templates don't take the player boost). The `as i16` in `user_info`
        // truncates for display, matching Java's `(int)` getter.
        // `SpeedFinalizer.getBaseSpeed`: a mounted player's base speeds are the
        // mount's `speed_on_ride` row (looked up at the *mount's* level),
        // halved when the mount is 10+ levels above the rider — the class
        // template only stands in when the species has no row (Java gets null
        // back and keeps `calcWeaponPlusBaseValue`).
        //
        // Java halves again on `player.isHungry()`, which is **inert** for a
        // rider — the predicate requires `hasPet()` and `mount()` unsummons the
        // pet a line after starting the feed, so it can never be true. See
        // `game_loop::admin::mounts::is_hungry`; omitted here deliberately
        // rather than "not ported".
        let ride = if self.is_mounted() {
            data.pet_data
                .get(self.mount_npc_id)
                .and_then(|pet| pet.level_row(self.mount_level))
        } else {
            None
        };
        let level_gap_penalty = if self.mount_level - self.level >= 10 {
            0.5
        } else {
            1.0
        };
        let base_speed = |ride_spd: Option<f64>, class_base: f64| {
            ride_spd.map_or(class_base, |s| s * level_gap_penalty) + caps.run_spd_boost
        };
        speeds.run_spd = finalize(
            mods,
            Stat::RunSpeed,
            base_speed(ride.map(|r| r.ride_run_spd), t.base_run_spd as f64),
        );
        speeds.walk_spd = finalize(
            mods,
            Stat::WalkSpeed,
            base_speed(ride.map(|r| r.ride_walk_spd), t.base_walk_spd as f64),
        );
        speeds.swim_run_spd = finalize(
            mods,
            Stat::SwimRunSpeed,
            base_speed(
                ride.map(|r| r.ride_fast_swim_spd),
                t.base_swim_run_spd as f64,
            ),
        );
        speeds.swim_walk_spd = finalize(
            mods,
            Stat::SwimWalkSpeed,
            base_speed(
                ride.map(|r| r.ride_slow_swim_spd),
                t.base_swim_walk_spd as f64,
            ),
        );

        // A transform replaces the class base run/walk with the template's
        // `<moving>` values (Java's transform move-speed override), still folding
        // the buff modifiers on top. Absolute template speeds — the class
        // `RUN_SPD_BOOST` is not re-added (the transform values are self-tuned).
        if self.transform_id != 0
            && let Some(tf) = data.transforms.get(self.transform_id)
        {
            let tmpl = tf.template(self.is_female);
            if let Some(run) = tmpl.run_spd {
                speeds.run_spd = finalize(mods, Stat::RunSpeed, run);
            }
            if let Some(walk) = tmpl.walk_spd {
                speeds.walk_spd = finalize(mods, Stat::WalkSpeed, walk);
            }
        }

        // `SpeedFinalizer`: a playable inside a `SwampZone` has every speed
        // scaled, after the boost and before the clamp.
        if speeds.swamp_multiplier != 1.0 {
            let m = speeds.swamp_multiplier;
            speeds.run_spd *= m;
            speeds.walk_spd *= m;
            speeds.swim_run_spd *= m;
            speeds.swim_walk_spd *= m;
        }

        // SpeedFinalizer's `validateValue`: players clamp to [1, MaxRunSpeed]
        // (300 on this dist).
        let speed_cap = cap(caps.max_run_speed);
        for spd in [
            &mut speeds.run_spd,
            &mut speeds.walk_spd,
            &mut speeds.swim_run_spd,
            &mut speeds.swim_walk_spd,
        ] {
            *spd = spd.clamp(1.0, speed_cap);
        }
    }

    /// Land a buff, applying Java `EffectList.addActive`'s stacking and the
    /// `MaxBuffAmount`/`MaxDanceAmount` slot caps, then recompute. Returns
    /// whether the buff actually landed (`false` = refused because a same-type
    /// buff of equal/higher level is already active). Java
    /// `BuffInfo.initializeEffects` → `AbstractEffect.pump`.
    pub fn apply_buff(
        &self,
        data: &GameData,
        base: &BaseStats,
        mods: &mut StatModifiers,
        inventory: &Inventory,
        buffs: &mut Buffs,
        speeds: &mut Speeds,
        combat: &mut CombatStats,
        buff: ActiveBuff,
    ) -> bool {
        // Passive stat-pump markers aren't real buffs — they never stack-conflict
        // or count against the caps; fold and push as before.
        if buff.passive {
            for effect in &buff.effects {
                apply_modifier(mods, effect);
            }
            buffs.0.push(buff);
            self.recalculate_stats(data, base, mods, inventory, speeds, combat);
            return true;
        }

        // Java `EffectList.addActive` stacking: effects with no abnormal type
        // conflict only with the same skill id; typed effects conflict with any
        // buff of the same abnormal type.
        let none_type = buff.abnormal_type.is_empty() || buff.abnormal_type == "NONE";
        let conflict = buffs.0.iter().position(|e| {
            if none_type {
                e.skill_id == buff.skill_id
            } else {
                e.abnormal_type == buff.abnormal_type
            }
        });
        if let Some(idx) = conflict {
            // The higher (or equal) abnormal level wins; a lower one is refused.
            if buff.abnormal_level >= buffs.0[idx].abnormal_level {
                buffs.0.remove(idx);
            } else {
                return false;
            }
        }

        // Slot count cap: drop the oldest same-pool buff until this one fits
        // (Java removes the oldest in-use buff of the exceeding category).
        // `EnlargeAbnormalSlot` (Divine Inspiration 1405) raises the *good
        // buff* cap only — Java's `setMaxBuffCount`, which `EffectList` reads
        // for the buff pool and never for dances (G34 S4).
        let bonus_slots = mods.add.get(&Stat::MaxBuffSlots).copied().unwrap_or(0.0) as i32;
        let cap = match buff.slot {
            BuffSlot::Buff => Some(data.combat_caps.max_buff_count + bonus_slots),
            BuffSlot::Dance => Some(data.combat_caps.max_dance_count),
            BuffSlot::Uncapped => None,
        };
        if let Some(cap) = cap.filter(|c| *c > 0) {
            while buffs.0.iter().filter(|b| b.slot == buff.slot).count() as i32 >= cap {
                let Some(oldest) = buffs.0.iter().position(|b| b.slot == buff.slot) else {
                    break;
                };
                buffs.0.remove(oldest);
            }
        }

        buffs.0.push(buff);
        // A removal/override means the maps must be rebuilt from the survivors
        // (can't just fold the new one in) — same as `remove_buff`.
        mods.add.clear();
        mods.mul.clear();
        mods.by_move_type.clear();
        mods.by_position.clear();
        for b in &buffs.0 {
            for effect in &b.effects {
                apply_modifier(mods, effect);
            }
        }
        self.recalculate_stats(data, base, mods, inventory, speeds, combat);
        true
    }

    /// Remove an expired/replaced buff and recompute from scratch (Java just
    /// removes the `BuffInfo` and calls `resetStats()`, which rebuilds the
    /// maps from the remaining active buffs — do the same here rather than
    /// trying to subtract in place, which would drift under rounding).
    pub fn remove_buff(
        &self,
        data: &GameData,
        base: &BaseStats,
        mods: &mut StatModifiers,
        inventory: &Inventory,
        buffs: &mut Buffs,
        speeds: &mut Speeds,
        combat: &mut CombatStats,
        skill_id: i32,
    ) {
        buffs.0.retain(|b| b.skill_id != skill_id);
        mods.add.clear();
        mods.mul.clear();
        mods.by_move_type.clear();
        mods.by_position.clear();
        for buff in &buffs.0 {
            for effect in &buff.effects {
                apply_modifier(mods, effect);
            }
        }
        self.recalculate_stats(data, base, mods, inventory, speeds, combat);
    }

    /// Fraction of the way through the current level (for XP-bar display).
    pub fn exp_percent(&self, data: &GameData) -> f64 {
        let base = data.experience.exp_for_level(self.level);
        let next = data.experience.exp_for_level(self.level + 1);
        if next - base <= 0 {
            0.0
        } else {
            (self.exp - base) as f64 / (next - base) as f64
        }
    }
}

/// `Stat.defaultValue`: `base * mul + add` from the accumulated modifier
/// maps (1.0/0.0 when nothing has touched this stat). `pub(crate)`: also used
/// by `game_loop::combat::shield_stats`, which finalizes `ShieldDefence`/
/// `ShieldDefenceRate` over the equipped shield's own `sDef`/`rShld` outside
/// the `recalculate_stats` pass (shield block stats aren't cached on
/// `CombatStats`, so they're finalized fresh at combat-lookup time instead).
///
/// **This is the one place the order lives.** Java's `getValue(stat, base)` is
/// `(mul × base) + add`, and the alternative reading — folding `add` inside the
/// multiply — agrees on every stat carrying only one kind of modifier, so a
/// respelling of this formula elsewhere can be wrong for years without a test
/// noticing. `water::breath_ms` was, until 2026-08-18. Call this; do not
/// rewrite it.
///
/// **One term is deliberately missing.** Java adds
/// `getMoveTypeValue(stat, creature.getMoveType())`, which this cannot: the
/// move type is not a property of `StatModifiers`. That is safe only because
/// of what the datapack contains — the sole stats any `StatByMoveType` effect
/// on this dist targets are `REGENERATE_*` (64 entries) and `EVASION` (1), and
/// both are finalized at their own call sites, which do add the term
/// (`game_loop::regen`, `game_loop::combat`). A stat that acquires a
/// `by_move_type` entry **and** comes through here would silently lose it, so
/// check that before routing a new stat to this function.
pub(crate) fn finalize(mods: &StatModifiers, stat: Stat, base: f64) -> f64 {
    if let Some(&fixed) = mods.fixed.get(&stat) {
        return fixed;
    }
    let mul = mods.mul.get(&stat).copied().unwrap_or(1.0);
    let add = mods.add.get(&stat).copied().unwrap_or(0.0);
    base * mul + add
}

/// `P/MDefenseFinalizer.defaultValue`: `mul` floors at 0.5, and the result is
/// floored at `base × 0.2` (the class template's naked defense × 0.2) so a
/// heavy defense debuff can't drop below a fifth of the naked value.
fn finalize_def(mods: &StatModifiers, stat: Stat, base: f64, floor: f64) -> f64 {
    if let Some(&fixed) = mods.fixed.get(&stat) {
        return fixed;
    }
    let mul = mods.mul.get(&stat).copied().unwrap_or(1.0).max(0.5);
    let add = mods.add.get(&stat).copied().unwrap_or(0.0);
    (base * mul + add).max(floor)
}

/// `P/MAttackSpeedFinalizer.defaultValue`: same shape, but `mul` floors at
/// 0.7 instead of applying whatever's in the map directly (so an absent or
/// tiny buff doesn't produce a slower-than-0.7x cast/attack speed).
fn finalize_speed(mods: &StatModifiers, stat: Stat, base: f64) -> f64 {
    if let Some(&fixed) = mods.fixed.get(&stat) {
        return fixed;
    }
    let mul = mods.mul.get(&stat).copied().unwrap_or(1.0).max(0.7);
    let add = mods.add.get(&stat).copied().unwrap_or(0.0);
    base * mul + add
}

/// The permanent stat modifiers an NPC's *passive* template skills contribute.
/// Java's `Creature` constructor copies every `template.getSkills()` onto the
/// mob (`for (Skill s : template.getSkills().values()) addSkill(s)`); the
/// passive ones (operateType `P`) pump stats through the same add/mul maps as
/// buffs — this is where a retail mob's real HP/atk/def come from (skills 4408
/// HP Increase, 4410 P.Atk, 4412 P.Def, …), on top of the raw `<vitals>`/
/// `<attack>` base. The NPC counterpart of the player's `conditioned_passive_buffs`.
///
/// Weapon-conditioned effects (4415 "One-handed Sword" mastery, …) evaluate
/// against the template's `<equipment>` right hand — the NPC counterpart of
/// the player's paperdoll check. Armor-conditioned ones stay skipped: an NPC
/// wears no armor pieces, and Java's `<using kind="Heavy">` evaluates false
/// there too.
fn npc_passive_mods(data: &GameData, t: &crate::data::npc_data::NpcTemplate) -> StatModifiers {
    use crate::model::skill::{OperateType, SkillEffect};
    let mut mods = StatModifiers::default();
    for &(skill_id, level) in &t.skill_list {
        let Some(skill) = data.skill_data.get(skill_id, level) else {
            continue;
        };
        if skill.operate_type != OperateType::Passive {
            continue;
        }
        for effect in &skill.effects {
            if let SkillEffect::StatModifier(m) = effect
                && m.armor_condition == 0
                && (m.weapon_condition == 0
                    || (t.rhand != 0
                        && m.weapon_condition & data.item_data.weapon_type(t.rhand).mask_bit()
                            != 0))
            {
                apply_modifier(&mut mods, m);
            }
        }
    }
    mods
}

/// The champion multipliers that reach the NPC stat pipeline, resolved from
/// `Custom/ChampionMonsters.ini` for one NPC. Neutral (all ×1) for an ordinary
/// mob and whenever `ChampionEnable` is off, so a caller that has no champion
/// state to offer can pass `Default` and change nothing.
///
/// This exists so the finalizers stay pure functions of (template, buffs,
/// mods) — the config itself lives on `World`, which the stat layer has no
/// access to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NpcStatMods {
    /// `ChampionAtk` — P.Atk and M.Atk.
    pub atk: f64,
    /// `ChampionSpdAtk` — P.Atk speed and M.Atk speed.
    pub spd_atk: f64,
    /// `RaidPAttackMultiplier` / `RaidMAttackMultiplier` — the raid-only pass
    /// in `P|MAttackFinalizer`, applied *after* the champion one.
    pub raid_p_atk: f64,
    pub raid_m_atk: f64,
    /// `RaidPDefenceMultiplier` / `RaidMDefenceMultiplier` — same, in
    /// `P|MDefenseFinalizer`. All four are 1.0 on this dist.
    pub raid_p_def: f64,
    pub raid_m_def: f64,
}

impl Default for NpcStatMods {
    fn default() -> Self {
        Self {
            atk: 1.0,
            spd_atk: 1.0,
            raid_p_atk: 1.0,
            raid_m_atk: 1.0,
            raid_p_def: 1.0,
            raid_m_def: 1.0,
        }
    }
}

impl NpcStatMods {
    /// The two guards the finalizers repeat: champion multipliers need
    /// `Config.CHAMPION_ENABLE && creature.isChampion()`, raid multipliers need
    /// `creature.isRaid()`. They are independent — a champion raid minion takes
    /// both.
    ///
    /// `is_raid` is the caller's, not the template's, because Java's `_isRaid`
    /// is an *instance* flag: `Monster.onSpawn` calls
    /// `setIsRaidMinion(_master.isRaid())`, which sets the very same field, so
    /// a raid boss's escort scales like the boss. Only the spawn site knows
    /// whether it is building a minion.
    pub(crate) fn of(cfg: &crate::config::CombatConfig, champion: bool, is_raid: bool) -> Self {
        let mut m = Self::default();
        if cfg.champion.enable && champion {
            m.atk = cfg.champion.atk;
            m.spd_atk = cfg.champion.spd_atk;
        }
        if is_raid {
            m.raid_p_atk = cfg.npc.raid_p_atk_multiplier;
            m.raid_m_atk = cfg.npc.raid_m_atk_multiplier;
            m.raid_p_def = cfg.npc.raid_p_def_multiplier;
            m.raid_m_def = cfg.npc.raid_m_def_multiplier;
        }
        m
    }
}

/// Finalize an NPC's `CombatStats`, `Speeds`, and max HP/MP from its template
/// base → passive template skills → active buffs, through the same add/mul maps
/// and `finalize*` the player uses. Max HP/MP fold in the CON/MEN bonus exactly
/// like Java's `Max{Hp,Mp}Finalizer` (`base × statBonus`, then the passive/buff
/// `mul`/`add`) — NPCs are uncapped there (the HP_LIMIT branch is player-only).
/// Shared by spawn ([`crate::model::npc::spawn`]) and buff recompute.
pub(crate) fn npc_finalized_stats(
    data: &GameData,
    t: &crate::data::npc_data::NpcTemplate,
    buffs: &Buffs,
    mods_in: NpcStatMods,
) -> (CombatStats, Speeds, f64, f64) {
    let sb = &data.stat_bonus;
    let caps = &data.combat_caps;
    let mut base = npc::npc_combat_stats(t, sb);
    // `PAttackFinalizer`/`MAttackFinalizer`/`P|MAttackSpeedFinalizer`:
    // `baseValue *= CHAMPION_ATK | CHAMPION_SPD_ATK` **before** the STR/DEX
    // bonus and before the buff mul/add. Multiplication commutes with the
    // bonus `npc_combat_stats` has already folded in, so scaling the base here
    // lands on the same number Java's chain does, and the caps below still
    // clamp last exactly like `validateValue`.
    base.p_atk *= mods_in.atk * mods_in.raid_p_atk;
    base.m_atk *= mods_in.atk * mods_in.raid_m_atk;
    base.p_atk_spd = (base.p_atk_spd as f64 * mods_in.spd_atk) as i32;
    base.m_atk_spd = (base.m_atk_spd as f64 * mods_in.spd_atk) as i32;
    // `P|MDefenseFinalizer`'s raid pass. There is no champion equivalent —
    // a champion hits harder but is no tougher.
    base.p_def *= mods_in.raid_p_def;
    base.m_def *= mods_in.raid_m_def;
    // Template passive skills are the NPC's innate stat base; player-cast buffs
    // (buffs.0) stack on top through the same maps.
    let mut mods = npc_passive_mods(data, t);
    for buff in &buffs.0 {
        for effect in &buff.effects {
            apply_modifier(&mut mods, effect);
        }
    }
    let combat = CombatStats {
        p_atk: finalize(&mods, Stat::PhysicalAttack, base.p_atk).clamp(0.0, caps.max_p_atk),
        m_atk: finalize(&mods, Stat::MagicalAttack, base.m_atk).clamp(0.0, caps.max_m_atk),
        // NPCs carry no naked-base/gear split, so the defense floor is a fifth
        // of the template value (mirrors the player's `base × 0.2`).
        p_def: finalize_def(&mods, Stat::PhysicalDefence, base.p_def, base.p_def * 0.2),
        m_def: finalize_def(&mods, Stat::MagicalDefence, base.m_def, base.m_def * 0.2),
        p_atk_spd: finalize_speed(&mods, Stat::PhysicalAttackSpeed, base.p_atk_spd as f64)
            .clamp(1.0, caps.max_p_atk_speed) as i32,
        m_atk_spd: finalize_speed(&mods, Stat::MagicAttackSpeed, base.m_atk_spd as f64)
            .clamp(1.0, caps.max_m_atk_speed) as i32,
        crit_hit: finalize(&mods, Stat::CriticalRate, base.crit_hit)
            .clamp(0.0, caps.max_p_crit_rate),
        m_crit_hit: base.m_crit_hit,
        accuracy: finalize(&mods, Stat::AccuracyCombat, base.accuracy as f64) as i32,
        evasion: finalize(&mods, Stat::EvasionRate, base.evasion as f64)
            .clamp(0.0, caps.max_evasion) as i32,
        magic_evasion: base.magic_evasion,
        magic_accuracy: base.magic_accuracy,
        // Range / random-damage aren't buffable here — keep the template values.
        atk_range: base.atk_range,
        random_dmg: base.random_dmg,
        // Buffs cannot move it: no skill on this dist declares a `shotBonus`
        // modifier, so an NPC's stays at the template's flat 1.
        shots_bonus_add: base.shots_bonus_add,
    };
    let speeds = Speeds {
        // No `RUN_SPD_BOOST` for NPCs (that's a player-only base add).
        run_spd: finalize(&mods, Stat::RunSpeed, t.base_run_spd),
        walk_spd: finalize(&mods, Stat::WalkSpeed, t.base_walk_spd),
        swim_run_spd: 0.0,
        swim_walk_spd: 0.0,
        move_multiplier: 1.0,
        base_run_spd: t.base_run_spd,
        base_walk_spd: t.base_walk_spd,
        // NPC templates on this dist declare no `<speed><…swim=…>`, so the
        // swim bases are 0 — `client_move_multiplier` falls back to 1.0 for
        // them, and nothing flips `swimming` on an NPC anyway (zone
        // revalidation is player-only in the port).
        base_swim_run_spd: 0.0,
        base_swim_walk_spd: 0.0,
        running: false,
        swimming: false,
        swamp_multiplier: 1.0,
    };
    // `Max{Hp,Mp}Finalizer`: `mul × (baseMax × {CON,MEN} bonus) + add`; the
    // bonus is skipped when the stat is 0 (`getX() > 0 ? bonus : 1`).
    let con_bonus = if t.base_con > 0 {
        sb.con_bonus(t.base_con)
    } else {
        1.0
    };
    let men_bonus = if t.base_men > 0 {
        sb.men_bonus(t.base_men)
    } else {
        1.0
    };
    let hp_mul = mods.mul.get(&Stat::MaxHp).copied().unwrap_or(1.0);
    let hp_add = mods.add.get(&Stat::MaxHp).copied().unwrap_or(0.0);
    let mp_mul = mods.mul.get(&Stat::MaxMp).copied().unwrap_or(1.0);
    let mp_add = mods.add.get(&Stat::MaxMp).copied().unwrap_or(0.0);
    let max_hp = hp_mul * (t.base_hp_max * con_bonus) + hp_add;
    let max_mp = mp_mul * (t.base_mp_max * men_bonus) + mp_add;
    (combat, speeds, max_hp, max_mp)
}

/// Rebuild an NPC's `CombatStats`/`Speeds`/max-HP·MP from its template (incl.
/// passive template skills) plus its active buffs — the NPC counterpart of
/// `Player::recalculate_stats` + `apply_buff`/`remove_buff`. Called on every
/// buff apply/expire, so it starts from a clean base each time and can't drift.
/// Current HP/MP are only clamped *down* to a new max (Java never heals on a
/// max increase).
pub(crate) fn recompute_npc_stats_from_buffs(
    data: &GameData,
    t: &crate::data::npc_data::NpcTemplate,
    buffs: &Buffs,
    mods_in: NpcStatMods,
    combat: &mut CombatStats,
    speeds: &mut Speeds,
    vitals: &mut Vitals,
) {
    let (new_combat, new_speeds, max_hp, max_mp) = npc_finalized_stats(data, t, buffs, mods_in);
    *combat = new_combat;
    // Preserve the live running/swimming state (a mid-chase mob is running);
    // only the speed magnitudes recompute.
    speeds.run_spd = new_speeds.run_spd;
    speeds.walk_spd = new_speeds.walk_spd;
    vitals.max_hp = max_hp as i32;
    vitals.max_mp = max_mp as i32;
    vitals.cur_hp = vitals.cur_hp.min(max_hp);
    vitals.cur_mp = vitals.cur_mp.min(max_mp);
}

/// Port of `ConditionUsingItemType.testImpl`'s armor branch (the only branch a
/// robe passive's `<armorType>` mask reaches): the condition passes when the
/// worn chest — and, unless the chest is full-armor, the worn legs — matches the
/// mask, treating a bare slot as `ArmorType::NONE`.
pub(crate) fn armor_condition_passes(
    mask: u8,
    inventory: &Inventory,
    items: &crate::data::item_data::ItemData,
) -> bool {
    use crate::data::item_data::{ArmorType, SLOT_FULL_ARMOR};
    use crate::model::inventory::PaperdollSlot;
    const NONE_BIT: u8 = ArmorType::None.mask_bit();
    let Some(chest) = inventory.paperdoll_item(PaperdollSlot::Chest) else {
        return mask & NONE_BIT != 0;
    };
    if mask & items.armor_type(chest.item_id).mask_bit() == 0 {
        return false;
    }
    if items
        .get(chest.item_id)
        .map(|t| t.body_part == SLOT_FULL_ARMOR)
        .unwrap_or(false)
    {
        return true;
    }
    let Some(legs) = inventory.paperdoll_item(PaperdollSlot::Legs) else {
        return mask & NONE_BIT != 0;
    };
    mask & items.armor_type(legs.item_id).mask_bit() != 0
}

/// Whether a skill effect's `<weaponType>` condition (`mask`, an OR of
/// `WeaponType::mask_bit`s) is satisfied by the currently equipped weapon. No
/// weapon (or a type not in the mask) → `false`, so e.g. Weapon Mastery 249's
/// `-30% MagicalAttackSpeed` only bites a BOW/POLE user, not a staff caster.
/// Java `ConditionUsingSlotType(ItemTemplate.SLOT_LR_HAND)` — the equipped
/// weapon occupies **both** hands.
///
/// Read off the weapon template's `bodypart`, which is how the datapack marks
/// a two-hander, rather than by inferring it from the left hand being empty
/// (that would also match an unarmed or shield-less one-hander).
pub(crate) fn two_handed_weapon_equipped(
    inventory: &Inventory,
    items: &crate::data::item_data::ItemData,
) -> bool {
    use crate::model::inventory::PaperdollSlot;
    inventory
        .paperdoll_item(PaperdollSlot::RHand)
        .and_then(|w| items.get(w.item_id))
        .is_some_and(|t| t.body_part == crate::data::item_data::SLOT_LR_HAND)
}

pub(crate) fn weapon_condition_passes(
    mask: u32,
    inventory: &Inventory,
    items: &crate::data::item_data::ItemData,
) -> bool {
    use crate::model::inventory::PaperdollSlot;
    let Some(weapon) = inventory.paperdoll_item(PaperdollSlot::RHand) else {
        return false;
    };
    mask & items.weapon_type(weapon.item_id).mask_bit() != 0
}

/// The armor-conditioned passive buffs currently in effect for a player: for
/// every known passive skill carrying stat effects, the subset whose
/// `<armorType>` condition passes against the worn gear, as a hidden permanent
/// `ActiveBuff` (Java's `Player.addSkill` passive effects, re-evaluated at pump
/// time). Skills whose effects are all gated out contribute nothing. Shared by
/// `from_char` (enter-world) and `game_loop::passive_skills` (equip changes).
/// `BaseStats` = the class template's six values **plus every flat bonus that
/// stacks onto them**: worn hennas (Java `recalcHennaStats`) and complete armor
/// sets (`BaseStatFinalizer`'s `getBaseStatValue`).
///
/// This exists because the composition had drifted into three hand-rolled
/// copies — the login build, the henna redraw, and (once sets landed) the
/// paperdoll change — and a term added to one is invisible to the others. Any
/// new flat base-stat source belongs here and nowhere else.
///
/// `None` when the object has no `Player`, i.e. nothing to compose for.
pub(crate) fn compose_base_stats(world: &crate::world::World, oid: i32) -> Option<BaseStats> {
    let (class_id, base_class_id) = world
        .objects
        .get_component::<Player>(&oid)
        .map(|p| (p.class_id, p.base_class_id))?;
    let t = world
        .data
        .player_templates
        .get_or_base(class_id, base_class_id)
        .cloned()
        .unwrap_or_default();
    let slots = world
        .objects
        .get_component::<components::HennaSlots>(&oid)
        .map(|h| h.0)
        .unwrap_or_default();
    let hs = world.data.hennas.stat_sums(&slots);
    let sets = crate::game_loop::armor_sets::set_stat_sums(world, oid);
    // Java sums the set bonus as a double into the finalizer's base value and
    // the consumer truncates; every `<stat val>` on this dist is a whole
    // number, so the cast is exact rather than lossy.
    Some(BaseStats {
        str_: t.base_str + hs.str_ + sets.str_ as i32,
        dex: t.base_dex + hs.dex + sets.dex as i32,
        con: t.base_con + hs.con + sets.con as i32,
        int_: t.base_int + hs.int_ + sets.int_ as i32,
        wit: t.base_wit + hs.wit + sets.wit as i32,
        men: t.base_men + hs.men + sets.men as i32,
    })
}

/// Java `Creature.getCurrentHpPercent()` — `(int) ((currentHp * 100) / maxHp)`.
///
/// The integer truncation is Java's and is kept: at 30.9 % HP this answers 30,
/// so a `<hpPercent>30</hpPercent>` effect is already up. A max of 0 answers 0
/// rather than dividing by zero, which keeps a not-yet-initialised creature on
/// the "hurt" side — the same side Java's `0/0 = NaN` comparison would fail to.
pub(crate) fn hp_percent_of(cur_hp: f64, max_hp: i32) -> i32 {
    if max_hp <= 0 {
        return 0;
    }
    ((cur_hp * 100.0) / max_hp as f64) as i32
}

/// The passive buffs a player's skill book contributes **right now**, with each
/// effect's own conditions evaluated against the state they name.
///
/// `hp_percent_now` is Java's `effected.getCurrentHpPercent()` —
/// `(int) ((currentHp * 100) / maxHp)` — read by the
/// `AbstractConditionalHpEffect` family. It is a parameter rather than a
/// component read because this runs from `Player::from_char`, before the
/// entity exists.
pub(crate) fn conditioned_passive_buffs(
    data: &GameData,
    skills: &SkillBook,
    inventory: &Inventory,
    hp_percent_now: i32,
) -> Vec<ActiveBuff> {
    use crate::model::skill::{OperateType, SkillEffect};
    let mut out = Vec::new();
    for (&skill_id, &level) in &skills.0 {
        let Some(skill) = data.skill_data.get(skill_id, level) else {
            continue;
        };
        if skill.operate_type != OperateType::Passive {
            continue;
        }
        // Java `checkConditions(PASSIVE, …)` — a passive whose own
        // `<passiveConditions>` don't hold contributes nothing (G34 S1).
        if !crate::game_loop::skills::conditions::passive_stat_gate(
            skill,
            inventory,
            &data.item_data,
        ) {
            continue;
        }
        let applicable: Vec<StatModifierEffect> = skill
            .effects
            .iter()
            .filter_map(|e| match e {
                SkillEffect::StatModifier(m) => Some(*m),
                _ => None,
            })
            .filter(|m| {
                (m.armor_condition == 0 || armor_condition_passes(m.armor_condition, inventory, &data.item_data))
                    && (m.weapon_condition == 0 || weapon_condition_passes(m.weapon_condition, inventory, &data.item_data))
                    // `ConditionUsingSlotType(SLOT_LR_HAND)` — a *separate*
                    // axis from the weapon type: the same blunt bonus is off
                    // while a one-handed mace is equipped.
                    && (!m.two_handed || two_handed_weapon_equipped(inventory, &data.item_data))
                    // `AbstractConditionalHpEffect.canPump`:
                    // `(_hpPercent <= 0) || (effected.getCurrentHpPercent() <= _hpPercent)`.
                    && (m.hp_percent <= 0 || hp_percent_now <= m.hp_percent)
            })
            .collect();
        if applicable.is_empty() {
            continue;
        }
        out.push(ActiveBuff::passive_pump(skill_id, level, applicable));
    }
    out
}

/// Java `CreatureStat.mergeAdd`/`mergeMul`/`mergeMoveTypeValue`/
/// `mergePositionTypeValue` — accumulate one effect's contribution into the
/// modifier maps (multiple buffs on the same stat stack).
///
/// A *qualified* effect goes to its own map instead of `add`/`mul`, exactly as
/// Java routes it: it must not be folded into `add`/`mul`, or it would apply in
/// every state rather than the one it names. Each kind keeps Java's own merge
/// and identity — move type adds into 0.0, position multiplies into 1.0 — so
/// `mode` is not consulted on either path.
pub(crate) fn apply_modifier(mods: &mut StatModifiers, effect: &StatModifierEffect) {
    use crate::model::stats::StatQualifier;
    match effect.qualifier {
        Some(StatQualifier::MoveType(move_type)) => {
            *mods
                .by_move_type
                .entry((effect.stat, move_type))
                .or_insert(0.0) += effect.amount;
            return;
        }
        Some(StatQualifier::Position(position)) => {
            // `mergePositionTypeValue(stat, position, (amount/100)+1, MathUtil::mul)`
            // — the percentage is turned into a multiplier by the *handler*,
            // not the merge, and stacking positions multiply.
            *mods
                .by_position
                .entry((effect.stat, position))
                .or_insert(1.0) *= (effect.amount / 100.0) + 1.0;
            return;
        }
        None => {}
    }
    match effect.mode {
        StatModifierType::Diff => {
            *mods.add.entry(effect.stat).or_insert(0.0) += effect.amount;
        }
        StatModifierType::Per => {
            let entry = mods.mul.entry(effect.stat).or_insert(1.0);
            *entry *= (effect.amount / 100.0) + 1.0;
        }
    }
}

/// Java `Stat.weaponBaseValue` → `IStatFunction.calcWeaponBaseValue`: for a
/// player, the **right-hand weapon's** own declaration of a stat *replaces* the
/// class-template base (a two-handed weapon lives in RHand too). `None`
/// bare-handed, or when the weapon declares nothing for that stat, which is
/// the caller's cue to keep the template value.
///
/// The stat recompute inlines this rule for the five weapon-replace stats it
/// finalizes; this is the same read for callers outside that pass —
/// `calcAtkSpdMultiplier` needs the attack-speed one to scale a physical
/// skill's cast time, and used to take the class base instead.
pub(crate) fn weapon_base_stat(inventory: &Inventory, data: &GameData, stat: Stat) -> Option<f64> {
    let weapon = inventory.paperdoll_item(crate::model::inventory::PaperdollSlot::RHand)?;
    let stats = data.item_data.item_stats(weapon.item_id)?;
    stats
        .bonuses
        .iter()
        .find(|&&(s, _)| s == stat)
        .map(|&(_, v)| v)
}

/// Sum of one `<stat>` across every equipped piece — the flat additive item
/// term the `MaxHp`/`MaxMp` finalizers apply *after* the CON/MEN multiply
/// (Java's `for (Item item : inv.getPaperdollItems()) maxHp += getStats(...)`).
fn equipped_stat_sum(inventory: &Inventory, data: &GameData, stat: Stat) -> f64 {
    inventory
        .equipped_items()
        .iter()
        .filter_map(|item| data.item_data.item_stats(item.item_id))
        .flat_map(|s| s.bonuses.iter())
        .filter(|(st, _)| *st == stat)
        .map(|(_, v)| *v)
        .sum()
}

/// `MaxHpFinalizer`: `mul·(baseHpMax(level)·CON bonus) + add`, plus each
/// equipped item's flat `maxHp` bonus (added *after* the buff `mul`, per Java —
/// items aren't scaled by the buff). `inventory = None` for the pre-equip
/// char-creation preview. The `mul`/`add` come from the buff modifier maps —
/// HP-boosting clan skills / buffs move the stat through here.
pub fn calc_max_hp(
    data: &GameData,
    t: &PlayerTemplate,
    level: i32,
    inventory: Option<&Inventory>,
    mods: &StatModifiers,
) -> f64 {
    let base = t.base_hp_max(level) * data.stat_bonus.con_bonus(t.base_con);
    let mul = mods.mul.get(&Stat::MaxHp).copied().unwrap_or(1.0);
    let add = mods.add.get(&Stat::MaxHp).copied().unwrap_or(0.0);
    let item = inventory
        .map(|inv| equipped_stat_sum(inv, data, Stat::MaxHp))
        .unwrap_or(0.0);
    let enchant = inventory
        .map(|inv| enchanted_armour_hp(inv, data))
        .unwrap_or(0.0);
    let total = mul * base + add + item + enchant;
    // `MaxHpFinalizer`'s HP_LIMIT arm: `min(maxHp, MAX_HP * mul + add)`. No
    // skill on this dist grants `hpLimit`, so the mul/add stay at 1/0 and the
    // ceiling is the flat config figure. Java lifts it outright for a
    // cursed-weapon wielder — Zariche and Akamanah, both Interlude weapons —
    // and for a dragon weapon, which is post-Interlude and unequippable here.
    let cursed = inventory.is_some_and(|inv| {
        inv.equipped_items().iter().any(|it| {
            data.cursed_weapons
                .weapons
                .iter()
                .any(|cw| cw.item_id == it.item_id)
        })
    });
    if cursed {
        total
    } else {
        total.min(data.combat_caps.max_hp)
    }
}

/// `MaxHpFinalizer`'s "Apply enchanted item bonus HP" arm: every equipped
/// **armour** piece that is enchanted adds a flat figure from
/// `enchantHPBonus.xml`, on top of its own `maxHp` stat.
///
/// Java excludes three slots by body part — necklace, earrings and rings —
/// which is why the test is on the slot rather than on "is it a jewel":
/// `ItemKind::Armor` covers jewellery too.
fn enchanted_armour_hp(inventory: &Inventory, data: &GameData) -> f64 {
    use crate::data::item_data::{ItemKind, SLOT_LR_EAR, SLOT_LR_FINGER, SLOT_NECK};
    inventory
        .equipped_items()
        .iter()
        .filter(|item| item.enchant_level > 0)
        .filter_map(|item| data.item_data.get(item.item_id).map(|t| (item, t)))
        .filter(|(_, t)| t.kind == ItemKind::Armor)
        .filter(|(_, t)| !matches!(t.body_part, SLOT_NECK | SLOT_LR_EAR | SLOT_LR_FINGER))
        .map(|(item, t)| {
            data.enchant_hp_bonus
                .bonus(t.crystal_type, item.enchant_level, t.body_part)
        })
        .sum()
}

/// `MaxMpFinalizer`: `mul·(baseMpMax(level)·MEN bonus) + add` + equipped `maxMp`.
pub fn calc_max_mp(
    data: &GameData,
    t: &PlayerTemplate,
    level: i32,
    inventory: Option<&Inventory>,
    mods: &StatModifiers,
) -> f64 {
    let base = t.base_mp_max(level) * data.stat_bonus.men_bonus(t.base_men);
    let mul = mods.mul.get(&Stat::MaxMp).copied().unwrap_or(1.0);
    let add = mods.add.get(&Stat::MaxMp).copied().unwrap_or(0.0);
    let item = inventory
        .map(|inv| equipped_stat_sum(inv, data, Stat::MaxMp))
        .unwrap_or(0.0);
    mul * base + add + item
}

/// `MaxCpFinalizer`: `mul·(baseCpMax(level)·CON bonus) + add`. No item bonus —
/// no item in this dist carries `maxCp`, and Java's `MaxCpFinalizer` has no
/// paperdoll loop.
pub fn calc_max_cp(data: &GameData, t: &PlayerTemplate, level: i32, mods: &StatModifiers) -> f64 {
    let base = t.base_cp_max(level) * data.stat_bonus.con_bonus(t.base_con);
    let mul = mods.mul.get(&Stat::MaxCp).copied().unwrap_or(1.0);
    let add = mods.add.get(&Stat::MaxCp).copied().unwrap_or(0.0);
    mul * base + add
}
