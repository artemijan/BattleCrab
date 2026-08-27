//! `Party`'s tactical signs — the four numbered tokens (`/tacticalsign1..4`,
//! the star buttons on the party window) a group sticks over a creature's
//! head, and the `/targettacticalsign1..4` recall that selects whatever wears
//! one.
//!
//! Java splits this across two `playeractions` handlers and four `Party`
//! methods; the split here is the same, because the interesting half is
//! `Party.addTacticalSign`'s three arms — the same button is *set*, *clear*
//! and *move* depending on what the sign already points at.
//!
//! **Both handlers refuse outright when the presser is not in a party**
//! (`ActionFailed`, no message). A solo player pressing a star gets nothing at
//! all, on retail as here — the signs are party state and there is nowhere to
//! put them.

use crate::game_loop::helpers::{is_creature, object_name, send_action_failed, send_to_player};
use crate::model::components::{PartyRef, TargetRef};
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::world::World;

use super::broadcast_to_party;

/// `Party.TACTICAL_SYS_STRINGS` — index = sign id, value = the `SysString`
/// entry naming that token in "C1 used S3 on C2". Index 0 is Java's unused
/// slot, kept so the ids line up with the array they came from.
const TACTICAL_SYS_STRINGS: [i32; 5] = [0, 2664, 2665, 2666, 2667];

/// `handlers/playeractions/TacticalSignUse` — stick sign `sign_id` on whatever
/// the presser has targeted.
pub(crate) fn handle_tactical_sign_use(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    sign_id: i32,
) {
    let party_id = world
        .objects
        .get_component::<PartyRef>(&object_id)
        .map(|&PartyRef(id)| id);
    let target = world
        .objects
        .get_component::<TargetRef>(&object_id)
        .and_then(|t| t.0);
    // `!player.isInParty() || player.getTarget() == null || !isCreature()`.
    let (Some(party_id), Some(target)) = (party_id, target) else {
        send_action_failed(world, client_id);
        return;
    };
    if !is_creature(world, target) {
        send_action_failed(world, client_id);
        return;
    }
    add_tactical_sign(world, party_id, object_id, sign_id, target);
}

/// `handlers/playeractions/TacticalSignTarget` — select whoever wears sign
/// `sign_id`.
pub(crate) fn handle_tactical_sign_target(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    sign_id: i32,
) {
    let Some(&PartyRef(party_id)) = world.objects.get_component::<PartyRef>(&object_id) else {
        send_action_failed(world, client_id);
        return;
    };
    set_target_based_on_tactical_sign_id(world, client_id, object_id, party_id, sign_id);
}

/// `Party.addTacticalSign`. Three arms, and the middle one is why the button
/// is not a plain setter: pressing a sign that already points at this very
/// target *removes* it.
fn add_tactical_sign(world: &mut World, party_id: u32, actor: i32, sign_id: i32, target: i32) {
    let Some(party) = world.parties.get(&party_id) else {
        return;
    };
    let current = party.tactical_signs.get(&sign_id).copied();
    match current {
        None => {
            // A target may wear only one sign: taking a new one strips the old
            // (`_tacticalSigns.values().remove(target)`). Java removes the
            // *mapping* silently — no clear packet goes out, because the new
            // sign's packet overwrites the marker on the client anyway.
            let party = world.parties.get_mut(&party_id).expect("checked");
            party.tactical_signs.retain(|_, &mut t| t != target);
            party.tactical_signs.insert(sign_id, target);

            let sm = used_message(world, actor, target, sign_id);
            let sign = server_packets::ex_tactical_sign(target, sign_id);
            broadcast_to_party(world, party_id, &sign, None);
            broadcast_to_party(world, party_id, &sm, None);
        }
        Some(previous) if previous == target => {
            // Same sign, same target — a toggle off. No system message: Java
            // announces the *placing* of a sign only.
            world
                .parties
                .get_mut(&party_id)
                .expect("checked")
                .tactical_signs
                .remove(&sign_id);
            let clear = server_packets::ex_tactical_sign(target, 0);
            broadcast_to_party(world, party_id, &clear, None);
        }
        Some(previous) => {
            // Moving the sign: the old wearer is cleared first, then the new
            // one is marked. Note Java `replace`s the entry rather than
            // stripping the new target's other sign, so a creature *can* end
            // up wearing two signs this way — kept.
            world
                .parties
                .get_mut(&party_id)
                .expect("checked")
                .tactical_signs
                .insert(sign_id, target);

            let sm = used_message(world, actor, target, sign_id);
            let clear = server_packets::ex_tactical_sign(previous, 0);
            let sign = server_packets::ex_tactical_sign(target, sign_id);
            broadcast_to_party(world, party_id, &clear, None);
            broadcast_to_party(world, party_id, &sign, None);
            broadcast_to_party(world, party_id, &sm, None);
        }
    }
}

/// `$c1 used $s3 on $c2.` — `addPcName(player)`, `addString(target.getName())`,
/// `addSystemString(TACTICAL_SYS_STRINGS[id])`, in that order.
fn used_message(world: &World, actor: i32, target: i32, sign_id: i32) -> Vec<u8> {
    let sys_string = TACTICAL_SYS_STRINGS
        .get(sign_id as usize)
        .copied()
        .unwrap_or(0);
    server_packets::system_message_with(
        sm_ids::C1_USED_S3_ON_C2,
        &[
            SmParam::PlayerName(object_name(world, actor)),
            SmParam::Text(object_name(world, target)),
            SmParam::SysString(sys_string),
        ],
    )
}

/// `Party.setTargetBasedOnTacticalSignId` — the recall. Silent when the sign
/// is unused or its wearer cannot be selected.
fn set_target_based_on_tactical_sign_id(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    party_id: u32,
    sign_id: i32,
) {
    let Some(target) = world
        .parties
        .get(&party_id)
        .and_then(|p| p.tactical_signs.get(&sign_id).copied())
    else {
        return;
    };
    let invisible = world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&target)
        .is_some_and(|f| f.hidden || f.untargetable);
    if invisible
        || crate::game_loop::abnormal::is_untargetable(world, target)
        || crate::game_loop::abnormal::is_targeting_disabled(world, object_id)
    {
        return;
    }
    crate::game_loop::target::set_target(world, client_id, object_id, Some(target));
}

/// `Party.applyTacticalSigns` — hand one member the party's current signs
/// (`remove = false`, on join) or wipe them from their client
/// (`remove = true`, on leave). The map itself is untouched either way: the
/// signs belong to the party, not to the member reading them.
pub(crate) fn apply_tactical_signs(world: &World, party_id: u32, member: i32, remove: bool) {
    let Some(party) = world.parties.get(&party_id) else {
        return;
    };
    if party.tactical_signs.is_empty() {
        return;
    }
    let packets: Vec<Vec<u8>> = party
        .tactical_signs
        .iter()
        .map(|(&sign_id, &target)| {
            server_packets::ex_tactical_sign(target, if remove { 0 } else { sign_id })
        })
        .collect();
    for pkt in packets {
        send_to_player(world, member, pkt);
    }
}
