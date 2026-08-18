//! The ex_show_* packet entry builders for seeds and crops.

/// `ExShowSeedSetting`'s list — every seed the castle can farm, with its
/// catalogue limits/prices and the owner's current/next-period settings.
use crate::network::server_packets::CropInfoEntry;
use crate::network::server_packets::CropSettingEntry;
use crate::network::server_packets::ManorDefaultEntry;
use crate::network::server_packets::SeedInfoEntry;
use crate::network::server_packets::SeedSettingEntry;
use crate::world::World;
pub(super) fn seed_setting_entries(world: &World, castle_id: i32) -> Vec<SeedSettingEntry> {
    let rate = world.cfg.rates.rate_drop_manor;
    world
        .data
        .manor
        .seeds_for_castle(castle_id)
        .iter()
        .map(|seed| {
            let price = reference_price(world, seed.seed_id);
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
pub(super) fn crop_setting_entries(world: &World, castle_id: i32) -> Vec<CropSettingEntry> {
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
/// [`SeedProduction`]: SeedProduction
pub(super) fn seed_info_entries(
    world: &World,
    castle_id: i32,
    next_period: bool,
) -> Vec<SeedInfoEntry> {
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
/// [`CropProcure`]: CropProcure
pub(super) fn crop_info_entries(
    world: &World,
    castle_id: i32,
    next_period: bool,
) -> Vec<CropInfoEntry> {
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
pub(super) fn default_entries(world: &World) -> Vec<ManorDefaultEntry> {
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

pub(super) fn reference_price(world: &World, item_id: i32) -> i32 {
    world
        .data
        .item_data
        .get(item_id)
        .map_or(1, |t| t.price as i32)
}
