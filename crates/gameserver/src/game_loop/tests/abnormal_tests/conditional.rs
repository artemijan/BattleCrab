//! Effects that only apply under a condition: lucky, focus attack, shadow
//! sense at night, and the residence death fortune.

use super::*;

/// `Lucky` (194) is an **empty effect** in Java — its handler has only a
/// `canStart` guard, and `Player.isLucky()` (`level <= 9 && affected by 194`)
/// is the entire mechanic. It exempts a newbie from the death exp penalty.
///
/// Both halves of the predicate are asserted: the buff alone is not enough
/// above level 9, which is what stops a twink from carrying it up.
#[test]
fn lucky_exempts_a_newbie_from_the_death_exp_penalty() {
    const LUCKY: i32 = 194;
    let (mut world, _db, _l) = cc2_world();
    let mut lucky = cc_skill(LUCKY, SkillEffect::Lucky, "LUCKY");
    lucky.effect_point = 100;
    lucky.is_debuff = false;
    world.data.skill_data.insert_for_test(lucky);
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    let set_level = |world: &mut World, level: i32| {
        if let Some(p) = world.objects.get_component_mut::<Player>(&CASTER) {
            p.level = level;
        }
    };

    set_level(&mut world, 5);
    assert!(
        !crate::game_loop::death::is_lucky(&world, CASTER),
        "level alone is not enough — the buff has to be up"
    );
    land(&mut world, LUCKY, CASTER);
    assert!(
        crate::game_loop::death::is_lucky(&world, CASTER),
        "a level-5 character holding Lucky is exempt"
    );

    set_level(&mut world, 10);
    assert!(
        !crate::game_loop::death::is_lucky(&world, CASTER),
        "…and the buff alone is not enough past level 9"
    );
}

/// The other half of Focus Attack: the buff has to actually *grant* the stat.
/// Testing the sweep gate against a hand-inserted modifier proves the consumer
/// and nothing about the grant — which is the same registry-line-without-a-
/// consumer trap this epic keeps hitting, just pointing the other way.
#[test]
fn focus_attack_grants_the_single_target_stat_and_gives_it_back() {
    use crate::model::stats::Stat;

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut focus = cc_skill(9414, SkillEffect::PolearmSingleTarget, "NONE");
    focus.target_type = TargetType::Self_;
    world.data.skill_data.insert_for_test(focus);

    let stat = |world: &World| {
        world
            .objects
            .get_component::<model::components::stats::StatModifiers>(&CASTER)
            .map(|m| model::stat_finalize::finalize(m, Stat::PhysicalPolearmTargetSingle, 0.0))
            .unwrap_or(0.0)
    };
    assert_eq!(stat(&world), 0.0, "nothing before the toggle");

    land(&mut world, 9414, CASTER);
    assert!(
        stat(&world) > 0.0,
        "the toggle grants it, got {}",
        stat(&world)
    );

    effects::handle_buff_expire(&mut world, CASTER, 9414);
    assert_eq!(
        stat(&world),
        0.0,
        "and `onExit` takes it back — otherwise the sweep is lost forever"
    );
}

/// **`ReduceDropPenalty`** (Residence Death Fortune 610) — the exp you lose on
/// death is scaled by *what killed you*: a raid, an ordinary monster, or a
/// playable each read a different stat. Residence Death Fortune grants the
/// **mob** one at `-12` (×0.88), so dying to a monster costs less while it is
/// up and dying to a player costs exactly as much as before.
#[test]
fn residence_death_fortune_softens_a_mob_death_but_not_a_pvp_one() {
    use crate::model::stats::Stat;

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    // A wide level band so the 12 % reduction is unmistakable, and exp set
    // *above* the level threshold — sitting exactly on it makes the delevel
    // clamp zero the loss and the test measure nothing.
    world.data.experience =
        crate::data::ExperienceData::from_table(vec![0, 0, 1000, 2000, 3000, 103_000], 5);
    let mob = NPC_OID;
    let killer_player = CASTER + 1;
    add_test_npc(&mut world, mob, 90101, "Monster", 5, 100, 0, 0);
    let _k = ingame_player(&mut world, CID + 1, killer_player, 50, 0, 0);

    let lost_against = |world: &mut World, killer: i32| -> i64 {
        if let Some(p) = world.objects.get_component_mut::<Player>(&CASTER) {
            p.level = 4;
            p.exp = 50_000;
        }
        crate::game_loop::death::apply_death_exp_penalty_ex(world, CASTER, false, Some(killer));
        50_000
            - world
                .objects
                .get_component::<Player>(&CASTER)
                .map(|p| p.exp)
                .unwrap_or(0)
    };

    let plain_mob = lost_against(&mut world, mob);
    let plain_pvp = lost_against(&mut world, killer_player);
    assert!(plain_mob > 0, "a mob death costs exp to begin with");

    // Grant the *mob* reduction only.
    if let Some(m) = world
        .objects
        .get_component_mut::<model::components::stats::StatModifiers>(&CASTER)
    {
        *m.mul.entry(Stat::ReduceExpLostByMob).or_insert(1.0) *= 0.88;
    }

    assert!(
        lost_against(&mut world, mob) < plain_mob,
        "dying to a monster now costs less"
    );
    assert_eq!(
        lost_against(&mut world, killer_player),
        plain_pvp,
        "but dying to a player is untouched — the stat is keyed on the killer"
    );
}

/// **`NightStatModify`** (Shadow Sense 294) — "increases Accuracy by 3 **at
/// night**". The stat is not a property of the buff but of the *clock*: it has
/// to appear at dusk and vanish at dawn while the buff sits there unchanged,
/// which is the half a plain stat grant would get wrong in both directions.
#[test]
fn shadow_sense_grants_its_accuracy_only_at_night() {
    use crate::model::stats::Stat;

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut sense = cc_skill(
        294,
        SkillEffect::NightStatModify {
            stat: Stat::AccuracyCombat,
            amount: 3.0,
            mode: model::stats::StatModifierType::Diff,
        },
        "SHADOW_SENSE",
    );
    sense.target_type = TargetType::Self_;
    world.data.skill_data.insert_for_test(sense);

    let accuracy = |world: &World| stat_add(world, CASTER, Stat::AccuracyCombat);

    land(&mut world, 294, CASTER);
    // The buff is up either way; only the clock decides.
    assert!(
        has_buff(&world, CASTER, 294),
        "the buff lands regardless of the hour"
    );

    crate::game_loop::stats::night_stats::refresh_one(&mut world, CASTER, false);
    assert_eq!(accuracy(&world), 0.0, "by day it grants nothing");

    crate::game_loop::stats::night_stats::refresh_one(&mut world, CASTER, true);
    assert_eq!(accuracy(&world), 3.0, "at night the accuracy appears");

    crate::game_loop::stats::night_stats::refresh_one(&mut world, CASTER, false);
    assert_eq!(accuracy(&world), 0.0, "and dawn takes it back again");
}
