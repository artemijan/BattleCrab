//! Outbound (server → client) packets. Ported 1:1 from
//! `gameserver/network/serverpackets`. Each builder returns the serialized body
//! (opcode + payload, unencrypted); the connection task encrypts and frames it.
//!
//! G1 covers only `KeyPacket`; the rest arrive with their milestones.
//!
//! The builders are split into per-domain submodules but flattened back into
//! this module via glob re-exports, so every call site keeps referring to them
//! as `server_packets::<name>` regardless of which file they live in.

pub mod opcodes;

mod char_info;
mod chat;
mod clan;
mod combat;
mod command_channel;
mod community_board;
mod door;
mod effect;
mod enchant;
mod fishing;
mod friend;
mod games;
mod gm_view;
mod ground_item;
mod henna;
mod lobby;
mod mail;
mod manor;
mod movement;
mod multisell;
mod npc;
mod olympiad;
mod party;
mod party_room;
mod player_trade;
mod private_store;
mod quest;
mod recipe;
mod residence;
mod shortcut;
mod siege;
mod skill;
mod status;
mod system_message;
mod target;
mod variation;
mod vehicle;
mod warehouse;

pub use char_info::{CharInfoState, char_info, delete_object, ex_vote_system_info};
pub use chat::{creature_say, creature_say_system, ex_world_chat_cnt, petition_vote, snoop};
pub use clan::{
    alliance_info, ally_crest, ask_join_ally, ask_join_pledge, ex_pledge_count,
    ex_pledge_draft_list_search, ex_pledge_emblem, ex_pledge_recruit_apply_info,
    ex_pledge_recruit_board_detail, ex_pledge_recruit_board_search, ex_pledge_recruit_info,
    ex_pledge_waiting_list, ex_pledge_waiting_list_alarm, ex_pledge_waiting_list_applied,
    ex_pledge_waiting_user, gm_view_pledge_info, join_pledge, manage_pledge_power, pledge_crest,
    pledge_info, pledge_power_grade_list, pledge_receive_member_info, pledge_receive_power_info,
    pledge_receive_sub_pledge_created, pledge_receive_war_list, pledge_show_info_update,
    pledge_show_member_list_add, pledge_show_member_list_all, pledge_show_member_list_all_of,
    pledge_show_member_list_all_tabs, pledge_show_member_list_delete,
    pledge_show_member_list_delete_all, pledge_show_member_list_update, pledge_skill_list,
    pledge_skill_list_add, surrender_pledge_war,
};
pub use combat::{
    AttackHit, DieOptions, SOCIAL_ACTION_LEVEL_UP, attack, auto_attack_start, auto_attack_stop,
    change_wait_type, dice, die, ex_duel_ask_start, ex_duel_end, ex_duel_ready, ex_duel_start,
    ex_duel_update_user_info, revive, social_action, wait_type,
};
pub use command_channel::{
    MpccRoomListView, MpccRoomMemberView, PartyMemberInfoView, ex_ask_join_mpcc, ex_close_mpcc,
    ex_dissmiss_mpcc_room, ex_list_mpcc_waiting, ex_manage_mpcc_room_member,
    ex_mpcc_party_info_update, ex_mpcc_partymaster_list, ex_mpcc_room_info, ex_mpcc_room_member,
    ex_mpcc_show_party_member_info, ex_multi_party_command_channel_info, ex_open_mpcc,
};
pub use community_board::{radar_control, show_board, show_board_hide};
pub use door::{door_status_update, static_object_info, static_object_info_door};
pub use effect::{
    PVP_MATCH_FINISH, PVP_MATCH_INITIALIZE, PVP_MATCH_UPDATE, earthquake, ex_pvp_match_cc_record,
    ex_red_sky, ex_show_screen_message, ex_show_screen_message_npc_string, magic_skill_use_raw,
    sun_rise, sun_set,
};
pub use enchant::{
    choose_inventory_item, enchant_result, ex_put_enchant_scroll_item_result,
    ex_put_enchant_support_item_result, ex_put_enchant_target_item_result,
    ex_remove_enchant_support_item_result,
};
pub use fishing::{ex_auto_fish_available, ex_fishing_end, ex_fishing_start, ex_user_info_fishing};
pub use friend::{
    FriendEntry, block_list, friend_add_request, friend_add_request_result, friend_remove,
    friend_status, friend_status_mode, l2_friend_list, l2_friend_say,
};
pub use games::{ex_cursed_weapon_list, ex_cursed_weapon_location, mon_race_info};
pub use gm_view::{
    gm_henna_info, gm_view_character_info, gm_view_quest_info, gm_view_skill_info,
    gm_view_warehouse_withdraw_list,
};
pub use ground_item::{GroundItemView, drop_item, get_item, spawn_item};
pub use henna::{
    HennaLine, HennaStatWire, StatPreview, henna_equip_list, henna_info, henna_item_draw_info,
    henna_item_remove_info, henna_remove_list,
};
pub use lobby::{
    char_create_fail, char_create_ok, char_delete_fail, char_delete_success, char_selected,
    char_selection_info, ex_is_char_name_creatable, key_packet, leave_world, login_fail,
    login_success, new_character_success, restart_response,
};
pub use mail::{
    MESSAGE_FEE, MESSAGE_FEE_PER_SLOT, MailListView, ex_change_post_state, ex_notice_post_arrived,
    ex_notice_post_sent, ex_reply_post_item_list, ex_reply_received_post, ex_reply_sent_post,
    ex_show_received_post_list, ex_show_sent_post_list, ex_unread_mail_count,
};
pub use manor::{
    CropInfoEntry, CropSettingEntry, ManorDefaultEntry, SeedInfoEntry, SeedSettingEntry,
    compass_zone, ex_auto_soul_shot, ex_pccafe_point_info, ex_send_manor_list,
    ex_set_compass_zone_code, ex_show_crop_info, ex_show_crop_setting, ex_show_manor_default_info,
    ex_show_seed_info, ex_show_seed_setting, ex_ui_setting, show_mini_map,
};
pub use movement::{
    FlyType, action_failed, change_move_type, ex_server_primitive,
    ex_teleport_to_location_activate, fly_to_location, move_to_location, move_to_pawn,
    observation_mode, observation_return, ride, start_rotation, stop_move, stop_rotation,
    teleport_to_location, validate_location,
};
pub use multisell::{ex_multi_sell_result, multi_sell_list};
pub use npc::{
    nickname_changed, npc_html_message, npc_html_message_item, npc_info, npc_say, npc_say_param,
    npc_say_param_typed, npc_say_text, npc_title, pet_item_list, set_summon_remain_time,
    shop_preview_info, special_camera, summon_info,
};
pub use olympiad::{
    HeroListRow, OlympiadMatchRow, ex_hero_list, ex_olympiad_match_list, ex_olympiad_mode,
};
pub use party::{
    PartyMemberView, PartySummonView, ask_join_party, ex_ask_modify_party_looting,
    ex_inzone_waiting, ex_set_party_looting, ex_tactical_sign, join_party, party_member_position,
    party_small_window_add, party_small_window_all, party_small_window_delete,
    party_small_window_delete_all, party_small_window_update, party_window_flags,
};
pub use party_room::{
    ROOMS_PER_PAGE, RoomListView, RoomMemberView, WaitingPlayerView, ex_ask_join_party_room,
    ex_close_party_room, ex_list_party_matching_waiting_room, ex_party_room_member,
    list_party_waiting, party_room_info,
};
pub use player_trade::{send_trade_request, trade_add, trade_done, trade_press_ok, trade_start};
pub use private_store::{
    StoreLine, ex_private_store_whole_msg, list_buy, list_sell, manage_list_buy, manage_list_sell,
    msg_buy, msg_sell,
};
pub use quest::{
    ex_npc_quest_html_message, ex_show_quest_mark, play_music, play_sound, play_sound_at,
    play_tutorial_voice, quest_sounds, tutorial_close_html, tutorial_show_html,
    tutorial_show_question_mark,
};
pub use recipe::{
    recipe_book_item_list, recipe_item_make_info, recipe_shop_item_info, recipe_shop_manage_list,
    recipe_shop_msg, recipe_shop_sell_list,
};
pub use residence::{ex_show_castle_info, ex_show_fortress_info};
pub use shortcut::{send_all_macros, send_macro_list, shortcut_init, shortcut_register};
pub use siege::{
    AttackerEntry, DefenderEntry, siege_attacker_list, siege_defender_list, siege_info,
};
pub use skill::{
    acquire_skill_done, acquire_skill_info, ex_acquirable_skill_list_by_class, magic_skill_canceld,
    magic_skill_launched, magic_skill_use, setup_gauge, setup_gauge_range, skill_cool_time,
};
pub use status::{relation_changed, status_update, status_update_type};
pub use system_message::{
    S1_3_MESSAGE_ID, SmParam, confirm_dlg, confirm_dlg_text, confirm_dlg_with, obtained_item_sm,
    sm_ids, system_message, system_message_with,
};
pub use target::{my_target_selected, target_selected, target_unselected};
pub use variation::{
    ex_put_commission_result_for_variation_make, ex_put_intensive_result_for_variation_make,
    ex_put_item_result_for_variation_cancel, ex_put_item_result_for_variation_make,
    ex_show_variation_cancel_window, ex_show_variation_make_window, ex_variation_cancel_result,
    ex_variation_result,
};
pub use vehicle::{
    get_off_vehicle, get_on_vehicle, move_to_location_in_vehicle, stop_move_in_vehicle,
    vehicle_departure, vehicle_info,
};
pub use warehouse::{
    WH_TYPE_CLAN, WH_TYPE_PRIVATE, package_sendable_list, package_to_list, warehouse_deposit_list,
    warehouse_withdrawal_list,
};

/// Java `ServerPacket.PAPERDOLL_ORDER` — the 33-slot equipment write order the
/// client expects, mapped from the `InventorySlot` wire order.
///
/// It is the base-class default, so it lives here rather than in either packet
/// that inherits it: `CharSelectionInfo` (`lobby`) and `GMViewCharacterInfo`
/// (`gm_view`) both write it because neither overrides `getPaperdollOrder()`
/// the way `CharInfo` does — those overrides stay private to their own files.
///
/// `RHand` appears twice (the slot the LRHAND display component reads), and
/// everything past `Brooch` is post-Interlude and always empty here.
pub const PAPERDOLL_ORDER: [crate::model::inventory::PaperdollSlot; 33] = {
    use crate::model::inventory::PaperdollSlot;
    [
        PaperdollSlot::Under,
        PaperdollSlot::REar,
        PaperdollSlot::LEar,
        PaperdollSlot::Neck,
        PaperdollSlot::RFinger,
        PaperdollSlot::LFinger,
        PaperdollSlot::Head,
        PaperdollSlot::RHand,
        PaperdollSlot::LHand,
        PaperdollSlot::Gloves,
        PaperdollSlot::Chest,
        PaperdollSlot::Legs,
        PaperdollSlot::Feet,
        PaperdollSlot::Cloak,
        PaperdollSlot::RHand,
        PaperdollSlot::Hair,
        PaperdollSlot::Hair2,
        PaperdollSlot::RBracelet,
        PaperdollSlot::LBracelet,
        PaperdollSlot::Deco1,
        PaperdollSlot::Deco2,
        PaperdollSlot::Deco3,
        PaperdollSlot::Deco4,
        PaperdollSlot::Deco5,
        PaperdollSlot::Deco6,
        PaperdollSlot::Belt,
        PaperdollSlot::Brooch,
        PaperdollSlot::BroochJewel1,
        PaperdollSlot::BroochJewel2,
        PaperdollSlot::BroochJewel3,
        PaperdollSlot::BroochJewel4,
        PaperdollSlot::BroochJewel5,
        PaperdollSlot::BroochJewel6,
    ]
};

/// An extended packet's header: the `0xFE` opcode plus its sub-opcode, ready
/// for the builder to append its own body. Every `Ex…` builder starts here, so
/// it lives beside the submodules rather than being redefined in each of them.
fn ex(sub: i16) -> commons::network::PacketWriter {
    let mut w = commons::network::PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(sub);
    w
}
