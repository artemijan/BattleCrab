//! The castle's defences during a siege: doors (targeting, breaching, and
//! the click that does nothing outside a siege), control towers, the
//! artifact, the stationed guards, and the attacker's headquarters flag.

use super::*;

/// Castle doors during a siege: start closes the gates (full HP), a breach
/// (damage to 0) swings a door open, and endSiege revives + closes them. Port
/// of Castle.spawnDoor + the door-breach engine.
#[test]
fn siege_doors_close_on_start_and_breach_on_damage() {
    use crate::data::door_data::DoorOpenMethod;
    use model::door::Door;
    use model::siege::Siege;
    let (mut world, ..) = test_world();
    insert_siege_zone(&mut world, 3, 0, 1000, -1000, 1000); // covers the door at (100, 0)
    world.sieges.insert(3, Siege::new(3));
    let door =
        model::door::spawn_door_for_test(&mut world, test_door(24190001, DoorOpenMethod::None)); // closed, hp 1000
    crate::game_loop::npc::doors::open_door(&mut world, door);
    assert!(world.geo.doors.is_open(24190001), "door starts open");

    // start_siege → the castle gate is closed at full HP.
    crate::game_loop::siege::start_siege(&mut world, 3);
    assert!(!world.geo.doors.is_open(24190001), "siege closes the gate");
    assert_eq!(
        world
            .objects
            .get_component::<Door>(&door)
            .unwrap()
            .current_hp,
        1000,
        "gate at full HP"
    );

    // Breach: damage to 0 → the gate is destroyed and swings open.
    assert!(
        crate::game_loop::siege::damage_door(&mut world, door, 1000),
        "breached this hit"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Door>(&door)
            .unwrap()
            .current_hp,
        0,
        "gate destroyed"
    );
    assert!(
        world.geo.doors.is_open(24190001),
        "breached gate swings open"
    );
    // A second hit on the dead gate does nothing.
    assert!(
        !crate::game_loop::siege::damage_door(&mut world, door, 500),
        "already breached"
    );

    // endSiege → spawnDoor revives the gate to full HP + closes it.
    crate::game_loop::siege::end_siege(&mut world, 3);
    let d = world.objects.get_component::<Door>(&door).unwrap();
    assert_eq!(d.current_hp, 1000, "revived to full HP");
    assert!(!world.geo.doors.is_open(24190001), "and closed");
}

/// DoorAction end to end: click a siege door to target it, then attack it —
/// the swing damages the gate and eventually breaches it (opens). Makes
/// siege::damage_door reachable in-game.
#[test]
fn siege_door_can_be_targeted_and_breached_by_attack() {
    use crate::data::door_data::DoorOpenMethod;
    use model::door::Door;
    use model::siege::Siege;
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    insert_siege_zone(&mut world, 3, 0, 1000, -1000, 1000); // covers the door at (100, 0)
    let mut siege = Siege::new(3);
    siege.in_progress = true;
    world.sieges.insert(3, siege);
    let door =
        model::door::spawn_door_for_test(&mut world, test_door(24190001, DoorOpenMethod::None));
    world
        .objects
        .get_component_mut::<Door>(&door)
        .unwrap()
        .current_hp = 50; // a few swings to breach
    let mut rx = ingame_caster(&mut world, 1, 3001, 80, 0); // within melee reach of the gate

    // Click the door → it becomes the target.
    handle_action(&mut world, 1, &action_body(door, 0));
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(door),
        "door targeted"
    );
    drain(&mut rx);

    // Attack it → the first swing is broadcast; the damage lands at
    // `timeToHit`, like a creature swing.
    handle_attack_request(&mut world, 1, &attack_request_body(door));
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter().any(|p| p[0] == server_packets::opcodes::ATTACK),
        "swing broadcast"
    );
    advance_ticks(&mut world, 20);
    assert!(
        world
            .objects
            .get_component::<Door>(&door)
            .unwrap()
            .current_hp
            < 50,
        "gate took damage at hit time"
    );

    // The attack loop auto-repeats each swing period (no re-clicking) until the
    // gate breaches.
    for _ in 0..40 {
        if world.geo.doors.is_open(24190001) {
            break;
        }
        advance_world(&mut world, 20);
    }
    assert_eq!(
        world
            .objects
            .get_component::<Door>(&door)
            .unwrap()
            .current_hp,
        0,
        "gate destroyed"
    );
    assert!(world.geo.doors.is_open(24190001), "breached gate is open");
}

/// Door chase: attacking a gate from out of melee reach walks the player to it
/// (`AI_INTENTION_ATTACK` → `maybeMoveToPawn`) instead of failing with
/// "out of range", then the auto-repeat swing loop breaches it on arrival.
#[test]
fn siege_door_out_of_reach_chases_and_breaches() {
    use crate::data::door_data::DoorOpenMethod;
    use model::components::space::{Movement, Position};
    use model::door::Door;
    use model::siege::Siege;
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    insert_siege_zone(&mut world, 3, 0, 2000, -1000, 1000); // covers the gate at (100, 0)
    let mut siege = Siege::new(3);
    siege.in_progress = true;
    world.sieges.insert(3, siege);
    let door =
        model::door::spawn_door_for_test(&mut world, test_door(24190001, DoorOpenMethod::None));
    world
        .objects
        .get_component_mut::<Door>(&door)
        .unwrap()
        .current_hp = 50;
    let mut rx = ingame_caster(&mut world, 1, 3001, 900, 0); // well out of reach of the gate

    // Ctrl-attack from out of reach → a chase begins, no out-of-range message.
    handle_action(&mut world, 1, &action_body(door, 0));
    drain(&mut rx);
    handle_attack_request(&mut world, 1, &attack_request_body(door));
    let start_x = world.objects.get_component::<Position>(&3001).unwrap().x;
    assert!(
        world.objects.has_component::<Movement>(&3001),
        "a chase leg starts toward the gate"
    );
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "MoveToPawn broadcast"
    );
    assert!(
        !pkts
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::YOUR_TARGET_IS_OUT_OF_RANGE),
        "no out-of-range message — the player walks instead",
    );

    // Walk in and swing until the gate breaches.
    for _ in 0..80 {
        if world.geo.doors.is_open(24190001) {
            break;
        }
        advance_world(&mut world, 20);
    }
    let end_x = world.objects.get_component::<Position>(&3001).unwrap().x;
    assert!(
        end_x < start_x,
        "the player closed distance to the gate ({start_x} → {end_x})"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Door>(&door)
            .unwrap()
            .current_hp,
        0,
        "gate breached after the chase"
    );
}

/// A plain double-click engages a siege gate: the first `Action` selects it,
/// the second (already targeted, non-shift) starts the swing — the
/// `DoorAction` attack path, not just the Ctrl-forced `AttackRequest`.
#[test]
fn siege_door_second_action_click_starts_attack() {
    use crate::data::door_data::DoorOpenMethod;
    use model::door::Door;
    use model::siege::Siege;
    let (mut world, ..) = combat_test_world();
    insert_siege_zone(&mut world, 3, 0, 1000, -1000, 1000);
    let mut siege = Siege::new(3);
    siege.in_progress = true;
    world.sieges.insert(3, siege);
    let door =
        model::door::spawn_door_for_test(&mut world, test_door(24190001, DoorOpenMethod::None));
    world
        .objects
        .get_component_mut::<Door>(&door)
        .unwrap()
        .current_hp = 5000;
    let mut rx = ingame_caster(&mut world, 1, 3001, 80, 0); // within melee reach

    // First click just selects — no damage.
    handle_action(&mut world, 1, &action_body(door, 0));
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(door),
        "door targeted"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Door>(&door)
            .unwrap()
            .current_hp,
        5000,
        "selecting doesn't damage"
    );
    drain(&mut rx);

    // Second click on the already-targeted gate engages it.
    handle_action(&mut world, 1, &action_body(door, 0));
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter().any(|p| p[0] == server_packets::opcodes::ATTACK),
        "swing broadcast on the second click"
    );
    // The damage lands at `timeToHit`, not at swing start (Java `doAttack`
    // schedules `onHitTimeNotDual`).
    assert_eq!(
        world
            .objects
            .get_component::<Door>(&door)
            .unwrap()
            .current_hp,
        5000,
        "nothing lands at swing start"
    );
    advance_ticks(&mut world, 20);
    assert!(
        world
            .objects
            .get_component::<Door>(&door)
            .unwrap()
            .current_hp
            < 5000,
        "gate took damage at hit time"
    );
}

/// A door is only engageable while its castle is under siege: outside a siege
/// a repeated `Action` click just re-selects it, never attacks.
#[test]
fn door_click_does_not_attack_outside_siege() {
    use crate::data::door_data::DoorOpenMethod;
    use model::door::Door;
    let (mut world, ..) = combat_test_world();
    insert_siege_zone(&mut world, 3, 0, 1000, -1000, 1000); // zone present, but no active siege
    let door =
        model::door::spawn_door_for_test(&mut world, test_door(24190001, DoorOpenMethod::None));
    world
        .objects
        .get_component_mut::<Door>(&door)
        .unwrap()
        .current_hp = 5000;
    let mut rx = ingame_caster(&mut world, 1, 3001, 80, 0);

    handle_action(&mut world, 1, &action_body(door, 0));
    handle_action(&mut world, 1, &action_body(door, 0));
    let pkts = drain(&mut rx);
    assert!(
        !pkts.iter().any(|p| p[0] == server_packets::opcodes::ATTACK),
        "no swing without an active siege"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Door>(&door)
            .unwrap()
            .current_hp,
        5000,
        "gate untouched"
    );
}

/// Touching the throne-room Holy Artifact (an Artefact NPC) as a registered
/// attacker during a siege seizes the castle — the artifact trigger for the
/// capture engine. Port of Artefact.onAction → Castle.setOwner → midVictory.
#[test]
fn siege_artifact_capture_seizes_the_castle_for_the_attacker() {
    use model::castle::{Castle, CastleSide};
    use model::clan::{Clan, ClanMember};
    use model::siege::{Siege, SiegeClanType};
    let (mut world, ..) = test_world();
    insert_siege_zone(&mut world, 3, 0, 1000, -1000, 1000);
    world.castles = vec![Castle {
        show_npc_crest: false,
        id: 3,
        name: "Giran".into(),
        side: CastleSide::Neutral,
        ticket_buy_count: 0,
        first_mid_victory: false,
        time_registration_over: true,
        siege_time_registration_end: 0,
        siege_date: 0,
        treasury: 0,
    }];
    let mut siege = Siege::new(3);
    siege.in_progress = true;
    siege.add_clan(700, SiegeClanType::Attacker);
    world.sieges.insert(3, siege);
    world.clans.insert(
        700,
        Clan {
            id: 700,
            name: "Attackers".into(),
            leader_id: 8003,
            level: 5,
            reputation_score: 0,
            castle_id: 0,
            members: vec![ClanMember {
                char_id: 8003,
                name: "P8003".into(),
                level: 40,
                class_id: 0,
                sex: 0,
                race: 0,
                power_grade: 5,
                title: String::new(),
                pledge_type: 0,
                apprentice: 0,
                sponsor: 0,
            }],
            skills: Default::default(),
            warehouse: Default::default(),
            char_penalty_expiry_time: 0,
            dissolving_expiry_time: 0,
            rank_privs: Default::default(),
            new_leader_id: 0,
            sub_pledges: Default::default(),
            ally_id: 0,
            ally_name: String::new(),
            ally_penalty_expiry_time: 0,
            ally_penalty_type: 0,
            crest_id: 0,
            crest_large_id: 0,
            ally_crest_id: 0,
            blood_alliance_count: 0,
        },
    );
    // The Giran Holy Artifact (type Artefact) at (100, 0) inside the siege zone.
    add_test_npc(&mut world, NPC_OID + 20, 35147, "Artefact", 20, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 8003, 90, 0, 0); // attacker clan member, next to it
    world
        .objects
        .get_component_mut::<Player>(&8003)
        .unwrap()
        .clan_id = 700;

    // Touch the artifact → the attacker seizes the castle.
    interact_with_npc(&mut world, 1, 8003, NPC_OID + 20);
    assert_eq!(world.clans[&700].castle_id, 3, "attacker seized the castle");
    assert_eq!(
        world.sieges[&3]
            .clans
            .iter()
            .find(|c| c.clan_id == 700)
            .map(|c| c.kind),
        Some(SiegeClanType::Owner),
        "captor becomes the owner side"
    );
}

/// Control towers are attackable during a siege; destroying one decrements the
/// siege's control-tower count (Java ControlTower.onDeath → Siege.killedCT).
#[test]
fn siege_control_tower_destruction_decrements_the_count() {
    use crate::game_loop::npc;
    use model::siege::{Siege, SiegeSpawn};
    let (mut world, ..) = test_world();
    insert_siege_zone(&mut world, 3, 0, 1000, -1000, 1000);
    world.sieges.insert(3, Siege::new(3));
    // A control-tower template (type ControlTower) + its spawn point in the zone.
    let mut t = crate::data::npc_data::default_template(13002);
    t.type_name = "ControlTower".into();
    t.base_hp_max = 100.0;
    world.data.npc_data.insert_for_test(t);
    world.data.siege_towers.insert(
        3,
        vec![SiegeSpawn {
            npc_id: 13002,
            x: 100,
            y: 0,
            z: 0,
            heading: 0,
        }],
    );

    crate::game_loop::siege::start_siege(&mut world, 3);
    assert_eq!(
        world.sieges[&3].control_tower_count, 1,
        "one control tower counted at spawn"
    );
    let tower = *world.sieges[&3].spawned_npcs.last().expect("tower spawned");
    assert!(
        crate::game_loop::siege::attackable_siege_tower(&world, tower),
        "attackable during the siege"
    );

    // Destroy it → the count drops.
    npc::npc_do_die(&mut world, tower, 0);
    assert_eq!(
        world.sieges[&3].control_tower_count, 0,
        "destruction decremented the count"
    );
}

/// The clan leader of an attacker plants an HQ flag; it becomes attackable (a
/// defender can destroy it) and the attacker's "to siege HQ" respawn point.
/// Java `HeadquarterCreate` + `Siege.getFlag`/`killedFlag`.
#[test]
fn siege_attacker_hq_flag_is_respawn_point_and_destructible() {
    use crate::game_loop::npc;
    use model::clan::{Clan, ClanMember};
    use model::siege::{Siege, SiegeClanType};
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    insert_siege_zone(&mut world, 3, -1000, 1000, -1000, 1000);
    // `BuildCampSkillCondition`'s HQ patch — the camp may only go up in one.
    insert_hq_zone(&mut world, 3, -1000, 1000, -1000, 1000);
    // The HQ flag NPC (35062) template.
    let mut t = crate::data::npc_data::default_template(35062);
    t.type_name = "Folk".into();
    t.base_hp_max = 100.0;
    world.data.npc_data.insert_for_test(t);
    // Attacker clan 700, its leader 3001.
    world.clans.insert(
        700,
        Clan {
            id: 700,
            name: "Attackers".into(),
            leader_id: 3001,
            level: 5,
            reputation_score: 0,
            castle_id: 0,
            members: vec![ClanMember {
                char_id: 3001,
                name: "P3001".into(),
                level: 40,
                class_id: 0,
                sex: 0,
                race: 0,
                power_grade: 5,
                title: String::new(),
                pledge_type: 0,
                apprentice: 0,
                sponsor: 0,
            }],
            skills: Default::default(),
            warehouse: Default::default(),
            char_penalty_expiry_time: 0,
            dissolving_expiry_time: 0,
            rank_privs: Default::default(),
            new_leader_id: 0,
            sub_pledges: Default::default(),
            ally_id: 0,
            ally_name: String::new(),
            ally_penalty_expiry_time: 0,
            ally_penalty_type: 0,
            crest_id: 0,
            crest_large_id: 0,
            ally_crest_id: 0,
            blood_alliance_count: 0,
        },
    );
    let mut siege = Siege::new(3);
    siege.in_progress = true;
    siege.add_clan(700, SiegeClanType::Attacker);
    world.sieges.insert(3, siege);

    let _rx = ingame_caster(&mut world, 1, 3001, 40, 50);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_id = 700;

    // Leader plants the flag (HeadquarterCreate).
    assert!(
        crate::game_loop::siege::place_siege_flag(&mut world, 3001, false),
        "leader plants the HQ"
    );
    let flag = world.sieges[&3].flag_of(700).expect("flag registered");
    assert_eq!(world.sieges[&3].flag_count(700), 1);
    // A second flag is refused (MaxFlags = 1).
    assert!(
        !crate::game_loop::siege::place_siege_flag(&mut world, 3001, false),
        "flag cap enforced"
    );
    assert!(
        crate::game_loop::siege::attackable_siege_flag(&world, flag),
        "flag is attackable"
    );

    // The attacker respawns at the flag on "to siege HQ" (type 4).
    world
        .objects
        .get_component_mut::<Vitals>(&3001)
        .unwrap()
        .dead = true;
    let flag_pos = *world.objects.get_component::<Position>(&flag).unwrap();
    handle_request_restart_point(&mut world, 1, &restart_to(4));
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!(
        (pos.x, pos.y),
        (flag_pos.x, flag_pos.y),
        "attacker respawns at the HQ flag"
    );

    // A defender destroys the flag → it stops being a respawn point.
    npc::npc_do_die(&mut world, flag, 0);
    assert_eq!(world.sieges[&3].flag_of(700), None, "killed flag removed");
    assert!(!crate::game_loop::siege::attackable_siege_flag(
        &world, flag
    ));
}

/// Register a stationed siege guard (`Defender`, npc 35085) in a running siege
/// for castle 3, plus an attacker clan (700, owns no castle) whose member is
/// `player_oid`. Returns the guard oid.
fn setup_siege_with_guard(world: &mut World, guard_oid: i32, gx: i32, gy: i32) {
    use model::siege::Siege;
    insert_siege_zone(world, 3, -2000, 2000, -2000, 2000);
    world.sieges.insert(3, {
        let mut s = Siege::new(3);
        s.in_progress = true;
        s
    });
    let mut t = crate::data::npc_data::default_template(35085);
    t.type_name = "Defender".into();
    t.aggro_range = 1000;
    t.base_hp_max = 500.0;
    t.base_p_atk = 50.0;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(world, guard_oid, 35085, "Defender", 75, gx, gy, 0);
}

fn attacker_clan(world: &mut World, player_oid: i32) {
    use model::clan::{Clan, ClanMember};
    world.clans.insert(
        700,
        Clan {
            id: 700,
            name: "Attackers".into(),
            leader_id: player_oid,
            level: 5,
            reputation_score: 0,
            castle_id: 0,
            members: vec![ClanMember {
                char_id: player_oid,
                name: "P".into(),
                level: 40,
                class_id: 0,
                sex: 0,
                race: 0,
                power_grade: 5,
                title: String::new(),
                pledge_type: 0,
                apprentice: 0,
                sponsor: 0,
            }],
            skills: Default::default(),
            warehouse: Default::default(),
            char_penalty_expiry_time: 0,
            dissolving_expiry_time: 0,
            rank_privs: Default::default(),
            new_leader_id: 0,
            sub_pledges: Default::default(),
            ally_id: 0,
            ally_name: String::new(),
            ally_penalty_expiry_time: 0,
            ally_penalty_type: 0,
            crest_id: 0,
            crest_large_id: 0,
            ally_crest_id: 0,
            blood_alliance_count: 0,
        },
    );
    world
        .objects
        .get_component_mut::<Player>(&player_oid)
        .unwrap()
        .clan_id = 700;
}

/// A stationed guard is attackable by an attacker (no Ctrl) but not by a
/// defender, and clicking it starts an attack instead of a chat window (Java
/// `Defender.isAutoAttackable` / `onAction`).
#[test]
fn siege_guard_attackable_by_attacker_not_defender() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let guard = NPC_OID + 40;
    setup_siege_with_guard(&mut world, guard, 40, 0);
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    attacker_clan(&mut world, 3001);

    assert!(
        crate::game_loop::siege::attackable_siege_guard(&world, guard, 3001),
        "guard attackable by an attacker"
    );
    // If that clan instead owns the castle it is a defender → not attackable.
    world.clans.get_mut(&700).unwrap().castle_id = 3;
    assert!(
        !crate::game_loop::siege::attackable_siege_guard(&world, guard, 3001),
        "guard not attackable by a defender"
    );
    world.clans.get_mut(&700).unwrap().castle_id = 0;

    // Clicking the already-targeted guard attacks it (not a menu).
    set_target(&mut world, 1, 3001, Some(guard));
    interact_with_npc(&mut world, 1, 3001, guard);
    assert!(
        matches!(
            world.objects.get_component::<Intent>(&3001),
            Some(Intent(model::PlayerIntent::Attack { .. }))
        ),
        "click starts an attack on the guard"
    );
}

/// A guard defends the castle: it aggros an intruding attacker within its aggro
/// range and switches to the attack intent (Java `SiegeGuardAI` aggro scan).
#[test]
fn siege_guard_aggros_intruding_attacker() {
    use model::npc::{AggroList, NpcAi, NpcIntention};
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let guard = NPC_OID + 41;
    setup_siege_with_guard(&mut world, guard, 120, 0);
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0); // attacker, in aggro range
    attacker_clan(&mut world, 3001);
    // Skip the spawn-calm so a single think acts.
    world
        .objects
        .get_component_mut::<NpcAi>(&guard)
        .unwrap()
        .global_aggro = 0;

    ai::npc_ai_tick(&mut world);

    assert!(
        world
            .objects
            .get_component::<AggroList>(&guard)
            .unwrap()
            .0
            .contains_key(&3001),
        "the attacker entered the guard's aggro list"
    );
    assert_eq!(
        world
            .objects
            .get_component::<NpcAi>(&guard)
            .unwrap()
            .intention,
        NpcIntention::Attack,
        "guard locks on to defend the castle"
    );
}

/// **An advanced HQ takes half damage** — skill 326's flag versus skill 247's.
///
/// Java's `SiegeFlagStatus.reduceHp` omits an `else`, so upstream the advanced
/// camp takes `value/2 + value` — 1.5x, making the noble-only skill *worse*
/// than the basic one. This port halves instead, a deliberate deviation
/// recorded in `docs/CUSTOM_DIST_DEVIATIONS.md`; the test pins the intended
/// behaviour so a future "fix toward Java" has to argue with it.
#[test]
fn an_advanced_headquarters_takes_half_damage() {
    use model::components::player::AdvancedHeadquarter;
    use model::components::stats::Vitals;

    let hp_after_hit = |advanced: bool| -> f64 {
        let (mut world, ..) = test_world();
        let flag = NPC_OID;
        add_test_npc(&mut world, flag, 35062, "Monster", 60, 100, 100, 0);
        {
            let v = world.objects.get_component_mut::<Vitals>(&flag).unwrap();
            v.max_hp = 1000;
            v.cur_hp = 1000.0;
        }
        if advanced {
            world.objects.add_components(&flag, AdvancedHeadquarter);
        }
        let _rx = ingame_player(&mut world, 1, 3001, 100, 100, 0);
        combat::npc_receive_damage(&mut world, flag, 3001, 100.0, false);
        world
            .objects
            .get_component::<Vitals>(&flag)
            .map(|v| v.cur_hp)
            .unwrap_or(0.0)
    };

    assert_eq!(hp_after_hit(false), 900.0, "a basic camp takes it all");
    assert_eq!(
        hp_after_hit(true),
        950.0,
        "an advanced camp takes half — not 1.5x, which is Java's missing-else bug"
    );
}
