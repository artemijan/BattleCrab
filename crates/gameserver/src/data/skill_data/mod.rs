//! Port of `data/xml/SkillData` — a generic per-level-value parser for
//! `dist/game/data/stats/skills/*.xml`, scoped to the fields G6's cast
//! pipeline reads (targeting/timing/costs/abnormal info) plus a curated
//! `<effect name>` → `Stat` registry (see module docs on `model::skill`).
//!
//! Every child of `<skill>` follows the same two shapes Java's loader
//! handles generically: a bare scalar (`<castRange>40</castRange>`, applies
//! to every level) or a per-level table (`<mpConsume><value level="1">4</value>
//! <value level="2">8</value></mpConsume>`). `<effects><effect name="X">…`
//! follows the same shape one level deeper for its `<amount>` child, plus a
//! scalar `<mode>DIFF|PER</mode>`. `<conditions>` and any other field this
//! milestone doesn't need are parsed (to keep the reader positioned
//! correctly) and discarded.
//!
//! Layout — the pieces are re-exported here, so callers keep saying
//! `skill_data::…`:
//!
//! - `parse` — `parse_str`: XML → the `Parsed*` staging types (which live in
//!   this file, so both children reach their private fields).
//! - `build` — `build_skill`: one staged skill + a level/sub-level → the
//!   finished `Skill`, resolving the per-level tables and effect params.
//! - `tests` — the test module, unchanged. The coverage census that measures
//!   effect coverage against the real dist files lives with the datapack
//!   tooling: `crates/tools/tests/coverage_census.rs`.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::{info, warn};

use crate::model::skill::{
    AffectObject, AffectScope, BasicProperty, CompanionKind, DispelSlot, EscapeDest, OperateType,
    ResidenceType, RestorationGroup, RestorationItem, Skill, SkillCondition, SkillEffect,
    StatModifierEffect, TargetType,
};
use crate::model::stats::{Stat, StatModifierType};

mod build;
mod parse;

pub(crate) use build::*;
pub(crate) use parse::*;

pub const SKILLS_DIR: &str = "data/stats/skills";

/// `<effect name="X">` → the `Stat` it modifies (Java: the concrete effect
/// class name, e.g. `PAtk.java` → `Stat.PHYSICAL_ATTACK`). Only the handful of
/// generic `AbstractStatEffect`-style modifiers live here; the
/// non-stat-modifier effect kinds grew into `build.rs`'s dedicated arms over
/// G9–G34, and what remains unregistered is inventoried by the coverage
/// census (`deferral_markers…`'s sibling tests), not by this comment.
/// Java `Fear.getTicks()` — hard-coded, not a datapack param.
pub(crate) const FEAR_TICKS: i32 = 5;

/// The `<effect name>` → [`Stat`] lookup behind [`EFFECT_REGISTRY`], shared with
/// the augment-option loader (`data/stats/augmentation/options/*` uses the same
/// effect names as skills).
pub fn stat_for_effect_name(name: &str) -> Option<Stat> {
    EFFECT_REGISTRY
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, s)| *s)
}

pub(crate) const EFFECT_REGISTRY: &[(&str, Stat)] = &[
    ("PAtk", Stat::PhysicalAttack),
    ("PhysicalDefence", Stat::PhysicalDefence),
    ("MAtk", Stat::MagicalAttack),
    ("MagicalDefence", Stat::MagicalDefence),
    ("PhysicalAttackSpeed", Stat::PhysicalAttackSpeed),
    ("MagicalAttackSpeed", Stat::MagicAttackSpeed),
    ("CriticalRate", Stat::CriticalRate),
    ("MagicCriticalRate", Stat::MagicCriticalRate),
    ("PhysicalEvasion", Stat::EvasionRate),
    ("MagicalEvasion", Stat::MagicEvasionRate),
    ("Accuracy", Stat::AccuracyCombat),
    ("MagicAccuracy", Stat::AccuracyMagic),
    ("MaxHp", Stat::MaxHp),
    ("MaxMp", Stat::MaxMp),
    ("MaxCp", Stat::MaxCp),
    ("HpRegen", Stat::RegenerateHpRate),
    ("MpRegen", Stat::RegenerateMpRate),
    ("CpRegen", Stat::RegenerateCpRate),
    // Concentration (1078) and the like: `ReduceCancel` → `Stat.ATTACK_CANCEL`.
    // Without this the effect fell through, produced no stat modifier, and the
    // buff was dropped whole at `apply_skill_effects`' empty-effects guard — so
    // the buff never landed (the community-board "Concentration doesn't work").
    ("ReduceCancel", Stat::AttackCancel),
    // Blessed Shield (1243): `ShieldDefenceRate` → `Stat.SHIELD_DEFENCE_RATE`
    // (single-stat `AbstractStatEffect`). Without this the effect fell through,
    // produced no modifier, and the buff was dropped whole at the empty-effects
    // guard (community-board "Blessed Shield doesn't apply"). `CriticalDamage`
    // is NOT here — it is two-stat (mul/add by mode), handled in a match arm.
    ("ShieldDefenceRate", Stat::ShieldDefenceRate),
    // Shield Mastery (153, a widely-learned shield-user passive), Shield
    // Fortress (322), Knighthood (429), …: `ShieldDefence` →
    // `Stat.SHIELD_DEFENCE` (single-stat `AbstractStatEffect`), folded into
    // `shield_stats` alongside `ShieldDefenceRate` above.
    ("ShieldDefence", Stat::ShieldDefence),
    // Archery 431/Long Shot 113/Rapid Fire 413/Snipe 972: `PhysicalAttackRange`
    // → `Stat.PHYSICAL_ATTACK_RANGE` (single-stat `AbstractStatEffect`, all
    // four `<weaponType>BOW</weaponType>`-conditioned — the condition mask is
    // already generic, read off `armor_condition`/`weapon_condition` like
    // every other registry entry). Folded into `recalculate_stats`'
    // `combat.atk_range` line, which previously read the equipped weapon's
    // raw range directly with no stat modifier applied at all.
    ("PhysicalAttackRange", Stat::PhysicalAttackRange),
    // Focus Death 355/Critical Blow 409/Mortal Strike 410/Assassination 432
    // (all `PER`): `FatalBlowRate` → `Stat.BLOW_RATE` (single-stat
    // `AbstractStatEffect`, multiplicative). Folded into
    // `formulas::calc_blow_success`, which previously had no term for it at
    // all — `BLOW_RATE` was identity 1.0 no matter what a caster had learned.
    ("FatalBlowRate", Stat::BlowRate),
    // Higher Mana Gain (285), a learnable passive: `ManaCharge` →
    // `Stat.MANA_CHARGE`, a flat bonus the recharge skills read off their
    // *recipient*. A plain single-stat `AbstractStatEffect`, so the generic
    // registry wiring is all it needs.
    ("ManaCharge", Stat::ManaCharge),
    // Anti Magic (146), M. Def. (147): `ResistDDMagic` →
    // `Stat.MAGIC_SUCCESS_RES`. An `AbstractStatPercentEffect`, so it is always
    // `PER` and the generic registry wiring (which honours the effect's own
    // `<mode>`, `PER` here) is all it needs.
    ("ResistDDMagic", Stat::MagicSuccessRes),
    // `PhysicalAbnormalResist` / `MagicalAbnormalResist` — both plain
    // `AbstractStatAddEffect`s over `Stat.ABNORMAL_RESIST_{PHYSICAL,MAGICAL}`,
    // which `Formulas.getAbnormalResist` subtracts from a mesmerizing debuff's
    // base landing chance (G34 S2, `game_loop::basic_property`). No learnable
    // source on this dist — 3 items each — but the consumer exists now, so the
    // registry entry is what makes those items real rather than inert.
    // G34 S4 — `Breath` (Boost Breath 195, Eva's Kiss 1073 + 19 Doom-set item
    // skills), consumed by `game_loop::water`'s breath gauge. Note the two
    // modes read very differently against the 60 000 ms base: Eva's Kiss is
    // `PER 400` (×5, five minutes), Boost Breath is `DIFF 180` (+0.18 s).
    // The second looks like a datapack unit slip, but Java computes exactly
    // that, so it is ported as written ([[l2r-port-behaviour-not-intent]]).
    // G34 S4 sub-slice 2 — the skill-damage multipliers. Each is consumed
    // where Java reads it: the two `*SkillPower`s multiply a skill's finished
    // damage, the crit pair replaces the flat 2.0 in `crit_damage_skill`'s
    // physical branch.
    // G34 S4 sub-slice 4 — the mitigation/counter family.
    ("SkillMasteryRate", Stat::SkillMasteryRate),
    // `CubicMastery` → `Stat.MAX_CUBIC`, which **nothing in Java reads** (the
    // cubic limit is `Config.ALLOWED_CUBIC_COUNT`). Registered so the skill
    // parses rather than losing its buff to the empty-effects guard; there is
    // deliberately no consumer, because Java has none either.
    ("CubicMastery", Stat::MaxCubic),
    ("LimitHp", Stat::MaxRecoverableHp),
    // The PvP/PvE balance family — every one an `AbstractStatPercentEffect`,
    // all consumed by `formulas::calculate_pvp_pve_bonus`.
    (
        "PvpPhysicalAttackDamageBonus",
        Stat::PvpPhysicalAttackDamage,
    ),
    (
        "PvpPhysicalAttackDefenceBonus",
        Stat::PvpPhysicalAttackDefence,
    ),
    ("PvpPhysicalSkillDamageBonus", Stat::PvpPhysicalSkillDamage),
    (
        "PvpPhysicalSkillDefenceBonus",
        Stat::PvpPhysicalSkillDefence,
    ),
    ("PvpMagicalSkillDamageBonus", Stat::PvpMagicalSkillDamage),
    ("PvpMagicalSkillDefenceBonus", Stat::PvpMagicalSkillDefence),
    (
        "PvePhysicalAttackDamageBonus",
        Stat::PvePhysicalAttackDamage,
    ),
    (
        "PvePhysicalAttackDefenceBonus",
        Stat::PvePhysicalAttackDefence,
    ),
    ("PvePhysicalSkillDamageBonus", Stat::PvePhysicalSkillDamage),
    (
        "PvePhysicalSkillDefenceBonus",
        Stat::PvePhysicalSkillDefence,
    ),
    ("PveMagicalSkillDamageBonus", Stat::PveMagicalSkillDamage),
    ("PveMagicalSkillDefenceBonus", Stat::PveMagicalSkillDefence),
    (
        "PveRaidPhysicalAttackDefenceBonus",
        Stat::PveRaidPhysicalAttackDefence,
    ),
    (
        "PveRaidPhysicalSkillDefenceBonus",
        Stat::PveRaidPhysicalSkillDefence,
    ),
    (
        "PveRaidMagicalSkillDefenceBonus",
        Stat::PveRaidMagicalSkillDefence,
    ),
    ("LimitCp", Stat::MaxRecoverableCp),
    ("AreaDamage", Stat::DamageZoneVuln),
    ("TransferDamageToSummon", Stat::TransferDamageSummonPercent),
    ("CounterPhysicalSkill", Stat::VengeanceSkillPhysicalDamage),
    ("HateAttack", Stat::HateAttack),
    ("PhysicalSkillPower", Stat::PhysicalSkillPower),
    ("MagicalSkillPower", Stat::MagicalSkillPower),
    (
        "PhysicalSkillCriticalDamage",
        Stat::PhysicalSkillCriticalDamage,
    ),
    (
        "DefencePhysicalSkillCriticalDamage",
        Stat::DefencePhysicalSkillCriticalDamage,
    ),
    ("Breath", Stat::Breath),
    // `WeightLimit` (Weight Limit 150, Quiver of Holding 418, Super Haste 7029)
    // and `WeightPenalty` (Decrease Weight 1257, Master's Blessing 7049).
    ("WeightLimit", Stat::WeightLimit),
    ("WeightPenalty", Stat::WeightPenalty),
    ("PhysicalAbnormalResist", Stat::AbnormalResistPhysical),
    ("MagicalAbnormalResist", Stat::AbnormalResistMagical),
];

/// One category of "the datapack said something this parser doesn't act on",
/// as `xml name → the skill ids that declared it`.
pub type GapMap = BTreeMap<String, BTreeSet<i32>>;

/// **The G34 coverage harness** (PLAN_G34_SKILL_PARITY.md §S0).
///
/// The skill parser is *fail-open* by design: an `<effect name>` it doesn't
/// recognise yields no [`SkillEffect`], an effect list that ends up empty is
/// then dropped by `apply_skill_effects`' empty-effects guard, and a
/// `<condition>` it doesn't recognise is simply not enforced. The skill still
/// loads, still casts, still burns MP and reuse — and does nothing, or fires
/// where Java would have refused it. Nothing on the Rust side says so.
///
/// This records what was dropped, per category, while parsing. Two consumers:
/// [`SkillData::load_from`] logs a summary at boot, and the coverage census in
/// the `tools` crate (`crates/tools/tests/coverage_census.rs`) asserts the
/// exact set so the gap can only ever shrink deliberately.
///
/// **This is a record of what the *parser* dropped, not of what the datapack
/// contains.** A name that appears here is unhandled; a name that does *not*
/// appear is only "recognised", which is not the same as "correctly ported" —
/// an effect can resolve to a `SkillEffect` variant that nothing downstream
/// consumes ([[l2r-skill-rate-stats]]). Absence from this list is not evidence.
#[derive(Default, Debug)]
pub struct SkillGaps {
    /// `<effect name>`s that matched neither a handler arm nor
    /// [`EFFECT_REGISTRY`], in a scope this port builds. Includes names that
    /// are handled *conditionally* and fell through the condition — e.g.
    /// `Escape` with a non-`TOWN` `escapeType` — which is correct: those are
    /// genuinely unported.
    pub effects: GapMap,
    /// Effects declared in an `<*Effects>` block this port does not build at
    /// all (`startEffects`/`endEffects` — Java's `START`/`END` scopes, which
    /// hang off lifecycle hooks that don't exist here). Keyed by
    /// `"<block>/<effect name>"`, since the *scope* is the reason it was
    /// dropped, not the name.
    pub effect_scopes: GapMap,
    /// `<condition name>`s under `<conditions>`, `<targetConditions>` or
    /// `<passiveConditions>`, keyed `"<block>/<name>"`. Everything except
    /// `conditions/OpExistNpc` is unenforced today.
    pub conditions: GapMap,
    /// `<targetType>` values that fell to [`TargetType::Other`].
    pub target_types: GapMap,
    /// `<affectScope>` values that fell to [`AffectScope::Other`].
    pub affect_scopes: GapMap,
    /// `<affectObject>` values that fell to [`AffectObject::Other`].
    pub affect_objects: GapMap,
    /// `<operateType>` values that fell to [`OperateType::Other`].
    pub operate_types: GapMap,
}

impl SkillGaps {
    /// `map[name] += id`, without allocating a key when the name is already
    /// known — this runs once per (skill, level, sub, effect) over the whole
    /// datapack.
    fn record(map: &mut GapMap, name: &str, skill_id: i32) {
        if let Some(ids) = map.get_mut(name) {
            ids.insert(skill_id);
        } else {
            map.insert(name.to_owned(), BTreeSet::from([skill_id]));
        }
    }

    /// The seven categories as `(label, map)`, in report order.
    pub fn categories(&self) -> [(&'static str, &GapMap); 7] {
        [
            ("effect", &self.effects),
            ("effect-scope", &self.effect_scopes),
            ("condition", &self.conditions),
            ("targetType", &self.target_types),
            ("affectScope", &self.affect_scopes),
            ("affectObject", &self.affect_objects),
            ("operateType", &self.operate_types),
        ]
    }
}

pub struct SkillData {
    skills: HashMap<(i32, i32), Skill>,
    /// The enchanted variants, keyed `(id, level, subLevel)` — Java pre-builds
    /// one `Skill` per declared sub-level (routes 1001–1020 / 2001–2020 /
    /// 3001–3020) exactly like this (PLAN_G19_SKILL_ENCHANT.md).
    enchanted: HashMap<(i32, i32, i32), Skill>,
    /// `EnchantSkillGroupsData`'s route map: which sub-level ranges each
    /// `(id, level)` can enchant into. Non-empty = `Skill.isEnchantable()`.
    routes: HashMap<(i32, i32), Vec<(i32, i32)>>,
    /// `Skill.getName()` — `<skill name="…">`, keyed by **id alone**: the name
    /// sits on the `<skill>` element, so every level and enchant sub-level of
    /// that skill shares it. Java hangs a copy off each `Skill` instance;
    /// one entry per id is the same data for a fraction of the strings. Read
    /// by the packets that name a skill back to the player.
    names: HashMap<i32, String>,
    /// What the parse dropped — see [`SkillGaps`].
    gaps: SkillGaps,
}

/// The maps one parse pass fills (skills + enchanted variants + routes + the
/// per-id names), plus the coverage record of everything it had to drop.
#[derive(Default)]
pub(crate) struct ParsedSkills {
    pub(crate) skills: HashMap<(i32, i32), Skill>,
    pub(crate) enchanted: HashMap<(i32, i32, i32), Skill>,
    pub(crate) routes: HashMap<(i32, i32), Vec<(i32, i32)>>,
    pub(crate) names: HashMap<i32, String>,
    /// `RefCell` because the recording sites sit inside `build_skill`'s
    /// `build_scope` closure, which is called once per effect scope and would
    /// otherwise have to become `FnMut` and fight the borrow checker for a
    /// diagnostic that must not shape the parser's structure.
    pub(crate) gaps: RefCell<SkillGaps>,
}

impl SkillData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut out = ParsedSkills::default();
        if let Ok(dir) = std::fs::read_dir(format!("{file_path}{SKILLS_DIR}")) {
            for entry in dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("xml") {
                    continue;
                }
                parse_file(&path, &mut out);
            }
        }
        info!(
            "SkillData: Loaded {} skill levels (+{} enchanted variants, {} enchantable).",
            out.skills.len(),
            out.enchanted.len(),
            out.routes.len()
        );
        let gaps = out.gaps.into_inner();
        log_gaps(&gaps);
        Self {
            skills: out.skills,
            enchanted: out.enchanted,
            routes: out.routes,
            names: out.names,
            gaps,
        }
    }

    /// Java `Skill.getName()` — the datapack name of a skill id ("Wind
    /// Strike"), for the messages that quote it back to the player. `None`
    /// for an id that never parsed.
    pub fn name(&self, id: i32) -> Option<&str> {
        self.names.get(&id).map(String::as_str)
    }

    /// What the parse dropped — see [`SkillGaps`].
    pub fn gaps(&self) -> &SkillGaps {
        &self.gaps
    }

    pub fn get(&self, id: i32, level: i32) -> Option<&Skill> {
        self.skills.get(&(id, level))
    }

    /// The skill at an enchant sub-level (`sub <= 0` = the plain skill).
    pub fn get_enchanted(&self, id: i32, level: i32, sub: i32) -> Option<&Skill> {
        if sub <= 0 {
            self.get(id, level)
        } else {
            self.enchanted.get(&(id, level, sub))
        }
    }

    /// The enchant routes available to `(id, level)` as `(first, last)`
    /// sub-level bounds — empty for a non-enchantable skill.
    pub fn enchant_routes(&self, id: i32, level: i32) -> &[(i32, i32)] {
        self.routes
            .get(&(id, level))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Java `SkillData.getMaxLevel(id)` — the highest loaded level for a skill
    /// id (0 if the id is unknown). Used by `//cast` when no level is given.
    pub fn max_level(&self, id: i32) -> i32 {
        self.skills
            .keys()
            .filter(|(sid, _)| *sid == id)
            .map(|(_, lvl)| *lvl)
            .max()
            .unwrap_or(0)
    }

    /// Java `Skill` constructor: `EnableModifySkillDuration` + `SkillDurationList`.
    /// When enabled, override a skill's `abnormalTime` from the config list — for
    /// ordinary levels (`< 100` or `> 140`) the config value replaces the XML
    /// time; for enchanted levels (`100..140`) it is *added* to the base. Toggles
    /// (`operateType=T`) are exempt. Applied once at boot (`main.rs`) so every
    /// downstream reader of `abnormal_time` (buff expiry, DoT ticks) sees it.
    pub fn apply_skill_duration_list(&mut self, list: &HashMap<i32, i32>) {
        if list.is_empty() {
            return;
        }
        for skill in self.skills.values_mut() {
            if skill.operate_type == OperateType::Toggle {
                continue;
            }
            if let Some(&secs) = list.get(&skill.id) {
                if skill.level < 100 || skill.level > 140 {
                    skill.abnormal_time = secs;
                } else {
                    skill.abnormal_time += secs;
                }
            }
        }
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self {
            skills: HashMap::new(),
            enchanted: HashMap::new(),
            routes: HashMap::new(),
            names: HashMap::new(),
            gaps: SkillGaps::default(),
        }
    }

    #[doc(hidden)]
    pub fn insert_for_test(&mut self, skill: Skill) {
        self.skills.insert((skill.id, skill.level), skill);
    }

    #[doc(hidden)]
    pub fn insert_name_for_test(&mut self, id: i32, name: &str) {
        self.names.insert(id, name.to_string());
    }

    #[doc(hidden)]
    pub fn insert_enchanted_for_test(&mut self, skill: Skill) {
        self.enchanted
            .insert((skill.id, skill.level, skill.sub_level), skill);
    }

    #[doc(hidden)]
    pub fn insert_route_for_test(&mut self, id: i32, level: i32, range: (i32, i32)) {
        self.routes.entry((id, level)).or_default().push(range);
    }
}

/// A field's per-level values, keyed by level; `0` is the "applies to every
/// level" sentinel used for bare scalars.
pub(crate) type LeveledValues = HashMap<String, HashMap<i32, String>>;

/// Look up `field` at `level`, falling back to the scalar (level 0) entry.
pub(crate) fn value_at<'a>(values: &'a LeveledValues, field: &str, level: i32) -> Option<&'a str> {
    let table = values.get(field)?;
    table
        .get(&level)
        .or_else(|| table.get(&0))
        .map(String::as_str)
}

/// The `<magicType>` **param of an effect** — Java's
/// `params.getInt("magicType", 0)`, the bucket a `MagicMpCost`/`Reuse` rate
/// applies to. Not to be confused with the *skill's* `<isMagic>`, which is
/// what picks the bucket at cast time.
pub(crate) fn effect_magic_type(values: &LeveledValues, level: i32) -> i32 {
    value_at(values, "magicType", level)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

fn parse_file(path: &std::path::Path, out: &mut ParsedSkills) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    parse_str(&content, out);
}

/// The level-range attributes on an `<effect>` element, with Java's defaulting:
/// `level` supplies the default for both `fromLevel` and `toLevel`, and
/// `subLevel` for both sub-level bounds.
pub(crate) fn effect_level_attrs(
    e: &quick_xml::events::BytesStart,
) -> (Option<i32>, Option<i32>, Option<i32>, Option<i32>) {
    let level = attr_i32(e, b"level");
    let sub_level = attr_i32(e, b"subLevel");
    (
        attr_i32(e, b"fromLevel").or(level),
        attr_i32(e, b"toLevel").or(level),
        attr_i32(e, b"fromSubLevel").or(sub_level),
        attr_i32(e, b"toSubLevel").or(sub_level),
    )
}

/// One `<value fromLevel toLevel [fromSubLevel toSubLevel]>` ranged row —
/// held raw and resolved per (level, sub) at finalize, where the `{…}`
/// expression variables (`base`/`index`/`subIndex`) are known.
#[derive(Debug, Clone)]
pub(crate) struct RangedRow {
    from_level: i32,
    to_level: i32,
    /// 0 for a plain per-level row; ≥ 1001 for an enchant-route row.
    from_sub: i32,
    to_sub: i32,
    text: String,
}

/// The ranged bounds off a `<value>` tag, `None` for a plain `level=`-keyed
/// (or bare) row. `fromLevel` defaults to 1 and `toLevel` to "all levels",
/// matching Java's `parseValues` defaults.
pub(crate) fn ranged_bounds(e: &quick_xml::events::BytesStart) -> Option<RangedRow> {
    let from_level = attr_i32(e, b"fromLevel");
    let from_sub = attr_i32(e, b"fromSubLevel");
    if from_level.is_none() && from_sub.is_none() {
        return None;
    }
    Some(RangedRow {
        from_level: from_level.unwrap_or(1),
        to_level: attr_i32(e, b"toLevel").unwrap_or(i32::MAX),
        from_sub: from_sub.unwrap_or(0),
        to_sub: attr_i32(e, b"toSubLevel").or(from_sub).unwrap_or(0),
        text: String::new(),
    })
}

/// Coverage record for one `<condition name>` (PLAN_G34 §S0), keyed
/// `"<block>/<name>"`.
///
/// Called **only when [`build_condition`] returned `None`** — i.e. the name has
/// no [`SkillCondition`] variant and the condition will therefore not be
/// enforced. Tying the record to the builder rather than to the parse is what
/// makes the census shrink when a condition lands: there is no second list of
/// "ported names" to keep in step.
pub(crate) fn record_unported_condition(gaps: &mut SkillGaps, c: &ParsedCondition, skill_id: i32) {
    let block = match c.scope {
        CondScope::General => "conditions",
        CondScope::Target => "targetConditions",
        CondScope::Passive => "passiveConditions",
    };
    SkillGaps::record(
        &mut gaps.conditions,
        &format!("{block}/{}", c.name),
        skill_id,
    );
}

/// Coverage record for an effect declared in an `<*Effects>` block this port
/// never builds (`startEffects`/`endEffects`). The name may well be handled
/// elsewhere — it is the *scope* that drops it here, so this is its own
/// category rather than a false entry in [`SkillGaps::effects`].
pub(crate) fn record_dropped_scope(
    gaps: &mut SkillGaps,
    block: &str,
    name: Option<&str>,
    skill_id: i32,
) {
    let Some(name) = name else { return };
    SkillGaps::record(
        &mut gaps.effect_scopes,
        &format!("{block}/{name}"),
        skill_id,
    );
}

/// Which `<*Effects>` block an effect was declared in — Java `EffectScope`.
///
/// `END` feeds `Skill.end_effects`, fired by `handle_buff_expire` — Java's
/// `EffectList` runs `applyEffectScope(EffectScope.END, …)` as the last thing
/// it does when a buff comes off. Anchor (1170) is the learnable carrier: the
/// first stage holds the body rigid, the end-effect fires skill 6091 for the
/// paralysis its own description promises.
///
/// `START` is still parsed as [`Self::Other`] and dropped — no reachable skill
/// on this dist declares one. `CHANNELING` feeds `Skill.channeling_effects`,
/// applied per `ChannelingTick` (PLAN_G19_GROUND_CHANNELING.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EffectScope {
    General,
    SelfScope,
    Pve,
    Pvp,
    Channeling,
    End,
    Other,
}

impl EffectScope {
    fn from_xml(node: &str) -> Self {
        match node {
            "effects" => Self::General,
            "selfEffects" => Self::SelfScope,
            "pveEffects" => Self::Pve,
            "pvpEffects" => Self::Pvp,
            "channelingEffects" => Self::Channeling,
            "endEffects" => Self::End,
            _ => Self::Other,
        }
    }
}

/// Java `SkillConditionScope` — which `<*onditions>` block a condition was
/// declared in. Java keys `Skill._conditionLists` by this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CondScope {
    General,
    Target,
    Passive,
}

impl CondScope {
    fn from_xml(node: &str) -> Option<Self> {
        match node {
            "conditions" => Some(Self::General),
            "targetConditions" => Some(Self::Target),
            "passiveConditions" => Some(Self::Passive),
            _ => None,
        }
    }
}

/// One `<condition name="…">` element as parsed. Structurally the same shape as
/// [`ParsedEffect`] — conditions carry per-level `<value level="N">` tables and
/// ranged rows too (`OpEnergyMax`'s `amount` is a 7-level table; `RemainHpPer`'s
/// uses `fromLevel`/`toLevel` *and* enchant sub-level rows), which is why they
/// cannot be read as flat scalars.
#[derive(Clone)]
pub(crate) struct ParsedCondition {
    scope: CondScope,
    name: String,
    params: LeveledValues,
    sub_params: HashMap<String, Vec<RangedRow>>,
    /// List-valued params — `<weaponType><item>DUAL</item>…` and
    /// `<npcIds><item>13018</item>…`. Never level-tabled in this datapack.
    lists: HashMap<String, Vec<String>>,
}

/// One `<effect>` element as parsed, before it is resolved into a per-level
/// [`Skill`]. Java's `SkillData.NamedParamInfo`.
#[derive(Clone)]
pub(crate) struct ParsedEffect {
    scope: EffectScope,
    name: String,
    params: LeveledValues,
    /// Ranged `<value>` rows inside this effect's params, resolved per
    /// (level, sub) at finalize.
    sub_params: HashMap<String, Vec<RangedRow>>,
    mode: String,
    groups: Vec<RestorationGroup>,
    armor_condition: u8,
    weapon_condition: u32,
    /// `fromLevel`/`toLevel`, with the `level` attribute as the default for
    /// **both** (Java: `parseInteger(attributes, "fromLevel", level)`), so
    /// `level="3"` means exactly level 3.
    from_level: Option<i32>,
    to_level: Option<i32>,
    /// `fromSubLevel`/`toSubLevel`, likewise defaulting from `subLevel`.
    /// Sub-levels are the **skill-enchant** routes (1001+/2001+); this port has
    /// no enchanted skills, so a sub-level always reads as 0 and any effect
    /// gated on a positive range never applies — see [`Self::applies_at`].
    from_sub_level: Option<i32>,
    to_sub_level: Option<i32>,
}

impl ParsedEffect {
    /// Java `SkillData.forEachNamedParamInfoParam`'s gate:
    ///
    /// ```java
    /// ((fromLevel == null && toLevel == null) || (fromLevel <= level && toLevel >= level))
    ///   && ((fromSubLevel == null && toSubLevel == null) || (fromSubLevel <= subLevel && toSubLevel >= subLevel))
    /// ```
    ///
    /// An effect with no level attributes applies at every level — which is
    /// what the parser previously assumed for *all* effects, so the 775
    /// level-gated elements in this datapack were applying outside their
    /// range. Frenzy 176's `PAtk`/`CriticalRate` (`fromLevel="6" toLevel="9"`)
    /// were live at levels 1-5, for instance.
    ///
    /// `sub` is 0 for the unenchanted skill — the sub-level clause then
    /// rejects every effect that names a range (all of which start at 1001+),
    /// matching Java; the enchanted variants pass their real sub-level.
    fn applies_at(&self, level: i32, sub: i32) -> bool {
        let sub_level: i32 = sub;
        let level_ok = match (self.from_level, self.to_level) {
            (None, None) => true,
            (from, to) => from.is_none_or(|f| f <= level) && to.is_none_or(|t| t >= level),
        };
        let sub_ok = match (self.from_sub_level, self.to_sub_level) {
            (None, None) => true,
            (from, to) => from.is_none_or(|f| f <= sub_level) && to.is_none_or(|t| t >= sub_level),
        };
        level_ok && sub_ok
    }
}

pub(crate) fn finalize_skill(
    id: i32,
    name: &str,
    to_level: i32,
    values: &LeveledValues,
    effects: &[ParsedEffect],
    field_rows: &HashMap<String, Vec<RangedRow>>,
    conditions: &[ParsedCondition],
    out: &mut ParsedSkills,
) {
    if id < 0 {
        return;
    }
    if !name.is_empty() {
        out.names.insert(id, name.to_string());
    }
    for level in 1..=to_level {
        // The plain (sub 0) skill: ranged level rows resolved, sub rows inert.
        let vals = patched_values(values, field_rows, level, 0);
        let effs = patched_effects(effects, level, 0);
        out.skills.insert(
            (id, level),
            build_skill(id, name, level, 0, &vals, &effs, conditions, &out.gaps),
        );

        // The enchanted variants — one instance per declared sub-level, like
        // Java's parse loop, plus the route registration `addRouteForSkill`
        // does per instance.
        for (from_sub, to_sub) in declared_sub_ranges(field_rows, effects, level) {
            out.routes
                .entry((id, level))
                .or_default()
                .push((from_sub, to_sub));
            for sub in from_sub..=to_sub {
                let vals = patched_values(values, field_rows, level, sub);
                let effs = patched_effects(effects, level, sub);
                out.enchanted.insert(
                    (id, level, sub),
                    build_skill(id, name, level, sub, &vals, &effs, conditions, &out.gaps),
                );
            }
        }
    }
}

/// Resolve `values` for one (level, sub): ranged level rows first (they form
/// the level's base — and `{N+index}` magic-level tables now actually parse),
/// then, for `sub > 0`, the matching enchant-route rows on top. `base` in an
/// expression is the field's value *before* the row applies, `index`/`subIndex`
/// are 1-based offsets into the row's ranges — Java `SkillData.parseValues`.
fn patched_values(
    values: &LeveledValues,
    field_rows: &HashMap<String, Vec<RangedRow>>,
    level: i32,
    sub: i32,
) -> LeveledValues {
    if field_rows.is_empty() {
        return values.clone();
    }
    let mut out = values.clone();
    for pass_sub in [false, true] {
        if pass_sub && sub == 0 {
            break;
        }
        for (field, rows) in field_rows {
            for r in rows {
                let is_sub_row = r.from_sub > 0;
                if is_sub_row != pass_sub || !(r.from_level <= level && level <= r.to_level) {
                    continue;
                }
                if is_sub_row && !(r.from_sub <= sub && sub <= r.to_sub) {
                    continue;
                }
                if let Some(resolved) = resolve_row(&out, field, r, level, sub) {
                    out.entry(field.clone())
                        .or_default()
                        .insert(level, resolved);
                }
            }
        }
    }
    out
}

/// Resolve one ranged row's text: `{…}` through the expression evaluator
/// (`None` drops the row, like Java's exception path), plain text verbatim.
fn resolve_row(
    current: &LeveledValues,
    field: &str,
    r: &RangedRow,
    level: i32,
    sub: i32,
) -> Option<String> {
    let text = r.text.trim();
    if !text.starts_with('{') {
        return Some(text.to_string());
    }
    let vars = crate::data::skill_expr::ExprVars {
        base: value_at(current, field, level).and_then(|v| v.parse().ok()),
        index: (level - r.from_level + 1) as f64,
        sub_index: if r.from_sub > 0 {
            (sub - r.from_sub + 1) as f64
        } else {
            0.0
        },
    };
    crate::data::skill_expr::eval_braced(text, vars).map(fmt_num)
}

/// Format an evaluated number so the downstream `parse::<i32>()`/`::<f64>()`
/// readers both work: whole values print bare (`214`, not `214.0`).
fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        v.to_string()
    }
}

/// The same two-pass resolution over each effect's ranged param rows.
fn patched_effects(effects: &[ParsedEffect], level: i32, sub: i32) -> Vec<ParsedEffect> {
    effects
        .iter()
        .map(|e| {
            if e.sub_params.is_empty() {
                return e.clone();
            }
            let mut out = e.clone();
            for pass_sub in [false, true] {
                if pass_sub && sub == 0 {
                    break;
                }
                for (field, rows) in &e.sub_params {
                    for r in rows {
                        let is_sub_row = r.from_sub > 0;
                        if is_sub_row != pass_sub || !(r.from_level <= level && level <= r.to_level)
                        {
                            continue;
                        }
                        if is_sub_row && !(r.from_sub <= sub && sub <= r.to_sub) {
                            continue;
                        }
                        if let Some(resolved) = resolve_row(&out.params, field, r, level, sub) {
                            out.params
                                .entry(field.clone())
                                .or_default()
                                .insert(level, resolved);
                        }
                    }
                }
            }
            out
        })
        .collect()
}

/// The distinct enchant-route sub-level ranges declared for this level —
/// from field rows, effect param rows and effect-level sub gates.
fn declared_sub_ranges(
    field_rows: &HashMap<String, Vec<RangedRow>>,
    effects: &[ParsedEffect],
    level: i32,
) -> Vec<(i32, i32)> {
    // Rows within one route are often fragmented (`1001–1005`, `1006–1006`,
    // …), so merge by the route bucket (`sub / 1000`) — the registry's unit
    // is the route, and instances are built over the merged span.
    let mut buckets: std::collections::BTreeMap<i32, (i32, i32)> =
        std::collections::BTreeMap::new();
    let mut add = |from_sub: i32, to_sub: i32| {
        let e = buckets.entry(from_sub / 1000).or_insert((from_sub, to_sub));
        e.0 = e.0.min(from_sub);
        e.1 = e.1.max(to_sub);
    };
    let covers = |from: i32, to: i32| from <= level && level <= to;
    for rows in field_rows.values() {
        for r in rows {
            if r.from_sub > 0 && covers(r.from_level, r.to_level) {
                add(r.from_sub, r.to_sub);
            }
        }
    }
    for e in effects {
        for rows in e.sub_params.values() {
            for r in rows {
                if r.from_sub > 0 && covers(r.from_level, r.to_level) {
                    add(r.from_sub, r.to_sub);
                }
            }
        }
        if let Some(from_sub) = e.from_sub_level.filter(|&f| f > 0) {
            let level_ok = match (e.from_level, e.to_level) {
                (None, None) => true,
                (f, t) => f.is_none_or(|f| f <= level) && t.is_none_or(|t| t >= level),
            };
            if level_ok {
                add(from_sub, e.to_sub_level.unwrap_or(from_sub));
            }
        }
    }
    buckets.into_values().collect()
}

/// `<condition name="X">` → [`SkillCondition`] for one (level, sub) — the
/// counterpart of the effect match, and the single place a condition name
/// becomes enforceable (PLAN_G34_SKILL_PARITY.md §S1).
///
/// Returns `None` for a name this port doesn't implement. That is the
/// **fail-open** path Java has no equivalent of: the condition simply isn't
/// enforced and the skill casts where Java would refuse it. Every `None` is
/// already recorded by [`SkillGaps::conditions`] at parse time, so the census
/// test names it — do not add a name here without also striking it off there.
pub(crate) fn build_condition(c: &ParsedCondition, level: i32, sub: i32) -> Option<SkillCondition> {
    use crate::model::skill::{AffectType, MountKind, PercentType, Vital};

    // Ranged rows (`RemainHpPer`'s `fromLevel`/`fromSubLevel` amounts) are
    // resolved the same way effect params are.
    let params = patched_condition_params(c, level, sub);
    let num = |key: &str| -> Option<i32> {
        value_at(&params, key, level).and_then(|v| v.parse::<f64>().ok().map(|f| f as i32))
    };
    let flag = |key: &str, default: bool| -> bool {
        value_at(&params, key, level).map_or(default, |v| v.eq_ignore_ascii_case("true"))
    };
    let percent =
        || value_at(&params, "percentType", level).map_or(PercentType::More, PercentType::from_xml);
    let affect =
        || value_at(&params, "affectType", level).map_or(AffectType::Caster, AffectType::from_xml);
    let weapon_mask = || {
        c.lists.get("weaponType").map_or(0, |types| {
            types.iter().fold(0u32, |acc, t| {
                acc | crate::data::item_data::WeaponType::from_name(t).mask_bit()
            })
        })
    };
    let remain = |vital: Vital| {
        Some(SkillCondition::RemainVital {
            vital,
            amount: num("amount").unwrap_or(0),
            percent: percent(),
            affect: affect(),
        })
    };

    match c.name.as_str() {
        "EquipWeapon" => Some(SkillCondition::EquipWeapon {
            mask: weapon_mask(),
        }),
        "EquipShield" => Some(SkillCondition::EquipShield),
        "Op1hWeapon" => Some(SkillCondition::HandedWeapon {
            mask: weapon_mask(),
            two_handed: false,
        }),
        "Op2hWeapon" => Some(SkillCondition::HandedWeapon {
            mask: weapon_mask(),
            two_handed: true,
        }),
        "OpEncumbered" => Some(SkillCondition::Encumbered {
            weight_percent: num("weightPercent").unwrap_or(0),
            slots_percent: num("slotsPercent").unwrap_or(0),
        }),
        "RemainHpPer" => remain(Vital::Hp),
        "RemainMpPer" => remain(Vital::Mp),
        "RemainCpPer" => remain(Vital::Cp),
        "EnergySaved" => Some(SkillCondition::EnergySaved {
            amount: num("amount").unwrap_or(0),
        }),
        "OpEnergyMax" => Some(SkillCondition::EnergyMax {
            amount: num("amount").unwrap_or(0),
        }),
        // `Creature.getRace()` — the NPC template race for a monster, the
        // character race for a player. An unknown name would silently match
        // nothing, so a bad value drops the condition instead.
        "TargetRace" => value_at(&params, "race", level)
            .and_then(crate::enums::Race::from_name)
            .map(|race| SkillCondition::TargetRace { race }),
        "TargetMyParty" => Some(SkillCondition::TargetMyParty {
            include_me: flag("includeMe", false),
        }),
        "ConsumeBody" => Some(SkillCondition::ConsumeBody),
        "OpCanEscape" => Some(SkillCondition::CanEscape),
        "OpResurrection" => Some(SkillCondition::Resurrection),
        "OpUnlock" => Some(SkillCondition::Unlock),
        "OpTargetPc" => Some(SkillCondition::TargetPc),
        "OpCallPc" => Some(SkillCondition::CallPc),
        "CanTransform" => Some(SkillCondition::CanTransform),
        "CanSummon" => Some(SkillCondition::CanSummon),
        "CanSummonCubic" => Some(SkillCondition::CanSummonCubic),
        "CanSummonSiegeGolem" => Some(SkillCondition::CanSummonSiegeGolem),
        // Two Java classes, one body.
        "CanUseInBattlefield" | "OpSiegeHammer" => Some(SkillCondition::InsideSiegeZone),
        "OpSocialClass" => Some(SkillCondition::SocialClass {
            social_class: num("socialClass").unwrap_or(-1),
        }),
        "BuildCamp" => Some(SkillCondition::BuildCamp),
        "OpSkillAcquire" => num("skillId").map(|skill_id| SkillCondition::SkillAcquire {
            skill_id,
            has_learned: flag("hasLearned", false),
        }),
        "OpStrider" => Some(SkillCondition::Mounted {
            kind: MountKind::Strider,
        }),
        "OpWyvern" => Some(SkillCondition::Mounted {
            kind: MountKind::Wyvern,
        }),
        "NotInUnderwater" => Some(SkillCondition::NotInUnderwater),
        "CheckLevel" => Some(SkillCondition::CheckLevel {
            min: num("minLevel").unwrap_or(i32::MIN),
            max: num("maxLevel").unwrap_or(i32::MAX),
            affect: affect(),
        }),
        // The symbol/totem gate. Before G34 S1 this had its **own** field on
        // `Skill` and its own inline check in `cast.rs`; it now goes through the
        // generic parse like every other condition, so there is one
        // representation instead of two that could drift.
        "OpExistNpc" => Some(SkillCondition::ExistNpc(
            crate::model::skill::OpExistNpcCondition {
                npc_ids: c
                    .lists
                    .get("npcIds")
                    .map(|ids| ids.iter().filter_map(|v| v.parse().ok()).collect())
                    .unwrap_or_default(),
                range: num("range").unwrap_or(0),
                is_around: flag("isAround", false),
            },
        )),
        "CheckSex" => Some(SkillCondition::CheckSex {
            // Java `params.getBoolean("isFemale")`.
            is_female: flag("isFemale", false),
        }),
        // An unrecognised `type`/`alignment` drops the whole condition rather
        // than guessing a default: a residence gate that silently passes is
        // worse than one the census still reports as missing.
        "OpHome" => value_at(&params, "type", level)
            .and_then(|v| match v {
                "CASTLE" => Some(ResidenceType::Castle),
                "CLANHALL" => Some(ResidenceType::ClanHall),
                "FORTRESS" => Some(ResidenceType::Fortress),
                _ => None,
            })
            .map(|residence| SkillCondition::Home { residence }),
        "OpTargetDoor" => Some(SkillCondition::TargetDoor {
            door_ids: id_list(c, "doorIds"),
        }),
        "OpTargetNpc" => Some(SkillCondition::TargetNpc {
            npc_ids: id_list(c, "npcIds"),
        }),
        "OpCompanion" => value_at(&params, "type", level)
            .and_then(|v| match v {
                "PET" => Some(CompanionKind::Pet),
                "MY_SUMMON" => Some(CompanionKind::MySummon),
                _ => None,
            })
            .map(|kind| SkillCondition::Companion { kind }),
        "OpAlignment" => value_at(&params, "alignment", level)
            .and_then(|v| match v {
                "LAWFUL" => Some(false),
                "CHAOTIC" => Some(true),
                _ => None,
            })
            .map(|chaotic| SkillCondition::Alignment {
                affect: affect(),
                chaotic,
            }),
        "OpSkill" => num("skillId").map(|skill_id| SkillCondition::SkillKnown {
            skill_id,
            skill_level: num("skillLevel").unwrap_or(1),
            has_learned: flag("hasLearned", false),
        }),
        _ => None,
    }
}

/// A condition's `<xxxIds>` child list as ids — the shape `OpTargetDoor`,
/// `OpTargetNpc` and `OpExistNpc` all use. Unparseable entries are skipped
/// rather than zeroed, since id `0` would match nothing anyway.
fn id_list(c: &ParsedCondition, key: &str) -> Vec<i32> {
    c.lists
        .get(key)
        .map(|ids| ids.iter().filter_map(|v| v.parse().ok()).collect())
        .unwrap_or_default()
}

/// [`patched_effects`] for one condition: resolve its ranged `<value>` rows
/// into the level table before reading params off it.
fn patched_condition_params(c: &ParsedCondition, level: i32, sub: i32) -> LeveledValues {
    if c.sub_params.is_empty() {
        return c.params.clone();
    }
    let mut out = c.params.clone();
    for pass_sub in [false, true] {
        if pass_sub && sub == 0 {
            break;
        }
        for (field, rows) in &c.sub_params {
            for r in rows {
                let is_sub_row = r.from_sub > 0;
                if is_sub_row != pass_sub || !(r.from_level <= level && level <= r.to_level) {
                    continue;
                }
                if is_sub_row && !(r.from_sub <= sub && sub <= r.to_sub) {
                    continue;
                }
                if let Some(resolved) = resolve_row(&out, field, r, level, sub) {
                    out.entry(field.clone())
                        .or_default()
                        .insert(level, resolved);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
