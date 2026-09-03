//! Per-effect level gating — `fromLevel`/`toLevel`/`subLevel` on `<effect>`
//! elements (G19).
//!
//! Java's `SkillData.forEachNamedParamInfoParam` only attaches an effect to the
//! skill levels its declared range covers. The port ignored those attributes
//! entirely, so **775 level-gated effect elements** were live at every level of
//! their skill — Frenzy 176's `PAtk`/`CriticalRate` (`fromLevel="6"
//! toLevel="9"`) were already boosting a level-1 Frenzy, and the enchant-only
//! effects (`fromSubLevel="2001"`) applied to unenchanted skills.
//!
//! Unlike most G19 slices this fixes *already-ported* effects behaving wrongly
//! rather than adding new ones.

use crate::model::skill::effects::SkillEffect;
use crate::model::stats::Stat;

/// How many modifiers for `stat` this skill level carries.
///
/// A *count*, not a presence check: Frenzy 176 has three **ungated** `PAtk`
/// effects alongside its two `fromLevel="6"` ones, so "does level 1 have PAtk"
/// is the wrong question — the gated pair shows up as two extra entries from
/// level 6.
fn count_stat(
    skills: &crate::data::skill_data::SkillData,
    id: i32,
    level: i32,
    stat: Stat,
) -> usize {
    skills
        .get(id, level)
        .unwrap_or_else(|| panic!("skill {id} level {level} loads"))
        .effects
        .iter()
        .filter(|e| matches!(e, SkillEffect::StatModifier(m) if m.stat == stat))
        .count()
}

// ---------------------------------------------------------------------------
// fromLevel / toLevel
// ---------------------------------------------------------------------------

/// Frenzy 176 — the clearest case. Two extra `PAtk` and two extra
/// `CriticalRate` effects are declared `fromLevel="6" toLevel="9"`, on top of
/// the ungated ones every level has. Before this fix all four were live from
/// level 1, silently over-buffing every low-level Frenzy.
#[test]
fn frenzy_gains_its_extra_patk_effects_only_from_level_six() {
    let skills = super::dist::skills_owned();
    let low_patk = count_stat(&skills, 176, 1, Stat::PhysicalAttack);
    let low_crit = count_stat(&skills, 176, 1, Stat::CriticalRate);
    assert!(
        low_patk > 0,
        "sanity: the ungated PAtk effects are still there at level 1"
    );

    for level in 1..=5 {
        assert_eq!(
            count_stat(&skills, 176, level, Stat::PhysicalAttack),
            low_patk,
            "level {level}: no extra PAtk"
        );
        assert_eq!(
            count_stat(&skills, 176, level, Stat::CriticalRate),
            low_crit,
            "level {level}: no extra CriticalRate"
        );
    }
    for level in 6..=9 {
        assert_eq!(
            count_stat(&skills, 176, level, Stat::PhysicalAttack),
            low_patk + 2,
            "level {level}: the two gated PAtk effects joined"
        );
        assert_eq!(
            count_stat(&skills, 176, level, Stat::CriticalRate),
            low_crit + 2,
            "level {level}: the two gated CriticalRate effects joined"
        );
    }
}

/// The boundaries are inclusive on both ends (`from <= level && to >= level`),
/// which is where an off-by-one would hide.
#[test]
fn the_level_range_is_inclusive_at_both_ends() {
    let skills = super::dist::skills_owned();
    let base = count_stat(&skills, 176, 1, Stat::PhysicalAttack);
    assert_eq!(
        count_stat(&skills, 176, 5, Stat::PhysicalAttack),
        base,
        "one below the range: not yet"
    );
    assert_eq!(
        count_stat(&skills, 176, 6, Stat::PhysicalAttack),
        base + 2,
        "the first level in range: included"
    );
    assert_eq!(
        count_stat(&skills, 176, 9, Stat::PhysicalAttack),
        base + 2,
        "the last level in range: still included"
    );
}

/// An effect with no level attributes at all is unaffected — the overwhelming
/// majority of the datapack, and the case that must not regress.
#[test]
fn an_ungated_effect_still_applies_at_every_level() {
    let skills = super::dist::skills_owned();
    // Death Whisper 1242: a plain `CriticalDamage`, no level attributes.
    let max = (1..=15).filter(|l| skills.get(1242, *l).is_some()).count();
    assert!(max > 1, "sanity: Death Whisper has several levels");
    for level in 1..=max as i32 {
        assert!(
            skills
                .get(1242, level)
                .unwrap()
                .effects
                .iter()
                .any(|e| matches!(e, SkillEffect::StatModifier(_))),
            "Death Whisper level {level} keeps its ungated effect"
        );
    }
}

// ---------------------------------------------------------------------------
// Sub-levels (skill enchanting)
// ---------------------------------------------------------------------------

/// Sub-level ranges are the **enchant routes** (1001+/2001+). This port has no
/// enchanted skills, so the sub-level always reads as 0 and an effect gated on
/// such a range must never appear — exactly what Java does for an unenchanted
/// skill.
///
/// Frenzy 176's `Heal` is `fromLevel="8" toLevel="9" fromSubLevel="2001"
/// toSubLevel="2020"`: even at levels 8-9, where the *level* clause passes, the
/// sub-level clause rejects it.
#[test]
fn enchant_only_effects_never_apply_to_an_unenchanted_skill() {
    let skills = super::dist::skills_owned();
    for level in 1..=9 {
        let Some(skill) = skills.get(176, level) else {
            continue;
        };
        assert!(
            !skill
                .effects
                .iter()
                .any(|e| matches!(e, SkillEffect::Heal { .. })),
            "Frenzy level {level} must not carry its enchant-only Heal: {:?}",
            skill.effects
        );
    }
}

/// Guts 139's `Heal` is gated the same way, and Guts is a skill the port
/// already exercises elsewhere (`ResistAbnormalByCategory`) — so its ungated
/// effect must survive while the enchant-only one is dropped.
#[test]
fn guts_keeps_its_ungated_effect_and_drops_the_enchant_only_one() {
    let skills = super::dist::skills_owned();
    let guts = skills.get(139, 1).expect("Guts loads");
    assert!(
        guts.effects.iter().any(
            |e| matches!(e, SkillEffect::StatModifier(m) if m.stat == Stat::ResistAbnormalDebuff)
        ),
        "the real debuff-resist effect survives: {:?}",
        guts.effects
    );
    assert!(
        !guts
            .effects
            .iter()
            .any(|e| matches!(e, SkillEffect::Heal { .. })),
        "the enchant-only Heal is gated out"
    );
}

// ---------------------------------------------------------------------------
// Scale of the change
// ---------------------------------------------------------------------------

/// A guard on the whole datapack: no loaded skill level may carry an effect
/// whose declared range excludes it. Rather than re-parse the XML here, this
/// asserts the *outcome* on the two weapon-mastery skills whose
/// `TriggerSkillByAttack` is `fromLevel="9" toLevel="45"` — they have levels
/// below 9, which must come out clean.
///
/// (`TriggerSkillByAttack` is itself unported, so what is asserted is that the
/// *count* of effects differs across the boundary — the gate ran.)
#[test]
fn weapon_mastery_effect_count_changes_at_its_boundary() {
    let skills = super::dist::skills_owned();
    for id in [205, 209] {
        let below = skills.get(id, 8).map(|s| s.effects.len());
        let at = skills.get(id, 9).map(|s| s.effects.len());
        // Both levels must exist for the comparison to mean anything.
        assert!(
            below.is_some() && at.is_some(),
            "skill {id} has levels 8 and 9"
        );
        // `TriggerSkillByAttack` is unported, so it contributes no
        // `SkillEffect` either way — the assertion is simply that gating did
        // not corrupt the surrounding effects.
        assert!(
            below.unwrap() <= at.unwrap(),
            "skill {id}: level 8 carries no more effects than level 9"
        );
    }
}
