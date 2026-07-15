//! Clans — the G11 creation/display slice: `ClanTable.createClan` behind
//! the village-master `create_clan` bypass, the pledge-window packets, and
//! the enter/leave-world roster notifications. Invites/wars/levels/crests
//! and everything else clan stay deferred (see the G11 plan).

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

/// `VillageMaster.onBypassFeedback`'s `create_clan` branch +
/// `ClanTable.createClan`, guards in Java's order. `args` is everything
/// after the verb (the `$name` the client substituted).
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

    // --- ClanTable.createClan guards, in order ---
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else { return };
    if p.level < 10 {
        send_sm(world, client_id, sm_ids::YOU_DO_NOT_MEET_THE_CRITERIA_IN_ORDER_TO_CREATE_A_CLAN);
        return;
    }
    if p.clan_id != 0 {
        send_sm(world, client_id, sm_ids::YOU_HAVE_FAILED_TO_CREATE_A_CLAN);
        return;
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    if now_ms < p.clan_create_expiry_time {
        send_sm(world, client_id, sm_ids::YOU_MUST_WAIT_10_DAYS_BEFORE_CREATING_A_NEW_CLAN);
        return;
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric()) || name.len() < 2 {
        send_sm(world, client_id, sm_ids::CLAN_NAME_IS_INVALID);
        return;
    }
    if name.len() > 16 {
        send_sm(world, client_id, sm_ids::CLAN_NAME_S_LENGTH_IS_INCORRECT);
        return;
    }
    if world.clans.values().any(|c| c.name.eq_ignore_ascii_case(&name)) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(
                sm_ids::S1_ALREADY_EXISTS,
                &[SmParam::Text(name.clone())],
            ));
        }
        return;
    }

    // --- create ---
    let Some(clan_id) = world.alloc_object_id() else {
        warn!("create_clan: object-id pool exhausted.");
        return;
    };
    let leader = {
        let p = world.objects.get_component::<Player>(&player_oid).expect("checked above");
        ClanMember {
            char_id: player_oid,
            name: p.name.clone(),
            level: p.level,
            class_id: p.class_id,
            sex: p.is_female as i32,
            race: p.race,
        }
    };
    let clan = Clan { id: clan_id, name: name.clone(), leader_id: player_oid, level: 0, members: vec![leader], warehouse: Default::default() };
    let _ = world.db.send(DbCommand::InsertClan { clan_id, name: name.clone(), leader_id: player_oid });
    let _ = world.db.send(DbCommand::UpdateCharClan {
        char_id: player_oid,
        clan_id,
        clan_privs: ALL_CLAN_PRIVILEGES,
    });
    {
        let p = world.objects.get_component_mut::<Player>(&player_oid).expect("checked above");
        p.clan_id = clan_id;
        p.clan_privs = ALL_CLAN_PRIVILEGES;
        p.clan_leader = true;
    }

    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::pledge_show_info_update(&clan));
        cs.send(server_packets::pledge_show_member_list_all(&clan, &world.objects));
        if let Some(m) = clan.member(player_oid) {
            cs.send(server_packets::pledge_show_member_list_update(m, true));
        }
    }
    send_sm(world, client_id, sm_ids::YOUR_CLAN_HAS_BEEN_CREATED);
    world.clans.insert(clan_id, clan);
    // `broadcastUserInfo(RELATION, CLAN)` — the full re-send stands in
    // (same G10 substitution for RelationChanged).
    super::party::broadcast_user_info(world, player_oid);
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
