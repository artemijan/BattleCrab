//! World chat — `handlers/chathandlers/ChatWorld` + `DailyTaskManager
//! .resetWorldChatPoints`.

use super::*;
use crate::db::DbCommand;
use crate::enums::ChatType;
use crate::game_loop::chat;
use crate::model::Player;
use crate::model::components::{PlayerVariables, WORLD_CHAT_USED};
use crate::network::server_packets::{opcodes, sm_ids};

fn world_body(text: &str) -> Vec<u8> {
    say2_body(text, ChatType::World.client_id(), None)
}

/// Everything world chat needs that `dummy_char` does not give: a level over
/// the dist's minimum, and the `PlayerVariables` component the quota counter
/// lives in (a bare test player is spawned without one).
fn set_up_speaker(world: &mut World, oid: i32, level: i32) {
    world
        .objects
        .get_component_mut::<Player>(&oid)
        .expect("speaker")
        .level = level;
    world
        .objects
        .add_components(&oid, PlayerVariables::default());
}

fn used(world: &World, oid: i32) -> i32 {
    world
        .objects
        .get_component::<PlayerVariables>(&oid)
        .map_or(0, |v| v.get_int(WORLD_CHAT_USED, 0))
}

/// Pull the `i32` payload out of the one `ExWorldChatCnt` in a packet batch.
fn quota_left(pkts: &[Vec<u8>]) -> Option<i32> {
    let p = pkts.iter().find(|p| is_ex(p, opcodes::EX_WORLD_CHAT_CNT))?;
    commons::network::PacketReader::new(&p[3..]).read_i32()
}

/// The happy path: the line reaches *every* online player including the
/// speaker, a point is spent, and the speaker alone is told what is left.
///
/// The broadcast is deliberately unfiltered by distance or region — world chat
/// is the one channel with no locality rule, so a listener parked at the far
/// corner of the map must still hear it.
#[test]
fn a_world_chat_line_reaches_every_online_player_and_spends_a_point() {
    let (mut world, ..) = test_world();
    let mut speaker_rx = ingame_player(&mut world, 1, 2001, 0, 0, 0);
    let mut listener_rx = ingame_player(&mut world, 2, 2002, 900_000, 900_000, 0);
    set_up_speaker(&mut world, 2001, 40);
    set_up_speaker(&mut world, 2002, 40);
    drain(&mut speaker_rx);
    drain(&mut listener_rx);

    chat::handle_say2(&mut world, 1, &world_body("hello world"));

    let heard = drain(&mut listener_rx);
    let say = heard
        .iter()
        .find(|p| p[0] == opcodes::SAY2)
        .expect("a listener on the other side of the map still hears world chat");
    let (oid, ty, name, text, _) = parse_creature_say(say);
    assert_eq!(
        (oid, ty, name.as_str(), text.as_str()),
        (2001, ChatType::World.client_id(), "P2001", "hello world")
    );

    let own = drain(&mut speaker_rx);
    assert!(
        own.iter().any(|p| p[0] == opcodes::SAY2),
        "the speaker hears their own line (Java broadcasts to all players, \
         with no self-exclusion)"
    );
    assert_eq!(used(&world, 2001), 1, "one point spent");
    assert_eq!(
        quota_left(&own),
        Some(9),
        "the speaker is told the 9 remaining of WorldChatPointsPerDay = 10"
    );
    assert!(
        quota_left(&heard).is_none(),
        "ExWorldChatCnt goes to the speaker only"
    );
}

/// The three refusals, each with its own message, and none of them spending a
/// point or reaching anyone.
///
/// **Order matters**: Java checks the level *before* the quota, so an
/// under-levelled character with a spent quota is told to level up. Asserting
/// the level case while `used` is already at the cap is what pins that order —
/// swap the two branches and this case reports the quota message instead.
#[test]
fn level_quota_and_reuse_each_refuse_with_their_own_message() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 2001, 0, 0, 0);
    let mut listener_rx = ingame_player(&mut world, 2, 2002, 0, 0, 0);
    set_up_speaker(&mut world, 2001, 40);
    set_up_speaker(&mut world, 2002, 40);

    // --- Under the minimum level, with the quota also exhausted. ---
    world
        .objects
        .get_component_mut::<Player>(&2001)
        .unwrap()
        .level = 39;
    world
        .objects
        .get_component_mut::<PlayerVariables>(&2001)
        .unwrap()
        .set_int(WORLD_CHAT_USED, 10);
    drain(&mut rx);
    drain(&mut listener_rx);

    chat::handle_say2(&mut world, 1, &world_body("too low"));
    assert_eq!(
        sm_ids_of(&drain(&mut rx)),
        vec![sm_ids::YOU_CAN_USE_WORLD_CHAT_FROM_LV_S1],
        "the level check precedes the quota check"
    );
    assert!(drain(&mut listener_rx).is_empty(), "nothing broadcast");

    // --- Level restored, quota still spent. ---
    world
        .objects
        .get_component_mut::<Player>(&2001)
        .unwrap()
        .level = 40;
    chat::handle_say2(&mut world, 1, &world_body("no points"));
    assert_eq!(
        sm_ids_of(&drain(&mut rx)),
        vec![sm_ids::YOU_HAVE_SPENT_YOUR_WORLD_CHAT_QUOTA_FOR_THE_DAY]
    );
    assert!(drain(&mut listener_rx).is_empty(), "nothing broadcast");

    // --- Quota restored: the line lands, then the reuse window bites. ---
    world
        .objects
        .get_component_mut::<PlayerVariables>(&2001)
        .unwrap()
        .set_int(WORLD_CHAT_USED, 0);
    chat::handle_say2(&mut world, 1, &world_body("first"));
    assert!(
        drain(&mut listener_rx)
            .iter()
            .any(|p| p[0] == opcodes::SAY2)
    );
    drain(&mut rx);

    chat::handle_say2(&mut world, 1, &world_body("too soon"));
    assert_eq!(
        sm_ids_of(&drain(&mut rx)),
        vec![sm_ids::YOU_HAVE_S1_SEC_UNTIL_YOU_ARE_ABLE_TO_USE_WORLD_CHAT],
        "the 20s WorldChatInterval refuses the second line"
    );
    assert!(
        drain(&mut listener_rx).is_empty(),
        "a reuse refusal broadcasts nothing"
    );
    assert_eq!(
        used(&world, 2001),
        1,
        "a refused line must not spend a point"
    );
}

/// `WorldChatEnabled = False` makes the channel vanish **silently** — Java's
/// first line is a bare `return`, with no notice to the speaker. A test that
/// only checked "nothing was broadcast" would also pass if the port sent a
/// refusal message, so the speaker's own stream is asserted empty too.
#[test]
fn the_disabled_channel_is_silent_not_refused() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 2001, 0, 0, 0);
    let mut listener_rx = ingame_player(&mut world, 2, 2002, 0, 0, 0);
    set_up_speaker(&mut world, 2001, 40);
    set_up_speaker(&mut world, 2002, 40);
    world.cfg.general.world_chat_enabled = false;
    drain(&mut rx);
    drain(&mut listener_rx);

    chat::handle_say2(&mut world, 1, &world_body("silence"));

    assert!(
        drain(&mut rx).is_empty(),
        "no refusal message — Java returns before sending anything"
    );
    assert!(drain(&mut listener_rx).is_empty());
    assert_eq!(used(&world, 2001), 0);
}

/// A jailed speaker is refused (`JailDisableChat`), and a GM holding
/// `PlayerCondOverride.CHAT_CONDITIONS` is not.
#[test]
fn jail_silences_world_chat_unless_the_cond_is_overridden() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 2001, 0, 0, 0);
    let mut listener_rx = ingame_player(&mut world, 2, 2002, 0, 0, 0);
    set_up_speaker(&mut world, 2001, 40);
    set_up_speaker(&mut world, 2002, 40);
    world
        .objects
        .get_component_mut::<Player>(&2001)
        .unwrap()
        .jailed = true;
    drain(&mut rx);
    drain(&mut listener_rx);

    chat::handle_say2(&mut world, 1, &world_body("let me out"));
    assert_eq!(
        sm_ids_of(&drain(&mut rx)),
        vec![sm_ids::CHATTING_IS_CURRENTLY_PROHIBITED]
    );
    assert!(drain(&mut listener_rx).is_empty());

    // `CHAT_CONDITIONS` is ordinal 8 in Java's `PlayerCondOverride`.
    world
        .objects
        .get_component_mut::<Player>(&2001)
        .unwrap()
        .cond_overrides |= 1 << 8;
    chat::handle_say2(&mut world, 1, &world_body("gm speaking"));
    assert!(
        drain(&mut listener_rx)
            .iter()
            .any(|p| p[0] == opcodes::SAY2),
        "CHAT_CONDITIONS overrides the jail gate"
    );
}

/// The daily reset clears the counter for the online population in memory,
/// tells each of them their refreshed quota, and fires the one UPDATE that
/// covers the offline rows.
#[test]
fn the_daily_reset_clears_the_quota_online_and_offline() {
    let (mut world, _tx, mut db_rx, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 2001, 0, 0, 0);
    set_up_speaker(&mut world, 2001, 40);
    world
        .objects
        .get_component_mut::<PlayerVariables>(&2001)
        .unwrap()
        .set_int(WORLD_CHAT_USED, 7);
    drain(&mut rx);
    while db_rx.try_recv().is_ok() {}

    crate::game_loop::daily_tasks::run_reset(&mut world, false);

    assert_eq!(used(&world, 2001), 0, "online counter cleared in memory");
    assert_eq!(
        quota_left(&drain(&mut rx)),
        Some(10),
        "and the client is told the full allowance is back"
    );
    let mut cmds = Vec::new();
    while let Ok(c) = db_rx.try_recv() {
        cmds.push(c);
    }
    assert!(
        cmds.iter()
            .any(|c| matches!(c, DbCommand::ResetWorldChatPoints)),
        "the offline population's UPDATE is queued"
    );
}

/// With the channel off the reset is a no-op in **both** halves — Java returns
/// before the UPDATE as well as before the in-memory loop, so a stored counter
/// is left exactly as it was.
#[test]
fn the_daily_reset_is_gated_on_the_channel_being_enabled() {
    let (mut world, _tx, mut db_rx, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 2001, 0, 0, 0);
    set_up_speaker(&mut world, 2001, 40);
    world
        .objects
        .get_component_mut::<PlayerVariables>(&2001)
        .unwrap()
        .set_int(WORLD_CHAT_USED, 7);
    world.cfg.general.world_chat_enabled = false;
    drain(&mut rx);
    while db_rx.try_recv().is_ok() {}

    crate::game_loop::daily_tasks::run_reset(&mut world, false);

    assert_eq!(used(&world, 2001), 7, "the stored counter is untouched");
    let mut cmds = Vec::new();
    while let Ok(c) = db_rx.try_recv() {
        cmds.push(c);
    }
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, DbCommand::ResetWorldChatPoints)),
        "no UPDATE queued either"
    );
}

/// `BanChatChannels` names `WORLD`, and the whole reason the port needed
/// `ChatType::World` at all: without the mapping the entry was dropped with a
/// boot warning and a chat-banned player got no prohibition notice here.
#[test]
fn the_shipped_ban_chat_channels_list_includes_world() {
    let cfg = crate::config::ChatFilterConfig::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    assert!(
        cfg.ban_chat_channels.contains(&ChatType::World),
        "the real General.ini's BanChatChannels must parse WORLD"
    );
    // The rest of the shipped list, so a typo that drops one is caught too.
    for ty in [
        ChatType::General,
        ChatType::Shout,
        ChatType::Trade,
        ChatType::HeroVoice,
        ChatType::Whisper,
    ] {
        assert!(cfg.ban_chat_channels.contains(&ty), "missing {ty:?}");
    }
}
