//! ECS components shared by players and NPCs — stage 2 of the `bevy_ecs`
//! adoption (`docs/PLAN_ECS_STAGE2.md` §2). Data only: components are split
//! along *system access seams* (what a per-tick sweep reads/writes without
//! the rest of the object), not per field, and carry no game logic beyond
//! trivial accessors. Player-only / NPC-only state stays in the (shrinking)
//! fat structs in `model/mod.rs` / `model/npc.rs` until its own phase.

use std::collections::HashMap;

use bevy_ecs::component::Component;

/// World position + facing (from Java `WorldObject`'s x/y/z +
/// `Creature._heading`). On both players and NPCs.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
}

impl Position {
    /// 2D center-to-center distance (the shape every range/reach check uses).
    pub fn distance_2d(&self, other: &Position) -> f64 {
        (((other.x - self.x) as f64).powi(2) + ((other.y - self.y) as f64).powi(2)).sqrt()
    }
}

/// The world-region cell this object is registered in (Java
/// `WorldObject._worldRegion`). Kept in sync with `Position` by the
/// visibility/movement systems (Java `updateWorldRegion`/`switchRegion`) —
/// visibility deltas and broadcast scoping compare this, never raw
/// coordinates. Separate from `Position` because it changes on a different
/// cadence (cell crossings) and has different readers (visibility, not math).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionCell(pub (i32, i32));

/// HP/MP + liveness (Java `CreatureStatus` + `Creature._isDead`). On both
/// players and NPCs; CP is player-only and lives in [`PlayerVitals`]. `dead`
/// rides here (not a marker component): every writer flips it in the same
/// breath as HP, and death is a branch inside systems rather than a sweep
/// filter — a field avoids an archetype move per death/revive.
#[derive(Component, Debug, Clone, Copy)]
pub struct Vitals {
    pub max_hp: i32,
    pub cur_hp: f64,
    pub max_mp: i32,
    pub cur_mp: f64,
    /// Java `Creature._isDead` — for NPCs: corpse until decay removes it.
    pub dead: bool,
}

impl Vitals {
    pub fn hp_full(max_hp: i32, max_mp: i32) -> Self {
        Self { max_hp, cur_hp: max_hp as f64, max_mp, cur_mp: max_mp as f64, dead: false }
    }
}

/// CP (`PcStatus`) — the player-only vitals extension, so NPC damage code
/// never sees a CP field it must ignore.
#[derive(Component, Debug, Clone, Copy)]
pub struct PlayerVitals {
    pub max_cp: i32,
    pub cur_cp: f64,
}

/// Movement speeds + run/walk mode. For players these are stat-finalizer
/// *outputs* (`recalculate_stats` writes them: template base × buff
/// modifiers); for NPCs they're memoized from the template at spawn (the
/// template never changes, so this is the same value the old code re-read
/// per use). f64 keeps NPC fractional speeds exact; player values are the
/// same rounded numbers as before, just stored as f64.
#[derive(Component, Debug, Clone, Copy)]
pub struct Speeds {
    pub run_spd: f64,
    pub walk_spd: f64,
    pub swim_run_spd: f64,
    pub swim_walk_spd: f64,
    pub move_multiplier: f64,
    /// `Creature._isRunning` — players spawn running; NPCs walk until AI
    /// flips to run on aggro.
    pub running: bool,
}

impl Speeds {
    /// The ground speed movement math uses (`Creature.getMoveSpeed`).
    pub fn move_speed(&self) -> f64 {
        (if self.running { self.run_spd } else { self.walk_spd }) * self.move_multiplier
    }
}

/// Collision cylinder (template `collision_radius`/`collision_height`) —
/// reach/range gates and packet fields.
#[derive(Component, Debug, Clone, Copy)]
pub struct Collision {
    pub radius: f64,
    pub height: f64,
}

/// Combat-stat finalizer outputs (Java `CreatureStat`'s computed values).
/// Players: written by `recalculate_stats` (base × stat bonus × level mod ×
/// buff modifiers), same rounded values as before stored as f64. NPCs:
/// memoized once at spawn from the (immutable) template through the same
/// finalizer math the old `combatant()` ran per call — values identical.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct CombatStats {
    pub p_atk: f64,
    pub m_atk: f64,
    pub p_def: f64,
    pub m_def: f64,
    pub p_atk_spd: i32,
    pub m_atk_spd: i32,
    /// Per-mille (×10), like Java's `PCriticalRateFinalizer` output.
    pub crit_hit: f64,
    pub m_crit_hit: f64,
    pub evasion: i32,
    pub accuracy: i32,
    pub magic_evasion: i32,
    pub magic_accuracy: i32,
    pub atk_range: i32,
    /// Weapon `randomDamage` (class templates all declare `baseRndDam = 10`;
    /// NPC templates carry their own).
    pub random_dmg: i32,
}

/// Swing/stance timing (Java `_attackEndTime` + `AttackStanceTaskManager`
/// membership). `stance_until_tick` is player-only in practice (the client
/// sword-drawn state); it stays 0 on NPCs.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct AttackState {
    /// Busy-swinging until this tick; the next swing may start once past.
    pub attack_end_tick: u64,
    /// In combat stance until this tick — 15 s past the last swing/hit;
    /// 0 = not in stance.
    pub stance_until_tick: u64,
}

/// An in-flight move — **present only while moving** (the stage-2 shape of
/// Java's nullable `Creature._move`). Presence is the movement tick's sweep
/// filter: the interpolation query visits only entities that carry this,
/// instead of scanning 34.9k static NPCs' `None`s every 100 ms. Insert =
/// `moveToLocation`, remove = arrival/stop/teleport/death.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Movement(pub crate::model::movement::MoveData);

/// A move deferred on the path worker — **present only while waiting** for
/// the `PathEvent` reply. `seq` is the request's sequence number: a reply
/// with an older one is stale (superseded by a newer click) and is dropped.
/// Java has no equivalent state — `CellPathFinding.findPath` runs
/// synchronously inside `moveToLocation`.
#[derive(Component, Debug, Clone, Copy)]
pub struct PathWait {
    pub seq: u64,
}

/// An in-flight cast — **present only mid-cast** (Java's single NORMAL
/// `SkillCaster` slot, `Player.cast` before stage 2). The generation counter
/// (`Player.cast_seq`) stays on the player: it must survive across casts for
/// the scheduler's stale-task no-op contract.
#[derive(Component, Debug, Clone)]
pub struct Casting(pub crate::model::CastState);

/// A persistent AI intention (the attack loop) — **present only while set**,
/// so the player combat tick sweeps intent-holders only.
#[derive(Component, Debug, Clone, Copy)]
pub struct Intent(pub crate::model::PlayerIntent);

/// Known skills (skill_id → level), loaded from `character_skills` (or the
/// class's autoGet initial set at creation). Player-only.
#[derive(Component, Debug, Clone, Default)]
pub struct SkillBook(pub HashMap<i32, i32>);

/// Live cooldowns (Java `_reuseTimeStampsSkills` + `_disabledSkills`,
/// unified), keyed by `Skill::reuse_key()`. Checked lazily — no expiry
/// tasks. TODO: persist across relog like Java's `character_skills_save`.
#[derive(Component, Debug, Clone, Default)]
pub struct Reuses(pub HashMap<i32, crate::model::SkillReuse>);

/// Active buffs/debuffs (Java `EffectList`). Expiry is driven by the
/// `Scheduler` (`ScheduledTask::BuffExpire`), not by anything here.
#[derive(Component, Debug, Clone, Default)]
pub struct Buffs(pub Vec<crate::model::skill::ActiveBuff>);

/// Java `CreatureStat`'s two modifier maps — buffs/gear push entries here;
/// `recalculate_stats` folds them into `CombatStats`/`Speeds`.
#[derive(Component, Debug, Clone, Default)]
pub struct StatModifiers {
    pub add: HashMap<crate::model::stats::Stat, f64>,
    pub mul: HashMap<crate::model::stats::Stat, f64>,
}

/// Currently targeted object id (Java `Creature._target`), player-only —
/// NPC targeting goes through the aggro list.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TargetRef(pub Option<i32>);

/// Last position/heading the client reported via `ValidatePosition`
/// (Java `Player._clientX/_clientY/_clientZ/_clientHeading`).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ClientPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
}

/// STR/DEX/CON/INT/WIT/MEN (player-only for now — NPC base stats stay on the
/// template until something buffs them). Inputs to the stat finalizers and
/// the regen bonuses.
#[derive(Component, Debug, Clone, Copy)]
pub struct BaseStats {
    pub str_: i32,
    pub dex: i32,
    pub con: i32,
    pub int_: i32,
    pub wit: i32,
    pub men: i32,
}
