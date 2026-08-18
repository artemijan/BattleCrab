//! The grand-boss respawn lifecycle.

use super::*;

use crate::game_loop::grand_boss::{ALIVE, DEAD};

/// Queen Ant — the first boss this drives.
const QUEEN: i32 = 29001;
const QUEEN_OID: i32 = NPC_OID + 60;

fn boss_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    let mut t = crate::data::npc_data::default_template(QUEEN);
    t.type_name = "GrandBoss".into();
    t.level = 40;
    t.base_hp_max = 100_000.0;
    world.data.npc_data.insert_for_test(t);
    world.grand_bosses.insert(
        QUEEN,
        model::grand_boss::GrandBoss {
            boss_id: QUEEN,
            loc_x: -21610,
            loc_y: 181594,
            loc_z: -5734,
            heading: 0,
            respawn_time: 0,
            current_hp: 0.0,
            current_mp: 0.0,
            status: ALIVE,
        },
    );
    (world, db, l)
}

fn boss_alive_in_world(world: &mut World) -> bool {
    let mut found = false;
    world.objects.for_each_mut::<&model::npc::Npc>(|n| {
        if n.npc_id == QUEEN {
            found = true;
        }
    });
    found
}

/// Regression: grand bosses must spawn when their `grandboss_data` row lands as
/// a `DbEvent`, not from the pre-loop boot call (where `grand_bosses` is still
/// empty). This is why the Queen was never in her nest.
#[test]
fn grand_bosses_spawn_when_their_data_arrives() {
    let (mut world, _db, _l) = combat_test_world();
    let mut t = crate::data::npc_data::default_template(QUEEN);
    t.type_name = "GrandBoss".into();
    t.base_hp_max = 100_000.0;
    world.data.npc_data.insert_for_test(t);
    assert!(world.grand_bosses.is_empty());
    assert!(
        !boss_alive_in_world(&mut world),
        "nothing spawned before the data arrives"
    );

    // The DB delivers the row; handling the event both loads it *and* resolves
    // the boot spawn.
    handle_db_event(
        &mut world,
        DbEvent::GrandBossesLoaded {
            bosses: vec![model::grand_boss::GrandBoss {
                boss_id: QUEEN,
                loc_x: -21610,
                loc_y: 181594,
                loc_z: -5734,
                heading: 0,
                respawn_time: 0,
                current_hp: 0.0,
                current_mp: 0.0,
                status: ALIVE,
            }],
        },
    );

    assert!(
        boss_alive_in_world(&mut world),
        "the Queen spawns once her data lands"
    );
}

/// Killing a grand boss marks it dead and arms a respawn window — the window
/// is randomised, so it is asserted as a range, not a value.
#[test]
fn killing_a_grand_boss_arms_a_respawn_window() {
    let (mut world, _db, _l) = boss_world();
    add_test_npc(&mut world, QUEEN_OID, QUEEN, "GrandBoss", 40, 0, 0, 0);

    crate::game_loop::grand_boss::on_grand_boss_killed(&mut world, QUEEN);

    let b = world.grand_bosses.get(&QUEEN).unwrap();
    assert_eq!(b.status, DEAD);
    // Queen Ant: 36 h ± 17 h.
    let hours = (b.respawn_time - commons::util::now_millis()) as f64 / 3_600_000.0;
    assert!(
        (19.0..=53.5).contains(&hours),
        "respawn within the configured window ({hours:.1} h)"
    );
}

/// The window is genuinely random — a fixed respawn would make every boss
/// predictable, which is what the spread exists to prevent.
#[test]
fn the_respawn_window_varies() {
    let mut seen = std::collections::HashSet::new();
    for _ in 0..25 {
        let (mut world, _db, _l) = boss_world();
        crate::game_loop::grand_boss::on_grand_boss_killed(&mut world, QUEEN);
        let b = world.grand_bosses.get(&QUEEN).unwrap();
        seen.insert((b.respawn_time - commons::util::now_millis()) / 3_600_000);
    }
    assert!(
        seen.len() > 1,
        "the respawn hour varies across kills, got {seen:?}"
    );
}

/// The timer firing brings the boss back, alive and in the world.
#[test]
fn the_respawn_timer_brings_the_boss_back() {
    let (mut world, _db, _l) = boss_world();
    world.grand_bosses.get_mut(&QUEEN).unwrap().status = DEAD;

    crate::game_loop::grand_boss::handle_grand_boss_respawn(&mut world, QUEEN);

    assert_eq!(world.grand_bosses.get(&QUEEN).unwrap().status, ALIVE);
    assert_eq!(
        world.grand_bosses.get(&QUEEN).unwrap().respawn_time,
        0,
        "the window is cleared"
    );
    assert!(
        boss_alive_in_world(&mut world),
        "and it is standing in the world"
    );
}

/// A respawn for a boss that is already alive must not stack a second one —
/// a duplicate timer or an admin spawn would otherwise double the boss.
#[test]
fn respawning_an_already_alive_boss_is_a_no_op() {
    let (mut world, _db, _l) = boss_world();
    crate::game_loop::grand_boss::handle_grand_boss_respawn(&mut world, QUEEN);

    let before = npc_count(&mut world, QUEEN);
    crate::game_loop::grand_boss::handle_grand_boss_respawn(&mut world, QUEEN);
    assert_eq!(npc_count(&mut world, QUEEN), before, "no second boss");
}

/// Boot with a boss **alive**: it is placed at its stored location, with its
/// stored HP — a boss wounded before a restart comes back wounded.
#[test]
fn boot_spawns_a_living_boss_with_its_stored_hp() {
    let (mut world, _db, _l) = boss_world();
    world.grand_bosses.get_mut(&QUEEN).unwrap().current_hp = 4_242.0;

    crate::game_loop::grand_boss::resolve_at_boot(&mut world);

    let mut hp = None;
    world
        .objects
        .for_each_mut::<(&model::npc::Npc, &Vitals)>(|(n, v)| {
            if n.npc_id == QUEEN {
                hp = Some(v.cur_hp);
            }
        });
    assert_eq!(hp, Some(4_242.0), "came back as wounded as it was left");
}

/// Boot with a window that **elapsed while the server was down** spawns the
/// boss immediately. Miss this and the boss stays dead forever: nothing else
/// can bring it back, because killing it again is impossible.
#[test]
fn boot_immediately_respawns_a_boss_whose_window_elapsed() {
    let (mut world, _db, _l) = boss_world();
    {
        let b = world.grand_bosses.get_mut(&QUEEN).unwrap();
        b.status = DEAD;
        b.respawn_time = commons::util::now_millis() - 1000; // expired
    }

    crate::game_loop::grand_boss::resolve_at_boot(&mut world);

    assert_eq!(world.grand_bosses.get(&QUEEN).unwrap().status, ALIVE);
    assert!(
        boss_alive_in_world(&mut world),
        "spawned rather than left dead forever"
    );
}

/// Boot with a window **still running** leaves the boss dead and waiting.
#[test]
fn boot_leaves_a_pending_boss_dead() {
    let (mut world, _db, _l) = boss_world();
    {
        let b = world.grand_bosses.get_mut(&QUEEN).unwrap();
        b.status = DEAD;
        b.respawn_time = commons::util::now_millis() + 10 * 3_600_000;
    }

    crate::game_loop::grand_boss::resolve_at_boot(&mut world);

    assert_eq!(world.grand_bosses.get(&QUEEN).unwrap().status, DEAD);
    assert!(
        !boss_alive_in_world(&mut world),
        "still waiting out its window"
    );
}

/// Zaken (29022). His whole Java script is the shared lifecycle — spawn at
/// the deck, die, roll 60 h ± 20 h, come back — so this end-to-end pass over
/// the generic paths *is* the boss.
const ZAKEN: i32 = 29022;

fn zaken_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    let mut t = crate::data::npc_data::default_template(ZAKEN);
    t.type_name = "GrandBoss".into();
    t.level = 60;
    t.base_hp_max = 800_000.0;
    world.data.npc_data.insert_for_test(t);
    world.grand_bosses.insert(
        ZAKEN,
        model::grand_boss::GrandBoss {
            boss_id: ZAKEN,
            loc_x: 52207,
            loc_y: 217230,
            loc_z: -3341,
            heading: 0,
            respawn_time: 0,
            current_hp: 0.0,
            current_mp: 0.0,
            status: ALIVE,
        },
    );
    (world, db, l)
}

/// Zaken's full lifecycle on the generic machinery: boot spawns him on his
/// deck, the kill arms his 168 h ± 48 h window, the timer brings him back.
///
/// The bound tracked `GrandBossConfig::default()`, which had drifted to
/// 60 h ± 20 h — neither Java's default nor the shipped `GrandBoss.ini`, both
/// of which say 168/48. It is stated as the shipped numbers here rather than
/// "whatever the default holds", so the two can only agree on purpose;
/// `default_config_matches_the_shipped_ini` guards the other direction.
#[test]
fn zaken_lives_and_dies_on_the_generic_lifecycle() {
    let (mut world, _db, _l) = zaken_world();

    crate::game_loop::grand_boss::resolve_at_boot(&mut world);
    let mut at = None;
    world
        .objects
        .for_each_mut::<(&model::npc::Npc, &Position)>(|(n, p)| {
            if n.npc_id == ZAKEN {
                at = Some((p.x, p.y, p.z));
            }
        });
    assert_eq!(at, Some((52207, 217230, -3341)), "spawned on his deck");

    crate::game_loop::grand_boss::on_grand_boss_killed(&mut world, ZAKEN);
    let b = world.grand_bosses.get(&ZAKEN).unwrap();
    assert_eq!(b.status, DEAD);
    let hours = (b.respawn_time - commons::util::now_millis()) as f64 / 3_600_000.0;
    assert!(
        (120.0..=216.5).contains(&hours),
        "respawn within 168 h ± 48 h ({hours:.1} h)"
    );

    crate::game_loop::grand_boss::handle_grand_boss_respawn(&mut world, ZAKEN);
    assert_eq!(world.grand_bosses.get(&ZAKEN).unwrap().status, ALIVE);
}

/// The simple bosses roar: `BS01_A` broadcast on spawn, `BS02_D` on death —
/// the two `PlaySound`s that are the only packets in Zaken's Java script.
#[test]
fn a_simple_boss_roars_on_spawn_and_on_death() {
    let (mut world, _db, _l) = zaken_world();
    let mut rx = ingame_player(&mut world, 9, 5001, 52207, 217230, -3341);

    crate::game_loop::grand_boss::resolve_at_boot(&mut world);
    assert!(
        sound_names(&drain(&mut rx)).contains(&"BS01_A".to_string()),
        "the spawn roar reaches a player on the deck"
    );

    crate::game_loop::grand_boss::on_grand_boss_killed(&mut world, ZAKEN);
    assert!(
        sound_names(&drain(&mut rx)).contains(&"BS02_D".to_string()),
        "the death roar reaches a player on the deck"
    );
}

/// The cinematic bosses do NOT get the stock roar — Antharas spawns DORMANT
/// and silent; his sounds belong to his own cinematic and death tail.
#[test]
fn a_cinematic_boss_spawns_silently() {
    let (mut world, _db, _l) = combat_test_world();
    const ANTHARAS: i32 = 29068;
    let mut t = crate::data::npc_data::default_template(ANTHARAS);
    t.type_name = "GrandBoss".into();
    world.data.npc_data.insert_for_test(t);
    world.grand_bosses.insert(
        ANTHARAS,
        model::grand_boss::GrandBoss {
            boss_id: ANTHARAS,
            loc_x: 185708,
            loc_y: 114298,
            loc_z: -8221,
            heading: 0,
            respawn_time: 0,
            current_hp: 0.0,
            current_mp: 0.0,
            status: 0,
        },
    );
    let mut rx = ingame_player(&mut world, 9, 5001, 185708, 114298, -8221);

    crate::game_loop::grand_boss::resolve_at_boot(&mut world);
    assert!(
        !sound_names(&drain(&mut rx)).contains(&"BS01_A".to_string()),
        "no stock roar for a dormant cinematic boss"
    );
}

/// The real `GrandBoss.ini` is read — a fixture cannot catch a key rename.
#[test]
fn the_real_grand_boss_config_loads() {
    let cfg = crate::config::GrandBossConfig::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    let q = cfg
        .window_for(QUEEN)
        .expect("Queen Ant has a configured window");
    assert_eq!((q.interval_hours, q.random_hours), (36, 17));
    // Baium ships no `RandomOfBaiumSpawn`, so its spread must default to 0
    // rather than being assumed symmetric with the others.
    assert_eq!(
        cfg.baium.random_hours, 0,
        "Baium has no random spread on this dist"
    );
    assert!(
        cfg.window_for(12077).is_none(),
        "an ordinary npc has no window"
    );
}

/// `GrandBossConfig::default()` claims to hold "the dist's own values, so a
/// test world matches production without reading the file". Nothing checked
/// that, and two entries had drifted off it: Baium's interval read 121 against
/// the shipped 168, and Zaken's whole window read 60/20 against the shipped
/// 168/48 — so every test built on the default config respawned Zaken in a
/// third of production's time.
///
/// Comparing the *whole* struct rather than a per-boss list means a boss added
/// to one side and not the other fails here too.
#[test]
fn default_config_matches_the_shipped_ini() {
    let from_file = crate::config::GrandBossConfig::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    assert_eq!(
        crate::config::GrandBossConfig::default(),
        from_file,
        "GrandBossConfig::default() must equal a real load of dist/game/config/GrandBoss.ini"
    );
}

/// **Every boss the config names must be an id `grandboss_data` actually
/// tracks.** Antharas is the trap: 29019 is a valid NPC template, but the
/// script and the boss table both use **29068** (the "strong" variant), so
/// keying the window off 29019 meant it never resolved — the boss would have
/// died and never come back.
///
/// Checked against the real boss table rather than a fixture, because the
/// whole failure mode is the two lists disagreeing.
#[test]
fn every_configured_boss_id_is_one_the_boss_table_tracks() {
    let cfg = crate::config::GrandBossConfig::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    // The ids `grandboss_data` ships on this dist.
    const TRACKED: [i32; 8] = [25512, 29001, 29006, 29014, 29020, 29022, 29028, 29068];
    for id in TRACKED {
        // Not every tracked id needs a window (Gigantic Chaos Golem is a
        // second form, not a boss with its own timer) — but every id the
        // config *does* name must be tracked.
        let _ = cfg.window_for(id);
    }
    for id in [29001, 29006, 29014, 29020, 29022, 29028, 29068] {
        assert!(
            cfg.window_for(id).is_some(),
            "boss {id} is tracked but has no configured window"
        );
        assert!(
            TRACKED.contains(&id),
            "boss {id} has a window but is not in grandboss_data"
        );
    }
    // The lookalike that must *not* resolve.
    assert!(
        cfg.window_for(29019).is_none(),
        "29019 is an Antharas template but not the tracked boss id"
    );
}
