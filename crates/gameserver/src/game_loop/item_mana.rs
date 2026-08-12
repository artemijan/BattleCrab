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
//!
//! ## Who spends a point
//!
//! Exactly three callers, all of them Java's:
//!
//! * the 60 s beat above (`ItemManaTaskManager.run`),
//! * `Player.useEquipableItem`, for the **one item the player just clicked**
//!   and only when it actually went on ([`on_item_equipped`]),
//! * `EnterWorld`'s inventory sweep, one point per worn shadow item at login
//!   ([`on_enter_world`]).
//!
//! Nothing else. In particular this must **not** hang off the shared
//! [`super::items::finish_equip_change`] helper: that one is also how an
//! enchant refreshes a worn item's glow, how augmenting re-applies its options
//! and how `//mount` takes a weapon off — none of which touch mana in Java,
//! but every one of which would have burned a point had the call lived there,
//! so a shadow weapon that got enchanted died sooner than the Java one.
//!
//! ## The flag is session-scoped
//!
//! Java's `_consumingMana` is a field on the `Item` instance, and a logout
//! throws every one of the player's `Item`s away: the next login rebuilds them
//! with the flag false, so `EnterWorld`'s `decreaseMana` re-arms the beat.
//! Ours is a `World` map keyed by object id — and those ids come straight back
//! out of the `items` table on the next login — so it has to be cleared by
//! hand when a player leaves ([`on_player_leave_world`]). Without that, a
//! logout taken while a beat was in flight left the flag set for good and the
//! weapon never ticked again: it then only ever lost the one point per equip,
//! which is precisely the "mana only moves when I re-equip it" symptom.
//!
//! The beat that was in flight when they left is dropped rather than honoured:
//! the map records the tick each beat is *due*, and [`on_mana_tick`] ignores
//! any firing that does not match the armed one. Java gets this for free — its
//! orphaned map entry points at the discarded `Item`, whose `getActingPlayer()`
//! is null, so `run` throws it away — and without it a relog inside the beat
//! window would leave two beats racing on one item, draining it twice as fast.

use crate::game_loop::helpers::{get_inventory_items_oids, send_sm_to_client, send_to_client};
use tracing::warn;

use crate::model::inventory::Inventory;
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
    if world.item_mana_consuming.contains_key(&item_oid) {
        return; // `if (_consumingMana) return;`
    }
    let due = world.tick + MANA_CONSUMPTION_TICKS;
    world.item_mana_consuming.insert(item_oid, due);
    world.scheduler.schedule(
        due,
        ScheduledTask::ItemManaTick {
            player_object_id: player_oid,
            item_object_id: item_oid,
        },
    );
}

/// Java's `_consumingMana` lives on the `Item` instance and dies with it when
/// the player logs out; ours is keyed by an object id that survives into the
/// next session, so the flags for everything this player owns are dropped
/// here. Any beat still in flight is orphaned by the same stroke — its due
/// tick no longer matches the (now absent) entry, so [`on_mana_tick`] discards
/// it, exactly as Java's `run` discards an entry whose `Item` has no acting
/// player.
pub(crate) fn on_player_leave_world(world: &mut World, player_oid: i32) {
    let owned: Vec<i32> = get_inventory_items_oids(world, player_oid);
    for oid in owned {
        world.item_mana_consuming.remove(&oid);
    }
}

/// `ItemManaTaskManager.run`'s per-entry body. The map entry is consumed
/// either way (Java's `iterator.remove()`), but the `_consumingMana` flag is
/// only cleared by `decrease_mana` when the item is still worn — see the
/// module note.
pub(crate) fn on_mana_tick(world: &mut World, player_oid: i32, item_oid: i32) {
    // A beat left over from a previous session (the player logged out and back
    // in inside the 60 s window) is not this session's beat — see the module
    // note. Java drops it because its `Item` has no acting player; here the
    // armed due tick is what tells the two apart.
    if world.item_mana_consuming.get(&item_oid) != Some(&world.tick) {
        return;
    }
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
            inv.by_object_id(item_oid)
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
    if let Some(sm) = warning {
        send_sm_to_client(world, client_id, sm, &[SmParam::ItemName(item_id)]);
    }

    if left > 0 {
        // "Reschedule if still equipped".
        if !world.item_mana_consuming.contains_key(&item_oid)
            && is_equipped(world, player_oid, item_oid)
        {
            schedule_consume_mana_task(world, player_oid, item_oid);
        }
        // Java's `_loc != WAREHOUSE` guard: only an inventory item refreshes
        // the client. Ours only ever ticks inventory items.
        let changes = super::helpers::modified_changes(world, player_oid, &[item_oid]);
        super::helpers::send_inventory_update(world, player_oid, changes);
        return;
    }

    // "The life time has expired."
    send_to_client(
        world,
        client_id,
        sp::system_message_with(
            sm_ids::S1_S_REMAINING_MANA_IS_NOW_0_AND_THE_ITEM_HAS_DISAPPEARED,
            &[SmParam::ItemName(item_id)],
        ),
    );
    world.item_mana_consuming.remove(&item_oid);
    // Java sends the unequip `InventoryUpdate` and `broadcastUserInfo` here;
    // `finish_equip_change` inside the helper is that pair plus the stat
    // recompute the paperdoll change owes (see its own doc comment).
    super::items::unequip_if_worn(world, client_id, player_oid, item_oid);
    let Some(change) = world
        .objects
        .get_component_mut::<Inventory>(&player_oid)
        .and_then(|inv| inv.remove_by_object_id(item_oid, 1))
    else {
        warn!("item_mana: {item_oid} vanished before its mana-0 destroy.");
        return;
    };
    super::helpers::send_inventory_update(world, player_oid, vec![change]);
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
