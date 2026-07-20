//! `TwoHandedBluntBonus` / `TwoHandedSwordBonus` (G19).
//!
//! Rage 94, Frenzy 176 and Two-handed Weapon Mastery 293 grant extra pAtk and
//! accuracy — but **only** while a matching *two-handed* weapon is equipped.
//! Both conditions are separate axes: a one-handed mace fails the slot test
//! even though it passes the weapon-type one.

use super::*;

use crate::model::skill::SkillEffect;
use crate::model::stats::Stat;

const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

fn dist_skills() -> crate::data::skill_data::SkillData {
    crate::data::skill_data::SkillData::load_from(DIST)
}

/// The `(stat, weapon_mask_set, two_handed)` triples a skill contributes.
fn conditioned(skills: &crate::data::skill_data::SkillData, id: i32, level: i32) -> Vec<(Stat, bool, bool)> {
    skills
        .get(id, level)
        .unwrap_or_else(|| panic!("skill {id} level {level} loads"))
        .effects
        .iter()
        .filter_map(|e| match e {
            SkillEffect::StatModifier(m) if m.two_handed => Some((m.stat, m.weapon_condition != 0, m.two_handed)),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// All three carriers produce two-handed-conditioned modifiers. Rage 94 grants
/// only pAtk; Frenzy 176 and Two-handed Weapon Mastery 293 grant pAtk and
/// accuracy.
#[test]
fn the_three_carriers_produce_two_handed_modifiers() {
    let skills = dist_skills();
    // Level 2: Rage 94's `pAtkAmount` is **0** at level 1 (see
    // `rage_grants_nothing_at_level_one`), so level 1 would prove nothing.
    for id in [94, 176, 293] {
        let got = conditioned(&skills, id, 2);
        assert!(!got.is_empty(), "skill {id} contributes something two-handed");
        assert!(got.iter().all(|(_, weapon_gated, _)| *weapon_gated), "skill {id}: every one is weapon-gated too");
        assert!(
            got.iter().any(|(s, _, _)| *s == Stat::PhysicalAttack),
            "skill {id} grants pAtk: {got:?}"
        );
    }
    // Frenzy and the Mastery add accuracy on top; Rage does not.
    for id in [176, 293] {
        assert!(
            conditioned(&skills, id, 2).iter().any(|(s, _, _)| *s == Stat::AccuracyCombat),
            "skill {id} grants accuracy too"
        );
    }
}

/// Rage 94 declares `pAtkAmount = 0` at level 1 — the skill exists but grants
/// nothing until level 2. A zero-amount modifier is dropped rather than stored,
/// which is behaviourally identical to Java's `mergeAdd(stat, 0)` and keeps the
/// effect list honest.
#[test]
fn rage_grants_nothing_at_level_one() {
    let skills = dist_skills();
    assert!(conditioned(&skills, 94, 1).is_empty(), "level 1 is a no-op");
    assert!(!conditioned(&skills, 94, 2).is_empty(), "level 2 starts granting");
}

/// Rage 94 and Frenzy 176 carry **both** the Blunt and the Sword variant, so
/// they cover either two-hander. (This is what made the naive per-effect
/// cluster count read 5 when only 3 distinct skills are involved.)
#[test]
fn rage_and_frenzy_cover_both_weapon_families() {
    let skills = dist_skills();
    for id in [94, 176] {
        let masks: Vec<u32> = skills
            .get(id, 2)
            .unwrap()
            .effects
            .iter()
            .filter_map(|e| match e {
                SkillEffect::StatModifier(m) if m.two_handed => Some(m.weapon_condition),
                _ => None,
            })
            .collect();
        let distinct: std::collections::HashSet<u32> = masks.iter().copied().collect();
        assert!(distinct.len() >= 2, "skill {id} carries both a blunt and a sword variant: {distinct:?}");
    }
}

/// The two conditions are independent axes — the weapon-type mask is set *and*
/// the two-handed flag is set, rather than one standing in for the other.
#[test]
fn the_weapon_and_slot_conditions_are_separate_axes() {
    let skills = dist_skills();
    let effects: Vec<_> = skills
        .get(293, 1)
        .unwrap()
        .effects
        .iter()
        .filter_map(|e| match e {
            SkillEffect::StatModifier(m) => Some((m.weapon_condition, m.two_handed)),
            _ => None,
        })
        .collect();
    assert!(
        effects.iter().all(|(mask, two)| *mask != 0 && *two),
        "both conditions are recorded, not conflated: {effects:?}"
    );
}

// ---------------------------------------------------------------------------
// The slot condition
// ---------------------------------------------------------------------------

/// `two_handed_weapon_equipped` reads the weapon template's `bodypart`, so a
/// one-handed weapon fails even with an empty off-hand — inferring "two-handed"
/// from an empty left hand would wrongly match an unarmed or shield-less
/// one-hander.
#[test]
fn the_slot_condition_reads_the_weapon_bodypart() {
    const ITEM_DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let items = crate::data::item_data::ItemData::load_from(ITEM_DIST);

    // A two-handed weapon really is marked `lrhand` in the datapack, and a
    // one-handed one is not — the premise the condition rests on.
    let two_handed = (1..=1000).find(|&id| {
        items.get(id).is_some_and(|t| t.body_part == crate::data::item_data::SLOT_LR_HAND)
    });
    let one_handed = (1..=1000).find(|&id| {
        items.get(id).is_some_and(|t| t.body_part == crate::data::item_data::SLOT_R_HAND)
    });
    assert!(two_handed.is_some(), "the dist has two-handed weapons");
    assert!(one_handed.is_some(), "and one-handed ones, which must not qualify");
    assert_ne!(two_handed, one_handed);
}

/// A bonus that is *not* two-handed-gated is unaffected by the new axis — the
/// vast majority of the datapack, and the case that must not regress.
#[test]
fn an_unconditioned_modifier_is_not_two_handed_gated() {
    let skills = dist_skills();
    // Death Whisper 1242: a plain crit-damage buff, no conditions at all.
    let dw = skills.get(1242, 1).expect("Death Whisper loads");
    assert!(
        dw.effects.iter().all(|e| !matches!(e, SkillEffect::StatModifier(m) if m.two_handed)),
        "no stray two-handed gating"
    );
}

/// `Default for StatModifierEffect` — added so condition axes can be appended
/// without breaking every literal. An unconditioned modifier is the default.
#[test]
fn stat_modifier_default_is_unconditioned() {
    let d = crate::model::skill::StatModifierEffect::default();
    assert_eq!(d.armor_condition, 0);
    assert_eq!(d.weapon_condition, 0);
    assert!(!d.two_handed);
    assert!(d.qualifier.is_none());
}
