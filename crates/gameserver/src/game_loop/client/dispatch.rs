//! Opcode dispatch: route an inbound client packet (or `0xD0` ex-packet) to
//! its handler — the Rust face of Java's one-class-per-packet
//! `network/clientpackets` registry. Handlers small enough not to warrant a
//! module of their own live inline here.

use super::lobby;
use crate::game_loop::admin::refresh_skill_list;
use crate::game_loop::party;
use crate::game_loop::social::friends;
use crate::network::client_packets::{self as cp, ex_opcodes as exop, opcodes as cop};
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::World;
use tracing::{error, trace};

use super::bypass::handle_request_bypass_to_server;
use super::flood;
use crate::game_loop::combat::{duel, handle_attack_request};
use crate::game_loop::death::{handle_appearing, handle_request_restart_point};
use crate::game_loop::social::chat::{block_list, handle_say2};

use crate::game_loop::items::{
    augment, enchant, ground_items, handle_request_item_list, handle_request_save_inventory_order,
    handle_request_un_equip_item, handle_use_item, item_auction,
};

use crate::game_loop::net::{handle_logout, handle_request_restart};

use super::shortcuts::{
    handle_request_delete_macro, handle_request_make_macro, handle_request_short_cut_del,
    handle_request_short_cut_reg,
};
use crate::game_loop::combat::target::{handle_action, handle_request_target_canceld};
use crate::game_loop::skills::cast::handle_request_magic_skill_use;
use crate::game_loop::skills::handle_request_acquire_skill;
use crate::game_loop::skills::handle_request_dispel;
use crate::game_loop::space::position::{
    handle_ex_send_selected_quest_zone_id, handle_move_backward_to_location,
    handle_request_stop_move, handle_validate_position,
};

/// Dispatch one decrypted client packet body (opcode + payload) on the game
/// thread, gated by the client's session state (Java's per-`ConnectionState`
/// validity).
pub(crate) fn on_packet(world: &mut World, client_id: u32, data: Vec<u8>) {
    let Some(&opcode) = data.first() else { return };
    let body = &data[1..];
    trace!(
        "client {client_id} → opcode 0x{opcode:02x} ({} B)",
        data.len()
    );
    // `ClientPackets.newPacket`'s `Config.DEBUG_CLIENT_PACKETS` trace — the
    // opcode stands in for Java's packet class name, see `log_client_packet`.
    log_client_packet(world, opcode as u16, false);
    // The GM Debug panel's packet toggle (Java `Config.DEBUG_CLIENT_PACKETS`,
    // flipped at runtime by `//debug packets on|off`).
    if world.debug_packets {
        tracing::info!(
            "[packet-debug] client {client_id} → opcode 0x{opcode:02x} ({} B)",
            data.len()
        );
    }
    // Java calls the flood protector from inside each of 41 packet handlers;
    // this port has one dispatch site, so the check is table-driven here
    // (`flood::action_for_opcode`). Extended (`0xD0`) packets are charged in
    // `on_ex_packet`, once their sub-opcode is known.
    if let Some(action) = flood::action_for_opcode(opcode)
        && !flood::gate(world, client_id, action)
    {
        // Java `MultiSellChoose` is the one call site with a side effect on
        // rejection: the open list is dropped, so a flooded window cannot be
        // exchanged against once the burst subsides.
        if opcode == cop::MULTI_SELL_CHOOSE
            && let Some(player) = world.player_oid(client_id)
        {
            world
                .objects
                .remove_component::<crate::model::components::ActiveMultisell>(&player);
        }
        return;
    }
    // `Player.onActionRequest()` — Java calls it from exactly these five
    // packets (its sixth caller, `AutoPlayTaskManager`, is post-Interlude).
    // Kept as one hook rather than five scattered calls so the set stays
    // greppable; see `game_loop::spawn_protection`.
    if matches!(
        opcode,
        cop::ACTION
            | cop::ATTACK
            | cop::ATTACK_REQUEST
            | cop::MOVE_BACKWARD_TO_LOCATION
            | cop::REQUEST_MAGIC_SKILL_USE
            | cop::USE_ITEM
    ) && let Some(player) = world.player_oid(client_id)
    {
        crate::game_loop::combat::spawn_protection::on_action_request(world, client_id, player);
    }
    match opcode {
        cop::AUTH_LOGIN => lobby::handle_auth_login(world, client_id, body),
        cop::NEW_CHARACTER => lobby::handle_new_character(world, client_id),
        cop::CHARACTER_CREATE => lobby::handle_character_create(world, client_id, body),
        cop::CHARACTER_DELETE => lobby::handle_character_delete(world, client_id, body),
        cop::CHARACTER_RESTORE => lobby::handle_character_restore(world, client_id, body),
        cop::CHARACTER_SELECT => lobby::handle_character_select(world, client_id, body),
        cop::ENTER_WORLD => lobby::handle_enter_world(world, client_id),
        // Boats (G24.5): board / step off a ferry — boatId + (x, y, z).
        cop::REQUEST_GET_ON_VEHICLE => {
            crate::game_loop::space::boats::handle_get_on_off_vehicle(world, client_id, body, true)
        }
        cop::REQUEST_GET_OFF_VEHICLE => {
            crate::game_loop::space::boats::handle_get_on_off_vehicle(world, client_id, body, false)
        }
        // Boats: walk around on deck — boatId + target (x,y,z) + origin (x,y,z).
        cop::REQUEST_MOVE_TO_LOCATION_IN_VEHICLE => {
            crate::game_loop::space::boats::handle_move_in_vehicle(world, client_id, body)
        }
        // RequestSkillCoolTime (IN_GAME): resend the reuse timers.
        cop::REQUEST_SKILL_COOL_TIME => {
            if let Some(cs @ ClientSession::InGame(session)) = world.clients.get(&client_id)
                && let Some(reuses) = world
                    .objects
                    .get_component::<crate::model::components::Reuses>(&session.player_object_id())
            {
                cs.send(server_packets::skill_cool_time(reuses, world.tick));
            }
        }
        // RequestSkillList (IN_GAME): empty body, just `player.sendSkillList()`.
        cop::REQUEST_SKILL_LIST => {
            if let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) {
                refresh_skill_list(world, session.player_object_id());
            }
        }
        cop::REQUEST_ITEM_LIST => handle_request_item_list(world, client_id),
        cop::USE_ITEM => handle_use_item(world, client_id, body),
        cop::REQUEST_UN_EQUIP_ITEM => handle_request_un_equip_item(world, client_id, body),
        cop::REQUEST_DESTROY_ITEM => {
            crate::game_loop::items::handle_request_destroy_item(world, client_id, body)
        }
        cop::REQUEST_CRYSTALLIZE_ITEM => {
            crate::game_loop::items::handle_request_crystallize_item(world, client_id, body)
        }
        cop::REQUEST_DROP_ITEM => ground_items::handle_request_drop_item(world, client_id, body),
        cop::REQUEST_PACKAGE_SENDABLE_ITEM_LIST => {
            crate::game_loop::commerce::warehouse::handle_package_sendable_list(
                world, client_id, body,
            )
        }
        cop::REQUEST_PACKAGE_SEND => {
            crate::game_loop::commerce::warehouse::handle_package_send(world, client_id, body)
        }
        cop::REQUEST_BLOCK => block_list::handle_request_block(world, client_id, body),
        cop::SEND_WARE_HOUSE_DEPOSIT_LIST => {
            crate::game_loop::commerce::warehouse::handle_deposit(world, client_id, body)
        }
        cop::SEND_WARE_HOUSE_WITH_DRAW_LIST => {
            crate::game_loop::commerce::warehouse::handle_withdraw(world, client_id, body)
        }
        cop::REQUEST_ENCHANT_ITEM => enchant::handle_enchant(world, client_id, body),
        cop::REQUEST_MAGIC_SKILL_USE => handle_request_magic_skill_use(world, client_id, body),
        cop::REQUEST_ACQUIRE_SKILL => handle_request_acquire_skill(world, client_id, body),
        cop::REQUEST_ACQUIRE_SKILL_INFO => {
            // `RequestAcquireSkillInfo` (ddd): only the PLEDGE branch is
            // answered (the class flow works off the enter-world skill list).
            // The SUBPLEDGE branch stays unanswered on purpose: squad skills
            // (`subPledgeSkillTree.xml`) need clan level 8+ and Knight's
            // Epaulettes (9910/9911), and the tree's own comment marks it
            // "Confirmed CT2.5" — no Interlude clan can reach it.
            let mut r = commons::network::PacketReader::new(body);
            if let (Some(id), Some(level), Some(kind)) = (r.read_i32(), r.read_i32(), r.read_i32())
                && kind == crate::network::client_packets::RequestAcquireSkill::PLEDGE
            {
                crate::game_loop::clans::handle_request_pledge_skill_info(
                    world, client_id, id, level,
                );
            }
        }
        cop::ACTION => handle_action(world, client_id, body),
        cop::ATTACK | cop::ATTACK_REQUEST => handle_attack_request(world, client_id, body),
        cop::APPEARING => handle_appearing(world, client_id),
        cop::REQUEST_RESTART_POINT => handle_request_restart_point(world, client_id, body),
        cop::REQUEST_TARGET_CANCELD => handle_request_target_canceld(world, client_id, body),
        cop::MOVE_BACKWARD_TO_LOCATION => handle_move_backward_to_location(world, client_id, body),
        cop::VALIDATE_POSITION => handle_validate_position(world, client_id, body),
        cop::CANNOT_MOVE_ANYMORE => {
            crate::game_loop::space::position::handle_cannot_move_anymore(world, client_id, body)
        }
        // `RequestSiegeInfo` is an **empty handler** in this Java build (both
        // `readImpl` and `runImpl` are no-ops) — the `SiegeInfo` window is
        // pushed by the castle Siege Manager's bypass instead. Accepted and
        // dropped, exactly as Java does.
        cop::REQUEST_SIEGE_INFO => {}
        cop::REQUEST_SHORT_CUT_REG => handle_request_short_cut_reg(world, client_id, body),
        cop::REQUEST_SHORT_CUT_DEL => handle_request_short_cut_del(world, client_id, body),
        cop::REQUEST_MAKE_MACRO => handle_request_make_macro(world, client_id, body),
        cop::REQUEST_DELETE_MACRO => handle_request_delete_macro(world, client_id, body),
        cop::SAY2 => handle_say2(world, client_id, body),
        cop::REQUEST_PETITION => {
            crate::game_loop::moderation::petition::on_request_petition(world, client_id, body)
        }
        cop::REQUEST_PETITION_CANCEL => {
            crate::game_loop::moderation::petition::on_request_petition_cancel(world, client_id)
        }
        cop::REQUEST_PETITION_FEEDBACK => {
            crate::game_loop::moderation::petition::on_request_petition_feedback(
                world, client_id, body,
            )
        }
        cop::REQUEST_BYPASS_TO_SERVER => handle_request_bypass_to_server(world, client_id, body),
        // RequestSiegeAttackerList / RequestSiegeDefenderList (G24): view a
        // castle's registered attackers / owner + defenders.
        cop::REQUEST_SIEGE_ATTACKER_LIST => {
            crate::game_loop::siege::handle_request_siege_attacker_list(world, client_id, body)
        }
        cop::REQUEST_SIEGE_DEFENDER_LIST => {
            crate::game_loop::siege::handle_request_siege_defender_list(world, client_id, body)
        }
        // RequestJoinSiege (G24): a clan leader registers/cancels for a siege.
        cop::REQUEST_JOIN_SIEGE => {
            crate::game_loop::siege::handle_request_join_siege(world, client_id, body)
        }
        // RequestConfirmSiegeWaitingList (G24): the owner approves/rejects a
        // pending defender clan.
        cop::REQUEST_CONFIRM_SIEGE_WAITING_LIST => {
            crate::game_loop::siege::handle_request_confirm_siege_waiting_list(
                world, client_id, body,
            )
        }
        // RequestSetCastleSiegeTime (G24): the owner picks the siege hour.
        cop::REQUEST_SET_CASTLE_SIEGE_TIME => {
            crate::game_loop::siege::handle_request_set_castle_siege_time(world, client_id, body)
        }
        // RequestShowBoard (IN_GAME): the community-board button → open at
        // `BBSDefault` (`_bbshome`). Body is one unused int.
        cop::REQUEST_SHOW_BOARD => {
            let command = world.cfg.community_board.bbs_default.clone();
            crate::game_loop::community_board::handle_parse_command(world, client_id, &command);
        }
        // RequestBBSwrite (IN_GAME): a board write/submit.
        cop::REQUEST_BBS_WRITE => {
            if let Some([url, a1, a2, a3, a4, a5]) = cp::read_bbs_write(body) {
                crate::game_loop::community_board::handle_write_command(
                    world,
                    client_id,
                    &url,
                    &[a1, a2, a3, a4, a5],
                );
            }
        }
        // SendBypassBuildCmd (IN_GAME): the `//command` GM bar → admin command
        // with the `admin_` prefix Java prepends.
        cop::SEND_BYPASS_BUILD_CMD => {
            if let Some(cmd) = cp::read_build_command(body)
                && !cmd.is_empty()
            {
                crate::game_loop::admin::use_admin_command(
                    world,
                    client_id,
                    &format!("admin_{cmd}"),
                    true,
                );
            }
        }
        // BypassUserCmd (IN_GAME): the client `/command` bar (`/loc`,
        // `/unstuck`, …).
        cop::BYPASS_USER_CMD => {
            super::user_commands::handle_bypass_user_cmd(world, client_id, body)
        }
        // DlgAnswer (IN_GAME): reply to a ConfirmDlg. Several flows share the
        // packet — a resurrection proposal, a Summon Friend prompt, `.offline`
        // and the admin confirm — so each claimant reports whether the reply
        // was its own and the admin handler takes what is left.
        cop::DLG_ANSWER => dispatch_dlg_answer(world, client_id, body),
        // RequestActionUse (IN_GAME): the action bar's non-skill buttons,
        // dispatched through `ActionData.xml`'s handler table
        // (`player_actions`) exactly as Java's `PlayerActionHandler` map is.
        cop::REQUEST_ACTION_USE => {
            super::actions::handle_request_action_use(world, client_id, body)
        }
        cop::REQUEST_GIVE_ITEM_TO_PET => {
            crate::game_loop::servitor::handle_give_item_to_pet(world, client_id, body)
        }
        cop::REQUEST_GET_ITEM_FROM_PET => {
            crate::game_loop::servitor::handle_get_item_from_pet(world, client_id, body)
        }
        cop::REQUEST_PET_USE_ITEM => {
            crate::game_loop::servitor::handle_pet_use_item(world, client_id, body)
        }
        cop::REQUEST_BUY_ITEM => {
            crate::game_loop::commerce::shop::handle_request_buy_item(world, client_id, body)
        }
        cop::REQUEST_SELL_ITEM => {
            crate::game_loop::commerce::shop::handle_request_sell_item(world, client_id, body)
        }
        // RequestBuySeed (IN_GAME): a player buys seeds at a Manor Manager.
        // Gated on `AllowManor` (off on this dist) like the rest of the system.
        cop::REQUEST_BUY_SEED if world.cfg.general.allow_manor => {
            crate::game_loop::manor::handle_request_buy_seed(world, client_id, body)
        }
        cop::MULTI_SELL_CHOOSE => {
            crate::game_loop::commerce::multisell::handle_multi_sell_choose(world, client_id, body)
        }
        cop::REQUEST_PRIVATE_STORE_MANAGE_SELL => {
            crate::game_loop::commerce::private_store::open_manage(world, client_id)
        }
        cop::SET_PRIVATE_STORE_LIST_SELL => {
            crate::game_loop::commerce::private_store::handle_set_list(world, client_id, body)
        }
        cop::REQUEST_PRIVATE_STORE_QUIT_SELL => {
            crate::game_loop::commerce::private_store::handle_quit(world, client_id)
        }
        cop::REQUEST_PRIVATE_STORE_BUY => {
            crate::game_loop::commerce::private_store::handle_buy(world, client_id, body)
        }
        // The buy-store half (G15's deferred sibling, landed with the
        // remaining-ports audit's row 6).
        cop::REQUEST_PRIVATE_STORE_MANAGE_BUY => {
            crate::game_loop::commerce::private_store::open_manage_buy(world, client_id)
        }
        cop::SET_PRIVATE_STORE_LIST_BUY => {
            crate::game_loop::commerce::private_store::handle_set_list_buy(world, client_id, body)
        }
        cop::REQUEST_PRIVATE_STORE_QUIT_BUY => {
            crate::game_loop::commerce::private_store::handle_quit_buy(world, client_id)
        }
        cop::REQUEST_PRIVATE_STORE_SELL => {
            crate::game_loop::commerce::private_store::handle_store_sell(world, client_id, body)
        }
        // Store titles, both kinds.
        cop::SET_PRIVATE_STORE_MSG_SELL => {
            crate::game_loop::commerce::private_store::handle_set_msg(world, client_id, body, false)
        }
        cop::SET_PRIVATE_STORE_MSG_BUY => {
            crate::game_loop::commerce::private_store::handle_set_msg(world, client_id, body, true)
        }
        cop::TRADE_REQUEST => {
            crate::game_loop::commerce::trade::handle_request(world, client_id, body)
        }
        cop::ANSWER_TRADE_REQUEST => {
            crate::game_loop::commerce::trade::handle_answer(world, client_id, body)
        }
        cop::ADD_TRADE_ITEM => {
            crate::game_loop::commerce::trade::handle_add_item(world, client_id, body)
        }
        cop::TRADE_DONE => crate::game_loop::commerce::trade::handle_done(world, client_id, body),
        // ObserverReturn (IN_GAME): leave the Broadcasting Tower's spectator
        // mode. The Olympiad's viewer answers `RequestOlympiadObserverEnd`
        // instead, and each handler ignores the other's state.
        cop::OBSERVER_RETURN => {
            if let Some(player) = world.player_oid(client_id) {
                crate::game_loop::space::observation::handle_observer_return(
                    world, client_id, player,
                );
            }
        }
        cop::REQUEST_QUEST_LIST => {
            crate::game_loop::quests::handle_request_quest_list(world, client_id)
        }
        cop::REQUEST_QUEST_ABORT => {
            crate::game_loop::quests::handle_request_quest_abort(world, client_id, body)
        }
        // Tutorial windows (Q255): link clicks and bypass presses share the
        // same router; question-mark clicks fire the global mark event; the
        // client-event echo is dead in this build (its Java handler looks the
        // quest up under a wrong name and always misses) — consumed silently.
        cop::REQUEST_TUTORIAL_LINK_HTML => {
            if let Some(pkt) = cp::RequestTutorialLinkHtml::read(body) {
                crate::game_loop::quests::handle_tutorial_bypass(world, client_id, &pkt.bypass);
            }
        }
        cop::REQUEST_TUTORIAL_PASS_CMD_TO_SERVER => {
            if let Some(pkt) = cp::RequestTutorialPassCmd::read(body) {
                crate::game_loop::quests::handle_tutorial_bypass(world, client_id, &pkt.bypass);
            }
        }
        cop::REQUEST_TUTORIAL_QUESTION_MARK => {
            if let Some(pkt) = cp::RequestTutorialQuestionMark::read(body)
                && let Some(ClientSession::InGame(s)) = world.clients.get(&client_id)
            {
                let player = s.player_object_id();
                crate::game_loop::quests::notify_tutorial_mark(
                    world, client_id, player, pkt.number,
                );
            }
        }
        cop::REQUEST_TUTORIAL_CLIENT_EVENT => {}
        cop::REQUEST_PLEDGE_INFO => {
            crate::game_loop::clans::handle_request_pledge_info(world, client_id, body)
        }
        cop::REQUEST_JOIN_PLEDGE => {
            crate::game_loop::clans::handle_request_join_pledge(world, client_id, body)
        }
        cop::REQUEST_ANSWER_JOIN_PLEDGE => {
            crate::game_loop::clans::handle_request_answer_join_pledge(world, client_id, body)
        }
        cop::REQUEST_WITHDRAWAL_PLEDGE => {
            crate::game_loop::clans::handle_request_withdrawal_pledge(world, client_id)
        }
        cop::REQUEST_OUST_PLEDGE_MEMBER => {
            crate::game_loop::clans::handle_request_oust_pledge_member(world, client_id, body)
        }
        cop::REQUEST_PLEDGE_POWER => {
            crate::game_loop::clans::handle_request_pledge_power(world, client_id, body)
        }
        cop::REQUEST_START_PLEDGE_WAR => {
            crate::game_loop::clans::handle_request_start_pledge_war(world, client_id, body)
        }
        cop::REQUEST_STOP_PLEDGE_WAR => {
            crate::game_loop::clans::handle_request_stop_pledge_war(world, client_id, body)
        }
        cop::REQUEST_SURRENDER_PLEDGE_WAR => {
            crate::game_loop::clans::handle_request_surrender_pledge_war(world, client_id, body)
        }
        cop::REQUEST_PLEDGE_MEMBER_LIST => {
            crate::game_loop::clans::handle_request_pledge_member_list(world, client_id)
        }
        cop::REQUEST_MAGIC_SKILL_LIST => {
            crate::game_loop::skills::handle_request_magic_skill_list(world, client_id, body)
        }
        cop::REQUEST_GM_LIST => crate::game_loop::admin::handle_request_gm_list(world, client_id),
        cop::SNOOP_QUIT => {
            crate::game_loop::social::chat::handle_snoop_quit(world, client_id, body)
        }
        cop::REQUEST_GIVE_NICK_NAME => {
            crate::game_loop::clans::handle_request_give_nick_name(world, client_id, body)
        }
        cop::REQUEST_LINK_HTML => super::bypass::handle_request_link_html(world, client_id, body),
        cop::REQUEST_PET_GET_ITEM => {
            crate::game_loop::servitor::handle_request_pet_get_item(world, client_id, body)
        }
        cop::REQUEST_PREVIEW_ITEM => {
            crate::game_loop::commerce::shop::handle_request_preview_item(world, client_id, body)
        }
        cop::REQUEST_GM_COMMAND => {
            crate::game_loop::admin::handle_request_gm_command(world, client_id, body)
        }
        cop::START_ROTATING => {
            crate::game_loop::space::position::handle_start_rotating(world, client_id, body)
        }
        cop::FINISH_ROTATING => {
            crate::game_loop::space::position::handle_finish_rotating(world, client_id, body)
        }
        cop::CANNOT_MOVE_ANYMORE_IN_VEHICLE => {
            crate::game_loop::space::position::handle_cannot_move_anymore_in_vehicle(
                world, client_id, body,
            )
        }
        cop::REQUEST_RECIPE_SHOP_MANAGE_PREV => {
            crate::game_loop::commerce::crafting::handle_request_recipe_shop_manage_prev(
                world, client_id,
            )
        }
        cop::REQUEST_ALLY_INFO => {
            crate::game_loop::clans::handle_request_ally_info(world, client_id)
        }
        cop::REQUEST_SET_PLEDGE_CREST => {
            crate::game_loop::clans::handle_request_set_pledge_crest(world, client_id, body)
        }
        cop::REQUEST_PLEDGE_CREST => {
            crate::game_loop::clans::handle_request_pledge_crest(world, client_id, body)
        }
        cop::REQUEST_SET_ALLY_CREST => {
            crate::game_loop::clans::handle_request_set_ally_crest(world, client_id, body)
        }
        cop::REQUEST_ALLY_CREST => {
            crate::game_loop::clans::handle_request_ally_crest(world, client_id, body)
        }
        cop::REQUEST_JOIN_ALLY => {
            crate::game_loop::clans::handle_request_join_ally(world, client_id, body)
        }
        cop::REQUEST_ANSWER_JOIN_ALLY => {
            crate::game_loop::clans::handle_request_answer_join_ally(world, client_id, body)
        }
        cop::ALLY_LEAVE => crate::game_loop::clans::handle_ally_leave(world, client_id),
        cop::ALLY_DISMISS => crate::game_loop::clans::handle_ally_dismiss(world, client_id, body),
        cop::REQUEST_DISMISS_ALLY => {
            crate::game_loop::clans::handle_request_dismiss_ally(world, client_id)
        }
        cop::REQUEST_JOIN_PARTY => party::handle_request_join_party(world, client_id, body),
        cop::REQUEST_ANSWER_JOIN_PARTY => {
            party::handle_request_answer_join_party(world, client_id, body)
        }
        cop::REQUEST_WITH_DRAWAL_PARTY => party::handle_request_withdrawal_party(world, client_id),
        cop::REQUEST_OUST_PARTY_MEMBER => {
            party::handle_request_oust_party_member(world, client_id, body)
        }
        cop::REQUEST_PARTY_MATCH_CONFIG => {
            crate::game_loop::party::rooms::handle_request_party_match_config(
                world, client_id, body,
            )
        }
        cop::REQUEST_PARTY_MATCH_LIST => {
            crate::game_loop::party::rooms::handle_request_party_match_list(world, client_id, body)
        }
        cop::REQUEST_PARTY_MATCH_DETAIL => {
            crate::game_loop::party::rooms::handle_request_party_match_detail(
                world, client_id, body,
            )
        }
        cop::REQUEST_FRIEND_INVITE => friends::handle_request_friend_invite(world, client_id, body),
        cop::REQUEST_ANSWER_FRIEND_INVITE => {
            friends::handle_request_answer_friend_invite(world, client_id, body)
        }
        cop::REQUEST_FRIEND_LIST => friends::handle_request_friend_list(world, client_id),
        cop::REQUEST_FRIEND_DEL => friends::handle_request_friend_del(world, client_id, body),
        cop::REQUEST_SEND_FRIEND_MSG => {
            friends::handle_request_send_friend_msg(world, client_id, body)
        }
        // RequestShowMiniMap (IN_GAME): empty body; open the world map.
        cop::REQUEST_SHOW_MINI_MAP => {
            if let Some(cs @ ClientSession::InGame(_)) = world.clients.get(&client_id) {
                cs.send(server_packets::show_mini_map(0));
            }
        }
        // RequestRecipeBookOpen (IN_GAME): the "Common Craft" / "Dwarven Craft"
        // action. Body is one int (`0` = dwarven).
        cop::REQUEST_RECIPE_BOOK_OPEN => {
            let is_dwarven = body
                .get(..4)
                .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]) == 0)
                .unwrap_or(true);
            crate::game_loop::commerce::crafting::request_book_open(world, client_id, is_dwarven);
        }
        cop::REQUEST_RECIPE_BOOK_DESTROY => {
            if let Some(id) = cp::read_recipe_single_int(body) {
                crate::game_loop::commerce::crafting::handle_book_destroy(world, client_id, id);
            }
        }
        cop::REQUEST_RECIPE_ITEM_MAKE_INFO => {
            if let Some(id) = cp::read_recipe_single_int(body) {
                crate::game_loop::commerce::crafting::handle_make_info(world, client_id, id);
            }
        }
        cop::REQUEST_RECIPE_ITEM_MAKE_SELF => {
            if let Some(id) = cp::read_recipe_single_int(body) {
                crate::game_loop::commerce::crafting::handle_make_self(world, client_id, id);
            }
        }
        cop::REQUEST_RECIPE_SHOP_MANAGE_LIST => {
            crate::game_loop::commerce::crafting::open_manage(world, client_id)
        }
        cop::REQUEST_RECIPE_SHOP_MESSAGE_SET => {
            if let Some(name) = cp::read_recipe_shop_message_set(body) {
                crate::game_loop::commerce::crafting::handle_message_set(world, client_id, name);
            }
        }
        cop::REQUEST_RECIPE_SHOP_LIST_SET => {
            if let Some(lines) = cp::read_recipe_shop_list_set(body) {
                crate::game_loop::commerce::crafting::handle_list_set(world, client_id, lines);
            }
        }
        cop::REQUEST_RECIPE_SHOP_MANAGE_QUIT => {
            crate::game_loop::commerce::crafting::handle_manage_quit(world, client_id)
        }
        cop::REQUEST_RECIPE_SHOP_MAKE_INFO => {
            if let Some((shop, recipe)) = cp::read_recipe_shop_make_info(body) {
                crate::game_loop::commerce::crafting::handle_shop_make_info(
                    world, client_id, shop, recipe,
                );
            }
        }
        cop::REQUEST_RECIPE_SHOP_MAKE_ITEM => {
            if let Some((manufacturer, recipe)) = cp::read_recipe_shop_make_item(body) {
                crate::game_loop::commerce::crafting::handle_shop_make_item(
                    world,
                    client_id,
                    manufacturer,
                    recipe,
                );
            }
        }
        // Henna / dye symbols (G16).
        cop::REQUEST_HENNA_ITEM_LIST => {
            crate::game_loop::character::henna::handle_item_list(world, client_id)
        }
        cop::REQUEST_HENNA_REMOVE_LIST => {
            crate::game_loop::character::henna::handle_remove_list(world, client_id)
        }
        cop::REQUEST_HENNA_ITEM_INFO => {
            if let Some(id) = cp::read_symbol_id(body) {
                crate::game_loop::character::henna::handle_item_info(world, client_id, id);
            }
        }
        cop::REQUEST_HENNA_ITEM_REMOVE_INFO => {
            if let Some(id) = cp::read_symbol_id(body) {
                crate::game_loop::character::henna::handle_item_remove_info(world, client_id, id);
            }
        }
        cop::REQUEST_HENNA_EQUIP => {
            if let Some(id) = cp::read_symbol_id(body) {
                crate::game_loop::character::henna::handle_equip(world, client_id, id);
            }
        }
        cop::REQUEST_HENNA_REMOVE => {
            if let Some(id) = cp::read_symbol_id(body) {
                crate::game_loop::character::henna::handle_remove(world, client_id, id);
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
    // `DEBUG_EX_CLIENT_PACKETS`, the extended half of the trace above.
    log_client_packet(world, sub, true);
    trace!(
        "client {client_id} → ex-opcode 0x{sub:04x} ({} B)",
        ex_body.len()
    );
    if let Some(action) = flood::action_for_ex_opcode(sub)
        && !flood::gate(world, client_id, action)
    {
        return;
    }
    match sub {
        exop::REQUEST_CHARACTER_NAME_CREATABLE => {
            lobby::handle_request_character_name_creatable(world, client_id, ex_body)
        }
        // `RequestKeyMapping` (ENTERING + IN_GAME): replay the stored UI key
        // mapping. Java answers nothing at all when `StoreCharUiSettings` is
        // off — not an empty layout — so the whole reply is gated, not just its
        // contents. A character who never saved one gets Java's empty payload.
        exop::REQUEST_KEY_MAPPING if world.cfg.character.store_ui_settings => {
            let mapping = super::settings::stored_key_mapping(world, client_id);
            if let Some(cs) = world.clients.get(&client_id)
                && matches!(cs, ClientSession::Entering(_) | ClientSession::InGame(_))
            {
                cs.send(server_packets::ex_ui_setting(&mapping));
            }
        }
        // `RequestSaveKeyMapping` opens with `if (!STORE_UI_SETTINGS … ) return`
        // — with the key off the layout is dropped rather than stored.
        exop::REQUEST_SAVE_KEY_MAPPING if world.cfg.character.store_ui_settings => {
            super::settings::handle_save_key_mapping(world, client_id, ex_body)
        }
        exop::REQUEST_CONFIRM_TARGET_ITEM => {
            augment::handle_confirm_target_item(world, client_id, ex_body)
        }
        exop::REQUEST_CONFIRM_GEMSTONE => {
            augment::handle_confirm_gemstone(world, client_id, ex_body)
        }
        exop::REQUEST_CONFIRM_CANCEL_ITEM => {
            augment::handle_confirm_cancel_item(world, client_id, ex_body)
        }
        // RequestManorList (IN_GAME): the castles that offer a manor — the seed
        // catalogue's castles when manor is enabled, else an empty list.
        exop::REQUEST_MANOR_LIST => {
            let castle_ids = if world.cfg.general.allow_manor {
                world.data.manor.manor_castle_ids()
            } else {
                Vec::new()
            };
            if let Some(cs @ ClientSession::InGame(_)) = world.clients.get(&client_id) {
                cs.send(server_packets::ex_send_manor_list(&castle_ids));
            }
        }
        // RequestProcureCropList (IN_GAME): a player sells crops at a Manor
        // Manager.
        exop::REQUEST_PROCURE_CROP_LIST if world.cfg.general.allow_manor => {
            crate::game_loop::manor::handle_request_procure_crop_list(world, client_id, ex_body);
        }
        // RequestSetSeed / RequestSetCrop (IN_GAME): the manor owner submits the
        // next-period seed/crop setup through the chamberlain's edit windows.
        // Gated on `AllowManor` (off on this dist) like the rest of the system.
        exop::REQUEST_SET_SEED if world.cfg.general.allow_manor => {
            crate::game_loop::manor::handle_request_set_seed(world, client_id, ex_body);
        }
        exop::REQUEST_SET_CROP if world.cfg.general.allow_manor => {
            crate::game_loop::manor::handle_request_set_crop(world, client_id, ex_body);
        }
        // EndScenePlayer / RequestExEscapeScene (IN_GAME): a cinematic ended
        // on its own, or the player pressed Esc during an escapable one
        // (`//playmovie` is the only movie route on this dist).
        exop::END_SCENE_PLAYER => {
            crate::game_loop::admin::effects::handle_end_scene_player(world, client_id, ex_body);
        }
        exop::REQUEST_EX_ESCAPE_SCENE => {
            crate::game_loop::admin::effects::handle_escape_scene(world, client_id);
        }
        // RequestUserBanInfo (IN_GAME): consumed silently, which is exactly
        // what Java does — `ExClientPackets` registers it as
        // `REQUEST_USER_BAN_INFO(0x138, null, IN_GAME)`, a **null** handler
        // factory. Its answer `ExUserBanInfo` (0xFE 0x1D1) exists only as a
        // `ServerPackets` enum entry: no such class is in the tree, so nothing
        // upstream ever builds or sends one. There is nothing here to port
        // (verified 2026-08-07).
        exop::REQUEST_USER_BAN_INFO => {}
        // ExSendClientIni (AUTHENTICATED): the client reports its client.ini
        // after auth; Mobius registers a null handler, so consume it silently.
        exop::EX_SEND_CLIENT_INI => {}
        // RequestHardWareInfo (G31): store the client's hardware fingerprint,
        // then apply any HWID punishment now known to match (the packet can
        // arrive after enter-world, so re-check here rather than only on login).
        exop::REQUEST_HARDWARE_INFO => {
            if let Some(hw) = cp::HardwareInfo::read(ex_body) {
                world.hwids.insert(client_id, hw);
                crate::game_loop::moderation::punishment::on_hwid_received(world, client_id);
            }
        }
        // Olympiad observer mode (G25): leave observing, or (re)open the
        // ongoing-match list. The list request/refresh just re-sends it.
        exop::REQUEST_OLYMPIAD_OBSERVER_END => {
            if let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) {
                let player = session.player_object_id();
                // Java's `if player.inObserverMode()` guard.
                if crate::game_loop::olympiad::is_observing(world, player) {
                    crate::game_loop::olympiad::leave_observer(world, client_id, player);
                }
            }
        }
        exop::REQUEST_OLYMPIAD_MATCH_LIST | exop::REQUEST_EX_OLYMPIAD_MATCH_LIST_REFRESH => {
            crate::game_loop::olympiad::send_match_list(world, client_id);
        }
        // RequestExMagicSkillUseGround (IN_GAME): a GROUND-target cast aimed
        // at a world position (G19).
        exop::REQUEST_CURSED_WEAPON_LIST => {
            crate::game_loop::items::cursed_weapon::handle_request_list(world, client_id)
        }
        exop::REQUEST_CURSED_WEAPON_LOCATION => {
            crate::game_loop::items::cursed_weapon::handle_request_location(world, client_id)
        }
        exop::SET_PRIVATE_STORE_WHOLE_MSG => {
            crate::game_loop::commerce::private_store::handle_set_whole_msg(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_EX_MAGIC_SKILL_USE_GROUND => {
            crate::game_loop::skills::cast::handle_request_magic_skill_use_ground(
                world, client_id, ex_body,
            )
        }
        // Skill enchanting (G19): the enchant window's info/detail queries and
        // the enchant itself.
        exop::REQUEST_EX_ENCHANT_SKILL_INFO => {
            crate::game_loop::skills::enchant::handle_request_enchant_skill_info(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_EX_ENCHANT_SKILL => {
            crate::game_loop::skills::enchant::handle_request_enchant_skill(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_EX_ENCHANT_SKILL_INFO_DETAIL => {
            crate::game_loop::skills::enchant::handle_request_enchant_skill_info_detail(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_DUEL_START => duel::handle_request_duel_start(world, client_id, ex_body),
        exop::REQUEST_DUEL_ANSWER_START => {
            duel::handle_request_duel_answer(world, client_id, ex_body)
        }
        exop::REQUEST_DUEL_SURRENDER => duel::handle_request_duel_surrender(world, client_id),
        exop::REQUEST_SAVE_INVENTORY_ORDER => {
            handle_request_save_inventory_order(world, client_id, ex_body)
        }
        // RequestExRqItemLink (IN_GAME): the "?" on a shift-clicked item link
        // in chat was clicked; answer with that item's row.
        exop::REQUEST_EX_RQ_ITEM_LINK => {
            crate::game_loop::social::chat::handle_request_item_link(world, client_id, ex_body)
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
                cs.send(server_packets::ex_show_castle_info(world));
                let player_id = session.player_object_id();
                if let Some(&crate::model::components::PartyRef(party_id)) =
                    world.objects.get_component(&player_id)
                    && let Some(party) = world.parties.get(&party_id)
                {
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
        exop::REQUEST_ALL_FORTRESS_INFO => {
            if let Some(cs @ ClientSession::InGame(_)) = world.clients.get(&client_id) {
                cs.send(server_packets::ex_show_fortress_info());
            }
        }
        exop::REQUEST_AUTO_SOULSHOT => {
            crate::game_loop::items::handle_request_auto_soul_shot(world, client_id, ex_body)
        }
        // ExRequestAutoFish (IN_GAME): toggle auto-fishing.
        exop::REQUEST_EX_AUTO_FISH => {
            if let Some(ClientSession::InGame(s)) = world.clients.get(&client_id) {
                let player = s.player_object_id();
                crate::game_loop::activities::fishing::toggle_fishing(world, player);
            }
        }
        // Item auction (G30.5): bid on / request info about an auction.
        exop::REQUEST_POST_ITEM_LIST => {
            crate::game_loop::mail::handle_post_item_list(world, client_id)
        }
        exop::REQUEST_RECEIVED_POST_LIST => {
            crate::game_loop::mail::handle_received_post_list(world, client_id)
        }
        exop::REQUEST_SENT_POST_LIST => {
            crate::game_loop::mail::handle_sent_post_list(world, client_id)
        }
        exop::REQUEST_SEND_POST => {
            crate::game_loop::mail::handle_send_post(world, client_id, ex_body)
        }
        exop::REQUEST_RECEIVED_POST => {
            crate::game_loop::mail::handle_received_post(world, client_id, ex_body)
        }
        exop::REQUEST_SENT_POST => {
            crate::game_loop::mail::handle_sent_post(world, client_id, ex_body)
        }
        exop::REQUEST_DELETE_RECEIVED_POST => {
            crate::game_loop::mail::handle_delete_received_post(world, client_id, ex_body)
        }
        exop::REQUEST_DELETE_SENT_POST => {
            crate::game_loop::mail::handle_delete_sent_post(world, client_id, ex_body)
        }
        exop::REQUEST_POST_ATTACHMENT => {
            crate::game_loop::mail::handle_post_attachment(world, client_id, ex_body)
        }
        exop::REQUEST_REJECT_POST_ATTACHMENT => {
            crate::game_loop::mail::handle_reject_post_attachment(world, client_id, ex_body)
        }
        exop::REQUEST_CANCEL_POST_ATTACHMENT => {
            crate::game_loop::mail::handle_cancel_post_attachment(world, client_id, ex_body)
        }
        exop::REQUEST_OUST_FROM_PARTY_ROOM => {
            crate::game_loop::party::rooms::handle_oust_from_party_room(world, client_id, ex_body)
        }
        exop::REQUEST_DISMISS_PARTY_ROOM => {
            crate::game_loop::party::rooms::handle_dismiss_party_room(world, client_id, ex_body)
        }
        exop::REQUEST_WITHDRAW_PARTY_ROOM => {
            crate::game_loop::party::rooms::handle_withdraw_party_room(world, client_id, ex_body)
        }
        exop::REQUEST_ASK_JOIN_PARTY_ROOM => {
            crate::game_loop::party::rooms::handle_ask_join_party_room(world, client_id, ex_body)
        }
        exop::ANSWER_JOIN_PARTY_ROOM => {
            crate::game_loop::party::rooms::handle_answer_join_party_room(world, client_id, ex_body)
        }
        exop::REQUEST_EXIT_PARTY_MATCHING_WAITING_ROOM => {
            crate::game_loop::party::rooms::handle_exit_waiting_room(world, client_id)
        }
        exop::REQUEST_LIST_PARTY_MATCHING_WAITING_ROOM => {
            crate::game_loop::party::rooms::handle_list_waiting_room(world, client_id, ex_body)
        }
        exop::REQUEST_BID_ITEM_AUCTION => item_auction::on_request_bid(world, client_id, ex_body),
        exop::REQUEST_INFO_ITEM_AUCTION => item_auction::on_request_info(world, client_id, ex_body),
        exop::REQUEST_EX_ADD_ENCHANT_SCROLL_ITEM => {
            enchant::handle_add_scroll(world, client_id, ex_body)
        }
        exop::REQUEST_EX_TRY_TO_PUT_ENCHANT_TARGET_ITEM => {
            enchant::handle_put_target(world, client_id, ex_body)
        }
        exop::REQUEST_EX_CANCEL_ENCHANT_ITEM => enchant::handle_cancel(world, client_id),
        exop::REQUEST_EX_TRY_TO_PUT_ENCHANT_SUPPORT_ITEM => {
            enchant::handle_put_support(world, client_id, ex_body)
        }
        exop::REQUEST_EX_REMOVE_ENCHANT_SUPPORT_ITEM => {
            enchant::handle_remove_support(world, client_id)
        }
        exop::REQUEST_CONFIRM_REFINER_ITEM => {
            augment::handle_confirm_refiner(world, client_id, ex_body)
        }
        exop::REQUEST_REFINE => augment::handle_refine(world, client_id, ex_body),
        exop::REQUEST_REFINE_CANCEL => augment::handle_refine_cancel(world, client_id, ex_body),
        exop::REQUEST_CHANGE_PARTY_LEADER => {
            party::handle_request_change_party_leader(world, client_id, ex_body)
        }
        exop::REQUEST_PARTY_LOOT_MODIFICATION => {
            party::handle_request_party_loot_modification(world, client_id, ex_body)
        }
        exop::ANSWER_PARTY_LOOT_MODIFICATION => {
            party::handle_answer_party_loot_modification(world, client_id, ex_body)
        }
        exop::REQUEST_VOTE_NEW => {
            crate::game_loop::character::reco::handle_request_vote_new(world, client_id, ex_body)
        }
        // Clan ranks & power grades (G18 slice 3).
        exop::REQUEST_PLEDGE_POWER_GRADE_LIST => {
            crate::game_loop::clans::handle_request_pledge_power_grade_list(world, client_id)
        }
        exop::REQUEST_PLEDGE_MEMBER_POWER_INFO => {
            crate::game_loop::clans::handle_request_pledge_member_power_info(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_PLEDGE_SET_ACADEMY_MASTER => {
            crate::game_loop::clans::academy::handle_set_academy_master(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_SET_MEMBER_POWER_GRADE => {
            crate::game_loop::clans::handle_request_pledge_set_member_power_grade(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_PLEDGE_MEMBER_INFO => {
            crate::game_loop::clans::handle_request_pledge_member_info(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_REORGANIZE_MEMBER => {
            crate::game_loop::clans::handle_request_pledge_reorganize_member(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_PLEDGE_WAR_LIST => {
            crate::game_loop::clans::handle_request_pledge_war_list(world, client_id, ex_body)
        }
        exop::REQUEST_EX_PLEDGE_CREST_LARGE => {
            crate::game_loop::clans::handle_request_ex_pledge_crest_large(world, client_id, ex_body)
        }
        exop::REQUEST_EX_SET_PLEDGE_CREST_LARGE => {
            crate::game_loop::clans::handle_request_ex_set_pledge_crest_large(
                world, client_id, ex_body,
            )
        }
        // Clan recruitment registry (G18 slice 8): the board (browse/search/
        // register/detail), the applicant queue (view/accept/reject), the
        // global waiting list (browse/register), and open-joining sign-in.
        exop::REQUEST_PLEDGE_RECRUIT_INFO => {
            crate::game_loop::clans::handle_request_pledge_recruit_info(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_RECRUIT_BOARD_SEARCH => {
            crate::game_loop::clans::handle_request_pledge_recruit_board_search(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_PLEDGE_RECRUIT_BOARD_ACCESS => {
            crate::game_loop::clans::handle_request_pledge_recruit_board_access(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_PLEDGE_RECRUIT_BOARD_DETAIL => {
            crate::game_loop::clans::handle_request_pledge_recruit_board_detail(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_PLEDGE_WAITING_APPLY => {
            crate::game_loop::clans::handle_request_pledge_waiting_apply(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_WAITING_LIST => {
            crate::game_loop::clans::handle_request_pledge_waiting_list(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_WAITING_USER => {
            crate::game_loop::clans::handle_request_pledge_waiting_user(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_WAITING_USER_ACCEPT => {
            crate::game_loop::clans::handle_request_pledge_waiting_user_accept(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_PLEDGE_DRAFT_LIST_SEARCH => {
            crate::game_loop::clans::handle_request_pledge_draft_list_search(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_PLEDGE_DRAFT_LIST_APPLY => {
            crate::game_loop::clans::handle_request_pledge_draft_list_apply(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_PLEDGE_SIGN_IN_FOR_OPEN_JOINING_METHOD => {
            crate::game_loop::clans::handle_request_pledge_sign_in_for_open_joining_method(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_PLEDGE_RECRUIT_APPLY_INFO => {
            crate::game_loop::clans::handle_request_pledge_recruit_apply_info(world, client_id)
        }
        exop::REQUEST_PLEDGE_WAITING_APPLIED => {
            crate::game_loop::clans::handle_request_pledge_waiting_applied(world, client_id)
        }
        // RequestDispel (IN_GAME): alt+click a buff icon to cancel the buff.
        exop::REQUEST_DISPEL => handle_request_dispel(world, client_id, ex_body),
        // RequestBuySellUIClose: Java answers `sendItemList(true)`, i.e. the
        // same refresh as RequestItemList — including its
        // `isInventoryDisabled()` gate, which `handle_request_item_list`
        // applies for both.
        exop::REQUEST_BUY_SELL_UI_CLOSE => handle_request_item_list(world, client_id),
        // RequestRefundItem (IN_GAME): buy back from the sell window's refund tab.
        exop::REQUEST_REFUND_ITEM => {
            crate::game_loop::commerce::shop::handle_request_refund_item(world, client_id, ex_body)
        }
        // Command channels (MPCC).
        exop::REQUEST_EX_ASK_JOIN_MPCC => {
            crate::game_loop::party::command_channel::handle_request_ex_ask_join_mpcc(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_EX_ACCEPT_JOIN_MPCC => {
            crate::game_loop::party::command_channel::handle_request_ex_accept_join_mpcc(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_EX_OUST_FROM_MPCC => {
            crate::game_loop::party::command_channel::handle_request_ex_oust_from_mpcc(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_EX_MPCC_SHOW_PARTY_MEMBERS_INFO => {
            crate::game_loop::party::command_channel::handle_request_ex_mpcc_show_party_members_info(
                world, client_id, ex_body,
            )
        }
        // MPCC matching rooms.
        exop::REQUEST_EX_LIST_MPCC_WAITING => {
            crate::game_loop::party::command_channel::handle_request_ex_list_mpcc_waiting(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_EX_MANAGE_MPCC_ROOM => {
            crate::game_loop::party::command_channel::handle_request_ex_manage_mpcc_room(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_EX_JOIN_MPCC_ROOM => {
            crate::game_loop::party::command_channel::handle_request_ex_join_mpcc_room(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_EX_OUST_FROM_MPCC_ROOM => {
            crate::game_loop::party::command_channel::handle_request_ex_oust_from_mpcc_room(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_EX_DISMISS_MPCC_ROOM => {
            crate::game_loop::party::command_channel::handle_request_ex_dismiss_mpcc_room(
                world, client_id,
            )
        }
        exop::REQUEST_EX_WITHDRAW_MPCC_ROOM => {
            crate::game_loop::party::command_channel::handle_request_ex_withdraw_mpcc_room(
                world, client_id,
            )
        }
        exop::REQUEST_EX_MPCC_PARTYMASTER_LIST => {
            crate::game_loop::party::command_channel::handle_request_ex_mpcc_partymaster_list(
                world, client_id,
            )
        }
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
                    &world.cursed_weapons,
                );
                session.send(body);
            }
        }
        _ => error!("GameLoop: client {client_id} sent ex-opcode 0x{sub:04x}, unhandled."),
    }
}

/// The `DlgAnswer` claim chain: which system does a `ConfirmDlg` answer
/// belong to? Tried in order — revive request, then the message-id-matched
/// prompts (Summon Friend, `.offline`'s exit confirm) — with the admin
/// command confirms as the unclaimed fallback.
fn dispatch_dlg_answer(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(answer) = cp::DlgAnswer::read(body) else {
        return;
    };
    let oid = match world.clients.get(&client_id) {
        Some(ClientSession::InGame(s)) => Some(s.player_object_id()),
        _ => None,
    };
    let claimed = oid
        .is_some_and(|oid| crate::game_loop::death::handle_revive_answer(world, oid, answer.answer == 1))
        // Summon Friend, matched by the echoed message id as Java's
        // `DlgAnswer` does. `requester_id` is checked inside: the client
        // echoes which summoner it is answering, and a prompt must not be
        // answered into a *different* summoner's teleport.
        || (answer.message_id
            == server_packets::sm_ids::C1_WISHES_TO_SUMMON_YOU_FROM_S2_DO_YOU_ACCEPT as i32
            && oid.is_some_and(|oid| {
                crate::game_loop::skills::effects::control::accept_summon_request(
                    world,
                    oid,
                    answer.requester_id,
                    answer.answer == 1,
                )
            }))
        // `MercTicket`'s "Place $s1 in the current location and direction" —
        // claimed by the pending ticket on the player rather than the message
        // id, since Java gates it on `removeAction(MERCENARY_CONFIRM)`.
        || oid.is_some_and(|oid| {
            crate::game_loop::siege::handle_mercenary_confirm(world, oid, answer.answer == 1)
        })
        // `.offline`'s "Do you wish to exit the game?" — matched by the
        // echoed message id, as Java's `DlgAnswer` does.
        || (answer.message_id == server_packets::sm_ids::DO_YOU_WISH_TO_EXIT_THE_GAME as i32
            && crate::game_loop::commerce::offline_trade::handle_exit_game_answer(world, client_id, answer.answer == 1));
    if !claimed {
        crate::game_loop::admin::handle_dlg_answer(world, client_id, answer);
    }
}

/// `Config.DEBUG_CLIENT_PACKETS` / `DEBUG_EX_CLIENT_PACKETS`: trace one inbound
/// packet, honouring `ExcludedPacketList`.
///
/// **One documented difference from Java, and it is visible to an operator.**
/// Java logs the packet's *class* name (`Say2`, `RequestBypassToServer`) and
/// matches `ExcludedPacketList` against that. The port has no per-packet type —
/// packets are opcodes dispatched to functions — and the two opcode tables hold
/// 514 constants between them, so a hand-written name table would be large and
/// would rot silently the first time an opcode moved. The trace therefore names
/// the **opcode**, and the exclusion list matches the same text
/// (`0x49`, `0xD0:0x005F`), case-insensitively.
///
/// Java's `else if (DEBUG_UNKNOWN_PACKETS)` is **nested inside** the trace
/// switch, so an unknown opcode is logged only while tracing is on at all —
/// which is why `DebugUnknownPackets = True` is inert on this dist. Reproduced
/// as written; the port's separate unconditional `error!` for an unhandled
/// opcode is a deliberate deviation documented on the config field.
pub(crate) fn client_packet_trace_line(
    world: &World,
    opcode: u16,
    extended: bool,
) -> Option<String> {
    let on = if extended {
        world.cfg.general.debug_ex_client_packets
    } else {
        world.cfg.general.debug_client_packets
    };
    if !on {
        return None;
    }
    let (tag, label) = if extended {
        ("[C-Ex]", format!("0xD0:0x{opcode:04X}"))
    } else {
        ("[C]", format!("0x{opcode:02X}"))
    };
    if world
        .cfg
        .general
        .excluded_packets
        .iter()
        .any(|e| e.eq_ignore_ascii_case(&label))
    {
        return None;
    }
    Some(format!("{tag} {label}"))
}

fn log_client_packet(world: &World, opcode: u16, extended: bool) {
    if let Some(line) = client_packet_trace_line(world, opcode, extended) {
        tracing::info!("{line}");
    }
}

/// `Config.DEBUG_SERVER_PACKETS`: the outbound half, same shape and same
/// opcode-instead-of-name caveat as [`log_client_packet`].
pub(crate) fn server_packet_trace_line(world: &World, opcode: u8) -> Option<String> {
    if !world.cfg.general.debug_server_packets {
        return None;
    }
    let label = format!("0x{opcode:02X}");
    if world
        .cfg
        .general
        .excluded_packets
        .iter()
        .any(|e| e.eq_ignore_ascii_case(&label))
    {
        return None;
    }
    Some(format!("[S] {label}"))
}

pub(crate) fn log_server_packet(world: &World, opcode: u8) {
    if let Some(line) = server_packet_trace_line(world, opcode) {
        tracing::info!("{line}");
    }
}
