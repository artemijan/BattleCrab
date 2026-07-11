//! Port of `model/skill/Skill.java` — scoped to the fields G6's cast pipeline
//! actually reads (targeting/timing/costs/abnormal info), plus the effect
//! list. Full `Skill.java` has ~40 more fields (traits, elements, fan/affect
//! shapes, …) — added when combat (G9) or AoE/PvP targeting need them.

use crate::model::stats::{Stat, StatModifierType};

/// Java `SkillOperateType`, scoped to what G6 dispatches on. Everything else
/// (`A2 static, `A3`, channeling, …) reads as `Other` and isn't castable yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperateType {
    /// `A1`/`A2`: an active, targeted or self-cast skill with a cast bar.
    Active,
    /// `P`: passive — never sent to `RequestMagicSkillUse`, no cast pipeline.
    Passive,
    /// `T`: toggle — out of scope for G6 (see plan's deferred list).
    Toggle,
    Other,
}

/// Java `TargetType`, scoped to the single-target types the cast pipeline
/// resolves (see `resolve_cast_target`) plus a catch-all so unhandled skills
/// still load instead of failing to parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetType {
    /// `SELF`: always the caster.
    Self_,
    /// `TARGET`: the current target, friendly or not (self allowed).
    Target,
    /// `ENEMY`: an attackable target (force-use required against unflagged
    /// players — see `targethandlers/Enemy.java`).
    Enemy,
    /// `ENEMY_ONLY`: like `ENEMY` minus the "attack anything with ctrl"
    /// leniencies; identical to `Enemy` in a world with only players.
    EnemyOnly,
    Other,
}

/// The Rust counterpart of Java's `AbstractStatAddEffect`/
/// `AbstractStatPercentEffect` — one generic type instead of the 63 one-line
/// subclasses Java has (each just names a `Stat` and a fixed mode).
#[derive(Debug, Clone, Copy)]
pub struct StatModifierEffect {
    pub stat: Stat,
    pub mode: StatModifierType,
    pub amount: f64,
}

/// A skill effect the pipeline knows how to apply. Java registers ~380 effect
/// handler scripts by name; here each supported kind is a variant —
/// `StatModifier` covers the whole `AbstractStatAddEffect`/
/// `AbstractStatPercentEffect` family, the instant kinds get one variant per
/// ported handler. Unregistered effect names are still dropped at load.
#[derive(Debug, Clone, Copy)]
pub enum SkillEffect {
    /// Continuous stat pump (goes into an `ActiveBuff` for `abnormal_time`).
    StatModifier(StatModifierEffect),
    /// `handlers/effecthandlers/MagicalAttack.java` — instant magic damage.
    MagicalAttack { power: f64 },
    /// `handlers/effecthandlers/Heal.java` — instant HP restore.
    Heal { power: f64 },
}

/// `dist/game/data/stats/skills/*.xml` → `Skill.java`, scoped to G6.
#[derive(Debug, Clone)]
pub struct Skill {
    pub id: i32,
    pub level: i32,
    pub name: String,
    pub operate_type: OperateType,
    pub target_type: TargetType,
    /// Java `isMagic`: 0 physical, 1 magic, 2 static, 3 dance/song, 4 trigger.
    /// Drives cast-time scaling (`calc_skill_time_factor`) and crit rolls.
    pub magic_type: i32,
    /// Java `effectPoint` — negative marks an offensive ("bad") skill.
    pub effect_point: i32,
    pub cast_range: i32,
    pub effect_range: i32,
    /// Milliseconds from cast start to the skill "landing" (Java `hitTime`),
    /// before casting-speed scaling.
    pub hit_time: i32,
    /// Java `hitCancelTime` (seconds) — the launch→finish phase length input;
    /// almost always 0, floored to 500 ms by `calc_skill_cancel_time`.
    pub hit_cancel_time: f64,
    /// Extra server-side cooldown after `finishSkill` (Java `coolTime`).
    pub cool_time: i32,
    /// Reuse delay in ms (Java `reuseDelay`) — enforced server-side via
    /// `Player.reuses` and shown client-side via the `MagicSkillUse` fields.
    pub reuse_delay: i32,
    pub mp_consume: i32,
    pub mp_initial_consume: i32,
    pub hp_consume: i32,
    /// Seconds a landed buff/debuff lasts (Java `abnormalTime`); 0 for
    /// instant/non-buff skills.
    pub abnormal_time: i32,
    pub abnormal_level: i32,
    /// Raw `<abnormalType>` XML text (Java `AbnormalType` has ~500 entries —
    /// only resolved to a client id, via `abnormal_type_client_id`, for the
    /// handful `AbnormalStatusUpdate` actually needs so far).
    pub abnormal_type: String,
    pub effects: Vec<SkillEffect>,
}

impl Skill {
    /// Java `Skill.isBad()`: `effectPoint < 0` (aggro/debuff/damage skills).
    pub fn is_bad(&self) -> bool {
        self.effect_point < 0
    }

    /// The continuous stat-pump subset of `effects` — what lands as an
    /// `ActiveBuff` (instant effects never enter a buff).
    pub fn stat_modifier_effects(&self) -> Vec<StatModifierEffect> {
        self.effects
            .iter()
            .filter_map(|e| match e {
                SkillEffect::StatModifier(m) => Some(*m),
                _ => None,
            })
            .collect()
    }
}

/// `AbnormalType.getClientId()`, scoped to the types skills registered in
/// `EFFECT_REGISTRY` actually use. Unknown/unregistered types map to `NONE`
/// (`-1`), same as Java's default. TODO: grow alongside `EFFECT_REGISTRY`.
pub fn abnormal_type_client_id(name: &str) -> i32 {
    match name {
        "PA_UP" => 94,
        "PD_UP" => 98,
        _ => -1,
    }
}

/// A landed buff/debuff on a `Player` (Java `BuffInfo`, trimmed to what G6
/// needs: which stats it's modifying and when it wears off — the "when" is
/// tracked by the `Scheduler`, not stored here).
#[derive(Debug, Clone)]
pub struct ActiveBuff {
    pub skill_id: i32,
    pub skill_level: i32,
    pub abnormal_type_client_id: i32,
    /// Absolute tick the buff expires at (for `AbnormalStatusUpdate`'s
    /// remaining-time field).
    pub expires_at_tick: u64,
    pub effects: Vec<StatModifierEffect>,
}
