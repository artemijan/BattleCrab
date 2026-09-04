//! Loot: the loot-rule voting flow, the spoil looter pick and item/adena
//! distribution.

// ---------------------------------------------------------------------------
// Loot-rule voting (`requestLootChange` / `answerLootChangeRequest`)
// ---------------------------------------------------------------------------

use super::LOOT_CHANGE_TIMEOUT_TICKS;
use super::broadcast_to_party;
use super::members_within;
use crate::game_loop::helpers::player_name_or_empty;
use crate::game_loop::helpers::send_sm_to_player;
use crate::model::components::social::PartyRef;
use crate::model::party::LootChangeRequest;
use crate::model::party::LootRule;
use crate::model::party::Party;
use crate::network::client_packets as cp;
use crate::network::server_packets;
use crate::network::server_packets::SmParam;
use crate::network::server_packets::sm_ids;
use crate::scheduler::ScheduledTask;
use crate::world::World;
pub(crate) fn handle_request_party_loot_modification(
    world: &mut World,
    client_id: u32,
    body: &[u8],
) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(rule) = cp::party::read_answer(body).and_then(LootRule::from_id) else {
        return;
    };
    let Some(PartyRef(party_id)) = world.objects.get_component::<PartyRef>(&player).copied() else {
        return;
    };
    let Some(party) = world.parties.get(&party_id) else {
        return;
    };
    if !party.is_leader(player) || party.loot_change.is_some() {
        return;
    }
    let seq = world.next_request_seq();
    let leader_name = player_name_or_empty(world, player);
    {
        let party = world.parties.get_mut(&party_id).expect("checked");
        party.loot_change = Some(LootChangeRequest {
            rule,
            answers: Default::default(),
        });
        party.seq = seq; // NOTE: shared generation — see handle_loot_change_timeout
    }
    world.scheduler.schedule(
        world.tick + LOOT_CHANGE_TIMEOUT_TICKS,
        ScheduledTask::PartyLootChangeTimeout { party_id, seq },
    );
    let ask = server_packets::ex_ask_modify_party_looting(&leader_name, rule.id());
    broadcast_to_party(world, party_id, &ask, Some(player));
    send_sm_to_player(
        world,
        player,
        sm_ids::REQUESTING_APPROVAL_FOR_CHANGING_PARTY_LOOT_TO_S1,
        &[SmParam::SysString(rule.sys_string_id())],
    );
}

pub(crate) fn handle_answer_party_loot_modification(
    world: &mut World,
    client_id: u32,
    body: &[u8],
) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let answer = cp::party::read_answer(body).unwrap_or(0);
    let Some(PartyRef(party_id)) = world.objects.get_component::<PartyRef>(&player).copied() else {
        return;
    };
    let Some(party) = world.parties.get_mut(&party_id) else {
        return;
    };
    let member_count = party.members.len();
    let Some(change) = party.loot_change.as_mut() else {
        return;
    };
    if change.answers.contains(&player) {
        return;
    }
    if answer != 1 {
        finish_loot_change(world, party_id, false);
        return;
    }
    change.answers.insert(player);
    if change.answers.len() >= member_count - 1 {
        finish_loot_change(world, party_id, true);
    }
}

pub(crate) fn handle_loot_change_timeout(world: &mut World, party_id: u32, seq: u64) {
    let live = world
        .parties
        .get(&party_id)
        .is_some_and(|p| p.seq == seq && p.loot_change.is_some());
    if live {
        finish_loot_change(world, party_id, false);
    }
}

/// `finishLootRequest`.
pub(super) fn finish_loot_change(world: &mut World, party_id: u32, success: bool) {
    let Some(party) = world.parties.get_mut(&party_id) else {
        return;
    };
    let Some(change) = party.loot_change.take() else {
        return;
    };
    if success {
        party.distribution = change.rule;
    }
    let rule = if success {
        change.rule
    } else {
        party.distribution
    };
    let set = server_packets::ex_set_party_looting(success as i32, rule.id());
    broadcast_to_party(world, party_id, &set, None);
    let sm = if success {
        server_packets::system_message_with(
            sm_ids::PARTY_LOOT_WAS_CHANGED_TO_S1,
            &[SmParam::SysString(rule.sys_string_id())],
        )
    } else {
        server_packets::system_message_with(sm_ids::PARTY_LOOT_CHANGE_WAS_CANCELLED, &[])
    };
    broadcast_to_party(world, party_id, &sm, None);
}

/// The member-change path already holds `&mut Party` — just void the vote;
/// the verdict packets follow from the caller when needed. Java cancels with
/// the full `finishLootRequest(false)` broadcast on member add; on removal
/// the vote silently dies with the member set. We void silently in both
/// spots and let `add_party_member` do the broadcast variant.
pub(super) fn finish_loot_change_inline(party: &mut Party) {
    party.loot_change = None;
}

pub(crate) fn spoil_looter(world: &mut World, sweeper: i32, corpse: (i32, i32)) -> i32 {
    let Some(party_id) = world
        .objects
        .get_component::<PartyRef>(&sweeper)
        .map(|r| r.0)
    else {
        return sweeper;
    };
    let Some((members, rule, last_loot)) = world
        .parties
        .get(&party_id)
        .map(|p| (p.members.clone(), p.distribution, p.item_last_loot))
    else {
        return sweeper;
    };
    if !rule.includes_spoil() {
        return sweeper;
    }
    let range = world.cfg.character.alt_party_range as f64;
    let in_range = members_within(world, &members, corpse, range);
    if in_range.is_empty() {
        return sweeper;
    }
    if rule.is_random() {
        in_range[world.roll(in_range.len() as i32) as usize]
    } else {
        // `getCheckedNextLooter`: advance the cursor over the member list,
        // skipping out-of-range members.
        let mut cursor = last_loot;
        let mut picked = sweeper;
        for _ in 0..members.len() {
            cursor = (cursor + 1) % members.len();
            if in_range.contains(&members[cursor]) {
                picked = members[cursor];
                break;
            }
        }
        if let Some(party) = world.parties.get_mut(&party_id) {
            party.item_last_loot = cursor;
        }
        picked
    }
}

/// `Party.distributeItem`/`distributeAdena` for an auto-looted drop. The
/// corpse position gates the in-range member set (`ALT_PARTY_RANGE`).
pub(crate) fn distribute_item(
    world: &mut World,
    party_id: u32,
    killer: i32,
    item_id: i32,
    count: i64,
    corpse: (i32, i32),
) {
    const ADENA_ID: i32 = crate::data::item_data::ADENA_ID;
    let range = world.cfg.character.alt_party_range as f64;
    let Some((members, rule, last_loot)) = world
        .parties
        .get(&party_id)
        .map(|p| (p.members.clone(), p.distribution, p.item_last_loot))
    else {
        crate::game_loop::death::give_item(world, killer, item_id, count);
        return;
    };
    let in_range = members_within(world, &members, corpse, range);

    if item_id == ADENA_ID {
        // `distributeAdena` — an even split over the in-range members.
        if in_range.is_empty() {
            return;
        }
        let share = count / in_range.len() as i64;
        if share > 0 {
            for m in in_range {
                crate::game_loop::death::give_item(world, m, item_id, share);
            }
        }
        return;
    }

    let looter = if rule.is_random() {
        if in_range.is_empty() {
            killer
        } else {
            in_range[world.roll(in_range.len() as i32) as usize]
        }
    } else if rule.is_by_turn() {
        // `getCheckedNextLooter`: advance the cursor over the member list,
        // skipping out-of-range members.
        let mut cursor = last_loot;
        let mut picked = None;
        for _ in 0..members.len() {
            cursor = (cursor + 1) % members.len();
            if in_range.contains(&members[cursor]) {
                picked = Some(members[cursor]);
                break;
            }
        }
        if let Some(party) = world.parties.get_mut(&party_id) {
            party.item_last_loot = cursor;
        }
        picked.unwrap_or(killer)
    } else {
        killer // FINDERS_KEEPERS
    };

    crate::game_loop::death::give_item(world, looter, item_id, count);

    // "C1 has obtained …" to the rest of the party.
    let looter_name = player_name_or_empty(world, looter);
    let sm = if count > 1 {
        server_packets::system_message_with(
            sm_ids::C1_HAS_OBTAINED_S3_S2,
            &[
                SmParam::Text(looter_name),
                SmParam::ItemName(item_id),
                SmParam::Long(count),
            ],
        )
    } else {
        server_packets::system_message_with(
            sm_ids::C1_HAS_OBTAINED_S2,
            &[SmParam::Text(looter_name), SmParam::ItemName(item_id)],
        )
    };
    broadcast_to_party(world, party_id, &sm, Some(looter));
}
