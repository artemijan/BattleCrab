//! Monster Race Track (G26.5) — port of Java `MonsterRace` + the `RaceManager`
//! NPC: the pure race math (per-lane speed roll → winner, pari-mutuel odds), the
//! 1-second race-cycle state machine (board/animation via `MonRaceInfo`), the
//! `mdt_history`/`mdt_bets` persistence, and the RaceManager betting dialog
//! (buy a lane ticket, view odds/history, cash a winning ticket out).

use crate::game_loop::helpers::send_to_client;
use std::collections::HashMap;

use commons::util::rnd;

use super::helpers::send_sm_bare_to_client as send_sm;
use crate::data::zone_data::ZoneKind;
use crate::db::DbCommand;
use crate::enums::ChatType;
use crate::model::components::{Position, RaceTicket};
use crate::model::inventory::{Inventory, ItemChange};
use crate::model::monster_race::{HistoryInfo, LANES, RaceState};
use crate::network::enter_world as ew;
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::World;

const TICKS_PER_SECOND: u64 = 10;
/// The Monster Race ticket item (Java 4443) + the eight bet-price tiers.
const RACE_TICKET_ITEM: i32 = 4443;
const ADENA_ID: i32 = 57;
const TICKET_PRICES: [i64; LANES] = [100, 500, 1000, 5000, 10000, 20000, 50000, 100000];
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
pub(crate) fn roll_speeds() -> ([[i32; 20]; LANES], (i32, i32), (i32, i32)) {
    let mut speeds = [[0i32; 20]; LANES];
    let mut first = (0i32, 0i32);
    let mut second = (0i32, 0i32);
    for (i, lane_speeds) in speeds.iter_mut().enumerate() {
        let mut total = 0;
        for (j, step) in lane_speeds.iter_mut().enumerate() {
            *step = if j == 19 {
                100
            } else {
                rnd::get_range(65, 124)
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
pub(crate) fn add_bet(bets: &mut HashMap<i32, i64>, lane: i32, amount: i64) {
    *bets.entry(lane).or_insert(0) += amount;
}

// ---------------------------------------------------------------------------
// The 1-second race cycle (Java `MonsterRace.Announcement`)
// ---------------------------------------------------------------------------

/// Boot restore (Java `MonsterRace` constructor's `loadHistory`/`loadBets`),
/// driven by `DbEvent::MdtLoaded`: seed the history + lane bets, set the race
/// number from the record count, then begin the cycle.
pub(crate) fn on_mdt_loaded(world: &mut World, history: Vec<HistoryInfo>, bets: Vec<(i32, i64)>) {
    world.monster_race.race_number = history.len() as i32 + 1;
    world.monster_race.history = history;
    world.monster_race.bets = bets.into_iter().collect();
    start(world);
}

/// Begin the perpetual race cycle (Java `scheduleAtFixedRate(new Announcement(),
/// 0, 1000)`). No-op unless `AllowRace`.
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
        // Java's reminder cadence, in the free-text style this module
        // announces with: a 30-second "tickets available" drumbeat while the
        // window is open, the 10/5/1-minute sale-closing warnings, and the
        // pre-race countdown notices.
        c @ 300 | c @ 600 | c @ 840 => {
            let n = world.monster_race.race_number;
            let minutes = match c {
                300 => 10,
                600 => 5,
                _ => 1,
            };
            announce(
                world,
                &format!("Now selling tickets for Monster Race #{n}."),
            );
            announce(
                world,
                &format!("Ticket sales for the Monster Race will end in {minutes} minute(s)."),
            );
        }
        c @ 30..=870 if c % 30 == 0 => {
            let n = world.monster_race.race_number;
            announce(
                world,
                &format!("Tickets are now available for Monster Race #{n}."),
            );
        }
        c @ 960 | c @ 1020 => {
            let n = world.monster_race.race_number;
            let minutes = if c == 960 { 2 } else { 1 };
            announce(
                world,
                &format!("Monster Race #{n} will begin in {minutes} minute(s)."),
            );
        }
        1050 => {
            let n = world.monster_race.race_number;
            announce(
                world,
                &format!("Monster Race #{n} will begin in 30 seconds."),
            );
        }
        1070 => {
            let n = world.monster_race.race_number;
            announce(
                world,
                &format!("Monster Race #{n} is about to begin. Countdown in five seconds."),
            );
        }
        c @ 1075..=1079 => {
            let seconds = 1080 - c;
            announce(
                world,
                &format!("The race will begin in {seconds} second(s)."),
            );
        }
        _ => {}
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
    rnd::shuffle(&mut templates);
    for (lane, &template) in templates.iter().take(LANES).enumerate() {
        let oid = world.next_npc_object_id;
        world.next_npc_object_id += 1;
        world.monster_race.monsters[lane] = oid;
        world.monster_race.monster_templates[lane] = template;
    }

    let (speeds, first, second) = roll_speeds();
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
    let race_id = if let Some(h) = world.monster_race.history.last_mut() {
        h.first = first_lane;
        h.second = second_lane;
        h.odd_rate = odd_rate;
        h.race_id
    } else {
        world.monster_race.race_number
    };
    // Persist the result (Java `saveHistory`) and clear the pooled bets in both
    // memory and the DB (Java `clearBets`). Winning tickets pay out later, when
    // their holder cashes them in at the RaceManager (Java `CalculateWin`).
    let _ = world.db.send(DbCommand::SaveMdtHistory {
        race_id,
        first: first_lane,
        second: second_lane,
        odd_rate,
    });
    for v in world.monster_race.bets.values_mut() {
        *v = 0;
    }
    let _ = world.db.send(DbCommand::ClearMdtBets);
    let n = world.monster_race.race_number;
    announce(
        world,
        &format!(
            "First prize goes to lane {first_lane}, second to lane {second_lane}. Monster Race #{n} is finished."
        ),
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

// ---------------------------------------------------------------------------
// The RaceManager NPC dialog (Java `RaceManager.onBypassFeedback`, NPC 30995)
// ---------------------------------------------------------------------------

/// Route a RaceManager bypass verb (`BuyTicket`/`ShowOdds`/…) to its handler.
pub(crate) fn race_bypass(
    world: &mut World,
    client_id: u32,
    player: i32,
    npc_oid: i32,
    command: &str,
) {
    match command.split(' ').next().unwrap_or("") {
        "BuyTicket" => buy_ticket(world, client_id, player, npc_oid, command),
        "ShowOdds" => show_odds(world, client_id, player, npc_oid),
        "ShowInfo" => show_info(world, client_id, npc_oid),
        "ShowTickets" => show_tickets(world, client_id, player, npc_oid),
        "ShowTicket" => show_ticket(world, client_id, player, npc_oid, command),
        "CalculateWin" => calculate_win(world, client_id, player, command),
        "ViewHistory" => view_history(world, client_id, npc_oid),
        _ => {}
    }
}

/// Java `RaceManager` `BuyTicket <val>`: the multi-step lane/price picker, and
/// the final purchase (`val >= 21`) that charges adena, mints ticket 4443, and
/// pools the bet.
fn buy_ticket(world: &mut World, client_id: u32, player: i32, npc_oid: i32, command: &str) {
    if !world.cfg.general.allow_race || world.monster_race.state != RaceState::AcceptingBets {
        send_sm(
            world,
            client_id,
            sm_ids::MONSTER_RACE_TICKETS_ARE_NO_LONGER_AVAILABLE,
        );
        chat0(world, client_id, npc_oid);
        return;
    }
    let mut val = arg(command);
    let mut t = ticket(world, player);
    if val == 0 {
        t = [0, 0];
    }
    // Java: a stray "next step" click with the prior step unset restarts.
    if (val == 10 && t[0] == 0) || (val == 20 && (t[0] == 0 || t[1] == 0)) {
        val = 0;
    }

    if val < 10 {
        // Pick a lane (page 2).
        let mut html = mob_names(page(world, npc_oid, 2), world);
        if val == 0 {
            html = html.replace("No1", "");
        } else {
            html = html.replace("No1", &val.to_string());
            t[0] = val;
        }
        set_ticket(world, player, t);
        finalize(world, client_id, npc_oid, html);
    } else if val < 20 {
        // Pick a price tier (page 3).
        if t[0] == 0 {
            return;
        }
        let mut html = page(world, npc_oid, 3)
            .replace("0place", &t[0].to_string())
            .replace("Mob1", &mob_name(world, t[0]));
        if val == 10 {
            html = html.replace("0adena", "");
        } else {
            html = html.replace("0adena", &TICKET_PRICES[(val - 11) as usize].to_string());
            t[1] = val - 10;
        }
        set_ticket(world, player, t);
        finalize(world, client_id, npc_oid, html);
    } else if val == 20 {
        // Confirm page (4).
        if t[0] == 0 || t[1] == 0 {
            return;
        }
        let price = TICKET_PRICES[(t[1] - 1) as usize];
        let html = page(world, npc_oid, 4)
            .replace("0place", &t[0].to_string())
            .replace("Mob1", &mob_name(world, t[0]))
            .replace("0adena", &price.to_string())
            .replace("0tax", "0")
            .replace("0total", &price.to_string());
        finalize(world, client_id, npc_oid, html);
    } else {
        // Execute the purchase.
        if t[0] == 0 || t[1] == 0 {
            return;
        }
        let (lane, price) = (t[0], TICKET_PRICES[(t[1] - 1) as usize]);
        let adena = world
            .objects
            .get_component::<Inventory>(&player)
            .map_or(0, |i| i.adena());
        if adena < price {
            send_sm(world, client_id, sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA);
            return;
        }
        set_ticket(world, player, [0, 0]);
        let race_number = world.monster_race.race_number;
        let mut changes: Vec<ItemChange> = world
            .objects
            .get_component_mut::<Inventory>(&player)
            .map(|inv| inv.remove_item(ADENA_ID, price))
            .unwrap_or_default();
        // Mint the ticket (enchant = race number, custom_type1 = lane,
        // custom_type2 = price / 100).
        if let Some(oids) = super::items::add_inventory_item(world, player, RACE_TICKET_ITEM, 1) {
            let oid = oids[0];
            if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player) {
                inv.set_lotto_fields(oid, lane, race_number, (price / 100) as i32);
                if let Some(it) = inv.by_object_id(oid) {
                    changes.push(ItemChange::Modified(*it));
                }
            }
        }
        // Pool the bet (memory + DB).
        add_bet(&mut world.monster_race.bets, lane, price);
        let pooled = world.monster_race.bets.get(&lane).copied().unwrap_or(price);
        let _ = world.db.send(DbCommand::SaveMdtBet { lane, bet: pooled });

        send_to_client(
            world,
            client_id,
            server_packets::system_message_with(
                sm_ids::ACQUIRED_S1_S2,
                &[
                    SmParam::Int(race_number),
                    SmParam::ItemName(RACE_TICKET_ITEM),
                ],
            ),
        );
        let iu = ew::inventory_update_changes(&world.data, &changes);
        super::helpers::send_inventory_update(world, client_id, player, iu);
        chat0(world, client_id, npc_oid);
    }
}

/// Java `ShowOdds` (page 5): the per-lane odds board (available only once sales
/// have closed).
fn show_odds(world: &mut World, client_id: u32, _player: i32, npc_oid: i32) {
    if !world.cfg.general.allow_race || world.monster_race.state == RaceState::AcceptingBets {
        send_sm(
            world,
            client_id,
            sm_ids::MONSTER_RACE_PAYOUT_INFORMATION_IS_NOT_AVAILABLE_WHILE_TICKETS_ARE_BEING_SOLD,
        );
        chat0(world, client_id, npc_oid);
        return;
    }
    let mut html = mob_names(page(world, npc_oid, 5), world);
    for lane in 1..=LANES as i32 {
        let odd = world
            .monster_race
            .odds
            .get((lane - 1) as usize)
            .copied()
            .unwrap_or(0.0);
        let shown = if odd > 0.0 {
            format!("{odd:.1}")
        } else {
            "&$804;".to_string()
        };
        html = html.replace(&format!("Odd{lane}"), &shown);
    }
    finalize(world, client_id, npc_oid, html);
}

/// Java `ShowInfo` (page 6): the racer names.
fn show_info(world: &World, client_id: u32, npc_oid: i32) {
    if !world.cfg.general.allow_race {
        return;
    }
    let html = mob_names(page(world, npc_oid, 6), world);
    finalize(world, client_id, npc_oid, html);
}

/// Java `ShowTickets` (page 7): the player's past-race tickets as claim links.
fn show_tickets(world: &mut World, client_id: u32, player: i32, npc_oid: i32) {
    if !world.cfg.general.allow_race {
        chat0(world, client_id, npc_oid);
        return;
    }
    let race_number = world.monster_race.race_number;
    let mut rows = String::new();
    if let Some(inv) = world.objects.get_component::<Inventory>(&player) {
        for it in inv.items() {
            // ticket: item 4443, enchant = race id; skip the current race.
            if it.item_id == RACE_TICKET_ITEM && it.enchant_level != race_number {
                rows.push_str(&format!(
                    "<tr><td><a action=\"bypass -h npc_{npc_oid}_ShowTicket {}\">{} Race Number</a></td><td align=right><font color=\"LEVEL\">{}</font> Number</td><td align=right><font color=\"LEVEL\">{}</font> Adena</td></tr>",
                    it.object_id, it.enchant_level, it.custom_type1, it.custom_type2 * 100
                ));
            }
        }
    }
    let html = page(world, npc_oid, 7).replace("%tickets%", &rows);
    finalize(world, client_id, npc_oid, html);
}

/// Java `ShowTicket <oid>` (page 8): one past ticket's result + a cash-out link.
fn show_ticket(world: &mut World, client_id: u32, player: i32, npc_oid: i32, command: &str) {
    if !world.cfg.general.allow_race {
        chat0(world, client_id, npc_oid);
        return;
    }
    let oid = arg(command);
    let Some((race_id, lane, bet)) = ticket_fields(world, player, oid) else {
        chat0(world, client_id, npc_oid);
        return;
    };
    let Some(info) = world
        .monster_race
        .history
        .iter()
        .find(|h| h.race_id == race_id)
    else {
        chat0(world, client_id, npc_oid);
        return;
    };
    let odd = if lane == info.first {
        format!("{:.2}", info.odd_rate)
    } else {
        "0.01".to_string()
    };
    let html = page(world, npc_oid, 8)
        .replace("%raceId%", &race_id.to_string())
        .replace("%lane%", &lane.to_string())
        .replace("%bet%", &bet.to_string())
        .replace("%firstLane%", &info.first.to_string())
        .replace("%odd%", &odd)
        .replace("%ticketObjectId%", &oid.to_string());
    finalize(world, client_id, npc_oid, html);
}

/// Java `CalculateWin <oid>`: destroy the ticket and pay its winnings —
/// `bet * (lane == winner ? oddRate : 0.01)`.
fn calculate_win(world: &mut World, client_id: u32, player: i32, command: &str) {
    if !world.cfg.general.allow_race {
        return;
    }
    let oid = arg(command);
    let Some((race_id, lane, bet)) = ticket_fields(world, player, oid) else {
        return;
    };
    let Some(info) = world
        .monster_race
        .history
        .iter()
        .find(|h| h.race_id == race_id)
        .copied()
    else {
        return;
    };
    // Destroy the ticket, then pay out.
    let removed = world
        .objects
        .get_component_mut::<Inventory>(&player)
        .and_then(|inv| inv.remove_by_object_id(oid, 1));
    if removed.is_none() {
        return;
    }
    let payout = (bet as f64
        * if lane == info.first {
            info.odd_rate
        } else {
            0.01
        }) as i64;
    if payout > 0 {
        super::items::add_inventory_item(world, player, ADENA_ID, payout);
    }
    let mut changes: Vec<ItemChange> = removed.into_iter().collect();
    if let Some(inv) = world.objects.get_component::<Inventory>(&player)
        && let Some(it) = inv.first_of_item(ADENA_ID)
    {
        changes.push(ItemChange::Modified(*it));
    }
    let iu = ew::inventory_update_changes(&world.data, &changes);
    super::helpers::send_inventory_update(world, client_id, player, iu);
}

/// Java `ViewHistory` (page 9): the last seven finished races.
fn view_history(world: &mut World, client_id: u32, npc_oid: i32) {
    if !world.cfg.general.allow_race {
        chat0(world, client_id, npc_oid);
        return;
    }
    let mut rows = String::new();
    for info in world.monster_race.history.iter().rev().take(7) {
        rows.push_str(&format!(
            "<tr><td><font color=\"LEVEL\">{}</font> th</td><td><font color=\"LEVEL\">{}</font> Lane </td><td><font color=\"LEVEL\">{}</font> Lane</td><td align=right><font color=00ffff>{:.2}</font> Times</td></tr>",
            info.race_id, info.first, info.second, info.odd_rate
        ));
    }
    let html = page(world, npc_oid, 9).replace("%infos%", &rows);
    finalize(world, client_id, npc_oid, html);
}

// --- dialog helpers ---

/// The trailing integer of a bypass command (e.g. `BuyTicket 12` → 12), 0 if none.
fn arg(command: &str) -> i32 {
    command
        .rsplit(' ')
        .next()
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0)
}

fn ticket(world: &World, player: i32) -> [i32; 2] {
    world
        .objects
        .get_component::<RaceTicket>(&player)
        .map_or([0, 0], |t| t.0)
}

fn set_ticket(world: &mut World, player: i32, values: [i32; 2]) {
    if let Some(t) = world.objects.get_component_mut::<RaceTicket>(&player) {
        t.0 = values;
    } else {
        world.objects.add_components(&player, RaceTicket(values));
    }
}

/// `(race_id, lane, bet)` of a player's ticket item 4443 by object id.
fn ticket_fields(world: &World, player: i32, oid: i32) -> Option<(i32, i32, i64)> {
    world
        .objects
        .get_component::<Inventory>(&player)
        .and_then(|inv| {
            inv.items()
                .iter()
                .find(|i| i.object_id == oid && i.item_id == RACE_TICKET_ITEM)
                .map(|i| {
                    (
                        i.enchant_level,
                        i.custom_type1,
                        (i.custom_type2 * 100) as i64,
                    )
                })
        })
}

fn mob_name(world: &World, lane: i32) -> String {
    let template = world
        .monster_race
        .monster_templates
        .get((lane - 1).max(0) as usize)
        .copied()
        .unwrap_or(0);
    world
        .data
        .npc_data
        .get(template)
        .map(|t| t.name.clone())
        .unwrap_or_default()
}

/// Replace `Mob1..Mob8` with the current racers' names.
fn mob_names(mut html: String, world: &World) -> String {
    for lane in 1..=LANES as i32 {
        html = html.replace(&format!("Mob{lane}"), &mob_name(world, lane));
    }
    html
}

/// Read a RaceManager html page (`data/html/default/<npcId>-<page>.htm`).
fn page(world: &World, npc_oid: i32, page: i32) -> String {
    let npc_id = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .map_or(0, |n| n.npc_id);
    crate::data::htm_cache::read_htm(format!(
        "{}data/html/default/{npc_id}-{page}.htm",
        world.data.root
    ))
    .unwrap_or_default()
}

fn finalize(world: &World, client_id: u32, npc_oid: i32, html: String) {
    let content = html
        .replace("1race", &world.monster_race.race_number.to_string())
        .replace("%objectId%", &npc_oid.to_string());
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::npc_html_message(npc_oid, &content));
        cs.send(server_packets::action_failed());
    }
}

/// Java `super.onBypassFeedback(player, "Chat 0")` — fall back to the default page.
fn chat0(world: &mut World, client_id: u32, npc_oid: i32) {
    crate::game_loop::target::show_chat_window(world, client_id, npc_oid, 0);
}
