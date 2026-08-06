//! Duels — the 1v1 vertical slice (`Duel`, `DuelManager`, the `RequestDuel*`
//! packet family).
//!
//! A duel is a consented, consequence-free fight: the loser is not killed, no
//! karma or PvP counters move, and both sides are restored when it ends. Java
//! models it as a `Duel` object owned by `DuelManager`, driven by a countdown
//! task and then a per-second condition check.
//!
//! **Scope — 1v1 only.** Java also supports *party* duels, which teleport both
//! parties into a dedicated arena instance, open its doors, and snapshot every
//! member's condition. That needs instances (G27) and the arena data, so this
//! slice implements the player-vs-player duel that happens where the two
//! players stand. `party_duel` on the wire is honoured only far enough to
//! refuse it politely (`TODO(G27)`).
//!
//! G25's olympiad matches reuse this shape, which is why the audit put duels
//! here rather than with the end-game milestones.

use crate::model::Player;
use crate::model::components::{DuelRef, PlayerVitals, Position, Vitals};
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::scheduler::ScheduledTask;
use crate::world::World;

/// `Duel.PLAYER_DUEL_DURATION` — 120 s, in 100 ms ticks.
const DUEL_DURATION_TICKS: u64 = 1200;
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
}

impl Duel {
    fn other(&self, oid: i32) -> i32 {
        if oid == self.player_a {
            self.player_b
        } else {
            self.player_a
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
        // TODO(G27): party duels teleport both parties into an arena instance.
        super::admin::send_message(world, client_id, "Party duels are not available yet.");
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
        let target_name = player_name(world, target);
        send_sm(
            world,
            challenger,
            reason,
            &[SmParam::PlayerName(target_name)],
        );
        return;
    }
    if distance(world, challenger, target) > DUEL_REQUEST_RANGE {
        let target_name = player_name(world, target);
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
        crate::model::components::PendingDuel { challenger },
    );
    let challenger_name = player_name(world, challenger);
    send_to(
        world,
        target,
        server_packets::ex_duel_ask_start(&challenger_name, 0),
    );
    send_sm(
        world,
        challenger,
        sm_ids::C1_HAS_BEEN_CHALLENGED_TO_A_DUEL,
        &[SmParam::PlayerName(player_name(world, target))],
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

    if response != 1 {
        send_sm(
            world,
            challenger,
            sm_ids::C1_HAS_DECLINED_YOUR_CHALLENGE_TO_A_DUEL,
            &[SmParam::PlayerName(player_name(world, responder))],
        );
        return;
    }
    // Both must *still* be able to duel (Java re-checks on the answer).
    if can_duel(world, challenger).is_err() || can_duel(world, responder).is_err() {
        send_sm(
            world,
            challenger,
            sm_ids::YOU_ARE_UNABLE_TO_REQUEST_A_DUEL_AT_THIS_TIME,
            &[],
        );
        return;
    }
    start_countdown(world, challenger, responder);
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
    let winner = duel.other(oid);
    end_duel(world, duel_id, DuelResult::Win { winner, loser: oid });
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

fn start_countdown(world: &mut World, a: i32, b: i32) {
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
        },
    );
    // Both are "in" the duel from the countdown on, so neither can be
    // challenged again mid-countdown.
    world.objects.add_components(&a, DuelRef(id));
    world.objects.add_components(&b, DuelRef(id));
    world.scheduler.schedule(
        world.tick + COUNTDOWN_STEP_TICKS,
        ScheduledTask::DuelCountdown { duel_id: id },
    );
}

/// One countdown second (`Duel.countdown`): announce, then either continue or
/// begin. The teleport step Java runs at count 4 is party-only.
pub(crate) fn handle_countdown(world: &mut World, duel_id: u32) {
    let Some(duel) = world.duels.get_mut(&duel_id) else {
        return;
    };
    duel.countdown -= 1;
    let count = duel.countdown;
    let (a, b) = (duel.player_a, duel.player_b);

    if count > 0 {
        for oid in [a, b] {
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
    for oid in [a, b] {
        send_sm(world, oid, sm_ids::LET_THE_DUEL_BEGIN, &[]);
    }
    start_duel(world, duel_id);
}

/// `Duel.startDuel` (the 1v1 branch): both sides go live, get the ready/start
/// packets, and the condition check begins ticking.
fn start_duel(world: &mut World, duel_id: u32) {
    let Some(duel) = world.duels.get_mut(&duel_id) else {
        return;
    };
    duel.ends_at_tick = world.tick + DUEL_DURATION_TICKS;
    let (a, b) = (duel.player_a, duel.player_b);

    for oid in [a, b] {
        send_to(world, oid, server_packets::ex_duel_ready(0));
        send_to(world, oid, server_packets::ex_duel_start(0));
        // Java broadcasts the opponent's duel HP/MP/CP bar to each side.
        let opponent = if oid == a { b } else { a };
        if let Some(pkt) = duel_user_info(world, opponent) {
            send_to(world, oid, pkt);
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

    // A duelist who logged out / vanished cancels it.
    if !world.objects.has_component::<Player>(&a) || !world.objects.has_component::<Player>(&b) {
        end_duel(world, duel_id, DuelResult::Canceled);
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

    for oid in [a, b] {
        world.objects.remove_component::<DuelRef>(&oid);
        send_to(world, oid, server_packets::ex_duel_end(0));
    }

    match result {
        DuelResult::Win { winner, loser } => {
            let (wname, lname) = (player_name(world, winner), player_name(world, loser));
            send_sm(
                world,
                winner,
                sm_ids::C1_HAS_WON_THE_DUEL,
                &[SmParam::PlayerName(wname.clone())],
            );
            send_sm(
                world,
                loser,
                sm_ids::C1_HAS_WON_THE_DUEL,
                &[SmParam::PlayerName(wname)],
            );
            let _ = lname;
        }
        DuelResult::Canceled => {
            for oid in [a, b] {
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
    for (i, oid) in [a, b].into_iter().enumerate() {
        let (hp, mp, cp) = snapshot[i];
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&oid) {
            v.dead = false;
            v.cur_hp = hp.min(v.max_hp as f64);
            v.cur_mp = mp.min(v.max_mp as f64);
        }
        if let Some(pv) = world.objects.get_component_mut::<PlayerVitals>(&oid) {
            pv.cur_cp = cp.min(pv.max_cp as f64);
        }
        super::party::broadcast_user_info(world, oid);
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
    if let Some(duel_id) = world.objects.get_component::<DuelRef>(&target).map(|r| r.0) {
        end_duel(
            world,
            duel_id,
            DuelResult::Win {
                winner: attacker,
                loser: target,
            },
        );
    }
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
    let (Some(pa), Some(pb)) = (
        world.objects.get_component::<Position>(&a).copied(),
        world.objects.get_component::<Position>(&b).copied(),
    ) else {
        return f64::MAX;
    };
    let (dx, dy) = ((pa.x - pb.x) as f64, (pa.y - pb.y) as f64);
    (dx * dx + dy * dy).sqrt()
}

fn player_name(world: &World, oid: i32) -> String {
    world
        .objects
        .get_component::<Player>(&oid)
        .map(|p| p.name.clone())
        .unwrap_or_default()
}

fn send_to(world: &World, oid: i32, packet: Vec<u8>) {
    crate::game_loop::helpers::send_to_player(world, oid, packet);
}

fn send_sm(world: &World, oid: i32, id: i16, params: &[SmParam]) {
    crate::game_loop::helpers::send_sm_to_player(world, oid, id, params);
}
