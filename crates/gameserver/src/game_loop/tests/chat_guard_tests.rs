//! `Say2`'s own pre-dispatch guards — the ones that run before any
//! `handlers/chathandlers/*` handler sees the line.

use super::*;
use crate::enums::ChatType;
use crate::game_loop::chat;
use crate::model::Player;
use crate::network::server_packets::{opcodes, sm_ids};

fn line(ty: ChatType, text: &str) -> Vec<u8> {
    // Whisper carries a target name; every other channel does not.
    let target = (ty == ChatType::Whisper).then_some("P2002");
    say2_body(text, ty.client_id(), target)
}

/// Two players in earshot of each other, both high enough for world chat.
fn two_players(
    world: &mut World,
) -> (
    tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
    tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
) {
    let speaker = ingame_player(world, 1, 2001, 0, 0, 0);
    let listener = ingame_player(world, 2, 2002, 0, 0, 0);
    for oid in [2001, 2002] {
        world
            .objects
            .get_component_mut::<Player>(&oid)
            .expect("player")
            .level = 40;
        world
            .objects
            .add_components(&oid, crate::model::components::PlayerVariables::default());
    }
    (speaker, listener)
}

/// `Say2`'s jail gate — the sibling of the one `world_chat` applies, and
/// deliberately **not** a copy of it. Java silences a prisoner on exactly four
/// channels (whisper, shout, trade, hero voice), leaves ordinary/party/clan
/// chat open, and answers with a literal `sendMessage` line rather than a
/// `SystemMessage`.
///
/// The channel *set* is the whole point. Asserting only that a jailed player is
/// refused somewhere would pass just as well if the gate covered every channel,
/// which is the plausible way to get this wrong — so the open channel is
/// asserted too.
#[test]
fn jail_silences_four_channels_and_leaves_the_rest_open() {
    let (mut world, ..) = test_world();
    let (mut rx, mut listener_rx) = two_players(&mut world);
    world
        .objects
        .get_component_mut::<Player>(&2001)
        .unwrap()
        .jailed = true;

    for ty in [
        ChatType::Whisper,
        ChatType::Shout,
        ChatType::Trade,
        ChatType::HeroVoice,
    ] {
        drain(&mut rx);
        drain(&mut listener_rx);
        chat::handle_say2(&mut world, 1, &line(ty, "hello"));
        assert!(
            drain(&mut listener_rx).is_empty(),
            "{ty:?} must not reach anyone from jail"
        );
        // Java answers with `sendMessage(String)`, which is SM `S1_TEXT` — not
        // one of the prohibition SystemMessages the chat-ban path sends.
        assert_eq!(
            ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE),
            vec![sm_ids::S1_TEXT],
            "{ty:?} refused with Java's literal jail line"
        );
    }

    // GENERAL is absent from Java's list: a prisoner may still talk locally.
    drain(&mut rx);
    drain(&mut listener_rx);
    chat::handle_say2(&mut world, 1, &line(ChatType::General, "hi"));
    assert!(
        drain(&mut listener_rx)
            .iter()
            .any(|p| p[0] == opcodes::SAY2),
        "ordinary chat stays open in jail"
    );
}

/// The jail gate is gated on `JailDisableChat`, which ships `True` — with it
/// off a prisoner chats freely on all four channels.
#[test]
fn the_jail_gate_honours_its_config_flag() {
    let (mut world, ..) = test_world();
    let (mut rx, mut listener_rx) = two_players(&mut world);
    world
        .objects
        .get_component_mut::<Player>(&2001)
        .unwrap()
        .jailed = true;
    world.cfg.general.jail_disable_chat = false;

    drain(&mut rx);
    drain(&mut listener_rx);
    chat::handle_say2(&mut world, 1, &line(ChatType::Shout, "hi"));
    assert!(
        drain(&mut listener_rx)
            .iter()
            .any(|p| p[0] == opcodes::SAY2),
        "JailDisableChat = False lets a prisoner shout"
    );
}

/// `Say2`'s olympiad gate: no chat on **any** channel while in a bout or merely
/// registered, and no GM exemption. Contrast the jail gate above, which is
/// per-channel — porting one as a copy of the other would be wrong twice.
#[test]
fn the_olympiad_gate_silences_every_channel() {
    let (mut world, ..) = test_world();
    let (mut rx, mut listener_rx) = two_players(&mut world);

    // Registration alone is enough — Java checks `isInOlympiadMode() ||
    // isRegistered()`, so no match need be running.
    world.olympiad.non_class_registers.insert(2001);

    for ty in [ChatType::General, ChatType::Shout, ChatType::World] {
        drain(&mut rx);
        drain(&mut listener_rx);
        chat::handle_say2(&mut world, 1, &line(ty, "hi"));
        assert!(
            drain(&mut listener_rx).is_empty(),
            "{ty:?} must not reach anyone while registered"
        );
        assert_eq!(
            ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE),
            vec![sm_ids::YOU_CANNOT_CHAT_WHILE_PARTICIPATING_IN_THE_OLYMPIAD],
            "{ty:?} refused with the olympiad line"
        );
    }

    world.olympiad.non_class_registers.remove(&2001);
    drain(&mut listener_rx);
    chat::handle_say2(&mut world, 1, &line(ChatType::General, "hi"));
    assert!(
        drain(&mut listener_rx)
            .iter()
            .any(|p| p[0] == opcodes::SAY2),
        "chat returns once the registration is gone"
    );
}
