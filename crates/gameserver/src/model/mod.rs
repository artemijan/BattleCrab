//! Port of `gameserver/model` — the game domain. G4 introduces the composed
//! `Player` (challenge #1: composition over inheritance) with just enough state
//! to enter the world and display correctly. Inventory, skills, effects, and the
//! full stat pipeline arrive in later milestones.

pub mod clan;
pub mod components;
pub mod formulas;
pub mod inventory;
pub mod movement;
pub mod npc;
pub mod party;
pub mod quest;
pub mod shortcut;
pub mod skill;
pub mod stats;

use std::collections::HashMap;

use crate::character::CharData;
use crate::data::player_template::PlayerTemplate;
use crate::data::GameData;
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
    pub inventory: Inventory,
    pub skills: SkillBook,
    pub shortcuts: Shortcuts,
    pub macros: Macros,
    pub friends: components::Friends,
    pub quests: components::Quests,
}


impl PlayerData {
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
                    Buffs::default(),
                    StatModifiers::default(),
                    Reuses::default(),
                ),
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
        }
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

        let max_hp = calc_max_hp(data, &t, c.level);
        let max_mp = calc_max_mp(data, &t, c.level);
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
            running: true,
        };
        let collision = Collision { radius: t.collision_radius, height: t.collision_height };
        let p = Player {
            object_id: c.object_id,
            name: c.name.clone(),
            account: c.account_name.clone(),
            title: String::new(),
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
        };
        // Filled in by `recalculate_stats`; atk_range/random_dmg are
        // template constants the finalizers never touch.
        let mut combat = CombatStats { atk_range: t.base_atk_range, random_dmg: 10, ..Default::default() };
        p.recalculate_stats(data, &base_stats, &StatModifiers::default(), &mut speeds, &mut combat);

        // `ShortCuts.restoreMe`'s verification tail: ITEM shortcuts whose
        // object id left the inventory are dropped (the caller fires the
        // `DeleteShortcut` DB command — see `stale_item_shortcuts`), surviving
        // *EtcItem* shortcuts pick up the template's shared reuse group
        // (weapons/armor keep -1 on restore — a Java quirk kept as-is).
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
            inventory: Inventory::from_rows(&c.items),
            skills: SkillBook(c.skills.iter().copied().collect()),
            shortcuts: Shortcuts::from_list(shortcuts),
            macros: Macros::from_list(c.macros.clone()),
            friends: components::Friends(c.friends.clone()),
            quests: components::Quests(c.quests.clone()),
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
    /// `self`, so it's idempotent) × `BaseStat` bonus × level mod, then folds
    /// in `stats_add`/`stats_mul` (buffs). Call after level/buff/gear changes.
    /// TODO(G8+): weapon/armor `<stats>` contributions — item stat bonuses
    /// aren't parsed yet (`data/item_data.rs`), so this is the unarmed/naked
    /// value, same simplification G5 already made for item stats.
    pub fn recalculate_stats(
        &self,
        data: &GameData,
        base: &BaseStats,
        mods: &StatModifiers,
        speeds: &mut Speeds,
        combat: &mut CombatStats,
    ) {
        let t = data
            .player_templates
            .get(self.class_id)
            .or_else(|| data.player_templates.get(self.base_class_id))
            .cloned()
            .unwrap_or_default();
        let level_mod = (self.level as f64 + 89.0) / 100.0;
        let sb = &data.stat_bonus;
        let str_bonus = sb.bonus(BaseStat::Str, base.str_);
        let dex_bonus = sb.bonus(BaseStat::Dex, base.dex);
        let int_bonus = sb.bonus(BaseStat::Int, base.int_);
        let wit_bonus = sb.bonus(BaseStat::Wit, base.wit);

        // PAttackFinalizer / MAttackFinalizer.
        combat.p_atk = finalize(mods, Stat::PhysicalAttack, t.base_p_atk as f64 * str_bonus * level_mod)
            .round()
            .clamp(0.0, MAX_PATK);
        combat.m_atk = finalize(mods, Stat::MagicalAttack, t.base_m_atk as f64 * (int_bonus * level_mod).powf(2.2072))
            .round()
            .clamp(0.0, MAX_MATK);

        // P/MDefenseFinalizer, naked value only (see TODO above).
        combat.p_def = finalize(mods, Stat::PhysicalDefence, t.base_p_def as f64).round().max(0.0);
        combat.m_def = finalize(mods, Stat::MagicalDefence, t.base_m_def as f64).round().max(0.0);

        // P/MAttackSpeedFinalizer: `mul` floors at 0.7, not the usual 1.0.
        combat.p_atk_spd = finalize_speed(mods, Stat::PhysicalAttackSpeed, t.base_p_atk_spd as f64 * dex_bonus)
            .round()
            .clamp(1.0, MAX_PATK_SPEED) as i32;
        combat.m_atk_spd = finalize_speed(mods, Stat::MagicAttackSpeed, t.base_m_atk_spd as f64 * wit_bonus)
            .round()
            .clamp(1.0, MAX_MATK_SPEED) as i32;

        // P/MCritRateFinalizer (in per-mille, ×10).
        combat.crit_hit = finalize(mods, Stat::CriticalRate, t.base_crit_rate as f64 * dex_bonus * 10.0)
            .round()
            .clamp(0.0, MAX_PCRIT_RATE);
        combat.m_crit_hit = finalize(mods, Stat::MagicCriticalRate, t.base_m_crit_rate as f64 * wit_bonus * 10.0)
            .round()
            .clamp(0.0, MAX_MCRIT_RATE);

        // P/MAccuracyFinalizer, P/MEvasionRateFinalizer (high-level +N steps
        // above level 69 skipped — base classes here don't reach that high).
        let level = self.level as f64;
        combat.accuracy = finalize(mods, Stat::AccuracyCombat, (base.dex as f64).sqrt() * 5.0 + level)
            .round() as i32;
        combat.magic_accuracy = finalize(mods, Stat::AccuracyMagic, (base.wit as f64).sqrt() * 3.0 + level * 2.0)
            .round() as i32;
        combat.evasion = finalize(mods, Stat::EvasionRate, (base.dex as f64).sqrt() * 5.0 + level)
            .round()
            .clamp(0.0, MAX_EVASION) as i32;
        combat.magic_evasion = finalize(mods, Stat::MagicEvasionRate, (base.wit as f64).sqrt() * 3.0 + level * 2.0)
            .round() as i32;

        // Speed: base template value, buffs (Speed effect) apply through the
        // add/mul maps exactly like the combat stats above. Rounded like the
        // old i32 fields, stored as f64 (Speeds is shared with NPCs).
        speeds.run_spd = finalize(mods, Stat::RunSpeed, t.base_run_spd as f64).round();
        speeds.walk_spd = finalize(mods, Stat::WalkSpeed, t.base_walk_spd as f64).round();
        speeds.swim_run_spd = finalize(mods, Stat::SwimRunSpeed, t.base_swim_run_spd as f64).round();
        speeds.swim_walk_spd = finalize(mods, Stat::SwimWalkSpeed, t.base_swim_walk_spd as f64).round();
    }

    /// Fold a landed buff's effects into the modifier maps and recompute.
    /// Java `BuffInfo.initializeEffects` → `AbstractEffect.pump`.
    /// Java `BuffInfo.initializeEffects` → `AbstractEffect.pump`.
    pub fn apply_buff(
        &self,
        data: &GameData,
        base: &BaseStats,
        mods: &mut StatModifiers,
        buffs: &mut Buffs,
        speeds: &mut Speeds,
        combat: &mut CombatStats,
        buff: ActiveBuff,
    ) {
        for effect in &buff.effects {
            apply_modifier(&mut mods.add, &mut mods.mul, effect);
        }
        buffs.0.push(buff);
        self.recalculate_stats(data, base, mods, speeds, combat);
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
        self.recalculate_stats(data, base, mods, speeds, combat);
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

/// `Character.ini` stat-cap defaults (`MaxPAtk`/`MaxPCritRate`/…). These are
/// effectively always left at their defaults in practice; TODO: thread real
/// `CharacterConfig` values through `World`/`GameData` if a deployment ever
/// needs to override them (no subsystem currently plumbs that far).
const MAX_PATK: f64 = 999_999.0;
const MAX_MATK: f64 = 999_999.0;
const MAX_PCRIT_RATE: f64 = 500.0;
const MAX_MCRIT_RATE: f64 = 200.0;
const MAX_PATK_SPEED: f64 = 1500.0;
const MAX_MATK_SPEED: f64 = 1999.0;
const MAX_EVASION: f64 = 250.0;

/// `Stat.defaultValue`: `base * mul + add` from the accumulated modifier
/// maps (1.0/0.0 when nothing has touched this stat).
fn finalize(mods: &StatModifiers, stat: Stat, base: f64) -> f64 {
    let mul = mods.mul.get(&stat).copied().unwrap_or(1.0);
    let add = mods.add.get(&stat).copied().unwrap_or(0.0);
    base * mul + add
}

/// `P/MAttackSpeedFinalizer.defaultValue`: same shape, but `mul` floors at
/// 0.7 instead of applying whatever's in the map directly (so an absent or
/// tiny buff doesn't produce a slower-than-0.7x cast/attack speed).
fn finalize_speed(mods: &StatModifiers, stat: Stat, base: f64) -> f64 {
    let mul = mods.mul.get(&stat).copied().unwrap_or(1.0).max(0.7);
    let add = mods.add.get(&stat).copied().unwrap_or(0.0);
    base * mul + add
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

/// `MaxHpFinalizer`: `baseHpMax(level) * CON bonus`.
/// TODO(G7): the multiplicative/additive item & buff modifiers (`mul`/`add`).
pub fn calc_max_hp(data: &GameData, t: &PlayerTemplate, level: i32) -> f64 {
    t.base_hp_max(level) * data.stat_bonus.con_bonus(t.base_con)
}

/// `MaxMpFinalizer`: `baseMpMax(level) * MEN bonus`.
pub fn calc_max_mp(data: &GameData, t: &PlayerTemplate, level: i32) -> f64 {
    t.base_mp_max(level) * data.stat_bonus.men_bonus(t.base_men)
}

/// `MaxCpFinalizer`: `baseCpMax(level) * CON bonus`.
pub fn calc_max_cp(data: &GameData, t: &PlayerTemplate, level: i32) -> f64 {
    t.base_cp_max(level) * data.stat_bonus.con_bonus(t.base_con)
}
