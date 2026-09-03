//! Mercenary posting — `handlers/itemhandlers/MercTicket` plus the half of
//! `instancemanager/SiegeGuardManager` that deals with **hired** guards.
//!
//! Between sieges the castle's owning clan buys posting tickets from the
//! Mercenary Manager and uses them where it wants a defender to stand. Each use
//! writes a `castle_siege_guards` row with `isHired = 1`, drops the ticket on
//! the ground to mark the spot, and the mercenary itself only appears when the
//! siege starts — beside the stationed garrison, which is the same table's
//! `isHired = 0` rows and was already ported.
//!
//! **The dropped ticket is the record, in Java and here.** `SiegeGuardManager`
//! keeps the `Item` objects in `_droppedTickets` and reads them back for the
//! spacing rule and the per-guard cap; this port keeps
//! [`Mercenary`](Mercenary) rows in `World::mercenaries`
//! and drops the ticket alongside, because the ground item is player-visible
//! and `ItemAction`'s pickup refusal already expects to find one there.
//!
//! SKIP(census): Java's `spawnMercenary` calls `scheduleDespawn(3000)` on the
//! guard it spawns from `addTicket` — a 3-second preview of the mercenary at
//! the moment of posting. The siege-start spawn is the real one and is
//! unaffected.

use crate::db::DbCommand;
use crate::game_loop::character::inventory;
use crate::game_loop::helpers::{send_sm_and_action_failed, send_to_client};
use crate::game_loop::space::position;
use crate::model::Player;
use crate::model::siege::Mercenary;
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::world::World;

/// Java `SiegeGuardManager.isTooCloseToAnotherTicket`: 25 units.
const TICKET_SPACING: f64 = 25.0;

/// `ConfirmDlg.addTime(15000)` — the prompt auto-declines after 15 s.
const CONFIRM_TIMEOUT_MS: i32 = 15_000;

/// `MercTicket.useItem` — the guards, then the confirmation prompt.
///
/// Returns whether the ticket was claimed; the item is **not** consumed here.
/// Java destroys it only once the player answers yes, which is why a declined
/// prompt leaves the ticket in the bag.
pub(crate) fn use_ticket(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    item_object_id: i32,
    item_id: i32,
) {
    let Some(pos) = position::maybe_position(world, object_id) else {
        return;
    };
    // `CastleManager.getCastle(player)` — which castle's grounds is this?
    let castle_id = world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z);
    let clan_id = world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| p.clan_id)
        .unwrap_or(0);
    let owns = castle_id.is_some_and(|id| {
        clan_id != 0 && world.clans.get(&clan_id).is_some_and(|c| c.castle_id == id)
    });
    if !owns
        || !crate::game_loop::clans::has_clan_privilege(
            world,
            object_id,
            crate::model::clan::CS_MERCENARIES,
        )
    {
        send_sm_and_action_failed(
            world,
            client_id,
            sm_ids::YOU_DO_NOT_HAVE_THE_AUTHORITY_TO_POSITION_MERCENARIES,
            &[],
        );
        return;
    }
    let castle_id = castle_id.unwrap_or(0);

    // This ticket has to belong to *this* castle — a Gludio ticket posts
    // nothing at Giran.
    let Some(holder) = world
        .data
        .castle_siege_guards
        .by_item(castle_id, item_id)
        .copied()
    else {
        send_sm_and_action_failed(
            world,
            client_id,
            sm_ids::MERCENARIES_CANNOT_BE_POSITIONED_HERE,
            &[],
        );
        return;
    };
    if world.sieges.get(&castle_id).is_some_and(|s| s.in_progress) {
        send_sm_and_action_failed(
            world,
            client_id,
            sm_ids::THIS_MERCENARY_CANNOT_BE_POSITIONED_ANYMORE,
            &[],
        );
        return;
    }
    if too_close_to_another_ticket(world, pos.x, pos.y, pos.z) {
        send_sm_and_action_failed(
            world,
            client_id,
            sm_ids::POSITIONING_CANNOT_BE_DONE_HERE_BECAUSE_THE_DISTANCE_BETWEEN_MERCENARIES_IS_TOO_SHORT,
            &[],
        );
        return;
    }
    if at_npc_limit(world, castle_id, item_id, holder.max_npc_amount) {
        send_sm_and_action_failed(
            world,
            client_id,
            sm_ids::THIS_MERCENARY_CANNOT_BE_POSITIONED_ANYMORE,
            &[],
        );
        return;
    }

    // `player.addAction(PlayerAction.MERCENARY_CONFIRM)` + the prompt. The
    // pending item rides on the player, so a second ticket used before the
    // first is answered replaces it — as Java's per-player map does.
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.pending_mercenary_ticket = Some(item_object_id);
    }
    send_to_client(
        world,
        client_id,
        server_packets::confirm_dlg_with(
            sm_ids::PLACE_S1_IN_THE_CURRENT_LOCATION_AND_DIRECTION_DO_YOU_WISH_TO_CONTINUE as i32,
            &[SmParam::NpcName(holder.npc_id)],
            CONFIRM_TIMEOUT_MS,
            0,
        ),
    );
}

/// `MercTicket.onPlayerDlgAnswer` — the confirmation came back.
///
/// Returns whether this handler claimed the answer, so the shared `DlgAnswer`
/// dispatch can fall through to its other claimants.
pub(crate) fn handle_confirm(world: &mut World, object_id: i32, accepted: bool) -> bool {
    let Some(item_object_id) = world
        .objects
        .get_component_mut::<Player>(&object_id)
        .and_then(|p| p.pending_mercenary_ticket.take())
    else {
        return false;
    };
    if !accepted {
        return true;
    }
    let Some(pos) = position::maybe_position(world, object_id) else {
        return true;
    };
    // Java re-checks the spacing on the *answer* as well as the use: 15 s is
    // long enough for someone else to have posted next to you.
    if too_close_to_another_ticket(world, pos.x, pos.y, pos.z) {
        if let Some(client_id) = crate::game_loop::helpers::client_for_player(world, object_id) {
            send_sm_and_action_failed(
                world,
                client_id,
                sm_ids::POSITIONING_CANNOT_BE_DONE_HERE_BECAUSE_THE_DISTANCE_BETWEEN_MERCENARIES_IS_TOO_SHORT,
                &[],
            );
        }
        return true;
    }
    let Some(item_id) = inventory::item_id_of(world, object_id, item_object_id) else {
        return true;
    };
    let Some(castle_id) = world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z) else {
        return true;
    };
    let Some(holder) = world
        .data
        .castle_siege_guards
        .by_item(castle_id, item_id)
        .copied()
    else {
        return true;
    };
    // `addTicket` re-checks the cap too, for the same reason.
    if at_npc_limit(world, castle_id, item_id, holder.max_npc_amount) {
        return true;
    }

    place(world, castle_id, &holder, pos.x, pos.y, pos.z, pos.heading);

    // `player.destroyItem("Consume", item.getObjectId(), 1, null, false)`.
    let changes = crate::game_loop::items::destroy_item_by_id(world, object_id, item_id, 1);
    if !changes.is_empty() {
        inventory::send_inventory_update(world, object_id, changes);
    }
    true
}

/// `SiegeGuardManager.addTicket`'s tail: persist the row, drop the ticket that
/// marks the spot, and remember the posting.
fn place(
    world: &mut World,
    castle_id: i32,
    holder: &crate::data::castle_siege_guards::SiegeGuardHolder,
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
) {
    let _ = world.db.send(DbCommand::AddHiredSiegeGuard {
        castle_id,
        npc_id: holder.npc_id,
        x,
        y,
        z,
        heading,
    });
    // The ticket lies where the mercenary will stand. `DropSource::Player` with
    // no dropper means no toss animation is owed to anyone in particular.
    let ticket_oid = crate::game_loop::items::ground_items::spawn_ground_item(
        world,
        holder.item_id,
        1,
        0,
        x,
        y,
        z,
        0,
        crate::game_loop::items::ground_items::DropSource::Npc,
    );
    world
        .mercenaries
        .entry(castle_id)
        .or_default()
        .push(Mercenary {
            item_id: holder.item_id,
            npc_id: holder.npc_id,
            x,
            y,
            z,
            heading,
            ticket_oid,
        });
}

/// Java `isTooCloseToAnotherTicket` — within 25 units of *any* posted ticket,
/// on any castle. The check is deliberately global, as Java's is: the ticket
/// set is not partitioned by castle.
fn too_close_to_another_ticket(world: &World, x: i32, y: i32, z: i32) -> bool {
    world.mercenaries.values().flatten().any(|m| {
        let (dx, dy, dz) = ((m.x - x) as f64, (m.y - y) as f64, (m.z - z) as f64);
        (dx * dx + dy * dy + dz * dz).sqrt() < TICKET_SPACING
    })
}

/// Java `isAtNpcLimit` — how many of *this ticket* are already posted, against
/// the `npcMaxAmount` its `<guard>` row declares.
///
/// Java counts across every castle (`_droppedTickets` is one flat set) and the
/// count is per **item id**, not per npc id; both are reproduced, since ticket
/// ids do not repeat between castles.
fn at_npc_limit(world: &World, _castle_id: i32, item_id: i32, max: i32) -> bool {
    let count = world
        .mercenaries
        .values()
        .flatten()
        .filter(|m| m.item_id == item_id)
        .count() as i32;
    count >= max
}

/// `SiegeGuardManager.spawnSiegeGuard`'s hired half — called at siege start
/// beside the garrison spawn.
pub(super) fn spawn_hired(world: &mut World, castle_id: i32) {
    let spawns: Vec<crate::model::siege::SiegeSpawn> = world
        .mercenaries
        .get(&castle_id)
        .map(|list| {
            list.iter()
                .map(|m| crate::model::siege::SiegeSpawn {
                    npc_id: m.npc_id,
                    x: m.x,
                    y: m.y,
                    // `npc.spawnMe(x, y, z + 20)`.
                    z: m.z + 20,
                    heading: m.heading,
                })
                .collect()
        })
        .unwrap_or_default();
    if spawns.is_empty() {
        return;
    }
    super::spawn_siege_npcs(world, castle_id, &spawns);
}

/// `SiegeGuardManager.deleteTickets` + `removeSiegeGuards` — every posting for
/// this castle is undone, which is what a change of ownership does.
pub(crate) fn clear_castle(world: &mut World, castle_id: i32) {
    let Some(list) = world.mercenaries.remove(&castle_id) else {
        return;
    };
    if list.is_empty() {
        return;
    }
    for m in &list {
        if m.ticket_oid != 0
            && let Some(region) = position::region_cell_of(world, m.ticket_oid)
        {
            crate::game_loop::items::ground_items::despawn_ground_item(world, m.ticket_oid, region);
        }
    }
    let _ = world
        .db
        .send(DbCommand::ClearHiredSiegeGuards { castle_id });
}
