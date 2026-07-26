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
//! The setup path is gated to the manor's **modifiable** period; the wall-clock
//! period scheduler (mode transitions + the daily production rollover) is not
//! ported yet, so the mode stays at its default (`Approved`) until a future
//! slice drives it.

use commons::network::PacketReader;
use tracing::warn;

use crate::model::clan::CS_MANOR_ADMIN;
use crate::model::components::LastFolkNpc;
use crate::model::manor::{CropProcure, SeedProduction};
use crate::model::npc::Npc;
use crate::model::Player;
use crate::network::server_packets::{
    self, CropInfoEntry, CropSettingEntry, ManorDefaultEntry, SeedInfoEntry, SeedSettingEntry,
};
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
        // Request 7: the owner's "Edit Seed Setup" view (`ExShowSeedSetting`).
        7 => {
            if world.manor.is_manor_approved() {
                // Java sends `A_MANOR_CANNOT_BE_SET_UP_BETWEEN_4_30_AM_AND_8_PM`
                // then returns. TODO(manor): source that SystemMessageId from the
                // client dat (not in this repo); the gate itself — no setup
                // outside the modifiable period — is honored here.
                return;
            }
            let seeds = seed_setting_entries(world, castle_id);
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::ex_show_seed_setting(castle_id, &seeds));
            }
        }
        // Request 8: the owner's "Edit Crop Setup" view (`ExShowCropSetting`).
        8 => {
            if world.manor.is_manor_approved() {
                return; // same approved-period guard as request 7
            }
            let crops = crop_setting_entries(world, castle_id);
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::ex_show_crop_setting(castle_id, &crops));
            }
        }
        _ => {
            warn!("Manor: unknown manor request {request}.");
        }
    }
}

/// `Seed.getSeedReferencePrice` — the seed item's reference price (Java `Seed`
/// resolves this from item data at load; missing item ⇒ 1).
fn seed_reference_price(world: &World, seed_id: i32) -> i32 {
    reference_price(world, seed_id)
}

/// `ExShowSeedSetting`'s list — every seed the castle can farm, with its
/// catalogue limits/prices and the owner's current/next-period settings.
fn seed_setting_entries(world: &World, castle_id: i32) -> Vec<SeedSettingEntry> {
    let rate = world.cfg.rates.rate_drop_manor;
    world
        .data
        .manor
        .seeds_for_castle(castle_id)
        .iter()
        .map(|seed| {
            let price = seed_reference_price(world, seed.seed_id);
            SeedSettingEntry {
                seed_id: seed.seed_id,
                level: seed.level,
                reward1_item_id: seed.reward1,
                reward2_item_id: seed.reward2,
                seed_limit: seed.limit_seeds * rate,
                seed_reference_price: price,
                seed_min_price: (price as f64 * 0.6) as i32,
                seed_max_price: price * 10,
                current: world
                    .manor
                    .seed_product(castle_id, seed.seed_id, false)
                    .map(|sp| (sp.start_amount, sp.price)),
                next: world
                    .manor
                    .seed_product(castle_id, seed.seed_id, true)
                    .map(|sp| (sp.start_amount, sp.price)),
            }
        })
        .collect()
}

/// `ExShowCropSetting`'s list — every crop the castle can buy, with its
/// catalogue limits/prices and the owner's current/next-period settings.
fn crop_setting_entries(world: &World, castle_id: i32) -> Vec<CropSettingEntry> {
    let rate = world.cfg.rates.rate_drop_manor;
    world
        .data
        .manor
        .seeds_for_castle(castle_id)
        .iter()
        .map(|seed| {
            let price = reference_price(world, seed.crop_id);
            CropSettingEntry {
                crop_id: seed.crop_id,
                level: seed.level,
                reward1_item_id: seed.reward1,
                reward2_item_id: seed.reward2,
                crop_limit: seed.limit_crops * rate,
                crop_min_price: (price as f64 * 0.6) as i32,
                crop_max_price: price * 10,
                current: world
                    .manor
                    .crop_procure_for(castle_id, seed.crop_id, false)
                    .map(|cp| (cp.start_amount, cp.price, cp.reward_type as u8)),
                next: world
                    .manor
                    .crop_procure_for(castle_id, seed.crop_id, true)
                    .map(|cp| (cp.start_amount, cp.price, cp.reward_type as u8)),
            }
        })
        .collect()
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

/// Java `RequestSetSeed`/`RequestSetCrop`'s shared owner gate. Returns the
/// player object id when: the manor is in its **modifiable** period, the
/// player's clan owns castle `manor_id`, they hold `CS_MANOR_ADMIN`, and they
/// are in range of the chamberlain (last folk NPC). Otherwise sends
/// `ActionFailed` and returns `None`, mirroring Java's early-outs.
fn manor_setup_gate(world: &mut World, client_id: u32, manor_id: i32) -> Option<i32> {
    let player_oid = match world.clients.get(&client_id) {
        Some(crate::session::ClientSession::InGame(s)) => s.player_object_id(),
        _ => return None,
    };
    let ok = world.manor.is_modifiable_period() && {
        let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
            return fail(world, client_id);
        };
        let owns = p.clan_id != 0
            && world.clans.get(&p.clan_id).is_some_and(|c| {
                c.castle_id == manor_id && c.has_privilege(player_oid, p.clan_privs, CS_MANOR_ADMIN)
            });
        // The last folk NPC (the chamberlain) must be in interaction range.
        let in_range = world
            .objects
            .get_component::<LastFolkNpc>(&player_oid)
            .is_some_and(|&LastFolkNpc(npc)| super::target::can_interact(world, player_oid, npc));
        owns && in_range
    };
    if ok {
        Some(player_oid)
    } else {
        fail(world, client_id)
    }
}

/// Send `ActionFailed` and yield `None` (the gate's rejection path).
fn fail(world: &World, client_id: u32) -> Option<i32> {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::action_failed());
    }
    None
}

/// Port of `clientpackets/RequestSetSeed` — the owner submits the next-period
/// seed setup. Reads `manorId, count, [seedId, sales, price]*`; keeps only known
/// seeds within their limit/price band; replaces the castle's next-period seed
/// production.
pub(crate) fn handle_request_set_seed(world: &mut World, client_id: u32, body: &[u8]) {
    const BATCH: usize = 4 + 8 + 8; // seedId + sales + price
    let mut r = PacketReader::new(body);
    let (Some(manor_id), Some(count)) = (r.read_i32(), r.read_i32()) else {
        return;
    };
    if count <= 0 || count > 1000 || r.remaining() != count as usize * BATCH {
        return;
    }
    let mut items = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let (Some(item_id), Some(sales), Some(price)) = (r.read_i32(), r.read_i64(), r.read_i64())
        else {
            return;
        };
        if item_id < 1 || sales < 0 || price < 0 {
            return;
        }
        if sales > 0 {
            items.push((item_id, sales, price));
        }
    }
    if items.is_empty() {
        return;
    }
    let Some(_player) = manor_setup_gate(world, client_id, manor_id) else {
        return;
    };
    // Filter to known seeds within the setup limit/price band.
    let rate = world.cfg.rates.rate_drop_manor;
    let list: Vec<SeedProduction> = items
        .into_iter()
        .filter_map(|(seed_id, sales, price)| {
            let seed = world.data.manor.seed_by_id(seed_id)?;
            let ref_price = reference_price(world, seed_id);
            let min = (ref_price as f64 * 0.6) as i64;
            let max = ref_price as i64 * 10;
            (sales <= (seed.limit_seeds * rate) as i64 && price >= min && price <= max).then_some(
                SeedProduction {
                    seed_id,
                    amount: sales,
                    price,
                    start_amount: sales,
                },
            )
        })
        .collect();
    world.manor.set_next_seed_production(manor_id, list);
    // TODO(manor): with `AltManorSaveAllActions` (off on this dist) Java
    // persists the next-period rows immediately; otherwise a periodic `storeMe`
    // (unported) does. Either way the setup survives in memory this slice.
    let _ = world.cfg.general.alt_manor_save_all_actions;
}

/// Port of `clientpackets/RequestSetCrop` — the owner submits the next-period
/// crop setup. Like [`handle_request_set_seed`] plus a per-line reward-type
/// byte; keeps only crops the castle farms, within their limit/price band.
pub(crate) fn handle_request_set_crop(world: &mut World, client_id: u32, body: &[u8]) {
    const BATCH: usize = 4 + 8 + 8 + 1; // cropId + sales + price + type
    let mut r = PacketReader::new(body);
    let (Some(manor_id), Some(count)) = (r.read_i32(), r.read_i32()) else {
        return;
    };
    if count <= 0 || count > 1000 || r.remaining() != count as usize * BATCH {
        return;
    }
    let mut items = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let (Some(item_id), Some(sales), Some(price), Some(reward_type)) =
            (r.read_i32(), r.read_i64(), r.read_i64(), r.read_u8())
        else {
            return;
        };
        if item_id < 1 || sales < 0 || price < 0 {
            return;
        }
        if sales > 0 {
            items.push((item_id, sales, price, reward_type as i32));
        }
    }
    if items.is_empty() {
        return;
    }
    let Some(_player) = manor_setup_gate(world, client_id, manor_id) else {
        return;
    };
    let rate = world.cfg.rates.rate_drop_manor;
    let list: Vec<CropProcure> = items
        .into_iter()
        .filter_map(|(crop_id, sales, price, reward_type)| {
            // Java `getSeedByCrop(cropId, castleId)` — the crop must be one this
            // castle actually farms.
            let seed = world
                .data
                .manor
                .seeds_for_castle(manor_id)
                .iter()
                .find(|s| s.crop_id == crop_id)?;
            let ref_price = reference_price(world, crop_id);
            let min = (ref_price as f64 * 0.6) as i64;
            let max = ref_price as i64 * 10;
            (sales <= (seed.limit_crops * rate) as i64 && price >= min && price <= max).then_some(
                CropProcure {
                    crop_id,
                    amount: sales,
                    price,
                    start_amount: sales,
                    reward_type,
                },
            )
        })
        .collect();
    world.manor.set_next_crop_procure(manor_id, list);
    let _ = world.cfg.general.alt_manor_save_all_actions;
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
