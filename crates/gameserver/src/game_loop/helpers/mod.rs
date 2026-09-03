//! Shared helpers for the packet handlers, split by theme: lookups, sends,
//! broadcasts, vitals, inventory and position.
//!
//! The `pub(crate) use` lines below re-export this module's **own** submodules,
//! which is the whole point of the split. Nothing else belongs here: a symbol
//! that lives in another module is imported from that module by every caller,
//! never forwarded through this one. A convenience re-export would make
//! `helpers` look like the home of something it does not own — `ms_to_ticks`
//! belongs to `crate::scheduler`, `npc_say` to `npc::say`, and so on.

mod lookup;
mod send;
mod vitals;

pub(crate) use lookup::{
    class_level, client_for_player, format_amount, get_others_in_matching_room, instance_of, is_gm,
    is_playable, level_of, maybe_object_name, nth_arg, object_name, player, player_name,
    player_name_or_empty, player_of, player_race, player_race_or_human, player_var, player_var_int,
    reuses_mut, set_player_var, set_player_var_int, unset_player_var, update_admin_flags,
};
pub(crate) use send::{
    announce_to_all_online, disconnect_player, send_action_failed, send_etc_status_update,
    send_message, send_sm_and_action_failed, send_sm_bare_to_client, send_sm_bare_to_player,
    send_sm_to_client, send_sm_to_player, send_to_client, send_to_player, skill_list_packet,
};
pub(crate) use vitals::{
    absorb_into_hp, hp_fraction, hp_pair, in_zone, is_dead, is_friend, recalculate_player_stats,
    recalculate_player_stats_and_vitals, restore_hp_mp, spend_mp, stat_add, stat_mul, vitals_pair,
};
