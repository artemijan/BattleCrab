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
    /// `NPC_BODY`: a dead NPC corpse (Java `targethandlers/NpcBody.java`) —
    /// used by corpse skills (Sweeper). Unlike the other types this requires
    /// the target to be **dead**, so the cast pipeline's "no dead targets"
    /// gate is inverted for it.
    NpcBody,
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
    /// `handlers/effecthandlers/PhysicalAttack.java` — instant physical skill
    /// damage (`77·((pAtk·pAtkMod)·levelMod + power) / (pDef·pDefMod)`, crit ×2,
    /// soulshot ×2). Also backs `PhysicalSoulAttack` (identical formula; its
    /// soul mAtk-style boost is ×1 until charges are modeled). The dagger-blow
    /// skills (`FatalBlow`/`Backstab`/`SoulBlow`) use a different `calcBlowDamage`
    /// formula and are NOT routed here.
    /// TODO(G20): ranged (bow) weaponMod 70 branch; shield-block `pDef` add.
    PhysicalAttack { power: f64, p_atk_mod: f64, p_def_mod: f64, critical_chance: f64 },
    /// `handlers/effecthandlers/Heal.java` — instant HP restore.
    Heal { power: f64 },
    /// Dagger blow skills (`FatalBlow`/`Backstab`/`SoulBlow`) — instant physical
    /// damage via `Formulas.calcBlowDamage`, gated by a `calcBlowSuccess` land
    /// roll (blows can miss). `critical_chance` is `Some` for FatalBlow/Backstab
    /// (rolls `calcCrit` to double the hit) and `None` for SoulBlow (whose
    /// charged-soul boost is ×1 until charges land). `backstab` requires the
    /// caster to be outside the target's front arc.
    /// TODO(G20): SoulBlow charged-soul boost; the accompanying `Lethal`
    /// instant-kill effect is still dropped.
    Blow { power: f64, chance_boost: f64, critical_chance: Option<f64>, backstab: bool },
    /// `handlers/effecthandlers/HpDrain.java` — magic damage (same
    /// `calcMagicDam` core as `MagicalAttack`) that also heals the caster by
    /// `percentage`% of the HP actually drained (CP absorbs first, clamped to
    /// the target's remaining HP). Backs Vampiric Touch/Claw.
    HpDrain { power: f64, percentage: f64 },
    /// `handlers/effecthandlers/DamOverTime.java` — a poison/bleed damage-over-
    /// time debuff. Lands as an `ActiveBuff` for `abnormalTime` and ticks every
    /// `ticks * EFFECT_TICK_RATIO` ms (Java `BuffInfo.scheduleEffects`) for
    /// `power * ticks * EFFECT_TICK_RATIO / 1000` damage per tick
    /// (`AbstractEffect.getTicksMultiplier`), stopping when the buff expires or
    /// the target dies. `can_kill == false` (the XML default) clamps each tick
    /// so it leaves the target at 1 HP. Backs Curse Poison (1168), Poison,
    /// Bleed, etc.
    DamOverTime { power: f64, ticks: i32, can_kill: bool },
    /// `handlers/effecthandlers/Restoration.java` — instant single-item
    /// grant. Backs item-use skills wrapping a fixed pack/box reward (e.g.
    /// spiritshot packs): the item's `<skills>` entry casts this, which is
    /// where the actual reward comes from.
    GiveItem { item_id: i32, item_count: i64, item_enchant_level: i32 },
    /// `handlers/effecthandlers/RestorationRandom.java` — one weighted
    /// roulette pick among reward groups (each group can grant multiple
    /// items at once). Used by "pick one of N" reward boxes.
    GiveItemRandom { groups: Vec<RestorationGroup> },
    /// `handlers/effecthandlers/Escape.java`, `escapeType=TOWN` only — the
    /// `/unstuck` skills (2099/2100) and scrolls of escape: teleport the
    /// target to its map-region town respawn on landing. The CASTLE/CLANHALL/
    /// FORTRESS variants wait for their residence systems (G24).
    EscapeToTown,
    /// `handlers/effecthandlers/GiveRecommendation.java` — grant the target
    /// `amount` recommendations received (`rec_have`), capped at 255. Backs the
    /// "recommendation certificate" self-target skills.
    GiveRecommendation { amount: i32 },
    /// `handlers/effecthandlers/HeadquarterCreate.java` — the "Build
    /// Headquarters" siege skill (247): the caster (an attacker clan leader)
    /// plants an HQ flag (NPC 35062) in the siege zone as a respawn point.
    /// (`isAdvanced` — the advanced HQ's extra abilities — is collapsed for now,
    /// TODO(G24).)
    CreateHeadquarter,
    /// `handlers/effecthandlers/Spoil.java` — marks a live monster as spoiled
    /// (`Attackable.setSpoilerObjectId`) so its `<spoil>` list rolls into sweep
    /// loot on death. Gated by `calcSuccess` = `Formulas.calcMagicSuccess`, and
    /// wakes the mob's AI (`EVT_ATTACKED`). Instant.
    Spoil,
    /// `handlers/effecthandlers/Sweeper.java` — on a dead, spoiled corpse the
    /// caster owns (or is in the spoiler's looter party), hands out the loot
    /// rolled at death (`takeSweep`), solo or party-distributed. Instant.
    Sweeper,
    /// `handlers/effecthandlers/ConsumeBody.java` — decays the targeted corpse
    /// immediately (`Npc.endDecayTask`). Paired with `Sweeper` on skill 42 so
    /// the swept body vanishes at once. Instant.
    ConsumeBody,
    /// `handlers/effecthandlers/DispelBySlot.java` — instant cleanse. Stops
    /// every active buff/debuff whose originating skill's `<abnormalType>` is in
    /// the dispel set, provided the listed level is negative (dispel all levels)
    /// or `>=` the buff skill's own `abnormalLevel`. Each `(abnormal_type, level)`
    /// pair comes from the `<dispel>` string (`"POISON,3"`), which is per-skill-
    /// level. Backs Cure Poison (1012), Cure Bleeding, etc. Java's special-cased
    /// `AbnormalType.TRANSFORM` branch is omitted — no transforms in scope yet.
    DispelBySlot { dispel: Vec<(String, i32)> },
    /// `handlers/effecthandlers/ProtectionBlessing.java` — the Newbie Helper's
    /// Blessing of Protection (5182): a chaotic (PK) character 10+ levels above
    /// the target cannot damage or be damaged by them. Carries no stat
    /// modifier, so it lands as an icon-only timed `ActiveBuff` (like a bare
    /// `DamOverTime`) — the `PK_PROTECT` abnormal + 7200 s duration are honored,
    /// but the actual damage-immunity check is deferred to the PvP milestone.
    /// TODO(G-pvp): gate PK damage on this buff in the combat/flag path.
    ProtectionBlessing,
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
    /// Java `affectScope`, narrowed to the one distinction the cast pipeline
    /// needs so far: `true` for `SINGLE` (the default when absent), `false` for
    /// any area scope (`RANGE`/`SQUARE`/`FAN`/…). Used to gate the per-target
    /// debuff-percentage message to single-target debuffs only.
    pub single_target: bool,
    pub effects: Vec<SkillEffect>,
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

    /// Java `Skill.getBuffType()` collapsed to the [`BuffSlot`] pools: a
    /// passive/toggle or a debuff is `Uncapped`, a dance/song (`isMagic == 3`)
    /// is `Dance`, everything else is a `Buff`.
    pub fn buff_slot(&self) -> BuffSlot {
        if matches!(self.operate_type, OperateType::Passive | OperateType::Toggle) || self.is_bad() {
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
    /// Java `Skill.getAbnormalType()` ("NONE" when unset) — the stacking key:
    /// effects of the same abnormal type don't stack (`EffectList.addActive`).
    pub abnormal_type: String,
    /// Java `Skill.getAbnormalLevel()` — decides which of two same-type buffs
    /// wins (the higher level overrides; a lower one is refused).
    pub abnormal_level: i32,
    /// Which slot pool this occupies for the count caps (`MaxBuffAmount` /
    /// `MaxDanceAmount`); debuffs/toggles/passives are `Uncapped`.
    pub slot: BuffSlot,
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

// The debuff landing-chance formula is unit-tested in `formulas.rs`
// (`effect_land_rate_clamps_and_special_cases`); the caster-facing chance line
// and the resist roll have end-to-end tests in `game_loop::tests::skills_tests`
// (`single_target_debuff_lands_and_reports_chance` /
// `single_target_debuff_resisted_leaves_target_and_reports`).
