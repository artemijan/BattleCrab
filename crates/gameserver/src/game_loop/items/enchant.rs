//! Enchant scroll flow (Java `EnchantScrolls` item handler +
//! `RequestExAddEnchantScrollItem` / `RequestExTryToPutEnchantTargetItem` /
//! `RequestExCancelEnchantItem` / `RequestEnchantItem`). Built on the
//! [`EnchantData`](crate::data::enchant_data::EnchantData) chance engine.
//!
//! Sequence: double-click a scroll → [`open`] adds an [`EnchantRequest`] and
//! sends `ChooseInventoryItem` → client picks scroll+item
//! ([`handle_add_scroll`]) → picks the target ([`handle_put_target`]) → hits
//! enchant ([`handle_enchant`]), which destroys the scroll, rolls, and applies
//! the outcome (a rolled step on success, or safe-retain / blessed-reset /
//! destroy + crystallize on failure).
//!
//! Scope: support items **are** modelled — `RequestExTryToPutEnchantSupportItem`
//! / `RequestExRemoveEnchantSupportItem`, `is_support_valid`, the consume, and
//! the `bonusRate` fold into the final chance all live below. (This header said
//! otherwise for a long time; it was describing the no-support first cut.)
//!
//! The success **step** is likewise `randomEnchantMin`/`Max` on both sides now
//! — see [`roll_enchant_step`]. Most scrolls omit the attributes and Java's
//! defaults (min 1, max = min) make those a plain `+1`, which is why the
//! hard-coded `+1` looked right for so long.
//!
//! The 2-second anti-autoenchant guard is in `handle_enchant`, between the
//! validation and the destroys, where Java puts it.
//!
//! Genuinely absent:
//! - The milestone announce/firework: no `announce` attribute exists anywhere in
//!   `EnchantItemData.xml` on this dist, so there is nothing to drive it.
//!
//! On-enchant armor skills were never an enchant gap: they belong to
//! `ArmorSetData` (`game_loop::armor_sets`).

use commons::network::PacketReader;

use crate::data::item_data::kinds::EtcItemType;
use crate::game_loop::character::inventory;
use crate::model::components::EnchantRequest;
use crate::model::inventory::Inventory;
use crate::network::server_packets::{self as sp, enchant_result};
use crate::world::World;

use crate::game_loop::helpers::{player_of, send_to_client as send};
use crate::game_loop::items::{finish_equip_change, unequip_if_worn};
use crate::game_loop::moderation::punishment;
/// Facts about an inventory item the enchant flow needs (item id + current
/// enchant), or `None` if the object id isn't in the player's inventory.
fn item_facts(world: &World, player: i32, object_id: i32) -> Option<(i32, i32)> {
    world
        .objects
        .get_component::<Inventory>(&player)
        .and_then(|inv| inv.by_object_id(object_id))
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
            // Java `AbstractRequest._timestamp` starts at 0 and only the four
            // window packets move it; opening the window is not an interaction.
            stamped_tick: None,
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

    let tick = world.tick;
    if let Some(q) = world.objects.get_component_mut::<EnchantRequest>(&player) {
        q.scroll_oid = scroll_oid;
        q.item_oid = item_oid;
        q.stamped_tick = Some(tick);
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
    let tick = world.tick;
    if let Some(q) = world.objects.get_component_mut::<EnchantRequest>(&player) {
        q.item_oid = item_oid;
        q.stamped_tick = Some(tick);
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
    let tick = world.tick;
    if let Some(q) = world.objects.get_component_mut::<EnchantRequest>(&player) {
        q.item_oid = item_oid;
        q.support_oid = support_oid;
        q.stamped_tick = Some(tick);
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
    let tick = world.tick;
    if let Some(q) = world.objects.get_component_mut::<EnchantRequest>(&player) {
        q.support_oid = 0;
        q.stamped_tick = Some(tick);
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
        target_gates(world, item_id),
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
        target_gates(world, item_id),
    )
}

/// The two `Character.ini` gates the data layer cannot reach, resolved for one
/// item: `EnchantBlackList` membership and `DisableOverEnchanting`.
fn target_gates(world: &World, item_id: i32) -> crate::data::enchant_data::TargetGates {
    crate::data::enchant_data::TargetGates {
        blacklisted: world
            .cfg
            .character
            .enchant_black_list
            .binary_search(&item_id)
            .is_ok(),
        disable_over_enchanting: world.cfg.character.disable_over_enchanting,
    }
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
            enchant_result(enchant_result::ERROR, 0, 0, 0),
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

    if !world.data.enchant.is_target_valid(
        &scroll,
        etc.is_enchant_weapon(),
        &target_tpl,
        current,
        target_gates(world, target_tpl.item_id),
    ) {
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
                target_gates(world, target_tpl.item_id),
            ) {
                err(world);
                return;
            }
            Some(sup)
        }
    };

    // Java's "fast auto-enchant cheat check", between the validation above and
    // the destroys below:
    //
    //     if ((request.getTimestamp() == 0)
    //         || ((System.currentTimeMillis() - request.getTimestamp()) < 2000))
    //
    // `_timestamp` is the moment of the **last window interaction** — the four
    // `RequestEx*Enchant*` packets each stamp it on their success path. A human
    // necessarily spends a moment picking the scroll and target; a script does
    // not.
    //
    // Java's first leg (`getTimestamp() == 0`) is **dead** in both engines and
    // kept only as a belt: the target item is resolved a few lines above, and
    // every packet that can set it also stamps, so a request that reaches here
    // has always been stamped. `None` is still not folded into "too fast",
    // because the two mean different things if that ever stops holding.
    //
    // Java measures wall-clock ms. This measures ticks (100 ms each, so 2 s is
    // 20 of them), which is the same threshold with a coarser grain and — the
    // reason to prefer it — no dependence on the host clock, so the test is not
    // one of this suite's wall-clock flakes.
    const AUTOENCHANT_MIN_TICKS: u64 = 20;
    let stamped = world
        .objects
        .get_component::<EnchantRequest>(&player)
        .and_then(|q| q.stamped_tick);
    if stamped.is_none_or(|t| world.tick.saturating_sub(t) < AUTOENCHANT_MIN_TICKS) {
        let punish = world.cfg.general.default_punish;
        punishment::handle_illegal_player_action(
            world,
            player,
            &format!("Player {player} use autoenchant program "),
            punish,
        );
        // Java drops the request here, unlike the plain validation failures
        // above which leave the window open.
        world.objects.remove_component::<EnchantRequest>(&player);
        send(
            world,
            client_id,
            enchant_result(enchant_result::ERROR, 0, 0, 0),
        );
        return;
    }

    // Consume one scroll (Java destroyItem). If it's gone, error out — and
    // punish: the client can't press Enchant without the scroll in the bag.
    let removed = inventory::remove_inventory_item_change(world, player, scroll_oid, 1).is_some();
    if !removed {
        punishment::illegal_action(
            world,
            player,
            &format!("Player {player} tried to enchant with a scroll he doesn't have"),
        );
        err(world);
        return;
    }
    // Consume the support item too, if present; same reasoning as the scroll.
    if support.is_some() {
        let removed =
            inventory::remove_inventory_item_change(world, player, support_oid, 1).is_some();
        if !removed {
            punishment::illegal_action(
                world,
                player,
                &format!("Player {player} tried to enchant with a support item he doesn't have"),
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
        // Success step. Java `RequestEnchantItem`'s SUCCESS arm rolls
        // `Rnd.get(randomEnchantMin, randomEnchantMax)` — inclusive both ends —
        // and caps at that template's `maxEnchant`. **The support, when present,
        // supplies both the range and the cap**; the scroll supplies them
        // otherwise. The port had the support half and hard-coded the scroll
        // half to `+1`, which is right for every scroll that omits the
        // attributes (min defaults to 1, max to min) and wrong for the 20 that
        // carry them — of which 5 are obtainable here, one through a quest this
        // port ships: Q375 rewards 33808, `randomEnchantMin=1 max=3`.
        let (cap, min, max) = match &support {
            Some(s) => (s.max_enchant, s.random_min, s.random_max),
            None => (scroll.max_enchant, scroll.random_min, scroll.random_max),
        };
        let step = roll_enchant_step(world, min, max);
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
    inventory::send_inventory_item_list(world, player);
}

/// Success: raise the enchant by `step` (Java's `Rnd.get(randomMin, randomMax)`,
/// default 1; a support widens it) capped at `cap`, then refresh. The guard on
/// `chance_no_bonus > 0` matches Java (a 0%-group enchant can't step up).
#[allow(clippy::too_many_arguments)]
/// Java `Rnd.get(origin, bound)` over an enchant template's random range:
/// **inclusive at both ends**, and `origin` itself when `origin >= bound`
/// (Java returns early rather than calling `nextInt` with an empty span).
fn roll_enchant_step(world: &mut World, min: i32, max: i32) -> i32 {
    if max > min {
        min + world.roll(max - min + 1)
    } else {
        min
    }
}

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
        enchant_result(enchant_result::SUCCESS, 0, 0, new_level),
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
    target_tpl: &crate::data::item_data::template::ItemTemplate,
) {
    // Safe scroll: level retained, nothing lost.
    if etc.is_safe() {
        send(
            world,
            client_id,
            enchant_result(enchant_result::SAFE_FAIL, 0, 0, current),
        );
        return;
    }

    // An equipped item is unequipped on a non-safe failure.
    unequip_if_worn(world, client_id, player, item_oid);

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
            enchant_result(enchant_result::BLESSED_FAIL, 0, 0, 0),
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
                enchant_result(enchant_result::FAIL, cid, n, 0),
            );
        }
        _ => send(
            world,
            client_id,
            enchant_result(enchant_result::NO_CRYSTAL, 0, 0, 0),
        ),
    }
}

// ---------------------------------------------------------------------------
// Over-enchant protection (`Character.ini`'s `OverEnchantProtection` /
// `OverEnchantPunishment`)
// ---------------------------------------------------------------------------

/// `EnterWorld`'s over-enchant sweep: destroy every equipable item enchanted
/// past its category's ceiling, then punish the owner once if anything went.
///
/// Java runs the same sweep from `UseItem` as well; the port hooks only the
/// login path, because the only way an item's enchant level changes in between
/// is this module — which cannot exceed the ceiling it just checked. An item
/// that arrives over the line got there out-of-band (a restored row, a GM
/// `//enchant`), and login is where that shows up.
///
/// GMs are exempt (`&& !player.isGM()`), which is what makes `//enchant`
/// testable at all.
pub(crate) fn over_enchant_sweep(world: &mut World, player: i32) {
    if !world.cfg.character.over_enchant_protection {
        return;
    }
    if world
        .objects
        .get_component::<crate::model::Player>(&player)
        .is_some_and(|p| p.is_gm(&world.data))
    {
        return;
    }
    // Collect first: the destroy path takes `&mut world`. Keyed on the
    // **object id**, because the rule is about one over-enchanted instance —
    // destroying by item id would also take a plain duplicate of the same id,
    // which is why Java passes the `Item` rather than its template id.
    let offenders: Vec<(i32, i32, i32, i32, i64)> = {
        let Some(inv) = world.objects.get_component::<Inventory>(&player) else {
            return;
        };
        inv.items()
            .iter()
            .filter_map(|it| {
                let t = world.data.item_data.get(it.item_id)?;
                if !t.is_equipable() {
                    return None;
                }
                let ceiling = world.data.enchant.max_enchant_for_type2(t.type2)?;
                (it.enchant_level > ceiling).then_some((
                    it.object_id,
                    it.item_id,
                    it.enchant_level,
                    ceiling,
                    it.count,
                ))
            })
            .collect()
    };
    if offenders.is_empty() {
        return;
    }
    for (object_id, item_id, enchant, ceiling, count) in &offenders {
        tracing::info!(
            "Over-enchanted (+{enchant}) item {item_id} (ceiling {ceiling}) removed from player {player}"
        );
        crate::game_loop::items::destroy_item_by_object_id(world, player, *object_id, *count);
    }
    let punishment = world.cfg.character.over_enchant_punishment;
    if punishment != crate::model::punishment::IllegalActionPunishment::None {
        crate::game_loop::moderation::punishment::handle_illegal_player_action(
            world,
            player,
            "has over-enchanted items.",
            punishment,
        );
    }
}
