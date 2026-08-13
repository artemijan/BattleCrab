//! Pending invite requests and the join/answer packet pair.

use super::*;
// ---------------------------------------------------------------------------
// Pending requests (Java `PartyRequest` / `_activeRequester`)
// ---------------------------------------------------------------------------

/// Install a linked request on both sides + its timeout tasks.
pub(crate) fn install_request(
    world: &mut World,
    requestor: i32,
    target: i32,
    kind: RequestKind,
    timeout_ticks: u64,
) {
    let seq = world.next_request_seq();
    world.objects.add_components(
        &requestor,
        PendingRequest {
            kind,
            other: target,
            answerer: false,
            seq,
        },
    );
    world.objects.add_components(
        &target,
        PendingRequest {
            kind,
            other: requestor,
            answerer: true,
            seq,
        },
    );
    let at = world.tick + timeout_ticks;
    world.scheduler.schedule(
        at,
        ScheduledTask::RequestTimeout {
            object_id: requestor,
            seq,
        },
    );
    world.scheduler.schedule(
        at,
        ScheduledTask::RequestTimeout {
            object_id: target,
            seq,
        },
    );
}

/// Drop one side's request slot; returns the removed request.
pub(super) fn take_request(world: &mut World, object_id: i32) -> Option<PendingRequest> {
    let req = world
        .objects
        .get_component::<PendingRequest>(&object_id)
        .copied()?;
    world.objects.remove_component::<PendingRequest>(&object_id);
    Some(req)
}

/// Clear a request from both sides (answer received / a side left the world).
pub(crate) fn clear_linked_request(world: &mut World, object_id: i32) -> Option<PendingRequest> {
    let req = take_request(world, object_id)?;
    if world
        .objects
        .get_component::<PendingRequest>(&req.other)
        .is_some_and(|r| r.seq == req.seq)
    {
        world.objects.remove_component::<PendingRequest>(&req.other);
    }
    Some(req)
}

/// `ScheduledTask::RequestTimeout` — the invite went unanswered.
pub(crate) fn handle_request_timeout(world: &mut World, object_id: i32, seq: u64) {
    let stale = !world
        .objects
        .get_component::<PendingRequest>(&object_id)
        .is_some_and(|r| r.seq == seq);
    if stale {
        return;
    }
    let Some(req) = take_request(world, object_id) else {
        return;
    };
    // An answered-side timeout for a never-attached fresh party drops it
    // (Java leaks the object to GC; our map needs the explicit remove).
    if req.answerer
        && let RequestKind::PartyInvite { party_id } = req.kind
    {
        drop_party_if_unborn(world, party_id);
    }
}

/// Remove a party that only ever existed inside a pending invite (single
/// member who never got `PartyRef` attached).
pub(super) fn drop_party_if_unborn(world: &mut World, party_id: u32) {
    let unborn = world.parties.get(&party_id).is_some_and(|p| {
        p.members.len() == 1 && !world.objects.has_component::<PartyRef>(&p.leader())
    });
    if unborn {
        world.parties.remove(&party_id);
    }
}

// ---------------------------------------------------------------------------
// Invite (`RequestJoinParty` → `AskJoinParty`)
// ---------------------------------------------------------------------------

pub(crate) fn handle_request_join_party(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(requestor) = world.player_oid(client_id) else {
        return;
    };
    let Some(pkt) = cp::RequestJoinParty::read(body) else {
        return;
    };

    let Some((_, target)) = find_player_by_name(world, &pkt.name) else {
        send_sm_to_player(
            world,
            requestor,
            sm_ids::YOU_MUST_FIRST_SELECT_A_USER_TO_INVITE_TO_YOUR_PARTY,
            &[],
        );
        return;
    };
    if world.objects.has_component::<PartyRef>(&target) {
        send_sm_to_player(
            world,
            requestor,
            sm_ids::C1_IS_A_MEMBER_OF_ANOTHER_PARTY_AND_CANNOT_BE_INVITED,
            &[SmParam::Text(player_name_or_empty(world, target))],
        );
        return;
    }
    if target == requestor {
        send_sm_to_player(
            world,
            requestor,
            sm_ids::YOU_HAVE_INVITED_THE_WRONG_TARGET,
            &[],
        );
        return;
    }
    // Party-ban gate (Java `RequestJoinParty`, G31): a party-banned requestor
    // can't invite, and a party-banned target can't be invited. CHARACTER-affect
    // only (Java `isPartyBanned`).
    if crate::game_loop::punishment::is_party_banned(world, requestor) {
        send_sm_to_player(
            world,
            requestor,
            sm_ids::YOU_HAVE_BEEN_REPORTED_AS_AN_ILLEGAL_PROGRAM_USER_SO_PARTICIPATING_IN_A_PARTY_IS_NOT_ALLOWED,
            &[],
        );
        send_action_failed(world, client_id);
        return;
    }
    if crate::game_loop::punishment::is_party_banned(world, target) {
        send_sm_to_player(
            world,
            requestor,
            sm_ids::C1_HAS_BEEN_REPORTED_AS_AN_ILLEGAL_PROGRAM_USER_AND_CANNOT_JOIN_A_PARTY,
            &[SmParam::Text(player_name_or_empty(world, target))],
        );
        return;
    }
    // Java `RequestJoinParty`: `BlockList.isBlocked(target, requestor)` — the
    // *invitee's* list decides. The refusal names the target, so the requestor
    // is told who is ignoring them.
    if crate::game_loop::block_list::is_blocked(world, target, requestor) {
        send_sm_to_player(
            world,
            requestor,
            sm_ids::C1_HAS_PLACED_YOU_ON_HIS_HER_IGNORE_LIST,
            &[SmParam::Text(player_name_or_empty(world, target))],
        );
        return;
    }
    // (Cursed weapons / jail / olympiad / event guards skipped — systems
    // absent.)

    // Java sends "C1 has been invited" before the create/add branches — a
    // later guard failing still leaves this on the requestor's screen.
    send_sm_to_player(
        world,
        requestor,
        sm_ids::C1_HAS_BEEN_INVITED_TO_THE_PARTY,
        &[SmParam::Text(player_name_or_empty(world, target))],
    );

    match world.objects.get_component::<PartyRef>(&requestor).copied() {
        None => {
            // `createNewParty`: the Party exists from the invite on, but the
            // requestor only links to it (`setParty`) when the target accepts.
            let Some(rule) = LootRule::from_id(pkt.loot_rule_id) else {
                return;
            };
            if world.objects.has_component::<PendingRequest>(&target) {
                send_sm_to_player(world, requestor, sm_ids::WAITING_FOR_ANOTHER_REPLY, &[]);
                return;
            }
            let party_id = world.next_party_id;
            world.next_party_id += 1;
            let seq = world.next_request_seq();
            let mut party = Party::new(requestor, rule, seq);
            party.pending_invitation = true;
            party.pending_invite_expiry_tick = world.tick + REQUEST_TIMEOUT_TICKS;
            world.parties.insert(party_id, party);
            install_request(
                world,
                requestor,
                target,
                RequestKind::PartyInvite { party_id },
                PARTY_REQUEST_TIMEOUT_TICKS,
            );
            send_to_player(
                world,
                target,
                server_packets::ask_join_party(&player_name_or_empty(world, requestor), rule.id()),
            );
        }
        Some(PartyRef(party_id)) => {
            // `addTargetToParty`.
            let Some(party) = world.parties.get(&party_id) else {
                return;
            };
            if !party.is_leader(requestor) {
                send_sm_to_player(
                    world,
                    requestor,
                    sm_ids::ONLY_THE_LEADER_CAN_GIVE_OUT_INVITATIONS,
                    &[],
                );
            } else if party.members.len() >= world.cfg.character.alt_party_max_members {
                send_sm_to_player(world, requestor, sm_ids::THE_PARTY_IS_FULL, &[]);
            } else if party.pending_invitation && party.pending_invite_expiry_tick > world.tick {
                send_sm_to_player(world, requestor, sm_ids::WAITING_FOR_ANOTHER_REPLY, &[]);
            } else if !world.objects.has_component::<PendingRequest>(&target) {
                let rule = party.distribution;
                install_request(
                    world,
                    requestor,
                    target,
                    RequestKind::PartyInvite { party_id },
                    PARTY_REQUEST_TIMEOUT_TICKS,
                );
                if let Some(party) = world.parties.get_mut(&party_id) {
                    party.pending_invitation = true;
                    party.pending_invite_expiry_tick = world.tick + REQUEST_TIMEOUT_TICKS;
                }
                send_to_player(
                    world,
                    target,
                    server_packets::ask_join_party(
                        &player_name_or_empty(world, requestor),
                        rule.id(),
                    ),
                );
            } else {
                send_sm_to_player(
                    world,
                    requestor,
                    sm_ids::C1_IS_ON_ANOTHER_TASK_PLEASE_TRY_AGAIN_LATER,
                    &[SmParam::Text(player_name_or_empty(world, target))],
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Answer (`RequestAnswerJoinParty` → `JoinParty` + `addPartyMember`)
// ---------------------------------------------------------------------------

pub(crate) fn handle_request_answer_join_party(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let response = cp::read_answer(body).unwrap_or(0);

    let Some(req) = world
        .objects
        .get_component::<PendingRequest>(&player)
        .copied()
    else {
        return;
    };
    let (RequestKind::PartyInvite { party_id }, true) = (req.kind, req.answerer) else {
        return;
    };
    clear_linked_request(world, player);
    let requestor = req.other;

    if !world.parties.contains_key(&party_id) {
        return;
    }
    // Java: a requestor who meanwhile joined a *different* party voids the
    // request.
    if world
        .objects
        .get_component::<PartyRef>(&requestor)
        .is_some_and(|r| r.0 != party_id)
    {
        if let Some(party) = world.parties.get_mut(&party_id) {
            party.pending_invitation = false;
        }
        drop_party_if_unborn(world, party_id);
        return;
    }

    send_to_player(
        world,
        requestor,
        server_packets::join_party(response.clamp(-1, 1)),
    );

    if response == 1 {
        let (member_count, max) = (
            world.parties[&party_id].members.len(),
            world.cfg.character.alt_party_max_members,
        );
        if member_count >= max {
            send_sm_to_player(world, player, sm_ids::THE_PARTY_IS_FULL, &[]);
            send_sm_to_player(world, requestor, sm_ids::THE_PARTY_IS_FULL, &[]);
        } else {
            // First accept binds the leader (`requestor.setParty(party)`).
            if !world.objects.has_component::<PartyRef>(&requestor) {
                world.objects.add_components(&requestor, PartyRef(party_id));
            }
            add_party_member(world, party_id, player);
            // Java `RequestAnswerJoinParty`: if the inviter runs a matching
            // room, the new party member joins that room too (G30).
            crate::game_loop::party_room::on_party_invite_accepted(world, requestor, player);
        }
    } else {
        if response == -1 {
            send_sm_to_player(
                world,
                requestor,
                sm_ids::C1_IS_SET_TO_REFUSE_PARTY_REQUESTS,
                &[SmParam::PlayerName(player_name_or_empty(world, player))],
            );
        }
        // A declined first invite dissolves the embryo party.
        drop_party_if_unborn(world, party_id);
    }

    if let Some(party) = world.parties.get_mut(&party_id) {
        party.pending_invitation = false;
    }
}
