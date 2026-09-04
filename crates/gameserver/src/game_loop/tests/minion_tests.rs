//! Minions (G21 slice 4): `MinionList` — escorts, respawn, master death,
//! and pack aggro.

use super::*;

use crate::data::npc_data::MinionHolder;
use crate::game_loop::npc::minions::MinionOf;
use crate::model::components::stats::Vitals;
use crate::model::npc::{AggroList, Npc, NpcAi, NpcIntention};

const PLAYER: i32 = 2001;
const CID: u32 = 1;
const LEADER_ID: i32 = 42000;
const RAID_LEADER_ID: i32 = 42001;
const MINION_ID: i32 = 42002;
const LEADER_OID: i32 = NPC_OID;

fn leader_template(
    id: i32,
    type_name: &str,
    minions: Vec<MinionHolder>,
) -> crate::data::npc_data::NpcTemplate {
    let mut t = crate::data::npc_data::default_template(id);
    t.type_name = type_name.into();
    t.name = format!("Leader {id}");
    t.level = 20;
    t.base_hp_max = 1000.0;
    t.collision_radius = 15.0;
    t.minions = minions;
    t
}

fn minion_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    let mut m = crate::data::npc_data::default_template(MINION_ID);
    m.type_name = "Monster".into();
    m.name = "Escort".into();
    m.level = 18;
    m.base_hp_max = 200.0;
    m.collision_radius = 10.0;
    world.data.npc_data.insert_for_test(m);
    world.data.npc_data.insert_for_test(leader_template(
        LEADER_ID,
        "Monster",
        vec![MinionHolder {
            npc_id: MINION_ID,
            count: 3,
            group: "Privates".into(),
        }],
    ));
    world.data.npc_data.insert_for_test(leader_template(
        RAID_LEADER_ID,
        "RaidBoss",
        vec![MinionHolder {
            npc_id: MINION_ID,
            count: 2,
            group: "Privates".into(),
        }],
    ));
    world.id_pool = 0x2200_0000..0x2200_1000;
    // `add_test_npc` hand-places the leader at `NPC_OID`, which is exactly
    // `FIRST_NPC_OBJECT_ID` — i.e. the next id the runtime allocator will hand
    // out. Minions spawn through that allocator, so without this the first
    // minion overwrites the leader's own entity.
    world.next_npc_object_id = LEADER_OID + 1;
    (world, db, l)
}

/// Place a leader and run its escort spawn (the `spawn_one` hook, exercised
/// directly so the test doesn't need a whole spawn definition).
fn place_leader(world: &mut World, npc_id: i32) -> i32 {
    add_test_npc(world, LEADER_OID, npc_id, "Monster", 20, 0, 0, 0);
    crate::game_loop::npc::minions::spawn_minions(world, LEADER_OID);
    LEADER_OID
}

fn minions_of(world: &mut World, master: i32) -> Vec<i32> {
    let mut out = Vec::new();
    world
        .objects
        .for_each_mut::<(&Npc, &MinionOf, &Vitals)>(|(n, m, v)| {
            if m.0 == master && !v.dead {
                out.push(n.object_id);
            }
        });
    out
}

fn kill(world: &mut World, oid: i32) {
    let v = world.objects.get_component_mut::<Vitals>(&oid).unwrap();
    v.cur_hp = 0.0;
    v.dead = true;
}

// ---------------------------------------------------------------------------
// Spawning.

#[test]
fn leader_spawns_its_declared_escort() {
    let (mut world, _db, _l) = minion_world();
    place_leader(&mut world, LEADER_ID);

    assert_eq!(
        minions_of(&mut world, LEADER_OID).len(),
        3,
        "count=3 means three minions"
    );
}

#[test]
fn minions_spawn_near_the_leader() {
    let (mut world, _db, _l) = minion_world();
    place_leader(&mut world, LEADER_ID);

    // Leader is at the origin; Java's placement keeps the escort within
    // roughly `offset` (200) plus the collision allowance.
    for oid in minions_of(&mut world, LEADER_OID) {
        let p = world
            .objects
            .get_component::<Position>(&oid)
            .copied()
            .unwrap();
        let dist = ((p.x as f64).powi(2) + (p.y as f64).powi(2)).sqrt();
        assert!(
            dist <= 400.0,
            "minion spawned {dist} away — should be in a ring around the leader"
        );
    }
}

#[test]
fn topping_up_does_not_overshoot_the_declared_count() {
    // `spawnMinions` spawns `count - alreadyAlive`, so calling it repeatedly
    // (which the respawn path does) must not stack extra minions.
    let (mut world, _db, _l) = minion_world();
    place_leader(&mut world, LEADER_ID);
    crate::game_loop::npc::minions::spawn_minions(&mut world, LEADER_OID);
    crate::game_loop::npc::minions::spawn_minions(&mut world, LEADER_OID);

    assert_eq!(
        minions_of(&mut world, LEADER_OID).len(),
        3,
        "still exactly three"
    );
}

#[test]
fn a_dead_leader_spawns_nothing() {
    let (mut world, _db, _l) = minion_world();
    add_test_npc(&mut world, LEADER_OID, LEADER_ID, "Monster", 20, 0, 0, 0);
    kill(&mut world, LEADER_OID);

    crate::game_loop::npc::minions::spawn_minions(&mut world, LEADER_OID);

    assert!(
        minions_of(&mut world, LEADER_OID).is_empty(),
        "a corpse doesn't call for an escort"
    );
}

// ---------------------------------------------------------------------------
// Respawn.

#[test]
fn an_ordinary_leaders_minion_does_not_come_back() {
    // Java: `respawnTime < 0 ? (isRaid ? cfg : 0) : respawnTime` — a non-raid
    // leader's minions are gone for good.
    let (mut world, _db, _l) = minion_world();
    place_leader(&mut world, LEADER_ID);
    let victim = minions_of(&mut world, LEADER_OID)[0];
    kill(&mut world, victim);

    crate::game_loop::npc::minions::on_minion_die(&mut world, victim);
    advance_ticks(&mut world, 6000); // 10 min, well past any raid timer

    assert_eq!(
        minions_of(&mut world, LEADER_OID).len(),
        2,
        "no respawn for a plain leader's minion"
    );
}

#[test]
fn a_raid_leaders_minion_returns_after_the_configured_delay() {
    let (mut world, _db, _l) = minion_world();
    world.cfg.npc.raid_minion_respawn_time = 300_000; // 5 min, the dist value
    add_test_npc(
        &mut world,
        LEADER_OID,
        RAID_LEADER_ID,
        "Monster",
        20,
        0,
        0,
        0,
    );
    crate::game_loop::npc::minions::spawn_minions(&mut world, LEADER_OID);
    let victim = minions_of(&mut world, LEADER_OID)[0];
    kill(&mut world, victim);
    crate::game_loop::npc::minions::on_minion_die(&mut world, victim);

    advance_ticks(&mut world, 2000); // 200 s — not yet
    assert_eq!(minions_of(&mut world, LEADER_OID).len(), 1, "too early");

    advance_ticks(&mut world, 1500); // past 300 s total
    assert_eq!(
        minions_of(&mut world, LEADER_OID).len(),
        2,
        "the raid's escort is rebuilt"
    );
}

#[test]
fn a_custom_zero_override_beats_the_raid_default() {
    // `CustomMinionsRespawnTime` entries with 0 (25605..25608 on this dist)
    // mean "never respawn" even though the leader is a raid boss.
    let (mut world, _db, _l) = minion_world();
    world
        .cfg
        .npc
        .custom_minions_respawn_time
        .insert(MINION_ID, 0);
    add_test_npc(
        &mut world,
        LEADER_OID,
        RAID_LEADER_ID,
        "Monster",
        20,
        0,
        0,
        0,
    );
    crate::game_loop::npc::minions::spawn_minions(&mut world, LEADER_OID);
    let victim = minions_of(&mut world, LEADER_OID)[0];
    kill(&mut world, victim);

    crate::game_loop::npc::minions::on_minion_die(&mut world, victim);
    advance_ticks(&mut world, 6000);

    assert_eq!(
        minions_of(&mut world, LEADER_OID).len(),
        1,
        "an explicit 0 override means gone for good"
    );
}

#[test]
fn no_respawn_once_the_leader_is_dead() {
    let (mut world, _db, _l) = minion_world();
    add_test_npc(
        &mut world,
        LEADER_OID,
        RAID_LEADER_ID,
        "Monster",
        20,
        0,
        0,
        0,
    );
    crate::game_loop::npc::minions::spawn_minions(&mut world, LEADER_OID);
    let victim = minions_of(&mut world, LEADER_OID)[0];
    kill(&mut world, victim);
    kill(&mut world, LEADER_OID);

    crate::game_loop::npc::minions::on_minion_die(&mut world, victim);
    advance_ticks(&mut world, 6000);

    assert_eq!(
        minions_of(&mut world, LEADER_OID).len(),
        1,
        "a dead leader rebuilds nothing"
    );
}

// ---------------------------------------------------------------------------
// Master death.

#[test]
fn a_raid_leaders_death_clears_its_escort() {
    let (mut world, _db, _l) = minion_world();
    add_test_npc(
        &mut world,
        LEADER_OID,
        RAID_LEADER_ID,
        "Monster",
        20,
        0,
        0,
        0,
    );
    crate::game_loop::npc::minions::spawn_minions(&mut world, LEADER_OID);
    assert_eq!(minions_of(&mut world, LEADER_OID).len(), 2);

    kill(&mut world, LEADER_OID);
    crate::game_loop::npc::minions::on_master_die(&mut world, LEADER_OID);

    assert!(
        minions_of(&mut world, LEADER_OID).is_empty(),
        "a dead raid boss takes its escort with it"
    );
}

#[test]
fn an_ordinary_leaders_death_leaves_its_minions_alive() {
    // Java's default (`ForceDeleteMinions = False` here): only raids clear.
    // This is why killing the big mob in a camp doesn't evaporate the camp.
    let (mut world, _db, _l) = minion_world();
    place_leader(&mut world, LEADER_ID);
    kill(&mut world, LEADER_OID);

    crate::game_loop::npc::minions::on_master_die(&mut world, LEADER_OID);

    assert_eq!(
        minions_of(&mut world, LEADER_OID).len(),
        3,
        "a plain leader's minions outlive it"
    );
}

#[test]
fn force_delete_minions_clears_an_ordinary_leaders_escort_too() {
    let (mut world, _db, _l) = minion_world();
    world.cfg.npc.force_delete_minions = true;
    place_leader(&mut world, LEADER_ID);
    kill(&mut world, LEADER_OID);

    crate::game_loop::npc::minions::on_master_die(&mut world, LEADER_OID);

    assert!(
        minions_of(&mut world, LEADER_OID).is_empty(),
        "ForceDeleteMinions overrides the raid-only rule"
    );
}

// ---------------------------------------------------------------------------
// Pack aggro.

#[test]
fn attacking_a_minion_pulls_in_the_leader_and_the_pack() {
    let (mut world, _db, _l) = minion_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    place_leader(&mut world, LEADER_ID);
    let pack = minions_of(&mut world, LEADER_OID);

    crate::game_loop::npc::minions::on_assist(&mut world, pack[0], PLAYER);

    assert!(
        hate_on(&world, LEADER_OID, PLAYER) > 0.0,
        "the leader joins in"
    );
    for oid in &pack {
        assert!(
            hate_on(&world, *oid, PLAYER) > 0.0,
            "every free minion joins in"
        );
    }
}

#[test]
fn attacking_the_leader_aggros_the_pack_harder_than_hitting_a_minion() {
    // Java: aggro is 10 when the leader was struck, 1 when a minion was.
    let (mut world, _db, _l) = minion_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    place_leader(&mut world, LEADER_ID);
    let pack = minions_of(&mut world, LEADER_OID);

    crate::game_loop::npc::minions::on_assist(&mut world, LEADER_OID, PLAYER);
    let via_leader = hate_on(&world, pack[0], PLAYER);

    // Reset and hit a minion instead.
    for oid in &pack {
        world
            .objects
            .get_component_mut::<AggroList>(oid)
            .unwrap()
            .0
            .clear();
        world
            .objects
            .get_component_mut::<NpcAi>(oid)
            .unwrap()
            .intention = NpcIntention::Active;
    }
    crate::game_loop::npc::minions::on_assist(&mut world, pack[0], PLAYER);
    let via_minion = hate_on(&world, pack[1], PLAYER);

    assert!(
        via_leader > via_minion,
        "hitting the leader should aggro the pack harder ({via_leader} vs {via_minion})"
    );
}

#[test]
fn a_raid_leader_multiplies_the_pack_aggro() {
    let (mut world, _db, _l) = minion_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(
        &mut world,
        LEADER_OID,
        RAID_LEADER_ID,
        "Monster",
        20,
        0,
        0,
        0,
    );
    crate::game_loop::npc::minions::spawn_minions(&mut world, LEADER_OID);
    let pack = minions_of(&mut world, LEADER_OID);

    crate::game_loop::npc::minions::on_assist(&mut world, LEADER_OID, PLAYER);

    // 10 (leader struck) x10 (raid) = 100.
    assert_eq!(
        hate_on(&world, pack[0], PLAYER),
        100.0,
        "raid packs aggro ten times harder"
    );
}
