//! Port of `gameserver/model` — the game domain. G4 introduces the composed
//! `Player` (challenge #1: composition over inheritance) with just enough state
//! to enter the world and display correctly. Inventory, skills, effects, and the
//! full stat pipeline arrive in later milestones.

pub mod clan;
pub mod components;
pub mod door;
pub mod formulas;
pub mod inventory;
pub mod mob_group;
pub mod movement;
pub mod npc;
pub mod party;
pub mod quest;
pub mod shortcut;
pub mod skill;
pub mod static_object;
pub mod stats;

use std::collections::HashMap;

use crate::character::CharData;
use crate::data::admin_data::AccessLevel;
use crate::data::player_template::PlayerTemplate;
use crate::data::GameData;

/// Client-default name/title colors for a normal (level-0) player, matching a
/// real UserInfo capture. See [`Player::name_color`].
pub const DEFAULT_NAME_COLOR: i32 = 0x00FF_FFFF;
pub const DEFAULT_TITLE_COLOR: i32 = 0x00FF_FF77;
use components::{AttackState, BaseStats, Buffs, ClientPos, Collision, CombatStats, Macros, PlayerVitals, Position, RegionCell, Reuses, Shortcuts, SkillBook, Speeds, StatModifiers, TargetRef, Vitals};
use inventory::Inventory;
use skill::{ActiveBuff, StatModifierEffect};
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
}

/// The player's current AI intention beyond standing/moving (Java
/// `CtrlIntention` narrowed to what exists). `Attack` keeps auto-attacking
/// (and walking into range of) the target until it dies, the player cancels
/// (Esc / move click), or the player dies — `PlayerAI.thinkAttack`'s loop.
/// `Cast` walks into cast range of the snapshotted target and then casts —
/// `PlayerAI.thinkCast` → `maybeMoveToPawn`. `Interact` walks into an NPC's
/// talk range and then re-runs the interact click — `PlayerAI.thinkInteract`
/// → `maybeMoveToPawn` → `Player.doInteract` re-dispatching `onAction`. All
/// three are driven from the combat tick system and cleared by the same
/// cancel paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerIntent {
    Attack { target_object_id: i32 },
    Cast { skill_id: i32, ctrl: bool, shift: bool, target_object_id: i32 },
    Interact { target_object_id: i32 },
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
    /// GMHeroAura)`, resolved at load (Java `CharInfo`/`UserInfo`). `isHero()`
    /// is always `false` for now — TODO(Olympiad): real hero status.
    pub hero_aura: bool,

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
    pub vitality_points: i32,
    /// `characters.pccafe_points` — PC-cafe loyalty points (`//pccafepoints`).
    pub pccafe_points: i32,
    /// Account-scoped NCoin balance (`account_gsdata` "PRIME_POINTS",
    /// `//primepoints`). Mirror of the account var, loaded at enter-world.
    pub prime_points: i32,
    pub fame: i32,

    // Clan membership (G11 — creation/display slice). The `Clan` itself
    // lives in `World.clans`; these are the per-player fields the
    // UserInfo/CharInfo builders write. `clan_leader` is fixed up at
    // enter-world from the live table (and by `create_clan`).
    pub clan_id: i32,
    pub clan_privs: i32,
    pub clan_leader: bool,
    /// `characters.clan_create_expiry_time` — the 10-day recreate cooldown.
    pub clan_create_expiry_time: i64,

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
    /// Java `Creature._isTeleporting`: position pushed server-side, waiting
    /// for the client's `Appearing`.
    pub teleporting: bool,
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
}

/// Port of `enums/ShotType`, narrowed to the kinds this slice charges. The
/// mask is `1 << ordinal`, matching Java's `ShotType._mask` so a single `u8`
/// on [`Player::charged_shots`] mirrors `Creature._chargedShots`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotType {
    Soulshots = 0,
    Spiritshots = 1,
    BlessedSpiritshots = 3,
}

impl ShotType {
    /// `ShotType.getMask()` (`1 << ordinal`).
    pub fn mask(self) -> u8 {
        1 << (self as u8)
    }
}

impl Player {
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
    pub shortcuts: Shortcuts,
    pub macros: Macros,
    pub friends: components::Friends,
    pub quests: components::Quests,
    /// Live skill-reuse cooldowns. Empty out of `from_char`; the real select
    /// path fills it from the DB via [`PlayerData::restore_reuses`] (buffs are
    /// not restored — a later milestone).
    pub reuses: Reuses,
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
                    until_tick: now_tick + (remaining_ms as u64).div_ceil(100),
                    total_ms: r.reuse_delay,
                },
            );
        }
    }

    /// Spawn into the world registry (Java `World.addObject` at EnterWorld).
    pub fn spawn_into(self, objects: &mut crate::store::EntityStore) {
        objects.spawn(
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
                (self.warehouse, self.freight),
            ),
        );
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
        })
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
#[derive(Default)]
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
}

impl EquippedBonuses {
    fn from_inventory(inventory: &Inventory, data: &GameData, t: &crate::data::player_template::PlayerTemplate) -> Self {
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

        // Weapon-replace stats come from the right-hand slot only (Java
        // `calcWeaponBaseValue`); a two-handed weapon also lives in RHand.
        if let Some(weapon) = inventory.paperdoll_item(PaperdollSlot::RHand) {
            if let Some(stats) = data.item_data.item_stats(weapon.item_id) {
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
        }

        // Sum-add stats are summed across every equipped piece (Java's
        // finalizer paperdoll loop / `calcWeaponPlusBaseValue`). `accCombat`
        // lives on weapons too, so this deliberately includes the weapon.
        for item in inventory.equipped_items() {
            let Some(stats) = data.item_data.item_stats(item.item_id) else { continue };
            for &(stat, val) in &stats.bonuses {
                match stat {
                    Stat::PhysicalDefence => eq.p_def += val,
                    Stat::MagicalDefence => eq.m_def += val,
                    Stat::AccuracyCombat => eq.accuracy += val,
                    Stat::AccuracyMagic => eq.magic_accuracy += val,
                    Stat::EvasionRate => eq.evasion += val,
                    Stat::MagicEvasionRate => eq.magic_evasion += val,
                    // maxHp/maxMp item bonuses aren't folded in yet — max HP/MP
                    // are computed by `calc_max_hp`/`calc_max_mp`, a separate
                    // path from these finalizers. TODO(G14): apply there.
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
    /// correctly; current HP/MP come from the row, clamped to the max.
    pub fn from_char(data: &GameData, c: &CharData) -> PlayerData {
        // The active class's template (base classes only in G4).
        let t = data
            .player_templates
            .get(c.class_id)
            .or_else(|| data.player_templates.get(c.base_class_id))
            .cloned()
            .unwrap_or_default();

        // Split stored items by location: warehouse / freight rows go to their
        // own containers, everything else (inventory + paperdoll) to inventory.
        let (wh_rows, rest): (Vec<_>, Vec<_>) = c.items.iter().cloned().partition(|r| r.loc == "WAREHOUSE");
        let (freight_rows, inv_rows): (Vec<_>, Vec<_>) = rest.into_iter().partition(|r| r.loc == "FREIGHT");
        let warehouse = inventory::Warehouse::from_rows(&wh_rows);
        let freight = inventory::Freight::from_rows(&freight_rows);

        // Built early so equipped gear feeds every finalizer below — max HP/MP
        // (item +MP jewelry) as well as the combat recompute further down.
        let inventory = Inventory::from_rows(&inv_rows);
        let max_hp = calc_max_hp(data, &t, c.level, Some(&inventory));
        let max_mp = calc_max_mp(data, &t, c.level, Some(&inventory));
        let max_cp = calc_max_cp(data, &t, c.level);

        let base_stats = BaseStats {
            str_: t.base_str,
            dex: t.base_dex,
            con: t.base_con,
            int_: t.base_int,
            wit: t.base_wit,
            men: t.base_men,
        };
        let vitals = Vitals {
            max_hp: max_hp as i32,
            cur_hp: c.cur_hp.min(max_hp),
            max_mp: max_mp as i32,
            cur_mp: c.cur_mp.min(max_mp),
            dead: c.cur_hp < 0.5,
        };
        let player_vitals = PlayerVitals { max_cp: max_cp as i32, cur_cp: 0.0 };
        let mut speeds = Speeds {
            run_spd: t.base_run_spd as f64,
            walk_spd: t.base_walk_spd as f64,
            swim_run_spd: t.base_swim_run_spd as f64,
            swim_walk_spd: t.base_swim_walk_spd as f64,
            move_multiplier: 1.0,
            base_run_spd: t.base_run_spd as f64,
            running: true,
            swimming: false,
        };
        let collision = Collision { radius: t.collision_radius, height: t.collision_height };
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
        // GM_HERO_AURA)`. `isHero()` is unported (TODO: Olympiad), so this is
        // purely the GM-aura branch for now.
        let hero_aura = access.is_gm && data.gm.hero_aura;
        let p = Player {
            object_id: c.object_id,
            name: c.name.clone(),
            account: c.account_name.clone(),
            title: String::new(),
            access_level: c.access_level,
            name_color,
            title_color,
            hero_aura,
            level: c.level,
            class_id: c.class_id,
            base_class_id: c.base_class_id,
            race: c.race,
            is_female: c.sex != 0,
            exp: c.exp,
            sp: c.sp,
            reputation: c.reputation,
            pk_kills: c.pk_kills,
            pvp_kills: c.pvp_kills,
            vitality_points: c.vitality_points,
            pccafe_points: c.pccafe_points,
            prime_points: c.prime_points,
            fame: 0,
            clan_id: c.clan_id,
            clan_privs: c.clan_privs,
            clan_leader: false, // fixed up at enter-world from World.clans
            clan_create_expiry_time: c.clan_create_expiry_time,
            face: c.face,
            hair_style: c.hair_style,
            hair_color: c.hair_color,
            cast_seq: 0,
            pending_revive: false,
            teleporting: false,
            quest_zone_id: -1,
            charged_shots: 0,
            auto_shots: Vec::new(),
            mount_type: 0,
            mount_npc_id: 0,
            transform_id: 0,
            transform_display_id: 0,
            store_type: 0,
        };
        // Filled in by `recalculate_stats` (incl. atk_range/random_dmg, which it
        // sets from the equipped weapon or the class template).
        let mut combat = CombatStats::default();
        let mut mods = StatModifiers::default();
        let mut buffs = Buffs::default();
        p.recalculate_stats(data, &base_stats, &mods, &inventory, &mut speeds, &mut combat);
        // Java `restoreCharData` → `addSkill`: fold the character's known
        // armor-conditioned passives (Spellcraft/Magician's Movement) into the
        // stat maps now, so the enter-world `UserInfo` burst already carries them
        // (no separate post-spawn resend). Timed buffs aren't restored yet.
        let skills = SkillBook(c.skills.iter().copied().collect());
        for buff in conditioned_passive_buffs(data, &skills, &inventory) {
            p.apply_buff(data, &base_stats, &mut mods, &inventory, &mut buffs, &mut speeds, &mut combat, buff);
        }

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
                    let is_etc = c.items.iter().find(|i| i.object_id == sc.id).and_then(|i| data.item_data.get(i.item_id)).is_some_and(|t| t.kind == crate::data::item_data::ItemKind::Etc);
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
            position: Position { x: c.x, y: c.y, z: c.z, heading: 0 },
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
            shortcuts: Shortcuts::from_list(shortcuts),
            macros: Macros::from_list(c.macros.clone()),
            friends: components::Friends(c.friends.clone()),
            quests: components::Quests(c.quests.clone()),
            // Filled by the select path via `restore_reuses` (needs the game
            // tick); empty here keeps the many test callers unchanged.
            reuses: Reuses::default(),
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
    /// *summed* on top. maxHp/maxMp gear bonuses are still TODO (computed on a
    /// separate path — see `calc_max_hp`).
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
            .get(self.class_id)
            .or_else(|| data.player_templates.get(self.base_class_id))
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

        // PAttackFinalizer / MAttackFinalizer: the equipped weapon's pAtk/mAtk
        // replaces the naked base (`calcWeaponBaseValue`) before STR/level.
        let p_atk_base = eq.weapon_p_atk.unwrap_or(t.base_p_atk as f64);
        let m_atk_base = eq.weapon_m_atk.unwrap_or(t.base_m_atk as f64);
        let caps = &data.combat_caps;
        // Every max cap below goes through Java's `validateValue`, which skips
        // the ceiling for creatures with the MAX_STATS_VALUE cond override —
        // granted to GMs on login (Player.restore). Floors still apply.
        let cap = |max: f64| if self.is_gm(data) { f64::MAX } else { max };
        combat.p_atk = finalize(mods, Stat::PhysicalAttack, p_atk_base * str_bonus * level_mod)
            .clamp(0.0, cap(caps.max_p_atk));
        combat.m_atk = finalize(mods, Stat::MagicalAttack, m_atk_base * (int_bonus * level_mod).powf(2.2072))
            .clamp(0.0, cap(caps.max_m_atk));

        // P/MDefenseFinalizer: (naked base + summed gear def − the naked defense
        // of every occupied slot) × levelMod (mDef also × MEN bonus), then the
        // `defaultValue` mul(≥0.5)/add and the `base × 0.2` floor.
        let p_def_pre = (t.base_p_def as f64 + eq.p_def - eq.p_def_slot_sub) * level_mod;
        combat.p_def = finalize_def(mods, Stat::PhysicalDefence, p_def_pre, t.base_p_def as f64 * 0.2);
        let men_bonus = if base.men > 0 { sb.bonus(BaseStat::Men, base.men) } else { 1.0 };
        let m_def_pre = (t.base_m_def as f64 + eq.m_def - eq.m_def_slot_sub) * men_bonus * level_mod;
        combat.m_def = finalize_def(mods, Stat::MagicalDefence, m_def_pre, t.base_m_def as f64 * 0.2);

        // P/MAttackSpeedFinalizer: weapon replaces base; `mul` floors at 0.7.
        let p_atk_spd_base = eq.weapon_p_atk_spd.unwrap_or(t.base_p_atk_spd as f64);
        combat.p_atk_spd = finalize_speed(mods, Stat::PhysicalAttackSpeed, p_atk_spd_base * dex_bonus)
            .clamp(1.0, cap(caps.max_p_atk_speed)) as i32;
        combat.m_atk_spd = finalize_speed(mods, Stat::MagicAttackSpeed, t.base_m_atk_spd as f64 * wit_bonus)
            .clamp(1.0, cap(caps.max_m_atk_speed)) as i32;

        // P/MCritRateFinalizer (in per-mille, ×10): weapon replaces base crit.
        let crit_base = eq.weapon_crit.unwrap_or(t.base_crit_rate as f64);
        let m_crit_base = eq.weapon_m_crit.unwrap_or(t.base_m_crit_rate as f64);
        combat.crit_hit =
            finalize(mods, Stat::CriticalRate, crit_base * dex_bonus * 10.0).clamp(0.0, cap(caps.max_p_crit_rate));
        combat.m_crit_hit =
            finalize(mods, Stat::MagicCriticalRate, m_crit_base * wit_bonus * 10.0).clamp(0.0, cap(caps.max_m_crit_rate));

        // P/MAccuracyFinalizer, P/MEvasionRateFinalizer (high-level +N steps
        // above level 69 skipped — base classes here don't reach that high).
        // Gear accuracy/evasion sums add on top (`calcWeaponPlusBaseValue`).
        // `as i32` truncates toward zero, matching Java's `(int)` display getter.
        let level = self.level as f64;
        combat.accuracy = finalize(mods, Stat::AccuracyCombat, (base.dex as f64).sqrt() * 5.0 + level + eq.accuracy) as i32;
        combat.magic_accuracy = finalize(mods, Stat::AccuracyMagic, (base.wit as f64).sqrt() * 3.0 + level * 2.0 + eq.magic_accuracy) as i32;
        combat.evasion = finalize(mods, Stat::EvasionRate, (base.dex as f64).sqrt() * 5.0 + level + eq.evasion)
            .clamp(0.0, cap(caps.max_evasion)) as i32;
        combat.magic_evasion = finalize(mods, Stat::MagicEvasionRate, (base.wit as f64).sqrt() * 3.0 + level * 2.0 + eq.magic_evasion) as i32;

        // Weapon range / damage spread replace the class template constants
        // while a weapon is equipped (`PRangeFinalizer` / `RandomDamageFinalizer`).
        combat.atk_range = eq.weapon_atk_range.unwrap_or(t.base_atk_range);
        combat.random_dmg = eq.weapon_random_dmg.unwrap_or(10);

        // SpeedFinalizer: every player speed stat gets `Config.RUN_SPD_BOOST`
        // added in `getBaseSpeed` (35 on this dist — see `CombatCaps`).
        // Buffs (Speed effect) apply through the add/mul maps like the combat
        // stats above; stored as f64 (Speeds is shared with NPCs, whose
        // templates don't take the player boost). The `as i16` in `user_info`
        // truncates for display, matching Java's `(int)` getter.
        speeds.run_spd = finalize(mods, Stat::RunSpeed, t.base_run_spd as f64 + caps.run_spd_boost);
        speeds.walk_spd = finalize(mods, Stat::WalkSpeed, t.base_walk_spd as f64 + caps.run_spd_boost);
        speeds.swim_run_spd = finalize(mods, Stat::SwimRunSpeed, t.base_swim_run_spd as f64 + caps.run_spd_boost);
        speeds.swim_walk_spd = finalize(mods, Stat::SwimWalkSpeed, t.base_swim_walk_spd as f64 + caps.run_spd_boost);

        // A transform replaces the class base run/walk with the template's
        // `<moving>` values (Java's transform move-speed override), still folding
        // the buff modifiers on top. Absolute template speeds — the class
        // `RUN_SPD_BOOST` is not re-added (the transform values are self-tuned).
        if self.transform_id != 0 {
            if let Some(tf) = data.transforms.get(self.transform_id) {
                let tmpl = tf.template(self.is_female);
                if let Some(run) = tmpl.run_spd {
                    speeds.run_spd = finalize(mods, Stat::RunSpeed, run);
                }
                if let Some(walk) = tmpl.walk_spd {
                    speeds.walk_spd = finalize(mods, Stat::WalkSpeed, walk);
                }
            }
        }

        // SpeedFinalizer's `validateValue`: players clamp to [1, MaxRunSpeed]
        // (300 on this dist).
        let speed_cap = cap(caps.max_run_speed);
        for spd in [&mut speeds.run_spd, &mut speeds.walk_spd, &mut speeds.swim_run_spd, &mut speeds.swim_walk_spd] {
            *spd = spd.clamp(1.0, speed_cap);
        }
    }

    /// Fold a landed buff's effects into the modifier maps and recompute.
    /// Java `BuffInfo.initializeEffects` → `AbstractEffect.pump`.
    /// Java `BuffInfo.initializeEffects` → `AbstractEffect.pump`.
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
    ) {
        for effect in &buff.effects {
            apply_modifier(&mut mods.add, &mut mods.mul, effect);
        }
        buffs.0.push(buff);
        self.recalculate_stats(data, base, mods, inventory, speeds, combat);
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
        for buff in &buffs.0 {
            for effect in &buff.effects {
                apply_modifier(&mut mods.add, &mut mods.mul, effect);
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
/// maps (1.0/0.0 when nothing has touched this stat).
fn finalize(mods: &StatModifiers, stat: Stat, base: f64) -> f64 {
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

/// Rebuild an NPC's `CombatStats`/`Speeds` from its template plus its active
/// buffs (the NPC counterpart of `Player::recalculate_stats` +
/// `apply_buff`/`remove_buff`). NPCs have no gear or base-stat components, so
/// the template-derived values (`npc_combat_stats`) are the finalizer bases;
/// each active buff's modifiers fold in through the same add/mul maps and
/// `finalize*` the player uses. Called on every buff apply/expire, so it starts
/// from a clean base each time and can't drift.
pub(crate) fn recompute_npc_stats_from_buffs(
    t: &crate::data::npc_data::NpcTemplate,
    sb: &crate::data::stat_bonus::StatBonus,
    caps: &crate::data::CombatCaps,
    buffs: &Buffs,
    combat: &mut CombatStats,
    speeds: &mut Speeds,
) {
    let base = npc::npc_combat_stats(t, sb);
    let mut mods = StatModifiers::default();
    for buff in &buffs.0 {
        for effect in &buff.effects {
            apply_modifier(&mut mods.add, &mut mods.mul, effect);
        }
    }
    combat.p_atk = finalize(&mods, Stat::PhysicalAttack, base.p_atk).clamp(0.0, caps.max_p_atk);
    combat.m_atk = finalize(&mods, Stat::MagicalAttack, base.m_atk).clamp(0.0, caps.max_m_atk);
    // NPCs carry no naked-base/gear split, so the defense floor is a fifth of
    // the template value (mirrors the player's `base × 0.2`).
    combat.p_def = finalize_def(&mods, Stat::PhysicalDefence, base.p_def, base.p_def * 0.2);
    combat.m_def = finalize_def(&mods, Stat::MagicalDefence, base.m_def, base.m_def * 0.2);
    combat.p_atk_spd =
        finalize_speed(&mods, Stat::PhysicalAttackSpeed, base.p_atk_spd as f64).clamp(1.0, caps.max_p_atk_speed) as i32;
    combat.m_atk_spd =
        finalize_speed(&mods, Stat::MagicAttackSpeed, base.m_atk_spd as f64).clamp(1.0, caps.max_m_atk_speed) as i32;
    combat.crit_hit = finalize(&mods, Stat::CriticalRate, base.crit_hit).clamp(0.0, caps.max_p_crit_rate);
    combat.accuracy = finalize(&mods, Stat::AccuracyCombat, base.accuracy as f64) as i32;
    combat.evasion = finalize(&mods, Stat::EvasionRate, base.evasion as f64).clamp(0.0, caps.max_evasion) as i32;
    // Range / random-damage aren't buffable here — keep the template values.
    combat.atk_range = base.atk_range;
    combat.random_dmg = base.random_dmg;
    // Speeds: no `RUN_SPD_BOOST` for NPCs (that's a player-only base add).
    speeds.run_spd = finalize(&mods, Stat::RunSpeed, t.base_run_spd);
    speeds.walk_spd = finalize(&mods, Stat::WalkSpeed, t.base_walk_spd);
}

/// Port of `ConditionUsingItemType.testImpl`'s armor branch (the only branch a
/// robe passive's `<armorType>` mask reaches): the condition passes when the
/// worn chest — and, unless the chest is full-armor, the worn legs — matches the
/// mask, treating a bare slot as `ArmorType::NONE`.
pub(crate) fn armor_condition_passes(mask: u8, inventory: &Inventory, items: &crate::data::item_data::ItemData) -> bool {
    use crate::data::item_data::{ArmorType, SLOT_FULL_ARMOR};
    use crate::model::inventory::PaperdollSlot;
    const NONE_BIT: u8 = ArmorType::None.mask_bit();
    let Some(chest) = inventory.paperdoll_item(PaperdollSlot::Chest) else {
        return mask & NONE_BIT != 0;
    };
    if mask & items.armor_type(chest.item_id).mask_bit() == 0 {
        return false;
    }
    if items.get(chest.item_id).map(|t| t.body_part == SLOT_FULL_ARMOR).unwrap_or(false) {
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
pub(crate) fn weapon_condition_passes(mask: u32, inventory: &Inventory, items: &crate::data::item_data::ItemData) -> bool {
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
pub(crate) fn conditioned_passive_buffs(data: &GameData, skills: &SkillBook, inventory: &Inventory) -> Vec<ActiveBuff> {
    use crate::model::skill::{OperateType, SkillEffect};
    let mut out = Vec::new();
    for (&skill_id, &level) in &skills.0 {
        let Some(skill) = data.skill_data.get(skill_id, level) else { continue };
        if skill.operate_type != OperateType::Passive {
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
            })
            .collect();
        if applicable.is_empty() {
            continue;
        }
        out.push(ActiveBuff {
            skill_id,
            skill_level: level,
            abnormal_type_client_id: -1,
            expires_at_tick: u64::MAX,
            passive: true,
            effects: applicable,
        });
    }
    out
}

/// Java `CreatureStat.mergeAdd`/`mergeMul` — accumulate one effect's
/// contribution into the add/mul maps (multiple buffs on the same stat stack).
fn apply_modifier(add: &mut HashMap<Stat, f64>, mul: &mut HashMap<Stat, f64>, effect: &StatModifierEffect) {
    match effect.mode {
        StatModifierType::Diff => {
            *add.entry(effect.stat).or_insert(0.0) += effect.amount;
        }
        StatModifierType::Per => {
            let entry = mul.entry(effect.stat).or_insert(1.0);
            *entry *= (effect.amount / 100.0) + 1.0;
        }
    }
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

/// `MaxHpFinalizer`: `baseHpMax(level) * CON bonus`, plus each equipped item's
/// flat `maxHp` bonus (`inventory = None` for the pre-equip char-creation
/// preview). TODO(G7): the multiplicative/additive *buff* modifiers (`mul`/`add`).
pub fn calc_max_hp(data: &GameData, t: &PlayerTemplate, level: i32, inventory: Option<&Inventory>) -> f64 {
    let base = t.base_hp_max(level) * data.stat_bonus.con_bonus(t.base_con);
    base + inventory.map(|inv| equipped_stat_sum(inv, data, Stat::MaxHp)).unwrap_or(0.0)
}

/// `MaxMpFinalizer`: `baseMpMax(level) * MEN bonus` + equipped `maxMp` bonuses.
pub fn calc_max_mp(data: &GameData, t: &PlayerTemplate, level: i32, inventory: Option<&Inventory>) -> f64 {
    let base = t.base_mp_max(level) * data.stat_bonus.men_bonus(t.base_men);
    base + inventory.map(|inv| equipped_stat_sum(inv, data, Stat::MaxMp)).unwrap_or(0.0)
}

/// `MaxCpFinalizer`: `baseCpMax(level) * CON bonus`. No item bonus — no item in
/// this dist carries `maxCp`, and Java's `MaxCpFinalizer` has no paperdoll loop.
pub fn calc_max_cp(data: &GameData, t: &PlayerTemplate, level: i32) -> f64 {
    t.base_cp_max(level) * data.stat_bonus.con_bonus(t.base_con)
}
