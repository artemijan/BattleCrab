use super::creature_level;
use crate::game_loop::helpers::npc_name_or_empty;
use crate::game_loop::helpers::npc_template;
use crate::model::components::BaseStats;
use crate::model::components::Buffs;
use crate::model::components::StatModifiers;
use crate::model::formulas;
use crate::model::skill::BuffSlot;
use crate::model::skill::Skill;
use crate::model::skill::SkillEffect;
use crate::world::World;

/// The caster's name for the damage system messages. NPCs cast skills as of
/// G21, so this can't `expect` a `Player` — a monster resolves to its template
/// name. These strings only ever reach the *caster's own* client, which an NPC
/// doesn't have, so the value is cosmetic for the NPC path; the helper exists
/// so the shared effect code stops panicking on a non-player caster.
pub(crate) fn caster_display_name(world: &World, oid: i32) -> String {
    if let Some(p) = world.objects.get_component::<crate::model::Player>(&oid) {
        return p.name.clone();
    }
    npc_name_or_empty(world, oid)
}

/// A creature's level (Java `Creature.getLevel()`, which both players and NPCs
/// implement) — the caster's `levelMod` in the physical-skill damage formula
/// and the target's level in the recharge penalty.
///
/// Distinct from [`creature_level`], which additionally resolves a cubic to its
/// owner; the two agree on every non-cubic object.
pub(crate) fn player_or_npc_level(world: &World, oid: i32) -> i32 {
    if let Some(p) = world.objects.get_component::<crate::model::Player>(&oid) {
        return p.level;
    }
    npc_template(world, oid).map(|t| t.level).unwrap_or(1)
}

/// The caster's STR bonus for `calcPhysicalSkillCrit`'s `critical_chance ×
/// STR bonus`. A caster without `BaseStats` (nothing in the dist, but the
/// component is optional) falls back to the neutral 1.0.
pub(crate) fn caster_str_bonus(world: &World, oid: i32) -> f64 {
    world
        .objects
        .get_component::<BaseStats>(&oid)
        .map(|b| {
            world
                .data
                .stat_bonus
                .bonus(crate::model::stats::BaseStat::Str, b.str_)
        })
        .unwrap_or(1.0)
}

/// Java `Formulas.calcGeneralTraitBonus(attacker, target, traitType, false)` —
/// how much a debuff's landing chance is scaled by the target's resistance to
/// its trait. **The clause order is Java's and is load-bearing:** `NONE` first,
/// then *invulnerability* — which applies to every group, not just the
/// resistable one — and only then the group gate.
///
/// - **group 3** (the resistable debuff traits: SHOCK, HOLD, SLEEP, POISON,
///   DERANGEMENT, PARALYZE, BLEED, …) is what the dist's `<trait>` tags almost
///   entirely declare, and what the learnable resistances defend.
/// - **group 2** (`*_WEAKNESS`, declared by 5 skills here) additionally needs
///   the *attacker* to carry a matching `AttackTrait` — the "Detect &lt;Category&gt;
///   Weakness" line and the Eye of Hunter/Slayer pair grant those, and the
///   effect is parsed and merged (`merge_attack_traits`), so the branch is
///   live rather than the no-op an older comment here claimed.
/// - **group 1** (weapon types, plus `ETC`) and `NONE` are never scaled here.
///
/// **Both sides are read, and a target with no `DefenceTraits` at all is not a
/// short circuit.** Java's tables default to `1.0` attack / `0.0` defence
/// (`CreatureStat` fills them in its constructor), so the last line is
/// `max(attackTrait − 0, 0.05)` for an untraited target — which is the
/// attacker's own bonus, not 1.0. The port used to bail out to 1.0 whenever the
/// target carried no defence traits, and since most targets carry none that
/// silently threw away every group-3 attack trait in the game: the four
/// augment options (3952–3955), the boss-jewel line and the two Dual - Trait
/// Increase skills all merge one.
///
/// `ignore_resistance` is Java's fourth argument: the **damage** formulas pass
/// `true` (group 3 short-circuits to 1.0 — a stun resistance does not soften
/// the stun's damage), the landing roll passes `false`.
pub(crate) fn calc_general_trait_bonus(
    world: &World,
    attacker_oid: i32,
    target_oid: i32,
    trait_type: crate::model::skill::TraitType,
    ignore_resistance: bool,
) -> f64 {
    use crate::model::components::DefenceTraits;
    use crate::model::skill::TraitType;
    if trait_type == TraitType::None {
        return 1.0;
    }
    // An absent component is an empty table, **not** an early return: Java's
    // arrays are always there, initialised to 1.0 attack / 0.0 defence.
    let traits = world.objects.get_component::<DefenceTraits>(&target_oid);
    // Java tests invulnerability *before* the group switch, so a weapon- or
    // weakness-trait immunity zeroes the chance too.
    if traits.is_some_and(|t| t.invulnerable.contains(&trait_type)) {
        return 0.0;
    }
    match trait_type.group() {
        // The `*_WEAKNESS` family needs **both** sides: the attacker's
        // `AttackTrait` and the target's `DefenceTrait`.
        2 => {
            if !has_attack_trait(world, attacker_oid, trait_type)
                || !traits.is_some_and(|t| t.resist.contains_key(&trait_type))
            {
                return 1.0;
            }
        }
        3 => {
            if ignore_resistance {
                return 1.0;
            }
        }
        _ => return 1.0,
    }
    let defence = traits
        .and_then(|t| t.resist.get(&trait_type).copied())
        .unwrap_or(0.0);
    // A *negative* defence trait is a vulnerability (4416's -15), so this can
    // legitimately exceed 1.0 — Java only floors it.
    (attack_trait(world, attacker_oid, trait_type) - defence).max(0.05)
}

/// Java `getAttackTrait` — **1.0** for anyone without a matching `AttackTrait`
/// buff (the table's identity), which is what makes the group-3 case read as
/// the plain `1 − defence`.
fn attack_trait(world: &World, oid: i32, trait_type: crate::model::skill::TraitType) -> f64 {
    world
        .objects
        .get_component::<crate::model::components::AttackTraits>(&oid)
        .and_then(|at| at.values.get(&trait_type).copied())
        .unwrap_or(1.0)
}

/// Java `hasAttackTrait` — membership, which is a *different* question from the
/// value: an unbuffed attacker's value is 1.0 but `hasAttackTrait` is false, and
/// the group-2 branch gates on the latter.
fn has_attack_trait(world: &World, oid: i32, trait_type: crate::model::skill::TraitType) -> bool {
    world
        .objects
        .get_component::<crate::model::components::AttackTraits>(&oid)
        .is_some_and(|at| at.values.contains_key(&trait_type))
}

/// `Formulas.calcWeaponTraitBonus` — `max(0.22, 1 − defenceTrait(weaponType))`.
///
/// The attacker's *weapon type* is itself a `TraitType` (SWORD, DAGGER, BOW …),
/// and the dist's armour buffs really do grant those defence traits (19 skills
/// name SWORD, 24 DAGGER, 45 BOW…). The 0.22 floor is Java's, and note there is
/// no `hasDefenceTrait` gate here — the raw table value is read, so an absent
/// entry is a clean 1.0.
pub(crate) fn calc_weapon_trait_bonus(world: &World, attacker_oid: i32, target_oid: i32) -> f64 {
    let weapon_trait = crate::model::skill::TraitType::of_weapon(
        crate::game_loop::combat::ranged::equipped_weapon_type(world, attacker_oid)
            .unwrap_or_default(),
    );
    let defence = world
        .objects
        .get_component::<crate::model::components::DefenceTraits>(&target_oid)
        .and_then(|d| d.resist.get(&weapon_trait).copied())
        .unwrap_or(0.0);
    (1.0 - defence).max(0.22)
}

/// `Formulas.calcWeaknessBonus` — the product over every `*_WEAKNESS` trait the
/// attacker carries *and* the target is weak to, **excluding the skill's own**
/// trait (that one is already counted by `calcGeneralTraitBonus`).
///
/// Java's invulnerability test in here reads `isInvulnerableTrait(traitType)` —
/// the **skill's** trait, not the loop variable. That looks like a slip, but it
/// is what the reference server does, so it is reproduced.
pub(crate) fn calc_weakness_bonus(
    world: &World,
    attacker_oid: i32,
    target_oid: i32,
    skill_trait: crate::model::skill::TraitType,
) -> f64 {
    use crate::model::components::DefenceTraits;
    let Some(defence) = world.objects.get_component::<DefenceTraits>(&target_oid) else {
        return 1.0;
    };
    if defence.invulnerable.contains(&skill_trait) {
        return 1.0;
    }
    let mut result = 1.0;
    for weakness in crate::model::skill::TraitType::ALL_WEAKNESS {
        if weakness == skill_trait {
            continue;
        }
        let Some(def) = defence.resist.get(&weakness).copied() else {
            continue;
        };
        if !has_attack_trait(world, attacker_oid, weakness) {
            continue;
        }
        result *= (attack_trait(world, attacker_oid, weakness) - def).max(0.05);
    }
    result
}

/// `Formulas.calcAttackTraitBonus` — the auto-attack's whole trait term: the
/// weapon bonus times every group-2 weakness, floored at 0.05.
/// Test hook for [`pvp_pve_bonus`], which is private to this module.
#[cfg(test)]
pub(crate) fn pvp_pve_bonus_for_test(
    world: &World,
    attacker_oid: i32,
    target_oid: i32,
    skill: Option<&Skill>,
) -> f64 {
    pvp_pve_bonus(world, attacker_oid, target_oid, skill)
}

/// `Formulas.calculatePvpPveBonus`, resolved against world state.
///
/// `skill = None` is Java's auto-attack branch (its `skill == null`), which
/// reads the `*_PHYSICAL_ATTACK_*` pair rather than either skill pair.
///
/// Returns 1.0 for any pairing that is neither playable-vs-playable nor
/// involves an `Attackable` — two non-attackable NPCs, or a door.
pub(crate) fn pvp_pve_bonus(
    world: &World,
    attacker_oid: i32,
    target_oid: i32,
    skill: Option<&Skill>,
) -> f64 {
    use crate::model::stats::Stat;

    let mul = |oid: i32, stat: Stat| -> f64 {
        world
            .objects
            .get_component::<StatModifiers>(&oid)
            .map(|m| crate::model::finalize(m, stat, 1.0))
            .unwrap_or(1.0)
    };

    // `isPlayable()` — a player or their summon (Java's `Playable` subtree).
    let is_playable = |oid: i32| crate::game_loop::helpers::is_playable(world, oid);
    let template = |oid: i32| {
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&oid)
            .and_then(|n| world.data.npc_data.get(n.npc_id))
    };
    let is_attackable = |oid: i32| template(oid).is_some_and(|t| t.is_attackable_class());

    // PvP: both sides playable.
    if is_playable(attacker_oid) && is_playable(target_oid) {
        let (atk_stat, def_stat) = match skill {
            None => (
                Stat::PvpPhysicalAttackDamage,
                Stat::PvpPhysicalAttackDefence,
            ),
            // `Skill.isMagic()` — `magicType == 1`.
            Some(s) if s.magic_type == 1 => {
                (Stat::PvpMagicalSkillDamage, Stat::PvpMagicalSkillDefence)
            }
            Some(_) => (Stat::PvpPhysicalSkillDamage, Stat::PvpPhysicalSkillDefence),
        };
        // Java folds in the class-balance config multipliers and a dragon
        // weapon's `DRAGON_WEAPON_DEFENCE` here; the former are blank on this
        // dist (every class 1.0) and dragon weapons post-date Interlude.
        return formulas::calculate_pvp_pve_bonus(
            mul(attacker_oid, atk_stat),
            mul(target_oid, def_stat),
            1.0,
            1.0,
            1.0,
        )
        .max(0.05);
    }

    // PvE: either side is an `Attackable`.
    if is_attackable(target_oid) || is_attackable(attacker_oid) {
        let (atk_stat, def_stat, raid_def_stat) = match skill {
            None => (
                Stat::PvePhysicalAttackDamage,
                Stat::PvePhysicalAttackDefence,
                Stat::PveRaidPhysicalAttackDefence,
            ),
            Some(s) if s.magic_type == 1 => (
                Stat::PveMagicalSkillDamage,
                Stat::PveMagicalSkillDefence,
                Stat::PveRaidMagicalSkillDefence,
            ),
            Some(_) => (
                Stat::PvePhysicalSkillDamage,
                Stat::PvePhysicalSkillDefence,
                Stat::PveRaidPhysicalSkillDefence,
            ),
        };
        // Java reads the raid pair off the **attacker** for both halves; there
        // is no `PVE_RAID_*_DAMAGE` source on this dist, so only the defence
        // half can ever move, and only while the attacker is a raid.
        let attacker_is_raid = template(attacker_oid).is_some_and(|t| t.is_raid());
        let raid_defence = if attacker_is_raid {
            mul(attacker_oid, raid_def_stat)
        } else {
            1.0
        };
        let penalty = formulas::npc_level_damage_penalty(
            &world.cfg.npc.skill_dmg_penalty_for_lvl_differences,
            creature_level(world, target_oid),
            creature_level(world, attacker_oid),
            template(target_oid).is_some_and(|t| t.is_raid()),
            world.cfg.npc.min_npc_level_for_dmg_penalty,
        );
        return formulas::calculate_pvp_pve_bonus(
            mul(attacker_oid, atk_stat),
            mul(target_oid, def_stat),
            1.0,
            raid_defence,
            penalty,
        )
        .max(0.05);
    }

    1.0
}

pub(crate) fn calc_attack_trait_bonus(world: &World, attacker_oid: i32, target_oid: i32) -> f64 {
    let weapon = calc_weapon_trait_bonus(world, attacker_oid, target_oid);
    if weapon == 0.0 {
        return 0.0;
    }
    let mut weakness = 1.0;
    for t in crate::model::skill::TraitType::ALL_WEAKNESS {
        weakness *= calc_general_trait_bonus(world, attacker_oid, target_oid, t, true);
        if weakness == 0.0 {
            return 0.0;
        }
    }
    (weapon * weakness).max(0.05)
}

/// `DefenceTrait.onStart` — merge this buff's resistances into the bearer.
pub(crate) fn merge_defence_traits(
    world: &mut World,
    target_oid: i32,
    traits: &[(crate::model::skill::TraitType, f64)],
) {
    use crate::model::components::DefenceTraits;
    if traits.is_empty() {
        return;
    }
    if world
        .objects
        .get_component::<DefenceTraits>(&target_oid)
        .is_none()
    {
        world
            .objects
            .add_components(&target_oid, DefenceTraits::default());
    }
    if let Some(dt) = world
        .objects
        .get_component_mut::<DefenceTraits>(&target_oid)
    {
        for &(t, value) in traits {
            // Java: `< 1.0` merges a resistance, otherwise it is outright
            // invulnerability — a 100 in the XML is not "100 % resist".
            if value < 1.0 {
                *dt.resist.entry(t).or_insert(0.0) += value;
            } else {
                dt.invulnerable.insert(t);
            }
        }
    }
}

/// `AttackTrait.onStart` — `mergeAttackTrait(trait, value)` onto a table whose
/// identity is **1.0**, so a `<BEAST_WEAKNESS>30</BEAST_WEAKNESS>` reads as
/// 1.30.
pub(crate) fn merge_attack_traits(
    world: &mut World,
    target_oid: i32,
    traits: &[(crate::model::skill::TraitType, f64)],
) {
    use crate::model::components::AttackTraits;
    if traits.is_empty() {
        return;
    }
    if world
        .objects
        .get_component::<AttackTraits>(&target_oid)
        .is_none()
    {
        world
            .objects
            .add_components(&target_oid, AttackTraits::default());
    }
    if let Some(at) = world.objects.get_component_mut::<AttackTraits>(&target_oid) {
        for &(t, value) in traits {
            *at.values.entry(t).or_insert(1.0) += value;
        }
    }
}

/// `AttackTrait.onExit`. Java's `removeAttackTrait` drops the trait from the
/// *set* once the value is back to 1 — i.e. `hasAttackTrait` goes false again —
/// which is exactly what removing the map entry does here.
pub(crate) fn remove_attack_traits(
    world: &mut World,
    target_oid: i32,
    traits: &[(crate::model::skill::TraitType, f64)],
) {
    use crate::model::components::AttackTraits;
    let Some(at) = world.objects.get_component_mut::<AttackTraits>(&target_oid) else {
        return;
    };
    for &(t, value) in traits {
        if let Some(cur) = at.values.get_mut(&t) {
            *cur -= value;
            if (*cur - 1.0).abs() < 1e-9 {
                at.values.remove(&t);
            }
        }
    }
}

/// `DefenceTrait.onExit` — unmerge them again.
pub(crate) fn remove_defence_traits(
    world: &mut World,
    target_oid: i32,
    traits: &[(crate::model::skill::TraitType, f64)],
) {
    use crate::model::components::DefenceTraits;
    let Some(dt) = world
        .objects
        .get_component_mut::<DefenceTraits>(&target_oid)
    else {
        return;
    };
    for &(t, value) in traits {
        if value < 1.0 {
            if let Some(cur) = dt.resist.get_mut(&t) {
                *cur -= value;
                // Float subtraction can leave a hair above zero; drop the entry
                // rather than leaving a phantom 1e-17 resistance behind.
                if *cur <= 1e-9 {
                    dt.resist.remove(&t);
                }
            }
        } else {
            dt.invulnerable.remove(&t);
        }
    }
}

/// `MagicMpCost.onStart` / `Reuse.onStart` — merge this buff's rates into the
/// bearer's per-`magicType` tables. Java merges with `mul`, so overlapping
/// songs compound rather than add.
pub(crate) fn merge_skill_rates(world: &mut World, target_oid: i32, skill: &Skill) {
    use crate::model::components::SkillRateStats;
    let rates = skill_rate_factors(skill);
    if rates.is_empty() {
        return;
    }
    if world
        .objects
        .get_component::<SkillRateStats>(&target_oid)
        .is_none()
    {
        world
            .objects
            .add_components(&target_oid, SkillRateStats::default());
    }
    if let Some(rs) = world
        .objects
        .get_component_mut::<SkillRateStats>(&target_oid)
    {
        for (kind, magic_type, factor) in rates {
            let table = match kind {
                RateKind::MpConsume => &mut rs.mp_consume,
                RateKind::Reuse => &mut rs.reuse,
            };
            *table.entry(magic_type).or_insert(1.0) *= factor;
        }
    }
}

/// Rebuild the **passive** halves of the rate tables from the skill book.
///
/// Java applies a passive skill's effects when it is learned and re-checks
/// them on every stat recompute, so `MagicMpCost`/`Reuse` on a passive are
/// live stats like any other. The port used to drop them entirely:
/// `conditioned_passive_buffs` keeps only `StatModifier` effects, so a passive
/// whose *only* effect is a rate produced no buff and reached no table. That
/// left Inner Rhythm (428), Quick Recovery (164), Summon Lore (435), Divine
/// Lore (436), Holy Squad (615), Magician's Will (945) and Expert Casting
/// (1527) doing nothing at all, plus the Clarity/Apella/boss-jewel item
/// skills.
///
/// Wholesale rather than incremental, and therefore idempotent — see
/// `SkillRateStats::passive_mp_consume` for why passives cannot share the
/// buff tables' merge/un-merge discipline.
pub(crate) fn refresh_passive_skill_rates(world: &mut World, object_id: i32) {
    use crate::model::components::{SkillBook, SkillRateStats};
    use crate::model::skill::OperateType;

    let Some(book) = world.objects.get_component::<SkillBook>(&object_id) else {
        return;
    };
    let Some(inventory) = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&object_id)
    else {
        return;
    };
    let mut mp: std::collections::HashMap<i32, f64> = std::collections::HashMap::new();
    let mut reuse: std::collections::HashMap<i32, f64> = std::collections::HashMap::new();
    for (&skill_id, &level) in &book.0 {
        let Some(skill) = world.data.skill_data.get(skill_id, level) else {
            continue;
        };
        if skill.operate_type != OperateType::Passive {
            continue;
        }
        // The same `checkConditions(PASSIVE, …)` gate the stat half runs.
        if !crate::game_loop::skills::conditions::passive_stat_gate(
            skill,
            inventory,
            &world.data.item_data,
        ) {
            continue;
        }
        for (kind, magic_type, factor) in skill_rate_factors(skill) {
            let table = match kind {
                RateKind::MpConsume => &mut mp,
                RateKind::Reuse => &mut reuse,
            };
            *table.entry(magic_type).or_insert(1.0) *= factor;
        }
    }
    if mp.is_empty()
        && reuse.is_empty()
        && world
            .objects
            .get_component::<SkillRateStats>(&object_id)
            .is_none()
    {
        return;
    }
    if world
        .objects
        .get_component::<SkillRateStats>(&object_id)
        .is_none()
    {
        world
            .objects
            .add_components(&object_id, SkillRateStats::default());
    }
    if let Some(rs) = world
        .objects
        .get_component_mut::<SkillRateStats>(&object_id)
    {
        rs.passive_mp_consume = mp;
        rs.passive_reuse = reuse;
    }
}

/// `MagicMpCost.onExit` / `Reuse.onExit` — Java's `div`, the exact inverse of
/// the `mul` above, so unmerging out of order still lands back on 1.
pub(crate) fn remove_skill_rates(world: &mut World, target_oid: i32, skill: &Skill) {
    use crate::model::components::SkillRateStats;
    let rates = skill_rate_factors(skill);
    if rates.is_empty() {
        return;
    }
    let Some(rs) = world
        .objects
        .get_component_mut::<SkillRateStats>(&target_oid)
    else {
        return;
    };
    for (kind, magic_type, factor) in rates {
        let table = match kind {
            RateKind::MpConsume => &mut rs.mp_consume,
            RateKind::Reuse => &mut rs.reuse,
        };
        if let Some(cur) = table.get_mut(&magic_type) {
            *cur /= factor;
            // Back to the identity → drop the entry, so a bearer with no live
            // rate buff reads as "no component state" rather than 0.999999.
            if (*cur - 1.0).abs() < 1e-9 {
                table.remove(&magic_type);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RateKind {
    MpConsume,
    Reuse,
}

/// The `(table, magicType, factor)` triples a skill's effects contribute.
/// Java's factor is `amount / 100 + 1`, so −30 → 0.70 and +200 → 3.0. A factor
/// of exactly 1 (Holy Squad 615's first two levels carry `amount` 0) is dropped
/// — merging it would be a no-op that still forces the component into
/// existence.
fn skill_rate_factors(skill: &Skill) -> Vec<(RateKind, i32, f64)> {
    skill
        .effects
        .iter()
        .filter_map(|e| match e {
            SkillEffect::MagicMpCost { magic_type, amount } => {
                Some((RateKind::MpConsume, *magic_type, amount / 100.0 + 1.0))
            }
            SkillEffect::Reuse { magic_type, amount } => {
                Some((RateKind::Reuse, *magic_type, amount / 100.0 + 1.0))
            }
            _ => None,
        })
        .filter(|(_, _, factor)| (factor - 1.0).abs() > 1e-9)
        .collect()
}

/// Java `CreatureStat.getMpConsume(skill)` — the skill's raw cost scaled by the
/// caster's rate for that skill's own `magicType`, **truncated** to an int as
/// Java's `(int)` cast does.
///
/// The dance-stacking surcharge is the other half of Java's method: each dance
/// already running adds `ceil(mpConsume / 2)`. It is gated on
/// `DanceConsumeAdditionalMP`, which this dist sets to **False**, so it stays
/// off here — but it is wired to the config rather than assumed away.
pub(crate) fn mp_consume_for(world: &World, caster_oid: i32, skill: &Skill) -> i32 {
    let mut mp_consume = skill.mp_consume as f64;
    if skill.is_dance() && world.cfg.character.dance_consume_additional_mp {
        let dances = world
            .objects
            .get_component::<Buffs>(&caster_oid)
            .map(|b| b.0.iter().filter(|x| x.slot == BuffSlot::Dance).count())
            .unwrap_or(0);
        if dances > 0 {
            mp_consume += dances as f64 * (skill.mp_consume as f64 / 2.0).ceil();
        }
    }
    (mp_consume * skill_rate(world, caster_oid, skill, RateKind::MpConsume)) as i32
}

/// Java `CreatureStat.getReuseTime(skill)` — the raw delay scaled by the
/// caster's reuse rate for that skill's `magicType`. **Static and static-reuse
/// skills return before the multiply**, which is what keeps Super Haste's −99 %
/// off the fixed cooldowns.
pub(crate) fn reuse_time_for(world: &World, caster_oid: i32, skill: &Skill) -> i32 {
    if skill.static_reuse || skill.is_static() {
        return skill.reuse_delay;
    }
    (skill.reuse_delay as f64 * skill_rate(world, caster_oid, skill, RateKind::Reuse)) as i32
}

/// `getMpConsumeTypeValue` / `getReuseTypeValue`: the bearer's factor for the
/// bucket this skill belongs to, defaulting to 1.
fn skill_rate(world: &World, caster_oid: i32, skill: &Skill, kind: RateKind) -> f64 {
    world
        .objects
        .get_component::<crate::model::components::SkillRateStats>(&caster_oid)
        .map(|rs| {
            let (buffs, passives) = match kind {
                RateKind::MpConsume => (&rs.mp_consume, &rs.passive_mp_consume),
                RateKind::Reuse => (&rs.reuse, &rs.passive_reuse),
            };
            // Passive and buff contributions compound, as Java's stacked
            // effects on the same stat do.
            buffs.get(&skill.magic_type).copied().unwrap_or(1.0)
                * passives.get(&skill.magic_type).copied().unwrap_or(1.0)
        })
        .unwrap_or(1.0)
}

/// The level a live buff was cast at, so its effect list can be looked back up
/// on expiry (a resistance's value is per level).
pub(crate) fn buff_level(world: &World, object_id: i32, skill_id: i32) -> i32 {
    maybe_buff_level(world, object_id, skill_id).unwrap_or(1)
}
pub(crate) fn maybe_buff_level(world: &World, object_id: i32, skill_id: i32) -> Option<i32> {
    world
        .objects
        .get_component::<Buffs>(&object_id)
        .and_then(|b| {
            b.0.iter()
                .find(|x| x.skill_id == skill_id)
                .map(|x| x.skill_level)
        })
}
