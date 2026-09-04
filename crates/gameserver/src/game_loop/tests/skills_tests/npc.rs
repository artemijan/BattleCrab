//! Skill behaviour on the NPC side: the corpse spoil gate and summon talk.

use super::*;

/// `NpcBody` targeting: the `OpSweeper` spoil gate belongs to the Sweeper
/// family alone — on this dist only Sweeper 42 carries the condition. A
/// corpse skill without the `Sweeper` effect (Corpse Burst 1155, Corpse Life
/// Drain 1151, Life Scavenge 46, Corpse Plague 103 — all learnable) casts on
/// any dead NPC, spoiled or not; Sweeper is still refused at cast time on an
/// unspoiled corpse and passes on the caster's own spoil.
#[test]
fn npc_body_spoil_gate_only_for_sweeper() {
    use crate::network::server_packets::sm_ids;
    use model::components::space::Position;
    use model::skill::Skill;
    use model::skill::effects::SkillEffect;
    use model::skill::target::TargetType;

    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    add_test_npc(&mut world, npc_oid, 40778, "Monster", 5, 50, 0, 0);
    world
        .objects
        .get_component_mut::<Vitals>(&npc_oid)
        .unwrap()
        .dead = true;

    let corpse_burst = Skill {
        id: 1155,
        target_type: TargetType::NpcBody,
        effects: vec![SkillEffect::MagicalAttack { power: 10.0 }],
        ..Default::default()
    };
    let sweeper = Skill {
        id: 42,
        target_type: TargetType::NpcBody,
        effects: vec![SkillEffect::Sweeper, SkillEffect::ConsumeBody],
        ..Default::default()
    };
    let caster = world.objects.get_component::<Player>(&3001).unwrap();
    let pos = *world.objects.get_component::<Position>(&3001).unwrap();

    assert_eq!(
        resolve_cast_target(
            &world,
            caster,
            &pos,
            Some(npc_oid),
            &corpse_burst,
            false,
            false
        ),
        Ok(npc_oid),
        "a corpse skill without the Sweeper effect casts on an unspoiled corpse"
    );
    assert_eq!(
        resolve_cast_target(&world, caster, &pos, Some(npc_oid), &sweeper, false, false),
        Err(sm_ids::SWEEPER_FAILED_TARGET_NOT_SPOILED),
        "Sweeper is still refused on an unspoiled corpse at cast time"
    );
    world
        .objects
        .get_component_mut::<model::npc::Npc>(&npc_oid)
        .unwrap()
        .spoiler_object_id = 3001;
    let caster = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(
        resolve_cast_target(&world, caster, &pos, Some(npc_oid), &sweeper, false, false),
        Ok(npc_oid),
        "the caster's own spoil passes the Sweeper gate"
    );
}

/// `PetAction`/`SummonAction`: an owner interacting with their **own** summon
/// never opens the NPC talk flow — it fires `ON_PLAYER_SUMMON_TALK`, whose
/// only listener on this dist is the Sin Eater (10 % chance of a grumble,
/// strings 42239–42242). A stranger's interact still takes the normal path.
#[test]
fn own_summon_interact_fires_summon_talk() {
    use model::components::summons::ServitorOf;

    let (mut world, ..) = cast_test_world();
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let pet_oid = NPC_OID + 30;
    add_test_npc(&mut world, pet_oid, 12564, "Pet", 20, 30, 0, 0);
    world.objects.add_components(
        &pet_oid,
        ServitorOf {
            owner_object_id: 3001,
            reference_skill: 0,
            expires_at_tick: u64::MAX,
            life_time_secs: 0,
            following: true,
            defending: false,
            consume_item_id: 0,
            consume_item_count: 0,
            next_consume_tick: u64::MAX,
        },
    );
    drain(&mut rx);

    // 10 % roll hits (0 < 10), then the 25 % band picks string 42240.
    world.force_rolls([5, 30]);
    interact_with_npc(&mut world, 1, 3001, pet_oid);
    let pkts = drain(&mut rx);
    let said = pkts.iter().any(|p| {
        p[0] == server_packets::opcodes::NPC_SAY
            && p[1..5] == pet_oid.to_le_bytes()
            && p.windows(4).any(|w| w == 42240i32.to_le_bytes())
    });
    assert!(said, "the Sin Eater grumbled string 42240 at its owner");

    // The 90 % miss stays quiet.
    world.force_roll(50);
    interact_with_npc(&mut world, 1, 3001, pet_oid);
    assert!(
        !drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::NPC_SAY),
        "a missed roll says nothing"
    );
}
