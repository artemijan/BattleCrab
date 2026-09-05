//! Sleep: what wakes a slept player, and what a slept monster ignores.

use super::*;

/// A sleep: `BlockActions` like a stun, but carrying the `<removedOnDamage>`
/// tag every real `Sleep` (1069, 1072, 4046, …) declares.
fn sleep_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = cc_world();
    let mut sleep = cc_skill(
        SLEEP_ID,
        SkillEffect::BlockActions { conditional: false },
        "SLEEP",
    );
    sleep.removed_on_damage = true;
    world.data.skill_data.insert_for_test(sleep);
    (world, db, l)
}

/// The bug this fixes: a slept player stayed action-blocked while the mob that
/// slept them beat on them. Java's `PlayerStatus.reduceHp` calls
/// `stopEffectsOnDamage()` on every hit, so the first blow wakes them.
///
/// The stun in the same world is the control: it carries no `<removedOnDamage>`
/// tag and must survive the identical hit, proving the removal keys off the tag
/// rather than clearing crowd control wholesale.
#[test]
fn a_hit_wakes_a_slept_player_but_leaves_a_stun_alone() {
    let (mut world, _db, _l) = sleep_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);

    land(&mut world, SLEEP_ID, VICTIM);
    assert!(
        abnormal::is_blocked_from_actions(&world, VICTIM),
        "the sleep landed"
    );

    combat::apply_physical_damage(&mut world, CASTER, VICTIM, 10.0, false, false);
    assert!(
        !abnormal::is_blocked_from_actions(&world, VICTIM),
        "one hit wakes the sleeper"
    );
    assert!(
        world
            .objects
            .get_component::<Buffs>(&VICTIM)
            .is_none_or(|b| b.0.iter().all(|x| x.skill_id != SLEEP_ID)),
        "and the buff row is gone, not just the flag"
    );

    // Control: same hit, a stun instead — a stun is not `removedOnDamage`, so
    // `stopEffectsOnDamage` must leave it be.
    let (mut world, _db, _l) = sleep_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    // `BreakStun` is turned **off** here deliberately. It ships `True` on this
    // dist — the comment that used to sit here claimed the opposite — and gives
    // every non-DoT hit a 1-in-14 chance to free a stunned target. That is a
    // *different* mechanic (`Formulas.calcStunBreak`, tested separately); left
    // on, it would make the assertion below fail one time in fourteen.
    world.cfg.character.alt_game_stun_break = false;
    land(&mut world, STUN_ID, VICTIM);
    combat::apply_physical_damage(&mut world, CASTER, VICTIM, 10.0, false, false);
    assert!(
        abnormal::is_blocked_from_actions(&world, VICTIM),
        "a stun is not `removedOnDamage` — hitting a stunned target does not \
         itself free them"
    );
}

/// The same tag on a slept **mob**: the first blow wakes it so it can fight
/// back, which is what makes sleep a pull tool rather than a permanent lock.
#[test]
fn a_hit_wakes_a_slept_monster() {
    let (mut world, _db, _l) = sleep_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);

    land(&mut world, SLEEP_ID, NPC_OID);
    assert!(
        abnormal::is_blocked_from_actions(&world, NPC_OID),
        "the mob is asleep"
    );

    combat::apply_physical_damage(&mut world, CASTER, NPC_OID, 5.0, false, false);
    assert!(
        !abnormal::is_blocked_from_actions(&world, NPC_OID),
        "and it wakes on the first blow"
    );
}

/// Java's one asymmetry between the two `reduceHp` overrides:
/// `CreatureStatus` guards the whole wake block with `!isDOT`, while
/// `PlayerStatus` puts `stopEffectsOnDamage()` *above* its `if (!isDOT)` block.
/// So a poison tick wakes a sleeping player but not a sleeping mob.
#[test]
fn a_dot_tick_wakes_a_slept_player_but_not_a_slept_mob() {
    let (mut world, _db, _l) = sleep_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);

    land(&mut world, SLEEP_ID, VICTIM);
    land(&mut world, SLEEP_ID, NPC_OID);

    combat::apply_physical_damage(&mut world, CASTER, VICTIM, 3.0, true, false);
    combat::apply_physical_damage(&mut world, CASTER, NPC_OID, 3.0, true, false);

    assert!(
        !abnormal::is_blocked_from_actions(&world, VICTIM),
        "a DoT tick still wakes a player"
    );
    assert!(
        abnormal::is_blocked_from_actions(&world, NPC_OID),
        "but a DoT tick alone does not rouse a mob"
    );
}

// ---------------------------------------------------------------------------
// G34 S3 — the flag-only abnormal states. Each is one `effect_flag` bit plus
// the single Java gate that reads it; `cc_skill`'s fixtures make each bit
// landable on demand.
// ---------------------------------------------------------------------------
