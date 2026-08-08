//! Clans — Java `ClanTable` / `Clan` and the pledge packet surface.
//!
//! This module keeps the clan core: `create_clan` (behind the village-master
//! `create_clan` bypass) and `destroy_clan`, level and reputation, the
//! enter/leave-world roster notifications, the pledge-info packets, and the
//! small shared helpers the rest of the module leans on (`send_sm_with`,
//! `broadcast_to_clan`, `has_clan_privilege`, `online_members`).
//!
//! Everything else lives in a submodule; all of it is re-exported here, so
//! callers keep saying `clans::handle_request_join_pledge` and never name a
//! submodule:
//!
//! - `skills` — pledge + residential skills, the Clan Advent leader aura,
//!   `//give_clan_skills`, and re-applying them to each member on login.
//! - `membership` — invite / answer / leave / oust, and the village-master
//!   `dissolve_clan`/`recover_clan` verbs with the delayed
//!   `ScheduledTask::ClanDissolve` removal.
//! - `ranks` — clan level-up, the pledge skill-learning window, rank
//!   privileges and member power grades, and the leader change flow.
//! - `wars` — declare / stop / surrender, the war-state timeout, and the
//!   on-kill reputation accounting.
//! - `alliance` — create / dissolve / join / leave / dismiss.
//! - `sub_pledge` — academy, royal guard and knight units.
//! - `crests` — pledge, large pledge and ally crests.
//! - `recruit` — the clan-entry board: recruit list, waiting list, draft
//!   list and applications.

pub(crate) use crate::game_loop::helpers::class_level;
use commons::network::PacketReader;
use tracing::warn;

use crate::db::DbCommand;
use crate::model::Player;
use crate::model::clan::{ALL_CLAN_PRIVILEGES, CL_DISMISS, CL_JOIN_CLAN, Clan, ClanMember};
use crate::model::components::ClanSkills;
use crate::model::skill::ActiveBuff;
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::world::World;

use super::helpers::client_for_player;

mod alliance;
mod crests;
mod membership;
mod ranks;
mod recruit;
pub(crate) mod skills;
mod sub_pledge;
pub(crate) mod wars;

pub(crate) use crate::game_loop::helpers::player_name_or_empty;
pub(crate) use crate::game_loop::helpers::{
    send_sm_bare_to_client as send_sm, send_sm_to_player as send_sm_with,
};
pub(crate) use alliance::*;
pub(crate) use crests::*;
pub(crate) use membership::*;
pub(crate) use ranks::*;
pub(crate) use recruit::*;
pub(crate) use skills::*;
pub(crate) use sub_pledge::*;
pub(crate) use wars::*;

/// Wall-clock millis (Java `System.currentTimeMillis()`).
pub(crate) fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// `DaysBeforeCreateAClan = 10` on this dist → the recreate cooldown in millis.
pub(crate) const CLAN_CREATE_COOLDOWN_MS: i64 = 10 * 86_400_000;

/// `CommonSkill.CLAN_ADVENT` (skill 19009 lv.1): the clan-leader-online aura —
/// PAtk/PDef/MDef +5%, MAtk +6%, HP/MP regen +5 on every clan member while the
/// leader is logged in. `abnormalTime=-1` (permanent) + `irreplacableBuff`, so
/// it lasts until explicitly removed on the leader's logout / clan dispersal.
pub(crate) const CLAN_ADVENT_SKILL_ID: i32 = 19009;
pub(crate) const CLAN_ADVENT_SKILL_LEVEL: i32 = 1;

/// `VillageMaster.onBypassFeedback`'s `create_clan` branch: parse the typed name
/// (rejecting embedded spaces, Java's `isValidName` reject) then run
/// [`create_clan`] for the acting player. `args` is everything after the verb.
pub(crate) fn handle_create_clan(world: &mut World, client_id: u32, player_oid: i32, args: &str) {
    let mut tokens = args.split(' ').filter(|t| !t.is_empty());
    let Some(name) = tokens.next().map(str::to_string) else {
        return;
    }; // empty → silent, like Java
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
        send_sm(
            world,
            leader_client,
            sm_ids::YOU_DO_NOT_MEET_THE_CRITERIA_IN_ORDER_TO_CREATE_A_CLAN,
        );
        return None;
    }
    if p.clan_id != 0 {
        send_sm(
            world,
            leader_client,
            sm_ids::YOU_HAVE_FAILED_TO_CREATE_A_CLAN,
        );
        return None;
    }
    if now_millis() < p.clan_create_expiry_time {
        send_sm(
            world,
            leader_client,
            sm_ids::YOU_MUST_WAIT_10_DAYS_BEFORE_CREATING_A_NEW_CLAN,
        );
        return None;
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric()) || name.len() < 2 {
        send_sm(world, leader_client, sm_ids::CLAN_NAME_IS_INVALID);
        return None;
    }
    if name.len() > 16 {
        send_sm(
            world,
            leader_client,
            sm_ids::CLAN_NAME_S_LENGTH_IS_INCORRECT,
        );
        return None;
    }
    if world
        .clans
        .values()
        .any(|c| c.name.eq_ignore_ascii_case(&name))
    {
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
        let p = world
            .objects
            .get_component::<Player>(&leader_oid)
            .expect("checked above");
        ClanMember {
            char_id: leader_oid,
            name: p.name.clone(),
            level: p.level,
            class_id: p.class_id,
            sex: p.is_female as i32,
            race: p.race,
            power_grade: 1, // Java restore: the leader holds grade 1
            title: p.title.clone(),
            pledge_type: 0,
            apprentice: 0,
            sponsor: 0,
        }
    };
    let clan = Clan {
        id: clan_id,
        name: name.clone(),
        leader_id: leader_oid,
        level: 0,
        reputation_score: 0,
        castle_id: 0,
        members: vec![leader],
        skills: Default::default(),
        warehouse: Default::default(),
        char_penalty_expiry_time: 0,
        dissolving_expiry_time: 0,
        rank_privs: Default::default(),
        new_leader_id: 0,
        sub_pledges: Default::default(),
        ally_id: 0,
        ally_name: String::new(),
        ally_penalty_expiry_time: 0,
        ally_penalty_type: 0,
        crest_id: 0,
        crest_large_id: 0,
        ally_crest_id: 0,
        blood_alliance_count: 0,
    };
    let _ = world.db.send(DbCommand::InsertClan {
        clan_id,
        name: name.clone(),
        leader_id: leader_oid,
    });
    let _ = world.db.send(DbCommand::UpdateCharClan {
        char_id: leader_oid,
        clan_id,
        clan_privs: ALL_CLAN_PRIVILEGES,
    });
    {
        let p = world
            .objects
            .get_component_mut::<Player>(&leader_oid)
            .expect("checked above");
        p.clan_id = clan_id;
        p.clan_privs = ALL_CLAN_PRIVILEGES;
        p.clan_leader = true;
        p.power_grade = 1;
        p.pledge_class = clan.pledge_class_of(leader_oid); // 0 at level 0
    }

    if let Some(cs) = world.clients.get(&leader_client) {
        cs.send(server_packets::pledge_show_info_update(&clan));
        cs.send(server_packets::pledge_show_member_list_all(
            &clan,
            &world.objects,
        ));
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
        let Some(clan) = world.clans.get_mut(&clan_id) else {
            return;
        };
        clan.level = level;
        clan.members.iter().map(|m| m.char_id).collect()
    };
    let _ = world.db.send(DbCommand::UpdateClanLevel { clan_id, level });
    let info =
        server_packets::pledge_show_info_update(world.clans.get(&clan_id).expect("checked above"));
    broadcast_to_clan(world, clan_id, &info);
    broadcast_to_clan(
        world,
        clan_id,
        &crate::network::enter_world::system_message(sm_ids::YOUR_CLAN_S_LEVEL_HAS_INCREASED),
    );
    for oid in member_ids {
        // The level change may cross a pledge-class boundary (the on-head crown);
        // recompute per member before the UserInfo/CharInfo re-broadcast.
        let pledge_class = world
            .clans
            .get(&clan_id)
            .map_or(0, |c| c.pledge_class_of(oid));
        if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
            p.pledge_class = pledge_class;
        }
        super::party::broadcast_user_info(world, oid);
    }
    // Java `Clan.changeLevel`: on reaching the siege min level the online leader
    // gains the siege/leader skills (`SiegeManager.addSiegeSkills(leader)`).
    let leader_id = world.clans.get(&clan_id).map(|c| c.leader_id).unwrap_or(0);
    if leader_id != 0 && client_for_player(world, leader_id).is_some() {
        apply_siege_skills_to_leader(world, clan_id, leader_id);
        // Java `changeLevel`: crossing level 5 tells the leader the clan can now
        // accumulate reputation.
        if level > 4
            && let Some(cid) = client_for_player(world, leader_id)
        {
            send_sm(
                world,
                cid,
                sm_ids::NOW_THAT_YOUR_CLAN_LEVEL_IS_ABOVE_LEVEL_5_IT_CAN_ACCUMULATE_CLAN_REPUTATION,
            );
        }
    }
}

/// `Clan.addReputationScore` (admin `//pledge rep`): add signed points, clamp,
/// persist, and refresh every online member's pledge window. Returns the new
/// score. Clan-skill (de)activation on the zero crossing is deferred (clan
/// skills unported).
pub(crate) fn add_clan_reputation(world: &mut World, clan_id: i32, points: i32) -> Option<i32> {
    let new_score = world.clans.get_mut(&clan_id)?.add_reputation_score(points);
    let _ = world.db.send(DbCommand::UpdateClanReputation {
        clan_id,
        reputation: new_score,
    });
    let info =
        server_packets::pledge_show_info_update(world.clans.get(&clan_id).expect("checked above"));
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
        let Some(clan) = world.clans.get(&clan_id) else {
            return;
        };
        (
            clan.leader_id,
            clan.members.iter().map(|m| m.char_id).collect::<Vec<_>>(),
        )
    };
    // Java broadcasts the dispersal message before tearing the roster down.
    broadcast_to_clan(
        world,
        clan_id,
        &crate::network::enter_world::system_message(sm_ids::CLAN_HAS_DISPERSED),
    );

    // Java stamps the recreate cooldown on the (online) leader in removeClanMember.
    let leader_expiry = now_millis() + CLAN_CREATE_COOLDOWN_MS;
    let delete_all = server_packets::pledge_show_member_list_delete_all();
    for oid in &member_ids {
        let online = {
            if let Some(p) = world.objects.get_component_mut::<Player>(oid) {
                p.clan_id = 0;
                p.clan_privs = 0;
                p.clan_leader = false;
                p.pledge_class = 0;
                p.ally_id = 0;
                if *oid == leader_id {
                    p.clan_create_expiry_time = leader_expiry;
                }
                // Java `Clan.removeClanMember`: `if (!player.isNoble())
                // player.setTitle("")`. A noble's title is their own standing,
                // not the clan's, so it survives the clan dissolving.
                if !p.is_noble {
                    p.title.clear();
                }
                true
            } else {
                false
            }
        };
        if online {
            // Java `removeClanMember` stops Clan Advent + all clan skills on each
            // member as the clan disperses; the ex-members stay online, so both
            // the aura and the pledge skills must drop.
            remove_clan_advent(world, *oid);
            remove_clan_skills_from_member(world, *oid);
            if let Some(cid) = client_for_player(world, *oid)
                && let Some(cs) = world.clients.get(&cid)
            {
                cs.send(delete_all.clone());
            }
        }
    }
    world.clans.remove(&clan_id);
    let _ = world.db.send(DbCommand::DestroyClan {
        clan_id,
        leader_id,
        leader_expiry,
    });
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
    if !matches!(
        world.clients.get(&client_id),
        Some(crate::session::ClientSession::InGame(_))
    ) {
        return;
    }
    let Some(clan_id) = PacketReader::new(body).read_i32() else {
        return;
    };
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::pledge_info(clan));
    }
}

/// `RequestPledgeRecruitInfo` (ex 0xD3): a clan's recruitment summary,
/// answered with `ExPledgeRecruitInfo`. Java resolves the clan through
/// `ClanTable` and stays silent for an unknown id.
pub(crate) fn handle_request_pledge_recruit_info(world: &World, client_id: u32, ex_body: &[u8]) {
    let Some(clan_id) = PacketReader::new(ex_body).read_i32() else {
        return;
    };
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::ex_pledge_recruit_info(clan));
    }
}

/// `RequestPledgeRecruitApplyInfo` (ex 0xDE): the clan window polls the
/// player's clan-entry status on open. Java's `ClanEntryStatus`: DEFAULT=0,
/// ORDERED=1 (the leader of a clan registered on the board),
/// CLAN_REGISTRATION=2, UNKNOWN=3, WAITING=4 (a clanless player with a
/// pending application to any clan).
pub(crate) fn handle_request_pledge_recruit_apply_info(world: &World, client_id: u32) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let status = if p.clan_id != 0 && p.clan_leader && world.recruit_clans.contains_key(&p.clan_id)
    {
        1 // ORDERED
    } else if p.clan_id == 0 && world.recruit_waiting.contains_key(&player) {
        4 // WAITING
    } else {
        0 // DEFAULT
    };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::ex_pledge_recruit_apply_info(status));
    }
}

/// `EnterWorld.runImpl`'s clan section (narrowed): fix the leader flag from
/// the live table, refresh the member's level in the roster, send the
/// pledge window to the enterer and the online-status update to the rest.
pub(crate) fn on_enter_world(world: &mut World, client_id: u32, object_id: i32) {
    let Some(p) = world.objects.get_component::<Player>(&object_id) else {
        return;
    };
    let clan_id = p.clan_id;
    if clan_id == 0 {
        return;
    }
    let level = p.level;
    let (is_leader, pledge_class) = world
        .clans
        .get(&clan_id)
        .map(|c| (c.leader_id == object_id, c.pledge_class_of(object_id)))
        .unwrap_or((false, 0));
    // Java `Player.restore`: the leader gets all privileges + grade 1; anyone
    // else gets their rank's mask (grade defaulting to 5) — the stored
    // `clan_privs` column never wins over the live rank table.
    let rank_privs = {
        let grade = p.power_grade;
        world.clans.get(&clan_id).map(|c| {
            let grade = if grade == 0 { 5 } else { grade };
            (grade, c.rank_privs_of(grade))
        })
    };
    let (ally_id, ally_crest_id, clan_crest_id, clan_crest_large_id) = world
        .clans
        .get(&clan_id)
        .map(|c| (c.ally_id, c.ally_crest_id, c.crest_id, c.crest_large_id))
        .unwrap_or((0, 0, 0, 0));
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.clan_leader = is_leader;
        p.pledge_class = pledge_class;
        p.ally_id = ally_id;
        p.ally_crest_id = ally_crest_id;
        p.clan_crest_id = clan_crest_id;
        p.clan_crest_large_id = clan_crest_large_id;
        if is_leader {
            p.clan_privs = ALL_CLAN_PRIVILEGES;
            p.power_grade = 1;
        } else if let Some((grade, privs)) = rank_privs {
            p.power_grade = grade;
            p.clan_privs = privs;
        }
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
        // Java `sendAllTo`: one tab per sub-unit, main pledge last.
        for pkt in server_packets::pledge_show_member_list_all_tabs(clan, &world.objects) {
            cs.send(pkt);
        }
    }
    notify_members(world, clan_id, object_id, true);
    // Clan Advent (skill 19009) — Java `ClanMaster.onPlayerLogin`.
    apply_clan_advent_on_login(world, clan_id, object_id);
    // Clan skills — Java `EnterWorld` → `clan.addSkillEffects(player)`.
    apply_clan_skills_to_member(world, clan_id, object_id);
    // Siege/leader skills — Java `EnterWorld`: `if (clan.getLevel() >=
    // siegeClanMinLevel && isClanLeader()) addSiegeSkills(player)`.
    apply_siege_skills_to_leader(world, clan_id, object_id);
}

/// `Player.deleteMe`'s clan half: the offline ping to online members, plus the
/// Clan Advent teardown (Java `ClanMaster.onPlayerLogout`). When the leader logs
/// out the aura drops from every *other* online member; the leaver themselves is
/// despawned right after this returns, so stripping it from self would be moot.
pub(crate) fn on_leave_world(world: &mut World, object_id: i32, clan_id: i32) {
    if clan_id == 0 {
        return;
    }
    notify_members(world, clan_id, object_id, false);
    let is_leader = world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.leader_id == object_id);
    if is_leader {
        for oid in online_members(world, clan_id) {
            if oid != object_id {
                remove_clan_advent(world, oid);
            }
        }
    }
}

/// `PledgeShowMemberListUpdate` about `subject` to every *other* online
/// clan member.
fn notify_members(world: &World, clan_id: i32, subject: i32, online: bool) {
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    let Some(subject_member) = clan.member(subject) else {
        return;
    };
    let pkt = server_packets::pledge_show_member_list_update(subject_member, online);
    for m in &clan.members {
        if m.char_id == subject {
            continue;
        }
        if let Some(cid) = client_for_player(world, m.char_id)
            && let Some(cs) = world.clients.get(&cid)
        {
            cs.send(pkt.clone());
        }
    }
}

/// Clan chat (`ChatType::Clan` in `Say2`): `CreatureSay` to every online
/// member including the speaker (Java `Clan.broadcastToOnlineMembers`).
pub(crate) fn broadcast_to_clan(world: &World, clan_id: i32, pkt: &[u8]) {
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    for m in &clan.members {
        if let Some(cid) = client_for_player(world, m.char_id)
            && let Some(cs) = world.clients.get(&cid)
        {
            cs.send(pkt.to_vec());
        }
    }
}

// --- G18 slice 1: membership lifecycle -------------------------------------

/// `DaysBeforeJoinAClan = 1` on this dist → the rejoin penalty in millis
/// (stamped on a leaver/oustee and on the ousting clan).
pub(crate) const CLAN_JOIN_PENALTY_MS: i64 = 86_400_000;

/// `DaysToPassToDissolveAClan = 7` on this dist → the dissolution delay.
pub(crate) const CLAN_DISSOLVE_DELAY_MS: i64 = 7 * 86_400_000;

/// The game loop runs at 10 ticks/s — wall-clock millis to scheduler ticks.
pub(crate) const MS_PER_TICK: i64 = 100;

pub(crate) fn send_to_member(world: &World, oid: i32, pkt: Vec<u8>) {
    if let Some(cs) = client_for_player(world, oid).and_then(|cid| world.clients.get(&cid)) {
        cs.send(pkt);
    }
}

/// `oid`'s standing in their clan: the clan id, their privilege mask, and
/// whether they lead it. `None` covers both "no `Player` there" and "in no
/// clan" — the distinction every clan handler opens by erasing, since a
/// clanless player and a vanished one get the same answer to "may I?".
pub(crate) fn clan_membership(world: &World, player_oid: i32) -> Option<(i32, i32, bool)> {
    world
        .objects
        .get_component::<Player>(&player_oid)
        .filter(|p| p.clan_id != 0)
        .map(|p| (p.clan_id, p.clan_privs, p.clan_leader))
}

/// The clan `player_oid` *leads*, or `None` — which the clanless and the
/// rank-and-file member share, because that is the only distinction the
/// leader-only village-master bypasses draw.
pub(crate) fn clan_leader_of(world: &World, player_oid: i32) -> Option<i32> {
    clan_membership(world, player_oid)
        .and_then(|(clan_id, _, is_leader)| is_leader.then_some(clan_id))
}

/// Whether `oid` holds `privilege` in their clan (leader always does — the
/// clan's `has_privilege` folds that in).
pub(crate) fn has_clan_privilege(world: &World, oid: i32, privilege: i32) -> bool {
    let Some(p) = world.objects.get_component::<Player>(&oid) else {
        return false;
    };
    world
        .clans
        .get(&p.clan_id)
        .is_some_and(|c| c.has_privilege(oid, p.clan_privs, privilege))
}

/// `Clan.removeClanMember(objectId, 0)` for a graduating academy member: the
/// **zero** rejoin expiry is the point — a graduate may join a new clan at
/// once, which is half of what the academy is worth to them.
pub(crate) fn remove_clan_member_for_academy(world: &mut World, clan_id: i32, member_oid: i32) {
    remove_clan_member(world, clan_id, member_oid, 0);
}

/// A clan's name, or `None` when no clan carries that id — a disbanded clan,
/// or the sentinel `0` a clanless player reports.
pub(crate) fn clan_name(world: &World, clan_id: i32) -> Option<String> {
    world.clans.get(&clan_id).map(|c| c.name.clone())
}

/// A clan's name, empty when no clan carries that id.
///
/// The shape the message formatters want, mirroring
/// [`crate::game_loop::helpers::player_name_or_empty`].
pub(crate) fn clan_name_or_empty(world: &World, clan_id: i32) -> String {
    clan_name(world, clan_id).unwrap_or_default()
}
