//! Opcode dispatch: route an inbound client packet (or `0xD0` ex-packet) to
//! its handler — the Rust face of Java's one-class-per-packet
//! `network/clientpackets` registry. Handlers small enough not to warrant a
//! module of their own live inline here.

use tracing::{error, trace};

use crate::network::client_packets::{self as cp, ex_opcodes as exop, opcodes as cop};
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::World;

use super::bypass::handle_request_bypass_to_server;
use super::chat::handle_say2;
use super::combat::handle_attack_request;
use super::death::{handle_appearing, handle_request_restart_point};
use super::friends::{
    handle_request_answer_friend_invite, handle_request_friend_del, handle_request_friend_invite,
    handle_request_friend_list, handle_request_send_friend_msg,
};
use super::items::{
    handle_request_item_list, handle_request_save_inventory_order, handle_request_un_equip_item,
    handle_use_item,
};
use super::lobby::{
    handle_auth_login, handle_character_create, handle_character_delete, handle_character_restore,
    handle_character_select, handle_enter_world, handle_new_character,
    handle_request_character_name_creatable,
};
use super::net::{handle_logout, handle_request_restart};
use super::party::{
    handle_answer_party_loot_modification, handle_request_answer_join_party,
    handle_request_change_party_leader, handle_request_join_party,
    handle_request_oust_party_member, handle_request_party_loot_modification,
    handle_request_withdrawal_party,
};
use super::position::{
    handle_ex_send_selected_quest_zone_id, handle_move_backward_to_location,
    handle_request_stop_move, handle_validate_position,
};
use super::shortcuts::{
    handle_request_delete_macro, handle_request_make_macro, handle_request_short_cut_del,
    handle_request_short_cut_reg,
};
use super::skills::cast::handle_request_magic_skill_use;
use super::skills::handle_request_acquire_skill;
use super::target::{handle_action, handle_request_target_canceld};

/// Dispatch one decrypted client packet body (opcode + payload) on the game
/// thread, gated by the client's session state (Java's per-`ConnectionState`
/// validity).
pub(crate) fn on_packet(world: &mut World, client_id: u32, data: Vec<u8>) {
    let Some(&opcode) = data.first() else { return };
    let body = &data[1..];
    trace!("client {client_id} → opcode 0x{opcode:02x} ({} B)", data.len());
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
                if let Some(reuses) = world
                    .objects
                    .get_component::<crate::model::components::Reuses>(&session.player_object_id())
                {
                    cs.send(server_packets::skill_cool_time(reuses, world.tick));
                }
            }
        }
        // RequestSkillList (IN_GAME): empty body, just `player.sendSkillList()`.
        cop::REQUEST_SKILL_LIST => {
            if let Some(cs @ ClientSession::InGame(session)) = world.clients.get(&client_id) {
                if let Some(skills) = world
                    .objects
                    .get_component::<crate::model::components::SkillBook>(
                        &session.player_object_id(),
                    )
                {
                    cs.send(crate::network::enter_world::skill_list(skills, &world.data));
                }
            }
        }
        cop::REQUEST_ITEM_LIST => handle_request_item_list(world, client_id),
        cop::USE_ITEM => handle_use_item(world, client_id, body),
        cop::REQUEST_UN_EQUIP_ITEM => handle_request_un_equip_item(world, client_id, body),
        cop::REQUEST_DESTROY_ITEM => super::items::handle_request_destroy_item(world, client_id, body),
        cop::REQUEST_CRYSTALLIZE_ITEM => super::items::handle_request_crystallize_item(world, client_id, body),
        cop::REQUEST_DROP_ITEM => super::ground_items::handle_request_drop_item(world, client_id, body),
        cop::SEND_WARE_HOUSE_DEPOSIT_LIST => super::warehouse::handle_deposit(world, client_id, body),
        cop::SEND_WARE_HOUSE_WITH_DRAW_LIST => super::warehouse::handle_withdraw(world, client_id, body),
        cop::REQUEST_ENCHANT_ITEM => super::enchant::handle_enchant(world, client_id, body),
        cop::REQUEST_MAGIC_SKILL_USE => handle_request_magic_skill_use(world, client_id, body),
        cop::REQUEST_ACQUIRE_SKILL => handle_request_acquire_skill(world, client_id, body),
        cop::ACTION => handle_action(world, client_id, body),
        cop::ATTACK | cop::ATTACK_REQUEST => handle_attack_request(world, client_id, body),
        cop::APPEARING => handle_appearing(world, client_id),
        cop::REQUEST_RESTART_POINT => handle_request_restart_point(world, client_id, body),
        cop::REQUEST_TARGET_CANCELD => handle_request_target_canceld(world, client_id, body),
        cop::MOVE_BACKWARD_TO_LOCATION => handle_move_backward_to_location(world, client_id, body),
        cop::VALIDATE_POSITION => handle_validate_position(world, client_id, body),
        cop::REQUEST_SHORT_CUT_REG => handle_request_short_cut_reg(world, client_id, body),
        cop::REQUEST_SHORT_CUT_DEL => handle_request_short_cut_del(world, client_id, body),
        cop::REQUEST_MAKE_MACRO => handle_request_make_macro(world, client_id, body),
        cop::REQUEST_DELETE_MACRO => handle_request_delete_macro(world, client_id, body),
        cop::SAY2 => handle_say2(world, client_id, body),
        cop::REQUEST_BYPASS_TO_SERVER => handle_request_bypass_to_server(world, client_id, body),
        // SendBypassBuildCmd (IN_GAME): the `//command` GM bar → admin command
        // with the `admin_` prefix Java prepends.
        cop::SEND_BYPASS_BUILD_CMD => {
            if let Some(cmd) = cp::read_build_command(body) {
                if !cmd.is_empty() {
                    super::admin::use_admin_command(
                        world,
                        client_id,
                        &format!("admin_{cmd}"),
                        true,
                    );
                }
            }
        }
        // BypassUserCmd (IN_GAME): the client `/command` bar (`/loc`,
        // `/unstuck`, …).
        cop::BYPASS_USER_CMD => {
            super::user_commands::handle_bypass_user_cmd(world, client_id, body)
        }
        // DlgAnswer (IN_GAME): reply to a ConfirmDlg — the admin-confirm flow.
        cop::DLG_ANSWER => {
            if let Some(answer) = cp::DlgAnswer::read(body) {
                super::admin::handle_dlg_answer(world, client_id, answer);
            }
        }
        cop::REQUEST_BUY_ITEM => super::shop::handle_request_buy_item(world, client_id, body),
        cop::REQUEST_SELL_ITEM => super::shop::handle_request_sell_item(world, client_id, body),
        cop::REQUEST_PRIVATE_STORE_MANAGE_SELL => super::private_store::open_manage(world, client_id),
        cop::SET_PRIVATE_STORE_LIST_SELL => super::private_store::handle_set_list(world, client_id, body),
        cop::REQUEST_PRIVATE_STORE_QUIT_SELL => super::private_store::handle_quit(world, client_id),
        cop::REQUEST_PRIVATE_STORE_BUY => super::private_store::handle_buy(world, client_id, body),
        cop::TRADE_REQUEST => super::trade::handle_request(world, client_id, body),
        cop::ANSWER_TRADE_REQUEST => super::trade::handle_answer(world, client_id, body),
        cop::ADD_TRADE_ITEM => super::trade::handle_add_item(world, client_id, body),
        cop::TRADE_DONE => super::trade::handle_done(world, client_id, body),
        cop::REQUEST_QUEST_ABORT => {
            super::quests::handle_request_quest_abort(world, client_id, body)
        }
        cop::REQUEST_PLEDGE_INFO => super::clans::handle_request_pledge_info(world, client_id, body),
        cop::REQUEST_JOIN_PARTY => handle_request_join_party(world, client_id, body),
        cop::REQUEST_ANSWER_JOIN_PARTY => handle_request_answer_join_party(world, client_id, body),
        cop::REQUEST_WITH_DRAWAL_PARTY => handle_request_withdrawal_party(world, client_id),
        cop::REQUEST_OUST_PARTY_MEMBER => handle_request_oust_party_member(world, client_id, body),
        cop::REQUEST_FRIEND_INVITE => handle_request_friend_invite(world, client_id, body),
        cop::REQUEST_ANSWER_FRIEND_INVITE => {
            handle_request_answer_friend_invite(world, client_id, body)
        }
        cop::REQUEST_FRIEND_LIST => handle_request_friend_list(world, client_id),
        cop::REQUEST_FRIEND_DEL => handle_request_friend_del(world, client_id, body),
        cop::REQUEST_SEND_FRIEND_MSG => handle_request_send_friend_msg(world, client_id, body),
        // RequestShowMiniMap (IN_GAME): empty body; open the world map.
        cop::REQUEST_SHOW_MINI_MAP => {
            if let Some(cs @ ClientSession::InGame(_)) = world.clients.get(&client_id) {
                cs.send(server_packets::show_mini_map(0));
            }
        }
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
    trace!("client {client_id} → ex-opcode 0x{sub:04x} ({} B)", ex_body.len());
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
        exop::REQUEST_SAVE_INVENTORY_ORDER => {
            handle_request_save_inventory_order(world, client_id, ex_body)
        }
        // RequestStopMove (IN_GAME): empty body; stop the walk at the current spot.
        exop::REQUEST_STOP_MOVE => handle_request_stop_move(world, client_id),
        // ExSendSelectedQuestZoneID (IN_GAME): store the selected quest zone id.
        exop::EX_SEND_SELECTED_QUEST_ZONE_ID => {
            handle_ex_send_selected_quest_zone_id(world, client_id, ex_body)
        }
        // RequestAllCastleInfo / RequestAllFortressInfo (IN_GAME): the world
        // map window asking for the ownership overlays. Java also refreshes
        // the requester's PartyMemberPosition on the castle request (the map
        // shows party member dots).
        exop::REQUEST_ALL_CASTLE_INFO => {
            if let Some(cs @ ClientSession::InGame(session)) = world.clients.get(&client_id) {
                cs.send(server_packets::ex_show_castle_info());
                let player_id = session.player_object_id();
                if let Some(&crate::model::components::PartyRef(party_id)) =
                    world.objects.get_component(&player_id)
                {
                    if let Some(party) = world.parties.get(&party_id) {
                        let locations: Vec<(i32, i32, i32, i32)> = party
                            .members
                            .iter()
                            .filter_map(|&m| {
                                world
                                    .objects
                                    .get_component::<crate::model::components::Position>(&m)
                                    .map(|p| (m, p.x, p.y, p.z))
                            })
                            .collect();
                        cs.send(server_packets::party_member_position(&locations));
                    }
                }
            }
        }
        exop::REQUEST_ALL_FORTRESS_INFO => {
            if let Some(cs @ ClientSession::InGame(_)) = world.clients.get(&client_id) {
                cs.send(server_packets::ex_show_fortress_info());
            }
        }
        exop::REQUEST_AUTO_SOULSHOT => {
            super::items::handle_request_auto_soul_shot(world, client_id, ex_body)
        }
        exop::REQUEST_EX_ADD_ENCHANT_SCROLL_ITEM => {
            super::enchant::handle_add_scroll(world, client_id, ex_body)
        }
        exop::REQUEST_EX_TRY_TO_PUT_ENCHANT_TARGET_ITEM => {
            super::enchant::handle_put_target(world, client_id, ex_body)
        }
        exop::REQUEST_EX_CANCEL_ENCHANT_ITEM => super::enchant::handle_cancel(world, client_id),
        exop::REQUEST_EX_TRY_TO_PUT_ENCHANT_SUPPORT_ITEM => {
            super::enchant::handle_put_support(world, client_id, ex_body)
        }
        exop::REQUEST_EX_REMOVE_ENCHANT_SUPPORT_ITEM => {
            super::enchant::handle_remove_support(world, client_id)
        }
        exop::REQUEST_CONFIRM_REFINER_ITEM => {
            super::augment::handle_confirm_refiner(world, client_id, ex_body)
        }
        exop::REQUEST_REFINE => super::augment::handle_refine(world, client_id, ex_body),
        exop::REQUEST_REFINE_CANCEL => super::augment::handle_refine_cancel(world, client_id, ex_body),
        exop::REQUEST_CHANGE_PARTY_LEADER => {
            handle_request_change_party_leader(world, client_id, ex_body)
        }
        exop::REQUEST_PARTY_LOOT_MODIFICATION => {
            handle_request_party_loot_modification(world, client_id, ex_body)
        }
        exop::ANSWER_PARTY_LOOT_MODIFICATION => {
            handle_answer_party_loot_modification(world, client_id, ex_body)
        }
        exop::REQUEST_VOTE_NEW => super::reco::handle_request_vote_new(world, client_id, ex_body),
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
