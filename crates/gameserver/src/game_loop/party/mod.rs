//! Port of the party flows: `RequestJoinParty`/`RequestAnswerJoinParty`/
//! `RequestWithDrawalParty`/`RequestOustPartyMember`/`RequestChangePartyLeader`
//! + `Party`'s member management, loot-rule voting, the 12 s position
//!   broadcast, and the `PartySmallWindowUpdate` vitals piggyback.
//!   Out of scope (PLAN_G10_SOCIAL.md): duels, block list.

use crate::game_loop::helpers::send_to_player;

use crate::geo::distance::within_2d_xy;

use crate::model::Player;

use crate::model::components::{PartyRef, PlayerVitals, Position, Vitals};
use crate::network::server_packets::{self, PartyMemberView};
use crate::scheduler::ScheduledTask;

use crate::world::World;

/// `Player.REQUEST_TIMEOUT` (15 s) in ticks — the `_pendingInvitation`
/// expiry and the friend-invite request timeout.
pub(crate) const REQUEST_TIMEOUT_TICKS: u64 = 15 * 10;
/// `PartyRequest.scheduleTimeout(30 * 1000)` in ticks.
const PARTY_REQUEST_TIMEOUT_TICKS: u64 = 30 * 10;
/// `PARTY_POSITION_BROADCAST_INTERVAL` (12 s) in ticks.
const POSITION_BROADCAST_TICKS: u64 = 12 * 10;
/// `PARTY_DISTRIBUTION_TYPE_REQUEST_TIMEOUT` (15 s) in ticks.
const LOOT_CHANGE_TIMEOUT_TICKS: u64 = 15 * 10;

// ---------------------------------------------------------------------------
// Small lookups
// ---------------------------------------------------------------------------

/// `World.getPlayer(name)` — case-insensitive scan over in-game players.
pub(crate) fn find_player_by_name(world: &World, name: &str) -> Option<(u32, i32)> {
    world.in_game_clients().find(|&(_, oid)| {
        world
            .objects
            .get_component::<Player>(&oid)
            .is_some_and(|p| p.name.eq_ignore_ascii_case(name))
    })
}

/// The member fields the party-window packets carry.
pub(crate) fn member_view(world: &World, object_id: i32) -> Option<PartyMemberView> {
    let p = world.objects.get_component::<Player>(&object_id)?;
    let v = world.objects.get_component::<Vitals>(&object_id)?;
    let pv = world.objects.get_component::<PlayerVitals>(&object_id)?;
    Some(PartyMemberView {
        object_id,
        name: p.name.clone(),
        cp: pv.cur_cp as i32,
        max_cp: pv.max_cp,
        hp: v.cur_hp as i32,
        max_hp: v.max_hp,
        mp: v.cur_mp as i32,
        max_mp: v.max_mp,
        vitality: p.vitality_points,
        level: p.level,
        class_id: p.class_id,
        race: p.race,
        summons: summon_views(world, object_id),
    })
}

/// A member's pet and servitor as party-window rows (Java writes the pet
/// first, then servitors). Reads the owner's `SummonRef` link rather than
/// sweeping — which is what makes this callable from `&World`.
fn summon_views(world: &World, owner_oid: i32) -> Vec<server_packets::PartySummonView> {
    use crate::game_loop::servitor::{pet_of, servitor_of};
    [
        (pet_of(world, owner_oid), 1u8),
        (servitor_of(world, owner_oid), 2u8),
    ]
    .into_iter()
    .filter_map(|(oid, summon_type)| {
        let oid = oid?;
        let npc = world
            .objects
            .get_component::<crate::model::npc::Npc>(&oid)?;
        let v = world.objects.get_component::<Vitals>(&oid)?;
        let name = npc
            .template(world)
            .map(|t| t.name.clone())
            .unwrap_or_default();
        Some(server_packets::PartySummonView {
            object_id: oid,
            npc_id: npc.npc_id,
            summon_type,
            name,
            hp: v.cur_hp as i32,
            max_hp: v.max_hp,
            mp: v.cur_mp as i32,
            max_mp: v.max_mp,
            level: npc.template(world).map(|t| t.level).unwrap_or(1),
        })
    })
    .collect()
}

/// `Party.broadcastPacket` — every member, or all but `exclude`.
pub(crate) fn broadcast_to_party(
    world: &World,
    party_id: u32,
    packet: &[u8],
    exclude: Option<i32>,
) {
    let Some(party) = world.parties.get(&party_id) else {
        return;
    };
    for &m in &party.members {
        if exclude == Some(m) {
            continue;
        }
        send_to_player(world, m, packet.to_vec());
    }
}

mod invite;
mod loot;
mod membership;
mod rewards;
mod tactical;

pub(crate) use invite::{
    clear_linked_request, handle_request_answer_join_party, handle_request_join_party,
    handle_request_timeout, install_request,
};

pub(crate) use loot::{
    distribute_item, handle_answer_party_loot_modification, handle_loot_change_timeout,
    handle_request_party_loot_modification, spoil_looter,
};
pub(crate) use membership::{
    LeaveType, add_party_member, handle_request_change_party_leader,
    handle_request_oust_party_member, handle_request_withdrawal_party, on_player_leave_world,
    remove_party_member,
};
pub(crate) use rewards::distribute_xp_and_sp;
pub(crate) use tactical::{
    apply_tactical_signs, handle_tactical_sign_target, handle_tactical_sign_use,
};

// ---------------------------------------------------------------------------
// Position broadcast + vitals piggyback
// ---------------------------------------------------------------------------

pub(crate) fn handle_position_broadcast(world: &mut World, party_id: u32, seq: u64) {
    let Some(party) = world.parties.get(&party_id) else {
        return;
    };
    if party.seq != seq {
        return;
    }
    let locations: Vec<(i32, i32, i32, i32)> = party
        .members
        .iter()
        .filter_map(|&m| {
            world
                .objects
                .get_component::<Position>(&m)
                .map(|p| (m, p.x, p.y, p.z))
        })
        .collect();
    let pkt = server_packets::party_member_position(&locations);
    broadcast_to_party(world, party_id, &pkt, None);
    world.scheduler.schedule(
        world.tick + POSITION_BROADCAST_TICKS,
        ScheduledTask::PartyPositionBroadcast { party_id, seq },
    );
}

/// The `PartySmallWindowUpdate` piggyback: whenever a party member's vitals
/// `StatusUpdate` goes out, the other members' windows refresh too (Java
/// `Player.broadcastStatusUpdate`, hysteresis dropped).
pub(crate) fn notify_party_vitals(world: &World, object_id: i32) {
    notify_party_window(world, object_id, server_packets::party_window_flags::VITALS);
}

/// The all-flags variant (level-ups — Java `PartySmallWindowUpdate(this, true)`).
pub(crate) fn notify_party_all(world: &World, object_id: i32) {
    notify_party_window(world, object_id, server_packets::party_window_flags::ALL);
}

/// The vitality-only variant (`PlayerStat.setVitalityPoints` adds just the
/// `VITALITY_POINTS` component type before broadcasting).
pub(crate) fn notify_party_vitality_points(world: &World, object_id: i32) {
    notify_party_window(
        world,
        object_id,
        server_packets::party_window_flags::VITALITY_POINTS,
    );
}

fn notify_party_window(world: &World, object_id: i32, flags: u16) {
    let Some(PartyRef(party_id)) = world.objects.get_component::<PartyRef>(&object_id).copied()
    else {
        return;
    };
    let Some(view) = member_view(world, object_id) else {
        return;
    };
    let pkt = server_packets::party_small_window_update(&view, flags);
    broadcast_to_party(world, party_id, &pkt, Some(object_id));
}

// ---------------------------------------------------------------------------
// Party chat (`ChatParty`)
// ---------------------------------------------------------------------------

/// Broadcast a party line to every member (speaker included). Returns false
/// when the speaker has no party (caller answers SM 4201).
pub(crate) fn party_say(world: &World, speaker: i32, packet: &[u8]) -> bool {
    let Some(PartyRef(party_id)) = world.objects.get_component::<PartyRef>(&speaker).copied()
    else {
        return false;
    };
    broadcast_to_party(world, party_id, packet, None);
    true
}

/// The members of `object_id`'s party, or `None` when they are not in one.
///
/// The honest lookup. What "solo" *means* differs by caller and the three
/// readings are not interchangeable, so the `Option` is deliberately left for
/// the call site to answer:
///
/// - a party of one — reach for [`group_or_self`];
/// - nobody at all — `.unwrap_or_default()`, e.g. quest kill credit, where a
///   solo killer is already handled separately;
/// - nothing happens — `else { return; }`, which is how Java's
///   `if (party == null)` guards read.
pub(crate) fn party_members(world: &World, object_id: i32) -> Option<Vec<i32>> {
    world
        .objects
        .get_component::<PartyRef>(&object_id)
        .and_then(|r| world.parties.get(&r.0))
        .map(|p| p.members.clone())
}

/// [`party_members`], counting an unpartied player as their own party of one.
///
/// Java's reading wherever a group effect must still reach a solo caster — the
/// trigger skills and the party-affect scope.
pub(crate) fn group_or_self(world: &World, object_id: i32) -> Vec<i32> {
    party_members(world, object_id).unwrap_or_else(|| vec![object_id])
}

/// `(leader, members)` of `object_id`'s party, `None` when solo.
///
/// The shape the raid-entry gates want: they check the leader carries the
/// portal item and then admit the whole group.
pub(crate) fn leader_and_members(world: &World, object_id: i32) -> Option<(i32, Vec<i32>)> {
    let party_id = world.objects.get_component::<PartyRef>(&object_id)?.0;
    let party = world.parties.get(&party_id)?;
    Some((party.leader(), party.members.clone()))
}

/// Whether `a` and `b` are in the same party (`Player.isInLooterParty` half —
/// the party-membership test, minus the online/proximity filtering the caller
/// doesn't need for the spoil-owner check).
pub(crate) fn same_party(world: &World, a: i32, b: i32) -> bool {
    match (
        world.objects.get_component::<PartyRef>(&a).map(|r| r.0),
        world.objects.get_component::<PartyRef>(&b).map(|r| r.0),
    ) {
        (Some(pa), Some(pb)) => pa == pb,
        _ => false,
    }
}

/// `Party.getActualLooter(sweeper, itemId, spoil=true, corpse)` — who receives
/// a Sweeper loot item. Solo (or a party rule that doesn't spread spoil) → the
/// sweeper. `*_INCLUDING_SPOIL` rules pick a random / by-turn member in loot
/// range of the corpse, falling back to the sweeper when none qualifies.
/// The party members standing within `range` (2D) of a corpse — Java's
/// `getPartyMembersInRange` check that gates both spoil and drop distribution.
fn members_within(world: &World, members: &[i32], corpse: (i32, i32), range: f64) -> Vec<i32> {
    members
        .iter()
        .copied()
        .filter(|&m| within_2d_xy(world, m, corpse.0, corpse.1, range))
        .collect()
}
