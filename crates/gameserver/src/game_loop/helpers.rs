//! Small send/broadcast/range helpers shared by the packet handlers.

use crate::model::components::Position;
use crate::model::inventory::Inventory;
use crate::network::server_packets;
use crate::world::World;

/// The client id of the in-game session linked to a `Player`, or `None` if
/// they've disconnected since the task was scheduled (dead-id ⇒ no-op, per
/// the scheduler's contract).
///
/// O(1): [`crate::session::ClientTable`] keeps the object-id → client-id
/// reverse index. This used to scan every connected session.
pub(crate) fn client_for_player(world: &World, player_object_id: i32) -> Option<u32> {
    world.clients.client_of_player(player_object_id)
}

/// The object id of the player driven by `client_id`, or `None` when that
/// session is not `InGame` (still logging in, in the lobby, or already gone).
///
/// The inverse of [`client_for_player`], and the first line of nearly every
/// packet handler — Java reaches the same state through `GameClient.getPlayer()`.
pub(crate) fn player_of(world: &World, client_id: u32) -> Option<i32> {
    match world.clients.get(&client_id) {
        Some(crate::session::ClientSession::InGame(s)) => Some(s.player_object_id()),
        _ => None,
    }
}

/// The world coordinates of any object carrying a [`Position`], or `None` if
/// it has despawned.
pub(crate) fn pos_of(world: &World, object_id: i32) -> Option<(i32, i32, i32)> {
    world
        .objects
        .get_component::<Position>(&object_id)
        .map(|p| (p.x, p.y, p.z))
}

/// Send one packet to a connected client — Java `GameClient.sendPacket`.
///
/// A direct `clients` lookup. Prefer this over [`send_to_player`] whenever the
/// handler already holds the client id, which packet handlers always do.
pub(crate) fn send_to_client(world: &World, client_id: u32, packet: Vec<u8>) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
}

/// Send one packet to the client driving `player_object_id` — Java
/// `Player.sendPacket`. No-op when that player is offline.
///
/// Keyed by **object id**, resolved through [`client_for_player`]'s reverse
/// index. Both this and [`send_to_client`] are O(1) now; prefer the latter
/// when the client id is already in hand, and reach for this when all you have
/// is the object id (scheduled tasks, effects resolved against a target).
pub(crate) fn send_to_player(world: &World, player_object_id: i32, packet: Vec<u8>) {
    if let Some(cid) = client_for_player(world, player_object_id) {
        send_to_client(world, cid, packet);
    }
}

/// `SystemMessage` to a connected client. Pass `&[]` for a message with no
/// substitution parameters.
pub(crate) fn send_sm_to_client(
    world: &World,
    client_id: u32,
    message_id: i16,
    params: &[server_packets::SmParam],
) {
    send_to_client(
        world,
        client_id,
        server_packets::system_message_with(message_id, params),
    );
}

/// `SystemMessage` to a player by object id — the scanning counterpart of
/// [`send_sm_to_client`]. Pass `&[]` when the message takes no parameters.
pub(crate) fn send_sm_to_player(
    world: &World,
    player_object_id: i32,
    message_id: i16,
    params: &[server_packets::SmParam],
) {
    send_to_player(
        world,
        player_object_id,
        server_packets::system_message_with(message_id, params),
    );
}

/// A **bare** `SystemMessage` — one that takes no substitution parameters — to
/// a connected client. Most system messages are bare, so this saves the `&[]`
/// at the call site; reach for [`send_sm_to_client`] when there are params.
pub(crate) fn send_sm_bare_to_client(world: &World, client_id: u32, message_id: i16) {
    send_sm_to_client(world, client_id, message_id, &[]);
}

/// A bare `SystemMessage` to a player by object id — the object-id counterpart
/// of [`send_sm_bare_to_client`].
pub(crate) fn send_sm_bare_to_player(world: &World, player_object_id: i32, message_id: i16) {
    send_sm_to_player(world, player_object_id, message_id, &[]);
}

/// How much adena `object_id` is carrying — Java `Inventory.getAdena`. Zero for
/// anything with no [`Inventory`] at all, which is what every caller wants.
pub(crate) fn adena(world: &World, object_id: i32) -> i64 {
    world
        .objects
        .get_component::<Inventory>(&object_id)
        .map_or(0, |inv| inv.adena())
}

/// Java `Player.sendInventoryUpdate`: an `InventoryUpdate` never travels alone —
/// it's always followed by the adena counter (`ExAdenaInvenCount`) and the
/// weight bar (`ExUserInfoInvenWeight`), so any inventory change refreshes both.
/// Ported paths that only sent the bare `InventoryUpdate` left the adena display
/// stale (e.g. `//create_coin Adena`). `iu` is the already-built InventoryUpdate.
pub(crate) fn send_inventory_update(world: &World, client_id: u32, object_id: i32, iu: Vec<u8>) {
    let max_load = crate::game_loop::weight::max_load(world, object_id);
    let extras = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&object_id)
        .map(|inv| {
            (
                crate::network::enter_world::ex_adena_inven_count(inv),
                crate::network::enter_world::ex_user_info_inven_weight(
                    object_id,
                    inv,
                    &world.data,
                    max_load,
                ),
            )
        });
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(iu);
        if let Some((adena, weight)) = extras {
            cs.send(adena);
            cs.send(weight);
        }
    }
}

/// The full `SkillList` packet for an in-world player — their skill book plus
/// any transiently-granted clan skills (Java `sendSkillList`). `None` when the
/// object carries no skill book (not a live player). The single funnel every
/// `SkillList` resend goes through, so clan skills never fall off the list.
pub(crate) fn skill_list_packet(world: &World, object_id: i32) -> Option<Vec<u8>> {
    use crate::model::components::{ClanSkills, OptionSkills, SkillBook, SkillEnchants};
    let book = world.objects.get_component::<SkillBook>(&object_id)?;
    let empty = ClanSkills::default();
    let clan = world
        .objects
        .get_component::<ClanSkills>(&object_id)
        .unwrap_or(&empty);
    let no_options = OptionSkills::default();
    let options = world
        .objects
        .get_component::<OptionSkills>(&object_id)
        .unwrap_or(&no_options);
    let no_enchants = SkillEnchants::default();
    let enchants = world
        .objects
        .get_component::<SkillEnchants>(&object_id)
        .unwrap_or(&no_enchants);
    Some(crate::network::enter_world::skill_list(
        book,
        enchants,
        clan,
        options,
        &world.data,
    ))
}

/// Send a fresh `EtcStatusUpdate` to one player, built from their current state
/// (expertise grade penalties + silence/message-refusal), mirroring Java's
/// `sendPacket(new EtcStatusUpdate(this))` which reads it all off the player.
/// This is what redraws the grade-penalty and chat-block icons.
pub(crate) fn send_etc_status_update(world: &World, client_id: u32, object_id: i32) {
    use crate::model::components::{AdminFlags, ExpertisePenalty};
    let ep = world
        .objects
        .get_component::<ExpertisePenalty>(&object_id)
        .copied()
        .unwrap_or_default();
    // Java `EtcStatusUpdate._mask` bit 0x01 = message-refusal OR chat-ban OR
    // silence; the chat-block icon is the union.
    let silence = world
        .objects
        .get_component::<AdminFlags>(&object_id)
        .is_some_and(|f| f.silence)
        || super::punishment::is_chat_banned(world, object_id);
    let charges = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .map_or(0, |p| p.charges);
    let wp = world
        .objects
        .get_component::<crate::model::components::WeightPenalty>(&object_id)
        .map_or(0, |w| w.level);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(crate::network::enter_world::etc_status_update(
            charges, wp, ep.weapon, ep.armor, silence,
        ));
    }
}

/// Send `packet` to every in-game player that can see `from_object_id`,
/// excluding the broadcaster — Java `Creature.broadcastPacket(packet)` via
/// The instance (world partition) an object is in (Java
/// `WorldObject.getInstanceId()`) — 0, the overworld, when uninstanced.
pub(crate) fn instance_of(world: &World, object_id: i32) -> i32 {
    world
        .objects
        .get_component::<crate::model::components::InstanceId>(&object_id)
        .map_or(0, |i| i.0)
}

/// `World.forEachVisibleObject`: only players whose world region is in the
/// broadcaster's 3×3 surrounding-region block **and same instance** receive it.
pub(crate) fn broadcast_to_others(world: &World, from_object_id: i32, packet: &[u8]) {
    // The packet is copied into `Bytes` once and refcounted from there, instead
    // of `to_vec()`-ing it per recipient — a crowded region turned one
    // broadcast into dozens of allocations on the game thread.
    broadcast_to_others_shared(world, from_object_id, bytes::Bytes::copy_from_slice(packet));
}

/// [`broadcast_to_others`] for a payload already in `Bytes` —
/// `broadcast_including_self` shares one buffer between the self-send and the
/// onlookers instead of copying the packet twice.
fn broadcast_to_others_shared(world: &World, from_object_id: i32, shared: bytes::Bytes) {
    use crate::model::components::RegionCell;
    let Some(from) = world.objects.get_component::<RegionCell>(&from_object_id) else {
        return;
    };
    let from_region = from.0;
    let from_instance = instance_of(world, from_object_id);
    // The 3×3 block *is* the recipient set, so walk the region index rather
    // than every connected client. Indexed players without a session (the
    // unattended shops) simply resolve to no client and are skipped, which is
    // what the old session scan did by never seeing them.
    for other_id in world.players_visible_from(from_region) {
        if other_id == from_object_id {
            continue;
        }
        if instance_of(world, other_id) != from_instance {
            continue;
        }
        if let Some(cs) = world
            .clients
            .client_of_player(other_id)
            .and_then(|cid| world.clients.get(&cid))
        {
            cs.send(shared.clone());
        }
    }
}

/// Send `packet` to every in-game player in `instance` whose region cell is
/// adjacent to `region` — the broadcast shape for NPC-originated packets (Java
/// `Npc.broadcastPacket`; NPCs never hold a session, so there is no self/others
/// split), scoped to the source's instance so instanced content stays private
/// (G27). `broadcast_near_region` is this with the overworld (instance 0).
pub(crate) fn broadcast_near_region_in(
    world: &World,
    region: (i32, i32),
    instance: i32,
    packet: &[u8],
) {
    // One `Bytes` for the whole block; see `broadcast_to_others`.
    let shared = bytes::Bytes::copy_from_slice(packet);
    for oid in world.players_visible_from(region) {
        if instance_of(world, oid) != instance {
            continue;
        }
        if let Some(cs) = world
            .clients
            .client_of_player(oid)
            .and_then(|cid| world.clients.get(&cid))
        {
            cs.send(shared.clone());
        }
    }
}

/// [`broadcast_near_region_in`] fixed to the overworld (instance 0) — the shape
/// for NPC packets that only ever originate in the open world (boats, fishing,
/// cursed weapons, town social actions, …).
pub(crate) fn broadcast_near_region(world: &World, region: (i32, i32), packet: &[u8]) {
    broadcast_near_region_in(world, region, 0, packet);
}

/// Round a millisecond duration up to whole 100 ms ticks.
pub(crate) fn ms_to_ticks(ms: i32) -> u64 {
    (ms.max(0) as u64).div_ceil(100)
}

/// Send a `SystemMessage` + `ActionFailed` to one client — the standard
/// "request rejected" reply shape all over `Player.useMagic` /
/// `SkillCaster.checkUseConditions`.
pub(crate) fn send_sm_and_action_failed(
    world: &World,
    client_id: u32,
    message_id: i16,
    params: &[server_packets::SmParam],
) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::system_message_with(message_id, params));
        cs.send(server_packets::action_failed());
    }
}

/// `npc.broadcastPacket(new NpcSay(npc, NPC_GENERAL, npcStringId))` — an NPC
/// says a line to everyone nearby.
///
/// Lifted out of `QuestCtx` so a **boss script** can use it: the body only ever
/// needed the world and the speaker, and the quest coupling was incidental.
/// `QuestCtx::npc_say` now delegates here.
pub(crate) fn npc_say(world: &World, npc_oid: i32, npc_string_id: i32) {
    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
    else {
        return;
    };
    let Some(region) = world
        .objects
        .get_component::<crate::model::components::RegionCell>(&npc_oid)
        .map(|r| r.0)
    else {
        return;
    };
    let pkt = crate::network::server_packets::npc_say(npc_oid, npc.npc_id, npc_string_id);
    broadcast_near_region(world, region, &pkt);
}

/// `npc.broadcastSay(NPC_GENERAL, text)` — a literal-text chat bubble.
pub(crate) fn npc_say_text(world: &World, npc_oid: i32, text: &str) {
    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
    else {
        return;
    };
    let Some(region) = world
        .objects
        .get_component::<crate::model::components::RegionCell>(&npc_oid)
        .map(|r| r.0)
    else {
        return;
    };
    let pkt = crate::network::server_packets::npc_say_text(npc_oid, npc.npc_id, text);
    broadcast_near_region(world, region, &pkt);
}

/// Send `packet` to a player's own client (if still connected) and every
/// player that can see them — Java `Creature.broadcastPacket(packet)` with
/// `includeSelf == true`.
pub(crate) fn broadcast_including_self(world: &World, object_id: i32, packet: &[u8]) {
    // One `Bytes` for the mover and every onlooker alike.
    let shared = bytes::Bytes::copy_from_slice(packet);
    if let Some(client_id) = client_for_player(world, object_id)
        && let Some(cs) = world.clients.get(&client_id)
    {
        cs.send(shared.clone());
    }
    broadcast_to_others_shared(world, object_id, shared);
}

/// Fire the held-back action — the tail of Java `SkillCaster.stopCasting`
/// (queued skill → `useMagic`, else `EVT_FINISH_CASTING` → the saved MOVE_TO)
/// and of `EVT_READY_TO_ACT` at swing end. Each replay re-enters the normal
/// handler pipeline, so it re-validates everything exactly like a fresh
/// click. No-op while still busy (casting or mid-swing) or dead — the slot
/// stays for the later stop.
pub(crate) fn run_queued_action(world: &mut World, object_id: i32) {
    use crate::model::components::{AttackState, Casting, Position, QueuedAction, Vitals};
    let Some(&action) = world.objects.get_component::<QueuedAction>(&object_id) else {
        return;
    };
    if world.objects.has_component::<Casting>(&object_id)
        || world
            .objects
            .get_component::<AttackState>(&object_id)
            .is_some_and(|st| st.attack_end_tick > world.tick)
        || world
            .objects
            .get_component::<Vitals>(&object_id)
            .is_some_and(|v| v.dead)
    {
        return;
    }
    world.objects.remove_component::<QueuedAction>(&object_id);
    let Some(client_id) = client_for_player(world, object_id) else {
        return;
    };
    match action {
        QueuedAction::Move { x, y, z } => {
            let Some(cur) = world.objects.get_component::<Position>(&object_id).copied() else {
                return;
            };
            crate::game_loop::position::intention_move_to(
                world,
                client_id,
                object_id,
                cur,
                (x, y, z),
            );
        }
        QueuedAction::Skill {
            skill_id,
            ctrl,
            shift,
        } => {
            crate::game_loop::skills::cast::use_magic(
                world, client_id, object_id, skill_id, ctrl, shift,
            );
        }
        QueuedAction::UseItem { item_object_id } => {
            crate::game_loop::items::use_equipable_item(
                world,
                client_id,
                object_id,
                item_object_id,
            );
        }
    }
}

/// Java `World.forEachVisibleObject(origin, Creature.class, …)` — every living
/// creature (player **or** NPC) in `origin`'s own region cell or an adjacent
/// one, excluding `origin` itself.
///
/// Java's "visible" is exactly this region-neighbourhood test; there is no
/// line-of-sight or radius term in `forEachVisibleObject`, so none is applied
/// here either. Callers that need a distance or LOS filter add it themselves.
///
/// This is the general neighbour query the `RandomizeHate` deferral in the
/// hate-effects slice was waiting on: `faction_call`'s scan only ever walked
/// NPCs, so a mob could never be pointed at a *player* it wasn't already
/// fighting.
pub(crate) fn visible_creatures(world: &mut World, origin_object_id: i32) -> Vec<i32> {
    use crate::model::components::Vitals;
    let Some(origin) = world
        .objects
        .get_component::<crate::model::components::RegionCell>(&origin_object_id)
        .map(|r| r.0)
    else {
        return Vec::new();
    };
    // Both halves come from the region indexes. This used to sweep every
    // entity in the store — all ~34.9k NPCs — and discard the 99.9% that were
    // nowhere near the origin.
    let mut out: Vec<i32> = world
        .players_visible_from(origin)
        .chain(world.npcs_visible_from(origin))
        .filter(|&oid| {
            oid != origin_object_id
                && world
                    .objects
                    .get_component::<Vitals>(&oid)
                    .is_some_and(|v| !v.dead)
        })
        .collect();
    // Sorted so the caller's `Rnd.get(size)` index maps to a stable candidate.
    // Java's iteration order is arbitrary too, and a uniform index over a
    // sorted list is still uniform — but this makes a forced roll in tests
    // pick a *known* creature instead of whatever the ECS happened to yield.
    out.sort_unstable();
    out
}

/// Java `Player.setInventoryBlockingStatus(true)` — suppress inventory
/// refreshes for this player, and schedule the 1500 ms `InventoryEnableTask`
/// that lifts it.
///
/// Called wherever Java calls it: opening a merchant buy list, a private or
/// clan warehouse, and the "wear" (try-on) shop.
pub(crate) fn block_inventory(world: &mut World, object_id: i32) {
    world.inventory_blocked.insert(object_id);
    world.scheduler.schedule(
        world.tick + ms_to_ticks(1500),
        crate::scheduler::ScheduledTask::InventoryEnable { object_id },
    );
}
