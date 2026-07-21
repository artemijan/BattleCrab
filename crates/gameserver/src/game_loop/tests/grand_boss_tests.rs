//! The grand-boss respawn lifecycle.

use super::*;

use crate::game_loop::grand_boss::{ALIVE, DEAD};

/// Queen Ant — the first boss this drives.
const QUEEN: i32 = 29001;
const QUEEN_OID: i32 = NPC_OID + 60;

fn boss_world() -> (World, db::CmdRx, tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    let mut t = crate::data::npc_data::default_template(QUEEN);
    t.type_name = "GrandBoss".into();
    t.level = 40;
    t.base_hp_max = 100_000.0;
    world.data.npc_data.insert_for_test(t);
    world.grand_bosses.insert(
        QUEEN,
        crate::model::grand_boss::GrandBoss {
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
    world.objects.for_each_mut::<&crate::model::npc::Npc>(|n| {
        if n.npc_id == QUEEN {
            found = true;
        }
    });
    found
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
    assert!((19.0..=53.5).contains(&hours), "respawn within the configured window ({hours:.1} h)");
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
    assert!(seen.len() > 1, "the respawn hour varies across kills, got {seen:?}");
}

/// The timer firing brings the boss back, alive and in the world.
#[test]
fn the_respawn_timer_brings_the_boss_back() {
    let (mut world, _db, _l) = boss_world();
    world.grand_bosses.get_mut(&QUEEN).unwrap().status = DEAD;

    crate::game_loop::grand_boss::handle_grand_boss_respawn(&mut world, QUEEN);

    assert_eq!(world.grand_bosses.get(&QUEEN).unwrap().status, ALIVE);
    assert_eq!(world.grand_bosses.get(&QUEEN).unwrap().respawn_time, 0, "the window is cleared");
    assert!(boss_alive_in_world(&mut world), "and it is standing in the world");
}

/// A respawn for a boss that is already alive must not stack a second one —
/// a duplicate timer or an admin spawn would otherwise double the boss.
#[test]
fn respawning_an_already_alive_boss_is_a_no_op() {
    let (mut world, _db, _l) = boss_world();
    crate::game_loop::grand_boss::handle_grand_boss_respawn(&mut world, QUEEN);
    let count = |w: &mut World| {
        let mut n = 0;
        w.objects.for_each_mut::<&crate::model::npc::Npc>(|x| {
            if x.npc_id == QUEEN {
                n += 1;
            }
        });
        n
    };
    let before = count(&mut world);
    crate::game_loop::grand_boss::handle_grand_boss_respawn(&mut world, QUEEN);
    assert_eq!(count(&mut world), before, "no second boss");
}

/// Boot with a boss **alive**: it is placed at its stored location, with its
/// stored HP — a boss wounded before a restart comes back wounded.
#[test]
fn boot_spawns_a_living_boss_with_its_stored_hp() {
    let (mut world, _db, _l) = boss_world();
    world.grand_bosses.get_mut(&QUEEN).unwrap().current_hp = 4_242.0;

    crate::game_loop::grand_boss::resolve_at_boot(&mut world);

    let mut hp = None;
    world.objects.for_each_mut::<(&crate::model::npc::Npc, &Vitals)>(|(n, v)| {
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
    assert!(boss_alive_in_world(&mut world), "spawned rather than left dead forever");
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
    assert!(!boss_alive_in_world(&mut world), "still waiting out its window");
}

/// The real `GrandBoss.ini` is read — a fixture cannot catch a key rename.
#[test]
fn the_real_grand_boss_config_loads() {
    let cfg = crate::config::GrandBossConfig::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let q = cfg.window_for(QUEEN).expect("Queen Ant has a configured window");
    assert_eq!((q.interval_hours, q.random_hours), (36, 17));
    // Baium ships no `RandomOfBaiumSpawn`, so its spread must default to 0
    // rather than being assumed symmetric with the others.
    assert_eq!(cfg.baium.random_hours, 0, "Baium has no random spread on this dist");
    assert!(cfg.window_for(12077).is_none(), "an ordinary npc has no window");
}
