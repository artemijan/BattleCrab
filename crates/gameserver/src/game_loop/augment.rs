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

use crate::data::item_data::ADENA_ID;
use commons::network::PacketReader;

use super::helpers::{player_of, send_to_client as send};
use super::helpers::{send_inventory_item_list, send_sm_bare_to_client as send_sm};
use crate::game_loop::helpers::item_id_of;
use crate::model::inventory::Inventory;
use crate::network::client_packets as cp;
use crate::network::server_packets as sp;
use crate::world::World;

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
fn resolve_fee(
    world: &World,
    player: i32,
    target_obj: i32,
    mineral_id: i32,
) -> Option<&crate::data::variation_data::VariationFee> {
    let inv = world.objects.get_component::<Inventory>(&player)?;
    let target = inv.by_object_id(target_obj)?;
    if target.is_augmented() {
        return None;
    }
    // `AbstractRefinePacket.isValid`: a shadow item is refused outright — its
    // own description says so ("cannot … be granted functions besides
    // enchantment"), and the augment would evaporate with the item anyway.
    if super::item_mana::is_shadow_item(target.mana_left) {
        return None;
    }
    let template = world.data.item_data.get(target.item_id)?;
    if template.kind != crate::data::item_data::ItemKind::Weapon {
        return None;
    }
    // `AbstractRefinePacket.isValid`'s last line — the blacklist check, after
    // every type test has passed.
    if world
        .cfg
        .character
        .augmentation_black_list
        .binary_search(&target.item_id)
        .is_ok()
    {
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

fn refresh_slot_if_equipped(world: &mut World, player: i32, target_obj: i32, client_id: u32) {
    if world
        .objects
        .get_component::<Inventory>(&player)
        .is_some_and(|inv| inv.paperdoll_slot_of(target_obj).is_some())
    {
        super::items::finish_equip_change(world, client_id, player, &[target_obj]);
    }
}

/// `RequestRefine` (Ex 0x3E): roll the augment, consume the life stone +
/// gemstones, and stamp the variation onto the weapon.
pub(crate) fn handle_refine(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = player_of(world, client_id) else {
        return;
    };
    let Some(cp::RefineRequest {
        target_obj,
        mineral_obj,
        fee_obj,
        fee_count,
    }) = cp::RefineRequest::read(body)
    else {
        return;
    };

    let fail = |world: &mut World| send(world, client_id, sp::ex_variation_result(0, 0, false));

    // Java `AbstractRefinePacket.isValid`: no augmenting while cursed.
    if super::cursed_weapon::is_cursed(world, player) {
        fail(world);
        return;
    }

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
        let Some(target) = inv.by_object_id(target_obj) else {
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
    send_inventory_item_list(world, player);
    // If the weapon is equipped, its equip-slot augment display must refresh.
    refresh_slot_if_equipped(world, player, target_obj, client_id);
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
    // Java `Item.setAugmentation(null)` removes the bonuses first — read the
    // option ids off the item *before* they are wiped, or nothing is removed
    // and the modifiers linger on an item that is no longer augmented.
    super::options::remove_item_options(world, player, target_obj);
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player) {
        inv.remove_item(ADENA_ID, price);
        inv.remove_augmentation(target_obj);
    }
    send(world, client_id, sp::ex_variation_cancel_result(true));
    send_inventory_item_list(world, player);
    refresh_slot_if_equipped(world, player, target_obj, client_id);
}

/// `RequestConfirmTargetItem` (ex 0x26): the player dropped a weapon into the
/// augment window's first slot. Java validates that the item *has* fee data
/// (i.e. is augmentable) and echoes it back; an unsuitable item gets
/// `THIS_IS_NOT_A_SUITABLE_ITEM` and no echo.
pub(crate) fn handle_confirm_target_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = player_of(world, client_id) else {
        return;
    };
    let Some(target_obj) = PacketReader::new(body).read_i32() else {
        return;
    };
    let Some(item_id) = item_id_of(world, player, target_obj) else {
        return;
    };
    // Java `VariationData.hasFeeData(itemId)` — any mineral will do.
    if !world.data.variations.has_fee_data(item_id) {
        send_sm(world, client_id, sp::sm_ids::THIS_IS_NOT_A_SUITABLE_ITEM);
        return;
    }
    send(
        world,
        client_id,
        sp::ex_put_item_result_for_variation_make(target_obj, item_id),
    );
}

/// `RequestConfirmGemStone` (ex 0x28): the player dropped the gemstone fee in.
/// Java re-validates the whole triple (weapon, life stone, gemstone) and echoes
/// the fee back; the port re-uses the same fee resolution the refiner step does.
pub(crate) fn handle_confirm_gemstone(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = player_of(world, client_id) else {
        return;
    };
    let Some(cp::RefineRequest {
        target_obj,
        mineral_obj,
        fee_obj,
        fee_count,
    }) = cp::RefineRequest::read(body)
    else {
        return;
    };
    let (Some(mineral_id), Some(gemstone_id)) = (
        item_id_of(world, player, mineral_obj),
        item_id_of(world, player, fee_obj),
    ) else {
        return;
    };
    let Some(fee) = resolve_fee(world, player, target_obj, mineral_id) else {
        send_sm(world, client_id, sp::sm_ids::THIS_IS_NOT_A_SUITABLE_ITEM);
        return;
    };
    // The gemstone the client offers must be the one this fee asks for, in the
    // amount it asks for (Java's `gemStoneItem.getId() != fee.getItemId()` and
    // count checks).
    if gemstone_id != fee.item_id || fee_count != fee.item_count {
        send_sm(world, client_id, sp::sm_ids::THIS_IS_NOT_A_SUITABLE_ITEM);
        return;
    }
    send(
        world,
        client_id,
        sp::ex_put_commission_result_for_variation_make(fee_obj, gemstone_id, fee_count),
    );
}

/// `RequestConfirmCancelItem` (ex 0x3F): the player dropped an augmented item
/// into the *cancel* window. Java refuses a non-augmented item with
/// `AUGMENTATION_REMOVAL_CAN_ONLY_BE_DONE_ON_AN_AUGMENTED_ITEM`, else echoes it
/// back with its two option ids and the adena price.
pub(crate) fn handle_confirm_cancel_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = player_of(world, client_id) else {
        return;
    };
    let Some(target_obj) = PacketReader::new(body).read_i32() else {
        return;
    };
    let Some(item_id) = item_id_of(world, player, target_obj) else {
        return;
    };
    let Some((option1, option2)) = world
        .objects
        .get_component::<Inventory>(&player)
        .and_then(|inv| inv.augmentation_of(target_obj))
    else {
        send_sm(
            world,
            client_id,
            sp::sm_ids::AUGMENTATION_REMOVAL_ONLY_ON_AN_AUGMENTED_ITEM,
        );
        return;
    };
    let mineral_id = world
        .objects
        .get_component::<Inventory>(&player)
        .and_then(|inv| inv.augment_mineral(target_obj))
        .unwrap_or(0);
    let Some(price) = world.data.variations.cancel_fee(item_id, mineral_id) else {
        send_sm(world, client_id, sp::sm_ids::THIS_IS_NOT_A_SUITABLE_ITEM);
        return;
    };
    send(
        world,
        client_id,
        sp::ex_put_item_result_for_variation_cancel(target_obj, item_id, option1, option2, price),
    );
}
