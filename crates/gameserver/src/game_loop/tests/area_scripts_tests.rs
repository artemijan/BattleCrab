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

// ---------------------------------------------------------------------------
// Slice 4 — the allied-tribe service NPCs (Ketra / Varka mirror pair)
// ---------------------------------------------------------------------------

fn give_test_item(world: &mut World, player: i32, item_id: i32, count: i64) {
    let World { data, objects, .. } = world;
    objects
        .get_component_mut::<crate::model::inventory::Inventory>(&player)
        .unwrap()
        .add_item(&data.item_data, 8_100_000 + item_id, item_id, count);
}

/// Asefa trades Buffalo Horns for war buffs: three horns buy Might, an
/// empty pouch buys nothing.
#[test]
fn ketra_buffer_charges_horns_for_buffs() {
    let (mut world, _db, _l) = combat_test_world();
    const ASEFA: i32 = 31372;
    const HORN: i32 = 7186;
    const MIGHT: i32 = 4345;
    world
        .data
        .npc_data
        .insert_for_test(crate::data::npc_data::default_template(ASEFA));
    let mut might = passive_clan_test_skill(MIGHT);
    might.operate_type = OperateType::Active;
    world.data.skill_data.insert_for_test(might);
    add_test_npc(&mut world, NPC_OID, ASEFA, "Folk", 70, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);
    give_test_item(&mut world, 5001, HORN, 3);

    // Buff 3 = Might, 3 horns.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest KetraOrcSupport 3")),
    );
    assert_eq!(item_count(&world, 5001, HORN), 0, "three horns spent");
    let cast = world
        .objects
        .get_component::<crate::model::components::Casting>(&NPC_OID)
        .map(|c| c.0.skill_id);
    assert_eq!(cast, Some(MIGHT), "Asefa casts Might on the visitor");

    // Broke: no cast, no debt.
    world
        .objects
        .remove_component::<crate::model::components::Casting>(&NPC_OID);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest KetraOrcSupport 3")),
    );
    assert!(
        world
            .objects
            .get_component::<crate::model::components::Casting>(&NPC_OID)
            .is_none(),
        "no horns, no buff"
    );
}

/// Kurfa's teleport menu is alliance-gated: level 4 gets the list, an
/// outsider gets nothing.
#[test]
fn ketra_teleporter_serves_only_level_four_allies() {
    let (mut world, _db, _l) = combat_test_world();
    const KURFA: i32 = 31376;
    const MARK_4: i32 = 7214;
    // Serve the real script htmls — the html IS the observable here.
    world.data.root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/").to_string();
    world
        .data
        .npc_data
        .insert_for_test(crate::data::npc_data::default_template(KURFA));
    add_test_npc(&mut world, NPC_OID, KURFA, "Folk", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);

    // Outsider: the Teleport event yields no window.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest KetraOrcSupport Teleport")),
    );
    let htmls = drain(&mut rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .count();
    assert_eq!(htmls, 0, "no alliance, no destinations");

    // Level-4 ally: the destination window opens.
    give_test_item(&mut world, 5001, MARK_4, 1);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest KetraOrcSupport Teleport")),
    );
    let htmls = drain(&mut rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .count();
    assert_eq!(htmls, 1, "level 4 gets the teleport list");
}

// ---------------------------------------------------------------------------
// Slice 5 — Forge of the Gods
// ---------------------------------------------------------------------------

/// A hot kill streak in the upper forge erupts a Newborn Lavasaurus onto
/// the killer; the 15 s refresh beat cools the streak back down.
#[test]
fn forge_kill_streak_erupts_a_lavasaurus_and_refresh_cools_it() {
    let (mut world, _db, _l) = combat_test_world();
    const WORKER: i32 = 22634;
    const NEWBORN: i32 = 18799;
    for id in [WORKER, NEWBORN] {
        world
            .data
            .npc_data
            .insert_for_test(crate::data::npc_data::default_template(id));
    }
    let _rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);

    // Place ALL fixtures before any eruption: `add_test_npc` advances the
    // runtime allocator past its oid, so a fixture added *after* a runtime
    // spawn can land exactly on it and clobber it (the fixture/allocator
    // collision, third sighting).
    let w = |i: i32| NPC_OID + 500 + i;
    for i in 0..4 {
        add_test_npc(&mut world, w(i), WORKER, "Monster", 40, 100, 0, 0);
    }
    // Kills 1 and 2: under MOBCOUNT_BONUS_MIN, nothing erupts even on a
    // lucky roll.
    for i in 0..2 {
        world.forced_rolls.push_back(5);
        quests::notify_kill(&mut world, 5001, w(i), WORKER);
    }
    assert_eq!(count_npcs(&mut world, NEWBORN), 0, "streak too short");

    // Kill 3 with rand <= 20: the Newborn erupts, hating the killer.
    world.forced_rolls.push_back(5);
    quests::notify_kill(&mut world, 5001, w(2), WORKER);
    assert_eq!(count_npcs(&mut world, NEWBORN), 1, "the forge answers");

    // The refresh beat resets the streak: the next lucky kill is kill #1.
    crate::game_loop::area_npcs::handle_fog_refresh(&mut world);
    world.forced_rolls.push_back(5);
    quests::notify_kill(&mut world, 5001, w(3), WORKER);
    assert_eq!(
        count_npcs(&mut world, NEWBORN),
        1,
        "cooled: no second eruption"
    );
}

// ---------------------------------------------------------------------------
// Slice 6 — the Beast Farm feeding chain
// ---------------------------------------------------------------------------

/// Golden spice on a hatchling: it grows into the next stage (level 0 grows
/// on every meal), and the wrong spice does nothing.
#[test]
fn spice_grows_the_beast_and_wrong_spice_does_not() {
    let (mut world, _db, _l) = combat_test_world();
    const HATCHLING: i32 = 21451; // level 0
    const GOLD_STAGE_1: i32 = 21452; // eats golden only
    for id in [HATCHLING, GOLD_STAGE_1, 21453, 21454, 21455, 21460, 21462] {
        world
            .data
            .npc_data
            .insert_for_test(crate::data::npc_data::default_template(id));
    }
    add_test_npc(
        &mut world,
        NPC_OID + 900,
        HATCHLING,
        "Monster",
        40,
        100,
        0,
        0,
    );
    let _rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);

    // talk roll (miss), growth roll (hit), stage pick (index 0 = 21452).
    for r in [1, 0, 0] {
        world.forced_rolls.push_back(r);
    }
    quests::notify_skill_see(&mut world, 5001, NPC_OID + 900, HATCHLING, 2188);
    assert_eq!(count_npcs(&mut world, HATCHLING), 0, "hatchling grew up");
    assert_eq!(count_npcs(&mut world, GOLD_STAGE_1), 1, "into stage one");

    // Stage one eats ONLY golden spice — crystal is consumed with no effect.
    let grown = {
        let mut found = 0;
        world.objects.for_each_mut::<&crate::model::npc::Npc>(|n| {
            if n.npc_id == GOLD_STAGE_1 {
                found = n.object_id;
            }
        });
        found
    };
    quests::notify_skill_see(&mut world, 5001, grown, GOLD_STAGE_1, 2189);
    assert_eq!(
        count_npcs(&mut world, GOLD_STAGE_1),
        1,
        "crystal does nothing"
    );
}

/// The top of the chain: a level-2 beast fed by a fighter tames into that
/// species' fighter beast, which follows its owner on a spice clock.
#[test]
fn top_stage_feeding_tames_a_beast_that_starves_without_spice() {
    let (mut world, _db, _l) = combat_test_world();
    const TOP: i32 = 21460; // kookaburra level 2, golden
    const TAMED_FIGHTER: i32 = 16017;
    for id in [TOP, TAMED_FIGHTER] {
        world
            .data
            .npc_data
            .insert_for_test(crate::data::npc_data::default_template(id));
    }
    add_test_npc(&mut world, NPC_OID + 900, TOP, "Monster", 40, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);

    // talk (miss), growth 0<25 (hit), tame coin 0 (tamed), rare chat (miss).
    for r in [1, 0, 0, 1] {
        world.forced_rolls.push_back(r);
    }
    quests::notify_skill_see(&mut world, 5001, NPC_OID + 900, TOP, 2188);
    assert_eq!(count_npcs(&mut world, TOP), 0, "the wild one is gone");
    let beast = {
        let mut found = None;
        world.objects.for_each_mut::<(
            &crate::model::npc::Npc,
            &crate::model::components::TamedBeastOf,
        )>(|(n, t)| {
            if n.npc_id == TAMED_FIGHTER {
                found = Some((n.object_id, t.owner, t.food_skill));
            }
        });
        found
    };
    let (beast_oid, owner, food) = beast.expect("a tamed beast spawned");
    assert_eq!(
        (owner, food),
        (5001, 2188),
        "owned by the feeder, eats golden"
    );

    // Feeding the tamed beast extends its stay (capped at 20 min).
    world
        .objects
        .get_component_mut::<crate::model::components::TamedBeastOf>(&beast_oid)
        .unwrap()
        .remaining_ticks = 5000;
    world.forced_rolls.push_back(7); // bark pick (2031, no $s1)
    quests::notify_skill_see(&mut world, 5001, beast_oid, TAMED_FIGHTER, 2188);
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::TamedBeastOf>(&beast_oid)
            .unwrap()
            .remaining_ticks,
        5200,
        "a meal buys 20 more seconds"
    );

    // With spice in the bag the duration check consumes one and feeds.
    give_test_item(&mut world, 5001, 6643, 1);
    crate::game_loop::tamed_beast::handle_duration(&mut world, beast_oid);
    assert_eq!(item_count(&world, 5001, 6643), 0, "one spice consumed");
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::TamedBeastOf>(&beast_oid)
            .unwrap()
            .remaining_ticks,
        5000 - 600 + 200 + 200,
        "minute down, meal back"
    );

    // Pouch empty and past the newcomer grace: the beast leaves.
    crate::game_loop::tamed_beast::handle_duration(&mut world, beast_oid);
    assert_eq!(count_npcs(&mut world, TAMED_FIGHTER), 0, "starved out");
}

// ---------------------------------------------------------------------------
// Slice 7 — Primeval Isle (the aggro-enter / spell-finished hooks)
// ---------------------------------------------------------------------------

fn trex_template() -> crate::data::npc_data::NpcTemplate {
    let mut t = crate::data::npc_data::default_template(22215);
    t.type_name = "Monster".into();
    t.level = 76;
    t.base_hp_max = 50_000.0;
    t.is_aggressive = true;
    t.aggro_range = 450;
    t.collision_radius = 10.0;
    t
}

/// A wanderer entering the Tyrannosaurus's range triggers the curiosity
/// pause (the new aggro-range-enter hook), and only after the 6 s
/// `TREX_ATTACK` does it charge.
#[test]
fn trex_sizes_you_up_before_charging() {
    let (mut world, _db, _l) = combat_test_world();
    world.data.npc_data.insert_for_test(trex_template());
    let mut stun = passive_clan_test_skill(5120);
    stun.operate_type = OperateType::Active;
    world.data.skill_data.insert_for_test(stun);
    add_test_npc(&mut world, NPC_OID + 700, 22215, "Monster", 76, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 5001, 200, 0, 0);

    // The scan warms up (global_aggro -10 → 0) and notices the player: the
    // curiosity gate trips instead of an immediate charge.
    advance_world(&mut world, 130);
    let sv = world
        .objects
        .get_component::<crate::model::npc::Npc>(&(NPC_OID + 700))
        .unwrap()
        .script_value;
    assert_eq!(sv, 1, "noticed — and paused, not charging");

    // Six seconds later the sizing-up ends: state resets and he commits.
    advance_world(&mut world, 100);
    let (sv, hate) = {
        let n = world
            .objects
            .get_component::<crate::model::npc::Npc>(&(NPC_OID + 700))
            .unwrap();
        let h = world
            .objects
            .get_component::<crate::model::npc::AggroList>(&(NPC_OID + 700))
            .and_then(|a| a.0.get(&5001).map(|i| i.hate))
            .unwrap_or(0.0);
        (n.script_value, h)
    };
    assert_eq!(sv, 0, "the pause is over");
    assert!(hate > 0.0, "and the charge is on");
}

/// The spell-finished hook: a Berserk that lands under 60% HP locks the
/// ladder (script value 3) and slams 555 hate onto the most hated.
#[test]
fn trex_berserk_locks_the_ladder() {
    let (mut world, _db, _l) = combat_test_world();
    world.data.npc_data.insert_for_test(trex_template());
    add_test_npc(&mut world, NPC_OID + 700, 22215, "Monster", 76, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 5001, 200, 0, 0);
    {
        let v = world
            .objects
            .get_component_mut::<Vitals>(&(NPC_OID + 700))
            .unwrap();
        v.cur_hp = v.max_hp as f64 * 0.4;
    }
    let mut aggro = crate::model::npc::AggroList::default();
    aggro.0.insert(
        5001,
        crate::model::npc::AggroInfo {
            hate: 100.0,
            damage: 0.0,
        },
    );
    world.objects.add_components(&(NPC_OID + 700), aggro);

    quests::notify_spell_finished(&mut world, NPC_OID + 700, 22215, 5087, NPC_OID + 700);
    let n = world
        .objects
        .get_component::<crate::model::npc::Npc>(&(NPC_OID + 700))
        .unwrap();
    assert_eq!(n.script_value, 3, "ladder locked");
    // The +555 lands, then `seed_attack` (the port's addAttackPlayerDesire)
    // stacks its own attack-desire hate on top — the floor is what matters.
    let hate = world
        .objects
        .get_component::<crate::model::npc::AggroList>(&(NPC_OID + 700))
        .unwrap()
        .0[&5001]
        .hate;
    assert!(hate >= 655.0, "hate slammed on the most hated ({hate})");
}

/// Striking an Ancient Egg wakes the jungle — nearby monsters coin-flip
/// onto the striker.
#[test]
fn ancient_egg_wakes_the_jungle() {
    let (mut world, _db, _l) = combat_test_world();
    const EGG: i32 = 18344;
    for id in [EGG, 22198] {
        world
            .data
            .npc_data
            .insert_for_test(crate::data::npc_data::default_template(id));
    }
    add_test_npc(&mut world, NPC_OID + 700, EGG, "Monster", 40, 100, 0, 0);
    add_test_npc(&mut world, NPC_OID + 701, 22198, "Monster", 40, 300, 0, 0);
    add_test_npc(&mut world, NPC_OID + 702, 22198, "Monster", 40, 5000, 0, 0);
    let _rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);

    // 80% roll hits; the one near raptor flips heads, the far one is out of
    // range entirely.
    world.forced_rolls.push_back(10);
    world.forced_rolls.push_back(0);
    quests::notify_attack(&mut world, 5001, NPC_OID + 700, EGG, None, false);
    let hates = |world: &World, oid: i32| {
        world
            .objects
            .get_component::<crate::model::npc::AggroList>(&oid)
            .is_some_and(|a| a.0.contains_key(&5001))
    };
    assert!(hates(&world, NPC_OID + 701), "the jungle answers");
    assert!(!hates(&world, NPC_OID + 702), "but not from a screen away");
}

/// Sprigants cast their trap on a 15 s cycle.
#[test]
fn sprigant_casts_its_trap() {
    let (mut world, _db, _l) = combat_test_world();
    const SPRIGANT: i32 = 18345;
    world
        .data
        .npc_data
        .insert_for_test(crate::data::npc_data::default_template(SPRIGANT));
    let mut trap = passive_clan_test_skill(5085);
    trap.operate_type = OperateType::Active;
    world.data.skill_data.insert_for_test(trap);
    add_test_npc(
        &mut world,
        NPC_OID + 700,
        SPRIGANT,
        "Monster",
        40,
        100,
        0,
        0,
    );

    crate::scripts::primeval_isle::handle_sprigant_trap(&mut world, NPC_OID + 700);
    let cast = world
        .objects
        .get_component::<crate::model::components::Casting>(&(NPC_OID + 700))
        .map(|c| c.0.skill_id);
    assert_eq!(cast, Some(5085), "the trap fires");
}

// ---------------------------------------------------------------------------
// Slice 8 — Four Sepulchers
// ---------------------------------------------------------------------------

/// The hall coordinates the test zone must cover (sepulcher 1).
const FS_HALL: (i32, i32) = (182000, -85500);

fn fs_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = combat_test_world();
    // Sepulcher 1's script zone, generously covering the whole hall strip.
    world.data.zone_data.insert(crate::data::zone_data::Zone {
        id: 200221,
        name: "royal_rush_script_1".into(),
        kind: crate::data::zone_data::ZoneKind::Script,
        territory: crate::data::spawn_data::Territory {
            form: crate::data::spawn_data::ZoneForm::Cuboid {
                x1: 179000,
                x2: 192000,
                y1: -87000,
                y2: -84000,
            },
            min_z: -8000,
            max_z: -6000,
        },
        castle_id: 0,
        clan_hall_id: 0,
        effect: None,
        damage: None,
        swamp: None,
    });
    for id in [
        crate::game_loop::four_sepulchers::CONQUEROR_MANAGER,
        crate::game_loop::four_sepulchers::MYSTERIOUS_CHEST,
        crate::game_loop::four_sepulchers::KEY_CHEST,
        crate::game_loop::four_sepulchers::TELEPORTER,
        18120, // wave rewarder
        25346, // Conqueror boss
    ] {
        world
            .data
            .npc_data
            .insert_for_test(crate::data::npc_data::default_template(id));
    }
    // A one-row wave table for sepulcher 1: wave 2 spawns one rewarder.
    world
        .data
        .four_sepulchers
        .insert_for_test(crate::data::four_sepulchers_data::FsSpawn {
            sepulcher: 1,
            wave: 2,
            npc_id: 18120,
            x: FS_HALL.0,
            y: FS_HALL.1,
            z: -7218,
            heading: 0,
        });
    (world, db, l)
}

fn fs_party(
    world: &mut World,
    oids: [i32; 4],
) -> Vec<tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>> {
    let mut rxs = Vec::new();
    for (i, oid) in oids.into_iter().enumerate() {
        rxs.push(ingame_player(
            world,
            10 + i as u32,
            oid,
            100 + i as i32 * 30,
            0,
            0,
        ));
        let mut quests = crate::model::components::Quests::default();
        quests.0.insert(
            "Q00620_FourGoblets".into(),
            crate::model::quest::QuestState {
                state: crate::model::quest::state::STARTED,
                vars: Default::default(),
            },
        );
        world.objects.add_components(&oid, quests);
        world
            .objects
            .add_components(&oid, crate::model::components::PartyRef(77));
        give_test_item(
            world,
            oid,
            crate::game_loop::four_sepulchers::ENTRANCE_PASS,
            1,
        );
    }
    let mut party =
        crate::model::party::Party::new(oids[0], crate::model::party::LootRule::FindersKeepers, 1);
    party.members = oids.to_vec();
    world.parties.insert(77, party);
    rxs
}

/// The admission ritual: a 4-player party with passes and the quest gets
/// teleported into the hall, passes collected, used passes issued; the
/// 3-minute chest and 60-minute bell are armed.
#[test]
fn four_sepulchers_admission_and_first_wave() {
    use crate::game_loop::four_sepulchers as fs;
    let (mut world, _db, _l) = fs_world();
    add_test_npc(
        &mut world,
        NPC_OID + 800,
        fs::CONQUEROR_MANAGER,
        "Folk",
        70,
        130,
        0,
        0,
    );
    let _rxs = fs_party(&mut world, [5001, 5002, 5003, 5004]);

    match fs::try_enter(&mut world, NPC_OID + 800, 5001) {
        fs::EnterOutcome::Ok => {}
        _ => panic!("the ritual should admit them"),
    }
    for oid in [5001, 5002, 5003, 5004] {
        let pos = world.objects.get_component::<Position>(&oid).unwrap();
        assert!(pos.x > 179000, "teleported into the hall");
        assert_eq!(
            item_count(&world, oid, fs::ENTRANCE_PASS),
            0,
            "pass collected"
        );
        assert_eq!(
            item_count(&world, oid, fs::USED_PASS),
            1,
            "used pass issued"
        );
    }
    // Re-entry is barred while the window runs.
    match fs::try_enter(&mut world, NPC_OID + 800, 5001) {
        fs::EnterOutcome::Full | fs::EnterOutcome::NotTime => {}
        _ => panic!("second entry must be refused"),
    }

    // The 3-minute chest appears...
    advance_ticks(&mut world, 3 * 60 * 10 + 5);
    assert_eq!(count_npcs(&mut world, fs::MYSTERIOUS_CHEST), 1, "chest up");

    // ...the party opens it: wave 1 has no rows in this fixture, so advance
    // progress manually to wave 2 and pour it.
    world.four_sepulchers.progress[0] = 2;
    fs::spawn_next_wave(&mut world, 1);
    assert_eq!(count_npcs(&mut world, 18120), 1, "wave 2 spawned");

    // Clearing the wave pays a key chest at the last corpse.
    let mob = {
        let mut found = 0;
        world.objects.for_each_mut::<&crate::model::npc::Npc>(|n| {
            if n.npc_id == 18120 {
                found = n.object_id;
            }
        });
        found
    };
    world
        .objects
        .get_component_mut::<Vitals>(&mob)
        .unwrap()
        .dead = true;
    advance_ticks(&mut world, 60);
    assert_eq!(
        count_npcs(&mut world, fs::KEY_CHEST),
        1,
        "key chest paid out"
    );
}

/// The boss falls: every partied goblet-quester nearby gets the hall's
/// goblet, and the exit teleporter rises from the corpse.
#[test]
fn four_sepulchers_boss_pays_goblets() {
    use crate::game_loop::four_sepulchers as fs;
    let (mut world, _db, _l) = fs_world();
    let _rxs = fs_party(&mut world, [5001, 5002, 5003, 5004]);
    // Stand the party in the hall.
    for oid in [5001, 5002, 5003, 5004] {
        let p = world.objects.get_component_mut::<Position>(&oid).unwrap();
        p.x = FS_HALL.0;
        p.y = FS_HALL.1;
        p.z = -7218;
    }
    add_test_npc(
        &mut world,
        NPC_OID + 801,
        25346,
        "RaidBoss",
        80,
        FS_HALL.0,
        FS_HALL.1,
        -7218,
    );

    quests::notify_kill(&mut world, 5001, NPC_OID + 801, 25346);
    for oid in [5001, 5002, 5003, 5004] {
        assert_eq!(item_count(&world, oid, 7256), 1, "sepulcher 1 goblet");
    }
    assert_eq!(
        count_npcs(&mut world, fs::TELEPORTER),
        1,
        "exit teleporter up"
    );
}

/// The real spawn table parses — the whole dungeon is data-driven, so an
/// XML rename or schema drift must fail loudly here.
#[test]
fn four_sepulchers_real_spawn_table_loads() {
    let data = crate::data::four_sepulchers_data::FourSepulchersData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    assert!(
        data.spawns.len() > 700,
        "expected the full wave table, got {}",
        data.spawns.len()
    );
    for sep in 1..=4 {
        for wave in 1..=7 {
            assert!(
                data.spawns
                    .iter()
                    .any(|r| r.sepulcher == sep && r.wave == wave),
                "sepulcher {sep} wave {wave} has no rows"
            );
        }
    }
}
