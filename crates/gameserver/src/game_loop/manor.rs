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
//! `AltManor*` cutover times and runs the production rollover; only the economic
//! settlement folded into Java's rollover remains `TODO(manor)`.
//!
//! The player-facing Manor Manager trader is [`handle_request_buy_seed`]
//! (`RequestBuySeed`, buy seeds from a castle's current production) and
//! [`handle_request_procure_crop_list`] (`RequestProcureCropList`, sell crops
//! for the crop's reward item, with a 5 % adena fee across castles). Note that
//! the reference build never sends the buy/sell *display* packets (`BuyListSeed`
//! /`ExShowSellCropList` are dead), so the trader window is client-native.

use commons::network::PacketReader;
use tracing::warn;

use crate::model::Player;
use crate::model::clan::CS_MANOR_ADMIN;
use crate::model::components::LastFolkNpc;
use crate::model::manor::{CropProcure, ManorMode, SeedProduction};
use crate::model::npc::Npc;
use crate::network::server_packets::{
    self, CropInfoEntry, CropSettingEntry, ManorDefaultEntry, SeedInfoEntry, SeedSettingEntry,
    SmParam, sm_ids,
};
use crate::scheduler::ScheduledTask;
use crate::world::World;

use super::death::ADENA_ID;

const MILLIS_PER_DAY: i64 = 86_400_000;
const MILLIS_PER_HOUR: i64 = 3_600_000;
const MILLIS_PER_MIN: i64 = 60_000;
const TICKS_PER_SECOND: u64 = 10;

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

// ---------------------------------------------------------------------------
// Period scheduler — port of `CastleManorManager`'s wall-clock mode machine.
// ---------------------------------------------------------------------------

/// The five daily cutover times (from `General.ini`), pulled off config.
#[derive(Debug, Clone, Copy)]
struct ModeTimes {
    refresh_h: i32,
    refresh_m: i32,
    maintenance_m: i32,
    approve_h: i32,
    approve_m: i32,
}

fn mode_times(world: &World) -> ModeTimes {
    let g = &world.cfg.general;
    ModeTimes {
        refresh_h: g.alt_manor_refresh_time,
        refresh_m: g.alt_manor_refresh_min,
        maintenance_m: g.alt_manor_maintenance_min,
        approve_h: g.alt_manor_approve_time,
        approve_m: g.alt_manor_approve_min,
    }
}

fn daily_millis(now_millis: i64, hour: i32, minute: i32) -> i64 {
    let day = now_millis.div_euclid(MILLIS_PER_DAY);
    day * MILLIS_PER_DAY + hour as i64 * MILLIS_PER_HOUR + minute as i64 * MILLIS_PER_MIN
}

/// Port of `CastleManorManager` init's wall-clock mode guess. The `refresh`
/// clause's `min >= maintenanceMin` check ignores the hour (a Java quirk kept
/// verbatim — the immediate-fire cascade in [`arm_next_mode_change`] corrects
/// any wrong guess within a tick or two).
fn boot_mode(now_millis: i64, t: ModeTimes) -> ManorMode {
    let day = now_millis.div_euclid(MILLIS_PER_DAY);
    let mins_into_day = (now_millis - day * MILLIS_PER_DAY) / MILLIS_PER_MIN;
    let hour = (mins_into_day / 60) as i32;
    let min = (mins_into_day % 60) as i32;
    let maintenance_min = t.refresh_m + t.maintenance_m;
    if (hour >= t.refresh_h && min >= maintenance_min)
        || hour < t.approve_h
        || (hour == t.approve_h && min <= t.approve_m)
    {
        ManorMode::Modifiable
    } else if hour == t.refresh_h && min >= t.refresh_m && min < maintenance_min {
        ManorMode::Maintenance
    } else {
        ManorMode::Approved
    }
}

/// Port of `scheduleModeChange`'s next-change time for the *current* mode. Only
/// `MODIFIABLE` gets Java's "+1 day if already past" guard; `APPROVED`/
/// `MAINTENANCE` return today's time even when past, so a stale boot mode
/// fires immediately and cascades to the right one (Java's `Math.max(0, …)`).
fn next_mode_change_millis(mode: ManorMode, now_millis: i64, t: ModeTimes) -> i64 {
    match mode {
        ManorMode::Modifiable => {
            let at = daily_millis(now_millis, t.approve_h, t.approve_m);
            if at < now_millis {
                at + MILLIS_PER_DAY
            } else {
                at
            }
        }
        ManorMode::Maintenance => {
            daily_millis(now_millis, t.refresh_h, t.refresh_m + t.maintenance_m)
        }
        // APPROVED (and the DISABLED fallback, which is never scheduled).
        _ => daily_millis(now_millis, t.refresh_h, t.refresh_m),
    }
}

/// Set the initial mode from the wall clock and arm the first change — the data
/// half of `CastleManorManager` init. When the manor is disabled the mode is
/// `DISABLED` and nothing is scheduled (Java's `else` branch). Called from the
/// `ManorLoaded` boot handler.
pub(crate) fn schedule_manor_at_boot(world: &mut World) {
    if !world.cfg.general.allow_manor {
        world.manor.set_mode(ManorMode::Disabled);
        return;
    }
    let now = commons::util::now_millis();
    let mode = boot_mode(now, mode_times(world));
    world.manor.set_mode(mode);
    arm_next_mode_change(world, now);
}

fn arm_next_mode_change(world: &mut World, now_millis: i64) {
    let at = next_mode_change_millis(world.manor.mode(), now_millis, mode_times(world));
    let delay_ticks = ((at - now_millis).max(0) / 1000) as u64 * TICKS_PER_SECOND;
    world
        .scheduler
        .schedule(world.tick + delay_ticks, ScheduledTask::ManorModeChange);
}

/// Port of `CastleManorManager.changeMode` — advance the period and re-arm the
/// next change. The mode transition + the production rollover
/// ([`crate::model::manor::ManorState::roll_period`]) are applied; the economic
/// settlement is deferred.
pub(crate) fn advance_manor_mode(world: &mut World) {
    let next_mode = match world.manor.mode() {
        ManorMode::Approved => {
            // Roll every owned castle's manor into the new period.
            let owned: Vec<i32> = world
                .data
                .manor
                .manor_castle_ids()
                .into_iter()
                .filter(|&id| castle_owner_clan_id(world, id).is_some())
                .collect();
            for castle_id in owned {
                // TODO(manor): Java also settles the closing period here — pay
                // bought crops (`getMatureId`, ×0.9) into the owner's clan
                // warehouse and refund unused reservation to the castle treasury,
                // then gate the next period on treasury affordability. That needs
                // the castle treasury (unported) + warehouse item-adds; only the
                // production rollover is applied for now.
                world.manor.roll_period(castle_id);
            }
            ManorMode::Maintenance
        }
        ManorMode::Maintenance => {
            // TODO(manor): Java notifies each owner's online leader with
            // `THE_MANOR_INFORMATION_HAS_BEEN_UPDATED` here.
            ManorMode::Modifiable
        }
        ManorMode::Modifiable => {
            // TODO(manor): Java charges the manor cost / validates warehouse
            // capacity here, clearing the next period + warning the leader when
            // the treasury can't cover it. Deferred with the treasury economics.
            ManorMode::Approved
        }
        // A disabled manor never scheduled a change; nothing to do.
        ManorMode::Disabled => return,
    };
    world.manor.set_mode(next_mode);
    arm_next_mode_change(world, commons::util::now_millis());
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

const MAX_ADENA: i64 = 99_999_999_999;

/// The Manor Manager's `manor_id` NPC parameter, if the player's last folk NPC
/// is a Merchant in interaction range whose `manor_id` matches (Java's
/// `manager instanceof Merchant && canInteract && getParameters().getInt(...)`).
fn manor_manager_castle(world: &World, player_oid: i32) -> Option<i32> {
    let &LastFolkNpc(npc) = world.objects.get_component::<LastFolkNpc>(&player_oid)?;
    if !super::shop::is_merchant(world, npc) || !super::target::can_interact(world, player_oid, npc)
    {
        return None;
    }
    let castle = world
        .objects
        .get_component::<Npc>(&npc)
        .and_then(|n| n.template(world))
        .map(|t| t.ai_param_i32("manor_id", -1))?;
    (castle >= 0).then_some(castle)
}

/// Port of `clientpackets/RequestBuySeed` — a player buys seeds from a Manor
/// Manager's current-period production. Reads `manorId, count, [seedId, cnt]*`;
/// validates the seeds (price/stock/adena) against `ManorState`, takes the adena
/// and decrements the manor's stock, and hands over the seeds.
pub(crate) fn handle_request_buy_seed(world: &mut World, client_id: u32, body: &[u8]) {
    const BATCH: usize = 4 + 8; // itemId + count
    let mut r = PacketReader::new(body);
    let (Some(manor_id), Some(count)) = (r.read_i32(), r.read_i32()) else {
        return;
    };
    if count <= 0 || count > 1000 || r.remaining() != count as usize * BATCH {
        return;
    }
    let mut items = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let (Some(item_id), Some(cnt)) = (r.read_i32(), r.read_i64()) else {
            return;
        };
        if cnt < 1 || item_id < 1 {
            return;
        }
        items.push((item_id, cnt));
    }

    let player_oid = match world.clients.get(&client_id) {
        Some(crate::session::ClientSession::InGame(s)) => s.player_object_id(),
        _ => return,
    };
    // Java gate: not under maintenance, the castle exists, and the last folk NPC
    // is this castle's Manor Manager in range.
    if world.manor.is_under_maintenance()
        || !world.data.manor.manor_castle_ids().contains(&manor_id)
        || manor_manager_castle(world, player_oid) != Some(manor_id)
    {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }

    // Validate every line against the live production, summing the price.
    let mut total_price = 0i64;
    for &(item_id, cnt) in &items {
        let ok = world
            .manor
            .seed_product(manor_id, item_id, false)
            .is_some_and(|sp| sp.price > 0 && sp.amount >= cnt && MAX_ADENA / cnt >= sp.price);
        if !ok {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::action_failed());
            }
            return;
        }
        let price = world
            .manor
            .seed_product(manor_id, item_id, false)
            .map_or(0, |sp| sp.price);
        total_price += price * cnt;
        if total_price > MAX_ADENA {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::action_failed());
            }
            return;
        }
    }
    // TODO(manor): Java also validates inventory weight/capacity here
    // (`validateWeight`/`validateCapacity`); the shop buy path skips these too.

    let adena = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&player_oid)
        .map_or(0, |i| i.adena());
    if adena < total_price {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(
                sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA,
                &[],
            ));
        }
        return;
    }
    if total_price > 0
        && !super::quests::take_items(world, client_id, player_oid, ADENA_ID, total_price)
    {
        return;
    }

    // Deliver: decrement each seed's stock and add it to the buyer.
    let mut added: Vec<i32> = Vec::new();
    for &(item_id, cnt) in &items {
        // A concurrent overdraw can't happen on the single game thread, but the
        // `decrease_amount` guard mirrors Java's per-line refund-on-failure.
        if world.manor.decrease_seed_amount(manor_id, item_id, cnt)
            && let Some(oids) = super::items::add_inventory_item(world, player_oid, item_id, cnt)
        {
            added.extend(oids);
        }
    }
    // Java: the sale price goes to the castle's vault, untaxed. An unowned
    // castle takes nothing (`addToTreasuryNoTax` returns false on `_ownerId <= 0`),
    // so the adena the buyer just paid simply leaves the economy.
    if total_price > 0 {
        super::castle::add_to_treasury_no_tax(world, manor_id, total_price);
    }
    if let (Some(inventory), Some(cs)) = (
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&player_oid),
        world.clients.get(&client_id),
    ) {
        cs.send(crate::network::enter_world::inventory_update(
            inventory,
            &world.data,
            &added,
        ));
        cs.send(crate::network::enter_world::ex_user_info_inven_weight(
            player_oid,
            inventory,
            &world.data,
        ));
        if total_price > 0 {
            cs.send(server_packets::system_message_with(
                sm_ids::S1_ADENA_DISAPPEARED,
                &[SmParam::Long(total_price)],
            ));
        }
    }
}

/// Port of `clientpackets/RequestProcureCropList` — a player sells crops to a
/// Manor Manager for the crop's reward item. Reads
/// `count, [objId, cropId, manorId, cnt]*`; validates every line against the
/// inventory + `CropProcure` state, then per line pays out
/// `price / rewardReferencePrice` of the reward item, charging a 5 % adena fee
/// when selling to a manor other than where the crop's procurement is set.
pub(crate) fn handle_request_procure_crop_list(world: &mut World, client_id: u32, body: &[u8]) {
    use crate::model::inventory::Inventory;
    const BATCH: usize = 4 + 4 + 4 + 8; // objId + cropId + manorId + cnt
    let mut r = PacketReader::new(body);
    let Some(count) = r.read_i32() else {
        return;
    };
    if count <= 0 || count > 1000 || r.remaining() != count as usize * BATCH {
        return;
    }
    let mut items = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let (Some(obj_id), Some(crop_id), Some(item_manor), Some(cnt)) =
            (r.read_i32(), r.read_i32(), r.read_i32(), r.read_i64())
        else {
            return;
        };
        if obj_id < 1 || crop_id < 1 || item_manor < 0 || cnt < 0 {
            return;
        }
        items.push((obj_id, crop_id, item_manor, cnt));
    }

    let player_oid = match world.clients.get(&client_id) {
        Some(crate::session::ClientSession::InGame(s)) => s.player_object_id(),
        _ => return,
    };
    // Gate: not under maintenance, and the last folk NPC is a Manor Manager in
    // range (its `manor_id` param is the manager's castle).
    let Some(castle_id) = (if world.manor.is_under_maintenance() {
        None
    } else {
        manor_manager_castle(world, player_oid)
    }) else {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    };

    // Loop 1: validate every line (any failure rejects the whole packet).
    for &(obj_id, crop_id, item_manor, cnt) in &items {
        let item_ok = world
            .objects
            .get_component::<Inventory>(&player_oid)
            .and_then(|i| i.item_by_object_id(obj_id))
            .is_some_and(|(id, held)| id == crop_id && held >= cnt);
        let cp_ok = world
            .manor
            .crop_procure_for(item_manor, crop_id, false)
            .is_some_and(|cp| cp.amount >= cnt);
        if !item_ok || !cp_ok {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::action_failed());
            }
            return;
        }
    }

    // Loop 2: execute, skipping (with a message) lines that can't pay out.
    let mut crop_changes = Vec::new();
    let mut reward_oids: Vec<i32> = Vec::new();
    for &(obj_id, crop_id, item_manor, cnt) in &items {
        let (price, reward_type) = world
            .manor
            .crop_procure_for(item_manor, crop_id, false)
            .map(|cp| (cnt * cp.price, cp.reward_type))
            .expect("validated in loop 1");
        let Some(reward_id) = world
            .data
            .manor
            .seed_by_crop(crop_id)
            .map(|s| s.reward(reward_type))
        else {
            continue;
        };
        let reward_price = reference_price(world, reward_id) as i64;
        if reward_price == 0 {
            continue;
        }
        let reward_count = price / reward_price;
        if reward_count < 1 {
            // Java sends `FAILED_IN_TRADING_S2_OF_S1_CROPS` and skips.
            // TODO(manor): source that SystemMessageId (not in this repo's data).
            continue;
        }
        // A 5 % adena fee when selling at a manor other than the crop's own.
        let fee = if castle_id == item_manor {
            0
        } else {
            (price as f64 * 0.05) as i64
        };
        if fee > 0 {
            let adena = world
                .objects
                .get_component::<Inventory>(&player_oid)
                .map_or(0, |i| i.adena());
            if adena < fee {
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(server_packets::system_message_with(
                        sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA,
                        &[],
                    ));
                }
                continue;
            }
        }

        // Everything validated → decrement the procurement, take the fee, take
        // the crops, hand over the reward.
        if !world.manor.decrease_crop_amount(item_manor, crop_id, cnt) {
            continue;
        }
        if fee > 0 {
            super::quests::take_items(world, client_id, player_oid, ADENA_ID, fee);
        }
        if let Some(change) = world
            .objects
            .get_component_mut::<Inventory>(&player_oid)
            .and_then(|inv| inv.remove_by_object_id(obj_id, cnt))
        {
            crop_changes.push(change);
        }
        if let Some(oids) =
            super::items::add_inventory_item(world, player_oid, reward_id, reward_count)
        {
            reward_oids.extend(oids);
        }
    }

    // Reflect the sold crops and the received rewards.
    if !crop_changes.is_empty()
        && let Some(cs) = world.clients.get(&client_id)
    {
        cs.send(crate::network::enter_world::inventory_update_changes(
            &world.data,
            &crop_changes,
        ));
    }
    if !reward_oids.is_empty()
        && let (Some(inventory), Some(cs)) = (
            world.objects.get_component::<Inventory>(&player_oid),
            world.clients.get(&client_id),
        )
    {
        cs.send(crate::network::enter_world::inventory_update(
            inventory,
            &world.data,
            &reward_oids,
        ));
        cs.send(crate::network::enter_world::ex_user_info_inven_weight(
            player_oid,
            inventory,
            &world.data,
        ));
    }
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

    // The dist cutover times: refresh 20:00, maintenance 6 min, approve 04:30.
    const DIST_TIMES: ModeTimes = ModeTimes {
        refresh_h: 20,
        refresh_m: 0,
        maintenance_m: 6,
        approve_h: 4,
        approve_m: 30,
    };

    fn at(day: i64, hour: i32, min: i32) -> i64 {
        day * MILLIS_PER_DAY + hour as i64 * MILLIS_PER_HOUR + min as i64 * MILLIS_PER_MIN
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
