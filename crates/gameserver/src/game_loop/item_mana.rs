//! Shadow-item mana — `Item.decreaseMana` / `Item.scheduleConsumeManaTask` /
//! `taskmanager.ItemManaTaskManager`.
//!
//! A **shadow item** is any item whose template declares `<set name="duration">`
//! (`ItemTemplate.getDuration()`): the instance is born with that many points of
//! "mana" (`Item._mana`, our [`ItemInstance::mana_left`]), and Java's
//! `isShadowItem()` is nothing more than `mana >= 0`. Mana is spent one point
//! per minute **while the item is worn**, and at zero the item unequips itself
//! and is destroyed. That is what keeps the free weapon a class-transfer coupon
//! buys (see [`crate::scripts::shadow_weapons`]) from being a permanent one:
//! 90 minutes of wear for the D-grade set, 300 for the C-grade.
//!
//! ## The beat
//!
//! Java arms one `ThreadPool` entry per item, 60 s out, and re-arms it from
//! inside `decreaseMana`; `_consumingMana` is the "already queued" flag that
//! keeps `scheduleConsumeManaTask` idempotent. Here the entry is a
//! [`ScheduledTask::ItemManaTick`] and the flag is `world.item_mana_consuming`,
//! but the state machine is the same one, including its quirk:
//!
//! `ItemManaTaskManager.run` calls `decreaseMana(item.isEquipped())` — the
//! *reset* argument. Worn → the flag clears and the item re-arms. Taken off
//! before the beat lands → the point is still spent, the flag stays set, and
//! nothing re-arms it. Since nothing ever clears the flag again, an unequipped
//! shadow item stops draining **for the rest of its life** — re-equipping it
//! burns exactly one more point (the `decreaseMana(false)` on equip) and then
//! goes quiet. That is upstream behaviour, not a porting slip; it is reproduced
//! here deliberately so a shadow weapon lasts exactly as long as it does on the
//! Java server.

use tracing::warn;

use crate::model::inventory::Inventory;
use crate::network::enter_world as ew;
use crate::network::server_packets::{self as sp, SmParam, sm_ids};
use crate::scheduler::ScheduledTask;
use crate::world::World;

/// `ItemManaTaskManager.MANA_CONSUMPTION_RATE` (60 000 ms), in 100 ms ticks.
pub const MANA_CONSUMPTION_TICKS: u64 = 600;

/// Java `Item.isShadowItem()` — mana was stamped from the template's
/// `duration`, so a non-negative value *is* the shadow flag.
pub fn is_shadow_item(mana_left: i32) -> bool {
    mana_left >= 0
}

/// `Item.scheduleConsumeManaTask()`: arm the 60 s beat unless this item
/// already has one in flight.
pub(crate) fn schedule_consume_mana_task(world: &mut World, player_oid: i32, item_oid: i32) {
    if !world.item_mana_consuming.insert(item_oid) {
        return; // `if (_consumingMana) return;`
    }
    world.scheduler.schedule(
        world.tick + MANA_CONSUMPTION_TICKS,
        ScheduledTask::ItemManaTick {
            player_object_id: player_oid,
            item_object_id: item_oid,
        },
    );
}

/// `ItemManaTaskManager.run`'s per-entry body. The map entry is consumed
/// either way (Java's `iterator.remove()`), but the `_consumingMana` flag is
/// only cleared by `decrease_mana` when the item is still worn — see the
/// module note.
pub(crate) fn on_mana_tick(world: &mut World, player_oid: i32, item_oid: i32) {
    // "if ((player == null) || player.isInOfflineMode()) continue" — an
    // offline shop's wares don't burn down while their owner is away.
    let offline = world.offline_traders.contains_key(&player_oid);
    if offline || super::helpers::client_for_player(world, player_oid).is_none() {
        return;
    }
    let equipped = is_equipped(world, player_oid, item_oid);
    decrease_mana(world, player_oid, item_oid, equipped, 1);
}

fn is_equipped(world: &World, player_oid: i32, item_oid: i32) -> bool {
    world
        .objects
        .get_component::<Inventory>(&player_oid)
        .is_some_and(|inv| inv.paperdoll_slot_of(item_oid).is_some())
}

/// `Item.decreaseMana(resetConsumingMana, count)`: spend `count` mana, warn at
/// 10/5/1, and at 0 unequip + destroy the item. A no-op for anything that
/// isn't a shadow item, exactly like Java's opening guard — which is why every
/// caller can fire it blindly at whatever the player just equipped.
pub(crate) fn decrease_mana(
    world: &mut World,
    player_oid: i32,
    item_oid: i32,
    reset_consuming_mana: bool,
    count: i32,
) {
    let Some((item_id, mana)) = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .and_then(|inv| {
            inv.items()
                .iter()
                .find(|it| it.object_id == item_oid)
                .map(|it| (it.item_id, it.mana_left))
        })
    else {
        return;
    };
    if !is_shadow_item(mana) {
        return;
    }
    // `if ((_mana - count) >= 0) _mana -= count; else _mana = 0;`
    let left = (mana - count).max(0);
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player_oid) {
        inv.set_mana_left(item_oid, left);
    }
    if reset_consuming_mana {
        world.item_mana_consuming.remove(&item_oid);
    }
    // Everything below is Java's `if (player != null)` block. Memory-first:
    // the new mana rides the player's next flush, no DB write per minute.
    let Some(client_id) = super::helpers::client_for_player(world, player_oid) else {
        return;
    };
    let warning = match left {
        10 => Some(sm_ids::S1_S_REMAINING_MANA_IS_NOW_10),
        5 => Some(sm_ids::S1_S_REMAINING_MANA_IS_NOW_5),
        1 => Some(sm_ids::S1_S_REMAINING_MANA_IS_NOW_1_IT_WILL_DISAPPEAR_SOON),
        _ => None,
    };
    if let Some(sm) = warning
        && let Some(cs) = world.clients.get(&client_id)
    {
        cs.send(sp::system_message_with(sm, &[SmParam::ItemName(item_id)]));
    }

    if left > 0 {
        // "Reschedule if still equipped".
        if !world.item_mana_consuming.contains(&item_oid)
            && is_equipped(world, player_oid, item_oid)
        {
            schedule_consume_mana_task(world, player_oid, item_oid);
        }
        // Java's `_loc != WAREHOUSE` guard: only an inventory item refreshes
        // the client. Ours only ever ticks inventory items.
        let iu = world
            .objects
            .get_component::<Inventory>(&player_oid)
            .map(|inv| ew::inventory_update(inv, &world.data, &[item_oid]));
        if let Some(iu) = iu {
            super::helpers::send_inventory_update(world, client_id, player_oid, iu);
        }
        return;
    }

    // "The life time has expired."
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(sp::system_message_with(
            sm_ids::S1_S_REMAINING_MANA_IS_NOW_0_AND_THE_ITEM_HAS_DISAPPEARED,
            &[SmParam::ItemName(item_id)],
        ));
    }
    world.item_mana_consuming.remove(&item_oid);
    if is_equipped(world, player_oid, item_oid) {
        let changed = world
            .objects
            .get_component_mut::<Inventory>(&player_oid)
            .map(|inv| inv.unequip_item(item_oid))
            .unwrap_or_default();
        // Java sends the unequip `InventoryUpdate` and `broadcastUserInfo`
        // here; `finish_equip_change` is that pair plus the stat recompute the
        // paperdoll change owes (see its own doc comment).
        super::items::finish_equip_change(world, client_id, player_oid, &changed);
    }
    let Some(change) = world
        .objects
        .get_component_mut::<Inventory>(&player_oid)
        .and_then(|inv| inv.remove_by_object_id(item_oid, 1))
    else {
        warn!("item_mana: {item_oid} vanished before its mana-0 destroy.");
        return;
    };
    let packet = ew::inventory_update_changes(&world.data, &[change]);
    super::helpers::send_inventory_update(world, client_id, player_oid, packet);
}

/// Java `Player.useEquipableItem`'s equip branch: "Consume mana - will start a
/// task if required; returns if item is not a shadow item". Called for every
/// item that just *became* equipped.
pub(crate) fn on_item_equipped(world: &mut World, player_oid: i32, item_oid: i32) {
    decrease_mana(world, player_oid, item_oid, false, 1);
}

/// `EnterWorld.runImpl`'s inventory sweep: "if (item.isShadowItem() &&
/// item.isEquipped()) item.decreaseMana(false)" — one point for the login, and
/// the beat starts from there. (Java's `isTimeLimitedItem` half of the same
/// loop is still unported; see `ItemTemplate.time`.)
pub(crate) fn on_enter_world(world: &mut World, player_oid: i32) {
    let worn: Vec<i32> = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .map(|inv| {
            inv.items()
                .iter()
                .filter(|it| {
                    is_shadow_item(it.mana_left) && inv.paperdoll_slot_of(it.object_id).is_some()
                })
                .map(|it| it.object_id)
                .collect()
        })
        .unwrap_or_default();
    for item_oid in worn {
        decrease_mana(world, player_oid, item_oid, false, 1);
    }
}
