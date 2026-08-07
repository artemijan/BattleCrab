//! Port of the party flows: `RequestJoinParty`/`RequestAnswerJoinParty`/
//! `RequestWithDrawalParty`/`RequestOustPartyMember`/`RequestChangePartyLeader`
//! + `Party`'s member management, loot-rule voting, the 12 s position
//! broadcast, and the `PartySmallWindowUpdate` vitals piggyback.
//! Out of scope (PLAN_G10_SOCIAL.md): command channels, matching rooms,
//! tactical signs, pets/servitors, duels, block list.

use crate::model::Player;
use crate::model::components::{
    PartyRef, PendingRequest, PlayerVitals, Position, RequestKind, Vitals,
};
use crate::model::party::{LootChangeRequest, LootRule, Party};
use crate::network::client_packets as cp;
use crate::network::server_packets::{self, PartyMemberView, SmParam, sm_ids};
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::World;

use super::helpers::broadcast_to_others;

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
    world.clients.iter().find_map(|(&cid, cs)| match cs {
        ClientSession::InGame(s) => {
            let oid = s.player_object_id();
            world
                .objects
                .get_component::<Player>(&oid)
                .filter(|p| p.name.eq_ignore_ascii_case(name))
                .map(|_| (cid, oid))
        }
        _ => None,
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
fn summon_views(
    world: &World,
    owner_oid: i32,
) -> Vec<crate::network::server_packets::PartySummonView> {
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
        Some(crate::network::server_packets::PartySummonView {
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

fn send_to_player(world: &World, object_id: i32, packet: Vec<u8>) {
    crate::game_loop::helpers::send_to_player(world, object_id, packet);
}

fn send_sm_to_player(world: &World, object_id: i32, message_id: i16, params: &[SmParam]) {
    crate::game_loop::helpers::send_sm_to_player(world, object_id, message_id, params);
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

/// Java `UserInfo.calculateRelation` — the party/clan/siege relation bitmask the
/// `UserInfo` RELATION block carries. Party membership comes off the
/// `PartyRef` component (absent → not in a party), clan off the `Player`.
/// Takes `&Player` so the clan bits are correct even before the object is
/// registered (the enter-world burst).
pub(crate) fn calculate_relation(world: &World, player: &Player) -> i32 {
    let mut relation = 0;
    if let Some(PartyRef(pid)) = world
        .objects
        .get_component::<PartyRef>(&player.object_id)
        .copied()
        && let Some(party) = world.parties.get(&pid)
    {
        relation |= 0x08; // party member
        if party.is_leader(player.object_id) {
            relation |= 0x10; // party leader
        }
    }
    if player.clan_id > 0 {
        relation |= 0x20; // clan member
        if player.clan_leader {
            relation |= 0x40; // clan leader
        }
    }
    if super::pvp::is_in_siege(world, player.object_id) {
        relation |= 0x80; // in siege — draws the siege crown (Java `isInSiege()`)
    }
    relation
}

/// Java `Player.getRelation(target)` — the bitmask the **`RelationChanged`**
/// packet carries. This is a *different* layout from [`calculate_relation`]
/// (which is `UserInfo`'s): here clan member is `0x40` and the clan-leader bit
/// (the one that draws the on-head crown) is `0x80`. Only the target-independent
/// bits the port models are produced; the siege enemy/ally bits are folded in
/// per-viewer by the caller, and clan-mate (`0x100`)/ally/party-index encoding
/// are TODO (they need the viewer and a fuller party model).
pub(crate) fn relation_changed_base(world: &World, oid: i32) -> i32 {
    let Some(p) = world.objects.get_component::<Player>(&oid) else {
        return 0;
    };
    let mut relation = 0;
    if let Some(PartyRef(pid)) = world.objects.get_component::<PartyRef>(&oid).copied()
        && world.parties.get(&pid).is_some()
    {
        relation |= 0x20; // RELATION_HAS_PARTY
        if world
            .parties
            .get(&pid)
            .is_some_and(|party| party.is_leader(oid))
        {
            relation |= 0x10; // RELATION_PARTYLEADER
        }
    }
    if p.clan_id > 0 {
        relation |= 0x40; // RELATION_CLAN_MEMBER
        if p.clan_leader {
            relation |= 0x80; // RELATION_LEADER — draws the clan-leader crown
        }
    }
    relation
}

/// `Player.broadcastUserInfo()` — fresh `UserInfo` to self, and Java's
/// **coalesced** `CharInfo` to everyone who can see them:
/// `broadcastCharInfo` never sends inline, it schedules
/// `_broadcastCharInfoTask` 50 ms out and folds every call made in that
/// window into one packet — which both spares onlookers a `CharInfo` per
/// update in a burst and lands the packet *after* whatever actor swap (a
/// `Ride`, a transform) preceded it.
pub(crate) fn broadcast_user_info(world: &mut World, object_id: i32) {
    let Some(v) = crate::model::PlayerView::of_world(world, object_id) else {
        return;
    };
    let relation = calculate_relation(world, v.p);
    send_to_player(
        world,
        object_id,
        crate::network::user_info::user_info(&v, &world.data, &world.cfg.character, relation),
    );
    // `if (_broadcastCharInfoTask == null) { schedule(50ms) }`.
    let pending = world
        .objects
        .get_component::<Player>(&object_id)
        .is_none_or(|p| p.char_info_pending);
    if pending {
        return;
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.char_info_pending = true;
    }
    world.scheduler.schedule(
        world.tick + crate::game_loop::helpers::ms_to_ticks(50),
        crate::scheduler::ScheduledTask::BroadcastCharInfo { object_id },
    );
}

/// The `_broadcastCharInfoTask` body: build the `CharInfo` **now** (state can
/// have moved since the calls that scheduled it) and send it to onlookers.
pub(crate) fn broadcast_char_info_now(world: &mut World, object_id: i32) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.char_info_pending = false;
    }
    let Some(v) = crate::model::PlayerView::of_world(world, object_id) else {
        return;
    };
    let cubics = world
        .objects
        .get_component::<super::cubic::Cubics>(&object_id)
        .map(|c| c.ids())
        .unwrap_or_default();
    // A hidden GM's CharInfo must not reach other players: Java's
    // `broadcastCharInfo` checks `isVisibleFor` per recipient; the port
    // suppresses wholesale, same as `visibility::send_char_info` (the
    // SEE_ALL_PLAYERS cond-override isn't modeled). Without this gate any
    // UserInfo-broadcasting action (transform, title, store, buff…) popped a
    // hidden GM back onto every nearby client.
    if world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&object_id)
        .is_some_and(|f| f.hidden)
    {
        return;
    }
    let char_info = server_packets::char_info(
        &v,
        &super::abnormal::visual_effects(world, object_id),
        &cubics,
        &char_info_state(world, object_id),
    );
    broadcast_to_others(world, object_id, &char_info);
}

/// Gather the manager-sourced `CharInfo` fields Java reads inside the packet
/// ctor (`CursedWeaponsManager`, `AttackStanceTaskManager`, the clan, death,
/// the fishing session) — see [`server_packets::CharInfoState`].
pub(crate) fn char_info_state(world: &World, object_id: i32) -> server_packets::CharInfoState {
    let p = world.objects.get_component::<Player>(&object_id);
    let clan = p
        .filter(|p| p.clan_id != 0)
        .and_then(|p| world.clans.get(&p.clan_id));
    server_packets::CharInfoState {
        in_combat: super::combat::has_attack_stance(world, object_id),
        // Java gates the byte on `!isInOlympiadMode()` so a downed Olympiad
        // fighter keeps standing until the match ends.
        alike_dead: !world.olympiad.is_in_competition(object_id)
            && world
                .objects
                .get_component::<crate::model::components::Vitals>(&object_id)
                .is_some_and(|v| v.dead),
        cursed_weapon_level: p
            .filter(|p| p.cursed_weapon_equipped_id != 0)
            .and_then(|p| {
                world
                    .cursed_weapons
                    .iter()
                    .find(|w| w.item_id == p.cursed_weapon_equipped_id)
            })
            .map_or(0, |w| w.level() as u8),
        clan_crest_large_id: clan.map_or(0, |c| c.crest_large_id),
        clan_reputation: clan.map_or(0, |c| c.reputation_score),
        fishing_bait: world
            .objects
            .get_component::<crate::model::components::FishingSession>(&object_id)
            .filter(|f| f.is_fishing)
            .map(|f| (f.bait_x, f.bait_y, f.bait_z)),
    }
}

fn player_name(world: &World, object_id: i32) -> String {
    world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| p.name.clone())
        .unwrap_or_default()
}

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
fn take_request(world: &mut World, object_id: i32) -> Option<PendingRequest> {
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
fn drop_party_if_unborn(world: &mut World, party_id: u32) {
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
            &[SmParam::Text(player_name(world, target))],
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
    if super::punishment::is_party_banned(world, requestor) {
        send_sm_to_player(
            world,
            requestor,
            sm_ids::YOU_HAVE_BEEN_REPORTED_AS_AN_ILLEGAL_PROGRAM_USER_SO_PARTICIPATING_IN_A_PARTY_IS_NOT_ALLOWED,
            &[],
        );
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }
    if super::punishment::is_party_banned(world, target) {
        send_sm_to_player(
            world,
            requestor,
            sm_ids::C1_HAS_BEEN_REPORTED_AS_AN_ILLEGAL_PROGRAM_USER_AND_CANNOT_JOIN_A_PARTY,
            &[SmParam::Text(player_name(world, target))],
        );
        return;
    }
    // Java `RequestJoinParty`: `BlockList.isBlocked(target, requestor)` — the
    // *invitee's* list decides. The refusal names the target, so the requestor
    // is told who is ignoring them.
    if super::block_list::is_blocked(world, target, requestor) {
        send_sm_to_player(
            world,
            requestor,
            sm_ids::C1_HAS_PLACED_YOU_ON_HIS_HER_IGNORE_LIST,
            &[SmParam::Text(player_name(world, target))],
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
        &[SmParam::Text(player_name(world, target))],
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
                server_packets::ask_join_party(&player_name(world, requestor), rule.id()),
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
                    server_packets::ask_join_party(&player_name(world, requestor), rule.id()),
                );
            } else {
                send_sm_to_player(
                    world,
                    requestor,
                    sm_ids::C1_IS_ON_ANOTHER_TASK_PLEASE_TRY_AGAIN_LATER,
                    &[SmParam::Text(player_name(world, target))],
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
            super::party_room::on_party_invite_accepted(world, requestor, player);
        }
    } else {
        if response == -1 {
            send_sm_to_player(
                world,
                requestor,
                sm_ids::C1_IS_SET_TO_REFUSE_PARTY_REQUESTS,
                &[SmParam::PlayerName(player_name(world, player))],
            );
        }
        // A declined first invite dissolves the embryo party.
        drop_party_if_unborn(world, party_id);
    }

    if let Some(party) = world.parties.get_mut(&party_id) {
        party.pending_invitation = false;
    }
}

/// `Party.addPartyMember` (pets/CC/duel/tactical-sign hooks dropped).
pub(crate) fn add_party_member(world: &mut World, party_id: u32, new_member: i32) {
    let Some(party) = world.parties.get(&party_id) else {
        return;
    };
    if party.contains(new_member) {
        return;
    }
    // A pending loot-rule vote dies on member change.
    if world.parties[&party_id].loot_change.is_some() {
        finish_loot_change(world, party_id, false);
    }

    let started_broadcast = {
        let party = world.parties.get_mut(&party_id).expect("checked");
        let first_add = party.members.len() == 1;
        party.members.push(new_member);
        first_add
    };
    world
        .objects
        .add_components(&new_member, PartyRef(party_id));

    let party = &world.parties[&party_id];
    let (leader, rule, members) = (party.leader(), party.distribution, party.members.clone());
    let leader_name = player_name(world, leader);
    let new_name = player_name(world, new_member);

    // New member: the full window (everyone but themselves) + "you joined".
    let others: Vec<PartyMemberView> = members
        .iter()
        .filter(|&&m| m != new_member)
        .filter_map(|&m| member_view(world, m))
        .collect();
    send_to_player(
        world,
        new_member,
        server_packets::party_small_window_all(leader, rule.id(), &others),
    );
    send_sm_to_player(
        world,
        new_member,
        sm_ids::YOU_HAVE_JOINED_S1_S_PARTY,
        &[SmParam::Text(leader_name)],
    );

    // Everyone (new member included, per Java's broadcast after add):
    // "C1 has joined the party"; existing members also get the Add window
    // entry and the new member's HP bar.
    let joined_sm = server_packets::system_message_with(
        sm_ids::C1_HAS_JOINED_THE_PARTY,
        &[SmParam::Text(new_name)],
    );
    broadcast_to_party(world, party_id, &joined_sm, None);
    if let Some(view) = member_view(world, new_member) {
        let add = server_packets::party_small_window_add(leader, rule.id(), &view);
        let su = server_packets::status_update(
            new_member,
            &[
                (server_packets::status_update_type::MAX_HP, view.max_hp),
                (server_packets::status_update_type::CUR_HP, view.hp),
            ],
        );
        for &m in &members {
            if m != new_member {
                send_to_player(world, m, add.clone());
            }
            send_to_player(world, m, su.clone());
        }
    }
    // `member.broadcastUserInfo()` for everyone (relation bits refresh).
    for &m in &members {
        broadcast_user_info(world, m);
    }

    // A member joining a channelled party gets the CC window opened (Java
    // `addPartyMember`'s `ExOpenMPCC` tail).
    super::command_channel::on_party_member_added(world, party_id, new_member);

    // The 12 s position broadcast starts with the first real member add
    // (Java: `_positionBroadcastTask == null`), initial delay = period / 2.
    if started_broadcast {
        let seq = world.parties[&party_id].seq;
        world.scheduler.schedule(
            world.tick + POSITION_BROADCAST_TICKS / 2,
            ScheduledTask::PartyPositionBroadcast { party_id, seq },
        );
    }
}

// ---------------------------------------------------------------------------
// Leave / oust / disband / leader change
// ---------------------------------------------------------------------------

/// `PartyMessageType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LeaveType {
    Left,
    Expelled,
    Disconnected,
    None,
}

pub(crate) fn handle_request_withdrawal_party(world: &mut World, client_id: u32) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    if let Some(PartyRef(party_id)) = world.objects.get_component::<PartyRef>(&player).copied() {
        remove_party_member(world, party_id, player, LeaveType::Left);
    }
    // Java `RequestWithDrawalParty` also drops the player from their matching
    // room (G30).
    super::party_room::on_party_withdraw(world, player);
}

pub(crate) fn handle_request_oust_party_member(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(name) = cp::read_name(body) else {
        return;
    };
    let Some(PartyRef(party_id)) = world.objects.get_component::<PartyRef>(&player).copied() else {
        return;
    };
    if !world
        .parties
        .get(&party_id)
        .is_some_and(|p| p.is_leader(player))
    {
        return;
    }
    let victim = world.parties[&party_id]
        .members
        .iter()
        .copied()
        .find(|&m| player_name(world, m).eq_ignore_ascii_case(&name));
    if let Some(victim) = victim {
        remove_party_member(world, party_id, victim, LeaveType::Expelled);
    }
}

pub(crate) fn handle_request_change_party_leader(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(name) = cp::read_name(body) else {
        return;
    };
    let Some(PartyRef(party_id)) = world.objects.get_component::<PartyRef>(&player).copied() else {
        return;
    };
    if !world
        .parties
        .get(&party_id)
        .is_some_and(|p| p.is_leader(player))
    {
        return;
    }
    let target = world.parties[&party_id]
        .members
        .iter()
        .copied()
        .find(|&m| player_name(world, m).eq_ignore_ascii_case(&name));
    let Some(target) = target else {
        // Java answers this to the *named* player object being absent from
        // the member list.
        send_sm_to_player(
            world,
            player,
            sm_ids::YOU_MAY_ONLY_TRANSFER_PARTY_LEADERSHIP,
            &[],
        );
        return;
    };
    if target == player {
        send_sm_to_player(
            world,
            player,
            sm_ids::SLOW_DOWN_YOU_ARE_ALREADY_THE_PARTY_LEADER,
            &[],
        );
        return;
    }
    {
        let party = world.parties.get_mut(&party_id).expect("checked");
        let idx = party
            .members
            .iter()
            .position(|&m| m == target)
            .expect("checked");
        party.members.swap(0, idx);
    }
    announce_new_leader(world, party_id);
    // CC authority follows the leading party's leadership (Java
    // `Party.setLeader`'s CC tail, SM 1589).
    super::command_channel::on_party_leader_changed(world, party_id, player, target);
}

/// SM 1384 + `broadcastToPartyMembersNewLeader` (window rebuild for all).
fn announce_new_leader(world: &mut World, party_id: u32) {
    let leader_name = player_name(world, world.parties[&party_id].leader());
    let sm = server_packets::system_message_with(
        sm_ids::C1_HAS_BECOME_THE_PARTY_LEADER,
        &[SmParam::Text(leader_name)],
    );
    broadcast_to_party(world, party_id, &sm, None);

    let party = &world.parties[&party_id];
    let (leader, rule, members) = (party.leader(), party.distribution, party.members.clone());
    for &m in &members {
        send_to_player(world, m, server_packets::party_small_window_delete_all());
        let others: Vec<PartyMemberView> = members
            .iter()
            .filter(|&&o| o != m)
            .filter_map(|&o| member_view(world, o))
            .collect();
        send_to_player(
            world,
            m,
            server_packets::party_small_window_all(leader, rule.id(), &others),
        );
        broadcast_user_info(world, m);
    }
}

/// `Party.removePartyMember(player, type)`.
pub(crate) fn remove_party_member(
    world: &mut World,
    party_id: u32,
    leaver: i32,
    leave_type: LeaveType,
) {
    let Some(party) = world.parties.get(&party_id) else {
        return;
    };
    if !party.contains(leaver) {
        return;
    }
    let was_leader = party.is_leader(leaver);
    let two_left = party.members.len() == 2;
    let leader_quit = was_leader
        && !world.cfg.character.alt_leave_party_leader
        && leave_type != LeaveType::Disconnected
        && leave_type != LeaveType::None;
    if two_left || leader_quit {
        disband_party(world, party_id);
        return;
    }

    {
        let party = world.parties.get_mut(&party_id).expect("checked");
        party.members.retain(|&m| m != leaver);
        if party.loot_change.is_some() {
            // Member change voids the vote (answers count against a stale
            // member set otherwise).
            finish_loot_change_inline(party);
        }
    }
    world.objects.remove_component::<PartyRef>(&leaver);

    let leaver_name = player_name(world, leaver);
    match leave_type {
        LeaveType::Expelled => {
            send_sm_to_player(
                world,
                leaver,
                sm_ids::YOU_HAVE_BEEN_EXPELLED_FROM_THE_PARTY,
                &[],
            );
            let sm = server_packets::system_message_with(
                sm_ids::C1_WAS_EXPELLED_FROM_THE_PARTY,
                &[SmParam::Text(leaver_name.clone())],
            );
            broadcast_to_party(world, party_id, &sm, None);
        }
        LeaveType::Left | LeaveType::Disconnected => {
            send_sm_to_player(
                world,
                leaver,
                sm_ids::YOU_HAVE_WITHDRAWN_FROM_THE_PARTY,
                &[],
            );
            let sm = server_packets::system_message_with(
                sm_ids::C1_HAS_LEFT_THE_PARTY,
                &[SmParam::Text(leaver_name.clone())],
            );
            broadcast_to_party(world, party_id, &sm, None);
        }
        LeaveType::None => {}
    }

    send_to_player(
        world,
        leaver,
        server_packets::party_small_window_delete_all(),
    );
    let delete = server_packets::party_small_window_delete(leaver, &leaver_name);
    broadcast_to_party(world, party_id, &delete, None);
    broadcast_user_info(world, leaver);

    // The leaver's CC window closes (Java `removePartyMember`'s `ExCloseMPCC`
    // tail). Java also leaves `CommandChannel._commandLeader` stale when the
    // CC leader disconnects but their party survives — kept.
    super::command_channel::on_party_member_removed(world, party_id, leaver);

    if was_leader {
        announce_new_leader(world, party_id);
    }
}

/// `Party.disbandParty` — SM 203 to everyone, all windows cleared, party
/// gone. (Java re-enters `removePartyMember` per member with `_disbanding`
/// set; the observable packets are the dissolve SM + each member's
/// `PartySmallWindowDeleteAll`, which this sends directly.)
pub(crate) fn disband_party(world: &mut World, party_id: u32) {
    // A collapsing party takes its command channel down with it when it is
    // the CC leader's party, or just detaches otherwise (Java
    // `removePartyMember`'s size==1 branch).
    super::command_channel::on_party_dissolving(world, party_id);
    let Some(party) = world.parties.get_mut(&party_id) else {
        return;
    };
    party.seq = party.seq.wrapping_add(1); // kill outstanding tasks
    let members = party.members.clone();
    let sm = server_packets::system_message_with(sm_ids::THE_PARTY_HAS_DISPERSED, &[]);
    for &m in &members {
        world.objects.remove_component::<PartyRef>(&m);
        send_to_player(world, m, sm.clone());
        send_to_player(world, m, server_packets::party_small_window_delete_all());
    }
    world.parties.remove(&party_id);
    for &m in &members {
        broadcast_user_info(world, m);
    }
}

/// Player left the world (logout/restart/disconnect): party + request
/// cleanup. Called by `store_and_remove_player`.
pub(crate) fn on_player_leave_world(world: &mut World, object_id: i32) {
    if let Some(req) = clear_linked_request(world, object_id) {
        // A vanished inviter's embryo party dies with the invite.
        if let RequestKind::PartyInvite { party_id } = req.kind {
            drop_party_if_unborn(world, party_id);
        }
    }
    if let Some(PartyRef(party_id)) = world.objects.get_component::<PartyRef>(&object_id).copied() {
        remove_party_member(world, party_id, object_id, LeaveType::Disconnected);
    }
}

// ---------------------------------------------------------------------------
// Loot-rule voting (`requestLootChange` / `answerLootChangeRequest`)
// ---------------------------------------------------------------------------

pub(crate) fn handle_request_party_loot_modification(
    world: &mut World,
    client_id: u32,
    body: &[u8],
) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(rule) = cp::read_answer(body).and_then(LootRule::from_id) else {
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
    let leader_name = player_name(world, player);
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
    let answer = cp::read_answer(body).unwrap_or(0);
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
fn finish_loot_change(world: &mut World, party_id: u32, success: bool) {
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
fn finish_loot_change_inline(party: &mut Party) {
    party.loot_change = None;
}

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

// ---------------------------------------------------------------------------
// Party rewards (`Party.distributeXpAndSp` + `distributeItem`/`distributeAdena`)
// ---------------------------------------------------------------------------

/// XP/SP for one party's damage share against one kill — the party branch of
/// `Attackable.calculateRewards` + `Party.distributeXpAndSp`.
/// `base_exp`/`base_sp` already carry `partyDmg/totalDamage × partyMul ×
/// level-gap` (the caller ports `calculateExpAndSp`); this adds the party
/// bonus ladder and splits by level².
pub(crate) fn distribute_xp_and_sp(
    world: &mut World,
    rewarded: &[(i32, i32)], // (object_id, level), alive + in range
    top_level: i32,
    base_exp: f64,
    base_sp: f64,
    // The killed monster's template — needed for the per-member vitality
    // charge (`target.getVitalityPoints(...)` in Java's loop).
    target: &crate::data::npc_data::NpcTemplate,
    // Java `Attackable.useVitalityRate()` — false for a champion unless
    // `ChampionEnableVitality`. It gates three things at once: the bonus
    // multiplier inside `addExpAndSp`, the vitality charge, and the PA points.
    use_vitality_rate: bool,
) {
    let cfg = &world.cfg.character;
    let valid = crate::model::party::valid_members(
        rewarded,
        top_level,
        &cfg.party_xp_cutoff_method,
        cfg.party_xp_cutoff_level,
        cfg.party_xp_cutoff_percent,
    );
    let xp_reward =
        base_exp * crate::model::party::exp_sp_bonus(valid.len(), world.cfg.rates.rate_party_xp);
    let sp_reward =
        base_sp * crate::model::party::exp_sp_bonus(valid.len(), world.cfg.rates.rate_party_sp);
    let sq_level_sum: f64 = rewarded
        .iter()
        .filter(|(id, _)| valid.contains(id))
        .map(|&(_, l)| (l as f64) * (l as f64))
        .sum();
    if sq_level_sum <= 0.0 {
        return;
    }

    let highfive = cfg.party_xp_cutoff_method == "highfive";
    let (gaps, percents) = (
        cfg.party_xp_cutoff_gaps.clone(),
        cfg.party_xp_cutoff_gap_percents.clone(),
    );
    for &(member, level) in rewarded {
        if !valid.contains(&member) {
            continue; // Java: `member.addExpAndSp(0, 0)` — a no-op here.
        }
        let pre = (level as f64) * (level as f64) / sq_level_sum;
        let mut xp = xp_reward * pre;
        let mut sp = sp_reward * pre;
        // `calculateExpSpPartyCutoff`: premium rates first, then the cutoff.
        if super::admin::premium::has_premium_status(world, member) {
            xp *= world.cfg.premium.rate_xp;
            sp *= world.cfg.premium.rate_sp;
        }
        if highfive {
            match crate::model::party::highfive_cutoff_percent(top_level - level, &gaps, &percents)
            {
                Some(pct) => {
                    xp = xp * pct as f64 / 100.0;
                    sp = sp * pct as f64 / 100.0;
                }
                None => continue, // outside every gap range: nothing at all
            }
        }
        super::death::add_exp_and_sp(world, member, xp, sp, use_vitality_rate);
        // Java charges each rewarded member's vitality on the post-cutoff xp,
        // and awards that member's PA points from the same value — both inside
        // the same `if (useVitalityRate())`.
        if xp > 0.0 && use_vitality_rate {
            super::death::consume_kill_vitality(world, member, level, target, xp);
            super::pc_cafe::give_point(world, member, xp);
        }
    }
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
    let in_range: Vec<i32> = members
        .iter()
        .copied()
        .filter(|&m| {
            world
                .objects
                .get_component::<Position>(&m)
                .is_some_and(|p| {
                    let (dx, dy) = ((p.x - corpse.0) as f64, (p.y - corpse.1) as f64);
                    (dx * dx + dy * dy).sqrt() <= range
                })
        })
        .collect();
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
    const ADENA_ID: i32 = 57;
    let range = world.cfg.character.alt_party_range as f64;
    let Some((members, rule, last_loot)) = world
        .parties
        .get(&party_id)
        .map(|p| (p.members.clone(), p.distribution, p.item_last_loot))
    else {
        super::death::give_item(world, killer, item_id, count);
        return;
    };
    let in_range: Vec<i32> = members
        .iter()
        .copied()
        .filter(|&m| {
            world
                .objects
                .get_component::<Position>(&m)
                .is_some_and(|p| {
                    let (dx, dy) = ((p.x - corpse.0) as f64, (p.y - corpse.1) as f64);
                    (dx * dx + dy * dy).sqrt() <= range
                })
        })
        .collect();

    if item_id == ADENA_ID {
        // `distributeAdena` — an even split over the in-range members.
        if in_range.is_empty() {
            return;
        }
        let share = count / in_range.len() as i64;
        if share > 0 {
            for m in in_range {
                super::death::give_item(world, m, item_id, share);
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

    super::death::give_item(world, looter, item_id, count);

    // "C1 has obtained …" to the rest of the party.
    let looter_name = player_name(world, looter);
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
