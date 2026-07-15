//! Ground items (`ItemsOnGroundManager`): items lying in the world as entities
//! with [`GroundItem`]/[`Position`]/[`RegionCell`], indexed in
//! `World::ground_item_regions`. Created by a player drop (`RequestDropItem`) or
//! monster death with auto-loot off, made visible to players entering the
//! region (`SpawnItem`, via `visibility`), and picked up by a click (`Action` →
//! [`pickup_ground_item`]).

use crate::model::components::{GroundItem, Position, RegionCell};
use crate::model::inventory::Inventory;
use crate::network::client_packets as cp;
use crate::network::server_packets::{self, GroundItemView};
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::{region_of, World};

/// `Config.AUTODESTROY_ITEM_AFTER` — a dropped ground item auto-destroys after
/// this many seconds (Java default 600).
const GROUND_ITEM_DECAY_SECS: u64 = 600;

/// Build a ground item's wire view (display id = item id — no disguised items;
/// stackable from the template).
pub(crate) fn ground_item_view(world: &World, oid: i32) -> Option<GroundItemView> {
    let g = world.objects.get_component::<GroundItem>(&oid)?;
    let pos = world.objects.get_component::<Position>(&oid)?;
    let stackable = world.data.item_data.get(g.item_id).map(|t| t.is_stackable).unwrap_or(false);
    Some(GroundItemView {
        object_id: g.object_id,
        display_id: g.item_id,
        x: pos.x,
        y: pos.y,
        z: pos.z,
        stackable,
        count: g.count,
        enchant: g.enchant,
    })
}

/// Drop an item into the world at `(x, y, z)` and broadcast the toss animation
/// (`DropItem` from `dropper_oid`). Returns the ground item's object id.
pub(crate) fn spawn_ground_item(
    world: &mut World,
    item_id: i32,
    count: i64,
    enchant: i32,
    x: i32,
    y: i32,
    z: i32,
    dropper_oid: i32,
) -> i32 {
    let object_id = world.next_npc_object_id;
    world.next_npc_object_id += 1;
    let region = region_of(x, y);
    world.ground_item_regions.entry(region).or_default().push(object_id);
    world.objects.spawn(
        object_id,
        (GroundItem { object_id, item_id, count, enchant }, Position { x, y, z, heading: 0 }, RegionCell(region)),
    );
    if let Some(view) = ground_item_view(world, object_id) {
        super::helpers::broadcast_near_region(world, region, &server_packets::drop_item(dropper_oid, &view));
    }
    world
        .scheduler
        .schedule(world.tick + GROUND_ITEM_DECAY_SECS * 10, ScheduledTask::GroundItemDecay { item_object_id: object_id });
    object_id
}

/// `ItemsOnGroundManager` cleanup task: remove a ground item that has lain past
/// its lifetime (no-op if it was already picked up).
pub(crate) fn handle_ground_item_decay(world: &mut World, item_object_id: i32) {
    let Some(region) = world.objects.get_component::<RegionCell>(&item_object_id).map(|r| r.0) else { return };
    if !world.objects.has_component::<GroundItem>(&item_object_id) {
        return;
    }
    despawn_ground_item(world, item_object_id, region);
}

/// Remove a ground item from the world (despawn + drop from the region index +
/// `DeleteObject` to nearby).
fn despawn_ground_item(world: &mut World, item_oid: i32, region: (i32, i32)) {
    world.objects.despawn(&item_oid);
    if let Some(ids) = world.ground_item_regions.get_mut(&region) {
        ids.retain(|&id| id != item_oid);
    }
    super::helpers::broadcast_near_region(world, region, &server_packets::delete_object(item_oid));
}

/// `Player.doPickupItem`: pick a ground item up into `player_oid`'s inventory —
/// the pickup animation to nearby, remove from the world, and add to inventory
/// with the "you obtained" message + `InventoryUpdate`. (Enchant is not carried
/// through the give path yet — stackable drops are enchant 0; enchanted gear
/// pickup keeping its level is a TODO.)
pub(crate) fn pickup_ground_item(world: &mut World, client_id: u32, player_oid: i32, item_oid: i32) {
    let Some(g) = world.objects.get_component::<GroundItem>(&item_oid).cloned() else { return };
    let Some(pos) = world.objects.get_component::<Position>(&item_oid).copied() else { return };
    let region = world.objects.get_component::<RegionCell>(&item_oid).map(|r| r.0).unwrap_or_else(|| region_of(pos.x, pos.y));
    super::helpers::broadcast_near_region(world, region, &server_packets::get_item(player_oid, item_oid, pos.x, pos.y, pos.z));
    despawn_ground_item(world, item_oid, region);
    super::quests::give_item_with_earned_message(world, client_id, player_oid, g.item_id, g.count);
}

/// Port of `clientpackets/RequestDropItem.runImpl` (narrowed): drop `count` of
/// an inventory item onto the ground at the player's feet. Quest items are
/// protected; a worn item is unequipped first. Java's precise drop-location /
/// distance / weight guards are simplified (drop at the player's position).
pub(crate) fn handle_request_drop_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestDropItem::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let player_oid = session.player_object_id();
    if pkt.count <= 0 {
        return;
    }
    let Some((item_id, held, enchant, is_stackable, is_quest)) = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .and_then(|inv| inv.items().iter().find(|it| it.object_id == pkt.object_id).map(|it| (it.item_id, it.count, it.enchant_level)))
        .map(|(id, cnt, ench)| {
            let t = world.data.item_data.get(id);
            (id, cnt, ench, t.map(|t| t.is_stackable).unwrap_or(false), t.map(|t| t.is_quest_item).unwrap_or(false))
        })
    else {
        return;
    };
    if is_quest || (!is_stackable && pkt.count > 1) {
        return;
    }
    let count = pkt.count.min(held);
    let Some(ppos) = world.objects.get_component::<Position>(&player_oid).copied() else { return };

    // Unequip first if worn (Java unequips before the drop, with its own update).
    if world.objects.get_component::<Inventory>(&player_oid).is_some_and(|inv| inv.paperdoll_slot_of(pkt.object_id).is_some()) {
        let changed = world
            .objects
            .get_component_mut::<Inventory>(&player_oid)
            .map(|inv| inv.unequip_item(pkt.object_id))
            .unwrap_or_default();
        super::items::finish_equip_change(world, client_id, player_oid, &changed);
    }

    let Some(change) = world.objects.get_component_mut::<Inventory>(&player_oid).and_then(|inv| inv.remove_by_object_id(pkt.object_id, count)) else {
        return;
    };
    let packet = crate::network::enter_world::inventory_update_changes(&world.data, &[change]);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
    spawn_ground_item(world, item_id, count, enchant, ppos.x, ppos.y, ppos.z, player_oid);
}
