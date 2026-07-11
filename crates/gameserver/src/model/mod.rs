//! Port of `gameserver/model` — the game domain. G4 introduces the composed
//! `Player` (challenge #1: composition over inheritance) with just enough state
//! to enter the world and display correctly. Inventory, skills, effects, and the
//! full stat pipeline arrive in later milestones.

pub mod formulas;
pub mod inventory;
pub mod movement;
pub mod npc;
pub mod skill;
pub mod stats;

use std::collections::HashMap;

use crate::character::CharData;
use crate::data::player_template::PlayerTemplate;
use crate::data::GameData;
use inventory::Inventory;
use movement::MoveData;
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
/// (Esc / move click), or the player dies — `PlayerAI.thinkAttack`'s loop,
/// driven from the combat tick system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerIntent {
    Attack { target_object_id: i32 },
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

/// A player character in (or entering) the world. Owned by the `World` object
/// registry once in game; the `InGame` session links to it by `object_id`.
/// An ECS component (one fat component per player entity for now — see
/// `store::EntityStore`).
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

    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// The world-region cell this player is registered in (Java
    /// `WorldObject._worldRegion`). Kept in sync with `x`/`y` by
    /// `game_loop::visibility` (Java `updateWorldRegion`/`switchRegion`) —
    /// visibility deltas are computed against this, not raw coordinates.
    pub region: (i32, i32),

    // Base primary stats (TODO(G7): + henna/items/buffs).
    pub str_: i32,
    pub dex: i32,
    pub con: i32,
    pub int_: i32,
    pub wit: i32,
    pub men: i32,

    pub max_hp: i32,
    pub cur_hp: f64,
    pub max_mp: i32,
    pub cur_mp: f64,
    pub max_cp: i32,
    pub cur_cp: f64,

    pub exp: i64,
    pub sp: i64,
    pub reputation: i32,
    pub pk_kills: i32,
    pub pvp_kills: i32,
    pub vitality_points: i32,
    pub fame: i32,

    pub face: i32,
    pub hair_style: i32,
    pub hair_color: i32,

    // Combat stats — base template values for now (TODO(G7): full stat calc).
    pub p_atk: i32,
    pub p_atk_spd: i32,
    pub p_def: i32,
    pub m_atk: i32,
    pub m_atk_spd: i32,
    pub m_def: i32,
    pub crit_hit: i32,
    pub m_crit_hit: i32,
    pub evasion: i32,
    pub accuracy: i32,
    pub magic_evasion: i32,
    pub magic_accuracy: i32,
    pub atk_range: i32,

    // Movement (pre-multiplier) + collision.
    pub run_spd: i32,
    pub walk_spd: i32,
    pub swim_run_spd: i32,
    pub swim_walk_spd: i32,
    pub move_multiplier: f64,
    pub collision_radius: f64,
    pub collision_height: f64,
    pub running: bool,

    pub inventory: Inventory,

    /// Known skills (skill_id → level), loaded from `character_skills` (or the
    /// class's autoGet initial set at creation — see `character.rs`/`db.rs`).
    pub skills: HashMap<i32, i32>,
    /// Active buffs/debuffs (Java `EffectList`). Expiry is driven by the
    /// `Scheduler` (`ScheduledTask::BuffExpire`), not by anything here.
    pub buffs: Vec<ActiveBuff>,
    /// Java `CreatureStat`'s two modifier maps — buffs/gear push entries here;
    /// `recalculate_stats` folds them into the displayed combat stats.
    pub stats_add: HashMap<Stat, f64>,
    pub stats_mul: HashMap<Stat, f64>,
    /// `Some` while a cast is in flight (Java `Creature._skillCasters`, single
    /// NORMAL slot). Replaces + extends the old `casting: bool` re-entrancy
    /// guard: also carries the target snapshot and the task generation.
    pub cast: Option<CastState>,
    /// Monotonic cast-generation counter, bumped every `startCasting`.
    pub cast_seq: u64,
    /// Java `Creature._reuseTimeStampsSkills` + `_disabledSkills`, unified,
    /// keyed by `Skill::reuse_key()` (the shared `reuseDelayGroup` when one is
    /// set, else the skill id — so grouped skills share one entry). Java
    /// splits short reuses from >3000 ms timestamps only for DB persistence
    /// and packet filtering, both derivable from one map. Checked lazily — no
    /// expiry tasks.
    /// TODO: persist across relog like Java's `character_skills_save`.
    pub reuses: HashMap<i32, SkillReuse>,

    /// Currently targeted object id (Java `Creature._target`).
    pub target: Option<i32>,
    /// `Some` while moving (Java's nullable `Creature._move`); cleared on
    /// arrival by `movement::tick`.
    pub move_data: Option<MoveData>,

    // --- Combat state (G9) ---
    /// Java `Creature._isDead`.
    pub dead: bool,
    /// Persistent AI intention (attack loop) — see `PlayerIntent`.
    pub intent: Option<PlayerIntent>,
    /// Busy-swinging until this tick (Java `_attackEndTime`); the next swing
    /// may start once it passes.
    pub attack_end_tick: u64,
    /// In combat stance (client sword-drawn state) until this tick — 15 s
    /// past the last swing/hit (`AttackStanceTaskManager`); 0 = not in stance.
    pub stance_until_tick: u64,
    /// `Player._reviveRequested`-ish: die → "to village" → teleport →
    /// revive on `Appearing` (Java `setPendingRevive` → `onTeleported`).
    pub pending_revive: bool,
    /// Java `Creature._isTeleporting`: position pushed server-side, waiting
    /// for the client's `Appearing`.
    pub teleporting: bool,

    /// Last position/heading the client reported via `ValidatePosition`
    /// (Java `Player._clientX/_clientY/_clientZ/_clientHeading`).
    pub client_x: i32,
    pub client_y: i32,
    pub client_z: i32,
    pub client_heading: i32,
}

impl Player {
    /// Build a `Player` from a stored character row + its class template.
    /// Max HP/MP/CP are recomputed (not read from the DB) so they display
    /// correctly; current HP/MP come from the row, clamped to the max.
    pub fn from_char(data: &GameData, c: &CharData) -> Self {
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

        let mut p = Player {
            object_id: c.object_id,
            name: c.name.clone(),
            account: c.account_name.clone(),
            title: String::new(),
            level: c.level,
            class_id: c.class_id,
            base_class_id: c.base_class_id,
            race: c.race,
            is_female: c.sex != 0,
            x: c.x,
            y: c.y,
            z: c.z,
            heading: 0,
            region: crate::world::region_of(c.x, c.y),
            str_: t.base_str,
            dex: t.base_dex,
            con: t.base_con,
            int_: t.base_int,
            wit: t.base_wit,
            men: t.base_men,
            max_hp: max_hp as i32,
            cur_hp: c.cur_hp.min(max_hp),
            max_mp: max_mp as i32,
            cur_mp: c.cur_mp.min(max_mp),
            max_cp: max_cp as i32,
            cur_cp: 0.0,
            exp: c.exp,
            sp: c.sp,
            reputation: c.reputation,
            pk_kills: c.pk_kills,
            pvp_kills: c.pvp_kills,
            vitality_points: c.vitality_points,
            fame: 0,
            face: c.face,
            hair_style: c.hair_style,
            hair_color: c.hair_color,
            // Filled in by `recalculate_stats` below.
            p_atk: 0,
            p_atk_spd: 0,
            p_def: 0,
            m_atk: 0,
            m_atk_spd: 0,
            m_def: 0,
            crit_hit: 0,
            m_crit_hit: 0,
            evasion: 0,
            accuracy: 0,
            magic_evasion: 0,
            magic_accuracy: 0,
            atk_range: t.base_atk_range,
            run_spd: t.base_run_spd,
            walk_spd: t.base_walk_spd,
            swim_run_spd: t.base_swim_run_spd,
            swim_walk_spd: t.base_swim_walk_spd,
            move_multiplier: 1.0,
            collision_radius: t.collision_radius,
            collision_height: t.collision_height,
            running: true,
            inventory: Inventory::from_rows(&c.items),
            skills: c.skills.iter().copied().collect(),
            buffs: Vec::new(),
            stats_add: HashMap::new(),
            stats_mul: HashMap::new(),
            cast: None,
            cast_seq: 0,
            reuses: HashMap::new(),
            target: None,
            move_data: None,
            dead: c.cur_hp < 0.5,
            intent: None,
            attack_end_tick: 0,
            stance_until_tick: 0,
            pending_revive: false,
            teleporting: false,
            client_x: 0,
            client_y: 0,
            client_z: 0,
            client_heading: 0,
        };
        p.recalculate_stats(data);
        p
    }

    /// Java `CreatureStat.recalculateStats` narrowed to the combat stats G6
    /// computes. Re-derives from the class template's base values (not from
    /// `self`, so it's idempotent) × `BaseStat` bonus × level mod, then folds
    /// in `stats_add`/`stats_mul` (buffs). Call after level/buff/gear changes.
    /// TODO(G8+): weapon/armor `<stats>` contributions — item stat bonuses
    /// aren't parsed yet (`data/item_data.rs`), so this is the unarmed/naked
    /// value, same simplification G5 already made for item stats.
    pub fn recalculate_stats(&mut self, data: &GameData) {
        let t = data
            .player_templates
            .get(self.class_id)
            .or_else(|| data.player_templates.get(self.base_class_id))
            .cloned()
            .unwrap_or_default();
        let level_mod = (self.level as f64 + 89.0) / 100.0;
        let sb = &data.stat_bonus;
        let str_bonus = sb.bonus(BaseStat::Str, self.str_);
        let dex_bonus = sb.bonus(BaseStat::Dex, self.dex);
        let int_bonus = sb.bonus(BaseStat::Int, self.int_);
        let wit_bonus = sb.bonus(BaseStat::Wit, self.wit);

        // PAttackFinalizer / MAttackFinalizer.
        self.p_atk = self
            .finalize(Stat::PhysicalAttack, t.base_p_atk as f64 * str_bonus * level_mod)
            .round()
            .clamp(0.0, MAX_PATK) as i32;
        self.m_atk = self
            .finalize(Stat::MagicalAttack, t.base_m_atk as f64 * (int_bonus * level_mod).powf(2.2072))
            .round()
            .clamp(0.0, MAX_MATK) as i32;

        // P/MDefenseFinalizer, naked value only (see TODO above).
        self.p_def = self.finalize(Stat::PhysicalDefence, t.base_p_def as f64).round().max(0.0) as i32;
        self.m_def = self.finalize(Stat::MagicalDefence, t.base_m_def as f64).round().max(0.0) as i32;

        // P/MAttackSpeedFinalizer: `mul` floors at 0.7, not the usual 1.0.
        self.p_atk_spd = self
            .finalize_speed(Stat::PhysicalAttackSpeed, t.base_p_atk_spd as f64 * dex_bonus)
            .round()
            .clamp(1.0, MAX_PATK_SPEED) as i32;
        self.m_atk_spd = self
            .finalize_speed(Stat::MagicAttackSpeed, t.base_m_atk_spd as f64 * wit_bonus)
            .round()
            .clamp(1.0, MAX_MATK_SPEED) as i32;

        // P/MCritRateFinalizer (in per-mille, ×10).
        self.crit_hit = self
            .finalize(Stat::CriticalRate, t.base_crit_rate as f64 * dex_bonus * 10.0)
            .round()
            .clamp(0.0, MAX_PCRIT_RATE) as i32;
        self.m_crit_hit = self
            .finalize(Stat::MagicCriticalRate, t.base_m_crit_rate as f64 * wit_bonus * 10.0)
            .round()
            .clamp(0.0, MAX_MCRIT_RATE) as i32;

        // P/MAccuracyFinalizer, P/MEvasionRateFinalizer (high-level +N steps
        // above level 69 skipped — base classes here don't reach that high).
        let level = self.level as f64;
        self.accuracy = self
            .finalize(Stat::AccuracyCombat, (self.dex as f64).sqrt() * 5.0 + level)
            .round() as i32;
        self.magic_accuracy = self
            .finalize(Stat::AccuracyMagic, (self.wit as f64).sqrt() * 3.0 + level * 2.0)
            .round() as i32;
        self.evasion = self
            .finalize(Stat::EvasionRate, (self.dex as f64).sqrt() * 5.0 + level)
            .round()
            .clamp(0.0, MAX_EVASION) as i32;
        self.magic_evasion = self
            .finalize(Stat::MagicEvasionRate, (self.wit as f64).sqrt() * 3.0 + level * 2.0)
            .round() as i32;

        // Speed: base template value, buffs (Speed effect) apply through the
        // add/mul maps exactly like the combat stats above.
        self.run_spd = self.finalize(Stat::RunSpeed, t.base_run_spd as f64).round() as i32;
        self.walk_spd = self.finalize(Stat::WalkSpeed, t.base_walk_spd as f64).round() as i32;
        self.swim_run_spd = self.finalize(Stat::SwimRunSpeed, t.base_swim_run_spd as f64).round() as i32;
        self.swim_walk_spd = self.finalize(Stat::SwimWalkSpeed, t.base_swim_walk_spd as f64).round() as i32;
    }

    /// `Stat.defaultValue`: `base * mul + add` from the accumulated modifier
    /// maps (1.0/0.0 when nothing has touched this stat).
    fn finalize(&self, stat: Stat, base: f64) -> f64 {
        let mul = self.stats_mul.get(&stat).copied().unwrap_or(1.0);
        let add = self.stats_add.get(&stat).copied().unwrap_or(0.0);
        base * mul + add
    }

    /// `P/MAttackSpeedFinalizer.defaultValue`: same shape, but `mul` floors at
    /// 0.7 instead of applying whatever's in the map directly (so an absent or
    /// tiny buff doesn't produce a slower-than-0.7x cast/attack speed).
    fn finalize_speed(&self, stat: Stat, base: f64) -> f64 {
        let mul = self.stats_mul.get(&stat).copied().unwrap_or(1.0).max(0.7);
        let add = self.stats_add.get(&stat).copied().unwrap_or(0.0);
        base * mul + add
    }

    /// Fold a landed buff's effects into the modifier maps and recompute.
    /// Java `BuffInfo.initializeEffects` → `AbstractEffect.pump`.
    pub fn apply_buff(&mut self, data: &GameData, buff: ActiveBuff) {
        for effect in &buff.effects {
            apply_modifier(&mut self.stats_add, &mut self.stats_mul, effect);
        }
        self.buffs.push(buff);
        self.recalculate_stats(data);
    }

    /// Remove an expired/replaced buff and recompute from scratch (Java just
    /// removes the `BuffInfo` and calls `resetStats()`, which rebuilds the
    /// maps from the remaining active buffs — do the same here rather than
    /// trying to subtract in place, which would drift under rounding).
    pub fn remove_buff(&mut self, data: &GameData, skill_id: i32) {
        self.buffs.retain(|b| b.skill_id != skill_id);
        self.stats_add.clear();
        self.stats_mul.clear();
        let buffs = self.buffs.clone();
        for buff in &buffs {
            for effect in &buff.effects {
                apply_modifier(&mut self.stats_add, &mut self.stats_mul, effect);
            }
        }
        self.recalculate_stats(data);
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
