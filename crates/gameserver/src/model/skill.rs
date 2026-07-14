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
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatModifierEffect {
    pub stat: Stat,
    pub mode: StatModifierType,
    pub amount: f64,
    /// `ConditionUsingItemType` armor mask from the effect's `<armorType>`
    /// list (OR of `ArmorType::mask_bit`s), or `0` when the effect has no such
    /// condition and always applies. Only meaningful for passive skills whose
    /// contribution depends on the worn armor (Spellcraft 163, Magician's
    /// Movement 118); active-buff effects leave it `0`.
    pub armor_condition: u8,
    /// OR of `WeaponType::mask_bit`s from the effect's `<weaponType>` list, or
    /// `0` when the effect has no such condition and always applies. Gates the
    /// effect on the *equipped weapon* (e.g. Weapon Mastery 249's
    /// `-30% MagicalAttackSpeed` applies only with a BOW/POLE in hand) — the
    /// weapon-side counterpart of `armor_condition`.
    pub weapon_condition: u32,
}

/// One entry inside a `RestorationRandom` reward group (Java
/// `RestorationItemHolder`). `min_enchant`/`max_enchant` drive the grant-time
/// enchant roll (`game_loop::skills::effects::give_item_random`: when
/// `max_enchant > 0`, the created item gets `Rnd.get(min_enchant, max_enchant)`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RestorationItem {
    pub item_id: i32,
    pub count: i64,
    pub min_enchant: i32,
    pub max_enchant: i32,
}

/// One `<items><item chance="..">...</item></items>` roulette slice (Java
/// `ExtractableProductItem`): a chance-weighted set of items granted
/// together when this slice is picked. `chance` is the raw XML percentage
/// (0-100 space, slices summing to ~100), matching Java's
/// `100 * Rnd.nextDouble()` roulette roll — not pre-scaled like
/// `item_data::CapsuledItem::chance`.
#[derive(Debug, Clone)]
pub struct RestorationGroup {
    pub chance: f64,
    pub items: Vec<RestorationItem>,
}

/// A skill effect the pipeline knows how to apply. Java registers ~380 effect
/// handler scripts by name; here each supported kind is a variant —
/// `StatModifier` covers the whole `AbstractStatAddEffect`/
/// `AbstractStatPercentEffect` family, the instant kinds get one variant per
/// ported handler. Unregistered effect names are still dropped at load.
#[derive(Debug, Clone)]
pub enum SkillEffect {
    /// Continuous stat pump (goes into an `ActiveBuff` for `abnormal_time`).
    StatModifier(StatModifierEffect),
    /// `handlers/effecthandlers/MagicalAttack.java` — instant magic damage.
    MagicalAttack { power: f64 },
    /// `handlers/effecthandlers/Heal.java` — instant HP restore.
    Heal { power: f64 },
    /// `handlers/effecthandlers/Restoration.java` — instant single-item
    /// grant. Backs item-use skills wrapping a fixed pack/box reward (e.g.
    /// spiritshot packs): the item's `<skills>` entry casts this, which is
    /// where the actual reward comes from.
    GiveItem { item_id: i32, item_count: i64, item_enchant_level: i32 },
    /// `handlers/effecthandlers/RestorationRandom.java` — one weighted
    /// roulette pick among reward groups (each group can grant multiple
    /// items at once). Used by "pick one of N" reward boxes.
    GiveItemRandom { groups: Vec<RestorationGroup> },
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
    /// Java `reuseDelayGroup` (default -1): skills sharing a positive group id
    /// share one cooldown. Sent raw in `MagicSkillUse`/`SkillList` — the
    /// client treats 0 as "every skill", so ungrouped must stay -1.
    pub reuse_delay_group: i32,
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

    /// The id a reuse is tracked and broadcast under: the shared
    /// `reuseDelayGroup` when one is set, else the skill's own id. Java's
    /// `Skill._reuseHashCode` minus the level/sub-level dimensions —
    /// `Player.reuses` is keyed per skill, not per level.
    pub fn reuse_key(&self) -> i32 {
        if self.reuse_delay_group > 0 {
            self.reuse_delay_group
        } else {
            self.id
        }
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
    /// True for entries that stand in for a passive skill's stat pump (the
    /// grade-penalty skills 6209/6213) rather than a timed buff. They drive
    /// stats through the same modifier maps but are hidden from
    /// `AbnormalStatusUpdate` (Java passive skills never show an abnormal icon)
    /// and never get a `BuffExpire` schedule.
    pub passive: bool,
    pub effects: Vec<StatModifierEffect>,
}
