//! Opcode dispatch: route an inbound client packet (or `0xD0` ex-packet) to
//! its handler — the Rust face of Java's one-class-per-packet
//! `network/clientpackets` registry. Handlers small enough not to warrant a
//! module of their own live inline here.

use tracing::error;

use crate::network::client_packets::{self as cp, ex_opcodes as exop, opcodes as cop};
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::World;

use super::combat::handle_attack_request;
use super::death::{handle_appearing, handle_request_restart_point};
use super::items::{handle_request_un_equip_item, handle_use_item};
use super::lobby::{
    handle_auth_login, handle_character_create, handle_character_delete,
    handle_character_restore, handle_character_select, handle_enter_world, handle_new_character,
    handle_request_character_name_creatable,
};
use super::net::{handle_logout, handle_request_restart};
use super::position::{handle_move_backward_to_location, handle_validate_position};
use super::skills::cast::handle_request_magic_skill_use;
use super::skills::handle_request_acquire_skill;
use super::target::{handle_action, handle_request_target_canceld};

/// Dispatch one decrypted client packet body (opcode + payload) on the game
/// thread, gated by the client's session state (Java's per-`ConnectionState`
/// validity).
pub(crate) fn on_packet(world: &mut World, client_id: u32, data: Vec<u8>) {
    let Some(&opcode) = data.first() else { return };
    let body = &data[1..];
    match opcode {
        cop::AUTH_LOGIN => handle_auth_login(world, client_id, body),
        cop::NEW_CHARACTER => handle_new_character(world, client_id),
        cop::CHARACTER_CREATE => handle_character_create(world, client_id, body),
        cop::CHARACTER_DELETE => handle_character_delete(world, client_id, body),
        cop::CHARACTER_RESTORE => handle_character_restore(world, client_id, body),
        cop::CHARACTER_SELECT => handle_character_select(world, client_id, body),
        cop::ENTER_WORLD => handle_enter_world(world, client_id),
        // RequestSkillCoolTime (IN_GAME): resend the reuse timers.
        cop::REQUEST_SKILL_COOL_TIME => {
            if let Some(cs @ ClientSession::InGame(session)) = world.clients.get(&client_id) {
                if let Some(player) = world.players.get(&session.player_object_id()) {
                    cs.send(server_packets::skill_cool_time(player, world.tick));
                }
            }
        }
        cop::USE_ITEM => handle_use_item(world, client_id, body),
        cop::REQUEST_UN_EQUIP_ITEM => handle_request_un_equip_item(world, client_id, body),
        cop::REQUEST_MAGIC_SKILL_USE => handle_request_magic_skill_use(world, client_id, body),
        cop::REQUEST_ACQUIRE_SKILL => handle_request_acquire_skill(world, client_id, body),
        cop::ACTION => handle_action(world, client_id, body),
        cop::ATTACK_REQUEST => handle_attack_request(world, client_id, body),
        cop::APPEARING => handle_appearing(world, client_id),
        cop::REQUEST_RESTART_POINT => handle_request_restart_point(world, client_id, body),
        cop::REQUEST_TARGET_CANCELD => handle_request_target_canceld(world, client_id, body),
        cop::MOVE_BACKWARD_TO_LOCATION => handle_move_backward_to_location(world, client_id, body),
        cop::VALIDATE_POSITION => handle_validate_position(world, client_id, body),
        cop::LOGOUT => handle_logout(world, client_id),
        cop::REQUEST_RESTART => handle_request_restart(world, client_id),
        cop::EX_PACKET => on_ex_packet(world, client_id, body),
        _ => error!("GameLoop: client {client_id} sent opcode 0x{opcode:02x}, unhandled."),
    }
}

/// Dispatch an extended (`0xD0`) client packet by its 2-byte sub-opcode.
pub(crate) fn on_ex_packet(world: &mut World, client_id: u32, body: &[u8]) {
    let Some((sub, ex_body)) = cp::read_ex_opcode(body) else {
        return;
    };
    match sub {
        exop::REQUEST_CHARACTER_NAME_CREATABLE => {
            handle_request_character_name_creatable(world, client_id, ex_body)
        }
        // RequestKeyMapping (ENTERING + IN_GAME): STORE_UI_SETTINGS is on, so
        // reply with the (empty) stored UI key mapping.
        exop::REQUEST_KEY_MAPPING => {
            if let Some(cs) = world.clients.get(&client_id) {
                if matches!(cs, ClientSession::Entering(_) | ClientSession::InGame(_)) {
                    cs.send(server_packets::ex_ui_setting());
                }
            }
        }
        // RequestManorList (IN_GAME): the castles that offer a manor.
        exop::REQUEST_MANOR_LIST => {
            if let Some(cs @ ClientSession::InGame(_)) = world.clients.get(&client_id) {
                cs.send(server_packets::ex_send_manor_list());
            }
        }
        // RequestUserBanInfo (IN_GAME): Mobius has a null handler — the client
        // tolerates no reply, so consume it silently. TODO: ExUserBanInfo.
        exop::REQUEST_USER_BAN_INFO => {}
        exop::REQUEST_GOTO_LOBBY => {
            let maybe_session = world.clients.get(&client_id);
            if let Some(ClientSession::InLobby(session)) = maybe_session {
                let body = server_packets::char_selection_info(
                    &session.state.account,
                    session.play_ok1(),
                    &session.state.chars,
                    -1,
                    world.max_characters_per_account,
                    &world.data.experience,
                );
                session.send(body);
            }
        }
        _ => error!("GameLoop: client {client_id} sent ex-opcode 0x{sub:04x}, unhandled."),
    }
}

