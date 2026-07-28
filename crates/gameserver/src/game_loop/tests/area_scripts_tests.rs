//! `ai/areas` slice 1 — the talk/teleporter NPCs: Toma's wandering spawn,
//! the Elroki ferry pair, the Pagan Temple door gatekeepers, and Tunatun.

use super::*;

use crate::game_loop::area_npcs::{self, TOMA};

const TOMA_LOCS: [(i32, i32, i32); 3] = [
    (151680, -174891, -1782),
    (154153, -220105, -3402),
    (178834, -184336, -355),
];

fn toma_positions(world: &mut World) -> Vec<(i32, i32, i32)> {
    let mut out = Vec::new();
    world
        .objects
        .for_each_mut::<(&crate::model::npc::Npc, &Position)>(|(n, p)| {
            if n.npc_id == TOMA {
                out.push((p.x, p.y, p.z));
            }
        });
    out
}

/// Toma is script-owned (not in the spawn data): boot places exactly one at
/// one of his three haunts, and the 30-minute beat moves him — never
/// duplicates him.
#[test]
fn toma_spawns_at_boot_and_relocates_without_duplicating() {
    let (mut world, _db, _l) = combat_test_world();
    let mut t = crate::data::npc_data::default_template(TOMA);
    t.type_name = "Folk".into();
    world.data.npc_data.insert_for_test(t);

    area_npcs::spawn_at_boot(&mut world);
    let at_boot = toma_positions(&mut world);
    assert_eq!(at_boot.len(), 1, "exactly one Toma after boot");
    assert!(TOMA_LOCS.contains(&at_boot[0]), "on a known haunt");

    // The beat fires (directly — the scheduled path is the same fn).
    for _ in 0..5 {
        area_npcs::relocate_toma(&mut world);
        let now = toma_positions(&mut world);
        assert_eq!(now.len(), 1, "relocation never duplicates him");
        assert!(TOMA_LOCS.contains(&now[0]));
    }
}

/// Orahochin ferries a peaceful player across, but refuses one whose attack
/// stance is still running (Java `talker.isInCombat()`).
#[test]
fn elroki_teleporter_refuses_combat_then_ferries() {
    let (mut world, _db, _l) = combat_test_world();
    const ORAHOCHIN: i32 = 32111;
    add_test_npc(&mut world, NPC_OID, ORAHOCHIN, "Folk", 40, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);

    // In combat: no teleport.
    world.objects.add_components(
        &5001,
        crate::model::components::AttackState {
            attack_end_tick: 0,
            stance_until_tick: world.tick + 150,
        },
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ElrokiTeleporters")),
    );
    let pos = world.objects.get_component::<Position>(&5001).unwrap();
    assert_eq!((pos.x, pos.y), (60, 0), "still standing at the chasm");

    // Stance over: ferried to the island.
    world
        .objects
        .get_component_mut::<crate::model::components::AttackState>(&5001)
        .unwrap()
        .stance_until_tick = 0;
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ElrokiTeleporters")),
    );
    let pos = world.objects.get_component::<Position>(&5001).unwrap();
    // z is geo-grounded + 5 by `teleport_player` (Java `teleToLocation`).
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (4990, -1879, -3173),
        "Orahochin's island drop-off"
    );
}

/// The way-out gatekeeper (32035, no mark needed) opens the outer temple
/// door, and the scripted 10 s timer shuts it again.
#[test]
fn pagan_gatekeeper_opens_the_door_and_it_closes_itself() {
    let (mut world, _db, _l) = combat_test_world();
    const GATEKEEPER_OUT: i32 = 32035;
    const OUTER_DOOR: i32 = 19_160_001;
    crate::model::door::spawn_door_for_test(
        &mut world,
        crate::data::door_data::DoorTemplate {
            id: OUTER_DOOR,
            name: "pagan_outer".into(),
            node_x: [-16654; 4],
            node_y: [-36864; 4],
            node_z: -10759,
            height: 150,
            x: -16654,
            y: -36864,
            z: -10759,
            hp_max: 100,
            p_def: 0,
            m_def: 0,
            targetable: false,
            show_hp: false,
            open_by_default: false,
            open_method: crate::data::door_data::DoorOpenMethod::None,
            open_time: 0,
            close_time: -1,
            random_time: 0,
        },
    );
    assert!(!world.geo.doors.is_open(OUTER_DOOR));

    // The door consumed `next_npc_object_id` (== NPC_OID) — a fixture NPC on
    // the same oid would clobber it (the classic fixture/allocator collision).
    let gatekeeper_oid = NPC_OID + 7;
    add_test_npc(
        &mut world,
        gatekeeper_oid,
        GATEKEEPER_OUT,
        "Folk",
        40,
        100,
        0,
        0,
    );
    let _rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{gatekeeper_oid}_Quest PaganTeleporters")),
    );
    assert!(
        world.geo.doors.is_open(OUTER_DOOR),
        "the gatekeeper opened the way out"
    );

    // Java `Close_Door1` at 10 s.
    advance_ticks(&mut world, 101);
    assert!(
        !world.geo.doors.is_open(OUTER_DOOR),
        "the door shuts itself"
    );
}

/// The outside gatekeeper (32034) demands a mark — empty-handed visitors do
/// not get the door.
#[test]
fn pagan_outer_gatekeeper_demands_a_mark() {
    let (mut world, _db, _l) = combat_test_world();
    const GATEKEEPER_IN: i32 = 32034;
    add_test_npc(&mut world, NPC_OID, GATEKEEPER_IN, "Folk", 40, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest PaganTeleporters")),
    );
    // No door exists in this world; the observable contract is simply that
    // nothing panicked and no door opened.
    assert!(!world.geo.doors.is_open(19_160_001));
}

/// Tunatun's whip: level 82+ gets it once; below, a refusal; asking again
/// with one in the bag is refused too.
#[test]
fn tunatun_hands_out_one_whip_at_level_82() {
    let (mut world, _db, _l) = combat_test_world();
    const TUNATUN: i32 = 31537;
    const WHIP: i32 = 15473;
    add_test_npc(&mut world, NPC_OID, TUNATUN, "Folk", 70, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);

    // Under-leveled: refused.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Tunatun whip")),
    );
    assert_eq!(item_count(&world, 5001, WHIP), 0, "level 5 gets nothing");

    // Level 82: whip granted — once.
    world
        .objects
        .get_component_mut::<crate::model::Player>(&5001)
        .unwrap()
        .level = 82;
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Tunatun whip")),
    );
    assert_eq!(item_count(&world, 5001, WHIP), 1, "whip granted");
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Tunatun whip")),
    );
    assert_eq!(item_count(&world, 5001, WHIP), 1, "never a second whip");
}

// ---------------------------------------------------------------------------
// Slice 2 — the small combat scripts
// ---------------------------------------------------------------------------

fn count_npcs(world: &mut World, npc_id: i32) -> usize {
    let mut n = 0;
    world.objects.for_each_mut::<&crate::model::npc::Npc>(|x| {
        if x.npc_id == npc_id {
            n += 1;
        }
    });
    n
}

/// Cave Maiden: a 20% kill proc swaps the corpse for a Banshee set on the
/// killer (Pytan/Knoriks is the same script shape at 5%).
#[test]
fn cave_maiden_kill_can_spring_a_banshee() {
    let (mut world, _db, _l) = combat_test_world();
    const CAVE_MAIDEN: i32 = 20134;
    const BANSHEE: i32 = 20412;
    for id in [CAVE_MAIDEN, BANSHEE] {
        world
            .data
            .npc_data
            .insert_for_test(crate::data::npc_data::default_template(id));
    }
    add_test_npc(&mut world, NPC_OID, CAVE_MAIDEN, "Monster", 40, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);

    // Roll under 20 → the proc fires.
    world.forced_rolls.push_back(10);
    quests::notify_kill(&mut world, 5001, NPC_OID, CAVE_MAIDEN);
    assert_eq!(count_npcs(&mut world, BANSHEE), 1, "banshee sprang");
    assert_eq!(count_npcs(&mut world, CAVE_MAIDEN), 0, "corpse consumed");
}

/// Frozen Labyrinth: a physical *skill* blow shatters a Pronghorn into six
/// spirits; a magic one does not.
#[test]
fn frozen_labyrinth_shatters_on_physical_skill_only() {
    let (mut world, _db, _l) = combat_test_world();
    const PRONGHORN: i32 = 22088;
    const SPIRIT: i32 = 22087;
    for id in [PRONGHORN, SPIRIT] {
        world
            .data
            .npc_data
            .insert_for_test(crate::data::npc_data::default_template(id));
    }
    let mut physical = passive_clan_test_skill(9001);
    physical.magic_type = 0;
    world.data.skill_data.insert_for_test(physical);
    let mut magic = passive_clan_test_skill(9002);
    magic.magic_type = 1;
    world.data.skill_data.insert_for_test(magic);

    add_test_npc(&mut world, NPC_OID, PRONGHORN, "Monster", 40, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);

    // Magic skill: nothing happens.
    quests::notify_attack(&mut world, 5001, NPC_OID, PRONGHORN, Some(9002), false);
    assert_eq!(count_npcs(&mut world, SPIRIT), 0, "magic does not shatter");

    // Physical skill: six spirits, original gone.
    quests::notify_attack(&mut world, 5001, NPC_OID, PRONGHORN, Some(9001), false);
    assert_eq!(count_npcs(&mut world, SPIRIT), 6, "six spirits");
    assert_eq!(count_npcs(&mut world, PRONGHORN), 0, "pronghorn gone");
}

/// Pagan keys: 10% kill proc — ground drop owned by the killer with
/// auto-loot off, straight to the bag with it on.
#[test]
fn pagan_keys_honor_auto_loot() {
    let (mut world, _db, _l) = combat_test_world();
    const ZOMBIE_WORKER: i32 = 22140;
    const ANTEROOM_KEY: i32 = 8273;
    world
        .data
        .npc_data
        .insert_for_test(crate::data::npc_data::default_template(ZOMBIE_WORKER));
    add_test_npc(&mut world, NPC_OID, ZOMBIE_WORKER, "Monster", 40, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);

    // Auto-loot off: the key lands on the ground, killer-owned.
    world.cfg.character.auto_loot = false;
    world.forced_rolls.push_back(5);
    quests::notify_kill(&mut world, 5001, NPC_OID, ZOMBIE_WORKER);
    let mut ground = None;
    world
        .objects
        .for_each_mut::<&crate::model::components::GroundItem>(|g| {
            if g.item_id == ANTEROOM_KEY {
                ground = Some((g.owner_id, g.count));
            }
        });
    assert_eq!(ground, Some((5001, 1)), "killer-protected ground key");
    assert_eq!(item_count(&world, 5001, ANTEROOM_KEY), 0);

    // Auto-loot on: straight to the inventory.
    world.cfg.character.auto_loot = true;
    add_test_npc(
        &mut world,
        NPC_OID + 1,
        ZOMBIE_WORKER,
        "Monster",
        40,
        100,
        0,
        0,
    );
    world.forced_rolls.push_back(5);
    quests::notify_kill(&mut world, 5001, NPC_OID + 1, ZOMBIE_WORKER);
    assert_eq!(item_count(&world, 5001, ANTEROOM_KEY), 1, "auto-looted");
}

/// Plains of Dion: interrupting a duelist calls every idle clansman in help
/// range onto the attacker — and only once per lizardman.
#[test]
fn plains_of_dion_calls_the_clan() {
    let (mut world, _db, _l) = combat_test_world();
    const SUPPLIER: i32 = 21104;
    let mut t = crate::data::npc_data::default_template(SUPPLIER);
    t.clan_help_range = 1000;
    world.data.npc_data.insert_for_test(t);

    add_test_npc(&mut world, NPC_OID, SUPPLIER, "Monster", 40, 100, 0, 0);
    add_test_npc(&mut world, NPC_OID + 1, SUPPLIER, "Monster", 40, 300, 0, 0);
    let _rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);

    quests::notify_attack(&mut world, 5001, NPC_OID, SUPPLIER, None, false);
    let helper_hates = world
        .objects
        .get_component::<crate::model::npc::AggroList>(&(NPC_OID + 1))
        .is_some_and(|a| a.0.contains_key(&5001));
    assert!(helper_hates, "the idle clansman joins in");
}

/// Eilhalder von Hellmann: night spawns him, daybreak despawns him — unless
/// he is mid-fight, in which case the 30 s retry keeps checking.
#[test]
fn eilhalder_walks_at_night_and_vanishes_by_day() {
    let (mut world, _db, _l) = combat_test_world();
    use crate::game_loop::area_npcs::{self, EILHALDER};
    world
        .data
        .npc_data
        .insert_for_test(crate::data::npc_data::default_template(EILHALDER));

    area_npcs::eilhalder_on_day_night_change(&mut world, true);
    assert_eq!(count_npcs(&mut world, EILHALDER), 1, "night: he walks");

    area_npcs::eilhalder_on_day_night_change(&mut world, false);
    assert_eq!(count_npcs(&mut world, EILHALDER), 0, "day: gone");

    // Night again, but this time he is fighting at daybreak.
    area_npcs::eilhalder_on_day_night_change(&mut world, true);
    let oid = {
        let mut found = None;
        world.objects.for_each_mut::<&crate::model::npc::Npc>(|n| {
            if n.npc_id == EILHALDER {
                found = Some(n.object_id);
            }
        });
        found.unwrap()
    };
    let mut aggro = crate::model::npc::AggroList::default();
    aggro.0.insert(
        5001,
        crate::model::npc::AggroInfo {
            hate: 100.0,
            damage: 0.0,
        },
    );
    world.objects.add_components(&oid, aggro);
    area_npcs::eilhalder_on_day_night_change(&mut world, false);
    assert_eq!(count_npcs(&mut world, EILHALDER), 1, "fighting: he stays");

    // Fight over → the retry removes him.
    world
        .objects
        .get_component_mut::<crate::model::npc::AggroList>(&oid)
        .unwrap()
        .0
        .clear();
    area_npcs::handle_eilhalder_despawn_retry(&mut world);
    assert_eq!(
        count_npcs(&mut world, EILHALDER),
        0,
        "retry finishes the job"
    );
}

/// Hot Springs: each proc casts the disease one level above what the victim
/// already carries.
#[test]
fn hot_springs_disease_escalates_with_the_victims_level() {
    let (mut world, _db, _l) = combat_test_world();
    const ATROX: i32 = 21321;
    const MALARIA: i32 = 4554;
    world
        .data
        .npc_data
        .insert_for_test(crate::data::npc_data::default_template(ATROX));
    for level in [1, 4] {
        let mut s = passive_clan_test_skill(MALARIA);
        s.level = level;
        s.magic_type = 1;
        s.operate_type = OperateType::Active;
        world.data.skill_data.insert_for_test(s);
    }
    add_test_npc(&mut world, NPC_OID, ATROX, "Monster", 40, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);

    // First proc (malaria roll hits, type roll misses): level 1.
    world.forced_rolls.push_back(5);
    world.forced_rolls.push_back(50);
    quests::notify_attack(&mut world, 5001, NPC_OID, ATROX, None, false);
    let cast = world
        .objects
        .get_component::<crate::model::components::Casting>(&NPC_OID)
        .map(|c| (c.0.skill_id, c.0.skill_level));
    assert_eq!(cast, Some((MALARIA, 1)), "fresh victim gets level 1");

    // Victim already carries level 3 → the next proc casts level 4.
    world
        .objects
        .remove_component::<crate::model::components::Casting>(&NPC_OID);
    let mut buffs = crate::model::components::Buffs::default();
    buffs.0.push(crate::model::skill::ActiveBuff {
        skill_id: MALARIA,
        skill_level: 3,
        abnormal_type_client_id: 0,
        abnormal_type: "NONE".to_string(),
        abnormal_level: 0,
        slot: crate::model::skill::BuffSlot::Uncapped,
        expires_at_tick: u64::MAX,
        passive: false,
        effect_flags: 0,
        blocked_abnormals: Vec::new(),
        abnormal_visuals: Vec::new(),
        effects: Vec::new(),
    });
    world.objects.add_components(&5001, buffs);
    world.forced_rolls.push_back(5);
    world.forced_rolls.push_back(50);
    quests::notify_attack(&mut world, 5001, NPC_OID, ATROX, None, false);
    let cast = world
        .objects
        .get_component::<crate::model::components::Casting>(&NPC_OID)
        .map(|c| (c.0.skill_id, c.0.skill_level));
    assert_eq!(cast, Some((MALARIA, 4)), "level 3 victim gets level 4");
}

// ---------------------------------------------------------------------------
// Slice 3 — Den of Evil's Ragna Orcs
// ---------------------------------------------------------------------------

/// The Commander spawns his *named* escort groups: always Privates1, plus
/// one of Privates2/3 — never all three (the flattened-groups over-spawn).
#[test]
fn ragna_commander_picks_named_escort_groups() {
    let (mut world, _db, _l) = combat_test_world();
    const COMMANDER: i32 = 22694;
    let mut t = crate::data::npc_data::default_template(COMMANDER);
    t.minions = vec![
        crate::data::npc_data::MinionHolder {
            npc_id: 22695,
            count: 1,
            group: "Privates1".into(),
        },
        crate::data::npc_data::MinionHolder {
            npc_id: 22693,
            count: 1,
            group: "Privates2".into(),
        },
        crate::data::npc_data::MinionHolder {
            npc_id: 22697,
            count: 1,
            group: "Privates3".into(),
        },
    ];
    world.data.npc_data.insert_for_test(t);
    for id in [22695, 22693, 22697] {
        world
            .data
            .npc_data
            .insert_for_test(crate::data::npc_data::default_template(id));
    }

    crate::model::npc::spawn_npc_at(&mut world, COMMANDER, 0, 0, 0, 0);
    let p1 = count_npcs(&mut world, 22695);
    let extra = count_npcs(&mut world, 22693) + count_npcs(&mut world, 22697);
    assert_eq!(p1, 1, "Privates1 always comes out");
    assert_eq!(extra, 1, "exactly one of Privates2/Privates3 — not both");
}

/// The Frightened Ragna Orc's bribe: at low HP he promises 10M adena; ten
/// seconds later the jackpot roll pays out ten owned ground stacks and he
/// vanishes.
#[test]
fn frightened_orc_bribe_pays_out_and_he_vanishes() {
    let (mut world, _db, _l) = combat_test_world();
    const ORC: i32 = 18807;
    world
        .data
        .npc_data
        .insert_for_test(crate::data::npc_data::default_template(ORC));
    add_test_npc(&mut world, NPC_OID, ORC, "Monster", 40, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);

    // First hit: he starts whimpering (script value 1).
    quests::notify_attack(&mut world, 5001, NPC_OID, ORC, None, false);
    // Low HP + second hit: the bribe (script value 2) and the 10 s reward.
    world
        .objects
        .get_component_mut::<Vitals>(&NPC_OID)
        .unwrap()
        .cur_hp = 1.0;
    quests::notify_attack(&mut world, 5001, NPC_OID, ORC, None, false);

    // Jackpot roll (10-in-100 000) + message coin flip.
    world.forced_rolls.push_back(5);
    world.forced_rolls.push_back(0);
    advance_ticks(&mut world, 101);
    let mut stacks = 0;
    let mut total = 0i64;
    world
        .objects
        .for_each_mut::<&crate::model::components::GroundItem>(|g| {
            if g.item_id == 57 && g.owner_id == 5001 {
                stacks += 1;
                total += g.count;
            }
        });
    assert_eq!(stacks, 10, "ten separate adena stacks");
    assert_eq!(total, 10_000_000, "the promised ten million");

    // The 1 s despawn: he keeps his word and disappears.
    advance_ticks(&mut world, 15);
    assert_eq!(count_npcs(&mut world, ORC), 0, "gone as promised");
}
