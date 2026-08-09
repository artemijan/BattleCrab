//! Vertical-movement parity for aggressive mobs — the "tower glide" bug,
//! where a mob whose hated target moved to another floor engaged straight
//! through the geometry. Java's protections, all ported here: the aggro scan
//! is a 3D sphere (`calculateDistance3D`) gated by geodata LOS,
//! `thinkAttack` refuses to engage a target it cannot see and walks a
//! geo-validated route instead, a chase runs through the same geodata clamp
//! + path worker as any other walk, hate zeroes for targets outside the 3×3
//! surrounding regions (`AggroInfo.checkHate`), and a timed-out fighting
//! monster teleports back to its spawn.

use super::*;

use crate::geo::worker::PathRequest;
use crate::geo::{NSWE_ALL, NSWE_EAST};
use crate::model::components::{AttackState, Movement, PathWait, Position, Vitals};
use crate::model::npc::{AggroList, NpcAi, NpcIntention};

const MOB_ID: i32 = 46000;
const PLAYER: i32 = 3001;
const CID: u32 = 1;

fn aggressive_template(world: &mut World, aggro_range: i32) {
    let mut t = crate::data::npc_data::default_template(MOB_ID);
    t.type_name = "Monster".into();
    t.name = "Cliff Stalker".into();
    t.level = 20;
    t.is_aggressive = true;
    t.aggro_range = aggro_range;
    t.base_hp_max = 500.0;
    world.data.npc_data.insert_for_test(t);
}

fn intention(world: &World, oid: i32) -> NpcIntention {
    world
        .objects
        .get_component::<NpcAi>(&oid)
        .unwrap()
        .intention
}

fn hate_on(world: &World, npc: i32, target: i32) -> f64 {
    world
        .objects
        .get_component::<AggroList>(&npc)
        .and_then(|a| a.0.get(&target).map(|i| i.hate))
        .unwrap_or(0.0)
}

/// Put a mob straight into the attack loop on `target`, as if it had been
/// fighting (hate seeded, calm-after-spawn over, timeout far away).
fn force_attack(world: &mut World, npc: i32, target: i32) {
    world
        .objects
        .get_component_mut::<AggroList>(&npc)
        .unwrap()
        .0
        .entry(target)
        .or_default()
        .hate = 100.0;
    let tick = world.tick;
    let ai = world.objects.get_component_mut::<NpcAi>(&npc).unwrap();
    ai.intention = NpcIntention::Attack;
    ai.global_aggro = 0;
    ai.attack_timeout_tick = tick + 10_000;
}

fn attack_packets(packets: &[Vec<u8>], attacker: i32) -> usize {
    packets
        .iter()
        .filter(|p| {
            p[0] == server_packets::opcodes::ATTACK
                && i32::from_le_bytes(p[1..5].try_into().unwrap()) == attacker
        })
        .count()
}

/// World coords of the centre of local cell (cx, cy) of synthetic test
/// region (11, 10) — the same anchor `geo::tests` uses.
fn cell_world(g: &crate::geo::GeoEngine, cx: i32, cy: i32) -> (i32, i32) {
    (
        g.get_world_x(11 * crate::geo::region::REGION_CELLS_X + cx),
        g.get_world_y(10 * crate::geo::region::REGION_CELLS_Y + cy),
    )
}

/// An observable path channel in place of the worker (requests pile up in
/// the receiver, no replies arrive — exactly what a blocked mob sees within
/// one think).
fn observe_paths(world: &mut World) -> std::sync::mpsc::Receiver<PathRequest> {
    let (tx, rx) = std::sync::mpsc::channel();
    world.path = tx;
    rx
}

fn requests(rx: &std::sync::mpsc::Receiver<PathRequest>) -> Vec<PathRequest> {
    let mut out = Vec::new();
    while let Ok(r) = rx.try_recv() {
        out.push(r);
    }
    out
}

// ---------------------------------------------------------------------------

/// `World.forEachVisibleObjectInRange` measures in 3D: a player 380 units
/// above a mob's head is outside its 300 aggro range even while horizontally
/// on top of it — with no geodata loaded at all, so the distance metric (not
/// LOS) is what decides.
#[test]
fn the_aggro_range_is_a_3d_sphere_not_a_cylinder() {
    let (mut world, _db, _l) = combat_test_world();
    aggressive_template(&mut world, 300);
    let mut rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&PLAYER) {
        v.max_hp = 5000;
        v.cur_hp = 5000.0;
    }
    // 150 away horizontally (inside 300) but 380 below the player: 3D ≈ 409.
    add_test_npc(&mut world, NPC_OID, MOB_ID, "Monster", 20, 150, 0, 380);
    drain(&mut rx);

    // Past the 10-think calm-down and well into scan territory.
    advance_world(&mut world, 140);
    assert_eq!(
        intention(&world, NPC_OID),
        NpcIntention::Active,
        "a target outside the 3D sphere must not aggro"
    );
    assert_eq!(hate_on(&world, NPC_OID, PLAYER), 0.0);

    // Control: same horizontal offset on the mob's own level does aggro.
    add_test_npc(&mut world, NPC_OID + 1, MOB_ID, "Monster", 20, 150, 0, 0);
    advance_world(&mut world, 140);
    assert_eq!(
        intention(&world, NPC_OID + 1),
        NpcIntention::Attack,
        "the same offset in-plane is inside the sphere"
    );
}

/// `thinkAttack`'s geodata gate: a mob whose hated target is behind a tall
/// wall neither swings nor walks through it — it asks the path worker for a
/// route to the target (Java `moveTo(target)`) and stands until one arrives.
#[test]
fn a_mob_refuses_to_engage_a_target_it_cannot_see() {
    let (mut world, _db, _l) = combat_test_world();
    aggressive_template(&mut world, 300);

    let mut engine = crate::geo::GeoEngine::empty();
    engine.set_region(
        11,
        10,
        crate::geo::synthetic_region(crate::geo::wall_column),
    );
    let engine = std::sync::Arc::new(engine);
    world.geo = engine.clone();
    let prx = observe_paths(&mut world);

    let (mx, my) = cell_world(&engine, 5, 5);
    let (px, py) = cell_world(&engine, 15, 5);
    let mut rx = ingame_player(&mut world, CID, PLAYER, px, py, 0);
    add_test_npc(&mut world, NPC_OID, MOB_ID, "Monster", 20, mx, my, 0);
    force_attack(&mut world, NPC_OID, PLAYER);
    drain(&mut rx);

    advance_world(&mut world, 10); // one think

    let packets = drain(&mut rx);
    assert_eq!(
        attack_packets(&packets, NPC_OID),
        0,
        "no swing through the wall"
    );
    assert!(
        world.objects.get_component::<Movement>(&NPC_OID).is_none(),
        "no straight-line walk into the wall either"
    );
    assert!(
        world.objects.has_component::<PathWait>(&NPC_OID),
        "the mob asked the path worker for a route instead"
    );
    let reqs = requests(&prx);
    assert_eq!(reqs.len(), 1);
    assert_eq!(
        reqs[0].to,
        (px, py, 0),
        "the route is aimed at the target itself (`moveTo(target)`)"
    );
}

/// The chase leg itself is geodata-clamped: over a low fence the mob *can*
/// see its target (48-unit see-over) but cannot walk the line, so the chase
/// must re-route through the path worker instead of walking through the
/// fence — before this slice `chase()` wrote a straight-line `Movement` with
/// no geodata at all.
#[test]
fn a_chase_with_sight_but_no_walkable_line_asks_for_a_route() {
    let (mut world, _db, _l) = combat_test_world();
    aggressive_template(&mut world, 300);

    let mut engine = crate::geo::GeoEngine::empty();
    engine.set_region(
        11,
        10,
        crate::geo::synthetic_region(|x, _y| {
            if x == 10 {
                (32, 0) // a fence: seen over, not stepped through
            } else if x == 9 {
                (0, NSWE_ALL & !NSWE_EAST)
            } else {
                (0, NSWE_ALL)
            }
        }),
    );
    let engine = std::sync::Arc::new(engine);
    world.geo = engine.clone();
    let prx = observe_paths(&mut world);

    let (mx, my) = cell_world(&engine, 5, 5);
    let (px, py) = cell_world(&engine, 15, 5);
    let mut rx = ingame_player(&mut world, CID, PLAYER, px, py, 0);
    add_test_npc(&mut world, NPC_OID, MOB_ID, "Monster", 20, mx, my, 0);
    force_attack(&mut world, NPC_OID, PLAYER);
    drain(&mut rx);

    advance_world(&mut world, 10);

    let packets = drain(&mut rx);
    assert_eq!(attack_packets(&packets, NPC_OID), 0, "out of reach");
    assert_eq!(
        packets
            .iter()
            .filter(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN)
            .count(),
        0,
        "the client must not be told to chase through the fence"
    );
    assert!(
        world.objects.get_component::<Movement>(&NPC_OID).is_none(),
        "no server-side straight chase either"
    );
    assert!(
        world.objects.has_component::<PathWait>(&NPC_OID),
        "the chase re-routed through the path worker"
    );
    assert_eq!(requests(&prx).len(), 1);
}

/// The reported bug, on the real Cruma Tower geodata: a mob on the ground
/// layer of a stacked cell must neither aggro a player standing on the floor
/// above (LOS through the slab) nor — with hate already seeded — glide
/// vertically up to engage it.
#[test]
fn a_tower_mob_neither_aggros_nor_glides_to_the_floor_above() {
    let (mut world, _db, _l) = combat_test_world();
    // Wide range so the 3D sphere covers the player and geodata LOS is the
    // deciding gate.
    aggressive_template(&mut world, 1000);
    let geo = std::sync::Arc::new(crate::geo::GeoEngine::load(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/geodata"
    ))));
    world.geo = geo.clone();
    let prx = observe_paths(&mut world);

    // A stacked cell inside Cruma Tower; re-derive the two layers from the
    // data so the fixture proves its own premise.
    let (x, y) = (15481, 112173);
    let (gx, gy) = (geo.get_geo_x(x), geo.get_geo_y(y));
    let ground = geo.get_nearest_z(gx, gy, -3656);
    let above = geo.get_next_higher_z(gx, gy, ground + 100);
    assert!(
        above - ground >= 300,
        "premise: a real floor above the mob (ground {ground}, above {above})"
    );
    assert!(
        !geo.can_see_target(x, y, ground, x + 300, y, above),
        "premise: the slab blocks sight"
    );

    // Out of melee reach so the *chase* leg (the pre-fix straight-line
    // vertical glide) is what a regression would exercise.
    let mut rx = ingame_player(&mut world, CID, PLAYER, x + 300, y, above);
    add_test_npc(&mut world, NPC_OID, MOB_ID, "Monster", 20, x, y, ground);
    drain(&mut rx);

    // Phase 1 — no hate: the scan must not see through the floor.
    advance_world(&mut world, 140);
    assert_eq!(
        intention(&world, NPC_OID),
        NpcIntention::Active,
        "no aggro through the slab"
    );
    assert_eq!(hate_on(&world, NPC_OID, PLAYER), 0.0);

    // Phase 2 — hate seeded (the player shot it from above): still no
    // vertical engagement.
    force_attack(&mut world, NPC_OID, PLAYER);
    advance_world(&mut world, 10);
    let packets = drain(&mut rx);
    assert_eq!(
        attack_packets(&packets, NPC_OID),
        0,
        "no attack through the floor"
    );
    if let Some(mv) = world.objects.get_component::<Movement>(&NPC_OID) {
        assert!(
            (mv.0.dest_z - ground).abs() < 100,
            "any walk stays on the mob's own floor (dest_z {}, ground {ground})",
            mv.0.dest_z
        );
    }
    let pos = world.objects.get_component::<Position>(&NPC_OID).unwrap();
    assert!(
        (pos.z - ground).abs() < 100,
        "the mob did not glide vertically (z {}, ground {ground})",
        pos.z
    );
    // It went looking for a real route instead (stairs), if anything at all.
    let vertical_reqs = requests(&prx)
        .iter()
        .filter(|r| r.to.2 - ground > 300)
        .count();
    assert!(
        world.objects.has_component::<PathWait>(&NPC_OID) || vertical_reqs == 0,
        "engagement went through the path worker, never a straight vertical move"
    );
}

/// `AggroInfo.checkHate`: a hated target that left the NPC's 3×3
/// surrounding-region block weighs zero — the mob drops out of the attack
/// loop instead of chasing across the world.
#[test]
fn hate_zeroes_for_a_target_that_left_the_neighbourhood() {
    let (mut world, _db, _l) = combat_test_world();
    aggressive_template(&mut world, 300);
    // An observer outside aggro range keeps the mob's region active after
    // the victim is gone.
    let _obs = ingame_player(&mut world, 2, 3002, 900, 0, 0);
    let mut rx = ingame_player(&mut world, CID, PLAYER, 100, 0, 0);
    add_test_npc(&mut world, NPC_OID, MOB_ID, "Monster", 20, 0, 0, 0);
    force_attack(&mut world, NPC_OID, PLAYER);
    drain(&mut rx);

    // The victim vanishes far away (teleport): 40 000 is ~19 regions out.
    if let Some(p) = world.objects.get_component_mut::<Position>(&PLAYER) {
        p.x = 40_000;
    }
    world.set_player_region(PLAYER, crate::world::region_of(40_000, 0));

    advance_world(&mut world, 10);
    assert_eq!(
        hate_on(&world, NPC_OID, PLAYER),
        0.0,
        "checkHate zeroes a departed target"
    );
    assert_eq!(
        intention(&world, NPC_OID),
        NpcIntention::Active,
        "and the mob leaves the attack loop"
    );
}

/// `thinkAttack`'s timeout: a monster still in combat when the 2-minute
/// attack timeout runs out teleports straight back to its spawn (Java
/// `npc.teleToLocation(npc.getSpawn(), false)`) — and its aggro list is NOT
/// cleared (Java keeps it; `checkHate` is what forgets departed targets).
#[test]
fn a_timed_out_fighting_monster_teleports_home() {
    let (mut world, _db, _l) = combat_test_world();
    aggressive_template(&mut world, 300);
    let mut rx = ingame_player(&mut world, CID, PLAYER, 200, 0, 0);
    add_test_npc(&mut world, NPC_OID, MOB_ID, "Monster", 20, 0, 0, 0);
    // Dragged from home mid-fight — but within the AggroDistanceCheck leash
    // (1500), so the timeout branch (not the leash) is what fires here.
    world
        .objects
        .get_component_mut::<crate::model::npc::Npc>(&NPC_OID)
        .unwrap()
        .spawn_loc = (800, 800, 0);
    force_attack(&mut world, NPC_OID, PLAYER);
    let tick = world.tick;
    world
        .objects
        .get_component_mut::<NpcAi>(&NPC_OID)
        .unwrap()
        .attack_timeout_tick = tick; // expired
    world
        .objects
        .get_component_mut::<AttackState>(&NPC_OID)
        .unwrap()
        .stance_until_tick = tick + 150; // swung moments ago → `isInCombat()`
    drain(&mut rx);

    advance_world(&mut world, 10);
    let pos = world.objects.get_component::<Position>(&NPC_OID).unwrap();
    assert_eq!(
        (pos.x, pos.y),
        (800, 800),
        "teleported to spawn, not walked"
    );
    assert_eq!(intention(&world, NPC_OID), NpcIntention::Active);
    assert!(
        hate_on(&world, NPC_OID, PLAYER) > 0.0,
        "Java keeps the aggro list on timeout"
    );
}

/// The other half of the timeout condition: a monster that never got to
/// fight (`!isInCombat()`) with players still around does not teleport — it
/// just falls back to the scan loop where it stands.
#[test]
fn a_timed_out_idle_monster_with_company_stays_put() {
    let (mut world, _db, _l) = combat_test_world();
    aggressive_template(&mut world, 300);
    let mut rx = ingame_player(&mut world, CID, PLAYER, 900, 0, 0);
    add_test_npc(&mut world, NPC_OID, MOB_ID, "Monster", 20, 0, 0, 0);
    world
        .objects
        .get_component_mut::<crate::model::npc::Npc>(&NPC_OID)
        .unwrap()
        .spawn_loc = (800, 800, 0);
    force_attack(&mut world, NPC_OID, PLAYER);
    let tick = world.tick;
    world
        .objects
        .get_component_mut::<NpcAi>(&NPC_OID)
        .unwrap()
        .attack_timeout_tick = tick; // expired, but it never swung
    drain(&mut rx);

    advance_world(&mut world, 10);
    let pos = world.objects.get_component::<Position>(&NPC_OID).unwrap();
    assert_eq!(
        (pos.x, pos.y),
        (0, 0),
        "no teleport while visible and out of combat"
    );
    assert_eq!(intention(&world, NPC_OID), NpcIntention::Active);
}

/// `thinkAttack`'s archer range override: `if (npc.getAiType() == AIType.ARCHER)
/// range = 850 + combinedCollision`.
///
/// A bow mob engages from its flat bow range, not from the `<attack range>` on
/// its template (40 on essentially all of them). Without the override all 220
/// `ARCHER` templates on this dist walked into melee before loosing a shot,
/// which is not how an archer mob fights.
#[test]
fn an_archer_mob_shoots_from_bow_range_instead_of_closing() {
    let (mut world, _db, _l) = combat_test_world();
    aggressive_template(&mut world, 300);
    {
        let mut t = world.data.npc_data.get(MOB_ID).unwrap().clone();
        t.ai_type = crate::data::npc_data::AiType::Archer;
        t.base_atk_range = 40;
        world.data.npc_data.insert_for_test(t);
    }
    // 400 units: far outside the 40-unit melee reach, well inside 850.
    let mut rx = ingame_player(&mut world, CID, PLAYER, 400, 0, 0);
    add_test_npc(&mut world, NPC_OID, MOB_ID, "Monster", 20, 0, 0, 0);
    force_attack(&mut world, NPC_OID, PLAYER);
    drain(&mut rx);

    advance_world(&mut world, 10); // one think

    let packets = drain(&mut rx);
    assert!(
        attack_packets(&packets, NPC_OID) > 0,
        "the archer should shoot from where it stands"
    );
    assert!(
        world.objects.get_component::<Movement>(&NPC_OID).is_none(),
        "and not walk into melee first"
    );
}

/// The control: the same mob as a FIGHTER closes the distance instead.
#[test]
fn a_melee_mob_at_the_same_distance_closes_first() {
    let (mut world, _db, _l) = combat_test_world();
    aggressive_template(&mut world, 300);
    {
        let mut t = world.data.npc_data.get(MOB_ID).unwrap().clone();
        t.ai_type = crate::data::npc_data::AiType::Fighter;
        t.base_atk_range = 40;
        world.data.npc_data.insert_for_test(t);
    }
    let mut rx = ingame_player(&mut world, CID, PLAYER, 400, 0, 0);
    add_test_npc(&mut world, NPC_OID, MOB_ID, "Monster", 20, 0, 0, 0);
    force_attack(&mut world, NPC_OID, PLAYER);
    drain(&mut rx);

    advance_world(&mut world, 10);

    let packets = drain(&mut rx);
    assert_eq!(
        attack_packets(&packets, NPC_OID),
        0,
        "40-unit reach: nothing to swing at from 400 away"
    );
    assert!(
        world.objects.get_component::<Movement>(&NPC_OID).is_some(),
        "it walks in"
    );
}
