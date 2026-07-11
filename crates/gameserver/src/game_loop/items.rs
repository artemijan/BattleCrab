//! Gear equip/unequip handlers (`UseItem`, `RequestUnEquipItem`).

use crate::db;
use crate::network::client_packets as cp;
use crate::session::ClientSession;
use crate::world::World;

/// Port of `clientpackets/UseItem.runImpl`, scoped to gear: right-clicking a
/// `Weapon`/`Armor` toggles equip/unequip (Java routes both through this same
/// packet). `EtcItem` "use" (potions, soulshots, …) is a later milestone — the
/// packet is consumed silently for those.
pub(crate) fn handle_use_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::UseItem::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    let catalog = &world.data.item_data;
    let Some(player) = world.players.get_mut(&object_id) else { return };

    let Some(item) = player.inventory.items().iter().find(|i| i.object_id == pkt.object_id) else { return };
    let Some(template) = catalog.get(item.item_id) else { return };
    if !template.is_equipable() {
        return; // EtcItem "use" (potions, shots, …): later milestone.
    }
    let body_part = template.body_part;

    let changed = if player.inventory.paperdoll_slot_of(pkt.object_id).is_some() {
        player.inventory.unequip_body_part(body_part)
    } else {
        player.inventory.equip_item(catalog, pkt.object_id)
    };
    finish_equip_change(world, client_id, object_id, &changed);
}

/// Port of `clientpackets/RequestUnEquipItem.runImpl` (combat/cursed-weapon
/// guards are skipped — there's no combat system yet).
pub(crate) fn handle_request_un_equip_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(body_part) = cp::read_char_slot(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    let Some(player) = world.players.get_mut(&object_id) else { return };
    let changed = player.inventory.unequip_slot(body_part);
    finish_equip_change(world, client_id, object_id, &changed);
}

/// Shared tail of the equip/unequip handlers: persist each changed slot
/// (`items.loc`/`loc_data`), then resend `InventoryUpdate` + `UserInfo` (Java:
/// `sendInventoryUpdate` + `broadcastUserInfo`).
pub(crate) fn finish_equip_change(world: &mut World, client_id: u32, object_id: i32, changed: &[i32]) {
    if changed.is_empty() {
        return;
    }
    let Some(player) = world.players.get(&object_id) else { return };
    for &oid in changed {
        let (loc, loc_data) = match player.inventory.paperdoll_slot_of(oid) {
            Some(slot) => ("PAPERDOLL", slot as i32),
            None => ("INVENTORY", 0),
        };
        let _ = world.db.send(db::DbCommand::UpdateItemLocation { object_id: oid, loc, loc_data });
    }
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(crate::network::enter_world::inventory_update(player, &world.data, changed));
        cs.send(crate::network::user_info::user_info(player, &world.data));
    }
}

