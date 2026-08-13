//! Duels — the 1v1 vertical slice (`Duel`, `DuelManager`, the `RequestDuel*`
//! packet family).
//!
//! A duel is a consented, consequence-free fight: the loser is not killed, no
//! karma or PvP counters move, and both sides are restored when it ends. Java
//! models it as a `Duel` object owned by `DuelManager`, driven by a countdown
//! task and then a per-second condition check.
//!
//! **Party duels** ride the same machinery: the leaders challenge and answer,
//! every member must pass `canDuel`, and at countdown 4 both teams snapshot
//! (vitals *and* positions) and teleport into a fresh instance built from a
//! random Olympiad arena template — the fight lasts 5 minutes, a surrender by
//! any member forfeits for the whole team, and the end restores and teleports
//! everyone back before the instance dies. Java quirk ported as-is: a member
//! knocked out while a teammate still stands hands the WIN to the *other*
//! team (`onPlayerDefeat`'s inverted `teamdefeated` test), and knocking out
//! the *last* member sets no winner at all — the bout then runs to its
//! timeout tie. Retail surely intended last-man-standing, but this is what
//! the reference does.
//!
//! G25's olympiad matches reuse this shape, which is why the audit put duels
//! here rather than with the end-game milestones.

use super::helpers::send_sm_to_player as send_sm;
use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers::player_name_or_empty;
use crate::game_loop::helpers::send_to_player;
use crate::model::Player;
use crate::model::components::{DuelRef, PlayerVitals, Position, Vitals};
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::scheduler::ScheduledTask;
use crate::world::World;

/// `Duel.PLAYER_DUEL_DURATION` — 120 s, in 100 ms ticks.
const DUEL_DURATION_TICKS: u64 = 1200;
/// `Duel.PARTY_DUEL_DURATION` — 300 s.
const PARTY_DUEL_DURATION_TICKS: u64 = 3000;
/// `DuelManager.ARENAS` — the four Olympiad arena instance templates (all
/// four share the grassy arena's geometry on this dist).
const DUEL_ARENAS: [i32; 4] = [147, 148, 149, 150];
/// The grassy-arena team spawn points (the same coordinates the olympiad
/// matches use; the zone's spawn list halves map to these two ends).
const DUEL_SPAWN_A: (i32, i32, i32) = (-89597, -252841, -3320);
const DUEL_SPAWN_B: (i32, i32, i32) = (-86544, -252846, -3320);
/// `Duel._countdown` starts at 5 and is decremented once per second; the
/// teleport step (count 4) is party-only, so a 1v1 counts 4…1 then begins.
const COUNTDOWN_START: i32 = 5;
/// One countdown step.
const COUNTDOWN_STEP_TICKS: u64 = 10;
/// `RequestDuelStart`: the challenger must be within 250 units.
const DUEL_REQUEST_RANGE: f64 = 250.0;
/// `checkEndDuelCondition`: drifting more than 1600 units apart cancels it.
const DUEL_MAX_SEPARATION: f64 = 1600.0;

/// One running duel (Java `Duel`), owned by `World.duels`.
#[derive(Debug, Clone)]
pub struct Duel {
    pub id: u32,
    pub player_a: i32,
    pub player_b: i32,
    /// Seconds left on the pre-fight countdown; the duel is live at 0.
    pub countdown: i32,
    /// Absolute tick the duel times out (`_duelEndTime`), set when it starts.
    pub ends_at_tick: u64,
    /// 0 none, 1 = A gave up, 2 = B gave up (`_surrenderRequest`).
    pub surrender: u8,
    /// `PlayerCondition`'s HP/MP/CP snapshot, taken when the duel is created
    /// (before the countdown) and restored at the end — a duel leaves no mark
    /// either way. `[a, b]`.
    pub snapshot: [(f64, f64, f64); 2],
    /// Party duel? (`_partyDuel`). The fields below are empty for a 1v1.
    pub party: bool,
    /// The two teams, leaders first (Java uses the live party; the roster is
    /// fixed when the countdown starts).
    pub team_a: Vec<i32>,
    pub team_b: Vec<i32>,
    /// Per-member `PlayerCondition` for a party duel: vitals + the return
    /// position (`_x/_y/_z`), captured at the count-4 teleport step.
    pub member_snapshot: Vec<(i32, (f64, f64, f64), (i32, i32, i32))>,
    /// The arena instance a party duel fights in (0 until the teleport step).
    pub instance_id: i32,
    /// Members knocked to `DUELSTATE_DEAD` (capped at 1 HP).
    pub defeated: Vec<i32>,
    /// 0 = undecided; 1/2 = that team was declared winner (Java's
    /// `DUELSTATE_WINNER` on the leaders, read by the tick).
    pub winner_team: u8,
}

impl Duel {
    fn other(&self, oid: i32) -> i32 {
        if oid == self.player_a {
            self.player_b
        } else {
            self.player_a
        }
    }

    /// Every duellist (both members of a 1v1, both rosters of a party duel).
    fn everyone(&self) -> Vec<i32> {
        if self.party {
            self.team_a.iter().chain(&self.team_b).copied().collect()
        } else {
            vec![self.player_a, self.player_b]
        }
    }

    /// 1 or 2 for a rostered member, 0 for a stranger.
    fn team_of(&self, oid: i32) -> u8 {
        if self.team_a.contains(&oid) {
            1
        } else if self.team_b.contains(&oid) {
            2
        } else {
            0
        }
    }
}

/// How a duel finished — Java's `DuelResult`, minus the party variants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DuelResult {
    /// Someone won outright (the loser dropped or gave up).
    Win { winner: i32, loser: i32 },
    /// Broke off without a winner: timeout, drifting apart, a disconnect.
    Canceled,
}

// ---------------------------------------------------------------------------
// Gates
// ---------------------------------------------------------------------------

/// Java `Player.canDuel()` — and, when it refuses, the message explaining why.
///
/// Ported: in combat, dead, below half HP **or** MP, already dueling. Java also
/// checks jail, trade/store/request state, transformation, fishing, mounts,
/// olympiad, sieges and the "no duel" zones — the ones with ported state are
/// here, the rest arrive with their systems.
pub(crate) fn can_duel(world: &World, oid: i32) -> Result<(), i16> {
    if super::combat::has_attack_stance(world, oid) {
        return Err(sm_ids::C1_CANNOT_DUEL_BECAUSE_C1_IS_CURRENTLY_ENGAGED_IN_BATTLE);
    }
    let Some(v) = world.objects.get_component::<Vitals>(&oid) else {
        return Err(sm_ids::YOU_ARE_UNABLE_TO_REQUEST_A_DUEL_AT_THIS_TIME);
    };
    if v.dead || v.cur_hp < (v.max_hp as f64 / 2.0) || v.cur_mp < (v.max_mp as f64 / 2.0) {
        return Err(sm_ids::C1_CANNOT_DUEL_BECAUSE_C1_S_HP_OR_MP_IS_BELOW_50);
    }
    if world.objects.has_component::<DuelRef>(&oid) {
        return Err(sm_ids::C1_CANNOT_DUEL_BECAUSE_C1_IS_ALREADY_ENGAGED_IN_A_DUEL);
    }
    Ok(())
}

pub(crate) fn is_in_duel(world: &World, oid: i32) -> bool {
    world.objects.has_component::<DuelRef>(&oid)
}

/// Are these two currently dueling *each other*? Damage between them is
/// consequence-free, and the loser is never actually killed.
pub(crate) fn are_dueling(world: &World, a: i32, b: i32) -> bool {
    match (
        world.objects.get_component::<DuelRef>(&a).map(|r| r.0),
        world.objects.get_component::<DuelRef>(&b).map(|r| r.0),
    ) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Request / answer / surrender
// ---------------------------------------------------------------------------

/// `RequestDuelStart` — challenge the named player.
pub(crate) fn handle_request_duel_start(world: &mut World, client_id: u32, body: &[u8]) {
    let Some((name, party_duel)) = crate::network::client_packets::read_duel_start(body) else {
        return;
    };
    let Some(challenger) = world.player_oid(client_id) else {
        return;
    };

    if party_duel != 0 {
        handle_party_duel_start(world, client_id, challenger, &name);
        return;
    }

    let Some((_, target)) = super::party::find_player_by_name(world, &name) else {
        send_sm(
            world,
            challenger,
            sm_ids::THERE_IS_NO_OPPONENT_TO_RECEIVE_YOUR_CHALLENGE_FOR_A_DUEL,
            &[],
        );
        return;
    };
    if target == challenger {
        send_sm(
            world,
            challenger,
            sm_ids::THERE_IS_NO_OPPONENT_TO_RECEIVE_YOUR_CHALLENGE_FOR_A_DUEL,
            &[],
        );
        return;
    }
    if can_duel(world, challenger).is_err() {
        send_sm(
            world,
            challenger,
            sm_ids::YOU_ARE_UNABLE_TO_REQUEST_A_DUEL_AT_THIS_TIME,
            &[],
        );
        return;
    }
    if let Err(reason) = can_duel(world, target) {
        // Java forwards the *target's* refusal reason to the challenger.
        let target_name = player_name_or_empty(world, target);
        send_sm(
            world,
            challenger,
            reason,
            &[SmParam::PlayerName(target_name)],
        );
        return;
    }
    if distance(world, challenger, target) > DUEL_REQUEST_RANGE {
        let target_name = player_name_or_empty(world, target);
        send_sm(
            world,
            challenger,
            sm_ids::C1_IS_TOO_FAR_AWAY_TO_RECEIVE_A_DUEL_CHALLENGE,
            &[SmParam::PlayerName(target_name)],
        );
        return;
    }

    // Park the pending challenge on the target and ask them.
    world.objects.add_components(
        &target,
        crate::model::components::PendingDuel {
            challenger,
            party: false,
        },
    );
    let challenger_name = player_name_or_empty(world, challenger);
    send_to_player(
        world,
        target,
        server_packets::ex_duel_ask_start(&challenger_name, 0),
    );
    send_sm(
        world,
        challenger,
        sm_ids::C1_HAS_BEEN_CHALLENGED_TO_A_DUEL,
        &[SmParam::PlayerName(player_name_or_empty(world, target))],
    );
}

/// `RequestDuelAnswerStart` — accept (1) or decline the pending challenge.
pub(crate) fn handle_request_duel_answer(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(response) = crate::network::client_packets::read_duel_answer(body) else {
        return;
    };
    let Some(responder) = world.player_oid(client_id) else {
        return;
    };
    let Some(pending) = world
        .objects
        .get_component::<crate::model::components::PendingDuel>(&responder)
        .copied()
    else {
        return;
    };
    world
        .objects
        .remove_component::<crate::model::components::PendingDuel>(&responder);
    let challenger = pending.challenger;
    let party = pending.party;

    if response != 1 {
        send_sm(
            world,
            challenger,
            sm_ids::C1_HAS_DECLINED_YOUR_CHALLENGE_TO_A_DUEL,
            &[SmParam::PlayerName(player_name_or_empty(world, responder))],
        );
        return;
    }
    // Both sides must *still* be able to duel (Java re-checks on the answer);
    // for a party duel that means every member of both rosters.
    let (ok, team_a, team_b) = if party {
        let ta = party_members_of(world, challenger);
        let tb = party_members_of(world, responder);
        let all_ok = !ta.is_empty()
            && !tb.is_empty()
            && ta.iter().chain(&tb).all(|&m| can_duel(world, m).is_ok());
        (all_ok, ta, tb)
    } else {
        (
            can_duel(world, challenger).is_ok() && can_duel(world, responder).is_ok(),
            Vec::new(),
            Vec::new(),
        )
    };
    if !ok {
        send_sm(
            world,
            challenger,
            sm_ids::YOU_ARE_UNABLE_TO_REQUEST_A_DUEL_AT_THIS_TIME,
            &[],
        );
        return;
    }
    start_countdown(world, challenger, responder, party, team_a, team_b);
}

/// `RequestDuelStart`'s party branch: the challenger must lead their party,
/// every member of both parties must pass `canDuel`, and the ask goes to the
/// *target's party leader* (Java sends `ExDuelAskStart(name, 1)` there).
fn handle_party_duel_start(world: &mut World, client_id: u32, challenger: i32, name: &str) {
    let team_a = party_members_of(world, challenger);
    if team_a.first() != Some(&challenger) {
        super::admin::send_message(
            world,
            client_id,
            "You have to be the leader of a party in order to request a party duel.",
        );
        return;
    }
    let Some((_, target)) = super::party::find_player_by_name(world, name) else {
        send_sm(
            world,
            challenger,
            sm_ids::THERE_IS_NO_OPPONENT_TO_RECEIVE_YOUR_CHALLENGE_FOR_A_DUEL,
            &[],
        );
        return;
    };
    let team_b = party_members_of(world, target);
    if team_b.is_empty() {
        super::admin::send_message(world, client_id, "This player is not in a party.");
        return;
    }
    if team_a.contains(&target) {
        super::admin::send_message(
            world,
            client_id,
            "This player is a member of your own party.",
        );
        return;
    }
    if team_a.iter().any(|&m| can_duel(world, m).is_err()) {
        super::admin::send_message(
            world,
            client_id,
            "Not all the members of your party are ready for a duel.",
        );
        return;
    }
    if team_b.iter().any(|&m| can_duel(world, m).is_err()) {
        // Java forwards the target-side refusal per member; one line covers it.
        super::admin::send_message(
            world,
            client_id,
            "The opposing party is currently unable to duel.",
        );
        return;
    }
    // The ask lands on the *other party's leader*, whoever was targeted.
    let leader_b = team_b[0];
    world.objects.add_components(
        &leader_b,
        crate::model::components::PendingDuel {
            challenger,
            party: true,
        },
    );
    let challenger_name = player_name_or_empty(world, challenger);
    send_to_player(
        world,
        leader_b,
        server_packets::ex_duel_ask_start(&challenger_name, 1),
    );
    send_sm(
        world,
        challenger,
        sm_ids::C1_HAS_BEEN_CHALLENGED_TO_A_DUEL,
        &[SmParam::PlayerName(player_name_or_empty(world, leader_b))],
    );
}

/// The player's party roster, leader first — empty when partyless.
fn party_members_of(world: &World, oid: i32) -> Vec<i32> {
    crate::game_loop::party::party_members(world, oid).unwrap_or_default()
}

/// `RequestDuelSurrender` — give up; the opponent wins.
pub(crate) fn handle_request_duel_surrender(world: &mut World, client_id: u32) {
    let Some(oid) = world.player_oid(client_id) else {
        return;
    };
    let Some(duel_id) = world.objects.get_component::<DuelRef>(&oid).map(|r| r.0) else {
        return;
    };
    let Some(duel) = world.duels.get(&duel_id) else {
        return;
    };
    if duel.party {
        // Java: any member may surrender, forfeiting for the whole team.
        let (winner, loser) = match duel.team_of(oid) {
            1 => (duel.player_b, duel.player_a),
            2 => (duel.player_a, duel.player_b),
            _ => return,
        };
        end_duel(world, duel_id, DuelResult::Win { winner, loser });
        return;
    }
    let winner = duel.other(oid);
    end_duel(world, duel_id, DuelResult::Win { winner, loser: oid });
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

fn start_countdown(
    world: &mut World,
    a: i32,
    b: i32,
    party: bool,
    team_a: Vec<i32>,
    team_b: Vec<i32>,
) {
    let id = world.next_duel_id;
    world.next_duel_id += 1;
    // `new PlayerCondition(player, …)` — the pre-duel vitals, taken at duel
    // creation.
    let snap = |oid: i32| {
        let v = world.objects.get_component::<Vitals>(&oid);
        let pv = world.objects.get_component::<PlayerVitals>(&oid);
        (
            v.map_or(0.0, |v| v.cur_hp),
            v.map_or(0.0, |v| v.cur_mp),
            pv.map_or(0.0, |p| p.cur_cp),
        )
    };
    let snapshot = [snap(a), snap(b)];
    let everyone: Vec<i32> = if party {
        team_a.iter().chain(&team_b).copied().collect()
    } else {
        Vec::new()
    };
    world.duels.insert(
        id,
        Duel {
            id,
            player_a: a,
            player_b: b,
            countdown: COUNTDOWN_START,
            ends_at_tick: 0,
            surrender: 0,
            snapshot,
            party,
            team_a,
            team_b,
            member_snapshot: Vec::new(),
            instance_id: 0,
            defeated: Vec::new(),
            winner_team: 0,
        },
    );
    // Everyone is "in" the duel from the countdown on, so nobody can be
    // challenged again mid-countdown.
    if party {
        // Java announces the upcoming teleport when the duel is created.
        for &oid in &everyone {
            world.objects.add_components(&oid, DuelRef(id));
            send_sm(
                world,
                oid,
                sm_ids::IN_A_MOMENT_YOU_WILL_BE_TRANSPORTED_TO_THE_SITE_WHERE_THE_DUEL_WILL_TAKE_PLACE,
                &[],
            );
        }
    } else {
        world.objects.add_components(&a, DuelRef(id));
        world.objects.add_components(&b, DuelRef(id));
    }
    world.scheduler.schedule(
        world.tick + COUNTDOWN_STEP_TICKS,
        ScheduledTask::DuelCountdown { duel_id: id },
    );
}

/// One countdown second (`Duel.countdown`): announce, then either continue or
/// begin. At count 4 a party duel snapshots everyone (vitals + return
/// position), builds the arena instance and teleports both teams in — then
/// waits 20 s (Java's post-teleport grace) before counting on.
pub(crate) fn handle_countdown(world: &mut World, duel_id: u32) {
    let Some(duel) = world.duels.get_mut(&duel_id) else {
        return;
    };
    duel.countdown -= 1;
    let count = duel.countdown;
    let (a, b) = (duel.player_a, duel.player_b);
    let party = duel.party;

    if party && count == 4 {
        teleport_party_duel(world, duel_id);
        world.scheduler.schedule(
            world.tick + 200, // Java: 20 s to complete the teleport
            ScheduledTask::DuelCountdown { duel_id },
        );
        return;
    }
    let members = world
        .duels
        .get(&duel_id)
        .map(|d| d.everyone())
        .unwrap_or_default();
    if count > 0 {
        for oid in members {
            send_sm(
                world,
                oid,
                sm_ids::THE_DUEL_WILL_BEGIN_IN_S1_SECOND_S,
                &[SmParam::Long(count as i64)],
            );
        }
        world.scheduler.schedule(
            world.tick + COUNTDOWN_STEP_TICKS,
            ScheduledTask::DuelCountdown { duel_id },
        );
        return;
    }
    for oid in members {
        send_sm(world, oid, sm_ids::LET_THE_DUEL_BEGIN, &[]);
    }
    let _ = (a, b);
    start_duel(world, duel_id);
}

/// `Duel.teleportPlayers` + `savePlayerConditions`: snapshot every member
/// (vitals and the spot to return to), spin up an instance from a random
/// Olympiad arena template, and port team A / team B to the two ends.
fn teleport_party_duel(world: &mut World, duel_id: u32) {
    let Some(duel) = world.duels.get(&duel_id) else {
        return;
    };
    let (team_a, team_b) = (duel.team_a.clone(), duel.team_b.clone());
    let template = DUEL_ARENAS[world.roll(DUEL_ARENAS.len() as i32) as usize];
    let instance_id = super::instances::create_from_template(world, template)
        .unwrap_or_else(|| world.instances.create(0));

    let mut snapshot = Vec::new();
    for &oid in team_a.iter().chain(&team_b) {
        let v = world.objects.get_component::<Vitals>(&oid);
        let pv = world.objects.get_component::<PlayerVitals>(&oid);
        let pos = world
            .objects
            .get_component::<Position>(&oid)
            .map_or((0, 0, 0), |p| (p.x, p.y, p.z));
        snapshot.push((
            oid,
            (
                v.map_or(0.0, |v| v.cur_hp),
                v.map_or(0.0, |v| v.cur_mp),
                pv.map_or(0.0, |p| p.cur_cp),
            ),
            pos,
        ));
    }
    if let Some(d) = world.duels.get_mut(&duel_id) {
        d.member_snapshot = snapshot;
        d.instance_id = instance_id;
    }
    for (team, spawn) in [(team_a, DUEL_SPAWN_A), (team_b, DUEL_SPAWN_B)] {
        for oid in team {
            world
                .objects
                .add_components(&oid, crate::model::components::InstanceId(instance_id));
            crate::game_loop::death::teleport_player(world, oid, spawn.0, spawn.1, spawn.2);
        }
    }
}

/// `Duel.startDuel` (the 1v1 branch): both sides go live, get the ready/start
/// packets, and the condition check begins ticking.
fn start_duel(world: &mut World, duel_id: u32) {
    let Some(duel) = world.duels.get_mut(&duel_id) else {
        return;
    };
    let party = duel.party;
    duel.ends_at_tick = world.tick
        + if party {
            PARTY_DUEL_DURATION_TICKS
        } else {
            DUEL_DURATION_TICKS
        };
    let (a, b) = (duel.player_a, duel.player_b);
    let flag = i32::from(party);
    let (team_a, team_b) = (duel.team_a.clone(), duel.team_b.clone());

    if party {
        // Each member gets the ready/start pair and every opponent's duel bar.
        for (mine, theirs) in [(&team_a, &team_b), (&team_b, &team_a)] {
            for &oid in mine {
                send_to_player(world, oid, server_packets::ex_duel_ready(flag));
                send_to_player(world, oid, server_packets::ex_duel_start(flag));
                for &opponent in theirs {
                    if let Some(pkt) = duel_user_info(world, opponent) {
                        send_to_player(world, oid, pkt);
                    }
                }
            }
        }
    } else {
        for oid in [a, b] {
            send_to_player(world, oid, server_packets::ex_duel_ready(flag));
            send_to_player(world, oid, server_packets::ex_duel_start(flag));
            // Java broadcasts the opponent's duel HP/MP/CP bar to each side.
            let opponent = if oid == a { b } else { a };
            if let Some(pkt) = duel_user_info(world, opponent) {
                send_to_player(world, oid, pkt);
            }
        }
    }
    world
        .scheduler
        .schedule(world.tick + 10, ScheduledTask::DuelTick { duel_id });
}

/// The per-second `checkEndDuelCondition` sweep.
pub(crate) fn handle_tick(world: &mut World, duel_id: u32) {
    let Some(duel) = world.duels.get(&duel_id).cloned() else {
        return;
    };
    let (a, b) = (duel.player_a, duel.player_b);

    // A leader who logged out / vanished cancels it.
    if !world.objects.has_component::<Player>(&a) || !world.objects.has_component::<Player>(&b) {
        end_duel(world, duel_id, DuelResult::Canceled);
        return;
    }
    if duel.party {
        // A team already declared winner (the defeat rule) ends it.
        match duel.winner_team {
            1 => {
                end_duel(
                    world,
                    duel_id,
                    DuelResult::Win {
                        winner: a,
                        loser: b,
                    },
                );
                return;
            }
            2 => {
                end_duel(
                    world,
                    duel_id,
                    DuelResult::Win {
                        winner: b,
                        loser: a,
                    },
                );
                return;
            }
            _ => {}
        }
        if world.tick >= duel.ends_at_tick {
            end_duel(world, duel_id, DuelResult::Canceled);
            return;
        }
        // The 1v1 separation / interruption checks don't apply — "party duels
        // take place in arenas" (Java `isDuelistInPvp`).
        world
            .scheduler
            .schedule(world.tick + 10, ScheduledTask::DuelTick { duel_id });
        return;
    }
    // Someone dropped → the other wins.
    for (loser, winner) in [(a, b), (b, a)] {
        if world
            .objects
            .get_component::<Vitals>(&loser)
            .is_some_and(|v| v.dead)
        {
            end_duel(world, duel_id, DuelResult::Win { winner, loser });
            return;
        }
    }
    if world.tick >= duel.ends_at_tick {
        end_duel(world, duel_id, DuelResult::Canceled);
        return;
    }
    if distance(world, a, b) > DUEL_MAX_SEPARATION {
        end_duel(world, duel_id, DuelResult::Canceled);
        return;
    }
    world
        .scheduler
        .schedule(world.tick + 10, ScheduledTask::DuelTick { duel_id });
}

/// `Duel.endDuel` — announce the outcome, clear the duel state, and restore
/// both sides.
pub(crate) fn end_duel(world: &mut World, duel_id: u32, result: DuelResult) {
    let Some(duel) = world.duels.remove(&duel_id) else {
        return;
    };
    let (a, b) = (duel.player_a, duel.player_b);
    let snapshot = duel.snapshot;
    let flag = i32::from(duel.party);
    let everyone = duel.everyone();

    for &oid in &everyone {
        world.objects.remove_component::<DuelRef>(&oid);
        send_to_player(world, oid, server_packets::ex_duel_end(flag));
    }

    match result {
        DuelResult::Win { winner, loser } => {
            // A party duel announces "C1's party has won"; a 1v1 "C1 has won".
            let sm = if duel.party {
                sm_ids::C1_S_PARTY_HAS_WON_THE_DUEL
            } else {
                sm_ids::C1_HAS_WON_THE_DUEL
            };
            let wname = player_name_or_empty(world, winner);
            for &oid in &everyone {
                send_sm(world, oid, sm, &[SmParam::PlayerName(wname.clone())]);
            }
            let _ = loser;
        }
        DuelResult::Canceled => {
            for &oid in &everyone {
                send_sm(world, oid, sm_ids::THE_DUEL_HAS_ENDED_IN_A_TIE, &[]);
            }
        }
    }

    // `restorePlayerConditions`: a duel leaves no mark. The loser was never
    // actually killed (see `duel_lethal_guard`), so this restores the
    // pre-duel HP/MP/CP snapshot exactly. (Java's `PlayerCondition` also has
    // a duel-debuff removal list, but its feeder — `DuelManager.onBuff` — has
    // no caller anywhere in this Java tree, so nothing is ever registered and
    // nothing is stripped there either.)
    if duel.party {
        // Party duels also teleport everyone back (`PlayerCondition` stored
        // the spot at the count-4 step) and tear the arena instance down.
        for (oid, (hp, mp, cp), (x, y, z)) in duel.member_snapshot {
            restore_condition(world, oid, (hp, mp, cp));
            world
                .objects
                .remove_component::<crate::model::components::InstanceId>(&oid);
            crate::game_loop::death::teleport_player(world, oid, x, y, z);
            super::player_info::broadcast_user_info(world, oid);
        }
        if duel.instance_id != 0 {
            super::instances::destroy(world, duel.instance_id);
        }
        return;
    }
    for (i, oid) in [a, b].into_iter().enumerate() {
        let (hp, mp, cp) = snapshot[i];
        restore_condition(world, oid, (hp, mp, cp));
        super::player_info::broadcast_user_info(world, oid);
    }
}

/// A duel never actually kills: Java stops the loser at 1 HP and ends the duel
/// instead. Called from the damage path before death is decided.
///
/// Returns true when the blow was capped, i.e. the target is a duel opponent of
/// the attacker and this hit would have finished them.
pub(crate) fn duel_lethal_guard(
    world: &mut World,
    attacker: i32,
    target: i32,
    damage: f64,
) -> bool {
    // The duellist is the acting player: a summon carries no `DuelRef`, so
    // without resolving, its blow is not recognised as duel damage and slips
    // past the cap — really killing the opponent and breaking the invariant
    // this function exists to hold.
    let attacker = crate::game_loop::pvp::acting_player(world, attacker);
    if !are_dueling(world, attacker, target) {
        return false;
    }
    let Some(v) = world.objects.get_component::<Vitals>(&target) else {
        return false;
    };
    if damage < v.cur_hp {
        return false;
    }
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&target) {
        v.cur_hp = 1.0;
    }
    let Some(duel_id) = world.objects.get_component::<DuelRef>(&target).map(|r| r.0) else {
        return true;
    };
    let is_party = world.duels.get(&duel_id).is_some_and(|d| d.party);
    if is_party {
        // Java `onPlayerDefeat`, quirk included: the knockout hands the WIN
        // to the attacker's team as long as a teammate of the victim still
        // stands; felling the LAST member sets no winner, and the bout runs
        // to its timeout tie. (Retail surely meant last-man-standing — port
        // the behaviour, not the intent.)
        if let Some(d) = world.duels.get_mut(&duel_id) {
            if !d.defeated.contains(&target) {
                d.defeated.push(target);
            }
            let victim_team = d.team_of(target);
            let teammate_standing = match victim_team {
                1 => d.team_a.iter().any(|m| !d.defeated.contains(m)),
                2 => d.team_b.iter().any(|m| !d.defeated.contains(m)),
                _ => false,
            };
            if teammate_standing && d.winner_team == 0 {
                d.winner_team = if victim_team == 1 { 2 } else { 1 };
            }
        }
        return true;
    }
    end_duel(
        world,
        duel_id,
        DuelResult::Win {
            winner: attacker,
            loser: target,
        },
    );
    true
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn duel_user_info(world: &World, oid: i32) -> Option<Vec<u8>> {
    let p = world.objects.get_component::<Player>(&oid)?;
    let v = world.objects.get_component::<Vitals>(&oid)?;
    let pv = world.objects.get_component::<PlayerVitals>(&oid)?;
    Some(server_packets::ex_duel_update_user_info(
        &p.name,
        oid,
        p.class_id,
        v.cur_hp as i32,
        v.max_hp,
        v.cur_mp as i32,
        v.max_mp,
        pv.cur_cp as i32,
        pv.max_cp,
        p.level,
    ))
}

fn distance(world: &World, a: i32, b: i32) -> f64 {
    let (Some(pa), Some(pb)) = (maybe_position(world, a), maybe_position(world, b)) else {
        return f64::MAX;
    };
    let (dx, dy) = ((pa.x - pb.x) as f64, (pa.y - pb.y) as f64);
    (dx * dx + dy * dy).sqrt()
}

/// Java `PlayerCondition.restoreCondition` — put a duellist's HP, MP and CP
/// back to the pre-duel snapshot and clear the death flag.
///
/// Each value is clamped to the *current* maximum rather than restored blind: a
/// buff that expired during the duel can have lowered the ceiling since the
/// snapshot was taken, and an over-max gauge would stick until the next stat
/// recalculation happened to notice.
fn restore_condition(world: &mut World, oid: i32, (hp, mp, cp): (f64, f64, f64)) {
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&oid) {
        v.dead = false;
        v.cur_hp = hp.min(v.max_hp as f64);
        v.cur_mp = mp.min(v.max_mp as f64);
    }
    if let Some(pv) = world.objects.get_component_mut::<PlayerVitals>(&oid) {
        pv.cur_cp = cp.min(pv.max_cp as f64);
    }
}
