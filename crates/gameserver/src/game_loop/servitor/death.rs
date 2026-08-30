//! Pet death: the death penalty, exp restore, and corpse decay.

use super::PetInfoKind;
use super::notify_owner;
use super::npc_template_id;
use super::send_pet_info;
use super::send_pet_item_list;
use super::set_summon_link;
use super::sync_pet_row;
use crate::game_loop::helpers::send_inventory_update;
use crate::model::components::ServitorOf;
use crate::world::World;
/// Java `Pet.doDie` — the pet-specific half, called from the NPC death path
/// once a dying NPC turns out to be a pet.
///
/// Returns the owner so the caller can finish its own bookkeeping.
pub(crate) fn pet_do_die(world: &mut World, pet_oid: i32) -> Option<i32> {
    use crate::network::server_packets::sm_ids;
    let owner = world
        .objects
        .get_component::<ServitorOf>(&pet_oid)?
        .owner_object_id;
    world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)?;

    // `if (owner != null && !owner.isInDuel() && (!isInsideZone(PVP) || isInsideZone(SIEGE)))`
    // — no exp is lost to a duel or an arena death.
    if !crate::game_loop::combat::duel::is_in_duel(world, owner) {
        // `SinEater`'s `ON_CREATURE_DEATH` bark, before the penalty maths.
        crate::scripts::sin_eater::on_death(world, pet_oid);
        pet_death_penalty(world, pet_oid);
    }

    // `stopFeed()` — the food clock stops with the pet. The scheduled tick
    // checks `dead` and ends its own chain, so there is nothing to cancel.
    notify_owner(world, owner, sm_ids::THE_PET_HAS_BEEN_KILLED, &[]);
    // The pet's state is captured now: the corpse can decay or be resurrected,
    // but either way the exp penalty is already what should persist.
    sync_pet_row(world, owner);
    send_pet_info(world, owner, pet_oid, PetInfoKind::Default);
    Some(owner)
}

/// Java `Pet.deathPenalty`, whose own comment admits the penalty is a guess
/// ("Need Correct Penalty") — ported as written.
///
/// `percentLost = -0.07 × level + 6.5`, applied to the size of the pet's
/// *current* level band — so the loss is a share of one level's worth of exp,
/// and it shrinks as the pet levels.
fn pet_death_penalty(world: &mut World, pet_oid: i32) {
    let Some(pet) = world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)
        .copied()
    else {
        return;
    };
    let Some(npc_id) = npc_template_id(world, pet_oid) else {
        return;
    };
    let (this_level, next_level) = {
        let Some(t) = world.data.pet_data.get(npc_id) else {
            return;
        };
        (t.exp_for_level(pet.level), t.exp_for_level(pet.level + 1))
    };
    let band = (next_level - this_level).max(0) as f64;
    let percent_lost = (-0.07 * pet.level as f64) + 6.5;
    let lost = ((band * percent_lost) / 100.0).round() as i64;

    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::components::PetOf>(&pet_oid)
    {
        // Captured *before* the penalty — `restoreExp` gives back a share of
        // the gap between this and the post-penalty total.
        p.exp_before_death = p.exp;
        // Java's `addExp(-lostExp)` cannot take a pet below its level floor.
        p.exp = (p.exp - lost).max(this_level);
    }
}

/// Java `Pet.restoreExp(restorePercent)` — hand back a share of what the death
/// penalty took. Called from the resurrection path with the skill's power.
pub(crate) fn pet_restore_exp(world: &mut World, pet_oid: i32, restore_percent: f64) {
    let Some(pet) = world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)
        .copied()
    else {
        return;
    };
    if pet.exp_before_death <= 0 {
        return;
    }
    let regained =
        (((pet.exp_before_death - pet.exp) as f64 * restore_percent) / 100.0).round() as i64;
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::components::PetOf>(&pet_oid)
    {
        p.exp += regained.max(0);
        // One resurrection consumes the record — a second revive restores
        // nothing, as in Java.
        p.exp_before_death = 0;
    }
}

/// `Summon.onDecay` → `unSummon(owner)` + `Pet.deleteMe(owner)` for a pet whose
/// corpse has decayed.
///
/// **This destroys the pet permanently.** Java's `deleteMe` is:
///
/// ```java
/// _inventory.transferItemsToOwner();
/// super.deleteMe(owner);
/// destroyControlItem(owner, false); // "this should also delete the pet from the db"
/// ```
///
/// So letting a dead pet rot costs the player the collar *and* everything the
/// pet was carrying stays only because the inventory is handed back first. The
/// corpse lasts `DefaultCorpseTime` — **7 seconds** on this dist, since no pet
/// NPC template overrides `corpseTime` and `DecayTaskManager` has no pet
/// branch. (The "24 hours" in the death message is flavour text that does not
/// match the mechanic; checked against the datapack rather than trusted.)
pub(crate) fn pet_decay(world: &mut World, pet_oid: i32) {
    let Some(owner) = world
        .objects
        .get_component::<ServitorOf>(&pet_oid)
        .map(|s| s.owner_object_id)
    else {
        return;
    };
    let Some(pet) = world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)
        .copied()
    else {
        return;
    };

    // `_inventory.transferItemsToOwner()` — the pet's bag is handed back
    // before the collar goes, so its contents are not lost with it.
    let carried: Vec<(i32, i64)> = world
        .objects
        .get_component::<crate::model::inventory::PetInventory>(&owner)
        .map(|pi| pi.0.items().iter().map(|i| (i.item_id, i.count)).collect())
        .unwrap_or_default();
    if let Some(pi) = world
        .objects
        .get_component_mut::<crate::model::inventory::PetInventory>(&owner)
    {
        pi.0 = Default::default();
    }
    for (item_id, count) in carried {
        let Some(oid) = world.alloc_object_id() else {
            break;
        };
        let World { data, objects, .. } = world;
        if let Some(inv) = objects.get_component_mut::<crate::model::inventory::Inventory>(&owner) {
            inv.add_item(&data.item_data, oid, item_id, count);
        }
    }

    // `destroyControlItem` — the collar is consumed, and with it the pet's
    // identity: the saved row is keyed by that object id.
    let collar = pet.collar_object_id;
    let removed = crate::game_loop::helpers::remove_inventory_item_change(world, owner, collar, 1);
    if let Some(change) = removed {
        send_inventory_update(world, owner, vec![change]);
    }
    world
        .objects
        .get_component_mut::<crate::model::components::PlayerPets>(&owner)
        .map(|p| p.0.remove(&collar));
    let _ = world.db.send(crate::db::DbCommand::DeletePetRow {
        collar_object_id: collar,
    });

    // The owner has no pet any more.
    set_summon_link(world, owner, None, None, true);
    send_pet_item_list(world, owner);
}
