//! `AggroDistanceCheck` — the chase leash in `AttackableAI.thinkAttack`.
//!
//! A monster dragged past its leash radius forgets every attacker, heals to
//! full and snaps back onto its spawn point with its escort. The dist enables
//! it (`NPC.ini`: `AggroDistanceCheckEnabled = True`, 2000 / 4000 units,
//! `RestoreLife = True`) — the `Default` config the fixtures use matches Java's
//! own code defaults (on, 1500 / 3000), so the numbers below are the 1500 one.

use super::*;

use crate::model::components::Vitals;
use crate::model::npc::{AggroList, Npc, NpcAi, NpcIntention};

const PLAYER: i32 = 3001;
const MOB: i32 = NPC_OID;
const MOB_ID: i32 = 45000;

/// A hurt monster standing at `at`, mid-fight with the player, whose spawn
/// point is `home`. Returns its object id.
fn place_dragged_mob(
    world: &mut World,
    type_name: &str,
    at: (i32, i32),
    home: (i32, i32, i32),
    chase_range: i32,
) -> i32 {
    add_test_npc(world, MOB, MOB_ID, type_name, 20, at.0, at.1, 0);
    {
        let npc = world.objects.get_component_mut::<Npc>(&MOB).unwrap();
        npc.spawn_loc = home;
        npc.chase_range = chase_range;
    }
    hurt(world, MOB);
    engage(world, MOB);
    MOB
}

fn hurt(world: &mut World, oid: i32) {
    let v = world.objects.get_component_mut::<Vitals>(&oid).unwrap();
    v.cur_hp = 10.0;
    v.cur_mp = 5.0;
}

/// Put the mob into the attack loop with the player at the top of its hate.
fn engage(world: &mut World, oid: i32) {
    add_hate(world, oid, PLAYER, 100.0, 100.0);
    let tick = world.tick;
    let ai = world.objects.get_component_mut::<NpcAi>(&oid).unwrap();
    ai.intention = NpcIntention::Attack;
    ai.global_aggro = 0;
    // Well past this think, so the *leash* is what fires and not the
    // attack-timeout branch below it (which also relocates).
    ai.attack_timeout_tick = tick + 10_000;
}

fn pos_of(world: &World, oid: i32) -> (i32, i32, i32) {
    crate::game_loop::helpers::pos_of(world, oid).expect("spawned mob has a position")
}

fn hate_count(world: &World, oid: i32) -> usize {
    world
        .objects
        .get_component::<AggroList>(&oid)
        .map(|a| a.0.len())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// The leash itself.
// ---------------------------------------------------------------------------

/// The operator-facing behaviour: drag a mob far enough and it teleports back
/// onto its spawn point at full HP/MP, with an empty hate list.
#[test]
fn a_mob_dragged_past_the_leash_teleports_home_at_full_hp() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 2000, 0);
    // 2000 from home, leash is 1500.
    place_dragged_mob(&mut world, "Monster", (2000, 0), (0, 0, 0), 0);
    drain(&mut rx);

    ai::npc_ai_tick(&mut world);

    assert_eq!(pos_of(&world, MOB), (0, 0, 0), "teleported onto its spawn");
    let v = world.objects.get_component::<Vitals>(&MOB).unwrap();
    assert_eq!(v.cur_hp, v.max_hp as f64, "healed to full HP");
    assert_eq!(v.cur_mp, v.max_mp as f64, "healed to full MP");
    assert_eq!(hate_count(&world, MOB), 0, "forgot every attacker");
    assert_eq!(
        world
            .objects
            .get_component::<NpcAi>(&MOB)
            .unwrap()
            .intention,
        NpcIntention::Active,
        "back to the scan loop"
    );
    assert!(
        !world.objects.has_component::<Movement>(&MOB),
        "the in-flight chase is dropped, or the movement sweep drags it back out"
    );
}

/// The zero case: inside the leash radius the mob keeps its target, its wounds
/// and its position.
#[test]
fn a_mob_inside_the_leash_keeps_chasing() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 1000, 0);
    // 1000 from home — under the 1500 leash.
    place_dragged_mob(&mut world, "Monster", (1000, 0), (0, 0, 0), 0);
    drain(&mut rx);

    ai::npc_ai_tick(&mut world);

    assert_ne!(pos_of(&world, MOB), (0, 0, 0), "not sent home");
    assert_eq!(hate_count(&world, MOB), 1, "still hates the player");
    let v = world.objects.get_component::<Vitals>(&MOB).unwrap();
    assert_eq!(v.cur_hp, 10.0, "not healed");
}

/// `spawn.getChaseRange()` — a spawn line that says `chaseRange="5000"` lets
/// its mobs follow that far before the leash bites. Silent Valley and Tower of
/// Insolence use this on the dist.
#[test]
fn a_spawn_line_chase_range_widens_the_leash() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 4000, 0);
    // 4000 out: past the global 1500, well inside this line's 5000.
    place_dragged_mob(&mut world, "Monster", (4000, 0), (0, 0, 0), 5000);
    drain(&mut rx);

    ai::npc_ai_tick(&mut world);

    assert_ne!(
        pos_of(&world, MOB),
        (0, 0, 0),
        "chaseRange overrode the leash"
    );
    assert_eq!(hate_count(&world, MOB), 1, "still hates the player");
}

/// Java takes `max(MAX_DRIFT_RANGE, chaseRange)`, so a chase range *below* the
/// mob's own random-walk radius can never shrink the leash under it.
#[test]
fn a_chase_range_under_the_drift_radius_is_floored_at_it() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 250, 0);
    // chaseRange 100 < MaxDriftRange 300, and the mob is 250 out: inside the
    // floor, so the leash must not fire on a mob that merely wandered.
    place_dragged_mob(&mut world, "Monster", (250, 0), (0, 0, 0), 100);
    drain(&mut rx);

    ai::npc_ai_tick(&mut world);

    assert_ne!(
        pos_of(&world, MOB),
        (0, 0, 0),
        "still floored at MaxDriftRange"
    );
    assert_eq!(hate_count(&world, MOB), 1, "still hates the player");
}

// ---------------------------------------------------------------------------
// Exemptions.
// ---------------------------------------------------------------------------

/// Grand bosses are never leashed — their own scripts own their reset rules.
#[test]
fn a_grand_boss_is_never_leashed() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 8000, 0);
    place_dragged_mob(&mut world, "GrandBoss", (8000, 0), (0, 0, 0), 0);
    drain(&mut rx);

    ai::npc_ai_tick(&mut world);

    assert_ne!(pos_of(&world, MOB), (0, 0, 0), "grand boss exempt");
    assert_eq!(hate_count(&world, MOB), 1, "keeps its hate");
}

/// A raid boss only leashes under `AggroDistanceCheckRaids` — off in the
/// fixture config (Java's code default), on in this dist's `NPC.ini`.
#[test]
fn a_raid_leashes_only_when_the_raid_flag_is_set() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 5000, 0);
    place_dragged_mob(&mut world, "RaidBoss", (5000, 0), (0, 0, 0), 0);
    drain(&mut rx);

    ai::npc_ai_tick(&mut world);
    assert_ne!(pos_of(&world, MOB), (0, 0, 0), "raids exempt by default");

    // Turn the flag on: 5000 is past the 3000 raid range.
    world.cfg.npc.aggro_distance_check_raids = true;
    ai::npc_ai_tick(&mut world);
    assert_eq!(pos_of(&world, MOB), (0, 0, 0), "raid leashed once enabled");
}

/// A route walker's home is wherever its route has it, so `!npc.isWalker()`
/// keeps the leash off it.
#[test]
fn a_route_walker_is_never_leashed() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 2000, 0);
    place_dragged_mob(&mut world, "Monster", (2000, 0), (0, 0, 0), 0);
    world.objects.add_components(
        &MOB,
        crate::game_loop::walkers::WalkState {
            route_idx: 0,
            node: 0,
            forward: true,
            travelling: false,
            resume_at: 0,
        },
    );
    drain(&mut rx);

    ai::npc_ai_tick(&mut world);

    assert_ne!(pos_of(&world, MOB), (0, 0, 0), "walker exempt");
}

/// `AggroDistanceCheckRestoreLife = False` sends the mob home still wounded.
#[test]
fn restore_life_off_sends_the_mob_home_hurt() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    world.cfg.npc.aggro_distance_check_restore_life = false;
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 2000, 0);
    place_dragged_mob(&mut world, "Monster", (2000, 0), (0, 0, 0), 0);
    drain(&mut rx);

    ai::npc_ai_tick(&mut world);

    assert_eq!(pos_of(&world, MOB), (0, 0, 0), "still goes home");
    assert_eq!(
        world.objects.get_component::<Vitals>(&MOB).unwrap().cur_hp,
        10.0,
        "but keeps its wounds"
    );
}

/// `AggroDistanceCheckEnabled = False` turns the whole block off.
#[test]
fn the_leash_can_be_switched_off() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    world.cfg.npc.aggro_distance_check_enabled = false;
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 2000, 0);
    place_dragged_mob(&mut world, "Monster", (2000, 0), (0, 0, 0), 0);
    drain(&mut rx);

    ai::npc_ai_tick(&mut world);

    assert_ne!(pos_of(&world, MOB), (0, 0, 0), "leash disabled");
    assert_eq!(hate_count(&world, MOB), 1, "keeps its hate");
}

/// The client side of the snap-back: Java's `teleToLocation` opens with
/// `decayMe()`, and `World.removeVisibleObject` there clears the selection of
/// every creature holding the teleporting object before sending the
/// `DeleteObject`. Without the `TargetUnselected` half our client keeps the mob
/// id locked and strands the ground selection ring at the drag spot (same
/// failure as the corpse ring at decay); without the unconditional
/// `DeleteObject` a same-region hop is announced by `NpcInfo` alone. A player
/// who never selected the mob must get neither packet's target half.
#[test]
fn a_leashed_mob_clears_the_selection_ring_it_leaves_behind() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 2000, 0);
    // An onlooker standing next to the puller, with nothing selected.
    let mut idle_rx = ingame_caster(&mut world, 2, PLAYER + 1, 2020, 0);
    place_dragged_mob(&mut world, "Monster", (2000, 0), (0, 0, 0), 0);

    handle_action(&mut world, 1, &action_body(MOB, 0));
    assert_eq!(
        world.objects.get_component::<TargetRef>(&PLAYER).unwrap().0,
        Some(MOB),
        "puller has the mob selected"
    );
    drain(&mut rx);
    drain(&mut idle_rx);

    ai::npc_ai_tick(&mut world);

    assert_eq!(pos_of(&world, MOB), (0, 0, 0), "leashed home");
    assert_eq!(
        world.objects.get_component::<TargetRef>(&PLAYER).unwrap().0,
        None,
        "server-side target released too"
    );
    let packets = drain(&mut rx);
    let unselect = packets
        .iter()
        .position(|p| is_for(p, server_packets::opcodes::TARGET_UNSELECTED, PLAYER));
    let delete = packets
        .iter()
        .position(|p| is_for(p, server_packets::opcodes::DELETE_OBJECT, MOB));
    let unselect = unselect.expect("TargetUnselected releases the ring");
    let delete = delete.expect("DeleteObject un-spawns the mob at its old spot");
    assert!(
        unselect < delete,
        "TargetUnselected first — Java's setTarget(null) runs before the DeleteObject"
    );
    assert!(
        packets
            .iter()
            .skip(delete)
            .any(|p| p[0] == server_packets::opcodes::NPC_INFO),
        "and the mob is re-introduced on its spawn point"
    );
    assert!(
        !drain(&mut idle_rx).iter().any(|p| is_for(
            p,
            server_packets::opcodes::TARGET_UNSELECTED,
            PLAYER + 1
        )),
        "the onlooker never selected it, so nothing to unselect"
    );
}

// ---------------------------------------------------------------------------
// "Minions should return as well."
// ---------------------------------------------------------------------------

/// The escort goes home with its leader — to the *leader's* spawn point, which
/// is what Java passes down — healed and with an empty hate list of its own.
#[test]
fn the_escort_returns_home_with_its_leader() {
    use crate::data::npc_data::MinionHolder;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, PLAYER, 2000, 0);

    const MINION_ID: i32 = 45001;
    let mut m = crate::data::npc_data::default_template(MINION_ID);
    m.type_name = "Monster".into();
    m.base_hp_max = 200.0;
    world.data.npc_data.insert_for_test(m);
    let mut leader = crate::data::npc_data::default_template(MOB_ID);
    leader.type_name = "Monster".into();
    leader.level = 20;
    leader.base_hp_max = 1000.0;
    leader.minions = vec![MinionHolder {
        npc_id: MINION_ID,
        count: 2,
        group: "Privates".into(),
    }];
    world.data.npc_data.insert_for_test(leader);
    // `add_test_npc` hand-places the leader on the allocator's next id.
    world.next_npc_object_id = MOB + 1;

    place_dragged_mob(&mut world, "Monster", (2000, 0), (0, 0, 0), 0);
    crate::game_loop::npc::minions::spawn_minions(&mut world, MOB);
    let escort = crate::game_loop::npc::minions::live_pack(&world, MOB);
    assert_eq!(escort.len(), 2, "escort spawned");
    for &oid in &escort {
        hurt(&mut world, oid);
        engage(&mut world, oid);
    }
    drain(&mut rx);

    ai::npc_ai_tick(&mut world);

    assert_eq!(pos_of(&world, MOB), (0, 0, 0), "leader went home");
    for oid in escort {
        assert_eq!(
            pos_of(&world, oid),
            (0, 0, 0),
            "minion returned to the leader's spawn point"
        );
        let v = world.objects.get_component::<Vitals>(&oid).unwrap();
        assert_eq!(v.cur_hp, v.max_hp as f64, "minion healed to full");
        assert_eq!(hate_count(&world, oid), 0, "minion forgot its attacker");
    }
}
