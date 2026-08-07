//! Henna / dye symbols (G16) — port of the `RequestHenna*` packet family +
//! `Player.addHenna`/`removeHenna`/`getHennaEmptySlots` + the SymbolMaker NPC
//! bypass. A worn dye's six base-stat bonuses are folded into `BaseStats` (see
//! `Player::from_char`), so drawing/removing one recomputes `BaseStats =
//! template + henna` and re-runs the stat finalizers.
//!
//! Interlude hennas are permanent (`duration = -1`), so the timed-henna
//! scheduler + `HennaDuration` character variables are out of scope; dye
//! `<skill>` grants (none on Interlude dyes) are likewise skipped.

use super::helpers::adena;
use super::helpers::{player_of, send_to_client as send};
use crate::data::henna_data::HennaStatSums;
use crate::model::Player;
use crate::model::components::{BaseStats, CombatStats, HennaSlots, Speeds, StatModifiers, Vitals};
use crate::model::inventory::Inventory;
use crate::model::stats::BaseStat;
use crate::network::server_packets::{self as sp, HennaStatWire, SmParam, StatPreview, sm_ids};
use crate::world::World;

const ADENA_ID: i32 = 57;

/// `ClassId.level()` — occupation tier (0 base, 1/2/3 for 1st/2nd/3rd class),
/// mapped from the `*_CLASS_GROUP` category the class belongs to.
fn class_level(world: &World, class_id: i32) -> i32 {
    let c = &world.data.categories;
    if c.contains("FOURTH_CLASS_GROUP", class_id) {
        3
    } else if c.contains("THIRD_CLASS_GROUP", class_id) {
        2
    } else if c.contains("SECOND_CLASS_GROUP", class_id) {
        1
    } else {
        0
    }
}

/// `Player.getHennaEmptySlots`: 2 slots at class level 1, 3 at level ≥ 2, 0 at
/// base class — minus the worn dyes.
fn empty_slots(world: &World, oid: i32) -> i32 {
    let Some(class_id) = world
        .objects
        .get_component::<Player>(&oid)
        .map(|p| p.class_id)
    else {
        return 0;
    };
    let total = match class_level(world, class_id) {
        1 => 2,
        n if n > 1 => 3,
        _ => 0,
    };
    let worn = world
        .objects
        .get_component::<HennaSlots>(&oid)
        .map(|h| h.worn() as i32)
        .unwrap_or(0);
    (total - worn).max(0)
}

fn class_id_of(world: &World, oid: i32) -> i32 {
    world
        .objects
        .get_component::<Player>(&oid)
        .map(|p| p.class_id)
        .unwrap_or(0)
}

fn worn_sums(world: &World, oid: i32) -> HennaStatSums {
    let slots = world
        .objects
        .get_component::<HennaSlots>(&oid)
        .map(|h| h.0)
        .unwrap_or_default();
    world.data.hennas.stat_sums(&slots)
}

// --- windows (SymbolMaker "Draw"/"Remove") --------------------------------

/// `RequestHennaItemList` / SymbolMaker "Draw" → `HennaEquipList`: the dyes the
/// player's class may wear and currently holds the item for.
pub(crate) fn handle_item_list(world: &mut World, client_id: u32) {
    let Some(oid) = player_of(world, client_id) else {
        return;
    };
    let class_id = class_id_of(world, oid);
    let lines: Vec<sp::HennaLine> = world
        .data
        .hennas
        .list_for_class(class_id)
        .iter()
        .filter(|h| {
            world
                .objects
                .get_component::<Inventory>(&oid)
                .is_some_and(|inv| inv.count_of(h.dye_item_id) > 0)
        })
        .map(|h| (h.dye_id, h.dye_item_id, h.wear_count, h.wear_fee, true))
        .collect();
    send(
        world,
        client_id,
        sp::henna_equip_list(adena(world, oid), &lines),
    );
}

/// `RequestHennaRemoveList` / SymbolMaker "Remove" → `HennaRemoveList`: the
/// worn dyes.
pub(crate) fn handle_remove_list(world: &mut World, client_id: u32) {
    let Some(oid) = player_of(world, client_id) else {
        return;
    };
    let class_id = class_id_of(world, oid);
    let worn = world
        .objects
        .get_component::<HennaSlots>(&oid)
        .map(|h| h.0)
        .unwrap_or_default();
    let lines: Vec<sp::HennaLine> = worn
        .iter()
        .filter_map(|d| *d)
        .filter_map(|dye_id| world.data.hennas.get(dye_id))
        .map(|h| {
            (
                h.dye_id,
                h.dye_item_id,
                h.cancel_count,
                h.cancel_fee,
                h.is_allowed_class(class_id),
            )
        })
        .collect();
    send(
        world,
        client_id,
        sp::henna_remove_list(adena(world, oid), lines.len() as i32, &lines),
    );
}

// --- per-dye previews -----------------------------------------------------

/// `RequestHennaItemInfo` → `HennaItemDrawInfo`: the current vs. after-adding
/// stat columns. Current stats read from `BaseStats` (which already includes
/// worn henna); the preview adds this candidate dye's bonus.
pub(crate) fn handle_item_info(world: &mut World, client_id: u32, symbol_id: i32) {
    let Some(oid) = player_of(world, client_id) else {
        return;
    };
    let Some(henna) = world.data.hennas.get(symbol_id).cloned() else {
        return;
    };
    let class_id = class_id_of(world, oid);
    let Some(base) = world.objects.get_component::<BaseStats>(&oid).copied() else {
        return;
    };
    let stats = stat_preview(&base, |s| current(&base, s) + henna.base_stat(s));
    send(
        world,
        client_id,
        sp::henna_item_draw_info(
            henna.dye_id,
            henna.dye_item_id,
            henna.wear_count,
            henna.wear_fee,
            henna.is_allowed_class(class_id),
            adena(world, oid),
            &stats,
        ),
    );
}

/// `RequestHennaItemRemoveInfo` → `HennaItemRemoveInfo`: current vs. after-removing.
pub(crate) fn handle_item_remove_info(world: &mut World, client_id: u32, symbol_id: i32) {
    let Some(oid) = player_of(world, client_id) else {
        return;
    };
    if symbol_id == 0 {
        return;
    }
    let Some(henna) = world.data.hennas.get(symbol_id).cloned() else {
        return;
    };
    let class_id = class_id_of(world, oid);
    let Some(base) = world.objects.get_component::<BaseStats>(&oid).copied() else {
        return;
    };
    let stats = stat_preview(&base, |s| current(&base, s) - henna.base_stat(s));
    send(
        world,
        client_id,
        sp::henna_item_remove_info(
            henna.dye_id,
            henna.dye_item_id,
            henna.cancel_count,
            henna.cancel_fee,
            henna.is_allowed_class(class_id),
            adena(world, oid),
            &stats,
        ),
    );
}

// --- draw / remove --------------------------------------------------------

/// `RequestHennaEquip`: draw the dye onto the first empty slot (class / count /
/// adena / free-slot gates), consuming the dyes + fee.
pub(crate) fn handle_equip(world: &mut World, client_id: u32, symbol_id: i32) {
    let Some(oid) = player_of(world, client_id) else {
        return;
    };
    if empty_slots(world, oid) == 0 {
        send(
            world,
            client_id,
            sp::system_message_with(sm_ids::NO_SLOT_EXISTS_TO_DRAW_THE_SYMBOL, &[]),
        );
        return;
    }
    let Some(henna) = world.data.hennas.get(symbol_id).cloned() else {
        return;
    };
    let class_id = class_id_of(world, oid);
    let count = world
        .objects
        .get_component::<Inventory>(&oid)
        .map(|i| i.count_of(henna.dye_item_id))
        .unwrap_or(0);
    let class_allowed = henna.is_allowed_class(class_id);
    let ok = class_allowed && count >= henna.wear_count && adena(world, oid) >= henna.wear_fee;
    if !ok {
        send(
            world,
            client_id,
            sp::system_message_with(sm_ids::THE_SYMBOL_CANNOT_BE_DRAWN, &[]),
        );
        // Java: a dye the class can't wear at all is an exploit attempt (the
        // client never offers it), on top of the cannot-draw notice.
        if !class_allowed {
            let punish = world.cfg.general.default_punish;
            super::punishment::handle_illegal_player_action(
                world,
                oid,
                &format!("Exploit attempt: player {oid} tryed to add a forbidden henna."),
                punish,
            );
        }
        return;
    }

    // Assign the first empty slot.
    let Some(slot) = world
        .objects
        .get_component_mut::<HennaSlots>(&oid)
        .and_then(|h| {
            let idx = h.0.iter().position(|s| s.is_none())?;
            h.0[idx] = Some(henna.dye_id);
            Some(idx)
        })
    else {
        return;
    };
    let _ = slot;

    // Consume dyes + adena.
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&oid) {
        inv.remove_item(henna.dye_item_id, henna.wear_count);
        inv.remove_item(ADENA_ID, henna.wear_fee);
    }

    apply_henna_change(world, client_id, oid);
    refresh_inventory(world, client_id, oid);
    send(
        world,
        client_id,
        sp::henna_equip_list(adena(world, oid), &equip_lines(world, oid)),
    );
    send(
        world,
        client_id,
        sp::system_message_with(sm_ids::THE_SYMBOL_HAS_BEEN_ADDED, &[]),
    );
}

/// `RequestHennaRemove`: erase the worn dye with this id (adena-fee gated),
/// refunding its cancel count of dyes.
pub(crate) fn handle_remove(world: &mut World, client_id: u32, symbol_id: i32) {
    let Some(oid) = player_of(world, client_id) else {
        return;
    };
    // Find the slot holding this dye.
    let slot = world
        .objects
        .get_component::<HennaSlots>(&oid)
        .and_then(|h| h.0.iter().position(|s| *s == Some(symbol_id)));
    let Some(slot) = slot else { return };
    let Some(henna) = world.data.hennas.get(symbol_id).cloned() else {
        return;
    };

    if adena(world, oid) < henna.cancel_fee {
        send(
            world,
            client_id,
            sp::system_message_with(sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA, &[]),
        );
        return;
    }

    // Clear the slot.
    if let Some(h) = world.objects.get_component_mut::<HennaSlots>(&oid) {
        h.0[slot] = None;
    }
    // Charge the cancel fee, refund the cancel count of dyes.
    if henna.cancel_fee > 0
        && let Some(inv) = world.objects.get_component_mut::<Inventory>(&oid)
    {
        inv.remove_item(ADENA_ID, henna.cancel_fee);
    }
    if henna.cancel_count > 0 {
        super::items::add_inventory_item(world, oid, henna.dye_item_id, henna.cancel_count);
        send(
            world,
            client_id,
            sp::system_message_with(
                sm_ids::YOU_HAVE_EARNED_S2_S1_S,
                &[
                    SmParam::ItemName(henna.dye_item_id),
                    SmParam::Long(henna.cancel_count),
                ],
            ),
        );
    }

    apply_henna_change(world, client_id, oid);
    refresh_inventory(world, client_id, oid);
    send(
        world,
        client_id,
        sp::system_message_with(sm_ids::THE_SYMBOL_HAS_BEEN_DELETED, &[]),
    );
}

// --- shared -----------------------------------------------------------------

/// Build the `HennaInfo` payload from a slot set. Split out from
/// [`send_henna_info`] so the enter-world burst can send the *real* panel
/// straight from the `Entering` bundle, before the player is in the world store
/// (Java sends `HennaInfo` inside the burst, ahead of the welcome message).
pub(crate) fn henna_info_packet(
    data: &crate::data::GameData,
    class_id: i32,
    slots: &HennaSlots,
) -> Vec<u8> {
    let sums = data.hennas.stat_sums(&slots.0);
    let wire = HennaStatWire {
        int_: sums.int_ as i16,
        str_: sums.str_ as i16,
        con: sums.con as i16,
        men: sums.men as i16,
        dex: sums.dex as i16,
        wit: sums.wit as i16,
    };
    let dyes: Vec<(i32, bool)> = slots
        .dye_ids()
        .map(|id| {
            (
                id,
                data.hennas
                    .get(id)
                    .is_some_and(|hn| hn.is_allowed_class(class_id)),
            )
        })
        .collect();
    sp::henna_info(wire, slots.worn() as i32, &dyes)
}

/// `HennaInfo` — the worn-dye panel. Sent after any dye change (the
/// enter-world copy goes out through [`henna_info_packet`]).
pub(crate) fn send_henna_info(world: &World, client_id: u32, oid: i32) {
    let class_id = class_id_of(world, oid);
    let slots = world
        .objects
        .get_component::<HennaSlots>(&oid)
        .cloned()
        .unwrap_or_default();
    let pkt = henna_info_packet(&world.data, class_id, &slots);
    send(world, client_id, pkt);
}

/// Recompute `BaseStats = template + worn-henna sums`, re-run the finalizers +
/// max-HP/MP, then push `UserInfo` + `HennaInfo` to the owner (Java `addHenna`/
/// `removeHenna`'s `recalcHennaStats` + `broadcastUserInfo(BASE_STATS, …)`).
pub(crate) fn apply_henna_change(world: &mut World, client_id: u32, oid: i32) {
    let (class_id, base_class_id) = world
        .objects
        .get_component::<Player>(&oid)
        .map(|p| (p.class_id, p.base_class_id))
        .unwrap_or((0, 0));
    let t = world
        .data
        .player_templates
        .get(class_id)
        .or_else(|| world.data.player_templates.get(base_class_id))
        .cloned()
        .unwrap_or_default();
    let sums = worn_sums(world, oid);

    if let Some((player, mut base, mods, inventory, mut vitals, mut speeds, mut combat)) =
        world.objects.get_many_mut::<(
            &Player,
            &mut BaseStats,
            &StatModifiers,
            &Inventory,
            &mut Vitals,
            &mut Speeds,
            &mut CombatStats,
        )>(&oid)
    {
        *base = BaseStats {
            str_: t.base_str + sums.str_,
            dex: t.base_dex + sums.dex,
            con: t.base_con + sums.con,
            int_: t.base_int + sums.int_,
            wit: t.base_wit + sums.wit,
            men: t.base_men + sums.men,
        };
        player.recalculate_stats(
            &world.data,
            &base,
            mods,
            inventory,
            &mut speeds,
            &mut combat,
        );
        vitals.max_hp =
            crate::model::calc_max_hp(&world.data, &t, player.level, Some(inventory), mods) as i32;
        vitals.max_mp =
            crate::model::calc_max_mp(&world.data, &t, player.level, Some(inventory), mods) as i32;
        vitals.cur_hp = vitals.cur_hp.min(vitals.max_hp as f64);
        vitals.cur_mp = vitals.cur_mp.min(vitals.max_mp as f64);
    }

    if let Some(v) = crate::model::PlayerView::of_world(world, oid) {
        send(
            world,
            client_id,
            crate::network::user_info::user_info(
                &v,
                &world.data,
                &world.cfg.character,
                super::party::calculate_relation(world, v.p),
            ),
        );
    }
    send_henna_info(world, client_id, oid);
}

fn refresh_inventory(world: &World, client_id: u32, oid: i32) {
    if let Some(inv) = world.objects.get_component::<Inventory>(&oid) {
        send(
            world,
            client_id,
            crate::network::enter_world::item_list(inv, &world.data, false),
        );
    }
}

fn equip_lines(world: &World, oid: i32) -> Vec<sp::HennaLine> {
    let class_id = class_id_of(world, oid);
    world
        .data
        .hennas
        .list_for_class(class_id)
        .iter()
        .filter(|h| {
            world
                .objects
                .get_component::<Inventory>(&oid)
                .is_some_and(|inv| inv.count_of(h.dye_item_id) > 0)
        })
        .map(|h| (h.dye_id, h.dye_item_id, h.wear_count, h.wear_fee, true))
        .collect()
}

/// The current effective value of a base stat (already includes worn henna).
fn current(base: &BaseStats, stat: BaseStat) -> i32 {
    match stat {
        BaseStat::Str => base.str_,
        BaseStat::Con => base.con,
        BaseStat::Dex => base.dex,
        BaseStat::Int => base.int_,
        BaseStat::Men => base.men,
        BaseStat::Wit => base.wit,
    }
}

/// Build the six `(current, preview)` stat pairs in INT/STR/CON/MEN/DEX/WIT
/// wire order, where `preview` is produced by `f`.
fn stat_preview(base: &BaseStats, f: impl Fn(BaseStat) -> i32) -> StatPreview {
    [
        (current(base, BaseStat::Int), f(BaseStat::Int) as i16),
        (current(base, BaseStat::Str), f(BaseStat::Str) as i16),
        (current(base, BaseStat::Con), f(BaseStat::Con) as i16),
        (current(base, BaseStat::Men), f(BaseStat::Men) as i16),
        (current(base, BaseStat::Dex), f(BaseStat::Dex) as i16),
        (current(base, BaseStat::Wit), f(BaseStat::Wit) as i16),
    ]
}
