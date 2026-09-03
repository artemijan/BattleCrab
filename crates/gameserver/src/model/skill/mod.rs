//! Port of `model/skill/Skill.java` — scoped to the fields G6's cast pipeline
//! actually reads (targeting/timing/costs/abnormal info), plus the effect
//! list. Full `Skill.java` has ~40 more fields (traits, elements, fan/affect
//! shapes, …) — added when combat (G9) or AoE/PvP targeting need them.

pub mod abnormal;
pub mod active_buff;
pub mod condition;
pub mod effect_flag;
pub mod effects;
pub mod target;
pub mod traits;

use crate::model::stats::{Stat, StatModifierType};

use condition::SkillCondition;
use effects::{SkillEffect, StatModifierEffect};
use target::{AffectObject, AffectScope, OperateType, TargetType};
use traits::TraitType;

/// Java `NextActionType` — what `SkillCaster.finishSkill` queues after a cast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum NextAction {
    #[default]
    None,
    Attack,
    Cast,
}

/// `ReduceDropType` — which of `ReduceDropPenalty`'s three stat pairs to grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ReduceDropKind {
    #[default]
    Mob,
    Pk,
    Raid,
}

/// `dist/game/data/stats/skills/*.xml` → `Skill.java`, scoped to G6.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Skill {
    pub id: i32,
    pub level: i32,
    pub name: String,
    /// `<icon>` — the client-side icon path (Java `Skill.getIcon()`, default
    /// `icon.skill0000`). Cosmetic: read by the shift-click NPC skill view.
    pub icon: String,
    pub operate_type: OperateType,
    /// Java `Skill.isContinuous()` — an effect that sits on the target for
    /// `abnormal_time` rather than resolving instantly. Drives the NPC AI's
    /// BUFF/DEBUFF bucketing and its "target already has this abnormal" skip.
    pub is_continuous: bool,
    pub target_type: TargetType,
    /// Java `<overHit>` — a killing blow from this skill grants bonus XP
    /// proportional to the *excess* damage (`Attackable.calculateOverhitExp`).
    /// 59 learnable skills carry it (Triple Slash, Power Strike, Sonic Storm…).
    pub over_hit: bool,
    /// Java `<abnormalVisualEffect>` as resolved client ids — what the client
    /// draws on anyone carrying this skill's abnormal. Cosmetic only.
    pub abnormal_visuals: Vec<i16>,
    /// Java `toggleGroupId` — toggles sharing a group are mutually exclusive:
    /// switching one on stops the others (`stopAllTogglesOfGroup`). 0 = no
    /// group.
    pub toggle_group_id: i32,
    /// Java `affectScope` — how the primary target expands into the affected
    /// set (`Skill.forEachTargetAffected`). Defaults to `SINGLE`.
    pub affect_scope: AffectScope,
    /// `<trait>` — the debuff's own `TraitType`, matched against the target's
    /// `DefenceTrait` resistances when it tries to land. `NONE` for most
    /// skills; the dist's stuns declare `SHOCK`, its fear/confuse
    /// `DERANGEMENT`, and so on.
    pub trait_type: TraitType,
    /// Java `affectObject` — the friend/foe filter each swept-up candidate must
    /// pass. Defaults to `ALL` (Java's "no handler" = no filtering).
    pub affect_object: AffectObject,
    /// Java `affectRange` — the radius the scope sweeps (0 = no sweep).
    pub affect_range: i32,
    /// Java `_affectLimit` `[min, max]` from `<affectLimit>min-max</affectLimit>`.
    /// Read through [`Skill::affect_limit`], which reproduces Java's roll.
    pub affect_limit: (i32, i32),
    /// Java `_fanRange` from `<fanRange>` — `unk;startDegree;fanAffectRange;
    /// fanAffectAngle`, the geometry behind the FAN/SQUARE/RING_RANGE scopes.
    /// `[1]` rotates the arc/rect off the caster's heading, `[2]` is the fan
    /// radius / rect length / ring inner radius, `[3]` the fan's full angle /
    /// rect width. `[0]` is never read (non-zero exactly once in the dist).
    /// Level-valued in the XML (one SQUARE breath declares six tuples).
    pub fan_range: [i32; 4],
    /// Java `isMagic`: 0 physical, 1 magic, 2 static, 3 dance/song, 4 trigger.
    /// Drives cast-time scaling (`calc_skill_time_factor`) and crit rolls.
    pub magic_type: i32,
    /// Java `magicLevel` — the skill's own level for magic-hit math. Feeds
    /// `Formulas.calcMagicSuccess` when `CalculateMagicSuccessBySkillMagicLevel`
    /// is on (the dist default), used by the Spoil landing roll.
    pub magic_level: i32,
    /// Java `activateRate` (default -1) — a debuff's base landing rate before the
    /// level/resist math in `Formulas.calcEffectSuccess`. `-1` means the effect
    /// always lands (no resist roll). Feeds `formulas::calc_effect_land_rate`.
    pub activate_rate: i32,
    /// Java `lvlBonusRate` (default 0) — how steeply the caster/target level gap
    /// swings the debuff landing rate; multiplies the level term in
    /// `calc_effect_land_rate`.
    pub lvl_bonus_rate: i32,
    /// Java `effectPoint` — negative marks an offensive ("bad") skill.
    pub effect_point: i32,
    pub cast_range: i32,
    pub effect_range: i32,
    /// Milliseconds from cast start to the skill "landing" (Java `hitTime`),
    /// before casting-speed scaling.
    pub hit_time: i32,
    /// Java `<nextAction>` — what the caster does once the cast finishes.
    /// `SkillCaster.finishSkill`: with `ATTACK` (339 skills on this dist) the
    /// caster resumes attacking the target, with `CAST` (11) it repeats the
    /// skill; `NONE` just fires `EVT_FINISH_CASTING`. Java gates both on the
    /// AI having no queued intention, a real target that is not the caster and
    /// is auto-attackable, and — for `ATTACK` only — shift not being held.
    ///
    /// This is why a Power Strike leaves you swinging rather than standing
    /// still: without it every offensive skill ends combat.
    pub next_action: NextAction,
    /// Java `<abnormalResists>` — abnormal types this skill makes its caster
    /// immune to **while it is casting** (`Formulas.calcEffectSuccess`:
    /// `target.isCastingNow(s -> s.getSkill().getAbnormalResists().contains(
    /// skill.getAbnormalType()))`). 176 skills declare one; the long list on
    /// 146 of them is the "uninterruptible ritual" set.
    pub abnormal_resists: Vec<String>,
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
    /// `<staticReuse>` (Java `Skill._staticReuse`, default false; **1297
    /// skills on this dist set it**). A static-reuse skill's cooldown is its
    /// raw `reuse_delay` — `CreatureStat.getReuseTime` returns before applying
    /// the per-magic-type reuse rate — so no [`SkillEffect::Reuse`] buff can
    /// shorten it.
    pub static_reuse: bool,
    pub mp_consume: i32,
    pub mp_initial_consume: i32,
    pub hp_consume: i32,
    /// `<withoutAction>` (Java `Skill._withoutAction`, default false). An
    /// item skill flagged this way is fired instantly by
    /// `ItemSkillsTemplate` (the `SkillCaster.triggerCast` branch) instead of
    /// going through `useMagic`'s cast bar. Only four skills in the whole
    /// dist set it, none in the Interlude ranges, but the flag is half of
    /// Java's instant/cast decision so it is parsed rather than assumed.
    pub without_action: bool,
    /// Java `isSuicideAttack` — its only consumer is `NpcData.parse`, which
    /// routes the skill into the AI's SUICIDE bucket (cast below 30 % HP).
    pub is_suicide_attack: bool,
    /// `<itemConsumeId>`/`<itemConsumeCount>` (Java `Skill.getItemConsumeId`
    /// / `getItemConsumeCount`, 0 = none) — the "reagent" the skill spends.
    /// Read by `ItemSkillsTemplate.checkConsume` to decide whether the item
    /// handler is the one that destroys the item.
    pub item_consume_id: i32,
    pub item_consume_count: i32,
    /// Seconds a landed buff/debuff lasts (Java `abnormalTime`); 0 for
    /// instant/non-buff skills.
    pub abnormal_time: i32,
    pub abnormal_level: i32,
    /// Raw `<abnormalType>` XML text (Java `AbnormalType` has ~500 entries —
    /// only resolved to a client id, via `abnormal_type_client_id`, for the
    /// handful `AbnormalStatusUpdate` actually needs so far).
    pub abnormal_type: String,
    /// Java `Skill.canBeDispelled()` (`<canBeDispelled>`, default true) — whether
    /// the client's alt+click buff-cancel (`RequestDispel`) is allowed to strip it.
    pub can_be_dispelled: bool,
    /// Java `Skill.isDebuff()` (`<isDebuff>`, default false). A debuff can't be
    /// self-dispelled via alt+click even when `can_be_dispelled` is set.
    pub is_debuff: bool,
    /// Java `Skill.isExcludedFromCheck()` (`<excludedFromCheck>`, default
    /// false) — the first arm of `SkillTreeData.isSkillAllowed`: a skill
    /// flagged here is legitimate no matter which tree does or does not list
    /// it. On this dist the 86 flagged levels are the subclass certification
    /// families (631–655, 799–804, 1956–1986), the Exalted line and the two
    /// storage expansions — all learned by routes that are not class trees.
    pub excluded_from_check: bool,
    /// Java `Skill.isSharedWithSummon()` (`<isSharedWithSummon>`, **default
    /// true**) — a continuous, non-debuff buff landing on a player is re-applied
    /// to each of their servitors (`Skill.applyEffects`'s "buff sharing"
    /// branch). The default being `true` is the load-bearing part: only three
    /// skills in the whole datapack declare the tag at all, so parsing this like
    /// a normal `false`-default flag would silently stop sharing every buff in
    /// the game.
    pub shared_with_summon: bool,
    /// Java `Skill.isStayAfterDeath()` (`<stayAfterDeath>`, default false) — the
    /// buff survives its holder's death (`EffectList
    /// .stopAllEffectsExceptThoseThatLastThroughDeath`).
    ///
    /// Java's getter is `_stayAfterDeath || _irreplacableBuff ||
    /// _isNecessaryToggle` — **one getter over three tags** — and all three are
    /// folded into this field at parse (G34 S3). `<irreplacableBuff>` alone is
    /// on 30 learnable skills, so reading only `<stayAfterDeath>` stripped the
    /// clan/pledge and noblesse buffs on every death.
    pub stay_after_death: bool,
    /// Java `Skill.isRemovedOnDamage()` (`<removedOnDamage>`, default false) —
    /// the buff drops the moment its holder takes damage
    /// (`CreatureStatus.reduceHp` → `EffectList.stopEffectsOnDamage`). This is
    /// what makes **sleep** a one-hit crowd control: 36 skills carry the tag on
    /// this dist and most of them are `SLEEP`, the rest `HIDE`,
    /// `FORCE_MEDITATION` and a few transforms. Without it a slept player stays
    /// action-blocked while the mob beats on them.
    pub removed_on_damage: bool,
    pub effects: Vec<SkillEffect>,
    /// Java `SkillOperateType.isSelfContinuous()` — true for `A3` alone.
    /// Read only by [`active_buff::ActiveBuff::displayed`]; the effects themselves behave
    /// exactly like any other active skill's.
    pub self_continuous: bool,
    /// Java `EffectScope.SELF` (`<selfEffects>`) — applied to the **caster**,
    /// as a separate `applyEffects(caster, caster, …)` after the target loop.
    /// Blinding Blow 321, Sonic Rage 345, Raging Force 346, Vengeance 368,
    /// Evade Shot 369, Critical Blow 409 all put a real self-buff here, and the
    /// parser used to read only `<effects>` — so none of them landed.
    pub self_effects: Vec<SkillEffect>,
    /// Java `EffectScope.PVE` / `PVP` (`<pveEffects>`/`<pvpEffects>`) — applied
    /// to the same target as `effects`, but only for the matching matchup:
    /// `effector.isPlayable() && effected.isAttackable()` → PVE, else
    /// `effector.isPlayable() && effected.isPlayable()` → PVP, else neither.
    pub pve_effects: Vec<SkillEffect>,
    pub pvp_effects: Vec<SkillEffect>,
    /// Java `EffectScope.CHANNELING` (`<channelingEffects>`) — applied by the
    /// `SkillChannelizer` tick to each swept target while a `CA1` cast runs
    /// (Volcano's `MagicalAttack power=500`), never at cast finish.
    pub channeling_effects: Vec<SkillEffect>,
    /// Java `EffectScope.END` (`<endEffects>`) — applied when the buff comes
    /// **off**, as the last thing `EffectList` does on removal. Anchor (1170)
    /// is the learnable carrier: its first stage holds the body rigid, and the
    /// end-effect fires skill 6091 for the paralysis its own description
    /// promises. Without it Anchor did half its job.
    pub end_effects: Vec<SkillEffect>,
    /// Java `mpPerChanneling` — MP drained per channeling tick, **defaulting
    /// to `mpConsume`** (`set.getInt("mpPerChanneling", _mpConsume)`), so a
    /// channeling skill without the tag still drains. Running dry aborts the
    /// cast with SM 140.
    pub mp_per_channeling: i32,
    /// Java `Skill.getChannelingSkillId()` (`<channelingSkillId>`) — the skill a
    /// channeler *applies to its targets* while the cast is held, as opposed to
    /// `channeling_effects` which it applies directly.
    ///
    /// The distinction matters because the applied **level is the number of
    /// distinct channelers** aimed at that target (capped at the channeled
    /// skill's max level), which is the whole point of the mechanic: two
    /// Warcryers holding Battle Stance 426 on the same ally stack it to Battle
    /// Force 5104 level 2. `0` when the skill channels effects instead.
    pub channeling_skill_id: i32,
    /// Java `channelingTickInterval` in ms (XML seconds × 1000; Java defaults
    /// the raw value to 2000 s — dead for non-channeling skills, and every
    /// channeler on this dist declares it).
    pub channeling_tick_ms: i32,
    /// Java `channelingStart` in ms — delay before the first tick.
    pub channeling_start_ms: i32,
    /// Java `<attributeType>`/`<attributeValue>` — the skill's element and its
    /// flat attack contribution (Volcano is FIRE 20). Feeds
    /// `Formulas.calcAttributeBonus`'s attack side; `None` = no element, and
    /// the attacker's strongest POWER stat elects the element instead.
    pub attribute_type: Option<crate::model::stats::Element>,
    pub attribute_value: i32,
    /// The enchant sub-level this instance was built for (0 = unenchanted;
    /// 1001–3020 = an enchant-route step — PLAN_G19_SKILL_ENCHANT.md).
    pub sub_level: i32,
    /// Java `Skill._conditionLists` — the parsed `<conditions>` /
    /// `<targetConditions>` / `<passiveConditions>` blocks
    /// (`SkillConditionScope.GENERAL` / `TARGET` / `PASSIVE`).
    ///
    /// **GENERAL and TARGET are both checked at cast**, in that order, by
    /// `Skill.checkCondition` — the split exists for the datapack's benefit,
    /// not the engine's, and Java evaluates them back to back. PASSIVE is read
    /// by `Player.isSkillActive`-style gating instead: a passive skill whose
    /// conditions fail contributes no stat modifiers.
    ///
    /// A condition name this port doesn't implement is **not** in these lists
    /// — it is recorded by `SkillGaps` instead and the skill behaves as if it
    /// weren't declared, which is what the port did for every condition before
    /// G34 S1. See PLAN_G34_SKILL_PARITY.md §S1.
    /// Java `<basicProperty>` (390 learnable skills declare one). See
    /// [`BasicProperty`] — this is what ties a debuff into the stun-lock
    /// resistance chain.
    pub basic_property: BasicProperty,
    pub conditions: Vec<SkillCondition>,
    pub target_conditions: Vec<SkillCondition>,
    pub passive_conditions: Vec<SkillCondition>,
}

/// Java `BasicProperty` — the "mesmerizing debuff" family a skill belongs to.
///
/// Quoting Java's own enum docs (from Juji): **PHYSICAL** is Stun, Paralyze,
/// Knockback, Knock Down, Hold, Disarm, Petrify; **MAGIC** is Sleep, Mutate,
/// Fear, Aerial Yoke, Silence. Everything else is `NONE`.
///
/// Two independent mechanics read it, and conflating them is how the port
/// missed both (see [`crate::game_loop::stats::basic_property`]):
///
/// 1. `Formulas.getAbnormalResist` — a *stat* lookup
///    (`ABNORMAL_RESIST_PHYSICAL` / `_MAGICAL`), subtracted inside `baseMod`.
/// 2. `Formulas.getBasicPropertyResistBonus` — the **accrual chain**, a
///    multiplier applied *after* the min/max clamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum BasicProperty {
    #[default]
    None,
    Physical,
    Magic,
}

impl BasicProperty {
    pub fn from_xml(name: &str) -> Self {
        match name {
            "PHYSICAL" => Self::Physical,
            "MAGIC" => Self::Magic,
            _ => Self::None,
        }
    }
}

/// Java `SkillConditionPercentType` — the comparison a `Remain*Per` condition
/// makes. `MORE` is `current >= amount`, `LESS` is `current <= amount`; both
/// are inclusive, which matters for the skills that gate on exactly 100 %.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PercentType {
    More,
    Less,
}

impl PercentType {
    pub fn test(self, current: i32, amount: i32) -> bool {
        match self {
            Self::More => current >= amount,
            Self::Less => current <= amount,
        }
    }

    pub fn from_xml(name: &str) -> Self {
        match name {
            "LESS" => Self::Less,
            _ => Self::More,
        }
    }
}

/// Java `SkillConditionAffectType` — whose state a condition reads. Java's
/// `BOTH` is declared but **no handler branches on it**: every `switch` in the
/// condition handlers covers `CASTER` and `TARGET` and falls through to
/// `return false` otherwise, so a `BOTH` condition refuses the cast outright.
/// Ported as written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum AffectType {
    Both,
    #[default]
    Caster,
    Target,
}

impl AffectType {
    pub fn from_xml(name: &str) -> Self {
        match name {
            "TARGET" => Self::Target,
            "BOTH" => Self::Both,
            _ => Self::Caster,
        }
    }
}

/// Which vital a `Remain*Per` condition reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Vital {
    Hp,
    Mp,
    Cp,
}

/// Java `MountType`, as far as the mount conditions need it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MountKind {
    Strider,
    Wyvern,
}

impl Default for Skill {
    /// A blank skill: no effects, no costs, single-target, instant.
    ///
    /// Exists so struct literals can use `..Default::default()` and stop
    /// breaking every time a field is added — adding `magic_critical_rate` once
    /// churned 15 test files and was backed out partly for that reason. Only
    /// the non-zero defaults below need thought; the rest are Java's own
    /// zero/absent values.
    fn default() -> Self {
        Self {
            trait_type: TraitType::None,
            static_reuse: false,
            id: 0,
            level: 1,
            name: String::new(),
            // Java's own `getString("icon", …)` default.
            icon: String::from("icon.skill0000"),
            operate_type: OperateType::Active,
            is_continuous: false,
            target_type: TargetType::Self_,
            over_hit: false,
            abnormal_visuals: Vec::new(),
            toggle_group_id: 0,
            affect_scope: AffectScope::Single,
            affect_object: AffectObject::All,
            affect_range: 0,
            affect_limit: (0, 0),
            fan_range: [0; 4],
            magic_type: 0,
            magic_level: 0,
            // Java's "no declared rate", which several gates test for
            // explicitly (a skill with -1 always lands and is never reflected).
            activate_rate: -1,
            lvl_bonus_rate: 0,
            effect_point: 0,
            cast_range: 0,
            effect_range: 0,
            hit_time: 0,
            next_action: NextAction::None,
            abnormal_resists: Vec::new(),
            hit_cancel_time: 0.0,
            cool_time: 0,
            reuse_delay: 0,
            // Java's "no group" sentinel.
            reuse_delay_group: -1,
            mp_consume: 0,
            mp_initial_consume: 0,
            hp_consume: 0,
            without_action: false,
            is_suicide_attack: false,
            item_consume_id: 0,
            item_consume_count: 0,
            abnormal_time: 0,
            abnormal_level: 0,
            abnormal_type: "NONE".to_string(),
            can_be_dispelled: true,
            is_debuff: false,
            excluded_from_check: false,
            shared_with_summon: true,
            stay_after_death: false,
            removed_on_damage: false,
            effects: Vec::new(),
            self_continuous: false,
            self_effects: Vec::new(),
            pve_effects: Vec::new(),
            pvp_effects: Vec::new(),
            channeling_effects: Vec::new(),
            end_effects: Vec::new(),
            mp_per_channeling: 0,
            channeling_skill_id: 0,
            channeling_tick_ms: 0,
            channeling_start_ms: 0,
            basic_property: BasicProperty::default(),
            conditions: Vec::new(),
            target_conditions: Vec::new(),
            passive_conditions: Vec::new(),
            attribute_type: None,
            attribute_value: 0,
            sub_level: 0,
        }
    }
}

/// Which count-cap pool a landed buff occupies (Java `SkillBuffType`, trimmed
/// to the pools the caps use). `Uncapped` folds Java's DEBUFF/TOGGLE/TRIGGER/
/// passive types — none are limited by `MaxBuffAmount`/`MaxDanceAmount`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffSlot {
    /// A good buff — counted against `MaxBuffAmount`.
    Buff,
    /// A dance/song (`isMagic == 3`) — counted against `MaxDanceAmount`.
    Dance,
    /// Debuff / toggle / passive — not slot-limited here.
    Uncapped,
}

impl Skill {
    /// Java `Skill.isBad()`: `effectPoint < 0` (aggro/debuff/damage skills).
    pub fn is_bad(&self) -> bool {
        self.effect_point < 0
    }

    /// Java `Skill.getAffectLimit()` — the per-cast cap on how many targets a
    /// scope may sweep up, rolled fresh each cast, or 0 for "no cap".
    ///
    /// The roll is `min + Rnd.get(max)`, **not** `Rnd.get(min..=max)`: Java
    /// passes the *max* as the exclusive bound of a 0-based roll, so the
    /// dist's common `5-12` yields 5..=16, and `10-10` yields 10..=19. That
    /// reads like a datapack authoring assumption more than an intent, but it
    /// is what the live server does, so it is reproduced exactly. `roll` takes
    /// the exclusive bound, matching `World::roll`.
    pub fn affect_limit(&self, roll: impl FnOnce(i32) -> i32) -> i32 {
        let (min, max) = self.affect_limit;
        if min > 0 || max > 0 {
            min + if max > 0 { roll(max) } else { 0 }
        } else {
            0
        }
    }

    /// Java `Skill.isStatic()` — `isMagic == 2`. A static skill's cast time and
    /// reuse are fixed (no attack-speed scaling, no reuse-rate buff).
    pub fn is_static(&self) -> bool {
        self.magic_type == 2
    }

    /// Java `Skill.isDance()` — `isMagic == 3`, the dance/song pool.
    pub fn is_dance(&self) -> bool {
        self.magic_type == 3
    }

    /// Java `Skill.getBuffType()` collapsed to the [`BuffSlot`] pools: a
    /// passive/toggle or a debuff is `Uncapped`, a dance/song (`isMagic == 3`)
    /// is `Dance`, everything else is a `Buff`.
    pub fn buff_slot(&self) -> BuffSlot {
        if matches!(
            self.operate_type,
            OperateType::Passive | OperateType::Toggle
        ) || self.is_bad()
        {
            BuffSlot::Uncapped
        } else if self.magic_type == 3 {
            BuffSlot::Dance
        } else {
            BuffSlot::Buff
        }
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
    /// Java `Skill.hasEffectType(EffectType.HATE)` — whether any of this
    /// skill's effects is an aggro-management one (`DeleteHate`,
    /// `DeleteHateOfMe`, `DeleteTopAgro`).
    ///
    /// `hasEffectType` scans **every** effect scope (`_effectLists.values()`),
    /// not just `<effects>`, so this does too. The one gate that reads it is
    /// `SkillCaster.callSkill`'s `EVT_ATTACKED` notify: a skill that exists to
    /// *shed* aggro must not wake the mob it was cast at. The hate *addition*
    /// beside it (`addDamageHate(caster, 0, -effectPoint)`) is **not** gated —
    /// only the AI wake is.
    ///
    /// `DeleteTopAgro` has no port variant: its sole carrier is Mischief
    /// (10526), an off-chronicle skill no class learns.
    pub fn has_hate_effect(&self) -> bool {
        [
            &self.effects,
            &self.self_effects,
            &self.pve_effects,
            &self.pvp_effects,
            &self.channeling_effects,
        ]
        .into_iter()
        .flatten()
        .any(|e| {
            matches!(
                e,
                SkillEffect::DeleteHate { .. } | SkillEffect::DeleteHateOfMe { .. }
            )
        })
    }

    /// OR of the [`effect_flag`] bits this skill's effects contribute — Java's
    /// `AbstractEffect.getEffectFlags()` summed over the effect list.
    pub fn effect_flags(&self) -> u32 {
        self.effects.iter().fold(0, |acc, e| {
            acc | match e {
                // Java splits these into BLOCK_ACTIONS vs
                // CONDITIONAL_BLOCK_ACTIONS, but `hasBlockActions()` ORs them,
                // so a single bit is behaviourally identical here.
                SkillEffect::BlockActions { .. } => effect_flag::BLOCK_ACTIONS,
                SkillEffect::Root => effect_flag::ROOTED,
                SkillEffect::Mute => effect_flag::MUTED,
                SkillEffect::PhysicalMute => effect_flag::PHYSICAL_MUTED,
                SkillEffect::DebuffBlock => effect_flag::DEBUFF_BLOCK,
                SkillEffect::BlockControl => effect_flag::BLOCK_CONTROL,
                SkillEffect::Fear { .. } => effect_flag::FEAR,
                SkillEffect::Confuse { .. } => effect_flag::CONFUSED,
                SkillEffect::BlockMove | SkillEffect::ImmobilePetBuff => effect_flag::IMMOBILIZED,
                SkillEffect::Betray => effect_flag::BETRAYED,
                SkillEffect::BlockChat => effect_flag::CHAT_BLOCK,
                SkillEffect::ResurrectionSpecial { .. } => effect_flag::RESURRECTION_SPECIAL,
                SkillEffect::SilentMove => effect_flag::SILENT_MOVE,
                // `ChameleonRest.getEffectFlags()` returns SILENT_MOVE **and**
                // RELAXING. The stealth half is what the skill is for — resting
                // under it hides you from a monster's pre-emptive aggro — and
                // it is the half with a consumer here; `RELAXING` is read in
                // Java only by `Player.standUp`, which this port expresses
                // through `sit_stand::stop_relaxing` instead.
                SkillEffect::ChameleonRest { .. } => effect_flag::SILENT_MOVE,
                SkillEffect::FakeDeath { .. } => effect_flag::FAKE_DEATH,
                SkillEffect::NoblesseBless => effect_flag::NOBLESS_BLESSING,
                // G34 S3 — flag-only effects: the whole mechanic is the bit,
                // so `apply_skill_effects`' empty-effects guard keeps them
                // alive via `has_state_flag` and nothing else is needed.
                SkillEffect::BuffBlock => effect_flag::BUFF_BLOCK,
                SkillEffect::PhysicalShieldAngleAll => effect_flag::PHYSICAL_SHIELD_ANGLE_ALL,
                SkillEffect::Passive => effect_flag::PASSIVE,
                SkillEffect::Untargetable => effect_flag::UNTARGETABLE,
                SkillEffect::DisableTargeting => effect_flag::TARGETING_DISABLED,
                SkillEffect::PhysicalAttackMute => effect_flag::PSYCHICAL_ATTACK_MUTED,
                SkillEffect::BlockResurrection => effect_flag::BLOCK_RESURRECTION,
                SkillEffect::BlockEscape => effect_flag::CANNOT_ESCAPE,
                SkillEffect::AbnormalShield => effect_flag::ABNORMAL_SHIELD,
                SkillEffect::DamageBlock { block_hp, block_mp } => {
                    (if *block_hp { effect_flag::HP_BLOCK } else { 0 })
                        | (if *block_mp { effect_flag::MP_BLOCK } else { 0 })
                }
                _ => 0,
            }
        })
    }

    /// The abnormal types this skill blocks while active — Java
    /// `EffectList.addBlockedAbnormalTypes` on effect start.
    pub fn blocked_abnormals(&self) -> Vec<String> {
        self.effects
            .iter()
            .filter_map(|e| match e {
                SkillEffect::BlockAbnormalSlot { slots } => Some(slots.clone()),
                _ => None,
            })
            .flatten()
            .collect()
    }

    pub fn stat_modifier_effects(&self) -> Vec<StatModifierEffect> {
        let one = |stat, amount| StatModifierEffect {
            stat,
            mode: StatModifierType::Diff,
            amount,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: None,
            two_handed: false,
            hp_percent: 0,
        };
        self.effects
            .iter()
            .flat_map(|e| match e {
                SkillEffect::StatModifier(m) => vec![*m],
                // `VampiricAttack.pump` grants **two** values, which is why this
                // is a `flat_map`: the absorb percentage (Java stores
                // `amount / 100`) and the `amount · chance` term the chance
                // finalizer divides back out.
                // `PolearmSingleTarget.onStart` is `addFixedValue(stat, 1.0)`
                // and `onExit` removes it. Expressed as an ordinary additive 1
                // so it rides the buff lifecycle that already merges and
                // unmerges every other stat grant — nothing else on this dist
                // touches the stat, so `fixed` and `add` are indistinguishable
                // at the one read site (`> 0`).
                SkillEffect::PolearmSingleTarget => {
                    vec![one(Stat::PhysicalPolearmTargetSingle, 1.0)]
                }
                // `ReduceDropPenalty.pump` merges a **mul**, not a diff — the
                // parser has already turned `amount` into `amount/100 + 1`.
                SkillEffect::ReduceDropPenalty { exp_mul, kind } => vec![StatModifierEffect {
                    stat: match kind {
                        ReduceDropKind::Mob => Stat::ReduceExpLostByMob,
                        ReduceDropKind::Pk => Stat::ReduceExpLostByPvp,
                        ReduceDropKind::Raid => Stat::ReduceExpLostByRaid,
                    },
                    mode: StatModifierType::Per,
                    amount: (exp_mul - 1.0) * 100.0,
                    ..Default::default()
                }],
                SkillEffect::VampiricAttack { amount, chance } => vec![
                    one(Stat::AbsorbDamagePercent, amount / 100.0),
                    one(Stat::VampiricSum, amount * chance),
                ],
                // `ReflectSkill.pump` is `mergeAdd(stat, amount)` — an ordinary
                // additive stat contribution that happens to have its own
                // handler class in Java rather than being an
                // `AbstractStatEffect`. Expressed here as the equivalent
                // `StatModifierEffect` so it rides the existing buff/passive
                // pipeline instead of needing its own plumbing.
                // `DamageShield`/`VampiricAttack` are the same shape: Java
                // handlers that only `pump` additive stats.
                SkillEffect::DamageShield { amount } => {
                    vec![one(Stat::ReflectDamagePercent, *amount)]
                }
                SkillEffect::ReflectSkill { magic, amount } => vec![one(
                    if *magic {
                        Stat::ReflectSkillMagic
                    } else {
                        Stat::ReflectSkillPhysic
                    },
                    *amount,
                )],
                _ => Vec::new(),
            })
            .collect()
    }
}
