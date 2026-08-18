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

fn action_use_body(action_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(action_id);
    w.write_i32(0); // ctrl
    w.write_u8(0); // shift
    w.into_bytes()
}

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
