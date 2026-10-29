//! What a hit shakes off in `CreatureStatus.reduceHp` / `PlayerStatus.reduceHp`:
//! a stun (`calcStunBreak`) and a Real Target mark (`calcRealTargetBreak`), on
//! a monster as on a player.

use super::*;

const REAL_TARGET_ID: i32 = 9320;

fn breaks_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = cc_world();
    world.data.skill_data.insert_for_test(cc_skill(
        REAL_TARGET_ID,
        SkillEffect::StatModifier(crate::model::skill::effects::StatModifierEffect {
            stat: crate::model::stats::Stat::PhysicalDefence,
            amount: -10.0,
            mode: crate::model::stats::StatModifierType::Diff,
            ..Default::default()
        }),
        "REAL_TARGET",
    ));
    world.cfg.character.alt_game_stun_break = true;
    (world, db, l)
}

fn hit(world: &mut World, target: i32) {
    combat::apply_physical_damage(world, CASTER, target, 1.0, combat::DamageHit::default());
}

fn has(world: &World, oid: i32, skill_id: i32) -> bool {
    world
        .objects
        .get_component::<Buffs>(&oid)
        .is_some_and(|b| b.0.iter().any(|x| x.skill_id == skill_id))
}

/// `CreatureStatus.reduceHp` breaks a stun on any creature: a monster rolling
/// `Rnd.get(14) == 0` is freed, as a player is.
#[test]
fn a_hit_can_break_a_monsters_stun() {
    let (mut world, _db, _l) = breaks_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);
    land(&mut world, STUN_ID, NPC_OID);
    assert!(abnormal::is_blocked_from_actions(&world, NPC_OID));

    // The stun roll misses (1), then the Real Target roll (50).
    world.force_rolls([1, 50]);
    hit(&mut world, NPC_OID);
    assert!(has(&world, NPC_OID, STUN_ID), "13 in 14 hits leave it");

    world.force_rolls([0, 50]);
    hit(&mut world, NPC_OID);
    assert!(!has(&world, NPC_OID, STUN_ID), "the 14th frees the mob");
    assert!(!abnormal::is_blocked_from_actions(&world, NPC_OID));
}

/// `calcStunBreak`'s `info.getTime() <= info.getSkill().getAbnormalTime()`: a
/// stun a Skill Mastery proc doubled survives the winning roll until it has
/// burned back down to its normal length.
#[test]
fn a_mastery_doubled_stun_holds_until_it_is_back_to_normal_length() {
    let (mut world, _db, _l) = breaks_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    land(&mut world, STUN_ID, VICTIM);
    // 18 s left on a 9 s stun: the doubled duration.
    let now = world.tick;
    for b in &mut world.objects.get_component_mut::<Buffs>(&VICTIM).unwrap().0 {
        if b.skill_id == STUN_ID {
            b.expires_at_tick = now + 180;
        }
    }
    world.force_rolls([0, 50]);
    hit(&mut world, VICTIM);
    assert!(has(&world, VICTIM, STUN_ID), "18 s left > the stun's 9 s");

    // Ten seconds on, 8 s remain — within the normal length.
    world.tick += 100;
    world.force_rolls([0, 50]);
    hit(&mut world, VICTIM);
    assert!(!has(&world, VICTIM, STUN_ID));
}

/// `calcRealTargetBreak`: `Rnd.get(100) <= 3` strips the mark, on a player and
/// on a monster; a DoT tick never rolls.
#[test]
fn a_hit_can_strip_a_real_target_mark() {
    let (mut world, _db, _l) = breaks_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);
    land(&mut world, REAL_TARGET_ID, VICTIM);
    land(&mut world, REAL_TARGET_ID, NPC_OID);

    world.force_rolls([4]);
    hit(&mut world, VICTIM);
    assert!(has(&world, VICTIM, REAL_TARGET_ID), "4 keeps it");
    world.force_rolls([3]);
    hit(&mut world, VICTIM);
    assert!(!has(&world, VICTIM, REAL_TARGET_ID), "3 strips it");

    world.force_rolls([0]);
    combat::apply_physical_damage(
        &mut world,
        CASTER,
        NPC_OID,
        1.0,
        combat::DamageHit {
            dot: true,
            ..Default::default()
        },
    );
    assert!(
        has(&world, NPC_OID, REAL_TARGET_ID),
        "a DoT tick does not roll"
    );
    hit(&mut world, NPC_OID);
    assert!(!has(&world, NPC_OID, REAL_TARGET_ID));
}
