//! Pets — the collar-summoned half: collar/owner links, DB row sync,
//! summoning from a collar, and pet equipment.

use super::*;

/// The object id of the collar that summoned the player's currently-out pet.
///
/// Java reads this as `player.getPet().getControlObjectId()` at each use site;
/// here it is one lookup so the sell/trade lists cannot drift apart. `None`
/// when no pet is out — which is also the case for a *servitor* owner, since a
/// skill-summoned servitor has no collar.
pub(crate) fn active_pet_collar(world: &World, owner_oid: i32) -> Option<i32> {
    let pet = pet_of(world, owner_oid)?;
    world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet)
        .map(|p| p.collar_object_id)
}

/// Java `Player.getPet()` — a player has at most one.
pub(crate) fn pet_of(world: &World, owner_oid: i32) -> Option<i32> {
    let oid = world
        .objects
        .get_component::<crate::model::components::SummonRef>(&owner_oid)?
        .pet?;
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&oid)
        .map(|_| oid)
}
/// Fold a live pet's state back into its owner's `PlayerPets` map (Java
/// `Pet.storeMe`, minus the DB write — the row rides out with the character's
/// next flush).
///
/// Called before every character save and on unsummon. A no-op when the player
/// has no pet out, which is the common case, so it is cheap to call
/// unconditionally rather than tracking a dirty flag.
pub(crate) fn sync_pet_row(world: &mut World, owner_oid: i32) {
    let Some(pet_oid) = pet_of(world, owner_oid) else {
        return;
    };
    let Some(pet) = world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)
        .copied()
    else {
        return;
    };
    let (cur_hp, cur_mp) = world
        .objects
        .get_component::<Vitals>(&pet_oid)
        .map(|v| (v.cur_hp, v.cur_mp))
        .unwrap_or((0.0, 0.0));
    // Java stores `getName()`, which for an unnamed pet is the template name.
    let name = npc_name_or_empty(world, pet_oid);
    let row = crate::db::PetRow {
        collar_object_id: pet.collar_object_id,
        name,
        level: pet.level,
        cur_hp,
        cur_mp,
        exp: pet.exp,
        sp: pet.sp,
        fed: pet.fed,
        // The pet is alive in the world at this moment, so if the owner is on
        // their way out it should come back next login. `on_owner_leave_world`
        // calls this *before* the unsummon precisely so this reads true.
        restore: world.cfg.character.restore_pet_on_reconnect,
    };
    if let Some(pets) = world
        .objects
        .get_component_mut::<crate::model::components::PlayerPets>(&owner_oid)
    {
        pets.0.insert(row.collar_object_id, row);
    }
}

/// `SummonPet.instant` — bring out the pet bound to the collar the player just
/// used.
///
/// The collar arrives through `Player.pending_pet_collar` (Java's
/// `PetItemHolder`) and is **taken**, so a stale value can never summon a
/// second pet. Every stat comes from `PetData`, keyed by the collar's *item*
/// id; the collar's *object* id becomes the pet's identity.
///
/// A pet reuses [`ServitorOf`] for the owner link and follow state, so it
/// inherits follow/attack/leash from the servitor AI — "owned summon" is the
/// same relationship whether it came from a skill or a collar. Its own state
/// (collar, food bar) lives in `PetOf`.
pub(crate) fn summon_pet(world: &mut World, owner_oid: i32) -> Option<i32> {
    use crate::model::components::PetOf;
    use crate::network::server_packets::sm_ids;

    world
        .objects
        .get_component::<crate::model::Player>(&owner_oid)?;
    // `if (player.hasPet() || player.isMounted())` → "You already have a pet."
    if pet_of(world, owner_oid).is_some()
        || world
            .objects
            .get_component::<crate::model::Player>(&owner_oid)
            .is_some_and(crate::model::Player::is_mounted)
    {
        send_sm_bare_to_player(world, owner_oid, sm_ids::YOU_ALREADY_HAVE_A_PET);
        return None;
    }
    // Java logs and bails when the holder is missing — the effect was reached
    // without going through the item handler.
    let collar_object_id = world
        .objects
        .get_component_mut::<crate::model::Player>(&owner_oid)
        .and_then(|p| p.pending_pet_collar.take())?;

    // The collar must still be in the owner's inventory (Java re-checks).
    let collar_item_id = item_id_of(world, owner_oid, collar_object_id)?;

    let npc_id = world.data.pet_data.by_item_id(collar_item_id)?.npc_id;

    // Java `Pet.restore`: the saved row keyed by this collar, or a fresh pet.
    // Here the row is already in memory (`PlayerPets`, loaded at login).
    let saved = world
        .objects
        .get_component::<crate::model::components::PlayerPets>(&owner_oid)
        .and_then(|p| p.0.get(&collar_object_id).cloned());

    let owner_level = world
        .objects
        .get_component::<crate::model::Player>(&owner_oid)
        .map(|p| p.level)
        .unwrap_or(1);
    let (template_level, display_id) = world
        .data
        .npc_data
        .get(npc_id)
        .map(|t| (t.level, t.display_id))
        .unwrap_or((1, npc_id));

    let level = match &saved {
        Some(row) => row.level,
        // `new Pet(template, owner, control)`: the Sin Eater (display id 12564)
        // is summoned at its *owner's* level; every other species starts at its
        // template level.
        None if display_id == SIN_EATER_DISPLAY_ID => owner_level,
        None => template_level,
    };
    let (level, max_fed, exp_floor) = {
        let t = world.data.pet_data.by_item_id(collar_item_id)?;
        // `Math.max(level, getPetMinLevel(id))` — see `PetTemplate::min_level`.
        let level = level.max(t.min_level());
        (level, t.max_meal(level), t.exp_for_level(level))
    };

    // Java spawns the pet beside its owner, not on top of them.
    let pos = maybe_position(world, owner_oid)?;
    let pet_oid = crate::model::npc::spawn_npc_at(
        world,
        npc_id,
        pos.x + 50,
        pos.y + 100,
        pos.z,
        pos.heading,
    )?;

    world.objects.add_components(
        &pet_oid,
        ServitorOf {
            owner_object_id: owner_oid,
            // A pet is not tied to a skill and never expires or pays upkeep —
            // it is fed instead, which `PetOf` tracks.
            reference_skill: 0,
            expires_at_tick: u64::MAX,
            life_time_secs: 0,
            following: true,
            consume_item_id: 0,
            consume_item_count: 0,
            next_consume_tick: u64::MAX,
        },
    );
    world.objects.add_components(
        &pet_oid,
        PetOf {
            collar_object_id,
            fed: saved
                .as_ref()
                .map(|r| r.fed.min(max_fed))
                .unwrap_or(max_fed),
            max_fed,
            level,
            // "DS: update experience based by level. Avoiding pet delevels due
            // to exp per level values changed." — a stored exp below what this
            // level now costs is raised to the level's floor, so a retuned
            // datapack curve can't demote a pet the player already levelled.
            exp: saved
                .as_ref()
                .map(|r| r.exp.max(exp_floor))
                .unwrap_or(exp_floor),
            sp: saved.as_ref().map(|r| r.sp).unwrap_or(0),
            exp_before_death: 0,
        },
    );

    // Stats first: they set max HP/MP, which the vitals below are measured
    // against. A pet's stats come from its per-level pet row, not the NPC
    // template, so this must run before either branch.
    recalculate_pet_stats(world, pet_oid);

    // A fresh pet spawns full; a restored one keeps the vitals it was stored
    // with (Java `setCurrentHp/Mp` from the row).
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&pet_oid) {
        match &saved {
            Some(row) => {
                v.cur_hp = row.cur_hp.min(v.max_hp as f64);
                v.cur_mp = row.cur_mp.min(v.max_mp as f64);
            }
            None => {
                v.cur_hp = v.max_hp as f64;
                v.cur_mp = v.max_mp as f64;
            }
        }
    }
    // Java's restore marks a pet stored with `curHp < 1` as dead
    // (`setDead(true)` + `stopHpMpRegeneration()`) and summons the corpse.
    // Reachable now that pets can die (slice 14).
    if saved.as_ref().is_some_and(|r| r.cur_hp < 1.0)
        && let Some(v) = world.objects.get_component_mut::<Vitals>(&pet_oid)
    {
        v.dead = true;
        v.cur_hp = 0.0;
    }
    set_summon_link(world, owner_oid, None, Some(pet_oid), true);
    // Java `Pet.spawnMe` → `startFeed()`: the food clock runs from summon.
    start_feed(world, pet_oid);
    send_pet_info(world, owner_oid, pet_oid, PetInfoKind::Summoned);
    broadcast_summon_info(world, pet_oid, true);
    send_pet_item_list(world, owner_oid);
    // `ai/others/Servitors/SinEater.onSummonSpawn` — the one pet with a voice.
    crate::scripts::sin_eater::on_spawn(world, pet_oid);
    Some(pet_oid)
}

/// Equip or unequip an item in the pet's own paperdoll.
///
/// `PetInventory` wraps the ordinary `Inventory`, which already owns a
/// paperdoll and all the slot-displacement rules — so a pet's armour reuses the
/// player's equip logic wholesale rather than growing a second copy. Java does
/// the same: `PetInventory extends Inventory`.
///
/// Toggling matches Java's `useEquippableItem`: clicking a worn item takes it
/// off.
pub(crate) fn equip_pet_item(world: &mut World, owner_oid: i32, pet_oid: i32, object_id: i32) {
    let World { data, objects, .. } = world;
    let Some(pi) = objects.get_component_mut::<crate::model::inventory::PetInventory>(&owner_oid)
    else {
        return;
    };
    let worn = pi.0.paperdoll_slot_of(object_id).is_some();
    if worn {
        pi.0.unequip_item(object_id);
    } else {
        pi.0.equip_item(&data.item_data, object_id);
    }
    // Gear changes the pet's defences, so its stats and the client's view of
    // them both have to be rebuilt.
    recalculate_pet_stats(world, pet_oid);
    send_pet_item_list(world, owner_oid);
    send_pet_info(world, owner_oid, pet_oid, PetInfoKind::Default);
    broadcast_summon_info(world, pet_oid, false);
}
