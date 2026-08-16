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
use super::flood;
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
use super::skills::handle_request_dispel;
use super::target::{handle_action, handle_request_target_canceld};

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
        super::spawn_protection::on_action_request(world, client_id, player);
    }
    match opcode {
        cop::AUTH_LOGIN => handle_auth_login(world, client_id, body),
        cop::NEW_CHARACTER => handle_new_character(world, client_id),
        cop::CHARACTER_CREATE => handle_character_create(world, client_id, body),
        cop::CHARACTER_DELETE => handle_character_delete(world, client_id, body),
        cop::CHARACTER_RESTORE => handle_character_restore(world, client_id, body),
        cop::CHARACTER_SELECT => handle_character_select(world, client_id, body),
        cop::ENTER_WORLD => handle_enter_world(world, client_id),
        // Boats (G24.5): board / step off a ferry — boatId + (x, y, z).
        cop::REQUEST_GET_ON_VEHICLE => {
            super::boats::handle_get_on_off_vehicle(world, client_id, body, true)
        }
        cop::REQUEST_GET_OFF_VEHICLE => {
            super::boats::handle_get_on_off_vehicle(world, client_id, body, false)
        }
        // Boats: walk around on deck — boatId + target (x,y,z) + origin (x,y,z).
        cop::REQUEST_MOVE_TO_LOCATION_IN_VEHICLE => {
            super::boats::handle_move_in_vehicle(world, client_id, body)
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
            if let Some(cs @ ClientSession::InGame(session)) = world.clients.get(&client_id)
                && let Some(pkt) =
                    super::helpers::skill_list_packet(world, session.player_object_id())
            {
                cs.send(pkt);
            }
        }
        cop::REQUEST_ITEM_LIST => handle_request_item_list(world, client_id),
        cop::USE_ITEM => handle_use_item(world, client_id, body),
        cop::REQUEST_UN_EQUIP_ITEM => handle_request_un_equip_item(world, client_id, body),
        cop::REQUEST_DESTROY_ITEM => {
            super::items::handle_request_destroy_item(world, client_id, body)
        }
        cop::REQUEST_CRYSTALLIZE_ITEM => {
            super::items::handle_request_crystallize_item(world, client_id, body)
        }
        cop::REQUEST_DROP_ITEM => {
            super::ground_items::handle_request_drop_item(world, client_id, body)
        }
        cop::REQUEST_PACKAGE_SENDABLE_ITEM_LIST => {
            super::warehouse::handle_package_sendable_list(world, client_id, body)
        }
        cop::REQUEST_PACKAGE_SEND => super::warehouse::handle_package_send(world, client_id, body),
        cop::REQUEST_BLOCK => super::block_list::handle_request_block(world, client_id, body),
        cop::SEND_WARE_HOUSE_DEPOSIT_LIST => {
            super::warehouse::handle_deposit(world, client_id, body)
        }
        cop::SEND_WARE_HOUSE_WITH_DRAW_LIST => {
            super::warehouse::handle_withdraw(world, client_id, body)
        }
        cop::REQUEST_ENCHANT_ITEM => super::enchant::handle_enchant(world, client_id, body),
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
                super::clans::handle_request_pledge_skill_info(world, client_id, id, level);
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
            super::position::handle_cannot_move_anymore(world, client_id, body)
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
        cop::REQUEST_PETITION => super::petition::on_request_petition(world, client_id, body),
        cop::REQUEST_PETITION_CANCEL => {
            super::petition::on_request_petition_cancel(world, client_id)
        }
        cop::REQUEST_PETITION_FEEDBACK => {
            super::petition::on_request_petition_feedback(world, client_id, body)
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
            super::community_board::handle_parse_command(world, client_id, &command);
        }
        // RequestBBSwrite (IN_GAME): a board write/submit.
        cop::REQUEST_BBS_WRITE => {
            if let Some([url, a1, a2, a3, a4, a5]) = cp::read_bbs_write(body) {
                super::community_board::handle_write_command(
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
                super::admin::use_admin_command(world, client_id, &format!("admin_{cmd}"), true);
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
            super::player_actions::handle_request_action_use(world, client_id, body)
        }
        cop::REQUEST_GIVE_ITEM_TO_PET => {
            super::servitor::handle_give_item_to_pet(world, client_id, body)
        }
        cop::REQUEST_GET_ITEM_FROM_PET => {
            super::servitor::handle_get_item_from_pet(world, client_id, body)
        }
        cop::REQUEST_PET_USE_ITEM => super::servitor::handle_pet_use_item(world, client_id, body),
        cop::REQUEST_BUY_ITEM => super::shop::handle_request_buy_item(world, client_id, body),
        cop::REQUEST_SELL_ITEM => super::shop::handle_request_sell_item(world, client_id, body),
        // RequestBuySeed (IN_GAME): a player buys seeds at a Manor Manager.
        // Gated on `AllowManor` (off on this dist) like the rest of the system.
        cop::REQUEST_BUY_SEED if world.cfg.general.allow_manor => {
            super::manor::handle_request_buy_seed(world, client_id, body)
        }
        cop::MULTI_SELL_CHOOSE => {
            super::multisell::handle_multi_sell_choose(world, client_id, body)
        }
        cop::REQUEST_PRIVATE_STORE_MANAGE_SELL => {
            super::private_store::open_manage(world, client_id)
        }
        cop::SET_PRIVATE_STORE_LIST_SELL => {
            super::private_store::handle_set_list(world, client_id, body)
        }
        cop::REQUEST_PRIVATE_STORE_QUIT_SELL => super::private_store::handle_quit(world, client_id),
        cop::REQUEST_PRIVATE_STORE_BUY => super::private_store::handle_buy(world, client_id, body),
        // The buy-store half (G15's deferred sibling, landed with the
        // remaining-ports audit's row 6).
        cop::REQUEST_PRIVATE_STORE_MANAGE_BUY => {
            super::private_store::open_manage_buy(world, client_id)
        }
        cop::SET_PRIVATE_STORE_LIST_BUY => {
            super::private_store::handle_set_list_buy(world, client_id, body)
        }
        cop::REQUEST_PRIVATE_STORE_QUIT_BUY => {
            super::private_store::handle_quit_buy(world, client_id)
        }
        cop::REQUEST_PRIVATE_STORE_SELL => {
            super::private_store::handle_store_sell(world, client_id, body)
        }
        // Store titles, both kinds.
        cop::SET_PRIVATE_STORE_MSG_SELL => {
            super::private_store::handle_set_msg(world, client_id, body, false)
        }
        cop::SET_PRIVATE_STORE_MSG_BUY => {
            super::private_store::handle_set_msg(world, client_id, body, true)
        }
        cop::TRADE_REQUEST => super::trade::handle_request(world, client_id, body),
        cop::ANSWER_TRADE_REQUEST => super::trade::handle_answer(world, client_id, body),
        cop::ADD_TRADE_ITEM => super::trade::handle_add_item(world, client_id, body),
        cop::TRADE_DONE => super::trade::handle_done(world, client_id, body),
        // ObserverReturn (IN_GAME): leave the Broadcasting Tower's spectator
        // mode. The Olympiad's viewer answers `RequestOlympiadObserverEnd`
        // instead, and each handler ignores the other's state.
        cop::OBSERVER_RETURN => {
            if let Some(player) = world.player_oid(client_id) {
                super::observation::handle_observer_return(world, client_id, player);
            }
        }
        cop::REQUEST_QUEST_LIST => super::quests::handle_request_quest_list(world, client_id),
        cop::REQUEST_QUEST_ABORT => {
            super::quests::handle_request_quest_abort(world, client_id, body)
        }
        // Tutorial windows (Q255): link clicks and bypass presses share the
        // same router; question-mark clicks fire the global mark event; the
        // client-event echo is dead in this build (its Java handler looks the
        // quest up under a wrong name and always misses) — consumed silently.
        cop::REQUEST_TUTORIAL_LINK_HTML => {
            if let Some(pkt) = cp::RequestTutorialLinkHtml::read(body) {
                super::quests::handle_tutorial_bypass(world, client_id, &pkt.bypass);
            }
        }
        cop::REQUEST_TUTORIAL_PASS_CMD_TO_SERVER => {
            if let Some(pkt) = cp::RequestTutorialPassCmd::read(body) {
                super::quests::handle_tutorial_bypass(world, client_id, &pkt.bypass);
            }
        }
        cop::REQUEST_TUTORIAL_QUESTION_MARK => {
            if let Some(pkt) = cp::RequestTutorialQuestionMark::read(body)
                && let Some(ClientSession::InGame(s)) = world.clients.get(&client_id)
            {
                let player = s.player_object_id();
                super::quests::notify_tutorial_mark(world, client_id, player, pkt.number);
            }
        }
        cop::REQUEST_TUTORIAL_CLIENT_EVENT => {}
        cop::REQUEST_PLEDGE_INFO => {
            super::clans::handle_request_pledge_info(world, client_id, body)
        }
        cop::REQUEST_JOIN_PLEDGE => {
            super::clans::handle_request_join_pledge(world, client_id, body)
        }
        cop::REQUEST_ANSWER_JOIN_PLEDGE => {
            super::clans::handle_request_answer_join_pledge(world, client_id, body)
        }
        cop::REQUEST_WITHDRAWAL_PLEDGE => {
            super::clans::handle_request_withdrawal_pledge(world, client_id)
        }
        cop::REQUEST_OUST_PLEDGE_MEMBER => {
            super::clans::handle_request_oust_pledge_member(world, client_id, body)
        }
        cop::REQUEST_PLEDGE_POWER => {
            super::clans::handle_request_pledge_power(world, client_id, body)
        }
        cop::REQUEST_START_PLEDGE_WAR => {
            super::clans::handle_request_start_pledge_war(world, client_id, body)
        }
        cop::REQUEST_STOP_PLEDGE_WAR => {
            super::clans::handle_request_stop_pledge_war(world, client_id, body)
        }
        cop::REQUEST_SURRENDER_PLEDGE_WAR => {
            super::clans::handle_request_surrender_pledge_war(world, client_id, body)
        }
        cop::REQUEST_PLEDGE_MEMBER_LIST => {
            super::clans::handle_request_pledge_member_list(world, client_id)
        }
        cop::REQUEST_MAGIC_SKILL_LIST => {
            super::skills::handle_request_magic_skill_list(world, client_id, body)
        }
        cop::REQUEST_GM_LIST => super::admin::handle_request_gm_list(world, client_id),
        cop::SNOOP_QUIT => super::chat::handle_snoop_quit(world, client_id, body),
        cop::REQUEST_GIVE_NICK_NAME => {
            super::clans::handle_request_give_nick_name(world, client_id, body)
        }
        cop::REQUEST_LINK_HTML => super::bypass::handle_request_link_html(world, client_id, body),
        cop::REQUEST_PET_GET_ITEM => {
            super::servitor::handle_request_pet_get_item(world, client_id, body)
        }
        cop::REQUEST_PREVIEW_ITEM => {
            super::shop::handle_request_preview_item(world, client_id, body)
        }
        cop::REQUEST_GM_COMMAND => super::admin::handle_request_gm_command(world, client_id, body),
        cop::START_ROTATING => super::position::handle_start_rotating(world, client_id, body),
        cop::FINISH_ROTATING => super::position::handle_finish_rotating(world, client_id, body),
        cop::CANNOT_MOVE_ANYMORE_IN_VEHICLE => {
            super::position::handle_cannot_move_anymore_in_vehicle(world, client_id, body)
        }
        cop::REQUEST_RECIPE_SHOP_MANAGE_PREV => {
            super::crafting::handle_request_recipe_shop_manage_prev(world, client_id)
        }
        cop::REQUEST_ALLY_INFO => super::clans::handle_request_ally_info(world, client_id),
        cop::REQUEST_SET_PLEDGE_CREST => {
            super::clans::handle_request_set_pledge_crest(world, client_id, body)
        }
        cop::REQUEST_PLEDGE_CREST => {
            super::clans::handle_request_pledge_crest(world, client_id, body)
        }
        cop::REQUEST_SET_ALLY_CREST => {
            super::clans::handle_request_set_ally_crest(world, client_id, body)
        }
        cop::REQUEST_ALLY_CREST => super::clans::handle_request_ally_crest(world, client_id, body),
        cop::REQUEST_JOIN_ALLY => super::clans::handle_request_join_ally(world, client_id, body),
        cop::REQUEST_ANSWER_JOIN_ALLY => {
            super::clans::handle_request_answer_join_ally(world, client_id, body)
        }
        cop::ALLY_LEAVE => super::clans::handle_ally_leave(world, client_id),
        cop::ALLY_DISMISS => super::clans::handle_ally_dismiss(world, client_id, body),
        cop::REQUEST_DISMISS_ALLY => super::clans::handle_request_dismiss_ally(world, client_id),
        cop::REQUEST_JOIN_PARTY => handle_request_join_party(world, client_id, body),
        cop::REQUEST_ANSWER_JOIN_PARTY => handle_request_answer_join_party(world, client_id, body),
        cop::REQUEST_WITH_DRAWAL_PARTY => handle_request_withdrawal_party(world, client_id),
        cop::REQUEST_OUST_PARTY_MEMBER => handle_request_oust_party_member(world, client_id, body),
        cop::REQUEST_PARTY_MATCH_CONFIG => {
            super::party_room::handle_request_party_match_config(world, client_id, body)
        }
        cop::REQUEST_PARTY_MATCH_LIST => {
            super::party_room::handle_request_party_match_list(world, client_id, body)
        }
        cop::REQUEST_PARTY_MATCH_DETAIL => {
            super::party_room::handle_request_party_match_detail(world, client_id, body)
        }
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
        // RequestRecipeBookOpen (IN_GAME): the "Common Craft" / "Dwarven Craft"
        // action. Body is one int (`0` = dwarven).
        cop::REQUEST_RECIPE_BOOK_OPEN => {
            let is_dwarven = body
                .get(..4)
                .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]) == 0)
                .unwrap_or(true);
            super::crafting::request_book_open(world, client_id, is_dwarven);
        }
        cop::REQUEST_RECIPE_BOOK_DESTROY => {
            if let Some(id) = cp::read_recipe_single_int(body) {
                super::crafting::handle_book_destroy(world, client_id, id);
            }
        }
        cop::REQUEST_RECIPE_ITEM_MAKE_INFO => {
            if let Some(id) = cp::read_recipe_single_int(body) {
                super::crafting::handle_make_info(world, client_id, id);
            }
        }
        cop::REQUEST_RECIPE_ITEM_MAKE_SELF => {
            if let Some(id) = cp::read_recipe_single_int(body) {
                super::crafting::handle_make_self(world, client_id, id);
            }
        }
        cop::REQUEST_RECIPE_SHOP_MANAGE_LIST => super::crafting::open_manage(world, client_id),
        cop::REQUEST_RECIPE_SHOP_MESSAGE_SET => {
            if let Some(name) = cp::read_recipe_shop_message_set(body) {
                super::crafting::handle_message_set(world, client_id, name);
            }
        }
        cop::REQUEST_RECIPE_SHOP_LIST_SET => {
            if let Some(lines) = cp::read_recipe_shop_list_set(body) {
                super::crafting::handle_list_set(world, client_id, lines);
            }
        }
        cop::REQUEST_RECIPE_SHOP_MANAGE_QUIT => {
            super::crafting::handle_manage_quit(world, client_id)
        }
        cop::REQUEST_RECIPE_SHOP_MAKE_INFO => {
            if let Some((shop, recipe)) = cp::read_recipe_shop_make_info(body) {
                super::crafting::handle_shop_make_info(world, client_id, shop, recipe);
            }
        }
        cop::REQUEST_RECIPE_SHOP_MAKE_ITEM => {
            if let Some((manufacturer, recipe)) = cp::read_recipe_shop_make_item(body) {
                super::crafting::handle_shop_make_item(world, client_id, manufacturer, recipe);
            }
        }
        // Henna / dye symbols (G16).
        cop::REQUEST_HENNA_ITEM_LIST => super::henna::handle_item_list(world, client_id),
        cop::REQUEST_HENNA_REMOVE_LIST => super::henna::handle_remove_list(world, client_id),
        cop::REQUEST_HENNA_ITEM_INFO => {
            if let Some(id) = cp::read_symbol_id(body) {
                super::henna::handle_item_info(world, client_id, id);
            }
        }
        cop::REQUEST_HENNA_ITEM_REMOVE_INFO => {
            if let Some(id) = cp::read_symbol_id(body) {
                super::henna::handle_item_remove_info(world, client_id, id);
            }
        }
        cop::REQUEST_HENNA_EQUIP => {
            if let Some(id) = cp::read_symbol_id(body) {
                super::henna::handle_equip(world, client_id, id);
            }
        }
        cop::REQUEST_HENNA_REMOVE => {
            if let Some(id) = cp::read_symbol_id(body) {
                super::henna::handle_remove(world, client_id, id);
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
            handle_request_character_name_creatable(world, client_id, ex_body)
        }
        // RequestKeyMapping (ENTERING + IN_GAME): STORE_UI_SETTINGS is on, so
        // reply with the (empty) stored UI key mapping.
        exop::REQUEST_KEY_MAPPING => {
            // `STORE_UI_SETTINGS` is on for this dist, so the stored layout (if
            // any) is replayed; a character who never saved one gets Java's
            // empty payload.
            let mapping = super::settings::stored_key_mapping(world, client_id);
            if let Some(cs) = world.clients.get(&client_id)
                && matches!(cs, ClientSession::Entering(_) | ClientSession::InGame(_))
            {
                cs.send(server_packets::ex_ui_setting(&mapping));
            }
        }
        exop::REQUEST_SAVE_KEY_MAPPING => {
            super::settings::handle_save_key_mapping(world, client_id, ex_body)
        }
        exop::REQUEST_CONFIRM_TARGET_ITEM => {
            super::augment::handle_confirm_target_item(world, client_id, ex_body)
        }
        exop::REQUEST_CONFIRM_GEMSTONE => {
            super::augment::handle_confirm_gemstone(world, client_id, ex_body)
        }
        exop::REQUEST_CONFIRM_CANCEL_ITEM => {
            super::augment::handle_confirm_cancel_item(world, client_id, ex_body)
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
            super::manor::handle_request_procure_crop_list(world, client_id, ex_body);
        }
        // RequestSetSeed / RequestSetCrop (IN_GAME): the manor owner submits the
        // next-period seed/crop setup through the chamberlain's edit windows.
        // Gated on `AllowManor` (off on this dist) like the rest of the system.
        exop::REQUEST_SET_SEED if world.cfg.general.allow_manor => {
            super::manor::handle_request_set_seed(world, client_id, ex_body);
        }
        exop::REQUEST_SET_CROP if world.cfg.general.allow_manor => {
            super::manor::handle_request_set_crop(world, client_id, ex_body);
        }
        // EndScenePlayer / RequestExEscapeScene (IN_GAME): a cinematic ended
        // on its own, or the player pressed Esc during an escapable one
        // (`//playmovie` is the only movie route on this dist).
        exop::END_SCENE_PLAYER => {
            super::admin::effects::handle_end_scene_player(world, client_id, ex_body);
        }
        exop::REQUEST_EX_ESCAPE_SCENE => {
            super::admin::effects::handle_escape_scene(world, client_id);
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
                super::punishment::on_hwid_received(world, client_id);
            }
        }
        // Olympiad observer mode (G25): leave observing, or (re)open the
        // ongoing-match list. The list request/refresh just re-sends it.
        exop::REQUEST_OLYMPIAD_OBSERVER_END => {
            if let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) {
                let player = session.player_object_id();
                // Java's `if player.inObserverMode()` guard.
                if super::olympiad::is_observing(world, player) {
                    super::olympiad::leave_observer(world, client_id, player);
                }
            }
        }
        exop::REQUEST_OLYMPIAD_MATCH_LIST | exop::REQUEST_EX_OLYMPIAD_MATCH_LIST_REFRESH => {
            super::olympiad::send_match_list(world, client_id);
        }
        // RequestExMagicSkillUseGround (IN_GAME): a GROUND-target cast aimed
        // at a world position (G19).
        exop::REQUEST_CURSED_WEAPON_LIST => {
            super::cursed_weapon::handle_request_list(world, client_id)
        }
        exop::REQUEST_CURSED_WEAPON_LOCATION => {
            super::cursed_weapon::handle_request_location(world, client_id)
        }
        exop::SET_PRIVATE_STORE_WHOLE_MSG => {
            super::private_store::handle_set_whole_msg(world, client_id, ex_body)
        }
        exop::REQUEST_EX_MAGIC_SKILL_USE_GROUND => {
            super::skills::cast::handle_request_magic_skill_use_ground(world, client_id, ex_body)
        }
        // Skill enchanting (G19): the enchant window's info/detail queries and
        // the enchant itself.
        exop::REQUEST_EX_ENCHANT_SKILL_INFO => {
            super::skills::enchant::handle_request_enchant_skill_info(world, client_id, ex_body)
        }
        exop::REQUEST_EX_ENCHANT_SKILL => {
            super::skills::enchant::handle_request_enchant_skill(world, client_id, ex_body)
        }
        exop::REQUEST_EX_ENCHANT_SKILL_INFO_DETAIL => {
            super::skills::enchant::handle_request_enchant_skill_info_detail(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_DUEL_START => {
            super::duel::handle_request_duel_start(world, client_id, ex_body)
        }
        exop::REQUEST_DUEL_ANSWER_START => {
            super::duel::handle_request_duel_answer(world, client_id, ex_body)
        }
        exop::REQUEST_DUEL_SURRENDER => {
            super::duel::handle_request_duel_surrender(world, client_id)
        }
        exop::REQUEST_SAVE_INVENTORY_ORDER => {
            handle_request_save_inventory_order(world, client_id, ex_body)
        }
        // RequestExRqItemLink (IN_GAME): the "?" on a shift-clicked item link
        // in chat was clicked; answer with that item's row.
        exop::REQUEST_EX_RQ_ITEM_LINK => {
            super::chat::handle_request_item_link(world, client_id, ex_body)
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
            super::items::handle_request_auto_soul_shot(world, client_id, ex_body)
        }
        // ExRequestAutoFish (IN_GAME): toggle auto-fishing.
        exop::REQUEST_EX_AUTO_FISH => {
            if let Some(crate::session::ClientSession::InGame(s)) = world.clients.get(&client_id) {
                let player = s.player_object_id();
                super::fishing::toggle_fishing(world, player);
            }
        }
        // Item auction (G30.5): bid on / request info about an auction.
        exop::REQUEST_POST_ITEM_LIST => super::mail::handle_post_item_list(world, client_id),
        exop::REQUEST_RECEIVED_POST_LIST => {
            super::mail::handle_received_post_list(world, client_id)
        }
        exop::REQUEST_SENT_POST_LIST => super::mail::handle_sent_post_list(world, client_id),
        exop::REQUEST_SEND_POST => super::mail::handle_send_post(world, client_id, ex_body),
        exop::REQUEST_RECEIVED_POST => super::mail::handle_received_post(world, client_id, ex_body),
        exop::REQUEST_SENT_POST => super::mail::handle_sent_post(world, client_id, ex_body),
        exop::REQUEST_DELETE_RECEIVED_POST => {
            super::mail::handle_delete_received_post(world, client_id, ex_body)
        }
        exop::REQUEST_DELETE_SENT_POST => {
            super::mail::handle_delete_sent_post(world, client_id, ex_body)
        }
        exop::REQUEST_POST_ATTACHMENT => {
            super::mail::handle_post_attachment(world, client_id, ex_body)
        }
        exop::REQUEST_REJECT_POST_ATTACHMENT => {
            super::mail::handle_reject_post_attachment(world, client_id, ex_body)
        }
        exop::REQUEST_CANCEL_POST_ATTACHMENT => {
            super::mail::handle_cancel_post_attachment(world, client_id, ex_body)
        }
        exop::REQUEST_OUST_FROM_PARTY_ROOM => {
            super::party_room::handle_oust_from_party_room(world, client_id, ex_body)
        }
        exop::REQUEST_DISMISS_PARTY_ROOM => {
            super::party_room::handle_dismiss_party_room(world, client_id, ex_body)
        }
        exop::REQUEST_WITHDRAW_PARTY_ROOM => {
            super::party_room::handle_withdraw_party_room(world, client_id, ex_body)
        }
        exop::REQUEST_ASK_JOIN_PARTY_ROOM => {
            super::party_room::handle_ask_join_party_room(world, client_id, ex_body)
        }
        exop::ANSWER_JOIN_PARTY_ROOM => {
            super::party_room::handle_answer_join_party_room(world, client_id, ex_body)
        }
        exop::REQUEST_EXIT_PARTY_MATCHING_WAITING_ROOM => {
            super::party_room::handle_exit_waiting_room(world, client_id)
        }
        exop::REQUEST_LIST_PARTY_MATCHING_WAITING_ROOM => {
            super::party_room::handle_list_waiting_room(world, client_id, ex_body)
        }
        exop::REQUEST_BID_ITEM_AUCTION => {
            super::item_auction::on_request_bid(world, client_id, ex_body)
        }
        exop::REQUEST_INFO_ITEM_AUCTION => {
            super::item_auction::on_request_info(world, client_id, ex_body)
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
        exop::REQUEST_REFINE_CANCEL => {
            super::augment::handle_refine_cancel(world, client_id, ex_body)
        }
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
        // Clan ranks & power grades (G18 slice 3).
        exop::REQUEST_PLEDGE_POWER_GRADE_LIST => {
            super::clans::handle_request_pledge_power_grade_list(world, client_id)
        }
        exop::REQUEST_PLEDGE_MEMBER_POWER_INFO => {
            super::clans::handle_request_pledge_member_power_info(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_SET_ACADEMY_MASTER => {
            super::clans::academy::handle_set_academy_master(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_SET_MEMBER_POWER_GRADE => {
            super::clans::handle_request_pledge_set_member_power_grade(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_MEMBER_INFO => {
            super::clans::handle_request_pledge_member_info(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_REORGANIZE_MEMBER => {
            super::clans::handle_request_pledge_reorganize_member(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_WAR_LIST => {
            super::clans::handle_request_pledge_war_list(world, client_id, ex_body)
        }
        exop::REQUEST_EX_PLEDGE_CREST_LARGE => {
            super::clans::handle_request_ex_pledge_crest_large(world, client_id, ex_body)
        }
        exop::REQUEST_EX_SET_PLEDGE_CREST_LARGE => {
            super::clans::handle_request_ex_set_pledge_crest_large(world, client_id, ex_body)
        }
        // Clan recruitment registry (G18 slice 8): the board (browse/search/
        // register/detail), the applicant queue (view/accept/reject), the
        // global waiting list (browse/register), and open-joining sign-in.
        exop::REQUEST_PLEDGE_RECRUIT_INFO => {
            super::clans::handle_request_pledge_recruit_info(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_RECRUIT_BOARD_SEARCH => {
            super::clans::handle_request_pledge_recruit_board_search(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_RECRUIT_BOARD_ACCESS => {
            super::clans::handle_request_pledge_recruit_board_access(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_RECRUIT_BOARD_DETAIL => {
            super::clans::handle_request_pledge_recruit_board_detail(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_WAITING_APPLY => {
            super::clans::handle_request_pledge_waiting_apply(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_WAITING_LIST => {
            super::clans::handle_request_pledge_waiting_list(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_WAITING_USER => {
            super::clans::handle_request_pledge_waiting_user(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_WAITING_USER_ACCEPT => {
            super::clans::handle_request_pledge_waiting_user_accept(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_DRAFT_LIST_SEARCH => {
            super::clans::handle_request_pledge_draft_list_search(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_DRAFT_LIST_APPLY => {
            super::clans::handle_request_pledge_draft_list_apply(world, client_id, ex_body)
        }
        exop::REQUEST_PLEDGE_SIGN_IN_FOR_OPEN_JOINING_METHOD => {
            super::clans::handle_request_pledge_sign_in_for_open_joining_method(
                world, client_id, ex_body,
            )
        }
        exop::REQUEST_PLEDGE_RECRUIT_APPLY_INFO => {
            super::clans::handle_request_pledge_recruit_apply_info(world, client_id)
        }
        exop::REQUEST_PLEDGE_WAITING_APPLIED => {
            super::clans::handle_request_pledge_waiting_applied(world, client_id)
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
            super::shop::handle_request_refund_item(world, client_id, ex_body)
        }
        // Command channels (MPCC).
        exop::REQUEST_EX_ASK_JOIN_MPCC => {
            super::command_channel::handle_request_ex_ask_join_mpcc(world, client_id, ex_body)
        }
        exop::REQUEST_EX_ACCEPT_JOIN_MPCC => {
            super::command_channel::handle_request_ex_accept_join_mpcc(world, client_id, ex_body)
        }
        exop::REQUEST_EX_OUST_FROM_MPCC => {
            super::command_channel::handle_request_ex_oust_from_mpcc(world, client_id, ex_body)
        }
        exop::REQUEST_EX_MPCC_SHOW_PARTY_MEMBERS_INFO => {
            super::command_channel::handle_request_ex_mpcc_show_party_members_info(
                world, client_id, ex_body,
            )
        }
        // MPCC matching rooms.
        exop::REQUEST_EX_LIST_MPCC_WAITING => {
            super::command_channel::handle_request_ex_list_mpcc_waiting(world, client_id, ex_body)
        }
        exop::REQUEST_EX_MANAGE_MPCC_ROOM => {
            super::command_channel::handle_request_ex_manage_mpcc_room(world, client_id, ex_body)
        }
        exop::REQUEST_EX_JOIN_MPCC_ROOM => {
            super::command_channel::handle_request_ex_join_mpcc_room(world, client_id, ex_body)
        }
        exop::REQUEST_EX_OUST_FROM_MPCC_ROOM => {
            super::command_channel::handle_request_ex_oust_from_mpcc_room(world, client_id, ex_body)
        }
        exop::REQUEST_EX_DISMISS_MPCC_ROOM => {
            super::command_channel::handle_request_ex_dismiss_mpcc_room(world, client_id)
        }
        exop::REQUEST_EX_WITHDRAW_MPCC_ROOM => {
            super::command_channel::handle_request_ex_withdraw_mpcc_room(world, client_id)
        }
        exop::REQUEST_EX_MPCC_PARTYMASTER_LIST => {
            super::command_channel::handle_request_ex_mpcc_partymaster_list(world, client_id)
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
        Some(crate::session::ClientSession::InGame(s)) => Some(s.player_object_id()),
        _ => None,
    };
    let claimed = oid
        .is_some_and(|oid| super::death::handle_revive_answer(world, oid, answer.answer == 1))
        // Summon Friend, matched by the echoed message id as Java's
        // `DlgAnswer` does. `requester_id` is checked inside: the client
        // echoes which summoner it is answering, and a prompt must not be
        // answered into a *different* summoner's teleport.
        || (answer.message_id
            == server_packets::sm_ids::C1_WISHES_TO_SUMMON_YOU_FROM_S2_DO_YOU_ACCEPT as i32
            && oid.is_some_and(|oid| {
                super::skills::effects::control::accept_summon_request(
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
            super::siege::handle_mercenary_confirm(world, oid, answer.answer == 1)
        })
        // `.offline`'s "Do you wish to exit the game?" — matched by the
        // echoed message id, as Java's `DlgAnswer` does.
        || (answer.message_id == server_packets::sm_ids::DO_YOU_WISH_TO_EXIT_THE_GAME as i32
            && super::offline_trade::handle_exit_game_answer(world, client_id, answer.answer == 1));
    if !claimed {
        super::admin::handle_dlg_answer(world, client_id, answer);
    }
}
