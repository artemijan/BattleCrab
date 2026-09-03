//! Critical-damage stats (G19).
//!
//! `CriticalDamage`/`CriticalDamageAdd` were parsed into `StatModifiers` and
//! then read by **nobody** — all three damage formulas hard-coded a ×2 crit.
//! So Death Whisper 1242, Focus Attack 317, Vicious Stance 312, Frenzy 176,
//! Dance of Fire 274 and 13 other learnable skills were completely inert.
//! Found by scanning for `Stat` variants with no consumer, the check the
//! previous slice's post-mortem called for.

use super::*;

use crate::model::components::StatModifiers;
use crate::model::formulas::{self, CritDamage};
use crate::model::movement::Position;
use crate::model::skill::effects::SkillEffect;
use crate::model::stats::{Stat, StatQualifier};

const DIST: &str = crate::data::DIST_GAME;

/// The `(stat, amount)` pairs a skill contributes with no qualifier.
fn plain_mods(
    skills: &crate::data::skill_data::SkillData,
    id: i32,
    level: i32,
) -> Vec<(Stat, f64)> {
    skills
        .get(id, level)
        .unwrap_or_else(|| panic!("skill {id} loads"))
        .effects
        .iter()
        .filter_map(|e| match e {
            SkillEffect::StatModifier(m) if m.qualifier.is_none() => Some((m.stat, m.amount)),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The formula itself
// ---------------------------------------------------------------------------

/// A non-crit swing must not read the crit stats at all — otherwise every
/// crit-damage buff would silently become a flat damage buff.
#[test]
fn crit_stats_do_not_touch_a_normal_hit() {
    let huge = CritDamage {
        mul: 10.0,
        add: 1000.0,
    };
    let plain = formulas::calc_auto_attack_damage(
        100.0,
        1.0,
        Position::Front,
        50.0,
        false,
        CritDamage::default(),
        false,
        1.0,
        false,
        1.0,
        1.0,
        1.0,
    );
    let with_stats = formulas::calc_auto_attack_damage(
        100.0,
        1.0,
        Position::Front,
        50.0,
        false,
        huge,
        false,
        1.0,
        false,
        1.0,
        1.0,
        1.0,
    );
    assert_eq!(
        plain, with_stats,
        "a non-crit ignores cAtk/cAtkAdd entirely"
    );
}

/// `CritDamage::default()` is Java's stat-free `2 * 1 * 1 * 1` / `0`, so the
/// whole refactor is behaviour-preserving for an actor with no crit buffs —
/// which is what every pre-existing damage test relies on.
#[test]
fn default_crit_damage_reproduces_the_old_hard_coded_double() {
    let base = formulas::calc_auto_attack_damage(
        100.0,
        1.0,
        Position::Front,
        50.0,
        false,
        CritDamage::default(),
        false,
        1.0,
        false,
        1.0,
        1.0,
        1.0,
    );
    let crit = formulas::calc_auto_attack_damage(
        100.0,
        1.0,
        Position::Front,
        50.0,
        true,
        CritDamage::default(),
        false,
        1.0,
        false,
        1.0,
        1.0,
        1.0,
    );
    assert!(
        (crit - base * 2.0).abs() < 1e-9,
        "default crit is exactly ×2: {base} -> {crit}"
    );
}

/// The multiplier scales the crit, and the flat add lands **after** the
/// soulshot multiply and **inside** the ×77 / ÷pDef — Java's bracketing, which
/// is what makes `cAtkAdd` worth far more than its face value.
#[test]
fn crit_multiplier_and_flat_add_follow_javas_bracketing() {
    // pAtk 100, no prox bonus, pDef 50 → base attack term is 100.
    let doubled = formulas::calc_auto_attack_damage(
        100.0,
        1.0,
        Position::Front,
        50.0,
        true,
        CritDamage { mul: 4.0, add: 0.0 },
        false,
        1.0,
        false,
        1.0,
        1.0,
        1.0,
    );
    let default_crit = formulas::calc_auto_attack_damage(
        100.0,
        1.0,
        Position::Front,
        50.0,
        true,
        CritDamage::default(),
        false,
        1.0,
        false,
        1.0,
        1.0,
        1.0,
    );
    assert!(
        (doubled - default_crit * 2.0).abs() < 1e-9,
        "cAtk 4 is twice cAtk 2"
    );

    // cAtkAdd = 50 → attack becomes (100*2 + 50) = 250, ×77 / 50.
    let with_add = formulas::calc_auto_attack_damage(
        100.0,
        1.0,
        Position::Front,
        50.0,
        true,
        CritDamage {
            mul: 2.0,
            add: 50.0,
        },
        false,
        1.0,
        false,
        1.0,
        1.0,
        1.0,
    );
    assert!(
        (with_add - (250.0 * 77.0 / 50.0)).abs() < 1e-9,
        "cAtkAdd lands inside the ×77, got {with_add}"
    );

    // With soulshots the add is applied *after* the ss multiply, so it is
    // NOT doubled: (100*2*2 + 50) rather than ((100*2 + 50)*2).
    let ss = formulas::calc_auto_attack_damage(
        100.0,
        1.0,
        Position::Front,
        50.0,
        true,
        CritDamage {
            mul: 2.0,
            add: 50.0,
        },
        true,
        1.0,
        false,
        1.0,
        1.0,
        1.0,
    );
    assert!(
        (ss - (450.0 * 77.0 / 50.0)).abs() < 1e-9,
        "soulshots do not scale cAtkAdd, got {ss}"
    );
}

/// The magic branch takes its own multiplier (`MAGIC_CRITICAL_DAMAGE`), and
/// only when the cast actually crit.
#[test]
fn magic_crit_multiplier_applies_only_on_a_magic_crit() {
    let none = formulas::MagicFailure::None;
    let plain = formulas::calc_magic_dam(100.0, 60.0, 12.0, false, 3.0, 1.0, none, 1.0);
    let base = formulas::calc_magic_dam(100.0, 60.0, 12.0, false, 2.0, 1.0, none, 1.0);
    assert_eq!(plain, base, "a non-crit cast ignores the crit multiplier");

    let crit = formulas::calc_magic_dam(100.0, 60.0, 12.0, true, 3.0, 1.0, none, 1.0);
    assert!(
        (crit - base * 3.0).abs() < 1e-9,
        "a magic crit takes the full multiplier"
    );
}

// ---------------------------------------------------------------------------
// Stat plumbing
// ---------------------------------------------------------------------------

/// `CriticalDamagePosition` is **multiplicative with identity 1.0**, unlike the
/// additive move-type map — mixing the two up would make an unqualified stat
/// read as a ×0 (or a missing one as +1).
#[test]
fn position_qualified_stats_multiply_from_one() {
    let mut mods = StatModifiers::default();
    assert_eq!(
        mods.position_value(Stat::CriticalDamage, Position::Back),
        1.0,
        "absent reads as 1.0, not 0.0"
    );

    model::stat_finalize::apply_modifier(
        &mut mods,
        &model::skill::effects::StatModifierEffect {
            stat: Stat::CriticalDamage,
            mode: model::stats::StatModifierType::Per,
            amount: 30.0,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: Some(StatQualifier::Position(Position::Back)),
            two_handed: false,
            hp_percent: 0,
        },
    );
    assert!(
        mods.mul.is_empty(),
        "a position-qualified effect must not leak into the plain mul map"
    );
    assert!(
        (mods.position_value(Stat::CriticalDamage, Position::Back) - 1.3).abs() < 1e-9,
        "+30% → ×1.3"
    );
    assert_eq!(
        mods.position_value(Stat::CriticalDamage, Position::Front),
        1.0,
        "and only from behind"
    );
}

// ---------------------------------------------------------------------------
// Real dist data
// ---------------------------------------------------------------------------

/// Death Whisper 1242 — the buff this whole slice is really about — parses to a
/// `PER` `CriticalDamage` modifier. Before the consumers landed it pumped this
/// stat and nothing read it.
#[test]
fn death_whisper_grants_a_critical_damage_multiplier() {
    let skills = dist::skills_owned();
    let mods = plain_mods(&skills, 1242, 1);
    let crit = mods
        .iter()
        .find(|(s, _)| *s == Stat::CriticalDamage)
        .expect("Death Whisper pumps CriticalDamage");
    assert!(crit.1 > 0.0, "and by a positive amount, got {}", crit.1);
}

/// A representative spread of the 18 learnable `CriticalDamage` skills all
/// reach `Stat::CriticalDamage` (PER) or `CriticalDamageAdd` (DIFF).
#[test]
fn learnable_critical_damage_skills_all_reach_a_crit_stat() {
    let skills = dist::skills_owned();
    for id in [
        176, 193, 274, 312, 317, 401, 414, 420, 1242, 1253, 1356, 1363,
    ] {
        let mods = plain_mods(&skills, id, 1);
        assert!(
            mods.iter()
                .any(|(s, _)| matches!(s, Stat::CriticalDamage | Stat::CriticalDamageAdd)),
            "skill {id} contributes a crit-damage stat, got {mods:?}"
        );
    }
}

/// Focus Death 355 carries **two** position-qualified entries with opposite
/// signs — front `-30%` and back `+90%` — so the skill makes you worse at
/// crit-damage head-on and far better from behind. That asymmetry is the
/// whole point of the effect, and it only survives because the position map is
/// multiplicative: `-30` becomes ×0.7, not a subtraction.
#[test]
fn focus_death_penalises_frontal_crits_and_rewards_backstabs() {
    let skills = dist::skills_owned();
    let qualified: Vec<_> = skills
        .get(355, 1)
        .expect("Focus Death loads")
        .effects
        .iter()
        .filter_map(|e| match e {
            SkillEffect::StatModifier(m) => match m.qualifier {
                Some(StatQualifier::Position(p)) => Some((m.stat, p, m.amount)),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(
        qualified,
        vec![
            (Stat::CriticalDamage, Position::Front, -30.0),
            (Stat::CriticalDamage, Position::Back, 90.0),
        ],
        "both halves of the effect parse, with their real signs"
    );

    // And they fold to the multipliers the formula reads.
    let mut mods = StatModifiers::default();
    for e in &skills.get(355, 1).unwrap().effects {
        if let SkillEffect::StatModifier(m) = e {
            model::stat_finalize::apply_modifier(&mut mods, m);
        }
    }
    assert!(
        (mods.position_value(Stat::CriticalDamage, Position::Front) - 0.7).abs() < 1e-9,
        "front -30% → ×0.7"
    );
    assert!(
        (mods.position_value(Stat::CriticalDamage, Position::Back) - 1.9).abs() < 1e-9,
        "back +90% → ×1.9"
    );
    assert_eq!(
        mods.position_value(Stat::CriticalDamage, Position::Side),
        1.0,
        "side is untouched"
    );
}

/// Prophecy of Wind 1357 grants the magic-crit multiplier — the one branch
/// besides autoattacks with a real learnable grantor.
#[test]
fn prophecy_of_wind_grants_magic_critical_damage() {
    let skills = dist::skills_owned();
    let mods = plain_mods(&skills, 1357, 1);
    assert!(
        mods.iter().any(|(s, _)| *s == Stat::MagicCriticalDamage),
        "Prophecy of Wind pumps MagicCriticalDamage, got {mods:?}"
    );
}

/// End to end through the passive path: a player who has learned a
/// `CriticalDamage` skill carries the multiplier in `StatModifiers.mul`, which
/// is exactly what `crit_damage_auto` reads.
#[test]
fn learned_crit_damage_passive_folds_into_stat_modifiers() {
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = dist::player_templates_owned();
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = dist::items_owned();
    data.skill_data = dist::skills_owned();
    let world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let bare = Player::from_char(&world.data, &dummy_char(4301, "Bare"));
    assert_eq!(
        bare.stat_modifiers.add.get(&Stat::CriticalDamageAdd),
        None,
        "no skill: no modifier at all"
    );

    // Skill 193 "Critical Damage" — a genuine `operateType=P` passive, and a
    // `mode=DIFF` one, so it feeds the *flat* `CriticalDamageAdd` rather than
    // the multiplier. (Most of the headline crit skills — Vicious Stance 312,
    // Focus Attack 317 — are *toggles*, which land through the buff path and
    // so are correctly absent from a freshly built `Player`.)
    let mut chr = dummy_char(4302, "Crit");
    chr.skills = vec![(193, 1, 0)];
    let bundle = Player::from_char(&world.data, &chr);
    let add = bundle
        .stat_modifiers
        .add
        .get(&Stat::CriticalDamageAdd)
        .copied()
        .unwrap_or(0.0);
    assert!(
        (add - 32.0).abs() < 1e-9,
        "Critical Damage lvl 1 is a flat +32 cAtkAdd, got {add}"
    );
    // Which, per the bracketing test above, is worth 32·77/pDef on a crit —
    // far more than its face value suggests.
    assert_eq!(
        bundle.stat_modifiers.mul.get(&Stat::CriticalDamage),
        None,
        "a DIFF effect never touches the multiplier"
    );
}

/// G34 S4 sub-slice 2 — `Formulas.calcCritDamage` reads the **skill** crit
/// stats when a skill is involved, not `CRITICAL_DAMAGE`. The port's physical
/// branch was a flat `2.0`, i.e. both its stats pinned at identity, so Heroic
/// Berserker (396) — the learnable `PhysicalSkillCriticalDamage` source — did
/// nothing.
///
/// Both sides are asserted: Java multiplies the attacker's stat by the
/// *target's* defence twin, so wiring only the attacker half looks right until
/// someone equips the defence.
#[test]
fn a_physical_skill_crit_reads_the_skill_crit_stats_not_the_autoattack_one() {
    use crate::model::components::StatModifiers;
    use crate::model::stats::Stat;
    let (mut world, ..) = test_world();
    let attacker = 6301;
    let target = 6302;
    let _a = ingame_player_access(&mut world, 1, attacker, 0);
    let _b = ingame_player_access(&mut world, 2, target, 0);

    let base = combat::crit_damage_skill(&world, attacker, target, false);
    assert_eq!(base, 2.0, "Java's `2 * 1 * 1` with no stats");

    let mut mods = world
        .objects
        .get_component::<StatModifiers>(&attacker)
        .cloned()
        .expect("modifiers");
    mods.mul.insert(Stat::PhysicalSkillCriticalDamage, 1.5);
    // The autoattack stat must NOT be read on this path — set it to something
    // conspicuous to prove the branch picks the skill stat.
    mods.mul.insert(Stat::CriticalDamage, 9.0);
    world.objects.add_components(&attacker, mods);
    assert_eq!(
        combat::crit_damage_skill(&world, attacker, target, false),
        3.0,
        "2 × 1.5 — the skill stat, not CRITICAL_DAMAGE"
    );

    let mut tmods = world
        .objects
        .get_component::<StatModifiers>(&target)
        .cloned()
        .expect("modifiers");
    tmods
        .mul
        .insert(Stat::DefencePhysicalSkillCriticalDamage, 0.5);
    world.objects.add_components(&target, tmods);
    assert_eq!(
        combat::crit_damage_skill(&world, attacker, target, false),
        1.5,
        "…and the target's defence twin divides it back down"
    );

    // The magic branch is unchanged and still reads its own pair.
    assert_eq!(
        combat::crit_damage_skill(&world, attacker, target, true),
        2.0,
        "a magic skill is unaffected by the physical-skill stats"
    );
}

/// G34 S4 sub-slice 7 — `CriticalRatePositionBonus` (Focus Chance 356), the
/// crit-*rate* twin of `CriticalDamagePosition`.
///
/// Focus Chance is the one skill on this dist that declares **all three**
/// positions — −30 % front, +30 % side, +60 % back — so it rewards a rogue who
/// circles and *punishes* one who stands in front. Dropping the front term
/// would read as a pure buff and pass any back-attack-only test.
#[test]
fn focus_chance_scales_crit_rate_per_position_including_downwards() {
    use crate::model::formulas::calc_critical_position_bonus;
    use crate::model::movement::Position;

    // Unbuffed: Java's flat 1.0 / 1.1 / 1.3.
    assert_eq!(calc_critical_position_bonus(Position::Front, 1.0), 1.0);
    assert!((calc_critical_position_bonus(Position::Side, 1.0) - 1.1).abs() < 1e-9);
    assert!((calc_critical_position_bonus(Position::Back, 1.0) - 1.3).abs() < 1e-9);

    // Focus Chance's own numbers, as `mergePositionTypeValue` stores them
    // (`(amount / 100) + 1`).
    let front = calc_critical_position_bonus(Position::Front, 0.7);
    let side = calc_critical_position_bonus(Position::Side, 1.3);
    let back = calc_critical_position_bonus(Position::Back, 1.6);
    assert!(
        (front - 0.7).abs() < 1e-9,
        "−30 % front is a penalty, not a no-op: {front}"
    );
    assert!((side - 1.43).abs() < 1e-9, "1.1 × 1.3: {side}");
    assert!((back - 2.08).abs() < 1e-9, "1.3 × 1.6: {back}");
}

// ---------------------------------------------------------------------------
// The blow formula's own crit block (formula-parity pass)
// ---------------------------------------------------------------------------

/// **A dagger's blow scales with the same crit-damage stats a swing does** —
/// `calcBlowDamage`'s `cdMult`, which the port had been leaving at 1. Death
/// Whisper on a Backstab did nothing before this.
///
/// The shape is Java's and worth spelling out: the position and vulnerability
/// multipliers count **half** (`((v−1)/2)+1`), so a ×1.4 position bonus moves
/// the damage by 20 %, not 40 %.
#[test]
fn a_blow_reads_the_crit_damage_stats() {
    use crate::model::formulas::BlowCritDamage;

    let blow = |cd: BlowCritDamage| {
        formulas::calc_blow_damage(200.0, 80.0, 60.0, Position::Back, 1.0, false, 1.0, cd)
    };
    let base = blow(BlowCritDamage::default());
    assert!(base > 0.0);

    // `cdMult` scales the whole hit.
    let buffed = blow(BlowCritDamage {
        mult: 1.5,
        p_atk_add: 0.0,
    });
    assert!(
        (buffed - base * 1.5).abs() < 1e-9,
        "cdMult multiplies the finished damage"
    );

    // `cdPatk` enters **inside** the bracket at ×6, so it is divided by
    // defence with the rest rather than added afterwards.
    let with_add = blow(BlowCritDamage {
        mult: 1.0,
        p_atk_add: 10.0,
    });
    assert!(
        (with_add - (base + (77.0 * 6.0 * 10.0 / 60.0))).abs() < 1e-9,
        "cdPatk lands inside the ×77/pDef bracket"
    );
}

/// …and the stats reach it from the world: `blow_crit_damage` reads the same
/// `CriticalDamage` family `crit_damage_auto` does, with the halving Java's
/// blow formula applies to the position and vulnerability halves.
#[test]
fn blow_crit_damage_reads_the_stat_maps() {
    let (mut world, ..) = combat_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 0, 0, 0);

    let bare = crate::game_loop::combat::blow_crit_damage(&world, 3001, NPC_OID, Position::Back);
    assert!((bare.mult - 1.0).abs() < 1e-9, "no stats, no bonus");
    assert!((bare.p_atk_add - 0.0).abs() < 1e-9);

    {
        let mods = world
            .objects
            .get_component_mut::<StatModifiers>(&3001)
            .expect("mods");
        mods.mul.insert(Stat::CriticalDamage, 1.4);
        mods.add.insert(Stat::CriticalDamageAdd, 25.0);
    }
    let buffed = crate::game_loop::combat::blow_crit_damage(&world, 3001, NPC_OID, Position::Back);
    assert!(
        (buffed.mult - 1.4).abs() < 1e-9,
        "the plain multiplier is not halved"
    );
    assert!(
        buffed.p_atk_add > 0.0,
        "the additive pair reaches the formula"
    );
}
