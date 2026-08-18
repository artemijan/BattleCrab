//! The castle-manor menu, driven by the `manor_menu_select` client bypass —
//! port of `CastleChamberlain.onNpcManorBypass` (the `ON_NPC_MANOR_BYPASS`
//! listener). The chamberlain's `manor` html button opens `manor.html`, whose
//! buttons send `manor_menu_select?ask=<request>&state=<manorId>&time=<0|1>`;
//! this routes each request to its `ExShow*` display packet.
//!
//! Wired requests: 3 (`ExShowSeedInfo`) / 4 (`ExShowCropInfo`) show the
//! castle's live production/procure ([`crate::model::manor::ManorState`]); 5
//! (`ExShowManorDefaultInfo`) the static catalogue reference table; 7
//! (`ExShowSeedSetting`) / 8 (`ExShowCropSetting`) the owner's editable setup,
//! plus the `RequestSetSeed`/`RequestSetCrop` write path back into the
//! next-period state ([`handle_request_set_seed`]/[`handle_request_set_crop`]).
//!
//! The setup path is gated to the manor's **modifiable** period. The wall-clock
//! period scheduler ([`schedule_manor_at_boot`] + [`advance_manor_mode`]) drives
//! the mode through APPROVED → MAINTENANCE → MODIFIABLE → APPROVED on the daily
//! `AltManor*` cutover times, runs the production rollover **and its economic
//! settlement** (crops sold are paid into the owner clan's warehouse, unspent
//! crop reservations return to the castle treasury, the next period is gated on
//! and charged to that treasury, and the state is written back with
//! `storeMe`).
//!
//! The player-facing Manor Manager trader is [`handle_request_buy_seed`]
//! (`RequestBuySeed`, buy seeds from a castle's current production) and
//! [`handle_request_procure_crop_list`] (`RequestProcureCropList`, sell crops
//! for the crop's reward item, with a 5 % adena fee across castles). Note that
//! the reference build never sends the buy/sell *display* packets (`BuyListSeed`
//! /`ExShowSellCropList` are dead), so the trader window is client-native.

use crate::game_loop::helpers::send_to_client;
#[cfg(test)]
use crate::game_loop::time::{MILLIS_PER_DAY, MILLIS_PER_HOUR, MILLIS_PER_MINUTE};
#[cfg(test)]
use crate::model::manor::ManorMode;
#[cfg(test)]
use schedule::{ModeTimes, boot_mode, next_mode_change_millis};

use tracing::warn;

use crate::model::components::LastFolkNpc;

use crate::network::server_packets::{self, sm_ids};
use crate::world::World;

use crate::game_loop::helpers::npc_id_of;

/// The clan that owns `castle_id`, if any (Java `Castle.getOwnerId()`), re-
/// exported so scripts outside `game_loop` can gate on castle ownership without
/// reaching into the private `siege` module.
pub(crate) fn castle_owner_clan_id(world: &World, castle_id: i32) -> Option<i32> {
    super::siege::owner_clan_id_opt(world, castle_id)
}

/// Chamberlain (of Light / of Darkness) NPC template id → the castle it serves.
/// Java resolves `npc.getCastle()` by zone; every chamberlain id belongs to
/// exactly one castle, so the mapping from `CastleChamberlain.NPC` (the paired
/// light/dark ids per castle) is exact.
pub(crate) fn chamberlain_castle_id(npc_id: i32) -> Option<i32> {
    Some(match npc_id {
        35100 | 36653 => 1, // Gludio
        35142 | 36654 => 2, // Dion
        35184 | 36655 => 3, // Giran
        35226 | 36656 => 4, // Oren
        35274 | 36657 => 5, // Aden
        35316 | 36658 => 6, // Innadril
        35363 | 36659 => 7, // Goddard
        35509 | 36660 => 8, // Rune
        35555 | 36661 => 9, // Schuttgart
        _ => return None,
    })
}

/// Port of `RequestBypassToServer`'s `manor_menu_select` branch feeding
/// `CastleChamberlain.onNpcManorBypass`. Parses `ask=<request>&state=<manorId>
/// &time=<0|1>` off the command, resolves the castle through the last folk NPC
/// (the chamberlain), and dispatches the request. `-1` manor id falls back to
/// the chamberlain's own castle (Java `(manorId == -1) ? npc.getCastle()…`).
pub(crate) fn handle_manor_menu_select(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    command: &str,
) {
    // Java gate: `Config.ALLOW_MANOR && lastNpc != null && canInteract && has
    // manor listener`. The caller (bypass.rs) verifies the folk NPC + range;
    // here we re-check the manor toggle and that the NPC is a chamberlain.
    if !world.cfg.general.allow_manor {
        return;
    }
    let Some((request, manor_id, next_period)) = parse_manor_select(command) else {
        warn!("Manor: malformed manor_menu_select [{command}].");
        return;
    };
    let Some(&LastFolkNpc(npc_object_id)) = world.objects.get_component::<LastFolkNpc>(&object_id)
    else {
        return;
    };
    let Some(npc_id) = npc_id_of(world, npc_object_id) else {
        return;
    };
    let Some(npc_castle) = chamberlain_castle_id(npc_id) else {
        return;
    };
    let castle_id = if manor_id == -1 { npc_castle } else { manor_id };

    match request {
        // Request 3: the "Seed Purchase" view — the castle's live seed
        // production (`ExShowSeedInfo`).
        3 => {
            let seeds = seed_info_entries(world, castle_id, next_period);
            send_to_client(
                world,
                client_id,
                server_packets::ex_show_seed_info(castle_id, true, Some(&seeds)),
            );
        }
        // Request 4: the "Crop Sales" view — the castle's live crop procure
        // (`ExShowCropInfo`).
        4 => {
            let crops = crop_info_entries(world, castle_id, next_period);
            send_to_client(
                world,
                client_id,
                server_packets::ex_show_crop_info(castle_id, true, Some(&crops)),
            );
        }
        // Request 5: the seed/crop reference table (static catalogue).
        5 => {
            let crops = default_entries(world);
            send_to_client(
                world,
                client_id,
                server_packets::ex_show_manor_default_info(&crops, true),
            );
        }
        // Request 7: the owner's "Edit Seed Setup" view (`ExShowSeedSetting`).
        7 => {
            if world.manor.is_manor_approved() {
                // Java: no setup outside the modifiable period.
                send_to_client(
                    world,
                    client_id,
                    server_packets::system_message_with(
                        sm_ids::A_MANOR_CANNOT_BE_SET_UP_BETWEEN_4_30_AM_AND_8_PM,
                        &[],
                    ),
                );
                return;
            }
            let seeds = seed_setting_entries(world, castle_id);
            send_to_client(
                world,
                client_id,
                server_packets::ex_show_seed_setting(castle_id, &seeds),
            );
        }
        // Request 8: the owner's "Edit Crop Setup" view (`ExShowCropSetting`).
        8 => {
            if world.manor.is_manor_approved() {
                // Same approved-period guard (and message) as request 7.
                send_to_client(
                    world,
                    client_id,
                    server_packets::system_message_with(
                        sm_ids::A_MANOR_CANNOT_BE_SET_UP_BETWEEN_4_30_AM_AND_8_PM,
                        &[],
                    ),
                );
                return;
            }
            let crops = crop_setting_entries(world, castle_id);
            send_to_client(
                world,
                client_id,
                server_packets::ex_show_crop_setting(castle_id, &crops),
            );
        }
        _ => {
            warn!("Manor: unknown manor request {request}.");
        }
    }
}

mod entries;
mod packets;
mod persist;
mod schedule;
mod settlement;

use entries::{
    crop_info_entries, crop_setting_entries, default_entries, reference_price, seed_info_entries,
    seed_setting_entries,
};
pub(crate) use packets::{
    handle_request_buy_seed, handle_request_procure_crop_list, handle_request_set_crop,
    handle_request_set_seed,
};

pub(crate) use persist::{arm_autosave, handle_autosave, save_all_on_shutdown};

pub(crate) use schedule::{advance_manor_mode, next_mode_change_at, schedule_manor_at_boot};

pub(crate) use settlement::manor_cost;

/// Parse `…?ask=<int>&state=<int>&time=<0|1>` (Java splits on `?`, then `&`,
/// each `key=value` on `=`, and `time` is `.equals("1")`).
fn parse_manor_select(command: &str) -> Option<(i32, i32, bool)> {
    let query = command.split_once('?')?.1;
    let mut ask = None;
    let mut state = None;
    let mut time = None;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=')?;
        match key {
            "ask" => ask = value.parse().ok(),
            "state" => state = value.parse().ok(),
            "time" => time = Some(value == "1"),
            _ => {}
        }
    }
    Some((ask?, state?, time?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manor_menu_select() {
        assert_eq!(
            parse_manor_select("manor_menu_select?ask=3&state=-1&time=0"),
            Some((3, -1, false))
        );
        assert_eq!(
            parse_manor_select("manor_menu_select?ask=7&state=5&time=1"),
            Some((7, 5, true))
        );
        assert_eq!(parse_manor_select("manor_menu_select"), None);
    }

    #[test]
    fn maps_every_chamberlain_to_its_castle() {
        // Both the light and the dark chamberlain of each castle resolve to it.
        assert_eq!(chamberlain_castle_id(35100), Some(1));
        assert_eq!(chamberlain_castle_id(36653), Some(1));
        assert_eq!(chamberlain_castle_id(35555), Some(9));
        assert_eq!(chamberlain_castle_id(36661), Some(9));
        assert_eq!(chamberlain_castle_id(30001), None);
    }

    // The dist cutover times: refresh 20:00, maintenance 6 min, approve 04:30.
    const DIST_TIMES: ModeTimes = ModeTimes {
        refresh_h: 20,
        refresh_m: 0,
        maintenance_m: 6,
        approve_h: 4,
        approve_m: 30,
    };

    fn at(day: i64, hour: i32, min: i32) -> i64 {
        day * MILLIS_PER_DAY + hour as i64 * MILLIS_PER_HOUR + min as i64 * MILLIS_PER_MINUTE
    }

    #[test]
    fn boot_mode_matches_java_windows() {
        let d = 20_000;
        // Daytime (04:30–20:00) is the settled, approved period.
        assert_eq!(boot_mode(at(d, 10, 0), DIST_TIMES), ManorMode::Approved);
        assert_eq!(boot_mode(at(d, 4, 45), DIST_TIMES), ManorMode::Approved);
        // Overnight (past 20:06, before 04:30) is the editable period.
        assert_eq!(boot_mode(at(d, 2, 0), DIST_TIMES), ManorMode::Modifiable);
        assert_eq!(boot_mode(at(d, 4, 15), DIST_TIMES), ManorMode::Modifiable);
        assert_eq!(boot_mode(at(d, 20, 10), DIST_TIMES), ManorMode::Modifiable);
        // The 6-minute maintenance window right at refresh time.
        assert_eq!(boot_mode(at(d, 20, 3), DIST_TIMES), ManorMode::Maintenance);
        // Java quirk (kept verbatim): late evening with min < 6 guesses APPROVED;
        // the immediate-fire cascade corrects it.
        assert_eq!(boot_mode(at(d, 22, 0), DIST_TIMES), ManorMode::Approved);
    }

    #[test]
    fn next_mode_change_times() {
        let d = 20_000;
        // APPROVED → today's refresh (20:00), even when already past (cascade).
        assert_eq!(
            next_mode_change_millis(ManorMode::Approved, at(d, 10, 0), DIST_TIMES),
            at(d, 20, 0)
        );
        assert_eq!(
            next_mode_change_millis(ManorMode::Approved, at(d, 21, 0), DIST_TIMES),
            at(d, 20, 0),
            "a past refresh time is returned as-is to fire immediately"
        );
        // MAINTENANCE → refresh + maintenance (20:06).
        assert_eq!(
            next_mode_change_millis(ManorMode::Maintenance, at(d, 20, 3), DIST_TIMES),
            at(d, 20, 6)
        );
        // MODIFIABLE → next approve time (04:30), +1 day when already past.
        assert_eq!(
            next_mode_change_millis(ManorMode::Modifiable, at(d, 2, 0), DIST_TIMES),
            at(d, 4, 30)
        );
        assert_eq!(
            next_mode_change_millis(ManorMode::Modifiable, at(d, 5, 0), DIST_TIMES),
            at(d + 1, 4, 30),
            "a past approve time rolls to tomorrow"
        );
    }
}
