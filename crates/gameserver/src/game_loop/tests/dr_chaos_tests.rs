//! Dr. Chaos — the paranoia transformation into the Gigantic Chaos Golem.

use super::*;

use crate::game_loop::dr_chaos::{self, CHAOS_GOLEM, CRAZY, DEAD, DOCTOR_CHAOS, NORMAL};
use crate::game_loop::grand_boss::find_spawned;
use crate::model::components::{DrChaosGolem, DrChaosState};

const CID: u32 = 1;
const PLAYER: i32 = 9970;

fn chaos_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    for (id, kind) in [(DOCTOR_CHAOS, "Folk"), (CHAOS_GOLEM, "GrandBoss")] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = kind.into();
        t.level = 70;
        t.base_hp_max = 100_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    world.grand_bosses.insert(
        CHAOS_GOLEM,
        model::grand_boss::GrandBoss {
            boss_id: CHAOS_GOLEM,
            loc_x: 96_080,
            loc_y: -110_822,
            loc_z: -3_343,
            heading: 0,
            respawn_time: 0,
            current_hp: 0.0,
            current_mp: 0.0,
            status: NORMAL,
        },
    );
    (world, db, l)
}

/// Boot: NORMAL spawns Dr. Chaos (paranoia armed); CRAZY spawns the golem;
/// DEAD with an elapsed window brings Dr. Chaos back.
#[test]
fn boot_resolves_each_status() {
    // NORMAL → Dr. Chaos.
    let (mut world, _db, _l) = chaos_world();
    dr_chaos::resolve_at_boot(&mut world);
    let dc = find_spawned(&world, DOCTOR_CHAOS).expect("Dr. Chaos stands");
    assert!(
        world.objects.get_component::<DrChaosState>(&dc).is_some(),
        "paranoia armed"
    );
    assert!(find_spawned(&world, CHAOS_GOLEM).is_none(), "no golem yet");

    // CRAZY → the golem.
    let (mut world, _db, _l) = chaos_world();
    world.grand_bosses.get_mut(&CHAOS_GOLEM).unwrap().status = CRAZY;
    dr_chaos::resolve_at_boot(&mut world);
    assert!(
        find_spawned(&world, CHAOS_GOLEM).is_some(),
        "golem restored"
    );
    assert!(
        find_spawned(&world, DOCTOR_CHAOS).is_none(),
        "Dr. Chaos not up while the golem is"
    );

    // DEAD, window already elapsed → Dr. Chaos returns now.
    let (mut world, _db, _l) = chaos_world();
    {
        let b = world.grand_bosses.get_mut(&CHAOS_GOLEM).unwrap();
        b.status = DEAD;
        b.respawn_time = 1; // in the past
    }
    dr_chaos::resolve_at_boot(&mut world);
    assert_eq!(dr_chaos_status(&world), NORMAL, "reset to NORMAL");
    assert!(
        find_spawned(&world, DOCTOR_CHAOS).is_some(),
        "Dr. Chaos back after downtime"
    );
}

fn dr_chaos_status(world: &World) -> i32 {
    world.grand_bosses.get(&CHAOS_GOLEM).unwrap().status
}

/// A player lingering in range drains the timer; at ≤0 Dr. Chaos becomes the
/// golem (status CRAZY, Dr. Chaos gone).
#[test]
fn a_lingering_player_triggers_the_transformation() {
    let (mut world, _db, _l) = chaos_world();
    dr_chaos::resolve_at_boot(&mut world);
    let dc = find_spawned(&world, DOCTOR_CHAOS).unwrap();
    let dc_pos = world
        .objects
        .get_component::<Position>(&dc)
        .copied()
        .unwrap();
    let _rx = ingame_caster(&mut world, CID, PLAYER, dc_pos.x + 100, dc_pos.y);
    // Wind the timer down to 1 so one tick tips him over.
    world
        .objects
        .get_component_mut::<DrChaosState>(&dc)
        .unwrap()
        .pissed_off = 1;

    dr_chaos::handle_paranoia(&mut world, dc);

    assert_eq!(dr_chaos_status(&world), CRAZY, "transformed");
    // Dr. Chaos lingers through the 17 s cinematic (Java deletes him on beat
    // 5); the golem replaces him only when the beats run out.
    assert!(
        find_spawned(&world, DOCTOR_CHAOS).is_some(),
        "still on-screen mid-cinematic"
    );
    assert!(
        find_spawned(&world, CHAOS_GOLEM).is_none(),
        "golem not up yet"
    );
    advance_ticks(&mut world, 18 * 10);
    assert!(
        find_spawned(&world, DOCTOR_CHAOS).is_none(),
        "Dr. Chaos gone after beat 5"
    );
    assert!(
        find_spawned(&world, CHAOS_GOLEM).is_some(),
        "the golem stands after the cinematic"
    );
}

/// No nearby player → no drain, no transform (he stays paranoid but calm).
#[test]
fn nobody_near_leaves_the_timer_alone() {
    let (mut world, _db, _l) = chaos_world();
    dr_chaos::resolve_at_boot(&mut world);
    let dc = find_spawned(&world, DOCTOR_CHAOS).unwrap();
    world
        .objects
        .get_component_mut::<DrChaosState>(&dc)
        .unwrap()
        .pissed_off = 5;

    dr_chaos::handle_paranoia(&mut world, dc);

    assert_eq!(
        world
            .objects
            .get_component::<DrChaosState>(&dc)
            .unwrap()
            .pissed_off,
        5,
        "no drain"
    );
    assert_eq!(dr_chaos_status(&world), NORMAL, "still Dr. Chaos");
}

/// A dead player standing on top of Dr. Chaos does not drain him.
#[test]
fn a_dead_player_does_not_drain() {
    let (mut world, _db, _l) = chaos_world();
    dr_chaos::resolve_at_boot(&mut world);
    let dc = find_spawned(&world, DOCTOR_CHAOS).unwrap();
    let dc_pos = world
        .objects
        .get_component::<Position>(&dc)
        .copied()
        .unwrap();
    let _rx = ingame_caster(&mut world, CID, PLAYER, dc_pos.x, dc_pos.y);
    world
        .objects
        .get_component_mut::<Vitals>(&PLAYER)
        .unwrap()
        .dead = true;
    world
        .objects
        .get_component_mut::<DrChaosState>(&dc)
        .unwrap()
        .pissed_off = 5;

    dr_chaos::handle_paranoia(&mut world, dc);
    assert_eq!(
        world
            .objects
            .get_component::<DrChaosState>(&dc)
            .unwrap()
            .pissed_off,
        5,
        "the corpse doesn't count"
    );
}

/// Talking drains 1–5 and, once the timer is spent, transforms.
#[test]
fn talking_drains_and_eventually_transforms() {
    let (mut world, _db, _l) = chaos_world();
    dr_chaos::resolve_at_boot(&mut world);
    let dc = find_spawned(&world, DOCTOR_CHAOS).unwrap();
    let before = world
        .objects
        .get_component::<DrChaosState>(&dc)
        .unwrap()
        .pissed_off;

    let html = dr_chaos::on_first_talk(&mut world, dc);
    assert!(html.is_some(), "a paranoid line while he's still calm");
    let after = world
        .objects
        .get_component::<DrChaosState>(&dc)
        .unwrap()
        .pissed_off;
    assert!(
        (before - after) >= 1 && (before - after) <= 5,
        "drained 1-5: {before} -> {after}"
    );

    // Spend the rest → transform on the talk itself.
    world
        .objects
        .get_component_mut::<DrChaosState>(&dc)
        .unwrap()
        .pissed_off = 1;
    let html = dr_chaos::on_first_talk(&mut world, dc);
    assert!(html.is_none(), "no html — he transforms");
    assert_eq!(dr_chaos_status(&world), CRAZY, "transformed on the talk");
}

/// Idle despawn: 30 minutes with no attack reverts to Dr. Chaos; an attack
/// inside the window refreshes the clock and keeps the golem.
#[test]
fn the_golem_despawns_after_idle_but_an_attack_refreshes_it() {
    let (mut world, _db, _l) = chaos_world();
    world.grand_bosses.get_mut(&CHAOS_GOLEM).unwrap().status = CRAZY;
    dr_chaos::resolve_at_boot(&mut world);
    let golem = find_spawned(&world, CHAOS_GOLEM).unwrap();

    // 29 minutes idle, then an attack — the despawn check must NOT fire.
    world
        .objects
        .get_component_mut::<DrChaosGolem>(&golem)
        .unwrap()
        .last_attack_tick = world.tick;
    world.tick += 29 * 60 * 10;
    dr_chaos::on_golem_attacked(&mut world, golem);
    dr_chaos::handle_golem_despawn(&mut world, golem);
    assert!(
        find_spawned(&world, CHAOS_GOLEM).is_some(),
        "the attack refreshed the clock"
    );
    assert_eq!(dr_chaos_status(&world), CRAZY);

    // Now 31 idle minutes → revert.
    world.tick += 31 * 60 * 10;
    dr_chaos::handle_golem_despawn(&mut world, golem);
    assert!(
        find_spawned(&world, CHAOS_GOLEM).is_none(),
        "idle golem despawned"
    );
    assert_eq!(dr_chaos_status(&world), NORMAL, "back to Dr. Chaos");
    assert!(find_spawned(&world, DOCTOR_CHAOS).is_some());
}

/// Killing the golem sets DEAD + a reset window; the reset respawns Dr. Chaos.
#[test]
fn killing_the_golem_arms_a_reset_that_brings_dr_chaos_back() {
    let (mut world, _db, _l) = chaos_world();
    world.grand_bosses.get_mut(&CHAOS_GOLEM).unwrap().status = CRAZY;
    dr_chaos::resolve_at_boot(&mut world);
    let golem = find_spawned(&world, CHAOS_GOLEM).unwrap();

    dr_chaos::on_golem_killed(&mut world, golem);
    assert_eq!(dr_chaos_status(&world), DEAD, "dead");
    assert!(
        world.grand_bosses.get(&CHAOS_GOLEM).unwrap().respawn_time > commons::util::now_millis(),
        "window set"
    );

    // The reset fires (elapsed) → Dr. Chaos at NORMAL.
    world
        .grand_bosses
        .get_mut(&CHAOS_GOLEM)
        .unwrap()
        .respawn_time = 1;
    dr_chaos::handle_reset(&mut world);
    assert_eq!(dr_chaos_status(&world), NORMAL);
    assert!(
        find_spawned(&world, DOCTOR_CHAOS).is_some(),
        "Dr. Chaos returns"
    );
}

/// The kill runs through the real death path (the slice-20 lesson: a direct
/// `on_golem_killed` call passes even if the `death.rs` hook is unwired).
#[test]
fn killing_the_golem_through_the_death_path_marks_it_dead() {
    let (mut world, _db, _l) = chaos_world();
    world.grand_bosses.get_mut(&CHAOS_GOLEM).unwrap().status = CRAZY;
    dr_chaos::resolve_at_boot(&mut world);
    let golem = find_spawned(&world, CHAOS_GOLEM).unwrap();

    crate::game_loop::npc::npc_do_die(&mut world, golem, 0);

    assert_eq!(
        dr_chaos_status(&world),
        DEAD,
        "the death hook fired through npc_do_die"
    );
    assert!(
        world.grand_bosses.get(&CHAOS_GOLEM).unwrap().respawn_time > commons::util::now_millis()
    );
}

/// The literal-text NpcSay writes `-1` then the UTF-16 string (the
/// `broadcastSay(type, String)` variant DrChaos's barks use).
#[test]
fn npc_say_text_writes_the_literal_string() {
    let pkt = server_packets::npc_say_text(500, DOCTOR_CHAOS, "Fools!");
    // opcode, objId(4), chat(4), npcId+1000000(4), then -1(4), then string.
    let npc_string = i32::from_le_bytes([pkt[13], pkt[14], pkt[15], pkt[16]]);
    assert_eq!(npc_string, -1, "literal-string marker");
    let text: Vec<u8> = "Fools!"
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    assert!(
        pkt.windows(text.len()).any(|w| w == text),
        "the line is on the wire"
    );
}
