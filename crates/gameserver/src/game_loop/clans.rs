//! Clans — the G11 creation/display slice: `ClanTable.createClan` behind
//! the village-master `create_clan` bypass, the pledge-window packets, and
//! the enter/leave-world roster notifications. Invites/wars/levels/crests
//! and everything else clan stay deferred (see the G11 plan).

use commons::network::PacketReader;
use tracing::warn;

use crate::db::DbCommand;
use crate::model::clan::{Clan, ClanMember, ALL_CLAN_PRIVILEGES};
use crate::model::Player;
use crate::network::server_packets::{self, sm_ids, SmParam};
use crate::world::World;

use super::helpers::client_for_player;

fn send_sm(world: &World, client_id: u32, id: i16) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(crate::network::enter_world::system_message(id));
    }
}

/// Wall-clock millis (Java `System.currentTimeMillis()`).
fn now_millis() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// `DaysBeforeCreateAClan = 10` on this dist → the recreate cooldown in millis.
const CLAN_CREATE_COOLDOWN_MS: i64 = 10 * 86_400_000;

/// `VillageMaster.onBypassFeedback`'s `create_clan` branch: parse the typed name
/// (rejecting embedded spaces, Java's `isValidName` reject) then run
/// [`create_clan`] for the acting player. `args` is everything after the verb.
pub(crate) fn handle_create_clan(world: &mut World, client_id: u32, player_oid: i32, args: &str) {
    let mut tokens = args.split(' ').filter(|t| !t.is_empty());
    let Some(name) = tokens.next().map(str::to_string) else { return }; // empty → silent, like Java
    if tokens.next().is_some() {
        // A second token means the typed name had a space — Java folds this
        // into the isValidName reject. (`ClanNameTemplate = .*` on this
        // dist, so the regex itself is not ported.)
        send_sm(world, client_id, sm_ids::CLAN_NAME_IS_INVALID);
        return;
    }
    create_clan(world, player_oid, &name);
}

/// Core `ClanTable.createClan(leader, name)`: the guards in Java's order, then
/// the insert + the pledge windows sent to the leader's own client. Returns the
/// new clan id, or `None` on any guarded failure (the matching sysmsg has
/// already gone to the leader). Admin `//pledge create` bypasses the recreate
/// cooldown the same way Java does — by zeroing the target's expiry field
/// *before* calling this (and restoring it on failure), not via a flag here.
pub(crate) fn create_clan(world: &mut World, leader_oid: i32, name: &str) -> Option<i32> {
    let leader_client = client_for_player(world, leader_oid)?;
    let name = name.to_string();

    // --- ClanTable.createClan guards, in order ---
    let p = world.objects.get_component::<Player>(&leader_oid)?;
    if p.level < 10 {
        send_sm(world, leader_client, sm_ids::YOU_DO_NOT_MEET_THE_CRITERIA_IN_ORDER_TO_CREATE_A_CLAN);
        return None;
    }
    if p.clan_id != 0 {
        send_sm(world, leader_client, sm_ids::YOU_HAVE_FAILED_TO_CREATE_A_CLAN);
        return None;
    }
    if now_millis() < p.clan_create_expiry_time {
        send_sm(world, leader_client, sm_ids::YOU_MUST_WAIT_10_DAYS_BEFORE_CREATING_A_NEW_CLAN);
        return None;
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric()) || name.len() < 2 {
        send_sm(world, leader_client, sm_ids::CLAN_NAME_IS_INVALID);
        return None;
    }
    if name.len() > 16 {
        send_sm(world, leader_client, sm_ids::CLAN_NAME_S_LENGTH_IS_INCORRECT);
        return None;
    }
    if world.clans.values().any(|c| c.name.eq_ignore_ascii_case(&name)) {
        if let Some(cs) = world.clients.get(&leader_client) {
            cs.send(server_packets::system_message_with(
                sm_ids::S1_ALREADY_EXISTS,
                &[SmParam::Text(name.clone())],
            ));
        }
        return None;
    }

    // --- create ---
    let Some(clan_id) = world.alloc_object_id() else {
        warn!("create_clan: object-id pool exhausted.");
        return None;
    };
    let leader = {
        let p = world.objects.get_component::<Player>(&leader_oid).expect("checked above");
        ClanMember {
            char_id: leader_oid,
            name: p.name.clone(),
            level: p.level,
            class_id: p.class_id,
            sex: p.is_female as i32,
            race: p.race,
        }
    };
    let clan = Clan { id: clan_id, name: name.clone(), leader_id: leader_oid, level: 0, reputation_score: 0, castle_id: 0, members: vec![leader], warehouse: Default::default() };
    let _ = world.db.send(DbCommand::InsertClan { clan_id, name: name.clone(), leader_id: leader_oid });
    let _ = world.db.send(DbCommand::UpdateCharClan {
        char_id: leader_oid,
        clan_id,
        clan_privs: ALL_CLAN_PRIVILEGES,
    });
    {
        let p = world.objects.get_component_mut::<Player>(&leader_oid).expect("checked above");
        p.clan_id = clan_id;
        p.clan_privs = ALL_CLAN_PRIVILEGES;
        p.clan_leader = true;
    }

    if let Some(cs) = world.clients.get(&leader_client) {
        cs.send(server_packets::pledge_show_info_update(&clan));
        cs.send(server_packets::pledge_show_member_list_all(&clan, &world.objects));
        if let Some(m) = clan.member(leader_oid) {
            cs.send(server_packets::pledge_show_member_list_update(m, true));
        }
    }
    send_sm(world, leader_client, sm_ids::YOUR_CLAN_HAS_BEEN_CREATED);
    world.clans.insert(clan_id, clan);
    // `broadcastUserInfo(RELATION, CLAN)` — the full re-send stands in
    // (same G10 substitution for RelationChanged).
    super::party::broadcast_user_info(world, leader_oid);
    Some(clan_id)
}

/// `Clan.changeLevel` (admin `//pledge setlevel`): set the level, persist it,
/// and refresh every online member — Java broadcasts `YOUR_CLAN_S_LEVEL_HAS_
/// INCREASED` + `PledgeShowInfoUpdate`, and AdminPledge re-broadcasts UserInfo
/// (`RELATION, CLAN`). Siege-skill grant/removal on the level-5 boundary is
/// deferred (no siege system yet).
pub(crate) fn set_clan_level(world: &mut World, clan_id: i32, level: i32) {
    let member_ids: Vec<i32> = {
        let Some(clan) = world.clans.get_mut(&clan_id) else { return };
        clan.level = level;
        clan.members.iter().map(|m| m.char_id).collect()
    };
    let _ = world.db.send(DbCommand::UpdateClanLevel { clan_id, level });
    let info = server_packets::pledge_show_info_update(world.clans.get(&clan_id).expect("checked above"));
    broadcast_to_clan(world, clan_id, &info);
    broadcast_to_clan(world, clan_id, &crate::network::enter_world::system_message(sm_ids::YOUR_CLAN_S_LEVEL_HAS_INCREASED));
    for oid in member_ids {
        super::party::broadcast_user_info(world, oid);
    }
}

/// `Clan.addReputationScore` (admin `//pledge rep`): add signed points, clamp,
/// persist, and refresh every online member's pledge window. Returns the new
/// score. Clan-skill (de)activation on the zero crossing is deferred (clan
/// skills unported).
pub(crate) fn add_clan_reputation(world: &mut World, clan_id: i32, points: i32) -> Option<i32> {
    let new_score = world.clans.get_mut(&clan_id)?.add_reputation_score(points);
    let _ = world.db.send(DbCommand::UpdateClanReputation { clan_id, reputation: new_score });
    let info = server_packets::pledge_show_info_update(world.clans.get(&clan_id).expect("checked above"));
    broadcast_to_clan(world, clan_id, &info);
    Some(new_score)
}

/// `ClanTable.destroyClan` (admin `//pledge dismiss`), narrowed to what the port
/// models: broadcast `CLAN_HAS_DISPERSED`, reset every member's clan state (in
/// memory for the online ones + a blanket DB reset for all), close their clan
/// windows, and drop the clan. Siege/fort/clan-hall/ally/war teardown, apprentice
/// links, and clan-skill removal are deferred (those systems are unported).
pub(crate) fn destroy_clan(world: &mut World, clan_id: i32) {
    let (leader_id, member_ids) = {
        let Some(clan) = world.clans.get(&clan_id) else { return };
        (clan.leader_id, clan.members.iter().map(|m| m.char_id).collect::<Vec<_>>())
    };
    // Java broadcasts the dispersal message before tearing the roster down.
    broadcast_to_clan(world, clan_id, &crate::network::enter_world::system_message(sm_ids::CLAN_HAS_DISPERSED));

    // Java stamps the recreate cooldown on the (online) leader in removeClanMember.
    let leader_expiry = now_millis() + CLAN_CREATE_COOLDOWN_MS;
    let delete_all = server_packets::pledge_show_member_list_delete_all();
    for oid in &member_ids {
        let online = {
            if let Some(p) = world.objects.get_component_mut::<Player>(oid) {
                p.clan_id = 0;
                p.clan_privs = 0;
                p.clan_leader = false;
                if *oid == leader_id {
                    p.clan_create_expiry_time = leader_expiry;
                }
                // TODO(G25): Java clears the title only for non-nobles; the noble
                // system is unported, so every ex-member loses their title here.
                p.title.clear();
                true
            } else {
                false
            }
        };
        if online {
            if let Some(cid) = client_for_player(world, *oid) {
                if let Some(cs) = world.clients.get(&cid) {
                    cs.send(delete_all.clone());
                }
            }
        }
    }
    world.clans.remove(&clan_id);
    let _ = world.db.send(DbCommand::DestroyClan { clan_id, leader_id, leader_expiry });
    // broadcastUserInfo for the now clan-less online members.
    for oid in member_ids {
        super::party::broadcast_user_info(world, oid);
    }
}

/// `RequestPledgeInfo.runImpl`: answer with the clan's name/ally names for a
/// clan id (Java resolves through `ClanTable.getClan`; unknown ids are
/// silently dropped, matching the "should not happen" early return).
pub(crate) fn handle_request_pledge_info(world: &World, client_id: u32, body: &[u8]) {
    // Java guards on a logged-in player before touching the clan table.
    if !matches!(world.clients.get(&client_id), Some(crate::session::ClientSession::InGame(_))) {
        return;
    }
    let Some(clan_id) = PacketReader::new(body).read_i32() else { return };
    let Some(clan) = world.clans.get(&clan_id) else { return };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::pledge_info(clan));
    }
}

/// `EnterWorld.runImpl`'s clan section (narrowed): fix the leader flag from
/// the live table, refresh the member's level in the roster, send the
/// pledge window to the enterer and the online-status update to the rest.
pub(crate) fn on_enter_world(world: &mut World, client_id: u32, object_id: i32) {
    let Some(p) = world.objects.get_component::<Player>(&object_id) else { return };
    let clan_id = p.clan_id;
    if clan_id == 0 {
        return;
    }
    let level = p.level;
    let is_leader = world.clans.get(&clan_id).is_some_and(|c| c.leader_id == object_id);
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.clan_leader = is_leader;
    }
    let Some(clan) = world.clans.get_mut(&clan_id) else {
        warn!("Player {object_id} carries unknown clan id {clan_id}.");
        return;
    };
    if let Some(m) = clan.members.iter_mut().find(|m| m.char_id == object_id) {
        m.level = level;
    }
    let clan = world.clans.get(&clan_id).expect("checked above");
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::pledge_show_member_list_all(clan, &world.objects));
    }
    notify_members(world, clan_id, object_id, true);
}

/// `Player.deleteMe`'s clan half: the offline ping to online members.
pub(crate) fn on_leave_world(world: &World, object_id: i32, clan_id: i32) {
    if clan_id != 0 {
        notify_members(world, clan_id, object_id, false);
    }
}

/// `PledgeShowMemberListUpdate` about `subject` to every *other* online
/// clan member.
fn notify_members(world: &World, clan_id: i32, subject: i32, online: bool) {
    let Some(clan) = world.clans.get(&clan_id) else { return };
    let Some(subject_member) = clan.member(subject) else { return };
    let pkt = server_packets::pledge_show_member_list_update(subject_member, online);
    for m in &clan.members {
        if m.char_id == subject {
            continue;
        }
        if let Some(cid) = client_for_player(world, m.char_id) {
            if let Some(cs) = world.clients.get(&cid) {
                cs.send(pkt.clone());
            }
        }
    }
}

/// Clan chat (`ChatType::Clan` in `Say2`): `CreatureSay` to every online
/// member including the speaker (Java `Clan.broadcastToOnlineMembers`).
pub(crate) fn broadcast_to_clan(world: &World, clan_id: i32, pkt: &[u8]) {
    let Some(clan) = world.clans.get(&clan_id) else { return };
    for m in &clan.members {
        if let Some(cid) = client_for_player(world, m.char_id) {
            if let Some(cs) = world.clients.get(&cid) {
                cs.send(pkt.to_vec());
            }
        }
    }
}
