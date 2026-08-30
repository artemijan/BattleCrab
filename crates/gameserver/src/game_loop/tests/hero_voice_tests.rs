//! Hero chat — `handlers/chathandlers/ChatHeroVoice` (`%`). The channel sat
//! unported behind the datapack blind spot the chat module header describes:
//! `//sethero` granted the flag, but every hero line was silently dropped at
//! dispatch.

use super::*;
use crate::enums::ChatType;
use crate::game_loop::social::chat;
use crate::model::Player;
use crate::network::server_packets::{opcodes, sm_ids};

fn hero_body(text: &str) -> Vec<u8> {
    say2_body(text, ChatType::HeroVoice.client_id(), None)
}

/// A speaker and a listener parked at the far corner of the map — hero voice,
/// like world chat, has no locality rule.
fn two_players(
    world: &mut World,
) -> (
    UnboundedReceiver<bytes::Bytes>,
    UnboundedReceiver<bytes::Bytes>,
) {
    let speaker = ingame_player(world, 1, 2001, 0, 0, 0);
    let listener = ingame_player(world, 2, 2002, 900_000, 900_000, 0);
    (speaker, listener)
}

/// The user-visible bug this file exists for: `//sethero` used to grant the
/// flag while the channel handler didn't exist, so a freshly-made hero typed
/// into the void. The line must now reach every online player — speaker
/// included, distance ignored.
#[test]
fn a_heros_line_reaches_every_online_player() {
    let (mut world, ..) = test_world();
    let (mut rx, mut listener_rx) = two_players(&mut world);
    crate::game_loop::admin::hero::set_hero(&mut world, 2001, true);
    drain(&mut rx);
    drain(&mut listener_rx);

    chat::handle_say2(&mut world, 1, &hero_body("for the realm"));

    let heard = drain(&mut listener_rx);
    let say = heard
        .iter()
        .find(|p| p[0] == opcodes::SAY2)
        .expect("a listener on the other side of the map still hears hero chat");
    let (oid, ty, name, text, _) = parse_creature_say(say);
    assert_eq!(
        (oid, ty, name.as_str(), text.as_str()),
        (
            2001,
            ChatType::HeroVoice.client_id(),
            "P2001",
            "for the realm"
        )
    );
    assert!(
        drain(&mut rx).iter().any(|p| p[0] == opcodes::SAY2),
        "the speaker hears their own line (Java broadcasts to all players)"
    );
}

/// A non-hero is refused with the hero-channel notice and nobody hears
/// anything; `PlayerCondOverride.CHAT_CONDITIONS` (ordinal 8) is the one
/// escape, exactly as on the world-chat jail gate.
#[test]
fn a_non_hero_is_refused_unless_the_cond_is_overridden() {
    let (mut world, ..) = test_world();
    let (mut rx, mut listener_rx) = two_players(&mut world);
    drain(&mut rx);
    drain(&mut listener_rx);

    chat::handle_say2(&mut world, 1, &hero_body("am I a hero?"));
    assert_eq!(
        ids_after_opcode(&drain(&mut rx), opcodes::SYSTEM_MESSAGE),
        vec![sm_ids::ONLY_HEROES_CAN_ENTER_THE_HERO_CHANNEL]
    );
    assert!(drain(&mut listener_rx).is_empty(), "nothing broadcast");

    world
        .objects
        .get_component_mut::<Player>(&2001)
        .unwrap()
        .cond_overrides |= 1 << 8;
    chat::handle_say2(&mut world, 1, &hero_body("gm speaking"));
    assert!(
        drain(&mut listener_rx)
            .iter()
            .any(|p| p[0] == opcodes::SAY2),
        "CHAT_CONDITIONS overrides the hero requirement"
    );
}

/// `canUseHeroVoice()` — the one chat channel this dist rate-limits. A second
/// line inside the 100-tick window is refused with Java's literal
/// `sendMessage` line (SM `S1_TEXT`, not a prohibition SystemMessage) and
/// reaches nobody; once the window rolls over the channel opens again.
#[test]
fn the_ten_second_window_refuses_a_second_line() {
    let (mut world, ..) = test_world();
    // The fixture ships with every protector off (see `test_world`); this test
    // is *about* the protector, so load the real dist config back in.
    world.cfg.flood_protector =
        crate::config::flood_protector::FloodProtectorsConfig::load_from(crate::data::DIST_GAME);
    let (mut rx, mut listener_rx) = two_players(&mut world);
    crate::game_loop::admin::hero::set_hero(&mut world, 2001, true);
    drain(&mut rx);
    drain(&mut listener_rx);

    chat::handle_say2(&mut world, 1, &hero_body("first"));
    assert!(
        drain(&mut listener_rx)
            .iter()
            .any(|p| p[0] == opcodes::SAY2)
    );
    drain(&mut rx);

    chat::handle_say2(&mut world, 1, &hero_body("too soon"));
    assert!(
        drain(&mut listener_rx).is_empty(),
        "a flooded line must not reach anyone"
    );
    assert_eq!(
        ids_after_opcode(&drain(&mut rx), opcodes::SYSTEM_MESSAGE),
        vec![sm_ids::S1_TEXT],
        "refused with Java's literal ten-second line"
    );

    world.tick += 100;
    chat::handle_say2(&mut world, 1, &hero_body("second"));
    assert!(
        drain(&mut listener_rx)
            .iter()
            .any(|p| p[0] == opcodes::SAY2),
        "the window rolls over after FloodProtectorHeroVoiceInterval ticks"
    );
}

/// `BlockList.isBlocked(player, activeChar)` — the listener's list decides,
/// and only for that listener; everyone else (the speaker included) still
/// hears the line.
#[test]
fn a_listener_who_blocked_the_hero_does_not_hear_them() {
    let (mut world, ..) = test_world();
    let (mut rx, mut listener_rx) = two_players(&mut world);
    crate::game_loop::admin::hero::set_hero(&mut world, 2001, true);
    world.block_lists.entry(2002).or_default().insert(2001);
    drain(&mut rx);
    drain(&mut listener_rx);

    chat::handle_say2(&mut world, 1, &hero_body("shunned"));

    assert!(
        !drain(&mut listener_rx)
            .iter()
            .any(|p| p[0] == opcodes::SAY2),
        "the blocking listener is skipped"
    );
    assert!(
        drain(&mut rx).iter().any(|p| p[0] == opcodes::SAY2),
        "the speaker still hears their own line"
    );
}
