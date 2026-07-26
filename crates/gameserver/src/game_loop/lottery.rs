//! Weekly Lucky Lottery round engine (G26.5) — port of the Java `Lottery`
//! singleton's lifecycle (`startLottery` / `stopSellingTickets` /
//! `finishLottery`). This slice covers the round lifecycle + `lottery`-table
//! persistence; ticket purchase, the prize draw, and the Loto NPC dialog are
//! slice 2 (see `docs/PLAN_G26_5_LOTTERY_RACE.md`).

use tracing::info;

use crate::db::DbCommand;
use crate::enums::ChatType;
use crate::model::lottery::LotteryRow;
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::World;

const MINUTE_MILLIS: i64 = 60_000;
const TICKS_PER_SECOND: u64 = 10;
/// Sunday, in the `Mon=0..Sun=6` weekday space `siege::next_siege_millis` uses.
const DRAW_WEEKDAY: u32 = 6;
/// The draw fires at 19:00 (Java `Calendar.HOUR_OF_DAY = 19`).
const DRAW_HOUR: u32 = 19;

/// Port of the `Lottery` constructor + `startLottery`'s load branch, driven by
/// the boot `DbEvent::LotteryLoaded`. Inert unless `AllowLottery`.
pub(crate) fn on_loaded(world: &mut World, row: Option<LotteryRow>) {
    if !world.cfg.general.allow_lottery {
        return;
    }
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
                finish_lottery(world);
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

/// Port of `finishLottery`. This slice rolls the round over **without a draw**
/// (no tickets are sold until slice 2), so the whole pot carries forward.
pub(crate) fn finish_lottery(world: &mut World) {
    if !world.cfg.general.allow_lottery {
        return;
    }
    let number = world.lottery.number;
    let prize = world.lottery.prize;
    // TODO(G26.5) slice 2: roll 5 winning numbers, match every sold ticket
    //   (item 4442, custom_type1 == this round), compute the three prize tiers +
    //   the flat 2-and-1 prize, and set `newprize = prize - paid`. With no
    //   tickets yet nothing is paid and the whole pot carries over.
    let newprize = prize;
    let _ = world.db.send(DbCommand::FinishLottery {
        idnr: number,
        prize,
        newprize,
        number1: 0,
        number2: 0,
        prize1: 0,
        prize2: 0,
        prize3: 0,
    });
    world.lottery.started = false;
    world.lottery.selling = false;
    world.lottery.number = number + 1;
    world.lottery.prize = newprize;
    // Java schedules a fresh `startLottery` one minute after the draw.
    schedule_in(world, MINUTE_MILLIS, ScheduledTask::LotteryStart);
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
