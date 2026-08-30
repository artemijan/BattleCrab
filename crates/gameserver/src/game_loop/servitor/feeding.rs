//! Pet feeding: hunger state, the feed tick, and the give/get/use
//! pet-inventory packets.

use super::PetInfoKind;
use super::equip_pet_item;
use super::notify_owner;
use super::pet_of;
use super::send_pet_info;
use super::sync_pet_row;
use super::unsummon_servitor;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::npc_id_of;
use crate::game_loop::helpers::send_inventory_update;
use crate::game_loop::helpers::send_to_player;
use crate::game_loop::items::item_skills;
use crate::game_loop::time::TICKS_PER_SECOND;
use crate::model::components::ServitorOf;
use crate::network::server_packets;
use crate::world::World;
/// Java `Pet.FeedTask`'s fixed period: `scheduleAtFixedRate(..., 10000, 10000)`.
const FEED_TICK_SECS: u64 = 10;

/// Arm the feed chain for a freshly summoned pet (Java `startFeed`).
pub(crate) fn start_feed(world: &mut World, pet_oid: i32) {
    world.scheduler.schedule(
        world.tick + FEED_TICK_SECS * TICKS_PER_SECOND,
        crate::scheduler::ScheduledTask::PetFeedTick { pet_oid },
    );
}

/// Java `Pet.isHungry()` — below `hungryLimit`% of the level's `maxMeal`.
/// A hungry pet is what triggers auto-eating; it is *not* the same as
/// [`is_uncontrollable`].
pub(crate) fn is_hungry(world: &World, pet_oid: i32) -> bool {
    let Some(pet) = world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)
    else {
        return false;
    };
    let limit = npc_template_id(world, pet_oid)
        .and_then(|id| world.data.pet_data.get(id))
        .map(|t| t.hungry_limit)
        .unwrap_or(0);
    (pet.fed as f64) < (limit as f64 / 100.0) * pet.max_fed as f64
}

/// Java `Pet.isUncontrollable()` — a starving (empty-bar) pet stops obeying.
pub(crate) fn is_uncontrollable(world: &World, pet_oid: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)
        .is_some_and(|p| p.fed <= 0)
}

pub(super) fn npc_template_id(world: &World, oid: i32) -> Option<i32> {
    npc_id_of(world, oid)
}

/// `effecthandlers/Feed.instant` for a pet: `setCurrentFed(fed + normal * rate)`.
///
/// `setCurrentFed` clamps at `getMaxFed()`, so over-feeding is capped rather
/// than banked — that clamp is why a "feeding restores N" test must measure
/// from a bar with room in it.
pub(crate) fn apply_feed(world: &mut World, pet_oid: i32, normal: i32) {
    let rate = world.cfg.rates.pet_food_rate;
    if let Some(pet) = world
        .objects
        .get_component_mut::<crate::model::components::PetOf>(&pet_oid)
    {
        pet.fed = (pet.fed + normal * rate).min(pet.max_fed);
    }
}

/// Java `Pet.FeedTask.run()` — burn one interval's food, then let the pet eat
/// from its own inventory if it's hungry.
pub(crate) fn handle_feed_tick(world: &mut World, pet_oid: i32) {
    use crate::network::server_packets::{SmParam, sm_ids};

    // "dead or gone → the chain ends", the same contract the life tick uses.
    let Some(pet) = world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)
        .copied()
    else {
        return;
    };
    if is_dead(world, pet_oid) {
        return;
    }
    let Some(owner) = world
        .objects
        .get_component::<ServitorOf>(&pet_oid)
        .map(|s| s.owner_object_id)
    else {
        return;
    };

    // `_curFed > getFeedConsume() ? fed - consume : 0` — note Java burns the
    // *battle* rate while attacking.
    let (normal_cost, battle_cost) = npc_template_id(world, pet_oid)
        .and_then(|id| world.data.pet_data.get(id))
        .and_then(|t| t.levels.get(&pet.level))
        .map(|l| (l.consume_meal_in_normal, l.consume_meal_in_battle))
        .unwrap_or((0, 0));
    // Java's `isAttackingNow()` — the battle rate applies mid-swing.
    let attacking = world
        .objects
        .get_component::<crate::model::components::AttackState>(&pet_oid)
        .is_some_and(|a| world.tick < a.attack_end_tick);
    let cost = if attacking { battle_cost } else { normal_cost };
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::components::PetOf>(&pet_oid)
    {
        p.fed = if p.fed > cost { p.fed - cost } else { 0 };
    }

    // Auto-eat: the food lives in the *pet's* inventory, not the owner's.
    let food_id = npc_template_id(world, pet_oid)
        .and_then(|id| world.data.pet_data.get(id))
        .map(|t| t.food_item_id)
        .unwrap_or(0);
    let has_food = food_id > 0
        && world
            .objects
            .get_component::<crate::model::inventory::PetInventory>(&owner)
            .is_some_and(|pi| pi.0.count_of(food_id) > 0);

    if is_hungry(world, pet_oid) && has_food {
        // `handler.useItem(pet, food, false)` → destroy one, apply the skill.
        if let Some(pi) = world
            .objects
            .get_component_mut::<crate::model::inventory::PetInventory>(&owner)
        {
            pi.0.remove_item(food_id, 1);
        }
        for (skill_id, skill_level) in item_skills(world, food_id) {
            apply_food_skill(world, pet_oid, skill_id, skill_level);
        }
        notify_owner(
            world,
            owner,
            sm_ids::YOUR_PET_WAS_HUNGRY_SO_IT_ATE_S1,
            &[SmParam::ItemName(food_id)],
        );
        send_pet_item_list(world, owner);
        // Still hungry after one helping — Java says so explicitly.
        if is_hungry(world, pet_oid) {
            notify_owner(
                world,
                owner,
                sm_ids::YOUR_PET_ATE_A_LITTLE_BUT_IS_STILL_HUNGRY,
                &[],
            );
        }
    } else if is_uncontrollable(world, pet_oid) {
        // Java `deleteMe` only when the species has *no* food ids at all;
        // otherwise it nags. A starving pet with a defined food item keeps
        // sulking until fed rather than vanishing.
        if food_id == 0 {
            notify_owner(world, owner, sm_ids::THE_PET_IS_NOW_LEAVING, &[]);
            sync_pet_row(world, owner);
            unsummon_servitor(world, owner);
            return;
        }
        notify_owner(world, owner, sm_ids::YOUR_PET_IS_STARVING, &[]);
    } else if is_hungry(world, pet_oid) {
        notify_owner(
            world,
            owner,
            sm_ids::THERE_IS_NOT_MUCH_TIME_REMAINING_UNTIL_THE_PET_LEAVES,
            &[],
        );
    }

    send_pet_info(world, owner, pet_oid, PetInfoKind::Default);
    world.scheduler.schedule(
        world.tick + FEED_TICK_SECS * TICKS_PER_SECOND,
        crate::scheduler::ScheduledTask::PetFeedTick { pet_oid },
    );
}

/// `PetItemList` for the owner — the pet's inventory is only ever shown to the
/// player who owns it.
pub(crate) fn send_pet_item_list(world: &World, owner_oid: i32) {
    let Some(pi) = world
        .objects
        .get_component::<crate::model::inventory::PetInventory>(&owner_oid)
    else {
        return;
    };
    send_to_player(
        world,
        owner_oid,
        server_packets::pet_item_list(&pi.0, &world.data),
    );
}

/// Run one food skill's effects on the pet. Only `Feed` is meaningful today;
/// going through the skill (rather than hard-coding a bar bump) is what lets a
/// food item that also heals work when those effects land.
fn apply_food_skill(world: &mut World, pet_oid: i32, skill_id: i32, skill_level: i32) {
    let Some(skill) = world.data.skill_data.get(skill_id, skill_level) else {
        return;
    };
    for effect in skill.effects.clone() {
        if let crate::model::skill::SkillEffect::Feed { normal, .. } = effect {
            apply_feed(world, pet_oid, normal);
        }
    }
}

/// `RequestGiveItemToPet` (0x95) — move an item from the owner's inventory into
/// the pet's. This is how food reaches the pet at all: Java's `PetFood` handler
/// refuses an unmounted *player*, so the owner cannot eat it on the pet's
/// behalf.
pub(crate) fn handle_give_item_to_pet(world: &mut World, client_id: u32, body: &[u8]) {
    let Some((object_id, amount)) = read_oid_and_count(body) else {
        return;
    };
    let Some(owner) = player_for_client(world, client_id) else {
        return;
    };
    if amount <= 0 || pet_of(world, owner).is_none() {
        return;
    }
    // Java refuses to hand over equipped gear or a quest item.
    let Some((item_id, held)) = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&owner)
        .and_then(|inv| inv.by_object_id(object_id).map(|i| (i.item_id, i.count)))
    else {
        return;
    };
    if world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&owner)
        .is_some_and(|inv| inv.paperdoll_slot_of(object_id).is_some())
    {
        return;
    }
    if world
        .data
        .item_data
        .get(item_id)
        .is_some_and(|t| t.is_quest_item)
    {
        return;
    }
    // The collar itself must not go into the pet it summons — Java blocks it,
    // and it would otherwise be unreachable when the pet is unsummoned.
    if world.data.pet_data.is_pet_collar(item_id) {
        return;
    }
    // Java: asking for more than the stack holds punishes.
    if amount > held {
        crate::game_loop::moderation::punishment::illegal_action(
            world,
            owner,
            &format!(
                "RequestGiveItemToPet: player {owner} tried to give item with oid {object_id} to pet but has invalid count {amount} item count: {held}"
            ),
        );
        return;
    }
    let count = amount.min(held);
    let changes = world
        .objects
        .get_component_mut::<crate::model::inventory::Inventory>(&owner)
        .map(|inv| inv.remove_item(item_id, count))
        .unwrap_or_default();
    let Some(next_oid) = world.alloc_object_id() else {
        return;
    };
    let World { data, objects, .. } = world;
    if let Some(pi) = objects.get_component_mut::<crate::model::inventory::PetInventory>(&owner) {
        pi.0.add_item(&data.item_data, next_oid, item_id, count);
    }
    send_inventory_update(world, owner, changes);
    send_pet_item_list(world, owner);
}

/// `RequestGetItemFromPet` (0x2C) — the reverse transfer.
pub(crate) fn handle_get_item_from_pet(world: &mut World, client_id: u32, body: &[u8]) {
    let Some((object_id, amount)) = read_oid_and_count(body) else {
        return;
    };
    let Some(owner) = player_for_client(world, client_id) else {
        return;
    };
    if amount <= 0 || pet_of(world, owner).is_none() {
        return;
    }
    let Some((item_id, held)) = world
        .objects
        .get_component::<crate::model::inventory::PetInventory>(&owner)
        .and_then(|pi| pi.0.by_object_id(object_id).map(|i| (i.item_id, i.count)))
    else {
        return;
    };
    // Java: asking for more than the stack holds punishes.
    if amount > held {
        crate::game_loop::moderation::punishment::illegal_action(
            world,
            owner,
            &format!(
                "RequestGetItemFromPet: player {owner} tried to get item with oid {object_id} from pet but has invalid count {amount} item count: {held}"
            ),
        );
        return;
    }
    let count = amount.min(held);
    if let Some(pi) = world
        .objects
        .get_component_mut::<crate::model::inventory::PetInventory>(&owner)
    {
        pi.0.remove_item(item_id, count);
    }
    let Some(next_oid) = world.alloc_object_id() else {
        return;
    };
    let World { data, objects, .. } = world;
    let changes = objects
        .get_component_mut::<crate::model::inventory::Inventory>(&owner)
        .map(|inv| {
            let oid = inv.add_item(&data.item_data, next_oid, item_id, count);
            inv.by_object_id(oid)
                .cloned()
                .map(crate::model::inventory::ItemChange::Modified)
                .into_iter()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    send_inventory_update(world, owner, changes);
    send_pet_item_list(world, owner);
}

/// `RequestPetUseItem` (0x94) — the owner clicks an item in the pet's window.
/// Only the `PetFood` handler is ported; anything else is ignored rather than
/// silently consumed.
pub(crate) fn handle_pet_use_item(world: &mut World, client_id: u32, body: &[u8]) {
    use crate::network::server_packets::sm_ids;
    if body.len() < 4 {
        return;
    }
    let object_id = i32::from_le_bytes([body[0], body[1], body[2], body[3]]);
    let Some(owner) = player_for_client(world, client_id) else {
        return;
    };
    let Some(pet_oid) = pet_of(world, owner) else {
        return;
    };
    let Some(item_id) = world
        .objects
        .get_component::<crate::model::inventory::PetInventory>(&owner)
        .and_then(|pi| pi.0.by_object_id(object_id).map(|i| i.item_id))
    else {
        return;
    };

    // `if (!item.getTemplate().isForNpc())` — Java's first gate on this
    // packet, and the reason the pet window cannot be used to feed a pet
    // anything at all: 508 items on this dist declare `for_npc`, and nothing
    // else may be handed over.
    if !world.data.item_data.get(item_id).is_some_and(|t| t.for_npc) {
        notify_owner(world, owner, sm_ids::THIS_PET_CANNOT_USE_THIS_ITEM, &[]);
        return;
    }

    // `if (!item.isEquipped() && !item.getTemplate().checkCondition(pet, pet,
    // true))` — evaluated against the **pet**, which is what makes
    // `categoryType="STRIDER"` on a saddle mean the wearer and not the owner.
    // A failing block answers with `THIS_PET_CANNOT_USE_THIS_ITEM` rather than
    // its own message (`checkCondition`'s `isSummon` arm).
    let worn = world
        .objects
        .get_component::<crate::model::inventory::PetInventory>(&owner)
        .is_some_and(|pi| pi.0.paperdoll_slot_of(object_id).is_some());
    let template = world.data.item_data.get(item_id).cloned();
    if let Some(template) = template.as_ref()
        && !worn
        && !crate::game_loop::items::check_condition(world, pet_oid, template, true)
    {
        return;
    }

    // Java `RequestPetUseItem`: an **equippable** item is worn rather than
    // consumed (`useEquippableItem`), which is how a battle pet gets its
    // armour. 96 pet-armour items ship on this dist.
    if template.as_ref().is_some_and(|t| t.is_equipable()) {
        // `useItem`'s own gate: pet gear is *defined* by carrying conditions,
        // so an equippable item with none is refused outright — the port had
        // been equipping any equippable item the pet window offered.
        if !template
            .as_ref()
            .is_some_and(crate::game_loop::items::is_condition_attached)
        {
            notify_owner(world, owner, sm_ids::THIS_PET_CANNOT_USE_THIS_ITEM, &[]);
            return;
        }
        equip_pet_item(world, owner, pet_oid, object_id);
        return;
    }

    // `if (playable.isPet() && !canEatFoodId(item.getId()))` → refuse.
    let eats = npc_template_id(world, pet_oid)
        .and_then(|id| world.data.pet_data.get(id))
        .is_some_and(|t| t.food_item_id == item_id);
    if !eats {
        notify_owner(world, owner, sm_ids::THIS_PET_CANNOT_USE_THIS_ITEM, &[]);
        return;
    }

    if let Some(pi) = world
        .objects
        .get_component_mut::<crate::model::inventory::PetInventory>(&owner)
    {
        pi.0.remove_item(item_id, 1);
    }
    for (skill_id, skill_level) in item_skills(world, item_id) {
        apply_food_skill(world, pet_oid, skill_id, skill_level);
    }
    if is_hungry(world, pet_oid) {
        notify_owner(
            world,
            owner,
            sm_ids::YOUR_PET_ATE_A_LITTLE_BUT_IS_STILL_HUNGRY,
            &[],
        );
    }
    send_pet_item_list(world, owner);
    send_pet_info(world, owner, pet_oid, PetInfoKind::Default);
}

/// `(objectId: i32, count: i64)` — the layout both transfer packets share.
fn read_oid_and_count(body: &[u8]) -> Option<(i32, i64)> {
    if body.len() < 12 {
        return None;
    }
    let oid = i32::from_le_bytes([body[0], body[1], body[2], body[3]]);
    let count = i64::from_le_bytes(body[4..12].try_into().ok()?);
    Some((oid, count))
}

fn player_for_client(world: &World, client_id: u32) -> Option<i32> {
    match world.clients.get(&client_id) {
        Some(crate::session::ClientSession::InGame(s)) => Some(s.player_object_id()),
        _ => None,
    }
}
