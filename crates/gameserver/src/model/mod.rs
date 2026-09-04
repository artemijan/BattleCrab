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

use crate::data::GameData;
use crate::data::admin_data::AccessLevel;

/// Client-default name/title colors for a normal (level-0) player, matching a
/// real UserInfo capture. See [`Player::name_color`].
pub const DEFAULT_NAME_COLOR: i32 = 0x00FF_FFFF;
pub const DEFAULT_TITLE_COLOR: i32 = 0x00FF_FF77;

/// `PlayerStat.MAX_VITALITY_POINTS` / `MIN_VITALITY_POINTS` — the bounds every
/// vitality read and write clamps to. Lives here (rather than in
/// `game_loop::character::vitality`) because both the config loader and the stat code need
/// them.
pub const MAX_VITALITY_POINTS: i32 = 140_000;
pub const MIN_VITALITY_POINTS: i32 = 0;
use components::player::{Macros, Shortcuts};
use components::skills::{Buffs, Reuses, SkillBook};
use components::space::{Collision, Position, RegionCell};
use components::stats::{BaseStats, CombatStats, PlayerVitals, Speeds, StatModifiers, Vitals};
use inventory::Inventory;

pub mod equip_conditions;
pub mod max_vitals;
pub mod npc_stats;
pub mod player_buffs;
pub mod player_load;
pub mod player_stats;
pub mod stat_finalize;

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
    /// exp/sp multiplier (see `game_loop::character::vitality`).
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
    /// (`game_loop::character::pc_cafe`). Re-stamped by every `run`, so an earlier
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
    pub skill_enchants: components::skills::SkillEnchants,
    /// Worn henna dyes (`character_hennas`); their stat bonus is already folded
    /// into `base_stats`.
    pub henna: components::skills::HennaSlots,
    /// Registered crafting recipes (`character_recipebook`), split by book.
    pub recipe_book: components::commerce::RecipeBook,
    /// `character_variables` key/value store (Java `PlayerVariables`).
    pub variables: components::player::PlayerVariables,
    /// Saved pet rows, keyed by collar object id (Java's `pets` table).
    pub pets: components::summons::PlayerPets,
    /// The servitor that was out at logout (`character_summons`).
    pub summons: components::summons::PlayerSummons,
    /// Items held by the player's pet (Java `PetInventory`, `loc="PET"`).
    pub pet_inventory: inventory::PetInventory,
    pub shortcuts: Shortcuts,
    pub macros: Macros,
    pub friends: components::social::Friends,
    pub quests: components::social::Quests,
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
    /// login path ([`game_loop::client::lobby`](crate::game_loop::client::lobby)) reports it —
    /// the same split `pending_buffs` above uses for the same reason.
    ///
    /// Whether the skills were also *removed* from [`Self::skills`] depends on
    /// `SkillCheckRemove`; this list is populated either way, because the audit
    /// half of the feature is the half that works with removal off.
    pub illegal_skills: Vec<(i32, i32)>,
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
                .get_component::<components::combat::PvpState>(&object_id)
                .map_or(0, |s| s.flag),
            in_matching_room: objects
                .has_component::<components::social::InMatchingRoom>(&object_id),
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
        v.in_water = crate::game_loop::space::position::is_in_water(world, object_id);
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

impl Player {
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
