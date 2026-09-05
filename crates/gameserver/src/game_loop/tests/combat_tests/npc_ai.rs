//! Idle NPC behaviour: the random walk and social animations, aggro on a
//! passing player, and the spawn hook.

use super::*;

/// An idle monster with random walk enabled wanders: with no target and
/// inside its drift radius, the 1-in-30 roll fires and it moves to a random
/// spot near its spawn, broadcasting `MoveToLocation`
/// (`AttackableAI.thinkActive`'s random-walk branch).
#[test]
fn idle_monster_random_walks_near_spawn() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    {
        // 40001 is passive (won't aggro the nearby player) but wanders.
        let mut t = world.data.npc_data.get(40001).unwrap().clone();
        t.random_walk = true;
        world.data.npc_data.insert_for_test(t);
    }
    // A player keeps the spawn region active so `npc_ai_tick` visits the mob.
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 0, 0, 0, 5000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    drain(&mut a_rx);

    // Force the walk-rate hit (0) and a delta landing well within drift (300):
    // deltaX = 500, deltaY = 500 + 83 = 583 → √(583²−500²) ≈ 299 → (200, −1).
    world.force_rolls([0, 500, 83]);
    ai::npc_ai_tick(&mut world);

    let mv = world
        .objects
        .get_component::<Movement>(&npc_oid)
        .expect("idle mob started a random walk");
    let from_spawn = ((mv.0.dest_x as f64).powi(2) + (mv.0.dest_y as f64).powi(2)).sqrt();
    assert!(
        from_spawn <= world.cfg.npc.max_drift_range as f64,
        "wander destination stays within drift range"
    );
    assert!(
        (mv.0.dest_x, mv.0.dest_y) != (0, 0),
        "actually moved off the spawn spot"
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| is_for(p, server_packets::opcodes::MOVE_TO_LOCATION, npc_oid)),
        "the wander is broadcast as MoveToLocation"
    );
}

/// An idle NPC in an active region plays a random social animation once its
/// pending timer elapses, broadcasting `SocialAction` with id 2 or 3
/// (`RandomAnimationTaskManager` → `onRandomAnimation`).
#[test]
fn idle_npc_plays_random_social_animation() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 0, 0, 0, 5000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    // Pretend the animation timer already elapsed (skip the 5–60 s wait).
    world.tick = 100;
    world
        .objects
        .get_component_mut::<NpcAi>(&npc_oid)
        .unwrap()
        .next_animation_tick = Some(50);
    drain(&mut a_rx);

    ai::npc_ai_tick(&mut world);

    let packets = drain(&mut a_rx);
    let social = packets
        .iter()
        .find(|p| is_for(p, server_packets::opcodes::SOCIAL_ACTION, npc_oid))
        .expect("idle NPC broadcast a SocialAction");
    let action_id = i32::from_le_bytes(social[5..9].try_into().unwrap());
    assert!(
        (2..=3).contains(&action_id),
        "random idle animation is 2 or 3, got {action_id}"
    );
    // The 6 s throttle is now armed and the next attempt was rescheduled out.
    let ai = world.objects.get_component::<NpcAi>(&npc_oid).unwrap();
    assert_eq!(ai.last_social_tick, 100);
    assert!(
        ai.next_animation_tick.unwrap() > 100,
        "next animation rescheduled into the future"
    );
}

/// A moving NPC does not play idle animations even when its timer is due
/// (Java gates on `!isMoving()`).
#[test]
fn moving_npc_skips_random_animation() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 0, 0, 0, 5000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    world.tick = 100;
    world
        .objects
        .get_component_mut::<NpcAi>(&npc_oid)
        .unwrap()
        .next_animation_tick = Some(50);
    // Currently walking somewhere (`isMoving()`), so no idle animation.
    world.objects.add_components(
        &npc_oid,
        Movement(model::movement::MoveData {
            start_x: 0,
            start_y: 0,
            start_z: 0,
            dest_x: 500,
            dest_y: 0,
            dest_z: 0,
            start_tick: 100,
            total_ticks: 100,
            geo_path: None,
        }),
    );
    drain(&mut a_rx);

    ai::npc_ai_tick(&mut world);

    let packets = drain(&mut a_rx);
    assert!(
        !packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION),
        "a walking NPC plays no idle animation"
    );
    // Still rescheduled, but the throttle stayed unarmed (nothing broadcast).
    let ai = world.objects.get_component::<NpcAi>(&npc_oid).unwrap();
    assert_eq!(ai.last_social_tick, 0);
    assert!(ai.next_animation_tick.unwrap() > 100);
}

/// An aggressive monster acquires a player who just stands inside its aggro
/// range: after the spawn-calm `_globalAggro` ticks up to 0, the scan seeds
/// hate and the AI attacks unprovoked.
#[test]
fn aggressive_monster_aggros_idle_player() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    {
        // Make 40001 aggressive for this test.
        let mut t = world.data.npc_data.get(40001).unwrap().clone();
        t.is_aggressive = true;
        t.aggro_range = 300;
        world.data.npc_data.insert_for_test(t);
    }
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 150, 0, 0, 5000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    // Give the idle victim a deep HP pool: an NPC now re-swings at its true
    // weapon rate (not once per 1 s AI think), so a 100 HP player would be dead
    // — and its target-cleared AI back to ACTIVE — before the 140-tick window
    // ends. The deep pool keeps the fight going so we can observe the lock-on.
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&3001) {
        v.max_hp = 5000;
        v.cur_hp = 5000.0;
    }
    drain(&mut a_rx);

    // 10 think seconds of calm (globalAggro −10 → 0), then the scan seeds hate
    // and the AI locks on, chases in, and swings (the first swings within the
    // 140-tick window forced to plain hits; later swings roll from the rng).
    world.force_rolls([0, 99, 10, 0, 99, 10]);
    advance_world(&mut world, 140);
    assert_eq!(
        world
            .objects
            .get_component::<NpcAi>(&npc_oid)
            .unwrap()
            .intention,
        NpcIntention::Attack
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| is_for(p, server_packets::opcodes::ATTACK, npc_oid)),
        "unprovoked attack on the idle player"
    );
    assert!(pvit(&world, 3001).cur_hp < 5000.0, "the swing landed");
}

/// The `on_spawn` hook fires for registered NPCs on every (re)spawn — a
/// synthetic script stamps the NPC's script value at spawn.
#[test]
fn on_spawn_hook_fires_for_registered_npcs() {
    struct SpawnStamp;
    impl crate::game_loop::quests::QuestScript for SpawnStamp {
        fn id(&self) -> i32 {
            -1
        }
        fn name(&self) -> &'static str {
            "SpawnStamp"
        }
        fn html_dir(&self) -> &'static str {
            ""
        }
        fn start_npcs(&self) -> &[i32] {
            &[]
        }
        fn talk_npcs(&self) -> &[i32] {
            &[]
        }
        fn spawn_npcs(&self) -> &[i32] {
            &[40001]
        }
        fn on_talk(&self, _ctx: &mut crate::game_loop::quests::QuestCtx) -> Option<String> {
            None
        }
        fn on_spawn(&self, ctx: &mut crate::game_loop::quests::QuestCtx) {
            ctx.set_npc_script_value(7);
        }
    }
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    world.quests = Arc::new(crate::game_loop::quests::QuestRegistry::new(vec![
        Arc::new(SpawnStamp),
    ]));
    // Spawn through the real spawn line (template 40001 registered by
    // combat_test_world's spawn_data? — spawn directly via spawn_one needs
    // a spawn line; use notify path through add_test_npc + explicit call).
    add_test_npc(&mut world, NPC_OID, 40001, "Monster", 5, 30, 0, 0);
    crate::game_loop::quests::notify_spawn(&mut world, NPC_OID, 40001);
    assert_eq!(
        world
            .objects
            .get_component::<model::npc::Npc>(&NPC_OID)
            .unwrap()
            .script_value,
        7
    );
}
