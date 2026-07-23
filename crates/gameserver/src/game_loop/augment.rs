//! Augmentation (variation) flow (Java `Augment` bypass +
//! `RequestConfirmRefinerItem` / `RequestRefine` / `RequestRefineCancel`), built
//! on the [`VariationData`](crate::data::variation_data::VariationData) roll
//! engine.
//!
//! Make: `Augment 1` bypass → `ExShowVariationMakeWindow` → the client picks the
//! weapon + life stone ([`handle_confirm_refiner`], which echoes the gemstone
//! fee) → [`handle_refine`] rolls the two options, consumes the life stone +
//! gemstones, and stamps the variation onto the weapon (`ExVariationResult`).
//! Cancel: `Augment 2` → `ExShowVariationCancelWindow` → [`handle_refine_cancel`]
//! charges the adena cancel fee and strips the variation.
//!
//! Scope: the augment is stored, displayed (`paperdoll_augmentation` →
//! `ExUserInfoEquipSlot`), and cancellable. **Not yet**: the option *effects*
//! (the `stats/augmentation/options/*` stat/skill bonuses — a huge separate
//! data set), and `item_variations` DB persistence (augments are session-only
//! for now). The item-list mask display bit is also still 0.

use commons::network::PacketReader;

use crate::model::inventory::Inventory;
use crate::network::server_packets as sp;
use crate::session::ClientSession;
use crate::world::World;

const ADENA_ID: i32 = 57;

fn player_of(world: &World, client_id: u32) -> Option<i32> {
    match world.clients.get(&client_id) {
        Some(ClientSession::InGame(s)) => Some(s.player_object_id()),
        _ => None,
    }
}

fn send(world: &World, client_id: u32, packet: Vec<u8>) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
}

/// The item id of an inventory instance by object id.
fn item_id_of(world: &World, player: i32, object_id: i32) -> Option<i32> {
    world
        .objects
        .get_component::<Inventory>(&player)
        .and_then(|inv| {
            inv.items()
                .iter()
                .find(|it| it.object_id == object_id)
                .map(|it| it.item_id)
        })
}

/// `Augment` bypass: `1` opens the make window, `2` the cancel window.
pub(crate) fn open_window(world: &mut World, client_id: u32, make: bool) {
    let packet = if make {
        sp::ex_show_variation_make_window()
    } else {
        sp::ex_show_variation_cancel_window()
    };
    send(world, client_id, packet);
}

/// Whether `target_obj` is a valid augment target for life stone `mineral_id`:
/// an un-augmented weapon with fee data for the pair (Java
/// `AbstractRefinePacket.isValid`, narrowed). Returns the gemstone fee.
fn resolve_fee<'a>(
    world: &'a World,
    player: i32,
    target_obj: i32,
    mineral_id: i32,
) -> Option<&'a crate::data::variation_data::VariationFee> {
    let inv = world.objects.get_component::<Inventory>(&player)?;
    let target = inv.items().iter().find(|it| it.object_id == target_obj)?;
    if target.is_augmented() {
        return None;
    }
    let template = world.data.item_data.get(target.item_id)?;
    if template.kind != crate::data::item_data::ItemKind::Weapon {
        return None;
    }
    if !world.data.variations.has_variation(mineral_id) {
        return None;
    }
    world.data.variations.fee(target.item_id, mineral_id)
}

/// `RequestConfirmRefinerItem` (Ex 0x27): validate the weapon + life stone and
/// echo the gemstone fee to the make window.
pub(crate) fn handle_confirm_refiner(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = player_of(world, client_id) else {
        return;
    };
    let mut r = PacketReader::new(body);
    let (Some(target_obj), Some(refiner_obj)) = (r.read_i32(), r.read_i32()) else {
        return;
    };
    let Some(mineral_id) = item_id_of(world, player, refiner_obj) else {
        return;
    };
    let Some(fee) = resolve_fee(world, player, target_obj, mineral_id) else {
        // "This is not a suitable item." — no confirm echo.
        return;
    };
    let packet = sp::ex_put_intensive_result_for_variation_make(
        refiner_obj,
        mineral_id,
        fee.item_id,
        fee.item_count,
    );
    send(world, client_id, packet);
}

/// `RequestRefine` (Ex 0x3E): roll the augment, consume the life stone +
/// gemstones, and stamp the variation onto the weapon.
pub(crate) fn handle_refine(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = player_of(world, client_id) else {
        return;
    };
    let mut r = PacketReader::new(body);
    let (Some(target_obj), Some(mineral_obj), Some(fee_obj), Some(fee_count)) =
        (r.read_i32(), r.read_i32(), r.read_i32(), r.read_i64())
    else {
        return;
    };

    let fail = |world: &mut World| send(world, client_id, sp::ex_variation_result(0, 0, false));

    let (Some(mineral_id), Some(fee_item_id)) = (
        item_id_of(world, player, mineral_obj),
        item_id_of(world, player, fee_obj),
    ) else {
        fail(world);
        return;
    };
    // Validate target + fee (the gemstone must match the fee, count included).
    let (target_item_id, is_magic) = {
        let Some(inv) = world.objects.get_component::<Inventory>(&player) else {
            return;
        };
        let Some(target) = inv.items().iter().find(|it| it.object_id == target_obj) else {
            fail(world);
            return;
        };
        let id = target.item_id;
        (
            id,
            world
                .data
                .item_data
                .get(id)
                .map(|t| t.is_magic_weapon)
                .unwrap_or(false),
        )
    };
    let Some(fee) = resolve_fee(world, player, target_obj, mineral_id).copied() else {
        fail(world);
        return;
    };
    if fee.item_id != fee_item_id || fee.item_count != fee_count {
        fail(world);
        return;
    }
    // Enough materials on hand?
    let inv_has = |world: &World, id: i32, n: i64| {
        world
            .objects
            .get_component::<Inventory>(&player)
            .is_some_and(|inv| inv.count_of(id) >= n)
    };
    if !inv_has(world, mineral_id, 1) || !inv_has(world, fee_item_id, fee_count) {
        fail(world);
        return;
    }

    // Roll the two options (the engine reads `data.variations` while drawing
    // from the RNG — `World::roll_augment` keeps that split borrow disjoint).
    let Some((option1, option2)) = world.roll_augment(mineral_id, is_magic) else {
        fail(world);
        return;
    };

    // Consume the life stone + gemstones, then stamp the augment.
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player) {
        inv.remove_by_object_id(mineral_obj, 1);
        inv.remove_by_object_id(fee_obj, fee_count);
        inv.set_augmentation(target_obj, mineral_id, option1, option2);
    }
    let _ = target_item_id;

    send(
        world,
        client_id,
        sp::ex_variation_result(option1, option2, true),
    );
    refresh(world, client_id, player);
    // If the weapon is equipped, its equip-slot augment display must refresh.
    if world
        .objects
        .get_component::<Inventory>(&player)
        .is_some_and(|inv| inv.paperdoll_slot_of(target_obj).is_some())
    {
        super::items::finish_equip_change(world, client_id, player, &[target_obj]);
    }
}

/// `RequestRefineCancel` (Ex 0x40): strip a weapon's augment for the adena
/// cancel fee.
pub(crate) fn handle_refine_cancel(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = player_of(world, client_id) else {
        return;
    };
    let Some(target_obj) = PacketReader::new(body).read_i32() else {
        return;
    };

    let fail = |world: &mut World| send(world, client_id, sp::ex_variation_cancel_result(false));

    let (Some(target_item_id), Some(mineral_id)) = (
        item_id_of(world, player, target_obj),
        world
            .objects
            .get_component::<Inventory>(&player)
            .and_then(|inv| inv.augment_mineral(target_obj)),
    ) else {
        fail(world);
        return;
    };
    let Some(price) = world.data.variations.cancel_fee(target_item_id, mineral_id) else {
        fail(world);
        return;
    };
    if price < 0
        || !world
            .objects
            .get_component::<Inventory>(&player)
            .is_some_and(|inv| inv.count_of(ADENA_ID) >= price)
    {
        fail(world);
        return;
    }
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player) {
        inv.remove_item(ADENA_ID, price);
        inv.remove_augmentation(target_obj);
    }
    send(world, client_id, sp::ex_variation_cancel_result(true));
    refresh(world, client_id, player);
    if world
        .objects
        .get_component::<Inventory>(&player)
        .is_some_and(|inv| inv.paperdoll_slot_of(target_obj).is_some())
    {
        super::items::finish_equip_change(world, client_id, player, &[target_obj]);
    }
}

fn refresh(world: &World, client_id: u32, player: i32) {
    if let Some(inv) = world.objects.get_component::<Inventory>(&player) {
        send(
            world,
            client_id,
            crate::network::enter_world::item_list(inv, &world.data, false),
        );
    }
}
