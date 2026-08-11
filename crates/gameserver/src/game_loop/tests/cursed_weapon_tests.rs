//! Cursed weapons — the autonomous gameplay loop (G28): a slain monster drops
//! one, a player who picks it up is cursed, and it expires on its deadline.
//! The activation engine itself is covered by `admin_tests`
//! (`admin_cursed_weapons_info_add_remove`).

use super::*;
use crate::game_loop::{admin, cursed_weapon, death, ground_items, passive_skills, pvp};

use crate::model::Player;

const ZARICHE: i32 = 8190; // Demonic Sword Zariche
const MONSTER_OID: i32 = 0x0400_0000;
const KILLER_OID: i32 = 6001;
const PICKER_OID: i32 = 6002;

/// Boot-equivalent load of `CursedWeapons.xml` into the runtime list, mirroring
/// `net.rs` (and the admin test).
fn load_cursed_weapons(world: &mut World) {
    const ROOT: &str = crate::data::DIST_GAME;
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

/// `(object_id, item_id)` reported for one paperdoll slot by the most recent
/// `ExUserInfoEquipSlot` (Ex 0x156) in `packets` — the packet that actually
/// paints the client's own paperdoll. `None` when no such packet was sent.
fn equip_slot(packets: &[Vec<u8>], want: crate::enums::InventorySlot) -> Option<(i32, i32)> {
    let pkt = packets
        .iter()
        .rev()
        .find(|p| p.len() > 2 && p[0] == 0xFE && u16::from_le_bytes([p[1], p[2]]) == 0x156)?;
    // 1 (Ex) + 2 (sub) + 4 (object id) + 2 (slot count) + 5 (mask) = 14.
    let mut offset = 14usize;
    let mut found = None;
    for slot in crate::enums::InventorySlot::VALUES {
        let block_len = u16::from_le_bytes([pkt[offset], pkt[offset + 1]]) as usize;
        if slot == want {
            found = Some((
                i32::from_le_bytes(pkt[offset + 2..offset + 6].try_into().unwrap()),
                i32::from_le_bytes(pkt[offset + 6..offset + 10].try_into().unwrap()),
            ));
        }
        offset += block_len; // the written length covers its own 2 bytes
    }
    found
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
    cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);

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
    cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);

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
    cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);

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
    cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);

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
    cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);
    drain(&mut killer_rx);
    drain(&mut picker_rx);
    while db_rx.try_recv().is_ok() {}

    let idx = cw_idx(&world, ZARICHE);
    let ground_oid = world.cursed_weapons[idx].dropped_item_oid;

    // Real pickup path (Player.doPickupItem → cursed-weapon route).
    ground_items::pickup_ground_item(&mut world, 2, PICKER_OID, ground_oid);

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
    cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);
    let idx = cw_idx(&world, ZARICHE);
    let ground_oid = world.cursed_weapons[idx].dropped_item_oid;

    ground_items::pickup_ground_item(&mut world, 2, PICKER_OID, ground_oid);

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
    cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);
    drain(&mut rx);
    while db_rx.try_recv().is_ok() {}

    let idx = cw_idx(&world, ZARICHE);
    world.cursed_weapons[idx].end_time = 1; // deadline in the distant past
    cursed_weapon::handle_expiry(&mut world, ZARICHE);

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
    cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);
    let idx = cw_idx(&world, ZARICHE);
    world.cursed_weapons[idx].end_time = now_millis_test() + 10 * 60_000; // 10 min out

    cursed_weapon::handle_expiry(&mut world, ZARICHE);

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
    cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);
    let idx = cw_idx(&world, ZARICHE);
    // Make the deadline due now and re-arm a zero-delay task so a single tick
    // fires it through `ScheduledTask::CursedWeaponExpiry`.
    world.cursed_weapons[idx].end_time = 1;
    cursed_weapon::arm_expiry(&mut world, idx);

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

    death::npc_do_die(&mut world, MONSTER_OID, KILLER_OID);

    assert!(
        world.cursed_weapons[idx].is_dropped,
        "npc_do_die dropped the cursed weapon"
    );
    assert_eq!(ground_item_count(&world), 1, "on the ground");
}

// ---------------------------------------------------------------------------
// The client's cursed-weapon window (ex 0x2A / 0x2B — row 10)
// ---------------------------------------------------------------------------

/// **The window lists every cursed weapon and locates the live ones.** Java's
/// `ExCursedWeaponList` carries all known ids; `ExCursedWeaponLocation` carries
/// only the *active* ones — and is not sent at all when none are live.
#[test]
fn the_cursed_weapon_window_lists_and_locates() {
    use crate::game_loop::cursed_weapon::{handle_request_list, handle_request_location};

    let (mut world, ..) = test_world();
    load_cursed_weapons(&mut world);
    let mut rx = ingame_player(&mut world, 1, 6100, 1000, 2000, -30);
    drain(&mut rx);

    // The list is the full catalogue, live or not.
    handle_request_list(&world, 1);
    let list = find_ex_opcode(&mut rx, server_packets::opcodes::EX_CURSED_WEAPON_LIST)
        .expect("the list packet");
    assert_eq!(
        i32::from_le_bytes([list[3], list[4], list[5], list[6]]),
        world.cursed_weapons.len() as i32,
        "every known cursed weapon is listed"
    );

    // Nothing is live yet → Java sends no location packet at all.
    handle_request_location(&world, 1);
    assert!(
        !drain(&mut rx).iter().any(|p| p.len() >= 3
            && p[0] == 0xFE
            && i16::from_le_bytes([p[1], p[2]])
                == server_packets::opcodes::EX_CURSED_WEAPON_LOCATION),
        "no location packet while none are active"
    );

    // Put Zariche in the player's hands: now it has a position to report. The
    // other weapon stays inactive and is left out.
    let idx = cw_idx(&world, ZARICHE);
    world.cursed_weapons[idx].is_activated = true;
    world.cursed_weapons[idx].player_id = 6100;
    handle_request_location(&world, 1);
    let loc = find_ex_opcode(&mut rx, server_packets::opcodes::EX_CURSED_WEAPON_LOCATION)
        .expect("the location packet");
    assert_eq!(
        i32::from_le_bytes([loc[3], loc[4], loc[5], loc[6]]),
        1,
        "one live weapon"
    );
    assert_eq!(
        i32::from_le_bytes([loc[7], loc[8], loc[9], loc[10]]),
        ZARICHE,
        "…and it is Zariche"
    );
    assert_eq!(
        i32::from_le_bytes([loc[15], loc[16], loc[17], loc[18]]),
        1000,
        "reported at the wielder's position"
    );
}

/// **A GM-granted cursed weapon still expires.** Java's `reActivate()` arms the
/// `RemoveTask` alongside setting the end time; without it the duration
/// argument would be decorative and the weapon would be permanent.
#[test]
fn a_gm_granted_cursed_weapon_arms_its_expiry() {
    let (mut world, _db, mut db_rx, _l) = test_world();
    load_cursed_weapons(&mut world);
    world.data.admin = crate::data::AdminData::load_from(crate::data::DIST_GAME);
    world.id_pool = 0x3000_0000..0x3000_0100;
    let mut gm_rx = ingame_player_access(&mut world, 1, KILLER_OID, 100);
    drain(&mut gm_rx);
    while db_rx.try_recv().is_ok() {}

    // `//cw_add <itemid>` on the GM themselves (no target → falls back to the
    // caster, as Java does).
    let id_arg = ZARICHE.to_string();
    admin::cursed_weapons::admin_cw_add(&mut world, 1, KILLER_OID, &[&id_arg]);
    let idx = cw_idx(&world, ZARICHE);
    assert!(
        world.cursed_weapons[idx].is_activated,
        "the GM now wields it"
    );

    // The `RemoveTask` is armed. `handle_expiry` guards on the **wall clock**,
    // not the tick, so the check that bites is "a task exists for this weapon";
    // firing it early is the `premature_expiry_is_ignored` case above.
    assert!(
        world
            .scheduler
            .pending_tasks_for_test()
            .iter()
            .any(|t| matches!(t, crate::scheduler::ScheduledTask::CursedWeaponExpiry { item_id } if *item_id == ZARICHE)),
        "reActivate arms the removal task — without it the duration is decorative"
    );

    // Wind the weapon's own end time back and the armed task ends it.
    world.cursed_weapons[idx].end_time = now_millis_test() - 1;
    cursed_weapon::handle_expiry(&mut world, ZARICHE);
    assert!(
        !world.cursed_weapons[idx].is_activated,
        "the duration actually runs out"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&KILLER_OID)
            .unwrap()
            .cursed_weapon_equipped_id,
        0,
        "and the wielder is freed"
    );
}

// ---------------------------------------------------------------------------
// Login restore + the equip locks it depends on
// ---------------------------------------------------------------------------

const DIST: &str = crate::data::DIST_GAME;
const AKAMANAH: i32 = 8689; // Blood Sword Akamanah
/// Squire's Sword — an ordinary one-hand weapon to try swapping to.
const SQUIRES_SWORD: i32 = 7816;

/// Load the data the login restore actually reads: skills (the cursed skill and
/// its max level), transforms (301/302) and items (body parts for the equip
/// gates).
fn load_curse_data(world: &mut World) {
    load_cursed_weapons(world);
    world.data.skill_data = dist::skills_owned();
    world.data.transforms = crate::data::TransformData::load_from(DIST);
    world.data.item_data = dist::items_owned();
    for cw in &mut world.cursed_weapons {
        cw.skill_max_level = (1..=100)
            .take_while(|l| world.data.skill_data.get(cw.skill_id, *l).is_some())
            .last()
            .unwrap_or(1);
    }
}

/// The object id of `item_id` in `owner`'s bag, but only when it is worn.
fn equipped_oid(world: &World, owner: i32, item_id: i32) -> Option<i32> {
    let inv = world.objects.get_component::<Inventory>(&owner)?;
    let oid = inv.items().iter().find(|i| i.item_id == item_id)?.object_id;
    inv.paperdoll_slot_of(oid).map(|_| oid)
}

/// Put `world` in the state a server restart leaves behind: the `cursed_weapons`
/// row is loaded and flagged activated for `owner`, who is online but not yet
/// restored.
fn restore_activated_on(world: &mut World, item_id: i32, owner: i32) -> usize {
    let idx = cw_idx(world, item_id);
    let cw = &mut world.cursed_weapons[idx];
    cw.is_activated = true;
    cw.player_id = owner;
    cw.nb_kills = 0;
    cw.end_time = now_millis_test() + 300 * 60_000; // a fresh 300-minute life
    idx
}

/// The bug this slice fixes: a character who logs back in wielding a cursed
/// weapon must come back **cursed** — Java's `CursedWeaponsManager.checkPlayer`
/// (the flag + skill) plus `CursedWeapon.cursedOnLogin` (the transform + the
/// announce). Before the fix the relog silently lifted the curse: the sword
/// stayed in hand as an ordinary weapon and the character was not transformed.
#[test]
fn relog_restores_transform_and_curse() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    let mut rx = ingame_player_access(&mut world, 1, PICKER_OID, 0);
    let idx = restore_activated_on(&mut world, ZARICHE, PICKER_OID);
    let skill_id = world.cursed_weapons[idx].skill_id;
    drain(&mut rx);

    cursed_weapon::on_enter_world(&mut world, 1, PICKER_OID);

    let p = world.objects.get_component::<Player>(&PICKER_OID).unwrap();
    assert_eq!(
        p.cursed_weapon_equipped_id, ZARICHE,
        "the curse flag every gate reads is back"
    );
    assert_eq!(p.transform_id, 301, "Zariche transforms into 301");
    assert_eq!(
        p.transform_display_id, 301,
        "and the client is told which model to draw"
    );
    assert!(
        world
            .objects
            .get_component::<SkillBook>(&PICKER_OID)
            .unwrap()
            .0
            .contains_key(&skill_id),
        "giveSkill re-grants the weapon's skill"
    );

    let pkts = drain(&mut rx);
    let sms = sm_ids_of(&pkts);
    assert!(
        sms.contains(&server_packets::sm_ids::S2_S_OWNER_HAS_LOGGED_INTO_THE_S1_REGION),
        "the login is announced"
    );
    assert!(
        sms.contains(&server_packets::sm_ids::S1_HAS_S2_MINUTE_S_OF_USAGE_TIME_REMAINING),
        "and the wielder is told how long is left"
    );
}

/// Akamanah takes the other transform — the two ids are easy to swap.
#[test]
fn relog_restores_akamanah_transform() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    let _rx = ingame_player_access(&mut world, 1, PICKER_OID, 0);
    restore_activated_on(&mut world, AKAMANAH, PICKER_OID);

    cursed_weapon::on_enter_world(&mut world, 1, PICKER_OID);

    assert_eq!(
        world
            .objects
            .get_component::<Player>(&PICKER_OID)
            .unwrap()
            .transform_id,
        302,
        "Akamanah transforms into 302"
    );
}

/// Someone else's cursed weapon must not curse *this* character on login.
#[test]
fn relog_of_a_bystander_restores_nothing() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    let _rx = ingame_player_access(&mut world, 1, PICKER_OID, 0);
    restore_activated_on(&mut world, ZARICHE, KILLER_OID); // owned by someone else

    cursed_weapon::on_enter_world(&mut world, 1, PICKER_OID);

    let p = world.objects.get_component::<Player>(&PICKER_OID).unwrap();
    assert_eq!(p.cursed_weapon_equipped_id, 0, "not this character's curse");
    assert_eq!(p.transform_id, 0, "and no transform");
}

/// `EnterWorld`'s "Remove demonic weapon if character is not cursed weapon
/// equipped" sweep: a Zariche left in the bag of someone the manager no longer
/// considers cursed (its life ran out while they were offline) is destroyed.
#[test]
fn relog_destroys_a_stray_cursed_weapon() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    world.id_pool = 0x3000_0000..0x3000_0100;
    let _rx = ingame_player_access(&mut world, 1, PICKER_OID, 0);
    items::add_inventory_item(&mut world, PICKER_OID, ZARICHE, 1)
        .expect("the leftover sword is in the bag");

    cursed_weapon::on_enter_world(&mut world, 1, PICKER_OID);

    assert!(
        world
            .objects
            .get_component::<Inventory>(&PICKER_OID)
            .unwrap()
            .items()
            .iter()
            .all(|i| i.item_id != ZARICHE),
        "the orphaned cursed weapon is destroyed"
    );
}

/// The curse is not something you can take off: `RequestUnEquipItem` on the
/// two-hand slot is refused outright while cursed (Java tests the requested
/// slot, not the item).
#[test]
fn cursed_wielder_cannot_unequip_the_weapon() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    world.id_pool = 0x3000_0000..0x3000_0100;
    let _rx = ingame_player_access(&mut world, 1, PICKER_OID, 0);
    let idx = cw_idx(&world, ZARICHE);
    admin::cursed_weapons::activate(&mut world, idx, PICKER_OID);
    assert!(
        equipped_oid(&world, PICKER_OID, ZARICHE).is_some(),
        "activate equipped the weapon"
    );

    let mut body = Vec::new();
    body.extend_from_slice(&crate::data::item_data::SLOT_LR_HAND.to_le_bytes());
    items::handle_request_un_equip_item(&mut world, 1, &body);

    assert!(
        equipped_oid(&world, PICKER_OID, ZARICHE).is_some(),
        "the cursed weapon stays in hand"
    );
}

/// …and you cannot swap it out by equipping something else either: `UseItem` on
/// any hand-slot item is refused while cursed.
#[test]
fn cursed_wielder_cannot_equip_another_weapon() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    world.id_pool = 0x3000_0000..0x3000_0100;
    let _rx = ingame_player_access(&mut world, 1, PICKER_OID, 0);
    let idx = cw_idx(&world, ZARICHE);
    admin::cursed_weapons::activate(&mut world, idx, PICKER_OID);
    let sword_oid = items::add_inventory_item(&mut world, PICKER_OID, SQUIRES_SWORD, 1)
        .expect("spare sword added")[0];

    let mut body = Vec::new();
    body.extend_from_slice(&sword_oid.to_le_bytes());
    body.extend_from_slice(&0i32.to_le_bytes()); // ctrlPressed
    items::handle_use_item(&mut world, 1, &body);

    assert!(
        world
            .objects
            .get_component::<Inventory>(&PICKER_OID)
            .unwrap()
            .paperdoll_slot_of(sword_oid)
            .is_none(),
        "the swap is refused — the spare sword never reaches the paperdoll"
    );
    assert!(
        equipped_oid(&world, PICKER_OID, ZARICHE).is_some(),
        "and the cursed weapon is still equipped"
    );
}

/// The same gate must not lock an *un*-cursed character out of their own
/// weapons — the zero case for the check above.
#[test]
fn uncursed_player_can_still_equip_a_weapon() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    world.id_pool = 0x3000_0000..0x3000_0100;
    let _rx = ingame_player_access(&mut world, 1, PICKER_OID, 0);
    let sword_oid = items::add_inventory_item(&mut world, PICKER_OID, SQUIRES_SWORD, 1)
        .expect("sword added")[0];

    let mut body = Vec::new();
    body.extend_from_slice(&sword_oid.to_le_bytes());
    body.extend_from_slice(&0i32.to_le_bytes());
    items::handle_use_item(&mut world, 1, &body);

    assert!(
        world
            .objects
            .get_component::<Inventory>(&PICKER_OID)
            .unwrap()
            .paperdoll_slot_of(sword_oid)
            .is_some(),
        "an ordinary player equips normally"
    );
}

/// A curse that runs out while its owner is logged off must still free them.
/// Java's offline `endOfLife` branch does the restore in the database; the
/// boot-armed expiry task makes this genuinely reachable, and without it the
/// character comes back holding the sword with reputation pinned at -9999999.
#[test]
fn offline_expiry_restores_the_owner_in_the_db() {
    let (mut world, _db, mut db_rx, _l) = test_world();
    load_curse_data(&mut world);
    // The owner is NOT online — no `ingame_player_access` for them.
    const OFFLINE_OWNER: i32 = 7003;
    let idx = restore_activated_on(&mut world, ZARICHE, OFFLINE_OWNER);
    let skill_id = world.cursed_weapons[idx].skill_id;
    world.cursed_weapons[idx].player_reputation = 4242;
    world.cursed_weapons[idx].player_pk_kills = 7;
    world.cursed_weapons[idx].end_time = now_millis_test() - 1;
    while db_rx.try_recv().is_ok() {}

    cursed_weapon::handle_expiry(&mut world, ZARICHE);

    let restore = drain_db(&mut db_rx).into_iter().find_map(|c| match c {
        db::DbCommand::RestoreOfflineCursedOwner {
            char_id,
            item_id,
            reputation,
            pk_kills,
            skill_ids,
        } => Some((char_id, item_id, reputation, pk_kills, skill_ids)),
        _ => None,
    });
    let (char_id, item_id, reputation, pk_kills, skill_ids) =
        restore.expect("the offline wielder is restored in the database");
    assert_eq!(char_id, OFFLINE_OWNER);
    assert_eq!(item_id, ZARICHE, "the weapon item row is deleted");
    assert_eq!(reputation, 4242, "the saved reputation comes back");
    assert_eq!(pk_kills, 7, "and the saved pk-kill count");
    assert!(
        skill_ids.contains(&skill_id),
        "the weapon's own skill is dropped"
    );
    assert!(
        skill_ids.contains(&3630) && skill_ids.contains(&3631),
        "and so are the transform's Void Burst / Void Flow — this port persists \
         the whole SkillBook, unlike Java's non-storing addSkill"
    );
    assert!(
        !world.cursed_weapons[idx].is_activated,
        "the weapon leaves the world"
    );
}

// ---------------------------------------------------------------------------
// Java parity: kills, death-drop, and the gates
// ---------------------------------------------------------------------------

/// `increaseKills`: a cursed wielder killing a player scores the weapon — the
/// tally overwrites the PK counter, the remaining life shrinks by
/// `durationLost` minutes, and the row is persisted.
#[test]
fn cursed_kill_scores_and_burns_time() {
    let (mut world, _db, mut db_rx, _l) = test_world();
    load_curse_data(&mut world);
    let _k = ingame_player_access(&mut world, 1, KILLER_OID, 0);
    let _v = ingame_player_access(&mut world, 2, PICKER_OID, 0);
    let idx = restore_activated_on(&mut world, ZARICHE, KILLER_OID);
    world
        .objects
        .get_component_mut::<Player>(&KILLER_OID)
        .unwrap()
        .cursed_weapon_equipped_id = ZARICHE;
    let before_end = world.cursed_weapons[idx].end_time;
    let duration_lost = world.cursed_weapons[idx].duration_lost as i64;
    while db_rx.try_recv().is_ok() {}

    let handled = cursed_weapon::on_player_kill(&mut world, KILLER_OID, PICKER_OID);

    assert!(handled, "the cursed branch claims the kill");
    assert_eq!(world.cursed_weapons[idx].nb_kills, 1, "kill counted");
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&KILLER_OID)
            .unwrap()
            .pk_kills,
        1,
        "Java shows the cursed tally in the PK counter"
    );
    assert_eq!(
        world.cursed_weapons[idx].end_time,
        before_end - duration_lost * 60_000,
        "each kill burns durationLost minutes off the life"
    );
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, db::DbCommand::StoreCursedWeapon { .. })),
        "increaseKills persists (Java saveData)"
    );
}

/// A cursed kill must **not** run the ordinary PvP/PK reputation path — Java
/// returns out of `onPlayerKill` before it. Without the early exit the wielder
/// would also rack up pvp kills or karma on top of the weapon's tally.
#[test]
fn cursed_kill_skips_normal_pvp_reputation() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    let _k = ingame_player_access(&mut world, 1, KILLER_OID, 0);
    let _v = ingame_player_access(&mut world, 2, PICKER_OID, 0);
    restore_activated_on(&mut world, ZARICHE, KILLER_OID);
    world
        .objects
        .get_component_mut::<Player>(&KILLER_OID)
        .unwrap()
        .cursed_weapon_equipped_id = ZARICHE;

    pvp::on_kill_update_pvp_reputation(&mut world, KILLER_OID, PICKER_OID);

    let p = world.objects.get_component::<Player>(&KILLER_OID).unwrap();
    assert_eq!(p.pvp_kills, 0, "no pvp kill credited");
    assert_eq!(
        p.pk_kills, 1,
        "pk_kills is the weapon's tally, not a PK penalty"
    );
    assert!(
        p.reputation >= 0 || p.reputation == -9_999_999,
        "no karma added on top"
    );
}

/// The stage boundary levels the weapon's skill: with `stageKills` reached, the
/// wielder's cursed skill steps up a level.
#[test]
fn stage_boundary_levels_the_skill() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    let _k = ingame_player_access(&mut world, 1, KILLER_OID, 0);
    let idx = restore_activated_on(&mut world, ZARICHE, KILLER_OID);
    world
        .objects
        .get_component_mut::<Player>(&KILLER_OID)
        .unwrap()
        .cursed_weapon_equipped_id = ZARICHE;
    let (skill_id, stage) = {
        let cw = &world.cursed_weapons[idx];
        (cw.skill_id, cw.stage_kills)
    };
    assert!(stage > 1, "fixture assumes a multi-kill stage");
    // One short of the boundary: still level 1.
    world.cursed_weapons[idx].nb_kills = stage - 2;
    cursed_weapon::increase_kills(&mut world, idx);
    let lvl_before = *world
        .objects
        .get_component::<SkillBook>(&KILLER_OID)
        .unwrap()
        .0
        .get(&skill_id)
        .unwrap_or(&1);

    cursed_weapon::increase_kills(&mut world, idx); // hits the boundary

    let lvl_after = *world
        .objects
        .get_component::<SkillBook>(&KILLER_OID)
        .unwrap()
        .0
        .get(&skill_id)
        .expect("skill granted at the stage boundary");
    assert!(
        lvl_after > lvl_before,
        "crossing stageKills levels the cursed skill ({lvl_before} -> {lvl_after})"
    );
}

/// The wielder dies and the disappear roll misses: the weapon drops at the
/// corpse for the next taker, and the dead player is freed — reputation and
/// pk-kills restored, curse flag cleared, transform reverted.
#[test]
fn wielder_death_drops_the_weapon() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    world.id_pool = 0x3000_0000..0x3000_0100;
    let _v = ingame_player_access(&mut world, 1, PICKER_OID, 0);
    let _k = ingame_player_access(&mut world, 2, KILLER_OID, 0);
    let idx = cw_idx(&world, ZARICHE);
    admin::cursed_weapons::activate(&mut world, idx, PICKER_OID);
    world.cursed_weapons[idx].player_reputation = 1234;
    world.cursed_weapons[idx].player_pk_kills = 5;

    // dropRate roll: disapearChance is 50 and Java tests `<= 50`, so 51 misses.
    world.forced_rolls.push_back(51);
    cursed_weapon::on_wielder_death(&mut world, PICKER_OID, KILLER_OID);

    let p = world.objects.get_component::<Player>(&PICKER_OID).unwrap();
    assert_eq!(p.cursed_weapon_equipped_id, 0, "curse lifted");
    assert_eq!(p.reputation, 1234, "saved reputation restored");
    assert_eq!(p.pk_kills, 5, "saved pk-kills restored");
    assert_eq!(p.transform_id, 0, "back to their own body");
    let cw = &world.cursed_weapons[idx];
    assert!(
        cw.is_dropped && !cw.is_activated,
        "lying on the ground again"
    );
    assert_eq!(ground_item_count(&world), 1, "dropped at the corpse");
}

/// The client paints its *own* character from `ExUserInfoEquipSlot`, never
/// from `UserInfo` (which carries only the right-hand enchant level) and never
/// from `ItemList`. Java gets this for free — `dropItem` → `removeItem`
/// unequips inside `Inventory.setPaperdollItem`, which sends the packet — so
/// dropping the curse on death has to send it here too. Without it the corpse
/// goes on wielding a sword that is already lying on the ground, while the
/// inventory window correctly shows nothing equipped.
#[test]
fn wielder_death_resends_the_paperdoll_snapshot() {
    use crate::enums::InventorySlot;

    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    world.id_pool = 0x3000_0000..0x3000_0100;
    let mut v_rx = ingame_player_access(&mut world, 1, PICKER_OID, 0);
    let _k = ingame_player_access(&mut world, 2, KILLER_OID, 0);
    let idx = cw_idx(&world, ZARICHE);
    admin::cursed_weapons::activate(&mut world, idx, PICKER_OID);

    // Pre-condition: the curse really is on the paperdoll, or the assertion
    // below would pass against an empty slot for the wrong reason.
    let weapon_oid = world
        .objects
        .get_component::<Inventory>(&PICKER_OID)
        .and_then(|inv| inv.items().iter().find(|i| i.item_id == ZARICHE).copied())
        .expect("the curse put Zariche in the bag")
        .object_id;
    assert!(
        world
            .objects
            .get_component::<Inventory>(&PICKER_OID)
            .unwrap()
            .paperdoll_slot_of(weapon_oid)
            .is_some(),
        "activate equips the weapon"
    );
    drain(&mut v_rx);

    world.forced_rolls.push_back(51); // 51 > disapearChance → drops, not destroyed
    cursed_weapon::on_wielder_death(&mut world, PICKER_OID, KILLER_OID);

    let packets = drain(&mut v_rx);
    assert_eq!(
        equip_slot(&packets, InventorySlot::RHand),
        Some((0, 0)),
        "ExUserInfoEquipSlot resent with an empty right hand"
    );
}

/// The same death with the disappear roll hitting: the weapon leaves the world
/// entirely rather than dropping.
#[test]
fn wielder_death_can_destroy_the_weapon() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    world.id_pool = 0x3000_0000..0x3000_0100;
    let _v = ingame_player_access(&mut world, 1, PICKER_OID, 0);
    let idx = cw_idx(&world, ZARICHE);
    admin::cursed_weapons::activate(&mut world, idx, PICKER_OID);

    world.forced_rolls.push_back(0); // 0 <= disapearChance → destroyed
    cursed_weapon::on_wielder_death(&mut world, PICKER_OID, KILLER_OID);

    assert!(
        !world.cursed_weapons[idx].is_active(),
        "the weapon is gone from the world"
    );
    assert_eq!(ground_item_count(&world), 0, "and not on the ground");
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&PICKER_OID)
            .unwrap()
            .cursed_weapon_equipped_id,
        0,
        "the dead wielder is freed either way"
    );
}

/// A wielder's clan identity is hidden while cursed — Java blanks clan id,
/// both crests, ally id and ally crest in `PlayerAppearance`.
#[test]
fn curse_hides_the_pledge_identity() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    let _p = ingame_player_access(&mut world, 1, PICKER_OID, 0);
    {
        let p = world
            .objects
            .get_component_mut::<Player>(&PICKER_OID)
            .unwrap();
        p.clan_id = 77;
        p.clan_crest_id = 88;
        p.clan_crest_large_id = 99;
        p.ally_id = 111;
        p.ally_crest_id = 222;
        assert_eq!(p.visible_clan_id(), 77, "shown normally when un-cursed");
        p.cursed_weapon_equipped_id = ZARICHE;
        assert_eq!(p.visible_clan_id(), 0, "clan hidden while cursed");
        assert_eq!(p.visible_clan_crest_id(), 0);
        assert_eq!(p.visible_clan_crest_large_id(), 0);
        assert_eq!(p.visible_ally_id(), 0);
        assert_eq!(p.visible_ally_crest_id(), 0);
    }
}

/// The death wire itself, driven through the real `player_do_die` rather than
/// calling the cursed helper directly — the drop must actually be reachable
/// from a kill, and a cursed wielder must drop *the weapon* instead of
/// scattering their bag (Java's if/else-if against `onDieDropItem`).
#[test]
fn real_death_path_drops_the_cursed_weapon() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    world.id_pool = 0x3000_0000..0x3000_0100;
    let _v = ingame_player_access(&mut world, 1, PICKER_OID, 0);
    let _k = ingame_player_access(&mut world, 2, KILLER_OID, 0);
    let idx = cw_idx(&world, ZARICHE);
    admin::cursed_weapons::activate(&mut world, idx, PICKER_OID);
    assert!(world.cursed_weapons[idx].is_activated, "wielded to start");

    world.forced_rolls.push_back(51); // miss the disappear roll → it drops
    death::player_do_die(&mut world, PICKER_OID, KILLER_OID);

    assert!(
        world.cursed_weapons[idx].is_dropped,
        "dying with a cursed weapon drops it — reached through player_do_die"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&PICKER_OID)
            .unwrap()
            .cursed_weapon_equipped_id,
        0,
        "and the corpse is freed of the curse"
    );
}

/// Java `activate` never touches `_endTime`, so picking a weapon off the ground
/// **inherits** the drop's remaining life instead of restarting the clock —
/// letting one lie around must not buy the finder a fresh 300 minutes.
#[test]
fn pickup_inherits_the_drops_deadline() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    world.id_pool = 0x3000_0000..0x3000_0100;
    let mut killer_rx = ingame_player_access(&mut world, 1, KILLER_OID, 0);
    let _picker = ingame_player_access(&mut world, 2, PICKER_OID, 0);
    add_test_npc(&mut world, MONSTER_OID, 900001, "Monster", 20, 500, 600, 0);
    world.forced_rolls.push_back(0);
    cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);
    drain(&mut killer_rx);

    let idx = cw_idx(&world, ZARICHE);
    // Pretend the sword lay on the ground a while: wind its deadline in.
    let inherited = now_millis_test() + 42 * 60_000;
    world.cursed_weapons[idx].end_time = inherited;
    let ground_oid = world.cursed_weapons[idx].dropped_item_oid;

    ground_items::pickup_ground_item(&mut world, 2, PICKER_OID, ground_oid);

    assert!(world.cursed_weapons[idx].is_activated, "picker is cursed");
    assert_eq!(
        world.cursed_weapons[idx].end_time, inherited,
        "the drop's deadline carries through the pickup — not reset to now+duration"
    );
}

/// …whereas `//cw_add` *does* start a fresh life (Java sets `endTime` itself
/// right after the grant), so the GM path is unaffected by the change above.
#[test]
fn gm_grant_starts_a_fresh_life() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    world.id_pool = 0x3000_0000..0x3000_0100;
    let mut gm_rx = ingame_player_access(&mut world, 1, KILLER_OID, 100);
    drain(&mut gm_rx);

    let id_arg = ZARICHE.to_string();
    admin::cursed_weapons::admin_cw_add(&mut world, 1, KILLER_OID, &[&id_arg]);

    let idx = cw_idx(&world, ZARICHE);
    let duration_ms = world.cursed_weapons[idx].duration as i64 * 60_000;
    let left = world.cursed_weapons[idx].end_time - now_millis_test();
    assert!(
        left > duration_ms - 60_000 && left <= duration_ms,
        "a GM grant runs the full duration (left = {left} ms of {duration_ms})"
    );
    assert!(
        world
            .scheduler
            .pending_tasks_for_test()
            .iter()
            .any(|t| matches!(t, crate::scheduler::ScheduledTask::CursedWeaponExpiry { item_id } if *item_id == ZARICHE)),
        "and its removal task is armed against that deadline"
    );
}

/// The character-selection screen shows the demon form for a character who
/// logged out holding a cursed weapon, so the owner can tell at a glance that
/// the curse is still on them before entering the world.
///
/// This is a **deliberate deviation from Java**, which hard-codes 0 into the
/// transform field with the comment "on retail when you are on character select
/// you don't see your transformation". The field itself is the one the client
/// reads for a polymorphed model — L2J just declines to fill it.
#[test]
fn char_selection_shows_the_cursed_transform() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    let chars = vec![dummy_char(PICKER_OID, "Cursed")];

    let plain =
        server_packets::char_selection_info("acct", 1, &chars, -1, 7, &world.data.experience, &[]);

    restore_activated_on(&mut world, AKAMANAH, PICKER_OID);
    let cursed = server_packets::char_selection_info(
        "acct",
        1,
        &chars,
        -1,
        7,
        &world.data.experience,
        &world.cursed_weapons,
    );

    assert_eq!(
        plain.len(),
        cursed.len(),
        "only a field value changes, never the layout"
    );
    let diffs: Vec<usize> = (0..plain.len())
        .filter(|&i| plain[i] != cursed[i])
        .collect();
    assert!(
        !diffs.is_empty(),
        "the cursed character's selection entry must differ — a hard 0 would \
         make these packets identical and the screen would look un-cursed"
    );
    // The differing bytes are one little-endian i32 holding Akamanah's
    // transform (302); locate it rather than hard-coding a packet offset.
    let at = diffs[0];
    let field = i32::from_le_bytes([cursed[at], cursed[at + 1], cursed[at + 2], cursed[at + 3]]);
    assert_eq!(field, 302, "Akamanah's transform id is sent");
    assert_eq!(
        i32::from_le_bytes([plain[at], plain[at + 1], plain[at + 2], plain[at + 3]]),
        0,
        "and an un-cursed character still gets 0, as Java always does"
    );
}

/// Killed by an **NPC** (a guard) rather than a player — the reported case.
/// `on_wielder_death` only uses the killer for a fallback drop position, so a
/// non-player killer must strip the curse just the same. Without this the
/// player-killer test above would pass while guard kills silently kept the
/// weapon.
#[test]
fn guard_kill_also_drops_the_cursed_weapon() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    world.id_pool = 0x3000_0000..0x3000_0100;
    let _v = ingame_player_access(&mut world, 1, PICKER_OID, 0);
    add_test_npc(&mut world, MONSTER_OID, 900003, "Guard", 40, 500, 600, 0);
    let idx = cw_idx(&world, ZARICHE);
    admin::cursed_weapons::activate(&mut world, idx, PICKER_OID);

    world.forced_rolls.push_back(51); // miss the disappear roll → it drops
    death::player_do_die(&mut world, PICKER_OID, MONSTER_OID);

    assert_eq!(
        world
            .objects
            .get_component::<Player>(&PICKER_OID)
            .unwrap()
            .cursed_weapon_equipped_id,
        0,
        "an NPC kill lifts the curse too"
    );
    assert!(
        world
            .objects
            .get_component::<Inventory>(&PICKER_OID)
            .unwrap()
            .items()
            .iter()
            .all(|i| i.item_id != ZARICHE),
        "and the weapon actually leaves the bag — the reported symptom"
    );
    assert!(world.cursed_weapons[idx].is_dropped, "it is on the ground");
}

/// Give the test class a CP table so `calc_max_cp` has a base to multiply —
/// `GameData::for_test` ships empty templates, which would make the curse's
/// `PER` pump invisible (0 × anything is 0) and leave only the flat `DIFF`.
fn give_cp_template(world: &mut World) {
    let mut t = world
        .data
        .player_templates
        .get(0)
        .cloned()
        .unwrap_or_default();
    t.class_id = 0;
    t.cp_table = vec![200.0; 90];
    world.data.player_templates = crate::data::PlayerTemplateData::from_vec(vec![t]);
}

/// The cursed weapon's own skill (Akamanah 3629 / Zariche 3603) is a *passive*
/// whose effects include two `MaxCp` pumps — `PER 1050` (Java `mergeMul`
/// `amount/100 + 1` → ×11.5) and `DIFF 1300`. Java applies them in
/// `giveSkill()`'s `addSkill` and unmerges them in `removeSkill()`
/// (`EffectList.stopSkillEffects`), so the CP bar swells while cursed and is
/// exactly back to normal once the weapon is gone.
///
/// Both halves were broken: taking the curse never recomputed the vitals (the
/// pumps sat in `StatModifiers` while `PlayerVitals.max_cp` kept the class
/// value), and ending it dropped the skill from the `SkillBook` without
/// removing the passive buff — so the *next* recompute (`remove_transform`)
/// finally folded the pumps in and the freed player was left with the cursed
/// CP bar forever. Reported as "lost Akamanah, untransformed, still MAX CP
/// 3844".
#[test]
fn cursed_weapon_moves_max_cp_and_gives_it_back() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    give_cp_template(&mut world);
    world.id_pool = 0x3000_0000..0x3000_0100;
    let _rx = ingame_player_access(&mut world, 1, PICKER_OID, 0);

    let base_cp = pcp(&world, PICKER_OID).max_cp;
    assert!(base_cp > 0, "the template gives the class a CP bar to grow");

    let idx = cw_idx(&world, AKAMANAH);
    admin::cursed_weapons::activate(&mut world, idx, PICKER_OID);

    let expected = (11.5 * f64::from(base_cp) + 1300.0) as i32;
    let cursed_cp = pcp(&world, PICKER_OID).max_cp;
    assert_eq!(
        cursed_cp, expected,
        "Akamanah 3629 L1 pumps MaxCp ×11.5 +1300 the moment it is taken"
    );
    assert_eq!(
        pcp(&world, PICKER_OID).cur_cp as i32,
        cursed_cp,
        "and Java's `setCurrentCp(getMaxCp())` fills the grown bar"
    );

    admin::cursed_weapons::end_of_life(&mut world, idx);

    assert_eq!(
        pcp(&world, PICKER_OID).max_cp,
        base_cp,
        "losing the weapon takes the whole pump back with it"
    );
    assert!(
        pcp(&world, PICKER_OID).cur_cp as i32 <= base_cp,
        "and the current value is clamped into the shrunk bar"
    );
}

/// The other way a curse ends: the wielder is killed and the weapon drops off
/// the corpse (`CursedWeapon.dropIt`, which runs the same `removeSkill()`).
/// Same pump, same cleanup — this is the second Java call site.
#[test]
fn death_drop_takes_the_cursed_cp_back() {
    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    give_cp_template(&mut world);
    world.id_pool = 0x3000_0000..0x3000_0100;
    let _rx = ingame_player_access(&mut world, 1, PICKER_OID, 0);
    add_test_npc(&mut world, MONSTER_OID, 900004, "Guard", 40, 500, 600, 0);

    let base_cp = pcp(&world, PICKER_OID).max_cp;
    let idx = cw_idx(&world, AKAMANAH);
    admin::cursed_weapons::activate(&mut world, idx, PICKER_OID);
    assert!(
        pcp(&world, PICKER_OID).max_cp > base_cp,
        "cursed CP is up while the sword is held"
    );

    world.forced_rolls.push_back(51); // miss the disappear roll → it drops
    death::player_do_die(&mut world, PICKER_OID, MONSTER_OID);

    assert_eq!(
        pcp(&world, PICKER_OID).max_cp,
        base_cp,
        "the drop-on-death path unmerges the pump too"
    );
}

/// Self-heal on login: a character the manager does *not* consider cursed must
/// not keep the curse's skills.
///
/// Java can't hit this — `giveSkill` uses `addSkill(…, false)`, so 3629 never
/// reaches `character_skills`. This port persists the whole `SkillBook`, so a
/// row that escaped (an older build, or a crash between `//cw_remove` and the
/// next flush) came back on every login and re-armed the `MaxCp` ×11.5 +1300
/// pump with no weapon anywhere to explain it. `EnterWorld`'s stray-weapon
/// sweep now scrubs the skills too.
#[test]
fn enter_world_scrubs_a_cursed_skill_with_no_curse_behind_it() {
    use crate::model::components::SkillBook;

    let (mut world, _db, _db_rx, _l) = test_world();
    load_curse_data(&mut world);
    give_cp_template(&mut world);
    world.id_pool = 0x3000_0000..0x3000_0100;
    let _rx = ingame_player_access(&mut world, 1, PICKER_OID, 0);

    let base_cp = pcp(&world, PICKER_OID).max_cp;
    assert!(base_cp > 0, "the template gives the class a CP bar to grow");

    // The bad state: 3629 restored from `character_skills` at login, while no
    // cursed weapon is activated on anyone.
    world
        .objects
        .get_component_mut::<SkillBook>(&PICKER_OID)
        .unwrap()
        .0
        .insert(3629, 1);
    passive_skills::refresh_conditioned_passives(&mut world, PICKER_OID);
    assert_eq!(
        pcp(&world, PICKER_OID).max_cp,
        (11.5 * f64::from(base_cp) + 1300.0) as i32,
        "the stale row really does re-arm the cursed CP pump"
    );
    assert!(
        world.cursed_weapons.iter().all(|cw| !cw.is_activated),
        "and nothing in the world justifies it"
    );

    cursed_weapon::on_enter_world(&mut world, 1, PICKER_OID);

    assert!(
        !world
            .objects
            .get_component::<SkillBook>(&PICKER_OID)
            .unwrap()
            .0
            .contains_key(&3629),
        "the orphaned cursed skill is scrubbed"
    );
    assert_eq!(
        pcp(&world, PICKER_OID).max_cp,
        base_cp,
        "and the CP bar is back to the class value"
    );
}

/// The drop announce names the **region**, and does it Java's way.
///
/// The port used to send `SysString(0)` here — parameter type 13 carrying a
/// system-string id of zero, which the client renders as nothing. Java sends
/// `addZoneName(x, y, z)`: parameter type **7** with the coordinates, and the
/// *client* resolves the region name. So the marker on these sites ("MapRegion
/// carries no sysstring id yet") had the mechanism wrong — no server-side
/// region table is involved at all.
///
/// Decoding the parameter rather than just the message id is the point: the
/// wrong-type version passed an id-only assertion perfectly happily.
#[test]
fn the_drop_announce_carries_the_zone_coordinates() {
    let (mut world, _db, mut _db_rx, _l) = test_world();
    load_cursed_weapons(&mut world);
    let mut killer_rx = ingame_player_access(&mut world, 1, KILLER_OID, 0);
    drain(&mut killer_rx);
    // The drop point is the *monster's* position — that is what Java hands to
    // `addZoneName`, so the coordinates below are deliberately distinctive.
    add_test_npc(
        &mut world,
        MONSTER_OID,
        900001,
        "Monster",
        20,
        1234,
        5678,
        -90,
    );
    world.forced_rolls.push_back(0);

    cursed_weapon::on_monster_killed(&mut world, MONSTER_OID, KILLER_OID);

    let pkt = drain(&mut killer_rx)
        .into_iter()
        .find(|p| {
            p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && i16::from_le_bytes([p[1], p[2]])
                    == server_packets::sm_ids::S2_WAS_DROPPED_IN_THE_S1_REGION
        })
        .expect("the drop announce");

    // Layout: opcode(1) id(i16) count(u8) then params; the first is the zone.
    assert_eq!(pkt[3], 2, "two parameters: the zone and the item name");
    assert_eq!(pkt[4], 7, "TYPE_ZONE_NAME, not TYPE_SYSTEM_STRING (13)");
    let coord =
        |off: usize| i32::from_le_bytes([pkt[off], pkt[off + 1], pkt[off + 2], pkt[off + 3]]);
    assert_eq!(
        (coord(5), coord(9), coord(13)),
        (1234, 5678, -90),
        "the drop point, which is what the client resolves the region from"
    );
}

/// `//cw_goto` has **both** of Java's branches: to the wielder while the
/// weapon is carried, and to the item on the ground while it waits to be
/// picked up.
///
/// The ground branch is the one a GM actually needs — a carried weapon
/// announces its holder, an un-grabbed drop is silent.
#[test]
fn cw_goto_reaches_a_weapon_lying_on_the_ground() {
    const GROUND_ITEM: i32 = 0x3100_0500;
    const AWAY: (i32, i32, i32) = (77_000, 88_000, -3_000);

    let (mut world, _db, _db_rx, _l) = test_world();
    load_cursed_weapons(&mut world);
    world.data.admin = crate::data::AdminData::load_from(crate::data::DIST_GAME);
    world.id_pool = 0x3100_0000..0x3100_0400;
    let mut gm_rx = ingame_player_access(&mut world, 1, KILLER_OID, 100);
    drain(&mut gm_rx);

    let idx = cw_idx(&world, ZARICHE);
    let id_arg = ZARICHE.to_string();

    // Not in the world at all: Java answers "isn't in the World" and the GM
    // stays put. Without this the assertion below could pass on a no-op.
    let home = *world
        .objects
        .get_component::<Position>(&KILLER_OID)
        .unwrap();
    admin::use_admin_command(&mut world, 1, &format!("admin_cw_goto {id_arg}"), false);
    let now = *world
        .objects
        .get_component::<Position>(&KILLER_OID)
        .unwrap();
    assert_eq!((now.x, now.y), (home.x, home.y), "nothing to go to");

    // Dropped on the ground, far away.
    world.cursed_weapons[idx].is_dropped = true;
    world.cursed_weapons[idx].dropped_item_oid = GROUND_ITEM;
    world.objects.spawn(
        GROUND_ITEM,
        Position {
            x: AWAY.0,
            y: AWAY.1,
            z: AWAY.2,
            heading: 0,
        },
    );

    admin::use_admin_command(&mut world, 1, &format!("admin_cw_goto {id_arg}"), false);
    let at = *world
        .objects
        .get_component::<Position>(&KILLER_OID)
        .unwrap();
    // x/y exactly; z within a few units, because `teleport_player` snaps the
    // arrival to the ground the way every other teleport does.
    assert_eq!(
        (at.x, at.y),
        (AWAY.0, AWAY.1),
        "the GM is teleported to the dropped weapon"
    );
    assert!(
        (at.z - AWAY.2).abs() <= 32,
        "…landing on the ground there, not at some other z: {}",
        at.z
    );
}
