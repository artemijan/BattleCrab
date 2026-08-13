use super::*;
use crate::game_loop::guard::clan_of_or_zero;
use crate::game_loop::helpers::send_to_client;

use crate::model::clan::{CL_PLEDGE_WAR, ClanWar, ClanWarState, WAR_TIMEOUT_MS};

/// `AltClanMembersForWar = 15` on this dist.
const CLAN_MEMBERS_FOR_WAR: usize = 15;

/// `ReputationScorePerKill = 1` (Feature.ini) — the mutual-war kill transfer.
const REPUTATION_SCORE_PER_KILL: i32 = 1;

/// The war between two clans, either direction (Java `Clan.getWarWith`).
pub(crate) fn war_between(world: &World, a: i32, b: i32) -> Option<&ClanWar> {
    world.clan_wars.iter().find(|w| {
        (w.attacker_id == a && w.attacked_id == b) || (w.attacker_id == b && w.attacked_id == a)
    })
}

fn war_between_mut(world: &mut World, a: i32, b: i32) -> Option<&mut ClanWar> {
    world.clan_wars.iter_mut().find(|w| {
        (w.attacker_id == a && w.attacked_id == b) || (w.attacker_id == b && w.attacked_id == a)
    })
}

/// Whether a **mutual** war runs between the two clans — the state that makes
/// kills lawful (`checkIfPvP`) and both sides freely attackable.
pub(crate) fn mutual_war_between(world: &World, a: i32, b: i32) -> bool {
    a != 0 && b != 0 && war_between(world, a, b).is_some_and(|w| w.state == ClanWarState::Mutual)
}

/// Java `Player.atWarWith` (any war, whatever state) — quarters the death-XP
/// penalty when the killer is a war enemy.
pub(crate) fn at_war_between(world: &World, a: i32, b: i32) -> bool {
    a != 0 && b != 0 && war_between(world, a, b).is_some()
}

/// Java `Clan.isAtWar` — the dissolve gate.
pub(crate) fn clan_is_at_war(world: &World, clan_id: i32) -> bool {
    world.clan_wars.iter().any(|w| w.involves(clan_id))
}

fn store_war(world: &World, war: &ClanWar) {
    let _ = world.db.send(DbCommand::SaveClanWar {
        attacker: war.attacker_id,
        attacked: war.attacked_id,
        attacker_kills: war.attacker_kills,
        attacked_kills: war.attacked_kills,
        winner: war.winner_id,
        start_time: war.start_time,
        end_time: war.end_time,
        state: war.state as i32,
    });
}

/// Java `RelationChanged`'s war bits, from `Player.getRelation(target)`:
/// the subject's clan at war with the viewer's — one sword for a pending
/// declaration *by the subject's side*, both swords for MUTUAL.
pub(crate) fn war_relation_bits(world: &World, subject_oid: i32, viewer_oid: i32) -> i32 {
    const RELATION_DECLARED_WAR: i32 = 0x4000; // single sword
    const RELATION_MUTUAL_WAR: i32 = 0x8000; // double swords
    let subject_clan = clan_of_or_zero(world, subject_oid);
    let viewer_clan = clan_of_or_zero(world, viewer_oid);
    if subject_clan == 0 || viewer_clan == 0 || subject_clan == viewer_clan {
        return 0;
    }
    let Some(war) = war_between(world, subject_clan, viewer_clan) else {
        return 0;
    };
    match war.state {
        ClanWarState::Declaration | ClanWarState::BloodDeclaration => {
            if war.attacker_id != viewer_clan {
                RELATION_DECLARED_WAR
            } else {
                0
            }
        }
        ClanWarState::Mutual => RELATION_DECLARED_WAR | RELATION_MUTUAL_WAR,
        _ => 0,
    }
}

/// `broadcastUserInfo(UserInfoType.CLAN)` to every online member of both war
/// sides + the per-viewer relation refresh (the war swords ride
/// `RelationChanged`).
fn broadcast_war_status(world: &mut World, clan_a: i32, clan_b: i32) {
    for clan_id in [clan_a, clan_b] {
        for oid in online_members(world, clan_id) {
            crate::game_loop::party::broadcast_user_info(world, oid);
            crate::game_loop::pvp::broadcast_siege_relation(world, oid);
        }
    }
}

/// The war tab rows for one clan's `PledgeReceiveWarList`.
fn war_list_rows(world: &World, clan_id: i32) -> Vec<(String, i32, i32, i32, i32)> {
    world
        .clan_wars
        .iter()
        .filter(|w| w.involves(clan_id))
        .filter_map(|w| {
            let other = world.clans.get(&w.opposing(clan_id))?;
            Some((
                other.name.clone(),
                w.state_for(clan_id) as i32,
                w.remaining_time(),
                w.kill_difference(clan_id),
                w.kill_to_start(),
            ))
        })
        .collect()
}

fn send_war_list(world: &World, client_id: u32, clan_id: i32, tab: i32) {
    send_to_client(
        world,
        client_id,
        server_packets::pledge_receive_war_list(tab, &war_list_rows(world, clan_id)),
    );
}

/// `RequestPledgeWarList` (ex 0x17).
pub(crate) fn handle_request_pledge_war_list(world: &World, client_id: u32, ex_body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = PacketReader::new(ex_body);
    let _unk = r.read_i32();
    let tab = r.read_i32().unwrap_or(0);
    let Some((clan_id, _, _)) = clan_membership(world, player) else {
        return;
    };
    send_war_list(world, client_id, clan_id, tab);
}

/// `RequestStartPledgeWar` (0x03): declare war by clan name — the full Java
/// guard chain, the redeclare-makes-mutual branch, then a fresh
/// BLOOD_DECLARATION war with the 7-day answer window.
pub(crate) fn handle_request_start_pledge_war(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(name) = PacketReader::new(body).read_string() else {
        return;
    };
    let Some((clan_id, privs, _)) = clan_membership(world, player) else {
        return;
    };
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if clan.level < 3 || clan.members.len() < CLAN_MEMBERS_FOR_WAR {
        send_sm_with(
            world,
            player,
            sm_ids::CLAN_WAR_NEEDS_LEVEL_3_AND_15_MEMBERS,
            &[],
        );
        return;
    }
    if !clan.has_privilege(player, privs, CL_PLEDGE_WAR) {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT,
            &[],
        );
        return;
    }
    if world
        .clan_wars
        .iter()
        .filter(|w| w.involves(clan_id))
        .count()
        >= 30
    {
        send_sm_with(
            world,
            player,
            sm_ids::CANNOT_DECLARE_WAR_ON_MORE_THAN_30_CLANS,
            &[],
        );
        return;
    }
    let Some(target) = by_name(world, &name) else {
        send_sm_with(world, player, sm_ids::CLAN_WAR_TARGET_DOES_NOT_EXIST, &[]);
        return;
    };
    let target_id = target.id;
    if target_id == clan_id {
        send_sm_with(
            world,
            player,
            sm_ids::FOOL_YOU_CANNOT_DECLARE_WAR_AGAINST_YOUR_OWN_CLAN,
            &[],
        );
        return;
    }
    let same_ally = {
        let a = world.clans.get(&clan_id).map(|c| c.ally_id).unwrap_or(0);
        a != 0 && a == target.ally_id
    };
    if same_ally {
        send_sm_with(
            world,
            player,
            sm_ids::CANNOT_DECLARE_WAR_ON_ALLIED_CLAN,
            &[],
        );
        return;
    }
    if target.level < 3 || target.members.len() < CLAN_MEMBERS_FOR_WAR {
        send_sm_with(
            world,
            player,
            sm_ids::CLAN_WAR_NEEDS_LEVEL_3_AND_15_MEMBERS,
            &[],
        );
        return;
    }
    if target.dissolving_expiry_time > now_millis() {
        send_sm_with(
            world,
            player,
            sm_ids::CANNOT_DECLARE_WAR_ON_DISSOLVING_CLAN,
            &[],
        );
        return;
    }
    let target_name = target.name.clone();

    if let Some(war) = war_between(world, clan_id, target_id) {
        match war.state_for(clan_id) {
            ClanWarState::Win => {
                send_sm_with(
                    world,
                    player,
                    sm_ids::CANNOT_DECLARE_WAR_21_DAYS_AFTER_DEFEAT_WITH_S1,
                    &[SmParam::Text(target_name)],
                );
                return;
            }
            ClanWarState::Mutual => {
                send_sm_with(
                    world,
                    player,
                    sm_ids::S1_TEXT,
                    &[SmParam::Text(format!(
                        "You have already been at war with {target_name}."
                    ))],
                );
                return;
            }
            ClanWarState::BloodDeclaration | ClanWarState::Declaration => {
                // Java `mutualClanWarAccepted`: the declaration answered in kind
                // goes MUTUAL (the pending timeout no-ops on the state change).
                if let Some(w) = war_between_mut(world, clan_id, target_id) {
                    w.state = ClanWarState::Mutual;
                }
                let war = war_between(world, clan_id, target_id)
                    .expect("just updated")
                    .clone();
                store_war(world, &war);
                let started_a = server_packets::system_message_with(
                    sm_ids::CLAN_WAR_STARTED_WITH_CLAN_S1,
                    &[SmParam::Text(target_name.clone())],
                );
                let clan_name = clan_name_or_empty(world, clan_id);
                let started_b = server_packets::system_message_with(
                    sm_ids::CLAN_WAR_STARTED_WITH_CLAN_S1,
                    &[SmParam::Text(clan_name)],
                );
                broadcast_to_clan(world, clan_id, &started_a);
                broadcast_to_clan(world, target_id, &started_b);
                broadcast_war_status(world, clan_id, target_id);
                send_war_list(world, client_id, clan_id, 0);
                return;
            }
            _ => {}
        }
    }

    // A fresh declaration.
    let war = ClanWar {
        attacker_id: clan_id,
        attacked_id: target_id,
        state: ClanWarState::BloodDeclaration,
        winner_id: 0,
        start_time: now_millis(),
        end_time: 0,
        attacker_kills: 0,
        attacked_kills: 0,
    };
    store_war(world, &war);
    let timeout_ticks = (WAR_TIMEOUT_MS / MS_PER_TICK) as u64;
    world.scheduler.schedule(
        world.tick + timeout_ticks,
        crate::scheduler::ScheduledTask::ClanWarTimeout {
            attacker: clan_id,
            attacked: target_id,
        },
    );
    world.clan_wars.push(war);
    let clan_name = clan_name_or_empty(world, clan_id);
    let declared = server_packets::system_message_with(
        sm_ids::YOU_HAVE_DECLARED_A_CLAN_WAR_WITH_S1,
        &[SmParam::Text(target_name.clone())],
    );
    broadcast_to_clan(world, clan_id, &declared);
    let warned = server_packets::system_message_with(
        sm_ids::S1_HAS_DECLARED_A_CLAN_WAR_KILL_5_TO_START,
        &[SmParam::Text(clan_name)],
    );
    broadcast_to_clan(world, target_id, &warned);
    broadcast_war_status(world, clan_id, target_id);
    send_war_list(world, client_id, clan_id, 0);
}

/// `RequestStopPledgeWar` (0x05): a mutual cease-fire — costs 500 reputation,
/// blocked while any clan member is in combat.
pub(crate) fn handle_request_stop_pledge_war(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(name) = PacketReader::new(body).read_string() else {
        return;
    };
    let Some((clan_id, privs, _)) = clan_membership(world, player) else {
        return;
    };
    let Some(target) = by_name(world, &name) else {
        send_sm_with(
            world,
            player,
            sm_ids::S1_TEXT,
            &[SmParam::Text("No such clan.".to_string())],
        );
        return;
    };
    let target_id = target.id;
    if war_between(world, clan_id, target_id).is_none() {
        send_sm_with(
            world,
            player,
            sm_ids::S1_TEXT,
            &[SmParam::Text(
                "You aren't at war with this clan.".to_string(),
            )],
        );
        return;
    }
    let has_priv = world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.has_privilege(player, privs, CL_PLEDGE_WAR));
    if !has_priv {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT,
            &[],
        );
        return;
    }
    if world
        .clans
        .get(&clan_id)
        .map(|c| c.reputation_score)
        .unwrap_or(0)
        <= 500
    {
        send_sm_with(world, player, sm_ids::THE_CLAN_REPUTATION_IS_TOO_LOW, &[]);
        return;
    }
    let member_in_combat = online_members(world, clan_id)
        .iter()
        .any(|&oid| crate::game_loop::combat::has_attack_stance(world, oid));
    if member_in_combat {
        send_sm_with(
            world,
            player,
            sm_ids::CEASE_FIRE_CANNOT_BE_CALLED_WHILE_MEMBERS_IN_BATTLE,
            &[],
        );
        return;
    }

    add_clan_reputation(world, clan_id, -500);
    let lost = crate::network::enter_world::system_message(
        sm_ids::YOUR_CLAN_LOST_500_REPUTATION_FOR_WITHDRAWING_FROM_THE_WAR,
    );
    broadcast_to_clan(world, clan_id, &lost);
    delete_clan_wars(world, clan_id, target_id);
    broadcast_war_status(world, clan_id, target_id);
}

/// `RequestSurrenderPledgeWar` (0x07) → `ClanWar.cancel`: declare defeat in a
/// mutual war — 500 reputation, the other side wins, the war ends and is torn
/// down moments later.
pub(crate) fn handle_request_surrender_pledge_war(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(name) = PacketReader::new(body).read_string() else {
        return;
    };
    let Some((clan_id, privs)) = crate::game_loop::guard::clan_and_privs(world, player) else {
        return;
    };
    let player_name = crate::game_loop::helpers::player_name_or_empty(world, player);
    if clan_id == 0 {
        return;
    }
    let member_in_combat = online_members(world, clan_id)
        .iter()
        .any(|&oid| crate::game_loop::combat::has_attack_stance(world, oid));
    if member_in_combat {
        send_sm_with(
            world,
            player,
            sm_ids::CEASE_FIRE_CANNOT_BE_CALLED_WHILE_MEMBERS_IN_BATTLE,
            &[],
        );
        return;
    }
    let Some(target) = by_name(world, &name) else {
        send_sm_with(
            world,
            player,
            sm_ids::S1_TEXT,
            &[SmParam::Text("No such clan.".to_string())],
        );
        return;
    };
    let target_id = target.id;
    let target_name = target.name.clone();
    let has_priv = world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.has_privilege(player, privs, CL_PLEDGE_WAR));
    if !has_priv {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT,
            &[],
        );
        return;
    }
    let Some(war) = war_between(world, clan_id, target_id) else {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_HAVE_NOT_DECLARED_A_CLAN_WAR_AGAINST_THE_CLAN_S1,
            &[SmParam::Text(target_name.clone())],
        );
        return;
    };
    if war.state == ClanWarState::BloodDeclaration {
        send_sm_with(
            world,
            player,
            sm_ids::CANNOT_DECLARE_DEFEAT_BEFORE_7_DAYS_WITH_CLAN_S1,
            &[SmParam::Text(target_name.clone())],
        );
        return;
    }

    // `ClanWar.cancel(player, cancelor)`.
    add_clan_reputation(world, clan_id, -500);
    let clan_name = clan_name_or_empty(world, clan_id);
    send_to_client(
        world,
        client_id,
        server_packets::surrender_pledge_war(&clan_name, &player_name),
    );
    let lost = server_packets::system_message_with(
        sm_ids::THE_WAR_ENDED_BY_YOUR_DEFEAT_DECLARATION_WITH_THE_S1_CLAN,
        &[SmParam::Text(target_name)],
    );
    broadcast_to_clan(world, clan_id, &lost);
    let won = server_packets::system_message_with(
        sm_ids::THE_WAR_ENDED_BY_THE_S1_CLAN_S_DEFEAT_DECLARATION,
        &[SmParam::Text(clan_name)],
    );
    broadcast_to_clan(world, target_id, &won);
    if let Some(w) = war_between_mut(world, clan_id, target_id) {
        w.winner_id = target_id;
        w.end_time = now_millis();
    }
    let war = war_between(world, clan_id, target_id)
        .expect("just updated")
        .clone();
    store_war(world, &war);
    // Java tears the ended war down 5 s later (the 21-day retention constant is
    // dead code in the live path).
    world.scheduler.schedule(
        world.tick + 50,
        crate::scheduler::ScheduledTask::ClanWarDelete {
            clan1: clan_id,
            clan2: target_id,
        },
    );
    broadcast_war_status(world, clan_id, target_id);
}

/// `ClanTable.deleteClanWars` — drop the war from memory + DB and reset both
/// clans' windows.
pub(crate) fn delete_clan_wars(world: &mut World, clan1: i32, clan2: i32) {
    world.clan_wars.retain(|w| {
        !((w.attacker_id == clan1 && w.attacked_id == clan2)
            || (w.attacker_id == clan2 && w.attacked_id == clan1))
    });
    let _ = world.db.send(DbCommand::DeleteClanWar { clan1, clan2 });
    broadcast_clan_status(world, clan1);
    broadcast_clan_status(world, clan2);
}

/// `ClanWar.clanWarTimeout` — 7 days of BLOOD_DECLARATION without an answer:
/// the war goes TIE and is torn down. A war gone MUTUAL in the meantime makes
/// this a no-op (Java cancels the task; the state check is our seq).
pub(crate) fn handle_clan_war_timeout(world: &mut World, attacker: i32, attacked: i32) {
    let Some(war) = war_between(world, attacker, attacked) else {
        return;
    };
    if war.state != ClanWarState::BloodDeclaration && war.state != ClanWarState::Declaration {
        return;
    }
    let attacker_name = clan_name_or_empty(world, attacker);
    let attacked_name = clan_name_or_empty(world, attacked);
    let cancelled = server_packets::system_message_with(
        sm_ids::A_CLAN_WAR_DECLARED_BY_CLAN_S1_WAS_CANCELLED,
        &[SmParam::Text(attacker_name)],
    );
    broadcast_to_clan(world, attacked, &cancelled);
    let no_fight_back = server_packets::system_message_with(
        sm_ids::BECAUSE_CLAN_S1_DID_NOT_FIGHT_BACK_THE_WAR_WAS_CANCELLED,
        &[SmParam::Text(attacked_name)],
    );
    broadcast_to_clan(world, attacker, &no_fight_back);
    if let Some(w) = war_between_mut(world, attacker, attacked) {
        w.state = ClanWarState::Tie;
        w.end_time = now_millis();
    }
    let war = war_between(world, attacker, attacked)
        .expect("just updated")
        .clone();
    store_war(world, &war);
    world.scheduler.schedule(
        world.tick + 50,
        crate::scheduler::ScheduledTask::ClanWarDelete {
            clan1: attacker,
            clan2: attacked,
        },
    );
    broadcast_war_status(world, attacker, attacked);
}

/// Boot re-arm (Java `ClanWar`'s restore constructor): pending declarations
/// get their remaining answer window; already-ended wars are torn down
/// shortly after boot (the live Java path's behavior).
pub(crate) fn rearm_clan_wars_at_boot(world: &mut World) {
    let now = now_millis();
    let wars: Vec<(i32, i32, i64, ClanWarState)> = world
        .clan_wars
        .iter()
        .map(|w| (w.attacker_id, w.attacked_id, w.start_time, w.state))
        .collect();
    for (attacker, attacked, start, state) in wars {
        let ended = world
            .clan_wars
            .iter()
            .find(|w| w.attacker_id == attacker && w.attacked_id == attacked)
            .is_some_and(|w| w.end_time > 0);
        if ended {
            world.scheduler.schedule(
                world.tick + 100,
                crate::scheduler::ScheduledTask::ClanWarDelete {
                    clan1: attacker,
                    clan2: attacked,
                },
            );
        } else if matches!(
            state,
            ClanWarState::BloodDeclaration | ClanWarState::Declaration
        ) {
            let remaining_ticks = (((start + WAR_TIMEOUT_MS) - now).max(0) / MS_PER_TICK) as u64;
            world.scheduler.schedule(
                world.tick + remaining_ticks,
                crate::scheduler::ScheduledTask::ClanWarTimeout { attacker, attacked },
            );
        }
    }
}

/// Java `ClanWar.onKill` — a war-relevant player kill. The caller (the death
/// pipeline) has already checked: killer and victim are players outside
/// PVP/siege zones, both clanned. Academy members on **either** side are
/// exempt, as in Java — a clan cannot farm war points off its own trainees.
/// (Java also runs an AntiFeed check, unported.)
pub(crate) fn clan_war_on_kill(world: &mut World, killer_oid: i32, victim_oid: i32) {
    let (killer_clan, killer_name) = match world.objects.get_component::<Player>(&killer_oid) {
        Some(p) => (p.clan_id, p.name.clone()),
        None => return,
    };
    let (victim_clan, victim_name, victim_level, victim_rep) =
        match world.objects.get_component::<Player>(&victim_oid) {
            Some(p) => (p.clan_id, p.name.clone(), p.level, p.reputation),
            None => return,
        };
    if killer_clan == 0 || victim_clan == 0 {
        return;
    }
    // Java `Player.doDie`: `!isAcademyMember() && !pk.isAcademyMember()`.
    if crate::game_loop::academy::is_academy_member(world, killer_oid)
        || crate::game_loop::academy::is_academy_member(world, victim_oid)
    {
        return;
    }
    let Some(war) = war_between(world, killer_clan, victim_clan) else {
        return;
    };
    let (state, attacker_id) = (war.state, war.attacker_id);

    if victim_level > 4 && state == ClanWarState::Mutual {
        // Mutual war: 1 reputation moves from the victim's clan to the
        // killer's — but only while the victim clan has any to lose.
        if world
            .clans
            .get(&victim_clan)
            .map(|c| c.reputation_score)
            .unwrap_or(0)
            > 0
        {
            add_clan_reputation(world, victim_clan, -REPUTATION_SCORE_PER_KILL);
            add_clan_reputation(world, killer_clan, REPUTATION_SCORE_PER_KILL);
        }
        let killer_clan_name = clan_name_or_empty(world, killer_clan);
        let victim_clan_name = clan_name_or_empty(world, victim_clan);
        let down = server_packets::system_message_with(
            sm_ids::BECAUSE_C1_KILLED_BY_S2_CLAN_REPUTATION_DECREASED_BY_1,
            &[
                SmParam::PlayerName(victim_name),
                SmParam::Text(killer_clan_name),
            ],
        );
        broadcast_to_clan_except(world, victim_clan, victim_oid, &down);
        let up = server_packets::system_message_with(
            sm_ids::BECAUSE_S1_MEMBER_KILLED_BY_C2_CLAN_REPUTATION_INCREASED_BY_1,
            &[
                SmParam::Text(victim_clan_name),
                SmParam::PlayerName(killer_name),
            ],
        );
        broadcast_to_clan_except(world, killer_clan, killer_oid, &up);
        if let Some(w) = war_between_mut(world, killer_clan, victim_clan) {
            if killer_clan == attacker_id {
                w.attacker_kills += 1;
            } else {
                w.attacked_kills += 1;
            }
        }
    } else if state == ClanWarState::BloodDeclaration
        && victim_clan == attacker_id
        && victim_rep >= 0
    {
        // The attacked side kills a declarer: 5 such kills force the war MUTUAL.
        let kill_count = {
            let w = war_between_mut(world, killer_clan, victim_clan).expect("checked above");
            w.attacked_kills += 1;
            w.attacked_kills
        };
        if kill_count >= 5 {
            if let Some(w) = war_between_mut(world, killer_clan, victim_clan) {
                w.state = ClanWarState::Mutual;
            }
            let killer_clan_name = clan_name_or_empty(world, killer_clan);
            let victim_clan_name = clan_name_or_empty(world, victim_clan);
            let started_k = server_packets::system_message_with(
                sm_ids::CLAN_WAR_STARTED_WITH_CLAN_S1,
                &[SmParam::Text(victim_clan_name)],
            );
            broadcast_to_clan(world, killer_clan, &started_k);
            let started_v = server_packets::system_message_with(
                sm_ids::CLAN_WAR_STARTED_WITH_CLAN_S1,
                &[SmParam::Text(killer_clan_name)],
            );
            broadcast_to_clan(world, victim_clan, &started_v);
            broadcast_war_status(world, killer_clan, victim_clan);
        } else {
            let victim_clan_name = clan_name_or_empty(world, victim_clan);
            let progress = server_packets::system_message_with(
                sm_ids::S1_MEMBER_KILLED_S2_MORE_KILLS_TO_START_WAR,
                &[
                    SmParam::Text(victim_clan_name),
                    SmParam::Int(5 - kill_count),
                ],
            );
            broadcast_to_clan(world, killer_clan, &progress);
        }
    } else {
        return;
    }
    let war = war_between(world, killer_clan, victim_clan)
        .expect("checked above")
        .clone();
    store_war(world, &war);
}

/// `Clan.broadcastToOtherOnlineMembers` — every online member except `except`.
fn broadcast_to_clan_except(world: &World, clan_id: i32, except: i32, pkt: &[u8]) {
    for oid in online_members(world, clan_id) {
        if oid != except {
            send_to_member(world, oid, pkt.to_vec());
        }
    }
}

// --- G18 slice 5: alliances ------------------------------------------------
