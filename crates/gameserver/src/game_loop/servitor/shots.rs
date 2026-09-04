//! Summon shots: Beast Soulshot/Spiritshot charging and spending.

use super::npc_template_id;
use crate::game_loop::character::inventory;
use crate::game_loop::helpers::{send_sm_to_player, send_to_player};
use crate::model::components::ServitorOf;
use crate::network::server_packets;
use crate::world::World;
/// Which Beast shot a recharge is after. Java has one
/// `Summon.rechargeShots(physical, magic, blessed)` covering both; the two
/// differ only in the four values gathered here.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SummonShot {
    Soulshot,
    Spiritshot,
}

impl SummonShot {
    /// The `default_action` that marks an auto-shot entry as this kind's.
    fn action(self) -> crate::data::item_data::kinds::ActionType {
        use crate::data::item_data::kinds::ActionType;
        match self {
            SummonShot::Soulshot => ActionType::SummonSoulshot,
            SummonShot::Spiritshot => ActionType::SummonSpiritshot,
        }
    }

    /// The pet level row's per-swing cost column.
    fn per_hit(self, level: &crate::data::pet_data::PetLevel) -> i32 {
        match self {
            SummonShot::Soulshot => level.soulshot_count,
            SummonShot::Spiritshot => level.spiritshot_count,
        }
    }

    /// `ExAutoSoulShot`'s type field, for the toggle-off echo.
    fn shot_type(self) -> i32 {
        match self {
            SummonShot::Soulshot => crate::model::ShotType::Soulshots as i32,
            SummonShot::Spiritshot => crate::model::ShotType::Spiritshots as i32,
        }
    }
}

/// Java `Player.disableAutoShot` — take an item off the auto-use list and tell
/// the client, so the lit toggle in the shot bar actually goes dark.
///
/// The echo is the point: the server is turning this off on the player's
/// behalf, and without `ExAutoSoulShot` the client keeps showing auto-use
/// armed for a shot that will never fire again.
fn disable_auto_shot(world: &mut World, owner: i32, item_id: i32, shot_type: i32) {
    if !crate::game_loop::items::remove_auto_shot(world, owner, item_id) {
        return;
    }
    send_to_player(
        world,
        owner,
        server_packets::ex_auto_soul_shot(item_id, false, shot_type),
    );
    send_sm_to_player(
        world,
        owner,
        server_packets::sm_ids::THE_AUTOMATIC_USE_OF_S1_HAS_BEEN_DEACTIVATED,
        &[server_packets::SmParam::ItemName(item_id)],
    );
}

/// Java `Summon.rechargeShots` plus the `BeastSoulShot`/`BeastSpiritShot`
/// handler it delegates to — charge a summon from its **owner's** Beast shots.
///
/// The owner's auto-shot list is the switch: a Beast shot only fires if the
/// player toggled it on. Each charge costs the *pet's current level row*
/// count, so a high-level pet is markedly more expensive to keep shotted —
/// which is the mechanic, not an incidental detail.
///
/// Both ways of coming up short retire the toggle, as Java does: the item gone
/// from the bag entirely is `Summon.rechargeShots`'s own `removeAutoSoulShot`,
/// and a stack too thin for one swing is the handler's `destroyItemWithoutTrace`
/// failing into `disableAutoShot`. Without them a player who runs dry keeps a
/// lit toggle over a summon that silently stopped using shots.
///
/// Java prunes *every* absent entry on this pass, ours only this kind's — the
/// rest are the player's own shots, which [`crate::game_loop::items::recharge_shots`]
/// already prunes on the owner's own swing and cast.
///
/// Returns true when the summon ends up charged.
fn recharge_summon_shot(world: &mut World, summon_oid: i32, kind: SummonShot) -> bool {
    use crate::model::components::ChargedShots;

    let charged = |world: &World| {
        world
            .objects
            .get_component::<ChargedShots>(&summon_oid)
            .is_some_and(|c| match kind {
                SummonShot::Soulshot => c.soulshot,
                SummonShot::Spiritshot => c.spiritshot,
            })
    };
    if charged(world) {
        return true;
    }
    let Some(owner) = world
        .objects
        .get_component::<ServitorOf>(&summon_oid)
        .map(|s| s.owner_object_id)
    else {
        return false;
    };
    // How many the swing costs: from the pet's level row. A servitor has no
    // pet row, so it uses one — Java reads `getSoulShotsPerHit()`, which for a
    // plain servitor is its template's.
    let per_hit = world
        .objects
        .get_component::<crate::model::components::PetOf>(&summon_oid)
        .and_then(|p| {
            npc_template_id(world, summon_oid)
                .and_then(|id| world.data.pet_data.get(id))
                .and_then(|t| t.levels.get(&p.level))
                .map(|l| kind.per_hit(l))
        })
        .unwrap_or(1)
        .max(1) as i64;

    // Java iterates the owner's auto-shot list and picks the entries whose
    // `default_action` marks them as *summon* shots.
    for item_id in crate::game_loop::items::auto_shots(world, owner) {
        if world.data.item_data.get(item_id).map(|t| t.default_action) != Some(kind.action()) {
            continue;
        }
        let have = world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&owner)
            .map(|inv| inv.count_of(item_id))
            .unwrap_or(0);
        if have < per_hit {
            disable_auto_shot(world, owner, item_id, kind.shot_type());
            continue;
        }
        let changes = world
            .objects
            .get_component_mut::<crate::model::inventory::Inventory>(&owner)
            .map(|inv| inv.remove_item(item_id, per_hit))
            .unwrap_or_default();
        inventory::send_inventory_update(world, owner, changes);
        if world
            .objects
            .get_component::<ChargedShots>(&summon_oid)
            .is_none()
        {
            world
                .objects
                .add_components(&summon_oid, ChargedShots::default());
        }
        if let Some(c) = world.objects.get_component_mut::<ChargedShots>(&summon_oid) {
            match kind {
                SummonShot::Soulshot => c.soulshot = true,
                SummonShot::Spiritshot => c.spiritshot = true,
            }
        }
        return true;
    }
    false
}

/// Charge a summon's Beast Soulshot before it swings. `physical` false is the
/// no-op arm of Java's `rechargeShots(physical, …)`.
pub(crate) fn recharge_shots(world: &mut World, summon_oid: i32, physical: bool) -> bool {
    if !physical {
        return world
            .objects
            .get_component::<crate::model::components::ChargedShots>(&summon_oid)
            .is_some_and(|c| c.soulshot);
    }
    recharge_summon_shot(world, summon_oid, SummonShot::Soulshot)
}

/// Spend a summon's charged soulshot (Java `unchargeShot(SOULSHOTS)`), which
/// happens on a landed hit only — a miss keeps the charge.
pub(crate) fn uncharge_soulshot(world: &mut World, summon_oid: i32) -> bool {
    use crate::model::components::ChargedShots;
    match world.objects.get_component_mut::<ChargedShots>(&summon_oid) {
        Some(c) if c.soulshot => {
            c.soulshot = false;
            true
        }
        _ => false,
    }
}

/// Charge a summon's Beast Spiritshot from its owner — the magic counterpart of
/// [`recharge_shots`], costing the pet level's `spiritshot_count`.
pub(crate) fn recharge_spiritshots(world: &mut World, summon_oid: i32) -> bool {
    recharge_summon_shot(world, summon_oid, SummonShot::Spiritshot)
}

/// Spend a summon's charged spiritshot. Unlike the soulshot — spent by a
/// landed swing — a magic shot is consumed by the **cast**, so this is called
/// from the effect path.
pub(crate) fn uncharge_spiritshot(world: &mut World, summon_oid: i32) -> bool {
    use crate::model::components::ChargedShots;
    match world.objects.get_component_mut::<ChargedShots>(&summon_oid) {
        Some(c) if c.spiritshot => {
            c.spiritshot = false;
            true
        }
        _ => false,
    }
}
