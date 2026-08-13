//! Servitor visibility and lifetime: the `SummonInfo` broadcast, the 5 s
//! life tick with its consume cost, and owner-logout cleanup.

use super::*;

/// `SummonInfo` to every nearby player except the owner (who has the
/// `PetInfo` view). Used when the servitor first appears.
pub(crate) fn broadcast_summon_info(world: &mut World, servitor_oid: i32, summoned: bool) {
    use crate::model::components::RegionCell;
    let Some(link) = world
        .objects
        .get_component::<ServitorOf>(&servitor_oid)
        .copied()
    else {
        return;
    };
    let Some(region) = region_cell_of(world, servitor_oid) else {
        return;
    };
    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&servitor_oid)
    else {
        return;
    };
    let Some(t) = npc.template(world) else { return };
    let (Some(pos), Some(vitals), Some(speeds), Some(combat)) = (
        world.objects.get_component::<Position>(&servitor_oid),
        world.objects.get_component::<Vitals>(&servitor_oid),
        world.objects.get_component::<Speeds>(&servitor_oid),
        world.objects.get_component::<CombatStats>(&servitor_oid),
    ) else {
        return;
    };
    let owner_name = world
        .objects
        .get_component::<crate::model::Player>(&link.owner_object_id)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    // `Summon.isBetrayed()` — read before the borrow, since the packet build
    // holds references into `world.objects`.
    let betrayed = crate::game_loop::abnormal::flags_of(world, servitor_oid)
        & crate::model::skill::effect_flag::BETRAYED
        != 0;
    let pkt = server_packets::summon_info(
        servitor_oid,
        t,
        pos,
        vitals,
        speeds,
        combat,
        &owner_name,
        0,
        summoned,
        betrayed,
    );
    for cs in world.clients.values() {
        let crate::session::ClientSession::InGame(session) = cs else {
            continue;
        };
        let viewer = session.player_object_id();
        if viewer == link.owner_object_id {
            continue; // the owner has the PetInfo view
        }
        let Some(vr) = world.objects.get_component::<RegionCell>(&viewer) else {
            continue;
        };
        if crate::world::regions_adjacent(region, vr.0) {
            cs.send(pkt.clone());
        }
    }
}

/// Java `Servitor.run()` — the 5-second upkeep tick.
///
/// In order: lifetime countdown (expiry → "Your servitor passed away" +
/// unsummon), the periodic upkeep item (missing → "not enough items" +
/// unsummon), the remain-time bar, and the far-from-owner leash. Reschedules
/// itself while the servitor lives, which is Java's `_summonLifeTask` cancelled
/// on death/despawn.
pub(crate) fn handle_life_tick(world: &mut World, servitor_oid: i32) {
    use crate::network::server_packets::{SmParam, sm_ids};
    let Some(link) = world
        .objects
        .get_component::<ServitorOf>(&servitor_oid)
        .copied()
    else {
        return;
    };
    // Dead or already gone → the chain ends (Java cancels the task).
    if is_dead(world, servitor_oid) {
        return;
    }
    let owner = link.owner_object_id;

    // 1. Lifetime.
    if world.tick >= link.expires_at_tick {
        notify_owner(world, owner, sm_ids::YOUR_SERVITOR_PASSED_AWAY, &[]);
        unsummon_servitor(world, owner);
        return;
    }

    // 2. Upkeep item.
    if link.consume_item_id > 0 && world.tick >= link.next_consume_tick {
        // `destroyItemByItemId` — take the upkeep item, or fail.
        use crate::model::inventory::Inventory;
        let have = world
            .objects
            .get_component::<Inventory>(&owner)
            .map(|inv| inv.count_of(link.consume_item_id))
            .unwrap_or(0);
        let taken = have >= link.consume_item_count;
        if taken {
            let changes = world
                .objects
                .get_component_mut::<Inventory>(&owner)
                .map(|inv| inv.remove_item(link.consume_item_id, link.consume_item_count))
                .unwrap_or_default();
            send_inventory_update(world, owner, changes);
            notify_owner(
                world,
                owner,
                sm_ids::A_SUMMONED_MONSTER_USES_S1,
                &[SmParam::ItemName(link.consume_item_id)],
            );
            if let Some(l) = world.objects.get_component_mut::<ServitorOf>(&servitor_oid) {
                l.next_consume_tick = world.tick + CONSUME_INTERVAL_SECS * TICKS_PER_SECOND;
            }
        } else {
            notify_owner(
                world,
                owner,
                sm_ids::NOT_ENOUGH_ITEMS_TO_MAINTAIN_SERVITOR,
                &[],
            );
            unsummon_servitor(world, owner);
            return;
        }
    }

    // 3. The remaining-time bar.
    if link.life_time_secs > 0 {
        let remaining = (link.expires_at_tick.saturating_sub(world.tick) / TICKS_PER_SECOND) as i32;
        send_to_player(
            world,
            owner,
            server_packets::set_summon_remain_time(link.life_time_secs, remaining),
        );
    }

    // 4. The leash — "using same task to check if owner is in visible range".
    // A servitor left too far behind is dragged back into follow whatever it
    // was doing, so an ordered attack can't strand it across the map.
    if crate::geo::distance::distance_3d(world, servitor_oid, owner)
        .is_some_and(|d| d > LEASH_DISTANCE)
    {
        if let Some(l) = world.objects.get_component_mut::<ServitorOf>(&servitor_oid) {
            l.following = true;
        }
        crate::game_loop::helpers::set_active_intention(world, servitor_oid);
    }

    world.scheduler.schedule(
        world.tick + LIFE_TICK_SECS * TICKS_PER_SECOND,
        crate::scheduler::ScheduledTask::ServitorLifeTick { servitor_oid },
    );
}

pub(super) fn notify_owner(
    world: &World,
    owner_oid: i32,
    sm: i16,
    params: &[crate::network::server_packets::SmParam],
) {
    send_sm_to_player(world, owner_oid, sm, params);
}

/// The owner left the world (logout/disconnect) — their servitor goes with
/// them. Java stores it in `CharSummonTable` for `RestoreServitorOnReconnect`;
/// persistence is a later slice, so this just removes it.
pub(crate) fn on_owner_leave_world(world: &mut World, owner_oid: i32) {
    // Capture the summon's state before the entity goes away — after
    // `unsummon_servitor` there is nothing left to read it from.
    sync_pet_row(world, owner_oid);
    sync_summon_row(world, owner_oid);
    unsummon_servitor(world, owner_oid);
}
