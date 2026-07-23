//! Raid-boss persistence (G21 slice 3): `DBSpawnManager` / `npc_respawns`.

use super::*;

use crate::db::NpcRespawnRow;
use crate::model::components::Vitals;

const BOSS_ID: i32 = 25999;

/// Register a raid-boss template plus a `dbSave` spawn line for it, exactly as
/// `RaidbossSpawns.xml` declares one (24 h respawn).
fn boss_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = combat_test_world();
    let mut t = crate::data::npc_data::default_template(BOSS_ID);
    t.type_name = "RaidBoss".into();
    t.name = "Test Raid Boss".into();
    t.level = 60;
    t.base_hp_max = 10_000.0;
    t.base_mp_max = 2_000.0;
    t.collision_radius = 20.0;
    world.data.npc_data.insert_for_test(t);

    world
        .data
        .spawn_data
        .spawns
        .push(crate::data::spawn_data::SpawnTemplate {
            groups: vec![crate::data::spawn_data::SpawnGroup {
                npcs: vec![crate::data::spawn_data::NpcSpawnDef {
                    npc_id: BOSS_ID,
                    count: 1,
                    loc: Some(crate::data::spawn_data::FixedLoc {
                        x: 5000,
                        y: 5000,
                        z: 0,
                        heading: 0,
                    }),
                    respawn_secs: 86_400,
                    respawn_random_secs: 0,
                    db_save: true,
                }],
                territories: Vec::new(),
            }],
            name: Some("test_raidboss".into()),
            territories: Vec::new(),
        });
    world.id_pool = 0x2100_0000..0x2100_1000;
    (world, db, l)
}

fn live_boss(world: &mut World) -> Option<i32> {
    let mut found = None;
    world
        .objects
        .for_each_mut::<(&crate::model::npc::Npc, &Vitals)>(|(n, v)| {
            if n.npc_id == BOSS_ID && !v.dead {
                found = Some(n.object_id);
            }
        });
    found
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn row(respawn_time: i64, cur_hp: f64, cur_mp: f64) -> NpcRespawnRow {
    NpcRespawnRow {
        npc_id: BOSS_ID,
        x: 5000,
        y: 5000,
        z: 0,
        heading: 0,
        respawn_time,
        cur_hp,
        cur_mp,
    }
}

// ---------------------------------------------------------------------------

#[test]
fn static_pass_defers_db_save_spawns_instead_of_placing_them() {
    // The whole ownership split: `spawn_all` must NOT place a dbSave boss, or
    // the DB restore below would double-spawn it.
    let (mut world, _db, _l) = boss_world();

    crate::model::npc::spawn_all(&mut world);

    assert!(
        live_boss(&mut world).is_none(),
        "a dbSave boss must not be placed by the static spawn pass"
    );
    assert_eq!(
        world.pending_boss_spawns.len(),
        1,
        "it should be queued for DBSpawnManager instead"
    );
}

#[test]
fn boss_with_no_stored_row_spawns_at_full_hp_and_is_persisted() {
    let (mut world, mut db, _l) = boss_world();
    crate::model::npc::spawn_all(&mut world);

    crate::game_loop::boss_respawn::resolve_boot(&mut world, Vec::new());

    let oid = live_boss(&mut world).expect("fresh DB → boss spawns");
    let v = world.objects.get_component::<Vitals>(&oid).unwrap();
    assert_eq!(v.cur_hp, v.max_hp as f64, "no stored row → full HP");
    let cmds = drain_db(&mut db);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            db::DbCommand::StoreNpcRespawn {
                npc_id: BOSS_ID,
                respawn_time: 0,
                ..
            }
        )),
        "a newly placed boss inserts its row (Java addNewSpawn storeInDb=true)"
    );
}

#[test]
fn stored_hp_is_restored_across_a_restart() {
    // The gate: a boss left at 1200/10000 comes back at 1200, not full.
    let (mut world, _db, _l) = boss_world();
    crate::model::npc::spawn_all(&mut world);

    crate::game_loop::boss_respawn::resolve_boot(&mut world, vec![row(0, 1200.0, 300.0)]);

    let oid = live_boss(&mut world).expect("alive row → boss spawns");
    let v = world.objects.get_component::<Vitals>(&oid).unwrap();
    assert_eq!(v.cur_hp, 1200.0, "stored HP must survive the restart");
    assert_eq!(v.cur_mp, 300.0, "stored MP too");
}

#[test]
fn a_boss_still_on_its_respawn_timer_does_not_spawn() {
    let (mut world, _db, _l) = boss_world();
    crate::model::npc::spawn_all(&mut world);

    // Due in an hour.
    crate::game_loop::boss_respawn::resolve_boot(
        &mut world,
        vec![row(now_ms() + 3_600_000, 0.0, 0.0)],
    );

    assert!(
        live_boss(&mut world).is_none(),
        "a boss killed before the restart stays dead until its time"
    );
    assert!(
        world.boss_spawn_refs.contains_key(&BOSS_ID),
        "but it is tracked for the scheduled respawn"
    );
}

#[test]
fn an_elapsed_respawn_time_spawns_the_boss_immediately() {
    let (mut world, _db, _l) = boss_world();
    crate::model::npc::spawn_all(&mut world);

    // Due an hour ago — the server was down past the window.
    crate::game_loop::boss_respawn::resolve_boot(
        &mut world,
        vec![row(now_ms() - 3_600_000, 0.0, 0.0)],
    );

    let oid = live_boss(&mut world).expect("overdue boss spawns at boot");
    let v = world.objects.get_component::<Vitals>(&oid).unwrap();
    assert_eq!(
        v.cur_hp, v.max_hp as f64,
        "a respawn is a fresh boss, at full HP"
    );
}

#[test]
fn a_dead_rows_zero_hp_is_not_restored_onto_a_respawned_boss() {
    // Guard against the obvious bug: a row written at death holds currentHp 0,
    // and restoring that literally would spawn a corpse.
    let (mut world, _db, _l) = boss_world();
    crate::model::npc::spawn_all(&mut world);

    crate::game_loop::boss_respawn::resolve_boot(&mut world, vec![row(now_ms() - 1000, 0.0, 0.0)]);

    let oid = live_boss(&mut world).expect("spawns");
    let v = world.objects.get_component::<Vitals>(&oid).unwrap();
    assert!(v.cur_hp > 0.0, "must not spawn on 0 HP");
    assert!(!v.dead);
}

#[test]
fn stored_hp_above_the_template_max_is_clamped() {
    let (mut world, _db, _l) = boss_world();
    crate::model::npc::spawn_all(&mut world);

    crate::game_loop::boss_respawn::resolve_boot(&mut world, vec![row(0, 999_999.0, 999_999.0)]);

    let oid = live_boss(&mut world).expect("spawns");
    let v = world.objects.get_component::<Vitals>(&oid).unwrap();
    assert_eq!(
        v.cur_hp, v.max_hp as f64,
        "a stale over-max row clamps rather than over-filling"
    );
}

#[test]
fn killing_a_boss_banks_its_absolute_respawn_time() {
    let (mut world, mut db, _l) = boss_world();
    crate::model::npc::spawn_all(&mut world);
    crate::game_loop::boss_respawn::resolve_boot(&mut world, Vec::new());
    let oid = live_boss(&mut world).expect("spawned");
    let _ = drain_db(&mut db); // discard the spawn-time insert

    // Kill it and run the corpse through to decay.
    {
        let v = world.objects.get_component_mut::<Vitals>(&oid).unwrap();
        v.cur_hp = 0.0;
        v.dead = true;
    }
    crate::game_loop::death::handle_npc_decay(&mut world, oid);

    let cmds = drain_db(&mut db);
    let banked = cmds.iter().find_map(|c| match c {
        db::DbCommand::StoreNpcRespawn {
            npc_id: BOSS_ID,
            respawn_time,
            ..
        } => Some(*respawn_time),
        _ => None,
    });
    let banked = banked.expect("a dbSave boss's death must write its respawn row");
    // 24 h out, give or take the moment the test ran.
    let expected = now_ms() + 86_400 * 1000;
    assert!(
        (banked - expected).abs() < 60_000,
        "expected the respawn banked ~24h out, got a {} ms difference",
        banked - expected
    );
}

#[test]
fn an_ordinary_monster_death_writes_no_respawn_row() {
    // Only dbSave spawns are DB-owned; a normal mob dying must not touch the
    // table (there are tens of thousands of them).
    let (mut world, mut db, _l) = boss_world();
    add_test_npc(&mut world, NPC_OID, 40001, "Monster", 5, 100, 0, 0);
    let _ = drain_db(&mut db);
    {
        let v = world.objects.get_component_mut::<Vitals>(&NPC_OID).unwrap();
        v.cur_hp = 0.0;
        v.dead = true;
    }
    crate::game_loop::death::handle_npc_decay(&mut world, NPC_OID);

    let cmds = drain_db(&mut db);
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, db::DbCommand::StoreNpcRespawn { .. })),
        "a plain monster must not write to npc_respawns"
    );
}

#[test]
fn shutdown_flushes_living_boss_hp() {
    let (mut world, mut db, _l) = boss_world();
    crate::model::npc::spawn_all(&mut world);
    crate::game_loop::boss_respawn::resolve_boot(&mut world, Vec::new());
    let oid = live_boss(&mut world).expect("spawned");
    world
        .objects
        .get_component_mut::<Vitals>(&oid)
        .unwrap()
        .cur_hp = 4321.0;
    let _ = drain_db(&mut db);

    crate::game_loop::boss_respawn::save_all_bosses(&mut world);

    let cmds = drain_db(&mut db);
    assert!(
        cmds.iter().any(
            |c| matches!(c, db::DbCommand::StoreNpcRespawn { npc_id: BOSS_ID, cur_hp, respawn_time: 0, .. }
                if (*cur_hp - 4321.0).abs() < 0.5)
        ),
        "the shutdown flush must write the boss's current HP"
    );
}

// ---------------------------------------------------------------------------
// Real datapack.

#[test]
fn real_dist_flags_raid_bosses_as_db_save() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    let spawns = crate::data::SpawnData::load_from(DIST);
    let db_save_count: usize = spawns
        .spawns
        .iter()
        .flat_map(|t| t.groups.iter())
        .flat_map(|g| g.npcs.iter())
        .filter(|d| d.db_save)
        .count();
    // 225 `dbSave="true"` lines on this dist, all in RaidbossSpawns.xml.
    assert_eq!(
        db_save_count, 225,
        "expected the dist's 225 dbSave raid-boss spawns"
    );
}
