//! Pets — the collar-summoned half: collar/owner links, DB row sync,
//! summoning from a collar, and pet equipment.

use super::PetInfoKind;
use super::SIN_EATER_DISPLAY_ID;
use super::broadcast_summon_info;
use super::is_uncontrollable;
use super::recalculate_pet_stats;
use super::send_pet_info;
use super::send_pet_item_list;
use super::set_summon_link;
use super::start_feed;
use crate::game_loop::helpers;
use crate::game_loop::helpers::maybe_position;

use crate::model::components::ServitorOf;
use crate::model::components::Vitals;
use crate::network::server_packets;
use crate::world::World;
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
    let name = helpers::npc_name_or_empty(world, pet_oid);
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
        helpers::send_sm_bare_to_player(world, owner_oid, sm_ids::YOU_ALREADY_HAVE_A_PET);
        return None;
    }
    // Java logs and bails when the holder is missing — the effect was reached
    // without going through the item handler.
    let collar_object_id = world
        .objects
        .get_component_mut::<crate::model::Player>(&owner_oid)
        .and_then(|p| p.pending_pet_collar.take())?;

    // The collar must still be in the owner's inventory (Java re-checks).
    let collar_item_id = helpers::item_id_of(world, owner_oid, collar_object_id)?;

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
    let pet_oid = crate::game_loop::npc::spawn_npc_at(
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
            defending: false,
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
    // Java `PetInventory.restore`'s tail: "check for equipped items from other
    // pets" — every worn item is re-judged against the *summoned* pet and
    // unequipped if it fails, which is what stops a strider's saddle staying on
    // a wolf. It runs here rather than at character load because the port keeps
    // the pet inventory on the owner: there is no pet to judge against until
    // this point. The link above must already exist for the conditions to see
    // the pet as a summon at all.
    unequip_items_this_pet_cannot_wear(world, owner_oid, pet_oid);
    // Java `Pet.spawnMe` → `startFeed()`: the food clock runs from summon.
    start_feed(world, pet_oid);
    send_pet_info(world, owner_oid, pet_oid, PetInfoKind::Summoned);
    broadcast_summon_info(world, pet_oid, true);
    send_pet_item_list(world, owner_oid);
    // `ai/others/Servitors/SinEater.onSummonSpawn` — the one pet with a voice.
    crate::scripts::sin_eater::on_spawn(world, pet_oid);
    // `ai/areas/BeastFarm/BabyPets.onSummonSpawn` — the three baby pets heal
    // their owner on a 5 s timer. A no-op for every other species.
    crate::scripts::baby_pets::on_summon_spawn(world, pet_oid);
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

/// `Npc.INTERACTION_DISTANCE`-ish reach for a pet lifting an item — Java's
/// `thinkPickUp` uses `maybeMoveToPawn(target, 36)`, the same 36 units the
/// player path uses.
const PET_PICKUP_RANGE: f64 = 36.0;

/// `RequestPetGetItem` (0x98) — order the pet to fetch a ground item.
///
/// Every guard here is Java's, in Java's order, and each answers with
/// `ActionFailed` rather than a message except the hunger one. The
/// fort-siege combat-flag check has no counterpart (forts are off-chronicle);
/// the castle siege-guard *ticket* check does — a mercenary ticket lying in a
/// castle's grounds may not be pocketed by a pet.
pub(crate) fn handle_request_pet_get_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(owner_oid) = world.player_oid(client_id) else {
        return;
    };
    let mut r = commons::network::PacketReader::new(body);
    let Some(item_oid) = r.read_i32() else {
        return;
    };
    // `!player.hasPet()` — a servitor is not a pet and cannot fetch.
    let Some(pet_oid) = pet_of(world, owner_oid) else {
        crate::game_loop::helpers::send_action_failed(world, client_id);
        return;
    };
    let Some(ground) = world
        .objects
        .get_component::<crate::model::components::GroundItem>(&item_oid)
        .cloned()
    else {
        crate::game_loop::helpers::send_action_failed(world, client_id);
        return;
    };
    // `CastleManager.getCastle(item)` + `getSiegeGuardByItem(castle, item)` —
    // a mercenary posting ticket lying on castle ground stays where it was
    // dropped, pet or no pet.
    let on_castle_ticket = maybe_position(world, item_oid)
        .and_then(|p| world.data.zone_data.siege_castle_at(p.x, p.y, p.z))
        .is_some_and(|castle_id| {
            world
                .data
                .castle_siege_guards
                .by_item(castle_id, ground.item_id)
                .is_some()
        });
    if on_castle_ticket {
        crate::game_loop::helpers::send_action_failed(world, client_id);
        return;
    }
    if helpers::is_dead(world, pet_oid) {
        crate::game_loop::helpers::send_action_failed(world, client_id);
        return;
    }
    // `pet.isUncontrollable()` — a starved pet takes no orders, and this one
    // *does* get told why.
    if is_uncontrollable(world, pet_oid) {
        crate::game_loop::helpers::send_sm_bare_to_client(
            world,
            client_id,
            server_packets::sm_ids::WHEN_YOUR_PETS_HUNGER_GAUGE_IS_AT_0_YOU_CANNOT_USE_YOUR_PET,
        );
        return;
    }

    // `setIntention(AI_INTENTION_PICK_UP, item)`: stop trailing the owner and
    // walk. The follow flag comes back when the errand ends, which is what
    // Java's `getFollowStatus()` save/restore around `doPickupItem` does.
    if let Some(l) = world.objects.get_component_mut::<ServitorOf>(&pet_oid) {
        l.following = false;
    }
    world.objects.add_components(
        &pet_oid,
        crate::model::components::SummonPickup {
            item_object_id: item_oid,
        },
    );
    // Think once now so a pet already standing on the item lifts it without
    // waiting a tick — the same reason the player path thinks synchronously.
    pet_pickup_think(world, pet_oid);
}

/// The `SummonPickup` half of the summon think: walk, then lift.
///
/// Returns `true` while the errand is still running, which tells the caller to
/// skip the follow tick — an errand outranks trailing the owner.
pub(crate) fn pet_pickup_think(world: &mut World, pet_oid: i32) -> bool {
    let Some(order) = world
        .objects
        .get_component::<crate::model::components::SummonPickup>(&pet_oid)
        .copied()
    else {
        return false;
    };
    let item_oid = order.item_object_id;
    // `checkTargetLost` — someone else lifted it, or it decayed.
    let gone = !world
        .objects
        .has_component::<crate::model::components::GroundItem>(&item_oid);
    if gone || helpers::is_dead(world, pet_oid) {
        end_pickup(world, pet_oid);
        return false;
    }
    let (Some(item_pos), Some(pet_pos)) = (
        maybe_position(world, item_oid),
        maybe_position(world, pet_oid),
    ) else {
        end_pickup(world, pet_oid);
        return false;
    };
    let dx = f64::from(item_pos.x - pet_pos.x);
    let dy = f64::from(item_pos.y - pet_pos.y);
    if (dx * dx + dy * dy).sqrt() > PET_PICKUP_RANGE {
        crate::game_loop::ai::move_npc_to(world, pet_oid, item_pos.x, item_pos.y, item_pos.z);
        return true;
    }
    end_pickup(world, pet_oid);
    pet_pickup_item(world, pet_oid, item_oid);
    false
}

/// Drop the errand and go back to trailing the owner (Java restores
/// `getFollowStatus()` after `doPickupItem`).
fn end_pickup(world: &mut World, pet_oid: i32) {
    world
        .objects
        .remove_component::<crate::model::components::SummonPickup>(&pet_oid);
    if let Some(l) = world.objects.get_component_mut::<ServitorOf>(&pet_oid) {
        l.following = true;
    }
}

/// `Pet.doPickupItem` — the pet's own version of the lift.
///
/// The loot rules are the **owner's**: drop protection is checked against the
/// player, because Java passes `getOwner()` into the looter-party test and the
/// pet has no party of its own. What differs from the player path is only
/// where the item lands (`PetInventory`) and the one message that says so.
fn pet_pickup_item(world: &mut World, pet_oid: i32, item_oid: i32) {
    use crate::model::components::GroundItem;
    let Some(owner_oid) = world
        .objects
        .get_component::<ServitorOf>(&pet_oid)
        .map(|l| l.owner_object_id)
    else {
        return;
    };
    let Some(client_id) = crate::game_loop::helpers::client_for_player(world, owner_oid) else {
        return;
    };
    let Some(g) = world
        .objects
        .get_component::<GroundItem>(&item_oid)
        .cloned()
    else {
        return;
    };
    // Loot protection, measured against the owner exactly as Java does.
    if g.owner_id != 0
        && world.tick < g.owner_until_tick
        && g.owner_id != owner_oid
        && !crate::game_loop::command_channel::is_in_looter_party(world, owner_oid, g.owner_id)
    {
        crate::game_loop::helpers::send_action_failed(world, client_id);
        crate::game_loop::helpers::send_sm_to_client(
            world,
            client_id,
            server_packets::sm_ids::YOU_HAVE_FAILED_TO_PICK_UP_S1,
            &[server_packets::SmParam::ItemName(g.item_id)],
        );
        return;
    }
    // `_inventory.validateCapacity(target)` — the pet's own bag, not the
    // owner's, and its own refusal message.
    if !pet_inventory_has_room(world, owner_oid, g.item_id) {
        crate::game_loop::helpers::send_action_failed(world, client_id);
        crate::game_loop::helpers::send_sm_bare_to_client(
            world,
            client_id,
            server_packets::sm_ids::YOUR_PET_CANNOT_CARRY_ANY_MORE_ITEMS,
        );
        return;
    }
    let Some(region) = helpers::region_cell_of(world, item_oid) else {
        return;
    };
    crate::game_loop::ground_items::despawn_ground_item(world, item_oid, region);
    let World { data, objects, .. } = world;
    let Some(oid) = objects
        .get_component::<crate::model::inventory::PetInventory>(&owner_oid)
        .map(|_| item_oid)
    else {
        return;
    };
    if let Some(pi) = objects.get_component_mut::<crate::model::inventory::PetInventory>(&owner_oid)
    {
        pi.0.add_item(&data.item_data, oid, g.item_id, g.count);
    }
    send_pet_item_list(world, owner_oid);
}

/// `PetInventory.validateCapacity` — a stackable the pet already holds needs
/// no new slot; anything else does.
pub(crate) fn pet_inventory_has_room(world: &World, owner_oid: i32, item_id: i32) -> bool {
    let Some(pi) = world
        .objects
        .get_component::<crate::model::inventory::PetInventory>(&owner_oid)
    else {
        return false;
    };
    let stackable = world
        .data
        .item_data
        .get(item_id)
        .is_some_and(|t| t.is_stackable);
    if stackable && pi.0.count_of(item_id) > 0 {
        return true;
    }
    pi.0.items().len() < world.cfg.npc.inventory_maximum_pet
}

/// `PetInventory.restore`'s validation pass: strip the equipped items whose
/// `<cond>` the summoned pet does not satisfy.
///
/// Java's `unEquipItemInSlot` here is silent — no message, no packet — because
/// the `PetItemList` that `summon_pet` sends next carries the corrected
/// paperdoll anyway.
fn unequip_items_this_pet_cannot_wear(world: &mut World, owner_oid: i32, pet_oid: i32) {
    let worn: Vec<(i32, i32)> = world
        .objects
        .get_component::<crate::model::inventory::PetInventory>(&owner_oid)
        .map(|pi| {
            pi.0.equipped_items()
                .iter()
                .map(|i| (i.object_id, i.item_id))
                .collect()
        })
        .unwrap_or_default();
    let failing: Vec<i32> = worn
        .into_iter()
        .filter(|&(_, item_id)| {
            world.data.item_data.get(item_id).is_some_and(|t| {
                !crate::game_loop::items::check_condition(world, pet_oid, t, false)
            })
        })
        .map(|(object_id, _)| object_id)
        .collect();
    if failing.is_empty() {
        return;
    }
    if let Some(pi) = world
        .objects
        .get_component_mut::<crate::model::inventory::PetInventory>(&owner_oid)
    {
        for object_id in failing {
            pi.0.unequip_item(object_id);
        }
    }
    recalculate_pet_stats(world, pet_oid);
}
