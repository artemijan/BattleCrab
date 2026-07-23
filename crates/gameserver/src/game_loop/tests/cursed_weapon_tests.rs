//! Cursed weapons — the autonomous gameplay loop (G28): a slain monster drops
//! one, a player who picks it up is cursed, and it expires on its deadline.
//! The activation engine itself is covered by `admin_tests`
//! (`admin_cursed_weapons_info_add_remove`).

use super::*;

use crate::model::Player;

const ZARICHE: i32 = 8190; // Demonic Sword Zariche
const MONSTER_OID: i32 = 0x0400_0000;
const KILLER_OID: i32 = 6001;
const PICKER_OID: i32 = 6002;

/// Boot-equivalent load of `CursedWeapons.xml` into the runtime list, mirroring
/// `net.rs` (and the admin test).
fn load_cursed_weapons(world: &mut World) {
    const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    world.data.root = ROOT.to_string();
    world.data.cursed_weapons = crate::data::CursedWeaponData::load_from(ROOT);
    world.cursed_weapons = world
        .data
        .cursed_weapons
        .weapons
        .iter()
        .cloned()
        .map(|mut cw| {
            cw.skill_max_level = (1..=100)
                .take_while(|l| world.data.skill_data.get(cw.skill_id, *l).is_some())
                .last()
                .unwrap_or(1);
            cw
        })
        .collect();
}

fn cw_idx(world: &World, item_id: i32) -> usize {
    world
        .cursed_weapons
        .iter()
        .position(|c| c.item_id == item_id)
        .unwrap()
}

fn ground_item_count(world: &World) -> usize {
    world.ground_item_regions.values().map(|v| v.len()).sum()
}

/// A monster killed by an un-cursed player, with the drop roll forced to hit,
/// puts a cursed weapon on the ground: a `RedSky`/`Earthquake`/drop-announce to
/// everyone, the ground item spawned at the kill site, and the weapon's state
/// flipped to "dropped" with its life task armed.
#[test]
fn monster_kill_drops_cursed_weapon() {
    let (mut world, _db, mut db_rx, _l) = test_world();
    load_cursed_weapons(&mut world);
    let mut killer_rx = ingame_player_access(&mut world, 1, KILLER_OID, 0);
    drain(&mut killer_rx);
    add_test_npc(&mut world, MONSTER_OID, 900001, "Monster", 20, 500, 600, 0);

    // Force the FIRST candidate's roll to land (0 < dropRate); the loop breaks
    // before the second weapon rolls.
    world.forced_rolls.push_back(0);
    crate::game_loop::cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);

    let idx = cw_idx(&world, ZARICHE);
    let cw = &world.cursed_weapons[idx];
    assert!(cw.is_dropped && !cw.is_activated, "flipped to dropped");
    assert!(cw.dropped_item_oid != 0, "records the ground item oid");
    assert!(cw.end_time > 0, "life task deadline armed");
    assert_eq!(ground_item_count(&world), 1, "the weapon is on the ground");

    let pkts = drain(&mut killer_rx);
    assert!(
        sm_ids_of(&pkts).contains(&server_packets::sm_ids::S2_WAS_DROPPED_IN_THE_S1_REGION),
        "drop announced"
    );
    assert!(
        pkts.iter().any(|p| p[0] == 0xFE
            && p.len() >= 3
            && i16::from_le_bytes([p[1], p[2]]) == server_packets::opcodes::EX_RED_SKY),
        "red sky"
    );
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::EARTHQUAKE),
        "earthquake"
    );
    let _ = &mut db_rx;
}

/// The drop roll missing leaves the weapon out of the world (both candidates
/// roll and both miss).
#[test]
fn missed_roll_drops_nothing() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_cursed_weapons(&mut world);
    let _rx = ingame_player_access(&mut world, 1, KILLER_OID, 0);
    add_test_npc(&mut world, MONSTER_OID, 900001, "Monster", 20, 500, 600, 0);

    // dropRate is 50; a roll of 50 is NOT < 50. Both weapons miss.
    world.forced_rolls.push_back(50);
    world.forced_rolls.push_back(50);
    crate::game_loop::cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);

    assert_eq!(ground_item_count(&world), 0, "nothing dropped");
    assert!(
        world.cursed_weapons.iter().all(|c| !c.is_active()),
        "both weapons still out of world"
    );
}

/// A monster killed by a player who ALREADY wields a cursed weapon can't drop
/// another (Java's `checkDrop` skips active weapons, and a cursed wielder is
/// excluded via the item flag).
#[test]
fn cursed_killer_gets_no_drop() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_cursed_weapons(&mut world);
    let _rx = ingame_player_access(&mut world, 1, KILLER_OID, 0);
    world
        .objects
        .get_component_mut::<Player>(&KILLER_OID)
        .unwrap()
        .cursed_weapon_equipped_id = 8689;
    add_test_npc(&mut world, MONSTER_OID, 900001, "Monster", 20, 500, 600, 0);

    world.forced_rolls.push_back(0); // would hit if the killer were eligible
    crate::game_loop::cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);

    assert_eq!(
        ground_item_count(&world),
        0,
        "a cursed wielder triggers no new drop"
    );
    // The forced roll was never consumed (we bailed before rolling).
    assert_eq!(world.forced_rolls.front(), Some(&0), "roll not reached");
}

/// A raid boss kill never drops a cursed weapon (Java excludes `GrandBoss`).
#[test]
fn raid_kill_drops_nothing() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_cursed_weapons(&mut world);
    let _rx = ingame_player_access(&mut world, 1, KILLER_OID, 0);
    add_test_npc(&mut world, MONSTER_OID, 900002, "RaidBoss", 60, 500, 600, 0);

    world.forced_rolls.push_back(0);
    crate::game_loop::cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);

    assert_eq!(ground_item_count(&world), 0, "raid boss is excluded");
}

/// Picking up a dropped cursed weapon curses the picker — driven through the
/// real `pickup_ground_item` entry so the `is_dropped_cursed` interception is
/// exercised, not just the helper.
#[test]
fn pickup_curses_the_finder() {
    let (mut world, _db, mut db_rx, _l) = test_world();
    load_cursed_weapons(&mut world);
    world.id_pool = 0x3000_0000..0x3000_0100; // activate allocates the wielded item's oid
    let mut killer_rx = ingame_player_access(&mut world, 1, KILLER_OID, 0);
    let mut picker_rx = ingame_player_access(&mut world, 2, PICKER_OID, 0);
    add_test_npc(&mut world, MONSTER_OID, 900001, "Monster", 20, 500, 600, 0);

    world.forced_rolls.push_back(0);
    crate::game_loop::cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);
    drain(&mut killer_rx);
    drain(&mut picker_rx);
    while db_rx.try_recv().is_ok() {}

    let idx = cw_idx(&world, ZARICHE);
    let ground_oid = world.cursed_weapons[idx].dropped_item_oid;

    // Real pickup path (Player.doPickupItem → cursed-weapon route).
    crate::game_loop::ground_items::pickup_ground_item(&mut world, 2, PICKER_OID, ground_oid);

    let p = world.objects.get_component::<Player>(&PICKER_OID).unwrap();
    assert_eq!(p.cursed_weapon_equipped_id, ZARICHE, "picker is now cursed");
    assert_eq!(
        p.reputation, -9_999_999,
        "karma slammed to the cursed value"
    );
    let cw = &world.cursed_weapons[idx];
    assert!(
        cw.is_activated && cw.player_id == PICKER_OID,
        "activated on the picker"
    );
    assert!(!cw.is_dropped, "no longer on the ground");
    assert_eq!(ground_item_count(&world), 0, "ground item consumed");

    let picked = drain(&mut picker_rx);
    assert!(
        sm_ids_of(&picked).contains(&server_packets::sm_ids::YOU_HAVE_EQUIPPED_YOUR_S1),
        "equip message to the picker"
    );
    // Persisted to the DB (activate's saveData).
    let mut saw_store = false;
    while let Ok(c) = db_rx.try_recv() {
        if matches!(
            c,
            db::DbCommand::StoreCursedWeapon {
                item_id: ZARICHE,
                char_id: PICKER_OID,
                ..
            }
        ) {
            saw_store = true;
        }
    }
    assert!(saw_store, "cursed weapon persisted on pickup");
}

/// A player already wielding a cursed weapon who picks up another consumes the
/// new one (it vanishes back to "not in world"); they do not end up holding two.
#[test]
fn already_cursed_picker_consumes_the_weapon() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_cursed_weapons(&mut world);
    let _krx = ingame_player_access(&mut world, 1, KILLER_OID, 0);
    let _prx = ingame_player_access(&mut world, 2, PICKER_OID, 0);
    world
        .objects
        .get_component_mut::<Player>(&PICKER_OID)
        .unwrap()
        .cursed_weapon_equipped_id = 8689;
    add_test_npc(&mut world, MONSTER_OID, 900001, "Monster", 20, 500, 600, 0);

    world.forced_rolls.push_back(0);
    crate::game_loop::cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);
    let idx = cw_idx(&world, ZARICHE);
    let ground_oid = world.cursed_weapons[idx].dropped_item_oid;

    crate::game_loop::ground_items::pickup_ground_item(&mut world, 2, PICKER_OID, ground_oid);

    // Zariche vanished; the picker still wields only their original weapon.
    assert!(
        !world.cursed_weapons[idx].is_active(),
        "the picked weapon is consumed"
    );
    assert_eq!(ground_item_count(&world), 0, "gone from the ground");
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&PICKER_OID)
            .unwrap()
            .cursed_weapon_equipped_id,
        8689,
        "still holds the original weapon, not Zariche"
    );
}

/// When the life task fires past the deadline, an un-grabbed drop vanishes: the
/// ground item is removed, the DB row cleared, the disappearance announced, and
/// the weapon reset.
#[test]
fn expiry_removes_ungrabbed_drop() {
    let (mut world, _db, mut db_rx, _l) = test_world();
    load_cursed_weapons(&mut world);
    let mut rx = ingame_player_access(&mut world, 1, KILLER_OID, 0);
    add_test_npc(&mut world, MONSTER_OID, 900001, "Monster", 20, 500, 600, 0);

    world.forced_rolls.push_back(0);
    crate::game_loop::cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);
    drain(&mut rx);
    while db_rx.try_recv().is_ok() {}

    let idx = cw_idx(&world, ZARICHE);
    world.cursed_weapons[idx].end_time = 1; // deadline in the distant past
    crate::game_loop::cursed_weapon::handle_expiry(&mut world, ZARICHE);

    assert!(
        !world.cursed_weapons[idx].is_active(),
        "weapon reset to not-in-world"
    );
    assert_eq!(ground_item_count(&world), 0, "ground item removed");
    let disappeared = drain(&mut rx);
    assert!(
        sm_ids_of(&disappeared).contains(&server_packets::sm_ids::S1_HAS_DISAPPEARED),
        "disappearance announced"
    );
    let mut saw_remove = false;
    while let Ok(c) = db_rx.try_recv() {
        if matches!(c, db::DbCommand::RemoveCursedWeapon { item_id: ZARICHE }) {
            saw_remove = true;
        }
    }
    assert!(saw_remove, "db row dropped on expiry");
}

/// A not-yet-due expiry timer is a no-op (a re-armed / superseded task).
#[test]
fn premature_expiry_is_ignored() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_cursed_weapons(&mut world);
    let _rx = ingame_player_access(&mut world, 1, KILLER_OID, 0);
    add_test_npc(&mut world, MONSTER_OID, 900001, "Monster", 20, 500, 600, 0);

    world.forced_rolls.push_back(0);
    crate::game_loop::cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);
    let idx = cw_idx(&world, ZARICHE);
    world.cursed_weapons[idx].end_time = now_millis_test() + 10 * 60_000; // 10 min out

    crate::game_loop::cursed_weapon::handle_expiry(&mut world, ZARICHE);

    assert!(
        world.cursed_weapons[idx].is_dropped,
        "still dropped — the timer wasn't due"
    );
    assert_eq!(ground_item_count(&world), 1, "ground item still present");
}

fn now_millis_test() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// The scheduled `CursedWeaponExpiry` task actually reaches `handle_expiry`
/// through the game-loop dispatch (not a direct call): a due timer, armed via
/// `arm_expiry`, fires when the loop advances and removes the drop.
#[test]
fn scheduled_expiry_fires_through_loop() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_cursed_weapons(&mut world);
    let _rx = ingame_player_access(&mut world, 1, KILLER_OID, 0);
    add_test_npc(&mut world, MONSTER_OID, 900001, "Monster", 20, 500, 600, 0);

    world.forced_rolls.push_back(0);
    crate::game_loop::cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);
    let idx = cw_idx(&world, ZARICHE);
    // Make the deadline due now and re-arm a zero-delay task so a single tick
    // fires it through `ScheduledTask::CursedWeaponExpiry`.
    world.cursed_weapons[idx].end_time = 1;
    crate::game_loop::cursed_weapon::arm_expiry(&mut world, idx);

    advance_ticks(&mut world, 1);

    assert!(
        !world.cursed_weapons[idx].is_active(),
        "the scheduled task expired the weapon"
    );
    assert_eq!(
        ground_item_count(&world),
        0,
        "ground item removed by the loop"
    );
}

/// The death path actually calls the cursed-weapon check: a guaranteed-rate
/// weapon drops when a monster dies through `npc_do_die` (the real wire, not a
/// direct `on_monster_killed` call).
#[test]
fn death_path_triggers_drop() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_cursed_weapons(&mut world);
    let _rx = ingame_player_access(&mut world, 1, KILLER_OID, 0);
    add_test_npc(&mut world, MONSTER_OID, 900001, "Monster", 20, 500, 600, 0);

    // Guaranteed drop rate so no forced roll is needed (reward rolls in
    // npc_do_die would otherwise consume the queue first).
    let idx = cw_idx(&world, ZARICHE);
    world.cursed_weapons[idx].drop_rate = 100_000;

    crate::game_loop::death::npc_do_die(&mut world, MONSTER_OID, KILLER_OID);

    assert!(
        world.cursed_weapons[idx].is_dropped,
        "npc_do_die dropped the cursed weapon"
    );
    assert_eq!(ground_item_count(&world), 1, "on the ground");
}
