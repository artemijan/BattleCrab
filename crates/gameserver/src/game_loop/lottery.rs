//! Weekly Lucky Lottery (G26.5) — port of the Java `Lottery` singleton
//! (`startLottery`/`stopSellingTickets`/`finishLottery`) plus the `Loto` NPC
//! dialog. The round lifecycle + `lottery`-table persistence, ticket purchase,
//! the two-phase draw, and prize claim.

use commons::util::rnd;
use tracing::info;

use super::helpers::send_sm_bare_to_client as send_sm;
use crate::db::DbCommand;
use crate::enums::ChatType;
use crate::game_loop::helpers::npc_id_of;
use crate::model::components::LotoPicks;
use crate::model::inventory::{Inventory, ItemChange};
use crate::model::lottery::LotteryRow;
use crate::network::enter_world as ew;
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::World;

const MINUTE_MILLIS: i64 = 60_000;
const TICKS_PER_SECOND: u64 = 10;
/// Sunday, in the `Mon=0..Sun=6` weekday space `siege::next_siege_millis` uses.
const DRAW_WEEKDAY: u32 = 6;
/// The draw fires at 19:00 (Java `Calendar.HOUR_OF_DAY = 19`).
const DRAW_HOUR: u32 = 19;
/// The Lottery Ticket item (Java 4442).
const TICKET_ITEM: i32 = 4442;
const ADENA_ID: i32 = 57;

/// Port of the `Lottery` constructor + `startLottery`'s load branch, driven by
/// the boot `DbEvent::LotteryLoaded`. Inert unless `AllowLottery`.
pub(crate) fn on_loaded(
    world: &mut World,
    row: Option<LotteryRow>,
    draws: Vec<(i32, crate::model::lottery::DrawnRound)>,
) {
    if !world.cfg.general.allow_lottery {
        return;
    }
    world.lottery.drawn = draws.into_iter().collect();
    let now = commons::util::now_millis();
    world.lottery.number = 1;
    world.lottery.prize = world.cfg.general.alt_lottery_prize;
    world.lottery.enddate = now;

    if let Some(row) = row {
        world.lottery.number = row.idnr;
        if row.finished {
            // The last round already drew: carry the pot into a fresh round.
            world.lottery.number += 1;
            world.lottery.prize = row.newprize;
        } else {
            // A round was live at shutdown: resume it.
            world.lottery.prize = row.prize;
            world.lottery.enddate = row.enddate;
            if row.enddate <= now + 2 * MINUTE_MILLIS {
                finish_begin(world);
                return;
            }
            world.lottery.started = true;
            schedule_at(world, row.enddate, ScheduledTask::LotteryFinish);
            if row.enddate > now + 12 * MINUTE_MILLIS {
                world.lottery.selling = true;
                schedule_at(
                    world,
                    row.enddate - 10 * MINUTE_MILLIS,
                    ScheduledTask::LotteryStopSelling,
                );
            }
            return;
        }
    }
    open_round(world);
}

/// Port of `startLottery`'s "create a new round" tail (also the scheduled
/// `LotteryStart` restart): open sales, set the next Sunday-19:00 draw, insert
/// the row.
pub(crate) fn open_round(world: &mut World) {
    if !world.cfg.general.allow_lottery {
        return;
    }
    let now = commons::util::now_millis();
    world.lottery.selling = true;
    world.lottery.started = true;
    announce(
        world,
        &format!(
            "Lottery tickets are now available for Lucky Lottery #{}.",
            world.lottery.number
        ),
    );

    // Next Sunday 19:00. Java rolls to *next* week when today is already Sunday;
    // `next_siege_millis` gives the next Sunday strictly after now (in UTC, like
    // the siege schedule) — the boot-on-Sunday edge aside, the same slot.
    let enddate = crate::game_loop::siege::next_siege_millis(now, DRAW_WEEKDAY, DRAW_HOUR);
    world.lottery.enddate = enddate;
    schedule_at(
        world,
        enddate - 10 * MINUTE_MILLIS,
        ScheduledTask::LotteryStopSelling,
    );
    schedule_at(world, enddate, ScheduledTask::LotteryFinish);

    info!(
        "Lottery: opened round #{} (draws at {} epoch-ms, pot {}).",
        world.lottery.number, enddate, world.lottery.prize
    );
    let _ = world.db.send(DbCommand::StoreLottery {
        idnr: world.lottery.number,
        enddate,
        prize: world.lottery.prize,
    });
}

/// Port of `stopSellingTickets`.
pub(crate) fn stop_selling(world: &mut World) {
    if !world.lottery.started {
        return;
    }
    world.lottery.selling = false;
    announce(
        world,
        "Lottery ticket sales have been temporarily suspended.",
    );
}

/// Java `finishLottery`, phase 1 (`LotteryFinish` task): roll the five winning
/// numbers, stash them, and request this round's sold tickets from the DB. The
/// draw completes in [`finish_complete`] once they arrive.
pub(crate) fn finish_begin(world: &mut World) {
    if !world.cfg.general.allow_lottery {
        return;
    }
    world.lottery.selling = false;

    // Five distinct numbers 1..=20 → the two-word bitmask (Java `finishLottery`).
    let mut nums = [0i32; 5];
    for i in 0..5 {
        loop {
            let n = rnd::get_range(1, 20);
            if !nums[..i].contains(&n) {
                nums[i] = n;
                break;
            }
        }
    }
    let (enchant, type2) = encode(&nums);
    world.lottery.draw_enchant = enchant;
    world.lottery.draw_type2 = type2;

    let _ = world.db.send(DbCommand::LoadLotteryTickets {
        round: world.lottery.number,
    });
}

/// Java `finishLottery`, phase 2 (the `LotteryTicketsLoaded` reply): merge the
/// offline `db_rows` with a scan of every online inventory (deduped by object
/// id), tally the match tiers, split the pot, persist, and roll over.
pub(crate) fn finish_complete(world: &mut World, round: i32, db_rows: Vec<(i32, i32, i32)>) {
    // Ignore a stale reply (only the round we're mid-drawing counts).
    if !world.cfg.general.allow_lottery || round != world.lottery.number {
        return;
    }
    let (d_enchant, d_type2) = (world.lottery.draw_enchant, world.lottery.draw_type2);
    let prize = world.lottery.prize;

    // Every sold ticket of this round as `(object_id, enchant, type2)`: the
    // online inventories first, then the offline DB rows minus any already seen
    // online (an online ticket may already have been flushed to the DB).
    let mut seen: std::collections::HashSet<i32> = std::collections::HashSet::new();
    let mut tickets: Vec<(i32, i32)> = Vec::new();
    let online: Vec<i32> = world.in_game_player_oids().collect();
    for player in online {
        if let Some(inv) = world.objects.get_component::<Inventory>(&player) {
            for it in inv.items() {
                if it.item_id == TICKET_ITEM
                    && it.custom_type1 == round
                    && seen.insert(it.object_id)
                {
                    tickets.push((it.enchant_level, it.custom_type2));
                }
            }
        }
    }
    for (oid, enchant, type2) in db_rows {
        if seen.insert(oid) {
            tickets.push((enchant, type2));
        }
    }

    // Tally the tiers by match count (Java's `count1..4`).
    let (mut count1, mut count2, mut count3, mut count4) = (0i64, 0i64, 0i64, 0i64);
    for (t_enchant, t_type2) in &tickets {
        match match_count(*t_enchant, *t_type2, d_enchant, d_type2) {
            5 => count1 += 1,
            4 => count2 += 1,
            3 => count3 += 1,
            1 | 2 => count4 += 1,
            _ => {}
        }
    }

    // Split the pot (Java `finishLottery`'s arithmetic, verbatim).
    let g = &world.cfg.general;
    let prize4 = count4 * g.alt_lottery_2and1_number_prize;
    let prize1 = if count1 > 0 {
        (((prize - prize4) as f64 * g.alt_lottery_5_number_rate) as i64) / count1
    } else {
        0
    };
    let prize2 = if count2 > 0 {
        (((prize - prize4) as f64 * g.alt_lottery_4_number_rate) as i64) / count2
    } else {
        0
    };
    let prize3 = if count3 > 0 {
        (((prize - prize4) as f64 * g.alt_lottery_3_number_rate) as i64) / count3
    } else {
        0
    };
    let newprize = prize - (prize1 + prize2 + prize3 + prize4);

    if count1 > 0 {
        announce(
            world,
            &format!(
                "The prize amount for the winner of Lottery #{round} is {prize} adena. We have {count1} first-prize winner(s)."
            ),
        );
    } else {
        announce(
            world,
            &format!(
                "The prize amount for Lucky Lottery #{round} is {prize} adena. There was no first-prize winner; the jackpot is added to the next drawing."
            ),
        );
    }

    let _ = world.db.send(DbCommand::FinishLottery {
        idnr: round,
        prize,
        newprize,
        number1: d_enchant,
        number2: d_type2,
        prize1,
        prize2,
        prize3,
    });
    info!(
        "Lottery: round #{round} drawn ({} tickets; 1st={count1} 2nd={count2} 3rd={count3} 4th={count4}); pot {newprize} carries.",
        tickets.len()
    );

    // Cache the result so tickets of this round can be claimed synchronously.
    world.lottery.drawn.insert(
        round,
        crate::model::lottery::DrawnRound {
            number1: d_enchant,
            number2: d_type2,
            prize1,
            prize2,
            prize3,
        },
    );

    world.lottery.started = false;
    world.lottery.number = round + 1;
    world.lottery.prize = newprize;
    // Java schedules a fresh `startLottery` one minute after the draw.
    schedule_in(world, MINUTE_MILLIS, ScheduledTask::LotteryStart);
}

/// Java `Lottery.checkTicket(id, enchant, type2)`: the `(tier, prize)` a ticket
/// of round `id` wins — `(0, 0)` if it lost or the round hasn't drawn. Tiers:
/// 1 = 5 matches (prize1), 2 = 4 (prize2), 3 = 3 (prize3), 4 = 1–2 (flat).
fn check_ticket(world: &World, id: i32, enchant: i32, type2: i32) -> (i32, i64) {
    let Some(d) = world.lottery.drawn.get(&id) else {
        return (0, 0);
    };
    match match_count(enchant, type2, d.number1, d.number2) {
        5 => (1, d.prize1),
        4 => (2, d.prize2),
        3 => (3, d.prize3),
        1 | 2 => (4, world.cfg.general.alt_lottery_2and1_number_prize),
        _ => (0, 0),
    }
}

/// Grow the current round's pot after a ticket sale (Java `increasePrize`).
fn increase_prize(world: &mut World, count: i64) {
    world.lottery.prize += count;
    let _ = world.db.send(DbCommand::IncreaseLotteryPrize {
        idnr: world.lottery.number,
        prize: world.lottery.prize,
    });
}

/// The two-word bitmask for five picked numbers (Java: `n < 17 → enchant |=
/// 1<<(n-1)`, else `type2 |= 1<<(n-17)`).
pub(crate) fn encode(nums: &[i32; 5]) -> (i32, i32) {
    let (mut enchant, mut type2) = (0i32, 0i32);
    for &n in nums {
        if n == 0 {
            continue;
        }
        if n < 17 {
            enchant |= 1 << (n - 1);
        } else {
            type2 |= 1 << (n - 17);
        }
    }
    (enchant, type2)
}

/// Reverse [`encode`] into up to five numbers (Java `decodeNumbers`).
pub(crate) fn decode(enchant: i32, type2: i32) -> [i32; 5] {
    let mut res = [0i32; 5];
    let mut id = 0;
    for bit in 0..16 {
        if id < 5 && (enchant & (1 << bit)) != 0 {
            res[id] = bit + 1; // numbers 1..=16
            id += 1;
        }
    }
    for bit in 0..4 {
        if id < 5 && (type2 & (1 << bit)) != 0 {
            res[id] = bit + 17; // numbers 17..=20
            id += 1;
        }
    }
    res
}

/// How many of a ticket's numbers match the draw (Java's 16-bit popcount of the
/// AND of each word).
fn match_count(t_enchant: i32, t_type2: i32, d_enchant: i32, d_type2: i32) -> i32 {
    ((t_enchant & d_enchant).count_ones() + (t_type2 & d_type2).count_ones()) as i32
}

fn schedule_at(world: &mut World, at_millis: i64, task: ScheduledTask) {
    let now = commons::util::now_millis();
    schedule_in(world, at_millis - now, task);
}

fn schedule_in(world: &mut World, delay_millis: i64, task: ScheduledTask) {
    let delay_ticks = (delay_millis.max(0) / 1000) as u64 * TICKS_PER_SECOND;
    world.scheduler.schedule(world.tick + delay_ticks, task);
}

/// Java `Broadcast.toAllOnlinePlayers` — an announcement line to every player.
fn announce(world: &World, text: &str) {
    let pkt = server_packets::creature_say(0, ChatType::Announcement, "", text, None);
    for cs in world.clients.values() {
        if let ClientSession::InGame(_) = cs {
            cs.send(pkt.clone());
        }
    }
}

// ---------------------------------------------------------------------------
// The Loto NPC dialog (Java `handlers/bypasshandlers/Loto`)
// ---------------------------------------------------------------------------

/// `bypass -h Loto <value>` from a Lottery Ticket Seller (NPCs 30990–30994).
pub(crate) fn loto_bypass(
    world: &mut World,
    client_id: u32,
    player: i32,
    npc_oid: i32,
    command: &str,
) {
    let value = command
        .strip_prefix("Loto")
        .and_then(|s| s.trim().parse::<i32>().ok())
        .unwrap_or(0);
    if value == 0 {
        set_picks(world, player, [0; 5]); // Java resets the pick buffer.
    }
    show_loto_window(world, client_id, player, npc_oid, value);
}

/// Java `Loto.showLotoWindow`. `value`: 0 buy window; 1–20 toggle a number; 21
/// the second buy window; 22 confirm-buy; 23 jackpot; 24 winning-numbers/claim
/// list; 25 instructions; >25 claim a ticket by its item object id.
fn show_loto_window(world: &mut World, client_id: u32, player: i32, npc_oid: i32, value: i32) {
    let Some(npc_id) = npc_id_of(world, npc_oid) else {
        return;
    };

    let html: String = match value {
        0 => page(world, npc_id, 1),
        1..=21 => {
            if !sale_open(world, client_id) {
                return;
            }
            let mut content = page(world, npc_id, 5);
            let mut p = picks(world, player);
            // Toggle: unset if already picked, else fill the first empty slot.
            let mut count = 0;
            let mut found = false;
            for slot in &mut p {
                if *slot == value {
                    *slot = 0;
                    found = true;
                } else if *slot > 0 {
                    count += 1;
                }
            }
            if count < 5
                && !found
                && value <= 20
                && let Some(slot) = p.iter_mut().find(|s| **s == 0)
            {
                *slot = value;
            }
            set_picks(world, player, p);
            // Highlight the pushed buttons.
            count = 0;
            for &n in &p {
                if n > 0 {
                    count += 1;
                    let b = format!("{n:02}");
                    content = content.replace(
                        &format!("fore=\"L2UI.lottoNum{b}\" back=\"L2UI.lottoNum{b}a_check\""),
                        &format!("fore=\"L2UI.lottoNum{b}a_check\" back=\"L2UI.lottoNum{b}\""),
                    );
                }
            }
            if count == 5 {
                content = content.replace(
                    "0\">Return",
                    "22\">Your lucky numbers have been selected above.",
                );
            }
            content
        }
        22 => match buy_ticket(world, client_id, player) {
            Some(()) => page(world, npc_id, 6),
            None => return,
        },
        23 => page(world, npc_id, 3),
        24 => {
            let mut content = page(world, npc_id, 4);
            content = content.replace("%result%", &claim_list(world, player, npc_oid));
            content
        }
        25 => page(world, npc_id, 2),
        v if v > 25 => {
            claim_ticket(world, client_id, player, v);
            return; // Java shows no window on a direct claim.
        }
        _ => return,
    };

    send_html(world, client_id, npc_oid, html);
}

/// Both sale gates (Java's two `sendPacket` early-returns).
fn sale_open(world: &World, client_id: u32) -> bool {
    if !world.lottery.started {
        send_sm(
            world,
            client_id,
            sm_ids::LOTTERY_TICKETS_ARE_NOT_CURRENTLY_BEING_SOLD,
        );
        return false;
    }
    if !world.lottery.selling {
        send_sm(
            world,
            client_id,
            sm_ids::TICKETS_FOR_THE_CURRENT_LOTTERY_ARE_NO_LONGER_AVAILABLE,
        );
        return false;
    }
    true
}

/// Java `showLotoWindow`'s `value == 22`: charge, grow the pot, mint the ticket.
/// `Some(())` on success (show page 6), `None` on any refusal (show nothing).
fn buy_ticket(world: &mut World, client_id: u32, player: i32) -> Option<()> {
    if !sale_open(world, client_id) {
        return None;
    }
    let p = picks(world, player);
    if p.contains(&0) {
        return None; // Java returns when a slot is still empty.
    }
    let (enchant, type2) = encode(&p);
    let round = world.lottery.number;
    let price = world.cfg.general.alt_lottery_ticket_price;

    let adena = world
        .objects
        .get_component::<Inventory>(&player)
        .map_or(0, |i| i.adena());
    if adena < price {
        send_sm(world, client_id, sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA);
        return None;
    }

    let mut changes: Vec<ItemChange> = world
        .objects
        .get_component_mut::<Inventory>(&player)?
        .remove_item(ADENA_ID, price);
    increase_prize(world, price);

    let ticket_oid = *super::items::add_inventory_item(world, player, TICKET_ITEM, 1)?
        .first()
        .unwrap();
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player) {
        inv.set_lotto_fields(ticket_oid, round, enchant, type2);
        if let Some(it) = inv.by_object_id(ticket_oid) {
            changes.push(ItemChange::Modified(*it));
        }
    }

    send_pkt(
        world,
        client_id,
        server_packets::system_message_with(
            sm_ids::YOU_HAVE_EARNED_S1,
            &[SmParam::ItemName(TICKET_ITEM)],
        ),
    );
    let iu = ew::inventory_update_changes(&world.data, &changes);
    super::helpers::send_inventory_update(world, client_id, player, iu);
    Some(())
}

/// Java `value == 24`: the `%result%` list of the player's past-round tickets,
/// each a claim link.
fn claim_list(world: &World, player: i32, npc_oid: i32) -> String {
    let round = world.lottery.number;
    let tickets: Vec<(i32, i32, i32, i32)> = world
        .objects
        .get_component::<Inventory>(&player)
        .map(|inv| {
            inv.items()
                .iter()
                .filter(|i| i.item_id == TICKET_ITEM && i.custom_type1 < round)
                .map(|i| (i.object_id, i.custom_type1, i.enchant_level, i.custom_type2))
                .collect()
        })
        .unwrap_or_default();

    let mut msg = String::new();
    for (oid, tround, enchant, type2) in tickets {
        msg.push_str(&format!(
            "<a action=\"bypass -h npc_{npc_oid}_Loto {oid}\">{tround} Event Number "
        ));
        for n in decode(enchant, type2) {
            msg.push_str(&format!("{n} "));
        }
        let (tier, prize) = check_ticket(world, tround, enchant, type2);
        if tier > 0 {
            msg.push_str(match tier {
                1 => "- 1st Prize",
                2 => "- 2nd Prize",
                3 => "- 3th Prize",
                _ => "- 4th Prize",
            });
            msg.push_str(&format!(" {prize}a."));
        }
        msg.push_str("</a><br>");
    }
    if msg.is_empty() {
        msg.push_str("There has been no winning lottery ticket.<br>");
    }
    msg
}

/// Java `value > 25`: cash in a past-round ticket by its item object id.
fn claim_ticket(world: &mut World, client_id: u32, player: i32, item_oid: i32) {
    let round = world.lottery.number;
    let ticket = world
        .objects
        .get_component::<Inventory>(&player)
        .and_then(|inv| {
            inv.items()
                .iter()
                .find(|i| {
                    i.object_id == item_oid && i.item_id == TICKET_ITEM && i.custom_type1 < round
                })
                .cloned()
        });
    let Some(ticket) = ticket else {
        return;
    };
    let (_, prize) = check_ticket(
        world,
        ticket.custom_type1,
        ticket.enchant_level,
        ticket.custom_type2,
    );

    send_pkt(
        world,
        client_id,
        server_packets::system_message_with(
            sm_ids::S1_HAS_DISAPPEARED,
            &[SmParam::ItemName(TICKET_ITEM)],
        ),
    );

    let mut changes: Vec<ItemChange> = world
        .objects
        .get_component_mut::<Inventory>(&player)
        .and_then(|inv| inv.remove_by_object_id(item_oid, 1))
        .into_iter()
        .collect();
    if prize > 0
        && let Some(oids) = super::items::add_inventory_item(world, player, ADENA_ID, prize)
        && let Some(inv) = world.objects.get_component::<Inventory>(&player)
        && let Some(it) = inv.by_object_id(oids[0])
    {
        changes.push(ItemChange::Modified(*it));
    }
    let iu = ew::inventory_update_changes(&world.data, &changes);
    super::helpers::send_inventory_update(world, client_id, player, iu);
}

fn picks(world: &World, player: i32) -> [i32; 5] {
    world
        .objects
        .get_component::<LotoPicks>(&player)
        .map_or([0; 5], |p| p.0)
}

fn set_picks(world: &mut World, player: i32, values: [i32; 5]) {
    if let Some(p) = world.objects.get_component_mut::<LotoPicks>(&player) {
        p.0 = values;
    } else {
        world.objects.add_components(&player, LotoPicks(values));
    }
}

/// Read a Lottery Seller html page (`data/html/default/<npcId>-<page>.htm`) and
/// apply the shared `%…%` replaces (Java's tail of `showLotoWindow`).
fn page(world: &World, npc_id: i32, page: i32) -> String {
    let g = &world.cfg.general;
    let g_enddate = world.lottery.enddate;
    crate::data::htm_cache::read_htm(format!(
        "{}data/html/default/{npc_id}-{page}.htm",
        world.data.root
    ))
    .unwrap_or_default()
    .replace("%race%", &world.lottery.number.to_string())
    .replace("%adena%", &world.lottery.prize.to_string())
    .replace("%ticket_price%", &g.alt_lottery_ticket_price.to_string())
    .replace(
        "%prize5%",
        &format!("{:.0}", g.alt_lottery_5_number_rate * 100.0),
    )
    .replace(
        "%prize4%",
        &format!("{:.0}", g.alt_lottery_4_number_rate * 100.0),
    )
    .replace(
        "%prize3%",
        &format!("{:.0}", g.alt_lottery_3_number_rate * 100.0),
    )
    .replace("%prize2%", &g.alt_lottery_2and1_number_prize.to_string())
    // Java `DateFormat.getDateInstance().format(getEndDate())` —
    // `commons::util::format_date` gives the same calendar date.
    .replace("%enddate%", &commons::util::format_date(g_enddate))
}

fn send_html(world: &World, client_id: u32, npc_oid: i32, content: String) {
    let content = content.replace("%objectId%", &npc_oid.to_string());
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::npc_html_message(npc_oid, &content));
        cs.send(server_packets::action_failed());
    }
}

fn send_pkt(world: &World, client_id: u32, pkt: Vec<u8>) {
    crate::game_loop::helpers::send_to_client(world, client_id, pkt);
}
