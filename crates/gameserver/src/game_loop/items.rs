//! Gear equip/unequip handlers (`UseItem`, `RequestUnEquipItem`) and the
//! `EtcItem` "use" dispatch (`ExtractableItems` for pack/box items).

use tracing::warn;

use crate::data::item_data::ItemHandler;
use crate::db;
use crate::model::inventory::{Inventory, ItemChange};
use crate::network::client_packets as cp;
use crate::network::enter_world as ew;
use crate::network::server_packets::{self, sm_ids, SmParam};
use crate::session::ClientSession;
use crate::world::World;

/// The stack-or-create core of `Player.addItem`: merge into an existing
/// stack (persisting the new count) or allocate an object id and insert a
/// fresh instance. Returns the touched instance's object id; `None` only on
/// id-pool exhaustion. Shared by the auto-loot path (`death::give_item`) and
/// quest rewards (`quests`); the caller owns messaging/`InventoryUpdate`.
pub(crate) fn add_inventory_item(world: &mut World, player_oid: i32, item_id: i32, count: i64) -> Option<i32> {
    let stackable = world.data.item_data.get(item_id).map(|t| t.is_stackable).unwrap_or(false);
    let existing_stack = stackable
        .then(|| {
            world
                .objects
                .get_component::<crate::model::inventory::Inventory>(&player_oid)
                .and_then(|inv| inv.items().iter().find(|i| i.item_id == item_id).map(|i| i.object_id))
        })
        .flatten();

    if let Some(stack_oid) = existing_stack {
        let new_count = {
            let inv = world
                .objects
                .get_component_mut::<crate::model::inventory::Inventory>(&player_oid)
                .expect("checked");
            inv.add_item(&world.data.item_data, stack_oid, item_id, count);
            inv.items().iter().find(|i| i.object_id == stack_oid).map(|i| i.count).unwrap_or(count)
        };
        let _ = world.db.send(db::DbCommand::UpdateItemCount { object_id: stack_oid, count: new_count });
        Some(stack_oid)
    } else {
        let new_oid = world.alloc_object_id()?;
        let inv = world.objects.get_component_mut::<crate::model::inventory::Inventory>(&player_oid)?;
        inv.add_item(&world.data.item_data, new_oid, item_id, count);
        let _ = world.db.send(db::DbCommand::InsertItem { owner_id: player_oid, object_id: new_oid, item_id, count });
        Some(new_oid)
    }
}

/// Port of `clientpackets/UseItem.runImpl`: right-clicking a `Weapon`/`Armor`
/// toggles equip/unequip; anything else routes through the `EtcItem` handler
/// dispatch (Java: `ItemHandler.getInstance().getHandler(etcItem)`).
pub(crate) fn handle_use_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::UseItem::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
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
    let Some(item) = inventory.items().iter().find(|i| i.object_id == item_object_id) else { return };
    let Some(template) = catalog.get(item.item_id) else { return };
    let body_part = template.body_part;

    let changed = if inventory.paperdoll_slot_of(item_object_id).is_some() {
        inventory.unequip_body_part(body_part)
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

/// Shared tail of the equip/unequip handlers: persist each changed slot
/// (`items.loc`/`loc_data`), then resend `InventoryUpdate` + `UserInfo` (Java:
/// `sendInventoryUpdate` + `broadcastUserInfo`).
pub(crate) fn finish_equip_change(world: &mut World, client_id: u32, object_id: i32, changed: &[i32]) {
    if changed.is_empty() {
        return;
    }
    let Some(inventory) = world.objects.get_component::<crate::model::inventory::Inventory>(&object_id) else {
        return;
    };
    for &oid in changed {
        let (loc, loc_data) = match inventory.paperdoll_slot_of(oid) {
            Some(slot) => ("PAPERDOLL", slot as i32),
            None => ("INVENTORY", 0),
        };
        let _ = world.db.send(db::DbCommand::UpdateItemLocation { object_id: oid, loc, loc_data });
    }
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(crate::network::enter_world::inventory_update(inventory, &world.data, changed));
        if let Some(v) = crate::model::PlayerView::of(&world.objects, object_id) {
            cs.send(crate::network::user_info::user_info(&v, &world.data));
        }
    }
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
        ItemHandler::None => {}
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
    match &destroyed {
        ItemChange::Modified(item) => {
            let _ = world.db.send(db::DbCommand::UpdateItemCount { object_id: item.object_id, count: item.count });
        }
        ItemChange::Removed(item) => {
            let _ = world.db.send(db::DbCommand::DeleteItem { object_id: item.object_id });
        }
    }
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
/// per-client thread. Inventory-capacity (`isInventoryUnder80`) and
/// per-entry enchant rolls are skipped (later milestone; nothing currently
/// loaded needs either).
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
        let Some(changed_oid) = add_inventory_item(world, object_id, item_id, amount) else {
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
            cs.send(ew::inventory_update(inventory, &world.data, &[changed_oid]));
        }
    }
}

