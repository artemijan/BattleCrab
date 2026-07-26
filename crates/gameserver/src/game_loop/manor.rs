//! The castle-manor menu, driven by the `manor_menu_select` client bypass —
//! port of `CastleChamberlain.onNpcManorBypass` (the `ON_NPC_MANOR_BYPASS`
//! listener). The chamberlain's `manor` html button opens `manor.html`, whose
//! buttons send `manor_menu_select?ask=<request>&state=<manorId>&time=<0|1>`;
//! this routes each request to its `ExShow*` display packet.
//!
//! Slice scope (G26): the reference view — request 5
//! (`ExShowManorDefaultInfo`), built from the static [`ManorData`] seed
//! catalogue and item reference prices. The seed/crop **production** views
//! (requests 3/4) and the owner **setup** views (requests 7/8) need the
//! `CastleManorManager` runtime state (`SeedProduction`/`CropProcure`), which
//! is unported — see the `TODO(manor)` arms below.

use tracing::warn;

use crate::model::components::LastFolkNpc;
use crate::model::npc::Npc;
use crate::network::server_packets::{self, CropInfoEntry, ManorDefaultEntry, SeedInfoEntry};
use crate::world::World;

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
    let Some(npc_id) = world
        .objects
        .get_component::<Npc>(&npc_object_id)
        .map(|n| n.npc_id)
    else {
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
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::ex_show_seed_info(
                    castle_id,
                    true,
                    Some(&seeds),
                ));
            }
        }
        // Request 4: the "Crop Sales" view — the castle's live crop procure
        // (`ExShowCropInfo`).
        4 => {
            let crops = crop_info_entries(world, castle_id, next_period);
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::ex_show_crop_info(
                    castle_id,
                    true,
                    Some(&crops),
                ));
            }
        }
        // Request 5: the seed/crop reference table (static catalogue).
        5 => {
            let crops = default_entries(world);
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::ex_show_manor_default_info(&crops, true));
            }
        }
        // TODO(manor): requests 7/8 are the owner's *editable* seed/crop setup
        // (`ExShowSeedSetting`/`ExShowCropSetting`) — they need the manor
        // period mode (`isManorApproved`) and the `RequestSetSeed`/
        // `RequestSetCrop` write path, a later slice.
        7 | 8 => {
            warn!(
                "Manor: setup request {request} for castle {castle_id} \
                 (next_period={next_period}) not wired — needs the setup slice (TODO)."
            );
        }
        _ => {
            warn!("Manor: unknown manor request {request}.");
        }
    }
}

/// `ExShowSeedInfo`'s list — the castle's live [`SeedProduction`] for the
/// period, each line's level/rewards resolved from the seed catalogue (Java's
/// `getSeed(seedId)`; unknown ⇒ level 0, rewards 0).
///
/// [`SeedProduction`]: crate::model::manor::SeedProduction
fn seed_info_entries(world: &World, castle_id: i32, next_period: bool) -> Vec<SeedInfoEntry> {
    world
        .manor
        .seed_production(castle_id, next_period)
        .iter()
        .map(|sp| {
            let seed = world.data.manor.seed_by_id(sp.seed_id);
            SeedInfoEntry {
                seed_id: sp.seed_id,
                amount: sp.amount,
                start_amount: sp.start_amount,
                price: sp.price,
                seed_level: seed.map_or(0, |s| s.level),
                reward1_item_id: seed.map_or(0, |s| s.reward1),
                reward2_item_id: seed.map_or(0, |s| s.reward2),
            }
        })
        .collect()
}

/// `ExShowCropInfo`'s list — the castle's live [`CropProcure`] for the period,
/// each line's level/rewards resolved via the crop's seed (Java's
/// `getSeedByCrop(cropId)`; unknown ⇒ level 0, rewards 0).
///
/// [`CropProcure`]: crate::model::manor::CropProcure
fn crop_info_entries(world: &World, castle_id: i32, next_period: bool) -> Vec<CropInfoEntry> {
    world
        .manor
        .crop_procure(castle_id, next_period)
        .iter()
        .map(|cp| {
            let seed = world.data.manor.seed_by_crop(cp.crop_id);
            CropInfoEntry {
                crop_id: cp.crop_id,
                amount: cp.amount,
                start_amount: cp.start_amount,
                price: cp.price,
                reward: cp.reward_type as u8,
                seed_level: seed.map_or(0, |s| s.level),
                reward1_item_id: seed.map_or(0, |s| s.reward1),
                reward2_item_id: seed.map_or(0, |s| s.reward2),
            }
        })
        .collect()
}

/// `ExShowManorDefaultInfo`'s crop list — one line per distinct crop
/// ([`ManorData::all_crops`]) with the seed/crop reference prices resolved from
/// item data (Java `Seed` resolves these from `ItemData` at load; missing item
/// ⇒ price 1, matching Java's `(item != null) ? … : 1`).
fn default_entries(world: &World) -> Vec<ManorDefaultEntry> {
    world
        .data
        .manor
        .all_crops()
        .into_iter()
        .map(|seed| ManorDefaultEntry {
            crop_id: seed.crop_id,
            level: seed.level,
            seed_reference_price: reference_price(world, seed.seed_id),
            crop_reference_price: reference_price(world, seed.crop_id),
            reward1_item_id: seed.reward1,
            reward2_item_id: seed.reward2,
        })
        .collect()
}

fn reference_price(world: &World, item_id: i32) -> i32 {
    world
        .data
        .item_data
        .get(item_id)
        .map_or(1, |t| t.price as i32)
}

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
}
