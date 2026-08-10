//! Ground items (`ItemsOnGroundManager`): items lying in the world as entities
//! with [`GroundItem`]/[`Position`]/[`RegionCell`], indexed in
//! `World::ground_item_regions`. Created by a player drop (`RequestDropItem`) or
//! monster death with auto-loot off, made visible to players entering the
//! region (`SpawnItem`, via `visibility`), and picked up by a click (`Action` →
//! [`pickup_ground_item`]).

use crate::game_loop::guard::position;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::region_cell_of;
use crate::game_loop::helpers::send_action_failed;
use crate::game_loop::helpers::send_to_client;
use crate::model::components::{GroundItem, Position, RegionCell};
use crate::model::inventory::Inventory;
use crate::network::client_packets as cp;
use crate::network::server_packets::{self, GroundItemView};
use crate::scheduler::ScheduledTask;
use crate::world::{World, region_of};

/// `ItemTemplate.TYPE2_QUEST` — the `type2` value quest-typed templates carry.
const TYPE2_QUEST: i32 = 3;

/// `RequestDropItem`'s `isInsideRadius2D(_x, _y, 0, 150)` — how far from the
/// player the client may ask for the item to land, in 2D.
const DROP_RADIUS: i64 = 150;

/// `RequestDropItem`'s `Math.abs(_z - player.getZ()) > 50` — the vertical
/// tolerance on that same request, so a drop can't be aimed off a ledge.
const DROP_MAX_Z_DIFF: i32 = 50;

/// `ItemData.createItem("loot")`'s ordinary drop protection: 15 s (150 ticks)
/// during which only the killer and their party may pick the stack up.
///
/// Raid drops use `RaidLootRightsInterval` instead — a config value, and a
/// different owner (the privileged command channel's leader) — so they pass
/// their own window to [`reserve_for`] rather than this.
pub(crate) const LOOT_PROTECTION_TICKS: u64 = 150;

/// Reserve a freshly dropped stack for `owner_oid` for `ticks`.
///
/// A no-op when `owner_oid` is 0 — Java's "owned by nobody", which is what a
/// raid drop with no active command-channel claim gets, and which must not be
/// confused with "owned by object 0".
pub(crate) fn reserve_for(world: &mut World, ground_oid: i32, owner_oid: i32, ticks: u64) {
    if owner_oid == 0 {
        return;
    }
    let until = world.tick + ticks;
    if let Some(g) = world.objects.get_component_mut::<GroundItem>(&ground_oid) {
        g.owner_id = owner_oid;
        g.owner_until_tick = until;
    }
}

/// Who dropped a ground item — Java gates auto-destroy differently for the two
/// (`Player.dropItem` vs `Npc.dropItem`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DropSource {
    /// A player's `RequestDropItem`: only auto-destroyed when
    /// `DestroyPlayerDroppedItem` is set (+ the equipable sub-gate).
    Player,
    /// An NPC death drop with auto-loot off: auto-destroyed whenever
    /// `AutoDestroyDroppedItemAfter > 0`.
    Npc,
    /// A cursed weapon dropped by a slain monster: Java calls
    /// `_item.setDropTime(0)` to exempt it from `ItemsAutoDestroy` — the
    /// `CursedWeapon` remove-task owns its lifetime instead.
    CursedWeapon,
}

/// The auto-destroy delay (seconds) for a freshly dropped item, or `None` when
/// it should never be scheduled — the port of the `ItemsAutoDestroyTaskManager.
/// addItem` gates in `Player.dropItem` / `Npc.dropItem`.
///
/// **Herbs run their own clock.** Java's gate is
/// `((AUTODESTROY_ITEM_AFTER > 0) && !hasExImmediateEffect()) ||
/// ((HERB_AUTO_DESTROY_TIME > 0) && hasExImmediateEffect())` — an *either/or*,
/// so a herb is scheduled off `AutoDestroyHerbTime` (60 s) whether or not the
/// ordinary destroyer is on, and never off the 600 s one.
fn auto_destroy_delay(world: &World, item_id: i32, source: DropSource) -> Option<u64> {
    let g = &world.cfg.general;
    if g.protected_items.contains(&item_id) {
        return None;
    }
    // `hasExImmediateEffect()` — the herb flag.
    let herb = world
        .data
        .item_data
        .get(item_id)
        .is_some_and(|t| t.ex_immediate_effect);
    let delay = if herb {
        g.herb_auto_destroy_time
    } else {
        g.autodestroy_item_after
    };
    if delay == 0 {
        return None;
    }
    match source {
        DropSource::CursedWeapon => None,
        DropSource::Npc => Some(delay),
        DropSource::Player => {
            if !g.destroy_dropped_player_item {
                return None;
            }
            let equipable = world
                .data
                .item_data
                .get(item_id)
                .is_some_and(|t| t.is_equipable());
            if equipable && !g.destroy_equipable_player_item {
                return None;
            }
            Some(delay)
        }
    }
}

/// Build a ground item's wire view (display id = item id — no disguised items;
/// stackable from the template).
pub(crate) fn ground_item_view(world: &World, oid: i32) -> Option<GroundItemView> {
    let g = world.objects.get_component::<GroundItem>(&oid)?;
    let pos = world.objects.get_component::<Position>(&oid)?;
    let stackable = world
        .data
        .item_data
        .get(g.item_id)
        .map(|t| t.is_stackable)
        .unwrap_or(false);
    Some(GroundItemView {
        object_id: g.object_id,
        display_id: g.item_id,
        x: pos.x,
        y: pos.y,
        z: pos.z,
        stackable,
        count: g.count,
        enchant: g.enchant,
    })
}

/// Drop an item into the world at `(x, y, z)` and broadcast the toss animation
/// (`DropItem` from `dropper_oid`). Returns the ground item's object id.
pub(crate) fn spawn_ground_item(
    world: &mut World,
    item_id: i32,
    count: i64,
    enchant: i32,
    x: i32,
    y: i32,
    z: i32,
    dropper_oid: i32,
    source: DropSource,
) -> i32 {
    let object_id = world.next_npc_object_id;
    world.next_npc_object_id += 1;
    let region = region_of(x, y);
    world
        .ground_item_regions
        .entry(region)
        .or_default()
        .push(object_id);
    world.objects.spawn(
        object_id,
        (
            GroundItem {
                object_id,
                item_id,
                count,
                enchant,
                owner_id: 0,
                owner_until_tick: 0,
            },
            Position {
                x,
                y,
                z,
                heading: 0,
            },
            RegionCell(region),
        ),
    );
    if let Some(view) = ground_item_view(world, object_id) {
        super::helpers::broadcast_near_region(
            world,
            region,
            &server_packets::drop_item(dropper_oid, &view),
        );
    }
    // Auto-destroy scheduling, gated by General.ini: a player's drop persists
    // unless `DestroyPlayerDroppedItem` is set (dist default: off), while an NPC
    // drop decays after `AutoDestroyDroppedItemAfter`.
    if let Some(delay_secs) = auto_destroy_delay(world, item_id, source) {
        world.scheduler.schedule(
            world.tick + delay_secs * 10,
            ScheduledTask::GroundItemDecay {
                item_object_id: object_id,
            },
        );
    }
    object_id
}

/// `ItemsOnGroundManager` cleanup task: remove a ground item that has lain past
/// its lifetime (no-op if it was already picked up).
pub(crate) fn handle_ground_item_decay(world: &mut World, item_object_id: i32) {
    let Some(region) = region_cell_of(world, item_object_id) else {
        return;
    };
    if !world.objects.has_component::<GroundItem>(&item_object_id) {
        return;
    }
    despawn_ground_item(world, item_object_id, region);
}

/// Remove a ground item from the world (despawn + drop from the region index +
/// `DeleteObject` to nearby).
pub(crate) fn despawn_ground_item(world: &mut World, item_oid: i32, region: (i32, i32)) {
    world.objects.despawn(&item_oid);
    if let Some(ids) = world.ground_item_regions.get_mut(&region) {
        ids.retain(|&id| id != item_oid);
    }
    super::helpers::broadcast_near_region(world, region, &server_packets::delete_object(item_oid));
}

/// `Player.doPickupItem`: pick a ground item up into `player_oid`'s inventory —
/// the pickup animation to nearby, remove from the world, and add to inventory
/// with the "you obtained" message + `InventoryUpdate`.
///
/// The **enchant level survives the round trip**. Java gets that for free —
/// drop and pickup move one `Item` instance between containers — while this
/// port mints a fresh instance on the give path, so `GroundItem::enchant` is
/// passed across explicitly. Without it a dropped `+7` weapon came back `+0`.
pub(crate) fn pickup_ground_item(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    item_oid: i32,
) {
    // `CreatureAI.onIntentionPickUp`'s REST branch. The click path enforces it
    // up front in `combat::start_pickup_intent` (where Java has it); repeating
    // it here covers the callers that reach `doPickupItem` without an
    // intention — auto-play looting — and the case where the player sits down
    // mid-walk. Loot stays on the floor until they stand.
    if super::sit_stand::is_resting(world, player_oid) {
        send_action_failed(world, client_id);
        return;
    }
    let Some(g) = world
        .objects
        .get_component::<GroundItem>(&item_oid)
        .cloned()
    else {
        return;
    };
    let Some(pos) = position(world, item_oid) else {
        return;
    };
    let region = region_cell_of(world, item_oid).unwrap_or_else(|| region_of(pos.x, pos.y));
    // A cursed weapon lying on the ground curses whoever grabs it — route into
    // the cursed-weapon pickup path (its own get-item broadcast + despawn +
    // activation) instead of the plain give.
    if super::cursed_weapon::is_dropped_cursed(world, g.item_id) {
        super::cursed_weapon::try_pickup(
            world, client_id, player_oid, item_oid, region, g.item_id, pos,
        );
        return;
    }
    // Loot protection (`Player.doPickupItem`): while owned, only the owner,
    // their party, or their command channel (raid drops) may take it.
    if g.owner_id != 0
        && world.tick < g.owner_until_tick
        && g.owner_id != player_oid
        && !super::command_channel::is_in_looter_party(world, player_oid, g.owner_id)
    {
        use crate::network::server_packets::{SmParam, sm_ids};
        let sm = if g.item_id == crate::data::item_data::ADENA_ID {
            server_packets::system_message_with(
                sm_ids::YOU_HAVE_FAILED_TO_PICK_UP_S1_ADENA,
                &[SmParam::Long(g.count)],
            )
        } else if g.count > 1 {
            server_packets::system_message_with(
                sm_ids::YOU_HAVE_FAILED_TO_PICK_UP_S2_S1_S,
                &[SmParam::ItemName(g.item_id), SmParam::Long(g.count)],
            )
        } else {
            server_packets::system_message_with(
                sm_ids::YOU_HAVE_FAILED_TO_PICK_UP_S1,
                &[SmParam::ItemName(g.item_id)],
            )
        };
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
            cs.send(sm);
        }
        return;
    }
    super::helpers::broadcast_near_region(
        world,
        region,
        &server_packets::get_item(player_oid, item_oid, pos.x, pos.y, pos.z),
    );
    despawn_ground_item(world, item_oid, region);
    super::quests::give_item_with_earned_message_enchanted(
        world, client_id, player_oid, g.item_id, g.count, g.enchant,
    );
    // Java `ON_PLAYER_ITEM_PICKUP` (the tutorial's Blue Gemstone listener).
    super::quests::notify_item_pickup(world, client_id, player_oid, g.item_id);
}

/// Port of `clientpackets/RequestDropItem.runImpl`: drop `count` of an
/// inventory item onto the ground **at the requested location**. Quest items
/// and items the datapack marks `is_dropable="false"` are protected; a worn
/// item is unequipped first.
///
/// The client sends where it wants the item to land (the cursor position when
/// the stack is dragged out of the inventory), and Java validates it twice:
/// here, the request must be within 150 units in 2D and 50 in z of the player
/// (else SM 151, "You cannot discard something that far away from you"), and
/// again in `Item.dropMe`, which runs the surviving point through
/// `GeoEngine.getValidLocation` so an item can never be thrown through a wall
/// or a closed door onto ground the player couldn't walk to. Standing inside a
/// `ConditionZone` with `NoItemDrop` (`no_drop_item.xml`) refuses outright.
pub(crate) fn handle_request_drop_item(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestDropItem::read(body) else {
        return;
    };
    let Some(player_oid) = world.player_oid(client_id) else {
        return;
    };
    // `(player == null) || player.isDead()`.
    if is_dead(world, player_oid) {
        return;
    }
    // Java's `_count < 0` branch punishes (`handleIllegalPlayerAction`);
    // `_count == 0` falls into the big refusal below. Neither may reach the
    // inventory.
    if pkt.count < 0 {
        super::punishment::illegal_action(
            world,
            player_oid,
            &format!(
                "[RequestDropItem] count < 0! player {player_oid} tried to drop item oid {}",
                pkt.object_id
            ),
        );
        return;
    }
    if pkt.count == 0 {
        return;
    }
    let Some((item_id, held, enchant, is_stackable, is_quest, dropable)) = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .and_then(|inv| {
            inv.by_object_id(pkt.object_id)
                .map(|it| (it.item_id, it.count, it.enchant_level))
        })
        .map(|(id, cnt, ench)| {
            let t = world.data.item_data.get(id);
            (
                id,
                cnt,
                ench,
                t.map(|t| t.is_stackable).unwrap_or(false),
                t.map(|t| t.is_quest_item).unwrap_or(false),
                t.map(|t| t.is_dropable()).unwrap_or(false),
            )
        })
    else {
        return;
    };
    let Some(ppos) = position(world, player_oid) else {
        return;
    };
    // `(item.getItemType() == EtcItemType.PET_COLLAR) && player.havePetInvItems()`
    // — a collar whose pet is still carrying things may not be thrown away;
    // the pet inventory would be stranded with no collar to reach it through.
    let loaded_collar = world.data.pet_data.is_pet_collar(item_id)
        && world
            .objects
            .get_component::<crate::model::inventory::PetInventory>(&player_oid)
            .is_some_and(|inv| !inv.0.items().is_empty());
    // Java's first (big OR) refusal: `!item.isDropable()` — bound reward boxes
    // (`is_dropable="false"`) never reach the ground — a loaded pet collar, or
    // standing inside `ZoneId.NO_ITEM_DROP`. All answer
    // `THAT_ITEM_CANNOT_BE_DISCARDED`. `_count > item.getCount()` refuses with
    // the same message rather than clamping, so a forged count cannot drop
    // more than is held.
    if !dropable
        || loaded_collar
        || pkt.count > held
        || world.data.zone_data.no_item_drop_at(ppos.x, ppos.y, ppos.z)
    {
        send_to_client(
            world,
            client_id,
            server_packets::system_message_with(
                server_packets::sm_ids::THAT_ITEM_CANNOT_BE_DISCARDED,
                &[],
            ),
        );
        return;
    }
    if is_quest || (!is_stackable && pkt.count > 1) {
        return;
    }
    // `Config.JAIL_DISABLE_TRANSACTION && player.isJailed()`.
    if world.cfg.general.jail_disable_transaction
        && world
            .objects
            .get_component::<crate::model::Player>(&player_oid)
            .is_some_and(|p| p.jailed)
    {
        super::items::send_item_message(world, client_id, "You cannot drop items in Jail.");
        return;
    }
    // `player.getPrivateStoreType() != PrivateStoreType.NONE` — a shop owner
    // may not discard out from under an in-flight sale.
    if world
        .objects
        .get_component::<crate::model::Player>(&player_oid)
        .is_some_and(|p| p.store_type != 0)
    {
        send_to_client(world, client_id, server_packets::system_message_with( server_packets::sm_ids::WHILE_OPERATING_A_PRIVATE_STORE_OR_WORKSHOP_YOU_CANNOT_DISCARD_DESTROY_OR_TRADE_AN_ITEM, &[], ));
        return;
    }
    // `player.isFishing()` — "You cannot do that while fishing."
    if world
        .objects
        .get_component::<crate::model::components::FishingSession>(&player_oid)
        .is_some_and(|f| f.is_fishing)
    {
        send_to_client(
            world,
            client_id,
            server_packets::system_message_with(
                server_packets::sm_ids::YOU_CANNOT_DO_THAT_WHILE_FISHING_2,
                &[],
            ),
        );
        return;
    }
    // `player.isFlying()` — a silent return in Java (no message on a wyvern).
    if world
        .objects
        .get_component::<crate::model::Player>(&player_oid)
        .is_some_and(|p| p.is_flying())
    {
        return;
    }
    // `player.hasItemRequest()` — an enchant window is open, so the inventory
    // is pinned until it resolves.
    if world
        .objects
        .has_component::<crate::model::components::EnchantRequest>(&player_oid)
    {
        send_to_client(world, client_id, server_packets::system_message_with( server_packets::sm_ids::YOU_CANNOT_DESTROY_OR_CRYSTALLIZE_ITEMS_WHILE_ENCHANTING_ATTRIBUTES, &[], ));
        return;
    }
    // `ItemTemplate.TYPE2_QUEST == item.getTemplate().getType2()` — a second,
    // wider quest gate than the `isQuestItem()` flag above: it catches the
    // quest-typed items whose template never sets that flag.
    if world
        .data
        .item_data
        .get(item_id)
        .is_some_and(|t| t.type2 == TYPE2_QUEST)
    {
        send_to_client(
            world,
            client_id,
            server_packets::system_message_with(
                server_packets::sm_ids::THAT_ITEM_CANNOT_BE_DISCARDED_OR_EXCHANGED,
                &[],
            ),
        );
        return;
    }
    // `!player.isInsideRadius2D(_x, _y, 0, 150) || (Math.abs(_z - player.getZ()) > 50)`.
    // Note Java's radius test is 2D and *inclusive* (`distance <= radius`),
    // while the z test is a strict `>`.
    let dx = (pkt.x - ppos.x) as i64;
    let dy = (pkt.y - ppos.y) as i64;
    if ((dx * dx) + (dy * dy)) > (DROP_RADIUS * DROP_RADIUS)
        || (pkt.z - ppos.z).abs() > DROP_MAX_Z_DIFF
    {
        send_to_client(
            world,
            client_id,
            server_packets::system_message_with(
                server_packets::sm_ids::YOU_CANNOT_DISCARD_SOMETHING_THAT_FAR_AWAY_FROM_YOU,
                &[],
            ),
        );
        return;
    }
    // "Do not drop items when casting known skills to avoid exploits." Java
    // walks the player's `SkillCaster`s and refuses if any of them is a skill
    // the character actually knows, then repeats the test for the *queued*
    // skill. The port holds one cast at a time, so one lookup covers both.
    // The message quotes the skill by name, as Java's does:
    // `"You cannot drop an item while casting " + skill.getName() + "."`.
    if let Some(casting) = world
        .objects
        .get_component::<crate::model::components::Casting>(&player_oid)
        && world
            .objects
            .get_component::<crate::model::components::SkillBook>(&player_oid)
            .is_some_and(|book| book.0.contains_key(&casting.0.skill_id))
    {
        // The fallback covers the 15 dist skills that declare `name=""` (and
        // an id that never parsed at all). Java would print its empty
        // `getName()` straight through — "…while casting ." — so this is a
        // deliberate cosmetic deviation, not a missing lookup.
        let text = match world.data.skill_data.name(casting.0.skill_id) {
            Some(name) => format!("You cannot drop an item while casting {name}."),
            None => "You cannot drop an item while casting.".to_string(),
        };
        super::items::send_item_message(world, client_id, &text);
        return;
    }
    let count = pkt.count;

    // Unequip first if worn (Java unequips before the drop, with its own update).
    if world
        .objects
        .get_component::<Inventory>(&player_oid)
        .is_some_and(|inv| inv.paperdoll_slot_of(pkt.object_id).is_some())
    {
        let changed = world
            .objects
            .get_component_mut::<Inventory>(&player_oid)
            .map(|inv| inv.unequip_item(pkt.object_id))
            .unwrap_or_default();
        super::items::finish_equip_change(world, client_id, player_oid, &changed);
    }

    let Some(change) = world
        .objects
        .get_component_mut::<Inventory>(&player_oid)
        .and_then(|inv| inv.remove_by_object_id(pkt.object_id, count))
    else {
        return;
    };
    super::helpers::send_inventory_update(world, player_oid, vec![change]);
    // `Item.dropMe` → `GeoEngine.getValidLocation(dropper, x, y, z)`: walk the
    // cell line from the dropper to the requested point and stop at the last
    // walkable cell, so the item lands short of a wall/closed door rather than
    // on the far side of it.
    let (dx, dy, dz) = world
        .geo
        .get_valid_location(ppos.x, ppos.y, ppos.z, pkt.x, pkt.y, pkt.z);
    spawn_ground_item(
        world,
        item_id,
        count,
        enchant,
        dx,
        dy,
        dz,
        player_oid,
        DropSource::Player,
    );
}
