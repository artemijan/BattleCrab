use super::academy;
use super::add_clan_reputation;
use super::add_clan_skill;
use super::broadcast_to_clan;
use super::clan_leader_of;
use super::clan_membership;
use super::clan_skill_pairs;
use super::has_clan_privilege;
use super::online_members;
use super::send_to_member;
use super::set_clan_level;
use crate::db::DbCommand;
use crate::game_loop::character::inventory;
use crate::game_loop::helpers::send_to_client;
use crate::game_loop::net::broadcast;
use crate::game_loop::space::position::maybe_position;
use crate::game_loop::{clans, helpers};
use crate::model::Player;
use crate::model::clan::ClanMember;
use crate::network::server_packets;
use crate::network::server_packets::SmParam;
use crate::network::server_packets::sm_ids;
use crate::world::World;
use commons::network::PacketReader;
use commons::util::now_millis;

/// `AcquireSkillType.PLEDGE` on the wire (skill lists + acquire packets).
const ACQUIRE_TYPE_PLEDGE: i16 = 2;

/// Blood Mark — the proof item the level 3/4/5 upgrades consume.
const BLOOD_MARK: i32 = 1419;
const ADENA: i32 = 57;

/// Java `Clan.levelUpClan`'s cost ladder (Classic values, `_level` → next):
/// `(sp, item_id, item_count)` — levels 0/1 charge adena, 2..4 Blood Marks.
const LEVEL_UP_COSTS: [(i64, i32, i64); 5] = [
    (1_000, ADENA, 150_000),
    (15_000, ADENA, 300_000),
    (100_000, BLOOD_MARK, 100),
    (1_000_000, BLOOD_MARK, 5_000),
    (5_000_000, BLOOD_MARK, 10_000),
];

/// `VillageMaster.onBypassFeedback`'s `increase_clan_level` branch →
/// `Clan.levelUpClan`: leader + not-dissolving gates, the SP/adena/proof-item
/// price for the current level, then `changeLevel(level + 1)` and the level-up
/// FX (`MagicSkillUse` 5103) broadcast from the leader.
pub(crate) fn handle_increase_clan_level(world: &mut World, client_id: u32, player_oid: i32) {
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return;
    };
    let clan_id = p.clan_id;
    let sp = p.sp;
    if clan_id == 0 || !p.clan_leader {
        helpers::send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT,
        );
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if now_millis() < clan.dissolving_expiry_time {
        helpers::send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::AS_YOU_ARE_SCHEDULED_FOR_CLAN_DISSOLUTION_LEVEL_CANNOT_INCREASE,
        );
        return;
    }
    let level = clan.level;
    let Some(&(sp_cost, item_id, item_count)) = LEVEL_UP_COSTS.get(level as usize) else {
        // Level 5+ has no village-master upgrade on this dist (Java returns
        // false with no message past the ladder).
        return;
    };
    let has_items = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&player_oid)
        .is_some_and(|inv| inv.count_of(item_id) >= item_count);
    if sp < sp_cost || !has_items {
        helpers::send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::THE_CONDITIONS_TO_INCREASE_THE_CLAN_S_LEVEL_HAVE_NOT_BEEN_MET,
        );
        return;
    }
    if !inventory::take_items(world, client_id, player_oid, item_id, item_count) {
        helpers::send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::THE_CONDITIONS_TO_INCREASE_THE_CLAN_S_LEVEL_HAVE_NOT_BEEN_MET,
        );
        return;
    }
    // The consumption messages: adena sends its own line (Java `reduceAdena
    // (sendMessage=true)`), proof items send the destroy line + `levelUpClan`'s
    // explicit `S1_DISAPPEARED` (Java double-messages here — kept faithful).
    if item_id == ADENA {
        helpers::send_sm_to_player(
            world,
            player_oid,
            sm_ids::S1_ADENA_DISAPPEARED,
            &[SmParam::Long(item_count)],
        );
    } else {
        helpers::send_sm_to_player(
            world,
            player_oid,
            sm_ids::S2_S1_S_DISAPPEARED,
            &[SmParam::ItemName(item_id), SmParam::Long(item_count)],
        );
        helpers::send_sm_to_player(
            world,
            player_oid,
            sm_ids::S1_DISAPPEARED,
            &[SmParam::ItemName(item_id)],
        );
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.sp -= sp_cost;
    }
    helpers::send_sm_to_player(
        world,
        player_oid,
        sm_ids::YOUR_SP_HAS_DECREASED_BY_S1,
        &[SmParam::Int(sp_cost as i32)],
    );

    // Java refreshes the leader's SP (UserInfo CURRENT_HPMPCP_EXP_SP) + item
    // list; the full re-send stands in (the port's usual substitution).
    crate::game_loop::character::player_info::broadcast_user_info(world, player_oid);

    set_clan_level(world, clan_id, level + 1);

    // The level-up flourish: `MagicSkillUse(player, 5103, 1, 0, 0)` +
    // `MagicSkillLaunched`, broadcast from the leader.
    if let Some(pos) = maybe_position(world, player_oid) {
        let use_pkt = server_packets::magic_skill_use_raw(
            (player_oid, pos.x, pos.y, pos.z),
            (player_oid, pos.x, pos.y, pos.z),
            5103,
            1,
            0,
        );
        broadcast::broadcast_including_self(world, player_oid, &use_pkt);
        let launched = server_packets::magic_skill_launched(player_oid, 5103, 1, &[player_oid]);
        broadcast::broadcast_including_self(world, player_oid, &launched);
    }
}

/// `VillageMaster.showPledgeSkillList`: the leader-only learnable pledge-skill
/// window. Non-leaders get `NotClanLeader.htm`; an empty list explains when to
/// come back (SM 607 below clan level 8, `NoMoreSkills.htm` at 8+); otherwise
/// `ExAcquirableSkillListByClass(PLEDGE)`.
pub(crate) fn show_pledge_skill_list(world: &World, client_id: u32, player_oid: i32) {
    let Some(clan_id) = clan_leader_of(world, player_oid) else {
        send_villagemaster_html(world, client_id, "NotClanLeader.htm");
        return;
    };
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    let available = world
        .data
        .pledge_skill_trees
        .available_pledge_skills(clan.level, &clan.skills);
    if available.is_empty() {
        if clan.level < 8 {
            let next = if clan.level < 5 { 5 } else { clan.level + 1 };
            helpers::send_sm_to_player(
                world,
                player_oid,
                sm_ids::YOU_DO_NOT_HAVE_ANY_FURTHER_SKILLS_TO_LEARN_COME_BACK_AT_LEVEL_S1,
                &[SmParam::Int(next)],
            );
        } else {
            send_villagemaster_html(world, client_id, "NoMoreSkills.htm");
        }
        return;
    }
    let rows: Vec<(i32, i32, i32, i64)> = available
        .iter()
        .map(|l| (l.skill_id, l.skill_level, l.get_level, l.level_up_sp))
        .collect();
    send_to_client(
        world,
        client_id,
        server_packets::ex_acquirable_skill_list_by_class(ACQUIRE_TYPE_PLEDGE, &rows),
    );
}

/// Serve a `data/html/villagemaster/<file>` window (Java `NpcHtmlMessage.
/// setFile` with no NPC binding — object id 0).
fn send_villagemaster_html(world: &World, client_id: u32, file: &str) {
    let html = crate::data::htm_cache::read_htm_for_client(
        world,
        client_id,
        format!("{}data/html/villagemaster/{file}", world.data.root),
    )
    .unwrap_or_else(|| "<html><body>My Text is missing:<br></body></html>".to_string());
    send_to_client(world, client_id, server_packets::npc_html_message(0, &html));
}

/// `RequestAcquireSkillInfo`'s PLEDGE branch: the leader clicked a skill in the
/// pledge list — answer with the reputation cost (`AcquireSkillInfo`).
pub(crate) fn handle_request_pledge_skill_info(
    world: &World,
    client_id: u32,
    skill_id: i32,
    skill_level: i32,
) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    if clan_leader_of(world, player).is_none() {
        return;
    }
    let Some(learn) = world
        .data
        .pledge_skill_trees
        .pledge_skill(skill_id, skill_level)
    else {
        return;
    };
    send_to_client(
        world,
        client_id,
        server_packets::acquire_skill_info(
            learn.skill_id,
            learn.skill_level,
            learn.level_up_sp,
            ACQUIRE_TYPE_PLEDGE as i32,
        ),
    );
}

/// `RequestAcquireSkill`'s PLEDGE case: the leader confirms a pledge-skill
/// learn — validate it is the clan's next level of a tree entry the clan
/// level qualifies for, spend clan reputation, and grant through
/// `add_clan_skill` (which broadcasts `PledgeSkillListAdd` + applies the
/// passive to qualifying members). No required items on this dist's pledge
/// tree, so `LifeCrystalNeeded`'s item loop has nothing to consume.
pub(crate) fn handle_learn_pledge_skill(
    world: &mut World,
    client_id: u32,
    skill_id: i32,
    skill_level: i32,
) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(clan_id) = clan_leader_of(world, player) else {
        return;
    };
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    let Some(learn) = world
        .data
        .pledge_skill_trees
        .pledge_skill(skill_id, skill_level)
        .cloned()
    else {
        return;
    };
    // Java's hack checks: the previous level must be the clan's current one
    // (`prevSkillLevel != _level - 1` reject) and the clan level must qualify
    // (the client list only ever offers qualifying entries).
    if clan.skills.get(&skill_id).copied().unwrap_or(0) != skill_level - 1
        || clan.level < learn.get_level
    {
        return;
    }
    let rep_cost = learn.level_up_sp as i32;
    if clan.reputation_score < rep_cost {
        helpers::send_sm_bare_to_player(
            world,
            player,
            sm_ids::SKILL_ACQUIRE_FAILED_INSUFFICIENT_CLAN_REPUTATION,
        );
        show_pledge_skill_list(world, client_id, player);
        return;
    }
    // `takeReputationScore` (negative add: clamp + persist + pledge-window
    // refresh to every online member).
    add_clan_reputation(world, clan_id, -rep_cost);
    helpers::send_sm_to_player(
        world,
        player,
        sm_ids::S1_POINTS_HAVE_BEEN_DEDUCTED_FROM_THE_CLAN_S_REPUTATION,
        &[SmParam::Int(rep_cost)],
    );
    add_clan_skill(world, clan_id, skill_id, skill_level);
    // Java broadcasts the full `PledgeSkillList` to online members, acks the
    // dialog, and re-opens the (now shorter) learnable list.
    let pkt = server_packets::pledge_skill_list(&clan_skill_pairs(world, clan_id));
    for oid in online_members(world, clan_id) {
        send_to_member(world, oid, pkt.clone());
    }
    send_to_client(world, client_id, server_packets::acquire_skill_done());
    show_pledge_skill_list(world, client_id, player);
}

// --- G18 slice 3: ranks & power grades + delegated leader transfer ---------

use crate::model::clan::{CL_MANAGE_RANKS, RANK9_PRIVS_MASK};

/// Java `Clan.broadcastClanStatus` — reset every online member's clan window
/// (DeleteAll + a fresh MemberListAll).
pub(crate) fn broadcast_clan_status(world: &World, clan_id: i32) {
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    let delete_all = server_packets::pledge_show_member_list_delete_all();
    let tabs = server_packets::pledge_show_member_list_all_tabs(clan, &world.objects);
    for oid in online_members(world, clan_id) {
        send_to_member(world, oid, delete_all.clone());
        for pkt in &tabs {
            send_to_member(world, oid, pkt.clone());
        }
    }
}

/// `RequestPledgePower` (0xCC): the rank-privilege editor. Every request is
/// answered with `ManagePledgePower`; `action == 2` from the leader stores the
/// edited mask (`Clan.setRankPrivs`) — rank 9 (academy) clamped to the
/// bestowable subset — and refreshes online members holding that rank.
pub(crate) fn handle_request_pledge_power(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = PacketReader::new(body);
    let Some(rank) = r.read_i32() else { return };
    let Some(action) = r.read_i32() else { return };
    let privs = if action == 2 {
        r.read_i32().unwrap_or(0)
    } else {
        0
    };

    let Some((clan_id, _, is_leader)) = clan_membership(world, player) else {
        return;
    };
    if action == 2 && is_leader {
        let privs = if rank == 9 {
            privs & RANK9_PRIVS_MASK
        } else {
            privs
        };
        set_rank_privs(world, clan_id, rank, privs);
    }
    let current = world
        .clans
        .get(&clan_id)
        .map(|c| c.rank_privs_of(rank))
        .unwrap_or(0);
    send_to_client(
        world,
        client_id,
        server_packets::manage_pledge_power(rank, action, current),
    );
}

/// Java `Clan.setRankPrivs`: store + persist the rank's mask, push it onto
/// every online member holding that grade (bitmask + UserInfo), then reset the
/// clan windows.
fn set_rank_privs(world: &mut World, clan_id: i32, rank: i32, privs: i32) {
    let Some(clan) = world.clans.get_mut(&clan_id) else {
        return;
    };
    clan.rank_privs.insert(rank, privs);
    let leader_id = clan.leader_id;
    let member_ids: Vec<i32> = clan.members.iter().map(|m| m.char_id).collect();
    let _ = world.db.send(DbCommand::SaveClanRankPrivs {
        clan_id,
        rank,
        privs,
    });
    for oid in member_ids {
        if oid == leader_id {
            continue;
        }
        let holds_rank = world
            .objects
            .get_component::<Player>(&oid)
            .is_some_and(|p| p.power_grade == rank);
        if holds_rank {
            if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
                p.clan_privs = privs;
            }
            crate::game_loop::character::player_info::broadcast_user_info(world, oid);
        }
    }
    broadcast_clan_status(world, clan_id);
}

/// `RequestPledgePowerGradeList` (ex 0x13): the rank list — Java sends all 9
/// initialized ranks regardless of stored rows.
pub(crate) fn handle_request_pledge_power_grade_list(world: &World, client_id: u32) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    if clan_membership(world, player).is_none() {
        return;
    }
    let ranks: Vec<i32> = (1..=9).collect();
    send_to_client(
        world,
        client_id,
        server_packets::pledge_power_grade_list(&ranks),
    );
}

/// Resolve a named member of the acting player's clan; `None` when the player
/// is clanless or the name is not in the roster.
fn clan_member_by_name(world: &World, player: i32, name: &str) -> Option<(i32, ClanMember)> {
    let clan_id = clans::clan_of(world, player)?;
    let clan = world.clans.get(&clan_id)?;
    clan.member_by_name(name).map(|m| (clan_id, m.clone()))
}

/// `RequestPledgeMemberPowerInfo` (ex 0x14): one member's rank + that rank's
/// current privilege mask.
pub(crate) fn handle_request_pledge_member_power_info(
    world: &World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = PacketReader::new(ex_body);
    let Some(_unk) = r.read_i32() else { return };
    let Some(name) = r.read_string() else { return };
    let Some((clan_id, member)) = clan_member_by_name(world, player, &name) else {
        return;
    };
    // The live grade for an online member (roster snapshots refresh lazily).
    let grade = world
        .objects
        .get_component::<Player>(&member.char_id)
        .map(|p| p.power_grade)
        .unwrap_or(member.power_grade);
    let privs = world
        .clans
        .get(&clan_id)
        .map(|c| c.rank_privs_of(grade))
        .unwrap_or(0);
    send_to_client(
        world,
        client_id,
        server_packets::pledge_receive_power_info(grade, &member.name, privs),
    );
}

/// `RequestPledgeMemberList` (0x4D): the clan window asking for the roster
/// again — Java `PledgeShowMemberListAll.sendAllTo(player)`, the same
/// one-packet-per-sub-unit fan-out login uses. A clanless player is answered
/// with nothing at all (Java's `if (clan != null)` has no else).
pub(crate) fn handle_request_pledge_member_list(world: &mut World, client_id: u32) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(clan_id) = clans::clan_of(world, player) else {
        return;
    };
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    for pkt in server_packets::pledge_show_member_list_all_tabs(clan, &world.objects) {
        send_to_client(world, client_id, pkt);
    }
}

/// `RequestPledgeMemberInfo` (ex 0x16): the member-detail pane.
pub(crate) fn handle_request_pledge_member_info(world: &World, client_id: u32, ex_body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = PacketReader::new(ex_body);
    let Some(_unk) = r.read_i32() else { return };
    let Some(name) = r.read_string() else { return };
    let Some((clan_id, mut member)) = clan_member_by_name(world, player, &name) else {
        return;
    };
    // Live title/grade for online members.
    if let Some(p) = world.objects.get_component::<Player>(&member.char_id) {
        member.title = p.title.clone();
        member.power_grade = p.power_grade;
    }
    // Java: a sub-unit member's pane names the *unit*, not the clan.
    let unit_name = world
        .clans
        .get(&clan_id)
        .map(|c| {
            if member.pledge_type == 0 {
                c.name.clone()
            } else {
                c.sub_pledges
                    .get(&member.pledge_type)
                    .map(|sp| sp.name.clone())
                    .unwrap_or_else(|| c.name.clone())
            }
        })
        .unwrap_or_default();
    let partner_name = academy::partner_name(world, clan_id, member.char_id);
    send_to_client(
        world,
        client_id,
        server_packets::pledge_receive_member_info(&member, &unit_name, &partner_name),
    );
}

/// `RequestPledgeSetMemberPowerGrade` (ex 0x15): a CL_MANAGE_RANKS holder
/// re-ranks a member (never the leader). The new rank's privileges apply to
/// the online member immediately through the rank table refresh at
/// `broadcastClanStatus`-time in Java only on relog — we mirror Java: the
/// grade changes now, the mask follows at login/rank-edit.
pub(crate) fn handle_request_pledge_set_member_power_grade(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = PacketReader::new(ex_body);
    let Some(name) = r.read_string() else { return };
    let Some(grade) = r.read_i32() else { return };

    let Some((clan_id, privs, _)) = clan_membership(world, player) else {
        return;
    };
    let has_priv = world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.has_privilege(player, privs, CL_MANAGE_RANKS));
    if !has_priv {
        return;
    }
    let Some((_, member)) = clan_member_by_name(world, player, &name) else {
        return;
    };
    let leader_id = world.clans.get(&clan_id).map(|c| c.leader_id).unwrap_or(0);
    if member.char_id == leader_id {
        return;
    }
    // Java: an academy member cannot be re-ranked out of rank 9.
    if academy::member_is_academy(world, clan_id, member.char_id) {
        helpers::send_sm_bare_to_player(
            world,
            player,
            sm_ids::THAT_PRIVILEGE_CANNOT_BE_GRANTED_TO_A_CLAN_ACADEMY_MEMBER,
        );
        return;
    }

    if let Some(c) = world.clans.get_mut(&clan_id)
        && let Some(m) = c.members.iter_mut().find(|m| m.char_id == member.char_id)
    {
        m.power_grade = grade;
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&member.char_id) {
        p.power_grade = grade;
    }
    let _ = world.db.send(DbCommand::UpdateCharPowerGrade {
        char_id: member.char_id,
        power_grade: grade,
    });

    let online = helpers::client_for_player(world, member.char_id).is_some();
    let update = {
        let c = world.clans.get(&clan_id).expect("checked above");
        c.member(member.char_id)
            .map(|m| server_packets::pledge_show_member_list_update(m, online))
    };
    if let Some(pkt) = update {
        broadcast_to_clan(world, clan_id, &pkt);
    }
    let sm = server_packets::system_message_with(
        sm_ids::CLAN_MEMBER_C1_S_PRIVILEGE_LEVEL_HAS_BEEN_CHANGED_TO_S2,
        &[SmParam::Text(member.name.clone()), SmParam::Int(grade)],
    );
    broadcast_to_clan(world, clan_id, &sm);
    broadcast_clan_status(world, clan_id);
}

/// `RequestPledgeReorganizeMember` (ex 0x2C): the leader (or a
/// CL_MANAGE_RANKS holder) swaps two main-pledge-or-below members' sub-unit
/// membership — `member_name` takes `new_pledge_type`, `selected_member`
/// takes whatever `member_name` had.
pub(crate) fn handle_request_pledge_reorganize_member(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = PacketReader::new(ex_body);
    let Some(is_selected) = r.read_i32() else {
        return;
    };
    let Some(member_name) = r.read_string() else {
        return;
    };
    let Some(new_pledge_type) = r.read_i32() else {
        return;
    };
    let Some(selected_member) = r.read_string() else {
        return;
    };
    if is_selected == 0 {
        return;
    }
    let Some((clan_id, privs)) = clans::clan_and_privs(world, player) else {
        return;
    };
    if clan_id == 0 {
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if !clan.has_privilege(player, privs, CL_MANAGE_RANKS) {
        return;
    }
    // A malformed/hacked target type: the client only ever offers 0 or a real
    // sub-unit id, so anything else is dropped defensively.
    if new_pledge_type != 0 && !clan.sub_pledges.contains_key(&new_pledge_type) {
        return;
    }
    let leader_id = clan.leader_id;
    let Some(m1) = clan.member_by_name(&member_name).cloned() else {
        return;
    };
    let Some(m2) = clan.member_by_name(&selected_member).cloned() else {
        return;
    };
    if m1.char_id == leader_id || m2.char_id == leader_id {
        return;
    }
    let old_pledge_type = m1.pledge_type;
    if old_pledge_type == new_pledge_type {
        return;
    }

    if let Some(c) = world.clans.get_mut(&clan_id) {
        if let Some(m) = c.members.iter_mut().find(|m| m.char_id == m1.char_id) {
            m.pledge_type = new_pledge_type;
        }
        if let Some(m) = c.members.iter_mut().find(|m| m.char_id == m2.char_id) {
            m.pledge_type = old_pledge_type;
        }
    }
    for (oid, pledge_type) in [(m1.char_id, new_pledge_type), (m2.char_id, old_pledge_type)] {
        let _ = world.db.send(DbCommand::UpdateCharPledgeType {
            char_id: oid,
            pledge_type,
        });
        let pledge_class = world
            .clans
            .get(&clan_id)
            .map(|c| c.pledge_class_of(oid))
            .unwrap_or(0);
        if let Some(mp) = world.objects.get_component_mut::<Player>(&oid) {
            mp.pledge_type = pledge_type;
            mp.pledge_class = pledge_class;
        }
        crate::game_loop::character::player_info::broadcast_user_info(world, oid);
    }
    broadcast_clan_status(world, clan_id);
}

/// `VillageMaster`'s `change_clan_leader <name>` bypass — the delegated
/// transfer flow (`AltClanLeaderInstantActivation = False` on this dist):
/// stamp `new_leader_id` + the confirmation html. The `setNewLeader` half runs
/// at the **Wednesday** daily reset (`daily_tasks::clan_leader_apply`, Java
/// `DailyTaskManager.clanLeaderApply`), so the stamp can wait up to a week.
pub(crate) fn handle_change_clan_leader(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    npc_oid: i32,
    args: &str,
) {
    let name = args.trim();
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return;
    };
    let clan_id = p.clan_id;
    let player_name = p.name.clone();
    if clan_id == 0 || !p.clan_leader {
        helpers::send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT,
        );
        return;
    }
    if player_name.eq_ignore_ascii_case(name) {
        return;
    }
    let Some((_, member)) = clan_member_by_name(world, player_oid, name) else {
        send_to_client(
            world,
            client_id,
            server_packets::system_message_with(
                sm_ids::S1_DOES_NOT_EXIST,
                &[SmParam::Text(name.to_string())],
            ),
        );
        return;
    };
    if helpers::client_for_player(world, member.char_id).is_none() {
        helpers::send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::THAT_PLAYER_IS_NOT_CURRENTLY_ONLINE,
        );
        return;
    }
    // Java: an academy member cannot be nominated clan leader.
    if academy::member_is_academy(world, clan_id, member.char_id) {
        helpers::send_sm_bare_to_player(
            world,
            player_oid,
            sm_ids::THAT_PRIVILEGE_CANNOT_BE_GRANTED_TO_A_CLAN_ACADEMY_MEMBER,
        );
        return;
    }
    let already_pending = world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.new_leader_id != 0);
    let file = if already_pending {
        "9000-07-in-progress.htm"
    } else {
        if let Some(c) = world.clans.get_mut(&clan_id) {
            c.new_leader_id = member.char_id;
        }
        let _ = world.db.send(DbCommand::UpdateClanNewLeader {
            clan_id,
            new_leader_id: member.char_id,
        });
        "9000-07-success.htm"
    };
    send_clan_master_html(world, client_id, npc_oid, file);
}

/// `VillageMaster`'s `cancel_clan_leader_change` bypass.
pub(crate) fn handle_cancel_clan_leader_change(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    npc_oid: i32,
) {
    let Some(clan_id) = clan_leader_of(world, player_oid) else {
        helpers::send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT,
        );
        return;
    };
    let pending = world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.new_leader_id != 0);
    if pending {
        if let Some(c) = world.clans.get_mut(&clan_id) {
            c.new_leader_id = 0;
        }
        let _ = world.db.send(DbCommand::UpdateClanNewLeader {
            clan_id,
            new_leader_id: 0,
        });
        send_clan_master_html(world, client_id, npc_oid, "9000-07-canceled.htm");
    } else {
        send_to_client(
            world,
            client_id,
            server_packets::npc_html_message(
                npc_oid,
                "<html><body>You don't have clan leader delegation applications submitted yet!</body></html>",
            ),
        );
    }
}

/// Serve a `data/scripts/village_master/ClanMaster/<file>` page through the
/// clicked NPC (the leader-transfer confirmations live with the script htmls).
fn send_clan_master_html(world: &World, client_id: u32, npc_oid: i32, file: &str) {
    let html = crate::data::htm_cache::read_htm_for_client(
        world,
        client_id,
        format!(
            "{}data/scripts/village_master/ClanMaster/{file}",
            world.data.root
        ),
    )
    .unwrap_or_else(|| "<html><body>My Text is missing:<br></body></html>".to_string())
    .replace("%objectId%", &npc_oid.to_string());
    send_to_client(
        world,
        client_id,
        server_packets::npc_html_message(npc_oid, &html),
    );
}

// --- G18 slice 4: clan wars ------------------------------------------------

/// `RequestGiveNickName` (0x0B) — grant a title, either to a clan member or
/// (for a noble) to yourself.
///
/// The self-branch is checked **first and unconditionally**: a noble typing
/// their own name gets the title with no clan, privilege or level test at all,
/// which is what makes nobless a personal cosmetic rather than a clan one.
/// Everyone else goes through `CL_GIVE_TITLE` + clan level 3.
///
/// Both success paths message the **recipient**, not the granter — a leader
/// retitling a member sees nothing.
pub(crate) fn handle_request_give_nick_name(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = PacketReader::new(body);
    let Some(target) = r.read_string() else {
        return;
    };
    let Some(title) = r.read_string() else { return };

    let (is_noble, own_name, clan_id) = match world.objects.get_component::<Player>(&player) {
        Some(p) => (p.is_noble, p.name.clone(), p.clan_id),
        None => return,
    };

    // "Noblesse can bestow a title to themselves".
    if is_noble && target.eq_ignore_ascii_case(&own_name) {
        set_title_and_broadcast(world, player, title);
        return;
    }

    if !has_clan_privilege(world, player, crate::model::clan::CL_GIVE_TITLE) {
        helpers::send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT,
        );
        return;
    }
    // Java reads `getClan().getLevel()` unguarded here; the privilege check
    // above already returned for a clanless player, so it cannot be null.
    let level = world.clans.get(&clan_id).map_or(0, |c| c.level);
    if level < 3 {
        helpers::send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::A_PLAYER_CAN_ONLY_BE_GRANTED_A_TITLE_IF_CLAN_LEVEL_3,
        );
        return;
    }

    // `getClan().getClanMember(_target)` — membership is decided by the
    // roster, so an offline member is "in the clan but not online" and a
    // non-member is a different message entirely.
    let member_id = world
        .clans
        .get(&clan_id)
        .and_then(|c| {
            c.members
                .iter()
                .find(|m| m.name.eq_ignore_ascii_case(&target))
        })
        .map(|m| m.char_id);
    let Some(member_id) = member_id else {
        helpers::send_sm_bare_to_client(world, client_id, sm_ids::THE_TARGET_MUST_BE_A_CLAN_MEMBER);
        return;
    };
    if !world.objects.has_component::<Player>(&member_id) {
        helpers::send_sm_bare_to_client(world, client_id, sm_ids::THAT_PLAYER_IS_NOT_ONLINE);
        return;
    }
    set_title_and_broadcast(world, member_id, title);
}

/// `member.setTitle(t)` + `broadcastTitleInfo()` — a `UserInfo` to the wearer
/// and a `NicknameChanged` to everyone who can see them.
fn set_title_and_broadcast(world: &mut World, oid: i32, title: String) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
        p.title = title.clone();
    }
    helpers::send_sm_bare_to_player(world, oid, sm_ids::YOUR_TITLE_HAS_BEEN_CHANGED);
    crate::game_loop::character::player_info::broadcast_user_info(world, oid);
    broadcast::broadcast_including_self(world, oid, &server_packets::nickname_changed(oid, &title));
}
