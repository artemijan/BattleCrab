//! Pet experience: exp split with the owner, levelling, and collar enchant
//! sync.

use super::PetInfoKind;
use super::is_uncontrollable;
use super::notify_owner;
use super::npc_template_id;
use super::pet_of;
use super::recalculate_pet_stats;
use super::send_pet_info;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::net::broadcast;
use crate::network::server_packets;
use crate::world::World;
/// Java `Config.ALT_PARTY_RANGE` — the pet only earns while it is near enough
/// to its owner to have plausibly helped.
const PET_EXP_RANGE: f64 = 1500.0;

/// The owner's share of a kill's exp/sp, and the pet's cut, as Java's
/// `PlayerStat.addExpAndSp` computes it.
///
/// `get_exp_type` is the **owner's** percentage (73 on most species), so the
/// pet takes the remainder. The owner's own award is then multiplied by that
/// same ratio — the pet's exp is taken *from* the owner, not minted on top, so
/// hunting with a pet genuinely costs the player exp.
///
/// Returns `(owner_ratio, pet_exp, pet_sp)`. `owner_ratio` is 1.0 with no
/// eligible pet, which leaves the owner's award untouched.
pub(crate) fn split_exp_with_pet(
    world: &World,
    owner_oid: i32,
    exp: f64,
    sp: f64,
) -> (f64, f64, f64) {
    let Some(pet_oid) = pet_of(world, owner_oid) else {
        return (1.0, 0.0, 0.0);
    };
    // A dead pet earns nothing (Java `if (!pet.isDead())`), but note the
    // owner's ratio is still reduced — Java adjusts it outside that guard, so
    // the exp is lost rather than returned to the player. Faithful.
    if is_dead(world, pet_oid) {
        return (1.0, 0.0, 0.0);
    }
    if !crate::geo::distance::within_3d(world, owner_oid, pet_oid, PET_EXP_RANGE) {
        return (1.0, 0.0, 0.0);
    }
    let level = world
        .objects
        .get_component::<crate::model::components::summons::PetOf>(&pet_oid)
        .map(|p| p.level)
        .unwrap_or(1);
    let owner_taken = npc_template_id(world, pet_oid)
        .and_then(|id| world.data.pet_data.get(id))
        .and_then(|t| t.levels.get(&level))
        .map(|l| l.owner_exp_taken)
        .unwrap_or(100);
    // "allow possible customizations that would have the pet earning more
    // than 100% of the owner's exp/sp" — but never a negative owner award.
    let ratio = (owner_taken as f64 / 100.0).min(1.0);
    (ratio, exp * (1.0 - ratio), sp * (1.0 - ratio))
}

/// Java `PetStat.addExpAndSp` — award the pet its cut and level it up.
///
/// **A starving pet earns nothing** (`isUncontrollable()` guards `addExp`),
/// which is a real link between the feeding loop and progression rather than
/// an incidental check.
pub(crate) fn add_pet_exp(world: &mut World, owner_oid: i32, exp: f64, sp: f64) {
    use crate::network::server_packets::{SmParam, sm_ids};
    let Some(pet_oid) = pet_of(world, owner_oid) else {
        return;
    };
    if is_uncontrollable(world, pet_oid) {
        return;
    }
    // `PetXpRate` / `SinEaterXpRate` — Java picks the Sin Eater's own rate for
    // that pet and the general one for every other.
    let rate = if crate::scripts::sin_eater::is_sin_eater(world, pet_oid) {
        world.cfg.rates.sin_eater_xp_rate
    } else {
        world.cfg.rates.pet_xp_rate
    };
    let exp = exp * rate;
    let sp = sp * rate;
    let gained = exp.round() as i64;
    if gained <= 0 && sp.round() as i64 <= 0 {
        return;
    }
    let max_level = max_pet_level(world, pet_oid);
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::components::summons::PetOf>(&pet_oid)
    {
        p.exp += gained.max(0);
        p.sp += (sp.round() as i64).max(0);
    }
    notify_owner(
        world,
        owner_oid,
        sm_ids::YOUR_PET_GAINED_S1_XP,
        &[SmParam::Int(gained as i32)],
    );
    level_up_pet(world, owner_oid, pet_oid, max_level);
    send_pet_info(world, owner_oid, pet_oid, PetInfoKind::Default);
}

/// The highest level this species has a row for. Java caps at
/// `ExperienceData.getMaxPetLevel() - 1`; here the species table is the
/// authority, and it is what every per-level lookup needs anyway.
fn max_pet_level(world: &World, pet_oid: i32) -> i32 {
    npc_template_id(world, pet_oid)
        .and_then(|id| world.data.pet_data.get(id))
        .and_then(|t| t.levels.keys().copied().max())
        .unwrap_or(1)
}

/// Advance the pet through every level its new exp total has earned.
fn level_up_pet(world: &mut World, owner_oid: i32, pet_oid: i32, max_level: i32) {
    let Some(npc_id) = npc_template_id(world, pet_oid) else {
        return;
    };
    let mut levelled = false;
    loop {
        let Some(pet) = world
            .objects
            .get_component::<crate::model::components::summons::PetOf>(&pet_oid)
            .copied()
        else {
            return;
        };
        if pet.level >= max_level {
            break;
        }
        let next = pet.level + 1;
        let needed = world
            .data
            .pet_data
            .get(npc_id)
            .map(|t| t.exp_for_level(next))
            .unwrap_or(i64::MAX);
        if needed <= 0 || pet.exp < needed {
            break;
        }
        // The food bar's capacity is per level, so it moves with the level.
        let new_max_fed = world
            .data
            .pet_data
            .get(npc_id)
            .map(|t| t.max_meal(next))
            .unwrap_or(pet.max_fed);
        if let Some(p) = world
            .objects
            .get_component_mut::<crate::model::components::summons::PetOf>(&pet_oid)
        {
            p.level = next;
            p.max_fed = new_max_fed;
            p.fed = p.fed.min(new_max_fed);
        }
        levelled = true;
    }
    if levelled {
        // The new level's stat row is what makes levelling mean anything.
        recalculate_pet_stats(world, pet_oid);
    }
    if levelled {
        // Java sends no system message for a pet level — just the animation.
        let pkt = server_packets::social_action(pet_oid, SOCIAL_LEVEL_UP);
        broadcast::broadcast_including_self(world, owner_oid, &pkt);
        sync_collar_enchant(world, owner_oid, pet_oid);
    }
}

/// `getControlItem().setEnchantLevel(getLevel())` — the collar's enchant level
/// *is* the pet's level, which is how the client shows "Wolf Collar +12" and
/// how a traded pet advertises what it is without being summoned.
pub(crate) fn sync_collar_enchant(world: &mut World, owner_oid: i32, pet_oid: i32) {
    let Some(pet) = world
        .objects
        .get_component::<crate::model::components::summons::PetOf>(&pet_oid)
        .copied()
    else {
        return;
    };
    if let Some(inv) = world
        .objects
        .get_component_mut::<crate::model::inventory::Inventory>(&owner_oid)
    {
        inv.set_item_enchant(pet.collar_object_id, pet.level);
    }
}

/// `SocialAction.LEVEL_UP`.
const SOCIAL_LEVEL_UP: i32 = 15;
