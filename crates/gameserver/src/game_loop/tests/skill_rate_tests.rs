//! `MagicMpCost` / `Reuse` — the per-`magicType` MP-cost and cooldown rates
//! (G16).
//!
//! Java keeps two `magicType → factor` maps on `CreatureStat` (`_mpConsumeStat`
//! / `_reuseStat`), merged multiplicatively by the two effect handlers and read
//! by `getMpConsume(skill)` / `getReuseTime(skill)`. Until this landed, both
//! effects were icon-only markers: Arcane Wisdom cost nothing to *have*.

use super::*;
use crate::game_loop::abnormal::has_buff;

use crate::game_loop::skills::effects::{
    merge_skill_rates, mp_consume_for, remove_skill_rates, reuse_time_for,
};
use crate::model::components::stats::SkillRateStats;
use crate::model::skill::Skill;
use crate::model::skill::effects::SkillEffect;
use crate::model::skill::target::TargetType;

const CASTER: i32 = 7001;
const CID: u32 = 1;

/// A skill of `magic_type` costing `mp_consume` MP with a `reuse_delay` ms
/// cooldown — the *victim* of the rates, not a carrier of them.
fn cost_skill(id: i32, magic_type: i32, mp_consume: i32, reuse_delay: i32) -> Skill {
    Skill {
        self_continuous: false,
        id,
        name: format!("Cost{id}"),
        magic_type,
        mp_consume,
        reuse_delay,
        ..Default::default()
    }
}

/// A rate buff: `MagicMpCost`/`Reuse` percentages on one bucket.
fn rate_skill(id: i32, effects: Vec<SkillEffect>) -> Skill {
    Skill {
        self_continuous: false,
        id,
        name: format!("Rate{id}"),
        target_type: TargetType::Self_,
        is_continuous: true,
        effect_point: 100,
        abnormal_time: 120,
        effects,
        ..Default::default()
    }
}

/// The cooldown actually armed for `skill` on `oid`, in ms. `None` when the
/// reuse map holds no entry — which is itself an outcome here, since Java's
/// `> 10` gate drops a short enough cooldown rather than registering it.
fn armed_reuse_ms(world: &World, oid: i32, skill: &Skill) -> Option<i32> {
    world
        .objects
        .get_component::<Reuses>(&oid)?
        .0
        .get(&skill.reuse_key())
        .map(|x| x.total_ms)
}

// ---------------------------------------------------------------------------
// The rate tables
// ---------------------------------------------------------------------------

/// `mergeMpConsumeTypeValue(magicType, amount/100 + 1, mul)`: −30 % becomes a
/// ×0.70 factor on **that bucket only**, and `getMpConsume` truncates.
#[test]
fn an_mp_cost_buff_discounts_only_its_own_magic_type() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);

    let magic = cost_skill(9500, 1, 100, 0);
    let physical = cost_skill(9501, 0, 100, 0);
    assert_eq!(mp_consume_for(&world, CASTER, &magic), 100, "unbuffed");

    // Arcane Wisdom's shape: −30 on magicType 1.
    let buff = rate_skill(
        9510,
        vec![SkillEffect::MagicMpCost {
            magic_type: 1,
            amount: -30.0,
        }],
    );
    merge_skill_rates(&mut world, CASTER, &buff);
    assert_eq!(
        mp_consume_for(&world, CASTER, &magic),
        70,
        "a magic skill costs 30 % less"
    );
    assert_eq!(
        mp_consume_for(&world, CASTER, &physical),
        100,
        "a physical skill is in a different bucket entirely"
    );

    remove_skill_rates(&mut world, CASTER, &buff);
    assert_eq!(mp_consume_for(&world, CASTER, &magic), 100, "gone with it");
    assert!(
        world
            .objects
            .get_component::<SkillRateStats>(&CASTER)
            .is_some_and(|rs| rs.mp_consume.is_empty()),
        "back to the identity, so the entry is dropped rather than left at 0.9999"
    );
}

/// Java merges with `mul`, so two −10 % songs are **0.81, not 0.80** — and the
/// `div` on exit is the exact inverse, so unmerging the *first* one out of
/// order still lands on the second's 0.90.
#[test]
fn overlapping_rate_buffs_compound_and_unmerge_out_of_order() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let victim = cost_skill(9500, 1, 1000, 0);

    let a = rate_skill(
        9511,
        vec![SkillEffect::MagicMpCost {
            magic_type: 1,
            amount: -10.0,
        }],
    );
    let b = rate_skill(
        9512,
        vec![SkillEffect::MagicMpCost {
            magic_type: 1,
            amount: -10.0,
        }],
    );
    merge_skill_rates(&mut world, CASTER, &a);
    merge_skill_rates(&mut world, CASTER, &b);
    assert_eq!(
        mp_consume_for(&world, CASTER, &victim),
        810,
        "0.9 x 0.9 = 0.81, not an additive 0.80"
    );

    remove_skill_rates(&mut world, CASTER, &a);
    assert_eq!(mp_consume_for(&world, CASTER, &victim), 900);
}

/// A positive amount is a **penalty**: Magical Backfire (1396) is `+200`, i.e.
/// ×3 MP cost, and Seal of Suspension (1248) trebles cooldowns the same way.
#[test]
fn a_positive_amount_is_a_penalty() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);

    let backfire = rate_skill(
        9513,
        vec![SkillEffect::MagicMpCost {
            magic_type: 1,
            amount: 200.0,
        }],
    );
    merge_skill_rates(&mut world, CASTER, &backfire);
    assert_eq!(
        mp_consume_for(&world, CASTER, &cost_skill(9500, 1, 50, 0)),
        150
    );
}

// ---------------------------------------------------------------------------
// Reuse
// ---------------------------------------------------------------------------

/// `getReuseTime` scales the delay the same way — but returns **before** the
/// multiply for a static-reuse or static (`isMagic == 2`) skill, which is what
/// keeps Super Haste's −99 % away from fixed cooldowns.
#[test]
fn a_reuse_buff_shortens_cooldowns_except_the_static_ones() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);

    let physical = cost_skill(9502, 0, 0, 10_000);
    let mut static_reuse = cost_skill(9503, 0, 0, 10_000);
    static_reuse.static_reuse = true;
    let static_skill = cost_skill(9504, 2, 0, 10_000);

    // Quick Recovery 3's shape: −20 on magicType 0 — plus the same on bucket 2,
    // so the static skill below is refused by the *bypass* and not merely by
    // having no rate in its bucket.
    let buff = rate_skill(
        9514,
        vec![
            SkillEffect::Reuse {
                magic_type: 0,
                amount: -20.0,
            },
            SkillEffect::Reuse {
                magic_type: 2,
                amount: -20.0,
            },
        ],
    );
    merge_skill_rates(&mut world, CASTER, &buff);

    assert_eq!(reuse_time_for(&world, CASTER, &physical), 8_000);
    assert_eq!(
        reuse_time_for(&world, CASTER, &static_reuse),
        10_000,
        "<staticReuse>true</staticReuse> bypasses the rate"
    );
    assert_eq!(
        reuse_time_for(&world, CASTER, &static_skill),
        10_000,
        "so does isMagic == 2"
    );
}

/// The cooldown the port actually *arms* is the scaled one, and Java's `> 10`
/// gate is applied to it — so a −99 % rate can take a short skill out of the
/// reuse map altogether rather than parking it there for 9 ms.
#[test]
fn the_armed_cooldown_is_the_scaled_one() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let skill = cost_skill(9505, 0, 0, 4_000);

    let buff = rate_skill(
        9515,
        vec![SkillEffect::Reuse {
            magic_type: 0,
            amount: -75.0,
        }],
    );
    merge_skill_rates(&mut world, CASTER, &buff);
    set_skill_reuse(&mut world, CASTER, &skill);
    let armed = armed_reuse_ms(&world, CASTER, &skill).expect("a cooldown was armed");
    assert_eq!(armed, 1_000, "4 s x 0.25");

    // Super Haste's -99 on a 1 s skill lands under the 10 ms floor.
    let super_haste = rate_skill(
        9516,
        vec![SkillEffect::Reuse {
            magic_type: 0,
            amount: -99.0,
        }],
    );
    remove_skill_rates(&mut world, CASTER, &buff);
    merge_skill_rates(&mut world, CASTER, &super_haste);
    let short = cost_skill(9506, 0, 0, 1_000);
    set_skill_reuse(&mut world, CASTER, &short);
    assert!(
        armed_reuse_ms(&world, CASTER, &short).is_none(),
        "10 ms is below Java's threshold, so nothing is registered at all"
    );
}

// ---------------------------------------------------------------------------
// Buff lifecycle + the dist data
// ---------------------------------------------------------------------------

/// The rates ride the *buff*, not the cast: landing the buff merges them and
/// its expiry unmerges them, like `DefenceTrait`. Both carriers are icon-only
/// otherwise, so this also proves they still survive the empty-effects guard.
#[test]
fn the_rates_arrive_and_leave_with_the_buff() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let victim = cost_skill(9500, 1, 200, 6_000);

    let mut song = rate_skill(
        9517,
        vec![
            SkillEffect::MagicMpCost {
                magic_type: 1,
                amount: -50.0,
            },
            SkillEffect::Reuse {
                magic_type: 1,
                amount: -50.0,
            },
        ],
    );
    song.name = "Test Song".into();
    world.data.skill_data.insert_for_test(song.clone());

    effects::apply_skill_effects(&mut world, CASTER, CASTER, &song);
    assert!(
        has_buff(&world, CASTER, 9517),
        "an effect-less rate buff still lands as a timed buff"
    );
    assert_eq!(mp_consume_for(&world, CASTER, &victim), 100);
    assert_eq!(reuse_time_for(&world, CASTER, &victim), 3_000);

    effects::handle_buff_expire(&mut world, CASTER, 9517);
    assert_eq!(mp_consume_for(&world, CASTER, &victim), 200);
    assert_eq!(reuse_time_for(&world, CASTER, &victim), 6_000);
}

/// The dist's real carriers parse with the right bucket and percentage —
/// including the per-level `amount` and the `magicType` that is *not* the
/// carrying skill's own.
#[test]
fn the_dist_carriers_parse_their_buckets_and_amounts() {
    let sd = dist::skills();

    // Arcane Wisdom: -30 % on magic (1). The skill itself is a passive.
    let aw = sd.get(336, 1).expect("Arcane Wisdom");
    assert!(
        aw.effects.iter().any(|e| matches!(
            e,
            SkillEffect::MagicMpCost {
                magic_type: 1,
                amount
            } if *amount == -30.0
        )),
        "{:?}",
        aw.effects
    );

    // Zealot: -50 % on *physical* (0) — the bucket is the effect's, not the
    // skill's.
    let zealot = sd.get(420, 1).expect("Zealot");
    assert!(
        zealot.effects.iter().any(|e| matches!(
            e,
            SkillEffect::MagicMpCost {
                magic_type: 0,
                amount
            } if *amount == -50.0
        )),
        "{:?}",
        zealot.effects
    );

    // Quick Recovery: a per-level Reuse, -10/-15/-20, on magic (1).
    for (level, pct) in [(1, -10.0), (2, -15.0), (3, -20.0)] {
        let qr = sd.get(164, level).expect("Quick Recovery");
        assert!(
            qr.effects.iter().any(|e| matches!(
                e,
                SkillEffect::Reuse {
                    magic_type: 1,
                    amount
                } if *amount == pct
            )),
            "level {level}: {:?}",
            qr.effects
        );
    }

    // Song of Renewal is the physical-bucket counterpart, on the same effect —
    // proof the bucket is read per effect rather than guessed from the family.
    let renewal = sd.get(349, 1).expect("Song of Renewal");
    assert!(
        renewal.effects.iter().any(|e| matches!(
            e,
            SkillEffect::Reuse {
                magic_type: 0,
                amount
            } if *amount == -20.0
        )),
        "{:?}",
        renewal.effects
    );

    // Seal of Suspension is the penalty direction: +200 % reuse.
    let seal = sd.get(1248, 1).expect("Seal of Suspension");
    assert!(
        seal.effects.iter().any(|e| matches!(
            e,
            SkillEffect::Reuse { amount, .. } if *amount == 200.0
        )),
        "{:?}",
        seal.effects
    );
}

/// `<staticReuse>` is parsed (1 297 skills declare it), which is what makes the
/// bypass above reachable from real data.
#[test]
fn static_reuse_is_read_from_the_dist() {
    let sd = dist::skills();
    let statics = (1..=30_000)
        .filter_map(|id| sd.get(id, 1))
        .filter(|s| s.static_reuse)
        .count();
    assert!(
        statics > 100,
        "the dist's static-reuse skills are recognised, found {statics}"
    );
}

// ---------------------------------------------------------------------------
// End to end through the cast pipeline
// ---------------------------------------------------------------------------

/// **The gate: a real cast spends the discounted MP and arms the shortened
/// cooldown.** Both accessors are exercised above in isolation; this is the
/// proof they are actually wired into `use_magic`'s precheck,
/// `handle_skill_finish`'s consume, and `set_skill_reuse`.
#[test]
fn a_cast_spends_the_discounted_mp_and_arms_the_shortened_cooldown() {
    use crate::model::components::skills::SkillBook;
    use crate::model::components::stats::Vitals;

    let (mut world, _db, _l) = cast_test_world();
    let mut nuke = cost_skill(9520, 1, 40, 20_000);
    nuke.hit_time = 100;
    nuke.target_type = TargetType::Self_;
    nuke.effects = vec![SkillEffect::Heal { power: 1.0 }];
    world.data.skill_data.insert_for_test(nuke.clone());

    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world
        .objects
        .get_component_mut::<SkillBook>(&CASTER)
        .unwrap()
        .0
        .insert(9520, 1);
    let mp_before = world
        .objects
        .get_component::<Vitals>(&CASTER)
        .unwrap()
        .cur_mp;

    // Arcane Wisdom's −30 % on magic, plus a −50 % reuse on the same bucket.
    let song = rate_skill(
        9521,
        vec![
            SkillEffect::MagicMpCost {
                magic_type: 1,
                amount: -30.0,
            },
            SkillEffect::Reuse {
                magic_type: 1,
                amount: -50.0,
            },
        ],
    );
    merge_skill_rates(&mut world, CASTER, &song);
    drain(&mut out);

    use_magic(&mut world, CID, CASTER, 9520, true, false);
    advance_ticks(&mut world, 60);

    let spent = mp_before
        - world
            .objects
            .get_component::<Vitals>(&CASTER)
            .unwrap()
            .cur_mp;
    assert!(
        (spent - 28.0).abs() < 1e-6,
        "the 40 MP nuke cost 28, spent {spent}"
    );
    let armed = armed_reuse_ms(&world, CASTER, &nuke).expect("a cooldown was armed");
    assert_eq!(armed, 10_000, "20 s halved");
}

/// The **precheck** reads the scaled cost too (Java `checkUseConditions`:
/// `getMpConsume(skill) + getMpInitialConsume(skill)`), so a discount doesn't
/// just refund MP after the fact — it makes an unaffordable skill castable.
#[test]
fn the_discount_makes_an_unaffordable_skill_castable() {
    use crate::model::components::combat::Casting;
    use crate::model::components::skills::SkillBook;
    use crate::model::components::stats::Vitals;

    let (mut world, _db, _l) = cast_test_world();
    let mut nuke = cost_skill(9522, 1, 60, 0);
    nuke.hit_time = 1_000;
    nuke.target_type = TargetType::Self_;
    nuke.effects = vec![SkillEffect::Heal { power: 1.0 }];
    world.data.skill_data.insert_for_test(nuke);

    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world
        .objects
        .get_component_mut::<SkillBook>(&CASTER)
        .unwrap()
        .0
        .insert(9522, 1);
    // The pool is 50 — less than the raw 60.
    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&CASTER)
            .unwrap()
            .cur_mp,
        50.0
    );
    drain(&mut out);

    use_magic(&mut world, CID, CASTER, 9522, true, false);
    assert!(
        world.objects.get_component::<Casting>(&CASTER).is_none(),
        "refused: 60 MP is more than the caster has"
    );

    let song = rate_skill(
        9523,
        vec![SkillEffect::MagicMpCost {
            magic_type: 1,
            amount: -50.0,
        }],
    );
    merge_skill_rates(&mut world, CASTER, &song);
    use_magic(&mut world, CID, CASTER, 9522, true, false);
    assert!(
        world.objects.get_component::<Casting>(&CASTER).is_some(),
        "at 30 MP it goes through"
    );
}
