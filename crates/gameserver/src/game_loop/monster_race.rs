//! Monster Race Track (G26.5) — port of Java `MonsterRace`: the pure race math
//! (per-lane speed roll → winner, pari-mutuel odds), and the 1-second race-cycle
//! state machine that runs the Derby Track (spawn placeholders, `MonRaceInfo`
//! board/animation, and the winner). Betting + payout via the `RaceManager` NPC
//! and `mdt_*` persistence are slice 4 (history/bets are in-memory here).

use std::collections::HashMap;

use rand::seq::SliceRandom;
use rand::Rng;

use crate::data::zone_data::ZoneKind;
use crate::enums::ChatType;
use crate::model::components::Position;
use crate::model::monster_race::{HistoryInfo, RaceState, LANES};
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::World;

const TICKS_PER_SECOND: u64 = 10;
/// The racer NPC templates (Java `for i in 31003..31027`).
const FIRST_RACER_TEMPLATE: i32 = 31003;
const LAST_RACER_TEMPLATE: i32 = 31026;
/// `MonRaceInfo` phase codes (Java `CODES`): set up / they're off / mid-race.
const CODE_SETUP: (i32, i32) = (-1, 0);
const CODE_OFF: (i32, i32) = (0, 15322);
const CODE_MID: (i32, i32) = (13765, -1);

/// Roll the eight lanes' 20-step speed tables and decide the placings (Java
/// `MonsterRace.newSpeeds`). Returns `(speeds, first, second)` where `first`/
/// `second` are `(lane, total_speed)` — lane `8 - i` for monster index `i`, so
/// index 0 is lane 8 and index 7 is lane 1. Each step is `Rnd.get(60) + 65`
/// (65..=124), except the final step which is a flat 100.
pub(crate) fn roll_speeds(rng: &mut impl Rng) -> ([[i32; 20]; LANES], (i32, i32), (i32, i32)) {
    let mut speeds = [[0i32; 20]; LANES];
    let mut first = (0i32, 0i32);
    let mut second = (0i32, 0i32);
    for (i, lane_speeds) in speeds.iter_mut().enumerate() {
        let mut total = 0;
        for (j, step) in lane_speeds.iter_mut().enumerate() {
            *step = if j == 19 {
                100
            } else {
                rng.gen_range(65..=124)
            };
            total += *step;
        }
        let lane = 8 - i as i32;
        if total >= first.1 {
            second = first;
            first = (lane, total);
        } else if total >= second.1 {
            second = (lane, total);
        }
    }
    (speeds, first, second)
}

/// Pari-mutuel odds per lane in lane order 1..=8 (Java `calculateOdds`): a lane
/// with no bets pays `0`, else `max(1.25, totalPool * 0.7 / laneBets)`.
pub(crate) fn calculate_odds(bets: &HashMap<i32, i64>) -> Vec<f64> {
    let total: i64 = (1..=LANES as i32)
        .map(|l| bets.get(&l).copied().unwrap_or(0))
        .sum();
    (1..=LANES as i32)
        .map(|lane| {
            let amount = bets.get(&lane).copied().unwrap_or(0);
            if amount == 0 {
                0.0
            } else {
                (total as f64 * 0.7 / amount as f64).max(1.25)
            }
        })
        .collect()
}

/// Add `amount` to a lane's pooled bet (Java `setBetOnLane`, the in-memory half).
#[allow(dead_code)]
pub(crate) fn add_bet(bets: &mut HashMap<i32, i64>, lane: i32, amount: i64) {
    *bets.entry(lane).or_insert(0) += amount;
}

// ---------------------------------------------------------------------------
// The 1-second race cycle (Java `MonsterRace.Announcement`)
// ---------------------------------------------------------------------------

/// Begin the perpetual race cycle at boot (Java `scheduleAtFixedRate(new
/// Announcement(), 0, 1000)`). No-op unless `AllowRace`.
pub(crate) fn start(world: &mut World) {
    if !world.cfg.general.allow_race {
        return;
    }
    if world.monster_race.race_number == 0 {
        world.monster_race.race_number = 1;
    }
    schedule_tick(world);
}

fn schedule_tick(world: &mut World) {
    world.scheduler.schedule(
        world.tick + TICKS_PER_SECOND,
        ScheduledTask::MonsterRaceTick,
    );
}

/// One 1-second beat of the race cycle: advance the `countdown` timeline (Java
/// `Announcement.run` on `_finalCountdown`), then re-arm.
pub(crate) fn tick(world: &mut World) {
    if !world.cfg.general.allow_race {
        return;
    }
    if world.monster_race.countdown > 1200 {
        world.monster_race.countdown = 0;
    }
    match world.monster_race.countdown {
        0 => {
            new_race(world);
            world.monster_race.state = RaceState::AcceptingBets;
            let pkt = race_packet(world, CODE_SETUP);
            broadcast_to_derby(world, &pkt);
            let n = world.monster_race.race_number;
            announce(
                world,
                &format!("Tickets are now available for Monster Race #{n}."),
            );
        }
        900 => {
            // Sales close; post the odds.
            world.monster_race.state = RaceState::Waiting;
            world.monster_race.odds = calculate_odds(&world.monster_race.bets);
            let n = world.monster_race.race_number;
            announce(
                world,
                &format!("Ticket sales are closed for Monster Race #{n}. Odds are posted."),
            );
        }
        1080 => {
            world.monster_race.state = RaceState::StartingRace;
            let pkt = race_packet(world, CODE_OFF);
            broadcast_to_derby(world, &pkt);
            announce(world, "They're off!");
        }
        1085 => {
            let pkt = race_packet(world, CODE_MID);
            broadcast_to_derby(world, &pkt);
        }
        1115 => {
            world.monster_race.state = RaceState::RaceEnd;
            finish_race(world);
        }
        1140 => {
            let monsters = world.monster_race.monsters;
            for oid in monsters {
                let pkt = server_packets::delete_object(oid);
                broadcast_to_derby(world, &pkt);
            }
        }
        _ => {
            // TODO(G26.5): the intermediate ticket-sale reminder cadence (Java's
            //   many 30-second SM cases) — cosmetic announcements only.
        }
    }
    world.monster_race.countdown += 1;
    schedule_tick(world);
}

/// Java `newRace` + `newSpeeds`: a fresh history row, eight shuffled racers
/// (packet-only object ids), and the speed roll that fixes the winner.
fn new_race(world: &mut World) {
    let race_number = world.monster_race.race_number;
    world.monster_race.history.push(HistoryInfo {
        race_id: race_number,
        ..Default::default()
    });

    let mut templates: Vec<i32> = (FIRST_RACER_TEMPLATE..=LAST_RACER_TEMPLATE).collect();
    templates.shuffle(&mut world.rng);
    for (lane, &template) in templates.iter().take(LANES).enumerate() {
        let oid = world.next_npc_object_id;
        world.next_npc_object_id += 1;
        world.monster_race.monsters[lane] = oid;
        world.monster_race.monster_templates[lane] = template;
    }

    let (speeds, first, second) = roll_speeds(&mut world.rng);
    world.monster_race.speeds = speeds;
    world.monster_race.first = first;
    world.monster_race.second = second;
}

/// Java's `RACE_END` block: record the placings + winning odds into the current
/// history row, clear the bets, announce the result, advance the race number.
fn finish_race(world: &mut World) {
    let (first_lane, second_lane) = (world.monster_race.first.0, world.monster_race.second.0);
    let odd_rate = world
        .monster_race
        .odds
        .get((first_lane - 1).max(0) as usize)
        .copied()
        .unwrap_or(0.0);
    if let Some(h) = world.monster_race.history.last_mut() {
        h.first = first_lane;
        h.second = second_lane;
        h.odd_rate = odd_rate;
    }
    // TODO(G26.5) slice 4: persist the history row + pay out winning bets;
    //   here the bets are only cleared in memory.
    for v in world.monster_race.bets.values_mut() {
        *v = 0;
    }
    let n = world.monster_race.race_number;
    announce(
        world,
        &format!("First prize goes to lane {first_lane}, second to lane {second_lane}. Monster Race #{n} is finished."),
    );
    world.monster_race.race_number += 1;
}

/// Build a `MonRaceInfo` for the current racers with the given phase code.
fn race_packet(world: &World, code: (i32, i32)) -> Vec<u8> {
    let mut monsters = [(0i32, 0i32, 0.0f64, 0.0f64); LANES];
    for (lane, m) in monsters.iter_mut().enumerate() {
        let template = world.monster_race.monster_templates[lane];
        let (display, coll_h, coll_r) = world
            .data
            .npc_data
            .get(template)
            .map(|t| (t.display_id, t.collision_height, t.collision_radius))
            .unwrap_or((template, 0.0, 0.0));
        *m = (world.monster_race.monsters[lane], display, coll_h, coll_r);
    }
    server_packets::mon_race_info(code.0, code.1, &monsters, &world.monster_race.speeds)
}

/// Send a packet to every player standing in a Derby Track zone (Java
/// `Broadcast.toAllPlayersInZoneType(DerbyTrackZone.class, …)`).
fn broadcast_to_derby(world: &World, pkt: &[u8]) {
    for cs in world.clients.values() {
        let ClientSession::InGame(s) = cs else {
            continue;
        };
        let Some(pos) = world
            .objects
            .get_component::<Position>(&s.player_object_id())
        else {
            continue;
        };
        if world
            .data
            .zone_data
            .zones_at(pos.x, pos.y, pos.z)
            .any(|z| z.kind == ZoneKind::DerbyTrack)
        {
            cs.send(pkt.to_vec());
        }
    }
}

fn announce(world: &World, text: &str) {
    let pkt = server_packets::creature_say(0, ChatType::Announcement, "", text, None);
    broadcast_to_derby(world, &pkt);
}
