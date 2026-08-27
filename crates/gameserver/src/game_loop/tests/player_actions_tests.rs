//! `RequestActionUse` — the `ActionData.xml` handler table and the two
//! handlers this slice ported, `SocialAction` and `RunWalk`.
//!
//! The bug these guard against is a *silence*: the dispatch used to be an
//! allow-list of seven ids, so an emote or a `/walk` produced no packet, no
//! message and no log line. Every assertion here is therefore about something
//! reaching the wire — an emote that broadcasts nothing looks exactly like the
//! old behaviour.

use super::*;
use crate::data::action_data::ActionData;
use crate::game_loop::dispatch::on_packet;
use crate::game_loop::player_actions;
use crate::model::components::Speeds;
use crate::network::server_packets::opcodes;

const DIST: &str = crate::data::DIST_GAME;

/// `ActionData.xml` id 12 → `SocialAction` option 2 (`/socialhello`).
const ACTION_HELLO: i32 = 12;
const SOCIAL_HELLO: i32 = 2;
/// `ActionData.xml` id 1 → `RunWalk`.
const ACTION_RUN_WALK: i32 = 1;

fn press(world: &mut World, client_id: u32, action_id: i32) {
    on_packet(
        world,
        client_id,
        [vec![cop::REQUEST_ACTION_USE], action_use_body(action_id)].concat(),
    );
}

/// The social id a `SocialAction` packet carries, if that is what this is.
fn social_id_of(packet: &[u8]) -> Option<i32> {
    (packet[0] == opcodes::SOCIAL_ACTION).then(|| {
        let mut r = commons::network::PacketReader::new(&packet[1..]);
        let _object_id = r.read_i32().unwrap();
        r.read_i32().unwrap()
    })
}

// ---------------------------------------------------------------------------
// SocialAction
// ---------------------------------------------------------------------------

/// An emote reaches **both** the actor and the bystanders — Java's
/// `broadcastPacket(packet)` defaults to `includeSelf = true`, so a player who
/// bows sees themselves bow.
#[test]
fn an_emote_broadcasts_to_the_actor_and_the_bystanders() {
    let (mut world, ..) = test_world();
    world
        .data
        .action_data
        .insert_row_for_test(ACTION_HELLO, "SocialAction", SOCIAL_HELLO);
    let mut actor_rx = ingame_player(&mut world, 1, 7001, 0, 0, 0);
    let mut bystander_rx = ingame_player(&mut world, 2, 7002, 100, 0, 0);
    drain(&mut actor_rx);
    drain(&mut bystander_rx);

    press(&mut world, 1, ACTION_HELLO);

    assert_eq!(
        drain(&mut actor_rx).iter().find_map(|p| social_id_of(p)),
        Some(SOCIAL_HELLO),
        "the actor sees their own emote"
    );
    assert_eq!(
        drain(&mut bystander_rx)
            .iter()
            .find_map(|p| social_id_of(p)),
        Some(SOCIAL_HELLO),
        "and so does everyone nearby"
    );
}

/// `Player.canMakeSocialAction()`'s IDLE clause. A player who is walking is not
/// idle, and Java refuses the emote outright — silently, which is why this
/// asserts on the absence of the packet rather than on a refusal message.
#[test]
fn an_emote_is_refused_while_the_player_is_moving() {
    let (mut world, ..) = test_world();
    world
        .data
        .action_data
        .insert_row_for_test(ACTION_HELLO, "SocialAction", SOCIAL_HELLO);
    let mut rx = ingame_player(&mut world, 1, 7003, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Speeds>(&7003)
        .unwrap()
        .run_spd = 100.0;
    world
        .objects
        .get_component_mut::<Speeds>(&7003)
        .unwrap()
        .running = true;
    handle_move_backward_to_location(&mut world, 1, &move_body((1000, 0, 0), (0, 0, 0), 1));
    assert!(
        world.objects.has_component::<Movement>(&7003),
        "the fixture has to actually be moving or the test proves nothing"
    );
    drain(&mut rx);

    press(&mut world, 1, ACTION_HELLO);

    assert!(
        drain(&mut rx).iter().all(|p| social_id_of(p).is_none()),
        "a moving player does not emote"
    );
}

/// A dead player is refused before the handler is even reached
/// (`RequestActionUse.runImpl`'s first guard), and gets `ActionFailed` for it —
/// the old dispatcher returned without that packet.
#[test]
fn a_dead_player_gets_action_failed_rather_than_silence() {
    let (mut world, ..) = test_world();
    world
        .data
        .action_data
        .insert_row_for_test(ACTION_HELLO, "SocialAction", SOCIAL_HELLO);
    let mut rx = ingame_player(&mut world, 1, 7004, 0, 0, 0);
    {
        let v = world.objects.get_component_mut::<Vitals>(&7004).unwrap();
        v.cur_hp = 0.0;
        v.dead = true;
    }
    drain(&mut rx);

    press(&mut world, 1, ACTION_HELLO);

    let packets = drain(&mut rx);
    assert!(
        packets.iter().any(|p| p[0] == opcodes::ACTION_FAIL),
        "Java answers the guard with ActionFailed"
    );
    assert!(
        packets.iter().all(|p| social_id_of(p).is_none()),
        "and no emote plays"
    );
}

// ---------------------------------------------------------------------------
// RunWalk
// ---------------------------------------------------------------------------

/// `RunWalk` toggles the gait, and `Creature.setRunning` broadcasts
/// `ChangeMoveType` so the client animates the change. Players spawn running,
/// so the first press walks them.
#[test]
fn run_walk_toggles_the_gait_and_broadcasts_it() {
    let (mut world, ..) = test_world();
    world
        .data
        .action_data
        .insert_row_for_test(ACTION_RUN_WALK, "RunWalk", 0);
    let mut rx = ingame_player(&mut world, 1, 7005, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Speeds>(&7005)
        .unwrap()
        .run_spd = 120.0;
    let running = |w: &World| w.objects.get_component::<Speeds>(&7005).unwrap().running;
    assert!(
        running(&world),
        "players spawn running (Creature._isRunning)"
    );
    drain(&mut rx);

    press(&mut world, 1, ACTION_RUN_WALK);
    assert!(!running(&world), "the first press walks");
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == opcodes::CHANGE_MOVE_TYPE),
        "and the client is told"
    );

    press(&mut world, 1, ACTION_RUN_WALK);
    assert!(running(&world), "the second press runs again");
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == opcodes::CHANGE_MOVE_TYPE),
        "and is told again"
    );
}

/// Walking has to actually reach the movement maths, or the toggle is a
/// cosmetic packet: `Speeds::move_speed` picks the walk figure off the same
/// flag, which is why no stat recalculation is needed.
#[test]
fn walking_slows_the_move_speed_the_movement_maths_reads() {
    let (mut world, ..) = test_world();
    world
        .data
        .action_data
        .insert_row_for_test(ACTION_RUN_WALK, "RunWalk", 0);
    let _rx = ingame_player(&mut world, 1, 7006, 0, 0, 0);
    {
        let s = world.objects.get_component_mut::<Speeds>(&7006).unwrap();
        s.run_spd = 120.0;
        s.walk_spd = 50.0;
    }
    let speed = |w: &World| {
        w.objects
            .get_component::<Speeds>(&7006)
            .unwrap()
            .move_speed()
    };
    assert_eq!(speed(&world), 120.0);

    press(&mut world, 1, ACTION_RUN_WALK);

    assert_eq!(speed(&world), 50.0, "the walk speed is what moves them now");
}

// ---------------------------------------------------------------------------
// The table itself
// ---------------------------------------------------------------------------

/// The dispatch is only as good as the table behind it, and a fixture cannot
/// catch a parse regression — so read the shipped file. These are the rows the
/// two ported handlers stand on.
#[test]
fn the_real_action_data_binds_the_emote_and_gait_rows() {
    let data = ActionData::load_from(DIST);

    let run_walk = data.row(ACTION_RUN_WALK).expect("id 1 ships");
    assert_eq!(run_walk.handler, "RunWalk");

    let hello = data.row(ACTION_HELLO).expect("id 12 ships");
    assert_eq!(hello.handler, "SocialAction");
    assert_eq!(hello.option, SOCIAL_HELLO, "/socialhello");

    // All 20 emote rows are present, and every option is one the handler has an
    // arm for — the three couple socials included, which are matched and
    // deliberately dropped rather than left to the unknown-option warn.
    let socials: Vec<i32> = data
        .action_ids()
        .iter()
        .filter_map(|id| data.row(*id))
        .filter(|r| r.handler == "SocialAction")
        .map(|r| r.option)
        .collect();
    assert_eq!(socials.len(), 20, "the dist's emote rows");
    assert!(
        socials.iter().all(|o| matches!(o, 2..=18 | 28 | 29 | 30)),
        "an option outside the handler's arms would warn instead of playing: {socials:?}"
    );

    // And the row `SitStand` has always been reached by — proof the table
    // carries the ids the old allow-list hard-coded.
    assert_eq!(
        data.row(player_actions::action::SIT_STAND)
            .map(|r| r.handler.as_str()),
        Some("SitStand")
    );
}

// ---------------------------------------------------------------------------
// TacticalSignUse / TacticalSignTarget
// ---------------------------------------------------------------------------

/// `ActionData.xml` ids 78..=81 → `TacticalSignUse` options 1..=4, and 82..=85
/// → `TacticalSignTarget`. Only the first of each family is exercised here;
/// the shipped-file test below checks all eight rows bind.
const ACTION_SIGN_1: i32 = 78;
const ACTION_SIGN_2: i32 = 79;
const ACTION_TARGET_SIGN_1: i32 = 82;

/// Register the rows the tactical tests press, mirroring the dist.
fn insert_tactical_rows(world: &mut World) {
    world
        .data
        .action_data
        .insert_row_for_test(ACTION_SIGN_1, "TacticalSignUse", 1);
    world
        .data
        .action_data
        .insert_row_for_test(ACTION_SIGN_2, "TacticalSignUse", 2);
    world
        .data
        .action_data
        .insert_row_for_test(ACTION_TARGET_SIGN_1, "TacticalSignTarget", 1);
}

/// The `(target, token)` pair of an `ExTacticalSign` (0xFE:0x100), if that is
/// what this packet is.
fn tactical_sign_of(pkt: &[u8]) -> Option<(i32, i32)> {
    (pkt[0] == opcodes::EX
        && pkt.len() >= 11
        && i16::from_le_bytes([pkt[1], pkt[2]]) == opcodes::EX_TACTICAL_SIGN)
        .then(|| {
            let mut r = commons::network::PacketReader::new(&pkt[3..]);
            (r.read_i32().unwrap(), r.read_i32().unwrap())
        })
}

fn tactical_signs_in(pkts: &[Vec<u8>]) -> Vec<(i32, i32)> {
    pkts.iter().filter_map(|p| tactical_sign_of(p)).collect()
}

fn signs_of(world: &World, party_id: u32) -> Vec<(i32, i32)> {
    world.parties[&party_id]
        .tactical_signs
        .iter()
        .map(|(&k, &v)| (k, v))
        .collect()
}

/// Pressing a star marks the target for **every** member, not just the
/// presser: the sign is party state, and Java broadcasts it to `_members`.
/// The announcement rides along — `$c1 used $s3 on $c2`.
#[test]
fn a_tactical_sign_marks_the_target_for_the_whole_party() {
    let (mut world, ..) = test_world();
    insert_tactical_rows(&mut world);
    let mut a_rx = ingame_player(&mut world, 1, 7301, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 7302, 100, 0, 0);
    let party = make_party(&mut world, &[7301, 7302], LootRule::FindersKeepers);
    world.objects.add_components(&7301, TargetRef(Some(7302)));
    drain(&mut a_rx);
    drain(&mut b_rx);

    press(&mut world, 1, ACTION_SIGN_1);

    let a = drain(&mut a_rx);
    let b = drain(&mut b_rx);
    assert_eq!(
        tactical_signs_in(&a),
        vec![(7302, 1)],
        "the presser sees the marker"
    );
    assert_eq!(
        tactical_signs_in(&b),
        vec![(7302, 1)],
        "so does the other member — this is the half that made it look broken"
    );
    assert!(
        has_system_message(&a, server_packets::sm_ids::C1_USED_S3_ON_C2),
        "the placement is announced"
    );
    assert_eq!(signs_of(&world, party), vec![(1, 7302)]);
}

/// The same star pressed twice is a **toggle**, not a repeat: Java's middle
/// arm removes the sign and clears it on every client with token 0. No system
/// message — Java announces placing a sign, never lifting one.
#[test]
fn pressing_the_same_sign_again_lifts_it() {
    let (mut world, ..) = test_world();
    insert_tactical_rows(&mut world);
    let mut a_rx = ingame_player(&mut world, 1, 7311, 0, 0, 0);
    let _b_rx = ingame_player(&mut world, 2, 7312, 100, 0, 0);
    let party = make_party(&mut world, &[7311, 7312], LootRule::FindersKeepers);
    world.objects.add_components(&7311, TargetRef(Some(7312)));

    press(&mut world, 1, ACTION_SIGN_1);
    drain(&mut a_rx);
    press(&mut world, 1, ACTION_SIGN_1);

    let a = drain(&mut a_rx);
    assert_eq!(
        tactical_signs_in(&a),
        vec![(7312, 0)],
        "token 0 is how a marker is taken off"
    );
    assert!(
        !has_system_message(&a, server_packets::sm_ids::C1_USED_S3_ON_C2),
        "lifting a sign is silent"
    );
    assert!(signs_of(&world, party).is_empty());
}

/// Moving a sign to someone else clears the old wearer *first*: two packets,
/// in Java's order, or the client is left with two stars for one token.
#[test]
fn moving_a_sign_clears_the_previous_wearer_first() {
    let (mut world, ..) = test_world();
    insert_tactical_rows(&mut world);
    let mut a_rx = ingame_player(&mut world, 1, 7321, 0, 0, 0);
    let _b_rx = ingame_player(&mut world, 2, 7322, 100, 0, 0);
    let _c_rx = ingame_player(&mut world, 3, 7323, 200, 0, 0);
    let party = make_party(&mut world, &[7321, 7322, 7323], LootRule::FindersKeepers);
    world.objects.add_components(&7321, TargetRef(Some(7322)));

    press(&mut world, 1, ACTION_SIGN_1);
    world.objects.add_components(&7321, TargetRef(Some(7323)));
    drain(&mut a_rx);
    press(&mut world, 1, ACTION_SIGN_1);

    assert_eq!(
        tactical_signs_in(&drain(&mut a_rx)),
        vec![(7322, 0), (7323, 1)],
        "clear the old, then mark the new"
    );
    assert_eq!(signs_of(&world, party), vec![(1, 7323)]);
}

/// A creature wears one sign at a time: giving it a second takes the first
/// away (`_tacticalSigns.values().remove(target)`). Java drops the mapping
/// without a clear packet, because the new token overwrites the marker.
#[test]
fn a_second_sign_on_the_same_target_replaces_the_first() {
    let (mut world, ..) = test_world();
    insert_tactical_rows(&mut world);
    let mut a_rx = ingame_player(&mut world, 1, 7331, 0, 0, 0);
    let _b_rx = ingame_player(&mut world, 2, 7332, 100, 0, 0);
    let party = make_party(&mut world, &[7331, 7332], LootRule::FindersKeepers);
    world.objects.add_components(&7331, TargetRef(Some(7332)));

    press(&mut world, 1, ACTION_SIGN_1);
    drain(&mut a_rx);
    press(&mut world, 1, ACTION_SIGN_2);

    assert_eq!(
        signs_of(&world, party),
        vec![(2, 7332)],
        "sign 1 is gone, not kept alongside sign 2"
    );
    assert_eq!(
        tactical_signs_in(&drain(&mut a_rx)),
        vec![(7332, 2)],
        "no clear packet — the new token overwrites the marker"
    );
}

/// **The reported case.** A player with no party presses a star and nothing
/// appears — because tactical signs are party state and Java's handler bails
/// with a bare `ActionFailed` before touching anything. Worth pinning: the
/// port used to fail here for a different reason (no handler at all), and the
/// two are indistinguishable from the client.
#[test]
fn a_solo_player_pressing_a_star_gets_nothing() {
    let (mut world, ..) = test_world();
    insert_tactical_rows(&mut world);
    let mut a_rx = ingame_player(&mut world, 1, 7341, 0, 0, 0);
    world.objects.add_components(&7341, TargetRef(Some(7341)));
    drain(&mut a_rx);

    press(&mut world, 1, ACTION_SIGN_1);

    let a = drain(&mut a_rx);
    assert!(
        tactical_signs_in(&a).is_empty(),
        "no marker without a party"
    );
    assert!(
        a.iter()
            .any(|p| p[0] == server_packets::opcodes::ACTION_FAIL),
        "Java answers ActionFailed rather than saying why"
    );
}

/// A partied player targeting *themselves* is legal — a player is a Creature,
/// which is the only thing `TacticalSignUse` checks.
#[test]
fn a_sign_may_be_put_on_the_presser_themselves() {
    let (mut world, ..) = test_world();
    insert_tactical_rows(&mut world);
    let mut a_rx = ingame_player(&mut world, 1, 7351, 0, 0, 0);
    let _b_rx = ingame_player(&mut world, 2, 7352, 100, 0, 0);
    let party = make_party(&mut world, &[7351, 7352], LootRule::FindersKeepers);
    world.objects.add_components(&7351, TargetRef(Some(7351)));
    drain(&mut a_rx);

    press(&mut world, 1, ACTION_SIGN_1);

    assert_eq!(tactical_signs_in(&drain(&mut a_rx)), vec![(7351, 1)]);
    assert_eq!(signs_of(&world, party), vec![(1, 7351)]);
}

/// The recall half: `/targettacticalsign1` selects whoever wears sign 1.
#[test]
fn the_recall_action_selects_the_signed_creature() {
    let (mut world, ..) = test_world();
    insert_tactical_rows(&mut world);
    let _a_rx = ingame_player(&mut world, 1, 7361, 0, 0, 0);
    let _b_rx = ingame_player(&mut world, 2, 7362, 100, 0, 0);
    let _c_rx = ingame_player(&mut world, 3, 7363, 200, 0, 0);
    make_party(&mut world, &[7361, 7362, 7363], LootRule::FindersKeepers);
    world.objects.add_components(&7361, TargetRef(Some(7363)));
    press(&mut world, 1, ACTION_SIGN_1);

    // B, targeting nothing, presses the recall.
    world.objects.add_components(&7362, TargetRef(None));
    press(&mut world, 2, ACTION_TARGET_SIGN_1);

    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&7362)
            .and_then(|t| t.0),
        Some(7363),
        "the marked creature is now B's target"
    );
}

/// An unused sign recalls nothing, and says nothing — Java returns on a null
/// map entry without a packet.
#[test]
fn recalling_an_unused_sign_does_nothing() {
    let (mut world, ..) = test_world();
    insert_tactical_rows(&mut world);
    let mut a_rx = ingame_player(&mut world, 1, 7371, 0, 0, 0);
    let _b_rx = ingame_player(&mut world, 2, 7372, 100, 0, 0);
    make_party(&mut world, &[7371, 7372], LootRule::FindersKeepers);
    world.objects.add_components(&7371, TargetRef(None));
    drain(&mut a_rx);

    press(&mut world, 1, ACTION_TARGET_SIGN_1);

    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&7371)
            .and_then(|t| t.0),
        None
    );
}

/// The signs belong to the party, so a latecomer is handed the ones already
/// set, and someone who leaves has their markers wiped — while the party keeps
/// them for whoever stays (`applyTacticalSigns(player, remove)`).
#[test]
fn signs_follow_party_membership_in_and_out() {
    let (mut world, ..) = test_world();
    insert_tactical_rows(&mut world);
    let _a_rx = ingame_player(&mut world, 1, 7381, 0, 0, 0);
    let _b_rx = ingame_player(&mut world, 2, 7382, 100, 0, 0);
    let mut c_rx = ingame_player(&mut world, 3, 7383, 200, 0, 0);
    let party = make_party(&mut world, &[7381, 7382], LootRule::FindersKeepers);
    world.objects.add_components(&7381, TargetRef(Some(7382)));
    press(&mut world, 1, ACTION_SIGN_1);
    drain(&mut c_rx);

    crate::game_loop::party::add_party_member(&mut world, party, 7383);
    assert_eq!(
        tactical_signs_in(&drain(&mut c_rx)),
        vec![(7382, 1)],
        "the joiner is caught up on the markers already out"
    );

    crate::game_loop::party::remove_party_member(
        &mut world,
        party,
        7383,
        crate::game_loop::party::LeaveType::Left,
    );
    assert_eq!(
        tactical_signs_in(&drain(&mut c_rx)),
        vec![(7382, 0)],
        "and loses them on the way out"
    );
    assert_eq!(
        signs_of(&world, party),
        vec![(1, 7382)],
        "the party keeps the sign itself"
    );
}

/// The eight rows the two handlers stand on, read from the shipped file: four
/// `TacticalSignUse` options and four `TacticalSignTarget`, numbered 1..=4.
#[test]
fn the_real_action_data_binds_all_eight_tactical_rows() {
    let data = ActionData::load_from(DIST);

    for (id, option) in (78..=81).zip(1..=4) {
        let row = data.row(id).unwrap_or_else(|| panic!("id {id} ships"));
        assert_eq!(row.handler, "TacticalSignUse", "id {id}");
        assert_eq!(row.option, option, "id {id} is /tacticalsign{option}");
    }
    for (id, option) in (82..=85).zip(1..=4) {
        let row = data.row(id).unwrap_or_else(|| panic!("id {id} ships"));
        assert_eq!(row.handler, "TacticalSignTarget", "id {id}");
        assert_eq!(row.option, option, "id {id} is /targettacticalsign{option}");
    }
}
