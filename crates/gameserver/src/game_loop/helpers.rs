//! Small send/broadcast/range helpers shared by the packet handlers.

use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::World;

/// The client id of the in-game session linked to a `Player`, or `None` if
/// they've disconnected since the task was scheduled (dead-id ⇒ no-op, per
/// the scheduler's contract).
pub(crate) fn client_for_player(world: &World, player_object_id: i32) -> Option<u32> {
    world.clients.iter().find_map(|(&cid, cs)| match cs {
        ClientSession::InGame(s) if s.player_object_id() == player_object_id => Some(cid),
        _ => None,
    })
}

/// Java `Player.sendInventoryUpdate`: an `InventoryUpdate` never travels alone —
/// it's always followed by the adena counter (`ExAdenaInvenCount`) and the
/// weight bar (`ExUserInfoInvenWeight`), so any inventory change refreshes both.
/// Ported paths that only sent the bare `InventoryUpdate` left the adena display
/// stale (e.g. `//create_coin Adena`). `iu` is the already-built InventoryUpdate.
pub(crate) fn send_inventory_update(world: &World, client_id: u32, object_id: i32, iu: Vec<u8>) {
    let extras = world.objects.get_component::<crate::model::inventory::Inventory>(&object_id).map(|inv| {
        (
            crate::network::enter_world::ex_adena_inven_count(inv),
            crate::network::enter_world::ex_user_info_inven_weight(object_id, inv, &world.data),
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
    use crate::model::components::{ClanSkills, SkillBook, SkillEnchants};
    let book = world.objects.get_component::<SkillBook>(&object_id)?;
    let empty = ClanSkills::default();
    let clan = world.objects.get_component::<ClanSkills>(&object_id).unwrap_or(&empty);
    let no_enchants = SkillEnchants::default();
    let enchants = world.objects.get_component::<SkillEnchants>(&object_id).unwrap_or(&no_enchants);
    Some(crate::network::enter_world::skill_list(book, enchants, clan, &world.data))
}

/// Send a fresh `EtcStatusUpdate` to one player, built from their current state
/// (expertise grade penalties + silence/message-refusal), mirroring Java's
/// `sendPacket(new EtcStatusUpdate(this))` which reads it all off the player.
/// This is what redraws the grade-penalty and chat-block icons.
pub(crate) fn send_etc_status_update(world: &World, client_id: u32, object_id: i32) {
    use crate::model::components::{AdminFlags, ExpertisePenalty};
    let ep = world.objects.get_component::<ExpertisePenalty>(&object_id).copied().unwrap_or_default();
    let silence = world.objects.get_component::<AdminFlags>(&object_id).is_some_and(|f| f.silence);
    let charges = world.objects.get_component::<crate::model::Player>(&object_id).map_or(0, |p| p.charges);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(crate::network::enter_world::etc_status_update(charges, ep.weapon, ep.armor, silence));
    }
}

/// Send `packet` to every in-game player that can see `from_object_id`,
/// excluding the broadcaster — Java `Creature.broadcastPacket(packet)` via
/// `World.forEachVisibleObject`: only players whose world region is in the
/// broadcaster's 3×3 surrounding-region block receive it.
pub(crate) fn broadcast_to_others(world: &World, from_object_id: i32, packet: &[u8]) {
    use crate::model::components::RegionCell;
    let Some(from) = world.objects.get_component::<RegionCell>(&from_object_id) else { return };
    let from_region = from.0;
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            let other_id = s.player_object_id();
            if other_id == from_object_id {
                continue;
            }
            let Some(other) = world.objects.get_component::<RegionCell>(&other_id) else { continue };
            if crate::world::regions_adjacent(from_region, other.0) {
                cs.send(packet.to_vec());
            }
        }
    }
}

/// Send `packet` to every in-game player whose region cell is adjacent to
/// `region` — the broadcast shape for NPC-originated packets (Java
/// `Npc.broadcastPacket`; NPCs never hold a session, so there is no
/// self/others split).
pub(crate) fn broadcast_near_region(world: &World, region: (i32, i32), packet: &[u8]) {
    use crate::model::components::RegionCell;
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            let Some(p) = world.objects.get_component::<RegionCell>(&s.player_object_id()) else { continue };
            if crate::world::regions_adjacent(region, p.0) {
                cs.send(packet.to_vec());
            }
        }
    }
}

/// Round a millisecond duration up to whole 100 ms ticks.
pub(crate) fn ms_to_ticks(ms: i32) -> u64 {
    (ms.max(0) as u64).div_ceil(100)
}

/// Send a `SystemMessage` + `ActionFailed` to one client — the standard
/// "request rejected" reply shape all over `Player.useMagic` /
/// `SkillCaster.checkUseConditions`.
pub(crate) fn send_sm_and_action_failed(world: &World, client_id: u32, message_id: i16, params: &[server_packets::SmParam]) {
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
    let Some(npc) = world.objects.get_component::<crate::model::npc::Npc>(&npc_oid) else { return };
    let Some(region) = world.objects.get_component::<crate::model::components::RegionCell>(&npc_oid).map(|r| r.0)
    else {
        return;
    };
    let pkt = crate::network::server_packets::npc_say(npc_oid, npc.npc_id, npc_string_id);
    broadcast_near_region(world, region, &pkt);
}

/// Send `packet` to a player's own client (if still connected) and every
/// player that can see them — Java `Creature.broadcastPacket(packet)` with
/// `includeSelf == true`.
pub(crate) fn broadcast_including_self(world: &World, object_id: i32, packet: &[u8]) {
    if let Some(client_id) = client_for_player(world, object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(packet.to_vec());
        }
    }
    broadcast_to_others(world, object_id, packet);
}

/// Fire the held-back action — the tail of Java `SkillCaster.stopCasting`
/// (queued skill → `useMagic`, else `EVT_FINISH_CASTING` → the saved MOVE_TO)
/// and of `EVT_READY_TO_ACT` at swing end. Each replay re-enters the normal
/// handler pipeline, so it re-validates everything exactly like a fresh
/// click. No-op while still busy (casting or mid-swing) or dead — the slot
/// stays for the later stop.
pub(crate) fn run_queued_action(world: &mut World, object_id: i32) {
    use crate::model::components::{AttackState, Casting, Position, QueuedAction, Vitals};
    let Some(&action) = world.objects.get_component::<QueuedAction>(&object_id) else { return };
    if world.objects.has_component::<Casting>(&object_id)
        || world.objects.get_component::<AttackState>(&object_id).is_some_and(|st| st.attack_end_tick > world.tick)
        || world.objects.get_component::<Vitals>(&object_id).is_some_and(|v| v.dead)
    {
        return;
    }
    world.objects.remove_component::<QueuedAction>(&object_id);
    let Some(client_id) = client_for_player(world, object_id) else { return };
    match action {
        QueuedAction::Move { x, y, z } => {
            let Some(cur) = world.objects.get_component::<Position>(&object_id).copied() else { return };
            crate::game_loop::position::intention_move_to(world, client_id, object_id, cur, (x, y, z));
        }
        QueuedAction::Skill { skill_id, ctrl, shift } => {
            crate::game_loop::skills::cast::use_magic(world, client_id, object_id, skill_id, ctrl, shift);
        }
        QueuedAction::UseItem { item_object_id } => {
            crate::game_loop::items::use_equipable_item(world, client_id, object_id, item_object_id);
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
    use crate::model::components::{RegionCell, Vitals};
    let Some(origin) = world.objects.get_component::<RegionCell>(&origin_object_id).map(|r| r.0) else {
        return Vec::new();
    };
    let mut out: Vec<i32> = Vec::new();
    // `for_each_mut` is the store's only sweep; the query itself borrows
    // everything shared, matching how the aggro scan reads the world.
    world.objects.for_each_mut::<(&RegionCell, &Vitals, Option<&crate::model::Player>, Option<&crate::model::npc::Npc>)>(
        |(region, vitals, player, npc)| {
            if vitals.dead || !crate::world::regions_adjacent(origin, region.0) {
                return;
            }
            let Some(oid) = player.map(|p| p.object_id).or(npc.map(|n| n.object_id)) else { return };
            if oid != origin_object_id {
                out.push(oid);
            }
        },
    );
    // Sorted so the caller's `Rnd.get(size)` index maps to a stable candidate.
    // Java's iteration order is arbitrary too, and a uniform index over a
    // sorted list is still uniform — but this makes a forced roll in tests
    // pick a *known* creature instead of whatever the ECS happened to yield.
    out.sort_unstable();
    out
}
