//! Enchant scroll flow (Java `EnchantScrolls` item handler +
//! `RequestExAddEnchantScrollItem` / `RequestExTryToPutEnchantTargetItem` /
//! `RequestExCancelEnchantItem` / `RequestEnchantItem`). Built on the
//! [`EnchantData`](crate::data::enchant_data::EnchantData) chance engine.
//!
//! Sequence: double-click a scroll → [`open`] adds an [`EnchantRequest`] and
//! sends `ChooseInventoryItem` → client picks scroll+item
//! ([`handle_add_scroll`]) → picks the target ([`handle_put_target`]) → hits
//! enchant ([`handle_enchant`]), which destroys the scroll, rolls, and applies
//! the outcome (`+1` on success, or safe-retain / blessed-reset / destroy +
//! crystallize on failure).
//!
//! Scope: support items **are** modelled — `RequestExTryToPutEnchantSupportItem`
//! / `RequestExRemoveEnchantSupportItem`, `is_support_valid`, the consume, and
//! the `bonusRate` fold into the final chance all live below. (This header said
//! otherwise for a long time; it was describing the no-support first cut.)
//!
//! Genuinely absent, and two of the three are reachable on this dist:
//! - TODO(enchant-random): `randomEnchantMin`/`Max` — a scroll enchants by a
//!   random amount in a range instead of `+1`. 20 scrolls carry it and **5 are
//!   obtainable**, one of them through a quest this port ships: Q375 Whisper of
//!   Dreams Part 2 rewards 33808 (`targetGrade="B"`, min 1 max 3), so a player
//!   who earns it today gets a flat +1 where retail rolls +1..+3.
//! - TODO(enchant-guard): the 2-second anti-autoenchant timestamp guard.
//! - The milestone announce/firework: no `announce` attribute exists anywhere in
//!   `EnchantItemData.xml` on this dist, so there is nothing to drive it.
//!
//! On-enchant armor skills are **not** an enchant gap: they belong to
//! `ArmorSetData`, which is unported in its entirety — see the note in
//! `network::user_info` at the ENCHANTLEVEL block.

use commons::network::PacketReader;

use crate::data::item_data::EtcItemType;
use crate::model::components::EnchantRequest;
use crate::model::inventory::Inventory;
use crate::network::server_packets::{self as sp, enchant_result};
use crate::world::World;

use super::helpers::{player_of, send_to_client as send};
use super::items::finish_equip_change;

/// Facts about an inventory item the enchant flow needs (item id + current
/// enchant), or `None` if the object id isn't in the player's inventory.
fn item_facts(world: &World, player: i32, object_id: i32) -> Option<(i32, i32)> {
    world
        .objects
        .get_component::<Inventory>(&player)
        .and_then(|inv| inv.items().iter().find(|it| it.object_id == object_id))
        .map(|it| (it.item_id, it.enchant_level))
}

/// `EnchantScrolls.useItem`: open the enchant window for a scroll — add the
/// request and send `ChooseInventoryItem`. Blocked if another enchant is
/// already in progress. (Called from `items::use_etc_item`.)
pub(crate) fn open(world: &mut World, client_id: u32, player: i32, scroll_object_id: i32) {
    if world.objects.has_component::<EnchantRequest>(&player) {
        return;
    }
    let Some((scroll_item_id, _)) = item_facts(world, player, scroll_object_id) else {
        return;
    };
    world.objects.add_components(
        &player,
        EnchantRequest {
            scroll_oid: scroll_object_id,
            item_oid: 0,
            support_oid: 0,
            processing: false,
        },
    );
    send(world, client_id, sp::choose_inventory_item(scroll_item_id));
}

/// `RequestExAddEnchantScrollItem` (Ex 0xE3): the client reports the scroll +
/// target it wants to enchant. Validates the scroll is a real enchant scroll
/// and acks with `ExPutEnchantScrollItemResult`.
pub(crate) fn handle_add_scroll(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = player_of(world, client_id) else {
        return;
    };
    let mut r = PacketReader::new(body);
    let (Some(scroll_oid), Some(item_oid)) = (r.read_i32(), r.read_i32()) else {
        return;
    };

    let processing = world
        .objects
        .get_component::<EnchantRequest>(&player)
        .map(|q| q.processing);
    match processing {
        Some(false) => {}
        _ => return, // no request, or one already mid-roll
    }

    let scroll_ok = item_facts(world, player, scroll_oid)
        .is_some_and(|(id, _)| world.data.enchant.scroll(id).is_some());
    let item_ok = item_facts(world, player, item_oid).is_some();
    if !scroll_ok || !item_ok {
        if let Some(q) = world.objects.get_component_mut::<EnchantRequest>(&player) {
            q.item_oid = 0;
            q.scroll_oid = 0;
        }
        send(world, client_id, sp::ex_put_enchant_scroll_item_result(0));
        return;
    }

    if let Some(q) = world.objects.get_component_mut::<EnchantRequest>(&player) {
        q.scroll_oid = scroll_oid;
        q.item_oid = item_oid;
    }
    send(
        world,
        client_id,
        sp::ex_put_enchant_scroll_item_result(scroll_oid),
    );
}

/// `RequestExTryToPutEnchantTargetItem` (Ex 0x49): the client picks the target
/// item; validate it against the scroll and ack with
/// `ExPutEnchantTargetItemResult`.
pub(crate) fn handle_put_target(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = player_of(world, client_id) else {
        return;
    };
    let Some(item_oid) = PacketReader::new(body).read_i32() else {
        return;
    };

    let Some(q) = world.objects.get_component::<EnchantRequest>(&player) else {
        return;
    };
    if q.processing || q.scroll_oid == 0 {
        return;
    }
    let scroll_oid = q.scroll_oid;

    let valid = validity(world, player, scroll_oid, item_oid);
    if !valid {
        world.objects.remove_component::<EnchantRequest>(&player);
        send(world, client_id, sp::ex_put_enchant_target_item_result(0));
        return;
    }
    if let Some(q) = world.objects.get_component_mut::<EnchantRequest>(&player) {
        q.item_oid = item_oid;
    }
    send(
        world,
        client_id,
        sp::ex_put_enchant_target_item_result(item_oid),
    );
}

/// `RequestExCancelEnchantItem` (Ex 0x4B): close the window.
pub(crate) fn handle_cancel(world: &mut World, client_id: u32) {
    let Some(player) = player_of(world, client_id) else {
        return;
    };
    world.objects.remove_component::<EnchantRequest>(&player);
}

/// `RequestExTryToPutEnchantSupportItem` (Ex 0x4A): attach a support item;
/// validate it against the scroll + target and ack with
/// `ExPutEnchantSupportItemResult` (Java `RequestExTryToPutEnchantSupportItem`).
pub(crate) fn handle_put_support(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = player_of(world, client_id) else {
        return;
    };
    let mut r = PacketReader::new(body);
    let (Some(support_oid), Some(item_oid)) = (r.read_i32(), r.read_i32()) else {
        return;
    };

    let Some(q) = world.objects.get_component::<EnchantRequest>(&player) else {
        return;
    };
    if q.processing || q.scroll_oid == 0 {
        return;
    }
    let scroll_oid = q.scroll_oid;

    if !support_valid(world, player, scroll_oid, item_oid, support_oid) {
        if let Some(q) = world.objects.get_component_mut::<EnchantRequest>(&player) {
            q.support_oid = 0;
        }
        send(world, client_id, sp::ex_put_enchant_support_item_result(0));
        return;
    }
    if let Some(q) = world.objects.get_component_mut::<EnchantRequest>(&player) {
        q.item_oid = item_oid;
        q.support_oid = support_oid;
    }
    send(
        world,
        client_id,
        sp::ex_put_enchant_support_item_result(support_oid),
    );
}

/// `RequestExRemoveEnchantSupportItem` (Ex 0xE4): clear the support.
pub(crate) fn handle_remove_support(world: &mut World, client_id: u32) {
    let Some(player) = player_of(world, client_id) else {
        return;
    };
    if let Some(q) = world.objects.get_component_mut::<EnchantRequest>(&player) {
        q.support_oid = 0;
    }
    send(
        world,
        client_id,
        sp::ex_remove_enchant_support_item_result(),
    );
}

/// Whether the support at `support_oid` is compatible with the scroll + target.
fn support_valid(
    world: &World,
    player: i32,
    scroll_oid: i32,
    item_oid: i32,
    support_oid: i32,
) -> bool {
    let (Some((scroll_item_id, _)), Some((item_id, enchant)), Some((support_item_id, _))) = (
        item_facts(world, player, scroll_oid),
        item_facts(world, player, item_oid),
        item_facts(world, player, support_oid),
    ) else {
        return false;
    };
    let (Some(scroll_tpl), Some(target), Some(support_tpl), Some(support)) = (
        world.data.item_data.get(scroll_item_id),
        world.data.item_data.get(item_id),
        world.data.item_data.get(support_item_id),
        world.data.enchant.support(support_item_id),
    ) else {
        return false;
    };
    let s = scroll_tpl.etc_item_type;
    let sup = support_tpl.etc_item_type;
    world.data.enchant.is_support_valid(
        (
            s.is_enchant_weapon(),
            s.is_blessed(),
            s.is_blessed_down(),
            s.is_giant(),
        ),
        support,
        (
            sup.support_is_weapon(),
            sup.support_is_blessed(),
            sup.support_is_giant(),
        ),
        target,
        enchant,
    )
}

/// Whether the scroll at `scroll_oid` can enchant the item at `item_oid`
/// (resolves both templates, then defers to `EnchantData::is_target_valid`).
fn validity(world: &World, player: i32, scroll_oid: i32, item_oid: i32) -> bool {
    let Some((scroll_item_id, _)) = item_facts(world, player, scroll_oid) else {
        return false;
    };
    let Some((item_id, enchant)) = item_facts(world, player, item_oid) else {
        return false;
    };
    let Some(scroll) = world.data.enchant.scroll(scroll_item_id) else {
        return false;
    };
    let Some(scroll_tpl) = world.data.item_data.get(scroll_item_id) else {
        return false;
    };
    let Some(target) = world.data.item_data.get(item_id) else {
        return false;
    };
    world.data.enchant.is_target_valid(
        scroll,
        scroll_tpl.etc_item_type.is_enchant_weapon(),
        target,
        enchant,
    )
}

/// `RequestEnchantItem` (0x5F): consume the scroll, roll, and apply the result.
pub(crate) fn handle_enchant(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = player_of(world, client_id) else {
        return;
    };
    let mut r = PacketReader::new(body);
    let Some(item_oid) = r.read_i32() else { return };
    let support_id = r.read_i32().unwrap_or(0);

    // Must have a non-processing request; mark it processing.
    match world
        .objects
        .get_component::<EnchantRequest>(&player)
        .map(|q| q.processing)
    {
        Some(false) => {}
        _ => return,
    }
    let (scroll_oid, support_oid) = world
        .objects
        .get_component::<EnchantRequest>(&player)
        .map(|q| (q.scroll_oid, q.support_oid))
        .unwrap_or((0, 0));
    if let Some(q) = world.objects.get_component_mut::<EnchantRequest>(&player) {
        q.item_oid = item_oid;
        q.processing = true;
    }

    let err = |world: &mut World| {
        world.objects.remove_component::<EnchantRequest>(&player);
        send(
            world,
            client_id,
            enchant_result(sp::enchant_result::ERROR, 0, 0, 0),
        );
    };

    // Resolve scroll + target templates.
    let (Some((scroll_item_id, _)), Some((item_id, current))) = (
        item_facts(world, player, scroll_oid),
        item_facts(world, player, item_oid),
    ) else {
        world.objects.remove_component::<EnchantRequest>(&player);
        return;
    };
    let Some(scroll) = world.data.enchant.scroll(scroll_item_id).cloned() else {
        world.objects.remove_component::<EnchantRequest>(&player);
        return;
    };
    let Some(scroll_tpl) = world.data.item_data.get(scroll_item_id).cloned() else {
        return;
    };
    let Some(target_tpl) = world.data.item_data.get(item_id).cloned() else {
        return;
    };
    let etc = scroll_tpl.etc_item_type;

    if !world
        .data
        .enchant
        .is_target_valid(&scroll, etc.is_enchant_weapon(), &target_tpl, current)
    {
        err(world);
        return;
    }

    // Optional support item (Java `RequestEnchantItem` support branch): must
    // match the stored request, be a real support, and pass `isValid` with the
    // scroll. `None` = no support (the common case).
    if support_oid != 0 && support_id != support_oid {
        err(world);
        return;
    }
    let support = match support_oid {
        0 => None,
        oid => {
            let Some((sid, _)) = item_facts(world, player, oid) else {
                err(world);
                return;
            };
            let Some(sup) = world.data.enchant.support(sid).copied() else {
                err(world);
                return;
            };
            let sup_etc = world
                .data
                .item_data
                .get(sid)
                .map(|t| t.etc_item_type)
                .unwrap_or_default();
            let scroll_flags = (
                etc.is_enchant_weapon(),
                etc.is_blessed(),
                etc.is_blessed_down(),
                etc.is_giant(),
            );
            let support_flags = (
                sup_etc.support_is_weapon(),
                sup_etc.support_is_blessed(),
                sup_etc.support_is_giant(),
            );
            if !world.data.enchant.is_support_valid(
                scroll_flags,
                &sup,
                support_flags,
                &target_tpl,
                current,
            ) {
                err(world);
                return;
            }
            Some(sup)
        }
    };

    // Consume one scroll (Java destroyItem). If it's gone, error out — and
    // punish: the client can't press Enchant without the scroll in the bag.
    let removed = world
        .objects
        .get_component_mut::<Inventory>(&player)
        .and_then(|inv| inv.remove_by_object_id(scroll_oid, 1))
        .is_some();
    if !removed {
        let punish = world.cfg.general.default_punish;
        super::punishment::handle_illegal_player_action(
            world,
            player,
            &format!("Player {player} tried to enchant with a scroll he doesn't have"),
            punish,
        );
        err(world);
        return;
    }
    // Consume the support item too, if present; same reasoning as the scroll.
    if support.is_some() {
        let removed = world
            .objects
            .get_component_mut::<Inventory>(&player)
            .and_then(|inv| inv.remove_by_object_id(support_oid, 1))
            .is_some();
        if !removed {
            let punish = world.cfg.general.default_punish;
            super::punishment::handle_illegal_player_action(
                world,
                player,
                &format!("Player {player} tried to enchant with a support item he doesn't have"),
                punish,
            );
            err(world);
            return;
        }
    }

    // Roll. `chance_no_bonus` is the group chance with the safe-enchant
    // short-circuit but without the flat bonus (Java `getChance`, used both as
    // the success-increment guard and the base of `finalChance`).
    let chance_no_bonus = world.data.enchant.base_chance(
        &target_tpl,
        target_tpl.is_magic_weapon,
        scroll.scroll_group_id,
        current,
        scroll.safe_enchant,
        0.0,
    );
    if chance_no_bonus < 0.0 {
        err(world);
        return;
    }
    let support_bonus = support.map(|s| s.bonus_rate).unwrap_or(0.0);
    let final_chance = (chance_no_bonus + scroll.bonus_rate + support_bonus).min(100.0);
    let success = world.roll_f64() * 100.0 < final_chance;

    // Java `RequestEnchantItem` `Config.LOG_ITEM_ENCHANTS` — but recorded once
    // here, at the decision, rather than duplicated across Java's six
    // success/fail/safe/blessed branches. The outcome is a field, so the record
    // carries the same information without six sites that can drift apart.
    if world.cfg.general.log_item_enchants {
        let char_name = world
            .objects
            .get_component::<crate::model::Player>(&player)
            .map(|p| p.name.clone());
        commons::audit::record(
            commons::audit::Category::Enchant,
            serde_json::json!({
                "kind": "item",
                "result": if success { "success" } else { "fail" },
                "char_name": char_name,
                "oid": player,
                "item_oid": item_oid,
                "enchant_from": current,
                "chance": final_chance,
                "scroll_id": scroll.id,
                "support_id": support.as_ref().map(|s| s.id),
            }),
        );
    }

    if success {
        // Success step: a support widens it (its `randomEnchant` range, capped
        // at the support's max), else the scroll's default +1.
        let (cap, step) = match &support {
            Some(s) => {
                let step = if s.random_max > s.random_min {
                    s.random_min + world.roll(s.random_max - s.random_min + 1)
                } else {
                    s.random_min
                };
                (s.max_enchant, step)
            }
            None => (scroll.max_enchant, 1),
        };
        apply_success(
            world,
            client_id,
            player,
            item_oid,
            current,
            chance_no_bonus,
            step,
            cap,
        );
    } else {
        apply_failure(
            world,
            client_id,
            player,
            item_oid,
            current,
            etc,
            &target_tpl,
        );
    }

    // The request persists until the client cancels the window (Java only
    // clears `_isProcessing`); refresh the item list.
    if let Some(q) = world.objects.get_component_mut::<EnchantRequest>(&player) {
        q.processing = false;
    }
    refresh_items(world, client_id, player);
}

/// Success: raise the enchant by `step` (Java's `Rnd.get(randomMin, randomMax)`,
/// default 1; a support widens it) capped at `cap`, then refresh. The guard on
/// `chance_no_bonus > 0` matches Java (a 0%-group enchant can't step up).
#[allow(clippy::too_many_arguments)]
fn apply_success(
    world: &mut World,
    client_id: u32,
    player: i32,
    item_oid: i32,
    current: i32,
    chance_no_bonus: f64,
    step: i32,
    cap: i32,
) {
    let new_level = if chance_no_bonus > 0.0 {
        (current + step).min(cap)
    } else {
        current
    };
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player) {
        inv.set_item_enchant(item_oid, new_level);
    }
    send(
        world,
        client_id,
        enchant_result(sp::enchant_result::SUCCESS, 0, 0, new_level),
    );
    // If the item is equipped, its enchant glow + stat bonus must refresh.
    if world
        .objects
        .get_component::<Inventory>(&player)
        .is_some_and(|inv| inv.paperdoll_slot_of(item_oid).is_some())
    {
        finish_equip_change(world, client_id, player, &[item_oid]);
    }
}

/// Failure: safe-retain, blessed-reset, or destroy + crystallize (Java's three
/// failure branches).
fn apply_failure(
    world: &mut World,
    client_id: u32,
    player: i32,
    item_oid: i32,
    current: i32,
    etc: EtcItemType,
    target_tpl: &crate::data::item_data::ItemTemplate,
) {
    // Safe scroll: level retained, nothing lost.
    if etc.is_safe() {
        send(
            world,
            client_id,
            enchant_result(sp::enchant_result::SAFE_FAIL, 0, 0, current),
        );
        return;
    }

    // An equipped item is unequipped on a non-safe failure.
    if world
        .objects
        .get_component::<Inventory>(&player)
        .is_some_and(|inv| inv.paperdoll_slot_of(item_oid).is_some())
    {
        let changed = world
            .objects
            .get_component_mut::<Inventory>(&player)
            .map(|inv| inv.unequip_item(item_oid))
            .unwrap_or_default();
        finish_equip_change(world, client_id, player, &changed);
    }

    // Blessed / blessed-down: the item survives; blessed resets to 0,
    // blessed-down drops by 1.
    if etc.is_blessed() || etc.is_blessed_down() {
        let new_level = if etc.is_blessed_down() {
            (current - 1).max(0)
        } else {
            0
        };
        if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player) {
            inv.set_item_enchant(item_oid, new_level);
        }
        send(
            world,
            client_id,
            enchant_result(sp::enchant_result::BLESSED_FAIL, 0, 0, 0),
        );
        return;
    }

    // Normal scroll: the item is destroyed and partly crystallized.
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player) {
        inv.remove_by_object_id(item_oid, 1);
    }
    let crystal_count = target_tpl.crystal_count;
    let count = if crystal_count > 0 {
        (crystal_count - (crystal_count + 1) / 2).max(0) as i64
    } else {
        0
    };
    let crystal_id = target_tpl.crystal_type.crystal_item_id();
    match (crystal_id, count) {
        (Some(cid), n) if n > 0 => {
            if let Some(new_oid) = world.alloc_object_id()
                && let Some(inv) = world.objects.get_component_mut::<Inventory>(&player)
            {
                inv.add_item(&world.data.item_data, new_oid, cid, n);
            }
            send(
                world,
                client_id,
                enchant_result(sp::enchant_result::FAIL, cid, n, 0),
            );
        }
        _ => send(
            world,
            client_id,
            enchant_result(sp::enchant_result::NO_CRYSTAL, 0, 0, 0),
        ),
    }
}

fn refresh_items(world: &World, client_id: u32, player: i32) {
    if let Some(inv) = world.objects.get_component::<Inventory>(&player) {
        send(
            world,
            client_id,
            crate::network::enter_world::item_list(inv, &world.data, false),
        );
    }
}
