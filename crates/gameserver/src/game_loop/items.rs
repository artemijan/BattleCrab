//! Gear equip/unequip handlers (`UseItem`, `RequestUnEquipItem`) and the
//! `EtcItem` "use" dispatch (`ExtractableItems` for pack/box items).

use tracing::warn;

use crate::data::item_data::ItemHandler;
use crate::model::inventory::Inventory;
use crate::network::client_packets as cp;
use crate::network::enter_world as ew;
use crate::network::server_packets::{self, sm_ids, SmParam};
use crate::session::ClientSession;
use crate::world::World;

/// The stack-or-create core of `Player.addItem`: merge into an existing
/// stack (persisting the new count) or allocate an object id and insert a
/// fresh instance. Non-stackable items get one instance *per unit*, mirroring
/// `ItemContainer.addItem`'s `for (i = 0; i < count; i++)` split under
/// `MultipleItemDrop = True` — the only value ever shipped in this dist's
/// `General.ini`, so it isn't wired up as a runtime toggle. Getting this
/// wrong is exactly the "2 earrings become 1 that vanishes on equip" class of
/// bug: a non-stackable item with count > 1 crammed into a single instance
/// is a state the paperdoll (one object id per slot) can't represent.
/// Returns every object id created/touched; `None` only on id-pool
/// exhaustion (any already-created units stay, matching Java's partial
/// completion when `createItem` fails mid-loop). Shared by the auto-loot
/// path (`death::give_item`), quest rewards (`quests`), the shop (`shop`),
/// and pack/box extraction (`extract_item` below); the caller owns
/// messaging/`InventoryUpdate`.
pub(crate) fn add_inventory_item(world: &mut World, player_oid: i32, item_id: i32, count: i64) -> Option<Vec<i32>> {
    let stackable = world.data.item_data.get(item_id).map(|t| t.is_stackable).unwrap_or(false);
    if stackable {
        let existing_stack = world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&player_oid)
            .and_then(|inv| inv.items().iter().find(|i| i.item_id == item_id).map(|i| i.object_id));

        if let Some(stack_oid) = existing_stack {
            // Memory-first: the stack grows in memory; the new count persists on
            // the next flush, not here.
            let inv = world
                .objects
                .get_component_mut::<crate::model::inventory::Inventory>(&player_oid)
                .expect("checked");
            inv.add_item(&world.data.item_data, stack_oid, item_id, count);
            return Some(vec![stack_oid]);
        }
        let new_oid = world.alloc_object_id()?;
        let inv = world.objects.get_component_mut::<crate::model::inventory::Inventory>(&player_oid)?;
        inv.add_item(&world.data.item_data, new_oid, item_id, count);
        return Some(vec![new_oid]);
    }

    let mut created = Vec::with_capacity(count.max(1) as usize);
    for _ in 0..count.max(1) {
        let new_oid = world.alloc_object_id()?;
        let inv = world.objects.get_component_mut::<crate::model::inventory::Inventory>(&player_oid)?;
        inv.add_item(&world.data.item_data, new_oid, item_id, 1);
        created.push(new_oid);
    }
    Some(created)
}

/// Port of `clientpackets/RequestItemList.runImpl`: the client opened its
/// inventory window and wants the current contents. Java calls
/// `player.sendItemList(true)`, which (after a 300 ms debounce we don't
/// replicate — there's no per-client timer here) sends `ItemList` with the
/// show-window flag set, then `ExQuestItemList`, `ExAdenaInvenCount` and
/// `ExUserInfoInvenWeight`. The `isInventoryDisabled` guard is a no-op: nothing
/// in this port blocks the inventory yet (set only by trades/some skills, both
/// unported).
pub(crate) fn handle_request_item_list(world: &mut World, client_id: u32) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else { return };
    let Some(cs) = world.clients.get(&client_id) else { return };
    cs.send(ew::item_list(inventory, &world.data, true));
    cs.send(ew::ex_quest_item_list(inventory, &world.data));
    cs.send(ew::ex_adena_inven_count(inventory));
    cs.send(ew::ex_user_info_inven_weight(object_id, inventory, &world.data));
}

/// Port of `clientpackets/UseItem.runImpl`: right-clicking a `Weapon`/`Armor`
/// toggles equip/unequip; anything else routes through the `EtcItem` handler
/// dispatch (Java: `ItemHandler.getInstance().getHandler(etcItem)`).
pub(crate) fn handle_use_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::UseItem::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    // `UseItem.runImpl`: `hasBlockActions() || isControlBlocked() ||
    // isAlikeDead()` refuses the use outright. (Death is gated further in.)
    if crate::game_loop::abnormal::is_blocked_from_actions(world, object_id)
        || crate::game_loop::abnormal::is_control_blocked(world, object_id)
    {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }
    use_equipable_item(world, client_id, object_id, pkt.object_id);
}

/// The equipable branch of `UseItem.runImpl`, entered from the packet handler
/// and from the queued replay (`run_queued_action`): while busy, Java defers
/// the equip instead of dropping it — to cast end via
/// `setNextAction(NextAction(EVT_FINISH_CASTING, …))`, to swing end via a
/// schedule at `attackEndTime` — sending no packet either way. Non-equipable
/// items never get queued this way (dispatched to `use_etc_item` immediately,
/// same as Java's else-branch which has no busy check).
pub(crate) fn use_equipable_item(world: &mut World, client_id: u32, object_id: i32, item_object_id: i32) {
    use crate::model::components::{AttackState, Casting, QueuedAction};

    let is_equipable = {
        let catalog = &world.data.item_data;
        let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else {
            return;
        };
        let Some(item) = inventory.items().iter().find(|i| i.object_id == item_object_id) else { return };
        let Some(template) = catalog.get(item.item_id) else { return };
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
        world.objects.add_components(&object_id, QueuedAction::UseItem { item_object_id });
        return;
    }

    let catalog = &world.data.item_data;
    let Some(inventory) = world.objects.get_component_mut::<crate::model::inventory::Inventory>(&object_id) else {
        return;
    };

    // Java resolves the item's *currently occupied* single-bit slot
    // (`getSlotFromItem`) before unequipping — not the item's raw template
    // body part, which is a combined bitmask for rings/earrings and would
    // silently no-op. `unequip_item` clears the exact slot we already know
    // the object id is in, sidestepping that resolution entirely.
    let changed = if inventory.paperdoll_slot_of(item_object_id).is_some() {
        inventory.unequip_item(item_object_id)
    } else {
        inventory.equip_item(catalog, item_object_id)
    };
    finish_equip_change(world, client_id, object_id, &changed);
}

/// Port of `clientpackets/RequestUnEquipItem.runImpl` (combat/cursed-weapon
/// guards are skipped — there's no combat system yet).
pub(crate) fn handle_request_un_equip_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(body_part) = cp::read_char_slot(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    let Some(inventory) = world.objects.get_component_mut::<crate::model::inventory::Inventory>(&object_id) else {
        return;
    };
    let changed = inventory.unequip_slot(body_part);
    finish_equip_change(world, client_id, object_id, &changed);
}

/// Port of `clientpackets/RequestDestroyItem.runImpl`: destroy `count` of an
/// inventory item. Quest items are protected (Java's non-`isDestroyable` +
/// `DESTROY_ALL_ITEMS=false` guard, narrowed to the flag the port models); an
/// equipped item is unequipped first. The cursed-weapon / hero-item / pet /
/// enchant-transaction guards are skipped (those subsystems aren't ported).
pub(crate) fn handle_request_destroy_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestDestroyItem::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    if pkt.count <= 0 {
        return;
    }
    // Locate the item + its template facts.
    let Some((item_id, held, is_stackable, is_quest)) = world
        .objects
        .get_component::<Inventory>(&object_id)
        .and_then(|inv| inv.items().iter().find(|it| it.object_id == pkt.object_id).map(|it| (it.item_id, it.count)))
        .map(|(id, cnt)| {
            let t = world.data.item_data.get(id);
            (id, cnt, t.map(|t| t.is_stackable).unwrap_or(false), t.map(|t| t.is_quest_item).unwrap_or(false))
        })
    else {
        send_item_message(world, client_id, "This item cannot be destroyed.");
        return;
    };
    let _ = item_id;
    if is_quest {
        send_item_message(world, client_id, "This item cannot be destroyed.");
        return;
    }
    // A non-stackable item can only be destroyed one at a time (Java returns).
    if !is_stackable && pkt.count > 1 {
        return;
    }
    let count = pkt.count.min(held);

    // Unequip first if it's worn (Java unequips, sending its own InventoryUpdate).
    if world.objects.get_component::<Inventory>(&object_id).is_some_and(|inv| inv.paperdoll_slot_of(pkt.object_id).is_some()) {
        let changed = world
            .objects
            .get_component_mut::<Inventory>(&object_id)
            .map(|inv| inv.unequip_item(pkt.object_id))
            .unwrap_or_default();
        finish_equip_change(world, client_id, object_id, &changed);
    }

    let Some(change) = world.objects.get_component_mut::<Inventory>(&object_id).and_then(|inv| inv.remove_by_object_id(pkt.object_id, count)) else {
        return;
    };
    let packet = ew::inventory_update_changes(&world.data, &[change]);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
}

/// The `Crystallize` common skill (`CommonSkill.CRYSTALLIZE`).
const CRYSTALLIZE_SKILL_ID: i32 = 248;

/// Port of `clientpackets/RequestCrystallizeItem.runImpl` (narrowed): destroy a
/// crystallizable item and yield its grade's crystals. Gated on the player's
/// `Crystallize` (248) skill level vs the item grade (D→1 … S→5). With no
/// `ItemCrystallizationData`, Java's fallback is `crystalCount` of the grade's
/// crystal at 100% — that's what we award. Hero/shadow/augment guards skipped.
pub(crate) fn handle_request_crystallize_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestDestroyItem::read(body) else { return }; // same layout (objectId, count)
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let player_oid = session.player_object_id();
    if pkt.count <= 0 {
        return;
    }
    // Locate the item + its crystallization facts.
    let Some((item_id, held, is_stackable)) = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .and_then(|inv| inv.items().iter().find(|it| it.object_id == pkt.object_id).map(|it| (it.item_id, it.count)))
        .map(|(id, cnt)| (id, cnt, world.data.item_data.get(id).map(|t| t.is_stackable).unwrap_or(false)))
    else {
        return;
    };
    let Some(t) = world.data.item_data.get(item_id) else { return };
    let (Some(crystal_item), crystal_count) = (t.crystal_type.crystal_item_id(), t.crystal_count) else {
        send_item_message(world, client_id, "This item cannot be crystallized.");
        return;
    };
    if crystal_count <= 0 {
        send_item_message(world, client_id, "This item cannot be crystallized.");
        return;
    }
    let required = t.crystal_type.required_crystallize_level();
    let skill_level = world.objects.get_component::<crate::model::components::SkillBook>(&player_oid).and_then(|b| b.0.get(&CRYSTALLIZE_SKILL_ID).copied()).unwrap_or(0);
    if skill_level < required {
        send_item_message(world, client_id, "Your crystallization skill level is too low.");
        return;
    }
    if !is_stackable && pkt.count > 1 {
        return;
    }
    let count = pkt.count.min(held);

    // Unequip first if worn, then destroy, then award the crystals.
    if world.objects.get_component::<Inventory>(&player_oid).is_some_and(|inv| inv.paperdoll_slot_of(pkt.object_id).is_some()) {
        let changed = world.objects.get_component_mut::<Inventory>(&player_oid).map(|inv| inv.unequip_item(pkt.object_id)).unwrap_or_default();
        finish_equip_change(world, client_id, player_oid, &changed);
    }
    let Some(removed) = world.objects.get_component_mut::<Inventory>(&player_oid).and_then(|inv| inv.remove_by_object_id(pkt.object_id, count)) else {
        return;
    };
    let total = crystal_count as i64 * count;
    add_inventory_item(world, player_oid, crystal_item, total);
    // InventoryUpdate: the destroyed item + the crystal stack (as a modify).
    let mut changes = vec![removed];
    if let Some(inv) = world.objects.get_component::<Inventory>(&player_oid) {
        if let Some(stack) = inv.items().iter().find(|it| it.item_id == crystal_item) {
            changes.push(crate::model::inventory::ItemChange::Modified(*stack));
        }
    }
    let packet = ew::inventory_update_changes(&world.data, &changes);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
}

/// Send a bare `$s1` system-message line to one client.
fn send_item_message(world: &World, client_id: u32, text: &str) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::system_message_with(sm_ids::S1_TEXT, &[SmParam::Text(text.to_string())]));
    }
}

/// Port of `clientpackets/RequestSaveInventoryOrder.runImpl`: persist the
/// client's custom inventory arrangement. For each `(object_id, order)` pair,
/// Java sets `item.setItemLocation(INVENTORY, order)` — but only for items
/// *currently* in `INVENTORY` (equipped/paperdoll items are skipped). We mirror
/// that guard via `paperdoll_slot_of`, then fire-and-forget the new `loc_data`
/// to the DB; `load_items` restores `ORDER BY loc_data`, so the arrangement
/// survives relog. No response packet — Java sends none either.
pub(crate) fn handle_request_save_inventory_order(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestSaveInventoryOrder::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    let Some(inventory) = world.objects.get_component_mut::<Inventory>(&object_id) else {
        return;
    };
    // Keep only the pairs naming an item actually in the inventory grid — an
    // equipped item occupies a paperdoll slot and keeps its slot index. Applied
    // to the in-memory order (memory-first); it persists to `loc_data` on the
    // next flush, not here.
    let order: Vec<(i32, i32)> = pkt
        .order
        .into_iter()
        .filter(|&(oid, _)| {
            inventory.items().iter().any(|i| i.object_id == oid) && inventory.paperdoll_slot_of(oid).is_none()
        })
        .collect();
    inventory.apply_inventory_order(&order);
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
pub(crate) fn finish_equip_change(world: &mut World, client_id: u32, object_id: i32, changed: &[i32]) {
    if changed.is_empty() {
        return;
    }
    // Memory-first: the paperdoll change already lives in the `Inventory`
    // component; the new `loc`/`loc_data` of each changed slot persists on the
    // next flush (`Inventory::to_rows`), so equip/unequip spam can't drive DB
    // writes.

    // Recompute combat stats now that the paperdoll changed: a newly equipped
    // weapon's pAtk / armor's pDef must reach the `UserInfo` below (Java
    // `Inventory.equipItem`/`unEquipItemInBodySlot` → `Creature.recalculateStats`
    // before `broadcastUserInfo`). Without it the client shows the item on the
    // paperdoll but the stat panel never moves.
    if let Some((player, base, mods, inventory, mut vitals, mut speeds, mut combat)) = world.objects.get_many_mut::<(
        &crate::model::Player,
        &crate::model::components::BaseStats,
        &crate::model::components::StatModifiers,
        &crate::model::inventory::Inventory,
        &mut crate::model::components::Vitals,
        &mut crate::model::components::Speeds,
        &mut crate::model::components::CombatStats,
    )>(&object_id)
    {
        player.recalculate_stats(&world.data, base, mods, &inventory, &mut speeds, &mut combat);
        // Max HP/MP can carry item bonuses (e.g. +MP jewelry), which live in
        // `Vitals` on a separate path from `recalculate_stats`. Recompute them
        // and clamp current values down if a bonus was just removed (Java's
        // MaxHp/MaxMp finalizers run inside the same `recalculateStats`).
        let t = world
            .data
            .player_templates
            .get(player.class_id)
            .or_else(|| world.data.player_templates.get(player.base_class_id))
            .cloned()
            .unwrap_or_default();
        vitals.max_hp = crate::model::calc_max_hp(&world.data, &t, player.level, Some(&inventory), mods) as i32;
        vitals.max_mp = crate::model::calc_max_mp(&world.data, &t, player.level, Some(&inventory), mods) as i32;
        vitals.cur_hp = vitals.cur_hp.min(vitals.max_hp as f64);
        vitals.cur_mp = vitals.cur_mp.min(vitals.max_mp as f64);
    }

    let Some(inventory) = world.objects.get_component::<crate::model::inventory::Inventory>(&object_id) else {
        return;
    };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(crate::network::enter_world::ex_user_info_equip_slot(object_id, inventory));
        if let Some(v) = crate::model::PlayerView::of(&world.objects, object_id) {
            cs.send(crate::network::user_info::user_info(&v, &world.data, &world.cfg.character, super::party::calculate_relation(world, v.p)));
        }
        cs.send(crate::network::enter_world::inventory_update(inventory, &world.data, changed));
    }
    // Java `Inventory.equipItem`/`unEquipItemInBodySlot` fire
    // `refreshExpertisePenalty` on the owner: a newly equipped over-grade item
    // (or one just removed) changes the grade penalty. Runs last so the borrow
    // of `inventory` above is released; it sends its own EtcStatusUpdate +
    // UserInfo when the penalty actually changed.
    crate::game_loop::expertise::refresh_expertise_penalty(world, object_id);
    // Java re-pumps passive skill effects on the same equip listeners: an
    // armor-conditioned passive (Spellcraft/Magician's Movement) flips as a
    // robe is worn or removed. Resends its own UserInfo when the set changed.
    crate::game_loop::passive_skills::refresh_conditioned_passives(world, object_id);
}

/// The `EtcItem` branch of `UseItem.runImpl` (Java:
/// `ItemHandler.getInstance().getHandler(etcItem)`). Dispatches on
/// `ItemTemplate.handler`; only `ExtractableItems` (pack/box items) is
/// implemented so far. Anything else is consumed as a no-op, matching Java's
/// "Unmanaged Item handler" branch (logged, no visible effect to the player).
fn use_etc_item(world: &mut World, client_id: u32, object_id: i32, item_object_id: i32) {
    let handler = {
        let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else { return };
        let Some(item) = inventory.items().iter().find(|i| i.object_id == item_object_id) else { return };
        world.data.item_data.get(item.item_id).map(|t| t.handler).unwrap_or_default()
    };
    match handler {
        ItemHandler::ExtractableItems => extract_item(world, client_id, object_id, item_object_id),
        ItemHandler::ItemSkills => use_item_skills(world, client_id, object_id, item_object_id),
        ItemHandler::SoulShots | ItemHandler::SpiritShot | ItemHandler::BlessedSpiritShot => {
            let item_id = world
                .objects
                .get_component::<Inventory>(&object_id)
                .and_then(|inv| inv.items().iter().find(|i| i.object_id == item_object_id).map(|i| i.item_id));
            if let Some(item_id) = item_id {
                charge_shot(world, object_id, item_id, handler, false);
            }
        }
        ItemHandler::EnchantScrolls => {
            super::enchant::open(world, client_id, object_id, item_object_id)
        }
        ItemHandler::Recipes => {
            super::crafting::learn_recipe(world, client_id, object_id, item_object_id)
        }
        ItemHandler::None => {}
    }
}

/// Port of `handlers/itemhandlers/{SoulShots,SpiritShot,BlessedSpiritShot}.useItem`:
/// charge the matching shot on the equipped weapon. `auto` = true is the
/// `rechargeShots` re-entry (an item toggled for auto-use): it suppresses the
/// enable/error chat and the not-enough message, exactly like Java gating those
/// on `!getAutoSoulShot().contains(itemId)`. Returns whether a shot was charged.
///
/// Narrowing vs. Java: the `reducedSoulshot`/`reducedSoulshotChance` weapon
/// perk (a chance to spend fewer shots) isn't modelled — no Interlude weapon in
/// the dist declares it — and the ruby/sapphire brooch visual swap doesn't
/// exist (no jewels), so the shot's own `<skills>` visual always plays.
pub(crate) fn charge_shot(world: &mut World, object_id: i32, shot_item_id: i32, handler: ItemHandler, auto: bool) -> bool {
    use crate::model::{Player, ShotType};

    let physical = handler.is_soulshot();
    let shot_type = match handler {
        ItemHandler::SoulShots => ShotType::Soulshots,
        ItemHandler::SpiritShot => ShotType::Spiritshots,
        ItemHandler::BlessedSpiritShot => ShotType::BlessedSpiritshots,
        _ => return false,
    };
    let client_id = crate::game_loop::helpers::client_for_player(world, object_id);
    let send = |world: &World, msg: i16| {
        if !auto {
            if let Some(cid) = client_id {
                if let Some(cs) = world.clients.get(&cid) {
                    cs.send(server_packets::system_message_with(msg, &[]));
                }
            }
        }
    };

    // Equipped weapon + its per-charge shot count / grade.
    let (weapon_item_id, shot_visual) = {
        let Some(inv) = world.objects.get_component::<Inventory>(&object_id) else { return false };
        let weapon = inv.paperdoll_item_id(crate::model::inventory::PaperdollSlot::RHand);
        let visual = world.data.item_data.get(shot_item_id).map(|t| t.item_skills.clone()).unwrap_or_default();
        (weapon, visual)
    };
    let shot_count = if physical {
        world.data.item_data.soulshot_count(weapon_item_id)
    } else {
        world.data.item_data.spiritshot_count(weapon_item_id)
    };

    // No weapon, or a weapon that can't take this shot kind.
    if weapon_item_id == 0 || shot_count == 0 {
        send(world, if physical { sm_ids::CANNOT_USE_SOULSHOTS } else { sm_ids::YOU_MAY_NOT_USE_SPIRITSHOTS });
        return false;
    }

    // Grade must match (`getCrystalTypePlus`).
    let weapon_grade = world.data.item_data.get(weapon_item_id).map(|t| t.crystal_type.plus());
    let shot_grade = world.data.item_data.get(shot_item_id).map(|t| t.crystal_type.plus());
    if weapon_grade != shot_grade {
        send(world, if physical { sm_ids::THE_SOULSHOT_YOU_ARE_ATTEMPTING_TO_USE_DOES_NOT_MATCH_THE_GRADE_OF_YOUR_EQUIPPED_WEAPON } else { sm_ids::YOUR_SPIRITSHOT_DOES_NOT_MATCH_THE_WEAPON_S_GRADE });
        return false;
    }

    // Already charged → no-op (also how the auto path avoids re-spending).
    if world.objects.get_component::<Player>(&object_id).is_some_and(|p| p.is_charged_shot(shot_type)) {
        return false;
    }

    // Consume the shots; not enough → drop auto-use for this item.
    let have = world.objects.get_component::<Inventory>(&object_id).map(|inv| inv.count_of(shot_item_id)).unwrap_or(0);
    if have < shot_count as i64 {
        if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
            p.auto_shots.retain(|&id| id != shot_item_id);
        }
        send(world, if physical { sm_ids::YOU_DO_NOT_HAVE_ENOUGH_SOULSHOTS_FOR_THAT } else { sm_ids::YOU_DO_NOT_HAVE_ENOUGH_SPIRITSHOT_FOR_THAT });
        return false;
    }
    let changes = world
        .objects
        .get_component_mut::<Inventory>(&object_id)
        .map(|inv| inv.remove_item(shot_item_id, shot_count as i64))
        .unwrap_or_default();

    // Charge, notify, replay the count change, play the visual.
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.charge_shot(shot_type);
    }
    if !changes.is_empty() {
        if let Some(cid) = client_id {
            if let Some(cs) = world.clients.get(&cid) {
                cs.send(ew::inventory_update_changes(&world.data, &changes));
            }
        }
    }
    send(world, if physical { sm_ids::YOUR_SOULSHOTS_ARE_ENABLED } else { sm_ids::YOUR_SPIRITSHOT_HAS_BEEN_ENABLED });
    broadcast_shot_visual(world, object_id, &shot_visual);
    true
}

/// Port of `clientpackets/RequestAutoSoulShot.runImpl` (player-shot branch —
/// summon shots aren't in scope): toggle a shot item into the auto-use set.
/// Body: `itemId:i32, enable:i32(1/0), type:i32`.
pub(crate) fn handle_request_auto_soul_shot(world: &mut World, client_id: u32, ex_body: &[u8]) {
    use crate::model::Player;

    if ex_body.len() < 12 {
        return;
    }
    let item_id = i32::from_le_bytes(ex_body[0..4].try_into().unwrap());
    let enable = i32::from_le_bytes(ex_body[4..8].try_into().unwrap()) == 1;
    let shot_type = i32::from_le_bytes(ex_body[8..12].try_into().unwrap());

    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    // `!player.isDead()` — a dead player can't toggle shots.
    if world.objects.get_component::<crate::model::components::Vitals>(&object_id).is_none_or(|v| v.dead) {
        return;
    }
    // The item must be in the inventory, and be a player shot we handle.
    let handler = {
        let Some(inv) = world.objects.get_component::<Inventory>(&object_id) else { return };
        if inv.count_of(item_id) == 0 {
            return;
        }
        world.data.item_data.get(item_id).map(|t| t.handler).unwrap_or_default()
    };
    if !handler.is_soulshot() && !handler.is_spiritshot() {
        return;
    }

    let send = |world: &World, msg: i16, params: &[SmParam]| {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(msg, params));
        }
    };

    if enable {
        // Grade check (`item.getCrystalType() != weapon.getCrystalTypePlus()`,
        // or no weapon at all — fists).
        let weapon_item_id = world
            .objects
            .get_component::<Inventory>(&object_id)
            .map(|inv| inv.paperdoll_item_id(crate::model::inventory::PaperdollSlot::RHand))
            .unwrap_or(0);
        let weapon_grade = world.data.item_data.get(weapon_item_id).map(|t| t.crystal_type.plus());
        let shot_grade = world.data.item_data.get(item_id).map(|t| t.crystal_type);
        if weapon_item_id == 0 || weapon_grade != shot_grade {
            send(
                world,
                if handler.is_soulshot() {
                    sm_ids::THE_SOULSHOT_YOU_ARE_ATTEMPTING_TO_USE_DOES_NOT_MATCH_THE_GRADE_OF_YOUR_EQUIPPED_WEAPON
                } else {
                    sm_ids::YOUR_SPIRITSHOT_DOES_NOT_MATCH_THE_WEAPON_S_GRADE
                },
                &[],
            );
            return;
        }
        // Activate.
        if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
            if !p.auto_shots.contains(&item_id) {
                p.auto_shots.push(item_id);
            }
        }
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::ex_auto_soul_shot(item_id, true, shot_type));
        }
        send(world, sm_ids::THE_AUTOMATIC_USE_OF_S1_HAS_BEEN_ACTIVATED, &[SmParam::ItemName(item_id)]);
        // Charge immediately (Java `player.rechargeShots(...)`).
        recharge_shots(world, object_id, handler.is_soulshot(), handler.is_spiritshot());
    } else {
        // Deactivate.
        if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
            p.auto_shots.retain(|&id| id != item_id);
        }
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::ex_auto_soul_shot(item_id, false, shot_type));
        }
        send(world, sm_ids::THE_AUTOMATIC_USE_OF_S1_HAS_BEEN_DEACTIVATED, &[SmParam::ItemName(item_id)]);
    }
}

/// Port of `Player.rechargeShots(physical, magic, fish)`: for each shot item
/// the player toggled for auto-use, if its category matches the requested one,
/// (re)charge it. Java runs this at the start of every attack (`physical`) and
/// cast (`magic`). A toggled item that's no longer in the inventory is dropped
/// from the auto set (Java's `removeAutoSoulShot` on `getItemByItemId == null`).
pub(crate) fn recharge_shots(world: &mut World, object_id: i32, physical: bool, magic: bool) {
    let auto = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .map(|p| p.auto_shots.clone())
        .unwrap_or_default();
    for item_id in auto {
        if world.objects.get_component::<Inventory>(&object_id).map(|inv| inv.count_of(item_id)).unwrap_or(0) == 0 {
            if let Some(p) = world.objects.get_component_mut::<crate::model::Player>(&object_id) {
                p.auto_shots.retain(|&id| id != item_id);
            }
            continue;
        }
        let handler = world.data.item_data.get(item_id).map(|t| t.handler).unwrap_or_default();
        if (magic && handler.is_spiritshot()) || (physical && handler.is_soulshot()) {
            charge_shot(world, object_id, item_id, handler, true);
        }
    }
}

/// `Broadcast.toSelfAndKnownPlayersInRadius(player, new MagicSkillUse(...))`:
/// the shot's `<skills>` (NORMAL) entries as a self-targeted, zero-time
/// `MagicSkillUse` — the client renders the charge glow off it.
fn broadcast_shot_visual(world: &mut World, object_id: i32, skills: &[(i32, i32)]) {
    let Some((player, pos)) = ({
        let p = world.objects.get_component::<crate::model::Player>(&object_id).cloned();
        let pos = world.objects.get_component::<crate::model::components::Position>(&object_id).copied();
        p.zip(pos)
    }) else {
        return;
    };
    for &(skill_id, skill_level) in skills {
        let pkt = server_packets::magic_skill_use(
            &player,
            &pos,
            (object_id, pos.x, pos.y, pos.z),
            skill_id,
            skill_level,
            0,
            0,
            0,
        );
        crate::game_loop::helpers::broadcast_including_self(world, object_id, &pkt);
    }
}

/// Port of `handlers/itemhandlers/ItemSkillsTemplate.useItem` (potions, buff
/// scrolls, …): casts each of the item's `<skills>` entries immediately
/// (Java's `SkillCaster.triggerCast` path — no cast bar, no MP/HP cost; items
/// never carry either), then consumes the item once at least one skill
/// landed. Narrowing: no pets (none exist yet), no Olympiad guard (no
/// Olympiad), no `<cond>` gating (not parsed for items — see `item_data`'s
/// header comment), and every use is treated as consume-on-success — Java's
/// `checkConsume` only withholds consumption for the `itemConsumeId`/
/// `SKILL_REDUCE_ON_SKILL_SUCCESS` combo, which needs a skill-side item-
/// consume effect this port doesn't have yet.
///
/// TODO(G15): Java only takes the instant `triggerCast` path when
/// `itemSkill.isWithoutAction()` or the item has `immediate_effect`/
/// `ex_immediate_effect`; everything else falls through to
/// `playable.useMagic(itemSkill, item, ...)` — a **real cast with a cast bar,
/// interruptible by damage**. This function always takes the instant path, so
/// e.g. a Scroll of Escape (item 736 → skill 2013, `hitTime` 20000, no
/// `immediate_effect`, not `isWithoutAction`) teleports the moment it is
/// double-clicked instead of after a 20 s interruptible cast. Closing the gap
/// needs `immediate_effect`/`ex_immediate_effect` parsed in `item_data`,
/// `isWithoutAction` parsed in `skill_data`, and the non-instant branch routed
/// through `cast::start_casting` (plus moving consumption to the skill's
/// `itemConsumeId`/`itemConsumeCount` at landing rather than up front here).
fn use_item_skills(world: &mut World, client_id: u32, object_id: i32, item_object_id: i32) {
    use crate::game_loop::skills::cast::{check_skill_reuse, resolve_cast_target, set_skill_reuse};
    use crate::game_loop::skills::effects::apply_skill_effects;
    use crate::model::components::{Position, TargetRef};
    use crate::model::skill::TargetType;
    use crate::model::Player;

    let item_skills = {
        let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else { return };
        let Some(item) = inventory.items().iter().find(|i| i.object_id == item_object_id) else { return };
        let Some(template) = world.data.item_data.get(item.item_id) else { return };
        template.item_skills.clone()
    };
    if item_skills.is_empty() {
        return;
    }

    let mut used = false;
    for (skill_id, skill_level) in item_skills {
        let Some(skill) = world.data.skill_data.get(skill_id, skill_level).cloned() else { continue };
        if !check_skill_reuse(world, client_id, object_id, &skill) {
            continue;
        }
        let target_oid = match skill.target_type {
            TargetType::Self_ => object_id,
            _ => {
                let Some(player) = world.objects.get_component::<Player>(&object_id) else { continue };
                let Some(pos) = world.objects.get_component::<Position>(&object_id).copied() else { continue };
                let target_ref = world.objects.get_component::<TargetRef>(&object_id).copied().unwrap_or_default().0;
                match resolve_cast_target(world, player, &pos, target_ref, &skill, true) {
                    Ok(oid) => oid,
                    Err(_) => continue,
                }
            }
        };
        apply_skill_effects(world, object_id, target_oid, &skill);
        set_skill_reuse(world, object_id, &skill);
        used = true;
    }

    if used {
        destroy_used_item(world, client_id, object_id, item_object_id);
    }
}

/// Destroys one unit of a used etc item and notifies the client — the
/// consume tail shared by `ExtractableItems` and `ItemSkills`.
fn destroy_used_item(world: &mut World, client_id: u32, object_id: i32, item_object_id: i32) {
    let Some(destroyed) = ({
        let Some(inventory) = world.objects.get_component_mut::<Inventory>(&object_id) else { return };
        inventory.remove_by_object_id(item_object_id, 1)
    }) else {
        return;
    };
    // Memory-first: the count decrement / removal already applied to the
    // `Inventory` component; it persists on the next flush.
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(ew::inventory_update_changes(&world.data, std::slice::from_ref(&destroyed)));
    }
}

/// Port of `handlers/itemhandlers/ExtractableItems.useItem`: destroys the
/// used item, then rolls its `<capsuled_items>` list and grants what hits.
/// `extractableCountMin == 0` (every currently-loaded pack/box item) takes a
/// single pass over the list; `> 0` re-rolls the whole list until at least
/// that many entries have been granted, mirroring Java's `while` loop (used
/// by "pick one of N" reward boxes) — capped at a generous iteration count
/// so a misconfigured item (chances that can never sum to the minimum)
/// can't hang the single-threaded game loop the way it could a Java
/// per-client thread. Per-entry enchant rolls are skipped (later milestone;
/// nothing currently loaded needs them).
fn extract_item(world: &mut World, client_id: u32, object_id: i32, item_object_id: i32) {
    let (capsules, count_min, count_max) = {
        let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else { return };
        let Some(item) = inventory.items().iter().find(|i| i.object_id == item_object_id) else { return };
        let Some(template) = world.data.item_data.get(item.item_id) else { return };
        (template.capsuled_items.clone(), template.extractable_count_min.max(0), template.extractable_count_max)
    };
    if capsules.is_empty() {
        return;
    }

    // Port of `Player.isInventoryUnder80(false)`, the gate
    // `ExtractableItems.useItem` checks before touching the item: refuse
    // (leaving the box and inventory untouched) if the bag is already too
    // full for the reward roll to have anywhere to go.
    let race = world.objects.get_component::<crate::model::Player>(&object_id).map(|p| p.race).unwrap_or(0);
    let normal_limit = world.cfg.character.inventory_limit(race);
    let under_80 = world
        .objects
        .get_component::<Inventory>(&object_id)
        .is_some_and(|inv| inv.is_under_80_percent(&world.data.item_data, normal_limit));
    if !under_80 {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(sm_ids::YOUR_INVENTORY_IS_FULL, &[]));
        }
        return;
    }

    destroy_used_item(world, client_id, object_id, item_object_id);

    let mut granted: Vec<(i32, i64)> = Vec::new();
    for _ in 0..1000 {
        for product in &capsules {
            if count_max > 0 && granted.len() as i32 >= count_max {
                break;
            }
            if world.roll(100_000) > product.chance {
                continue;
            }
            let span = (product.max - product.min + 1).max(1) as i32;
            let amount = if product.max == product.min { product.min } else { product.min + world.roll(span) as i64 };
            if amount != 0 {
                granted.push((product.item_id, amount));
            }
        }
        if granted.len() as i32 >= count_min {
            break;
        }
    }

    if granted.is_empty() {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(sm_ids::THERE_WAS_NOTHING_FOUND_INSIDE, &[]));
        }
        return;
    }

    for (item_id, amount) in granted {
        let Some(changed_oids) = add_inventory_item(world, object_id, item_id, amount) else {
            warn!("ExtractableItems: object-id pool exhausted, dropping {item_id}x{amount}");
            continue;
        };
        let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else { continue };
        if let Some(cs) = world.clients.get(&client_id) {
            let sm = if amount > 1 {
                server_packets::system_message_with(sm_ids::YOU_HAVE_OBTAINED_S2_S1, &[SmParam::ItemName(item_id), SmParam::Long(amount)])
            } else {
                server_packets::system_message_with(sm_ids::YOU_HAVE_OBTAINED_S1, &[SmParam::ItemName(item_id)])
            };
            cs.send(sm);
            cs.send(ew::inventory_update(inventory, &world.data, &changed_oids));
        }
    }
}

