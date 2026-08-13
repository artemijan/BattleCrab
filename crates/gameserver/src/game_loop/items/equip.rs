//! The equip/paperdoll lifecycle: equip clicks, unequip, slot displacement
//! refresh and equipped-item destruction.

use super::*;
/// The cursed-weapon half of `UseItem.runImpl`'s equipable branch: a wielder of
/// Zariche/Akamanah may neither put on formal wear (6408) nor touch a hand slot
/// — the curse locks the weapon in place, so the "just swap to another sword"
/// escape hatch does not exist.
///
/// Deliberately sits in the *packet* handler rather than in
/// [`use_equipable_item`], mirroring Java: `CursedWeapon.activate` equips the
/// weapon through `getInventory().equipItem(…)`, well below this check, and the
/// queued-while-busy replay re-enters at `useEquippableItem`, past it too.
/// Moving the gate down would make the curse unable to equip itself.
pub(super) fn cursed_weapon_blocks_equip(
    world: &World,
    object_id: i32,
    item_object_id: i32,
) -> bool {
    use crate::data::item_data::{SLOT_L_HAND, SLOT_LR_HAND, SLOT_R_HAND};

    if world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .is_none_or(|p| p.cursed_weapon_equipped_id == 0)
    {
        return false;
    }
    let Some((item_id, body_part)) = item_id_of(world, object_id, item_object_id)
        .map(|id| (id, world.data.item_data.get(id).map_or(0, |t| t.body_part)))
    else {
        return false;
    };
    // "Don't allow to put formal wear while a cursed weapon is equipped."
    item_id == FORMAL_WEAR_ITEM_ID || matches!(body_part, SLOT_LR_HAND | SLOT_L_HAND | SLOT_R_HAND)
}

/// Formal Wear — Java `UseItem` names the id inline in the cursed-weapon guard.
const FORMAL_WEAR_ITEM_ID: i32 = 6408;

/// The equipable branch of `UseItem.runImpl`, entered from the packet handler
/// and from the queued replay (`run_queued_action`): while busy, Java defers
/// the equip instead of dropping it — to cast end via
/// `setNextAction(NextAction(EVT_FINISH_CASTING, …))`, to swing end via a
/// schedule at `attackEndTime` — sending no packet either way. Non-equipable
/// items never get queued this way (dispatched to `use_etc_item` immediately,
/// same as Java's else-branch which has no busy check).
pub(crate) fn use_equipable_item(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    item_object_id: i32,
) {
    use crate::model::components::{AttackState, Casting, QueuedAction};

    let is_equipable = {
        let catalog = &world.data.item_data;
        let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else {
            return;
        };
        let Some(item) = inventory.by_object_id(item_object_id) else {
            return;
        };
        let Some(template) = catalog.get(item.item_id) else {
            return;
        };
        template.is_equipable()
    };
    if !is_equipable {
        use_etc_item(world, client_id, object_id, item_object_id);
        return;
    }

    let mid_swing = world
        .objects
        .get_component::<AttackState>(&object_id)
        .is_some_and(|st| st.attack_end_tick > world.tick);
    if mid_swing || world.objects.has_component::<Casting>(&object_id) {
        world
            .objects
            .add_components(&object_id, QueuedAction::UseItem { item_object_id });
        return;
    }

    let catalog = &world.data.item_data;
    let Some(inventory) = world.objects.get_component_mut::<Inventory>(&object_id) else {
        return;
    };

    // Java resolves the item's *currently occupied* single-bit slot
    // (`getSlotFromItem`) before unequipping — not the item's raw template
    // body part, which is a combined bitmask for rings/earrings and would
    // silently no-op. `unequip_item` clears the exact slot we already know
    // the object id is in, sidestepping that resolution entirely.
    let was_equipped = inventory.paperdoll_slot_of(item_object_id).is_some();
    let changed = if was_equipped {
        inventory.unequip_item(item_object_id)
    } else {
        inventory.equip_item(catalog, item_object_id)
    };
    finish_equip_change(world, client_id, object_id, &changed);
    // Java `Player.useEquipableItem`, right after the "you have equipped"
    // message: "Consume mana - will start a task if required; returns if item
    // is not a shadow item". It is the *clicked* item that pays, and only on
    // the equip half of the branch — Java's `if (item.isEquipped())` after
    // `equipItemAndRecord`, which is why a swap's displaced items never pay
    // and taking something off never does either. A shadow weapon therefore
    // burns its first point the moment it goes on, and that call is what arms
    // the 60 s beat. Last, because at mana 1 it destroys the item and
    // re-enters `finish_equip_change` for the unequip.
    if !was_equipped
        && world
            .objects
            .get_component::<Inventory>(&object_id)
            .is_some_and(|inv| inv.paperdoll_slot_of(item_object_id).is_some())
    {
        crate::game_loop::item_mana::on_item_equipped(world, object_id, item_object_id);
    }
}

/// Shared tail of the equip/unequip handlers: persist each changed slot
/// (`items.loc`/`loc_data`), then resend `ExUserInfoEquipSlot` + `UserInfo` +
/// `InventoryUpdate` — in that order, mirroring Java's equip flow:
///   1. `Inventory.setPaperdollItem` sends `ExUserInfoEquipSlot` synchronously
///      *during* the equip, once per paperdoll slot it mutates;
///   2. `Player.useEquippableItem` then calls `broadcastUserInfo` (`UserInfo`);
///   3. …and finally `sendInventoryUpdate` (`InventoryUpdate`).
/// `ExUserInfoEquipSlot` — not just `InventoryUpdate` — is what drives the
/// client's own paperdoll rendering; skipping it leaves newly equipped
/// rings/earrings invisible on the paperdoll even though the inventory list is
/// correct. Two deliberate divergences from Java, both verified in-game:
///   * We send one `ExUserInfoEquipSlot` for the whole action instead of one
///     per `setPaperdollItem` call. The packet is a full 33-slot paperdoll
///     snapshot, so a single send after all slot mutations already carries the
///     final state; Java's per-slot sends only differ in transient intermediate
///     snapshots the client immediately overwrites.
///   * We omit Java's *extra* `ThreadPool.schedule(new ExUserInfoEquipSlot, 100)`
///     in `useEquippableItem` — a redundant second copy of that same snapshot.
pub(crate) fn finish_equip_change(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    changed: &[i32],
) {
    if changed.is_empty() {
        return;
    }
    apply_paperdoll_change(world, client_id, object_id, changed);

    // …and finally Java's `sendInventoryUpdate` — the `InventoryUpdate` plus the
    // adena counter and weight bar it always drags along.
    let changes = crate::game_loop::helpers::modified_changes(world, object_id, changed);
    crate::game_loop::helpers::send_inventory_update(world, object_id, changes);
    refresh_after_paperdoll_change(world, object_id);
    // NB: no shadow-item mana is spent here. Java burns a point in
    // `Player.useEquipableItem` alone — for the one item the player clicked —
    // and this helper stands in for a good deal more than that click: an
    // enchant refreshing a worn item's glow, an augment re-applying its
    // options, `//mount` stripping a weapon. Charging mana from here made a
    // shadow weapon die early for reasons Java never charges for; the call
    // lives at the `use_equipable_item` equip branch instead. See
    // [`super::item_mana`].
}

/// Take `item_oid` off the paperdoll if the owner is wearing it, with the full
/// client refresh [`finish_equip_change`] owes. The shared prologue of every
/// path that takes an item away from under its owner — destroy, crystallize,
/// drop, a failed enchant, a shadow item hitting mana 0, `//mount` disarming a
/// hand.
///
/// No `paperdoll_slot_of` guard: [`Inventory::unequip_item`] already returns an
/// empty change list for an item that isn't on the paperdoll, and
/// [`finish_equip_change`] early-returns on one — so the guard call sites used
/// to write only repeated the lookup `unequip_item` does internally.
pub(crate) fn unequip_if_worn(world: &mut World, client_id: u32, object_id: i32, item_oid: i32) {
    let changed = world
        .objects
        .get_component_mut::<Inventory>(&object_id)
        .map(|inv| inv.unequip_item(item_oid))
        .unwrap_or_default();
    finish_equip_change(world, client_id, object_id, &changed);
}

/// The head of [`finish_equip_change`]: re-apply or drop each changed item's
/// option bonuses, then recompute stats and push the paperdoll to the client.
///
/// Java's equip/unequip listeners fire the augment bonuses first
/// (`Inventory.equipItem`: "Apply augmentation bonuses on equip";
/// `unEquipItemInBodySlot`: "Remove augmentation bonuses on unequip"), and
/// *then* recalculate stats — so an option's modifiers are already in the maps
/// when the recompute runs. `changed` carries the object ids whose paperdoll
/// slot moved either way; which direction it went is read off the inventory
/// here, so an id that has since left the bag entirely (a destroy) correctly
/// takes the "remove" branch.
fn apply_paperdoll_change(world: &mut World, client_id: u32, object_id: i32, changed: &[i32]) {
    for &item_oid in changed {
        let equipped = world
            .objects
            .get_component::<Inventory>(&object_id)
            .is_some_and(|inv| inv.paperdoll_slot_of(item_oid).is_some());
        if equipped {
            crate::game_loop::options::apply_item_options(world, object_id, item_oid);
        } else {
            crate::game_loop::options::remove_item_options(world, object_id, item_oid);
        }
    }
    // Memory-first: the paperdoll change already lives in the `Inventory`
    // component; the new `loc`/`loc_data` of each changed slot persists on the
    // next flush (`Inventory::to_rows`), so equip/unequip spam can't drive DB
    // writes.
    refresh_equip_state(world, client_id, object_id);
}

/// Destroy `count` of `item_id` from `owner_oid`'s bag, running everything the
/// removal implies when the instance was **worn**.
///
/// The whole `equipped_object_ids` protocol in one call. It used to be four
/// hand-rolled steps at each destroy site — snapshot, remove, intersect,
/// finish — and predictably most sites did one or two of them: of the eight
/// paths that can destroy a worn item, exactly one had all four. Prefer this
/// over calling `Inventory::remove_item` directly whenever the item could
/// plausibly be equipped.
///
/// Returns the removal's `ItemChange`s so the caller can still build its own
/// `InventoryUpdate`.
pub(crate) fn destroy_item_by_id(
    world: &mut World,
    owner_oid: i32,
    item_id: i32,
    count: i64,
) -> Vec<crate::model::inventory::ItemChange> {
    use crate::model::inventory::Inventory;
    let before = world
        .objects
        .get_component::<Inventory>(&owner_oid)
        .map(|inv| inv.equipped_object_ids())
        .unwrap_or_default();
    let changes = world
        .objects
        .get_component_mut::<Inventory>(&owner_oid)
        .map(|inv| inv.remove_item(item_id, count))
        .unwrap_or_default();
    let unequipped = unequipped_by_removal(&before, &changes);
    if !unequipped.is_empty() {
        // An offline owner has no client; the packet halves no-op on id 0 while
        // the stat and option halves still run, which is what matters for a
        // character whose inventory is being edited out from under them.
        let client_id = crate::game_loop::helpers::client_for_player(world, owner_oid).unwrap_or(0);
        finish_equipped_item_destroyed(world, client_id, owner_oid, &unequipped);
    }
    changes
}

/// The tail of [`finish_equip_change`]: the owner-wide penalties and passives
/// a paperdoll change can flip. Each sends its own packets, and only when the
/// value it owns actually moved.
pub(crate) fn refresh_after_paperdoll_change(world: &mut World, object_id: i32) {
    // Java `Inventory.equipItem`/`unEquipItemInBodySlot` fire
    // `refreshExpertisePenalty` on the owner: a newly equipped over-grade item
    // (or one just removed) changes the grade penalty. It sends its own
    // EtcStatusUpdate + UserInfo when the penalty actually changed.
    crate::game_loop::expertise::refresh_expertise_penalty(world, object_id);
    crate::game_loop::weight::refresh_weight_penalty(world, object_id);
    // Java re-pumps passive skill effects on the same equip listeners: an
    // armor-conditioned passive (Spellcraft/Magician's Movement) flips as a
    // robe is worn or removed. Resends its own UserInfo when the set changed.
    crate::game_loop::passive_skills::refresh_conditioned_passives(world, object_id);
    // Java `Inventory.ArmorSetListener` — the same paperdoll listener chain.
    // Runs last because it re-pumps the conditioned passives itself once the
    // granted set actually moved, and re-composes `BaseStats` for a `<stats>`
    // set completing or breaking.
    crate::game_loop::armor_sets::refresh_armor_sets(world, object_id);
}

/// The unequip Java runs for free when a *worn* item leaves the bag:
/// `Inventory.removeItem` is overridden to `unEquipItemInSlot` whatever it is
/// about to take out, so `setPaperdollItem(slot, null)` drops the item's
/// bonuses, recalculates the wearer's stats and pushes `ExUserInfoEquipSlot`
/// before the destroy's own `InventoryUpdate` goes out. Here the paperdoll is
/// a plain data component that cannot reach the client, so each destroy path
/// has to call this with the object ids the removal unequipped — snapshot
/// [`crate::model::inventory::Inventory::equipped_object_ids`] before the
/// removal and intersect it with the removal's result via
/// [`unequipped_by_removal`].
///
/// Skipping it is not a cosmetic inventory-window bug: `UserInfo` carries only
/// the right-hand *enchant level*, never the paperdoll item ids, so the client
/// keeps rendering a weapon the character no longer owns while the inventory
/// window correctly shows nothing equipped. Q229 `Test of Witchcraft` hits
/// this — the Sword of Seal is a registered quest item *and* a weapon, so the
/// hand-in's `exitQuest` destroys it straight out of the player's hand.
///
/// Call before the caller's own `InventoryUpdate`, matching Java's ordering.
pub(crate) fn finish_equipped_item_destroyed(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    unequipped: &[crate::model::inventory::ItemInstance],
) {
    if unequipped.is_empty() {
        return;
    }
    // Takes the *instances*, not their ids, because the option ids have to be
    // read off the snapshot: routing through `apply_paperdoll_change` here
    // looked right — the item is absent, so it takes the "unequipped" branch —
    // but that branch then looks the instance up in the bag to find its option
    // ids, finds nothing, and silently removes no bonuses at all. A destroyed
    // augmented weapon left its stats and granted skills on the wearer.
    for it in unequipped {
        crate::game_loop::options::remove_option_ids(
            world,
            object_id,
            &[it.augment_option1, it.augment_option2],
        );
    }
    refresh_equip_state(world, client_id, object_id);
    refresh_after_paperdoll_change(world, object_id);
}

/// The object ids in `changes` that were worn before the removal ran — i.e.
/// the ones [`finish_equipped_item_destroyed`] has to be told about. `before`
/// is an `Inventory::equipped_object_ids` snapshot taken *before* the removal.
pub(crate) fn unequipped_by_removal(
    before: &[i32],
    changes: &[crate::model::inventory::ItemChange],
) -> Vec<crate::model::inventory::ItemInstance> {
    use crate::model::inventory::ItemChange;
    changes
        .iter()
        .filter_map(|c| match c {
            // Only a full removal clears a paperdoll slot; a partial
            // decrement leaves the instance — and its slot — in place.
            // The whole instance travels on: it is the last record of the
            // augment options that have to come off with it.
            ItemChange::Removed(it) if before.contains(&it.object_id) => Some(*it),
            _ => None,
        })
        .collect()
}

/// The stat-and-paperdoll half of [`finish_equip_change`]: recompute the
/// wearer's stats, then push the client's own paperdoll snapshot
/// (`ExUserInfoEquipSlot`) and `UserInfo`.
///
/// Java emits `ExUserInfoEquipSlot` from inside `Inventory.setPaperdollItem`,
/// the single choke point *every* paperdoll mutation goes through — including
/// the implicit ones, where nobody called "unequip" at all: `ItemContainer`'s
/// `removeItem` is overridden by `Inventory.removeItem` to unequip whatever it
/// is about to take out of the bag, so dropping, destroying or transferring a
/// worn item refreshes the paperdoll for free. Here the paperdoll lives in a
/// plain data component that cannot reach the client, so each of those paths
/// has to call this itself.
///
/// Forgetting it is not a cosmetic inventory-window bug: `UserInfo` carries
/// only the right-hand *enchant level*, never the paperdoll item ids, so the
/// client keeps rendering a weapon the character no longer owns while the
/// inventory window correctly shows nothing equipped.
pub(crate) fn refresh_equip_state(world: &mut World, client_id: u32, object_id: i32) {
    // Recompute combat stats now that the paperdoll changed: a newly equipped
    // weapon's pAtk / armor's pDef must reach the `UserInfo` below (Java
    // `Inventory.equipItem`/`unEquipItemInBodySlot` → `Creature.recalculateStats`
    // before `broadcastUserInfo`). Without it the client shows the item on the
    // paperdoll but the stat panel never moves.
    if let Some((player, base, mods, inventory, mut vitals, mut speeds, mut combat)) =
        world.objects.get_many_mut::<(
            &crate::model::Player,
            &crate::model::components::BaseStats,
            &crate::model::components::StatModifiers,
            &Inventory,
            &mut crate::model::components::Vitals,
            &mut crate::model::components::Speeds,
            &mut crate::model::components::CombatStats,
        )>(&object_id)
    {
        player.recalculate_stats(&world.data, base, mods, inventory, &mut speeds, &mut combat);
        // Max HP/MP can carry item bonuses (e.g. +MP jewelry), which live in
        // `Vitals` on a separate path from `recalculate_stats`. Recompute them
        // and clamp current values down if a bonus was just removed (Java's
        // MaxHp/MaxMp finalizers run inside the same `recalculateStats`).
        let t = world
            .data
            .player_templates
            .get_or_base(player.class_id, player.base_class_id)
            .cloned()
            .unwrap_or_default();
        vitals.max_hp =
            crate::model::calc_max_hp(&world.data, &t, player.level, Some(inventory), mods) as i32;
        vitals.max_mp =
            crate::model::calc_max_mp(&world.data, &t, player.level, Some(inventory), mods) as i32;
        vitals.cur_hp = vitals.cur_hp.min(vitals.max_hp as f64);
        vitals.cur_mp = vitals.cur_mp.min(vitals.max_mp as f64);
    }

    let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else {
        return;
    };
    send_to_client(
        world,
        client_id,
        crate::network::enter_world::ex_user_info_equip_slot(object_id, inventory),
    );
    if let Some(v) = crate::model::PlayerView::of_world(world, object_id) {
        send_to_client(
            world,
            client_id,
            crate::network::user_info::user_info(
                &v,
                &world.data,
                &world.cfg.character,
                crate::game_loop::party::calculate_relation(world, v.p),
            ),
        );
    }
}
