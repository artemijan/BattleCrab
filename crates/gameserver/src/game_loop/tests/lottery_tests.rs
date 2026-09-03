//! Lucky Lottery (G26.5) — the round lifecycle + persistence, the two-phase
//! prize draw + tier split, ticket purchase, and prize claim through the Loto
//! NPC dialog.

use super::*;
use crate::game_loop::character::inventory;

use crate::game_loop::activities::lottery;
use crate::game_loop::character::inventory::adena;
use crate::model::inventory::Inventory;
use crate::model::lottery::{DrawnRound, LotteryRow};
use crate::scheduler::ScheduledTask;

/// A test world with the lottery enabled + the dist economics (dist ships the
/// feature off).
fn enabled_world() -> (World, db::CmdRx) {
    let (mut world, _tx, db_rx, _link) = test_world();
    let g = &mut world.cfg.general;
    g.allow_lottery = true;
    g.alt_lottery_prize = 50000;
    g.alt_lottery_ticket_price = 2000;
    g.alt_lottery_5_number_rate = 0.6;
    g.alt_lottery_4_number_rate = 0.2;
    g.alt_lottery_3_number_rate = 0.2;
    g.alt_lottery_2and1_number_prize = 200;
    (world, db_rx)
}

#[test]
fn fresh_boot_opens_round_one() {
    let (mut world, mut db_rx) = enabled_world();

    lottery::on_loaded(&mut world, None, vec![]);

    assert_eq!(world.lottery.number, 1);
    assert_eq!(world.lottery.prize, 50000);
    assert!(world.lottery.selling && world.lottery.started);
    let pending = world.scheduler.pending_tasks_for_test();
    assert!(pending.contains(&ScheduledTask::LotteryFinish));
    assert!(pending.contains(&ScheduledTask::LotteryStopSelling));
    // The new round was persisted.
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, db::DbCommand::StoreLottery { idnr: 1, .. }))
    );
}

#[test]
fn a_disabled_lottery_stays_inert() {
    let (mut world, _tx, _db, _l) = test_world(); // AllowLottery defaults false
    lottery::on_loaded(&mut world, None, vec![]);
    assert_eq!(world.lottery.number, 0);
    assert!(!world.lottery.started && !world.lottery.selling);
    assert!(world.scheduler.pending_tasks_for_test().is_empty());
}

#[test]
fn a_finished_row_carries_the_pot_into_the_next_round() {
    let (mut world, _db) = enabled_world();

    lottery::on_loaded(
        &mut world,
        Some(LotteryRow {
            idnr: 7,
            prize: 999,
            newprize: 123_456,
            enddate: 0,
            finished: true,
        }),
        vec![],
    );

    assert_eq!(world.lottery.number, 8); // idnr + 1
    assert_eq!(world.lottery.prize, 123_456); // newprize carried forward
    assert!(world.lottery.started);
}

#[test]
fn a_live_round_resumes_with_its_draw_armed() {
    let (mut world, _db) = enabled_world();
    let far = commons::util::now_millis() + 7 * 24 * 3600 * 1000; // a week out

    lottery::on_loaded(
        &mut world,
        Some(LotteryRow {
            idnr: 3,
            prize: 777,
            newprize: 777,
            enddate: far,
            finished: false,
        }),
        vec![],
    );

    assert_eq!(world.lottery.number, 3);
    assert_eq!(world.lottery.prize, 777);
    assert!(world.lottery.started && world.lottery.selling);
    let pending = world.scheduler.pending_tasks_for_test();
    assert!(pending.contains(&ScheduledTask::LotteryFinish));
    assert!(pending.contains(&ScheduledTask::LotteryStopSelling));
}

#[test]
fn finish_with_no_tickets_rolls_over_and_carries_the_whole_pot() {
    let (mut world, mut db_rx) = enabled_world();
    lottery::on_loaded(&mut world, None, vec![]); // round 1, pot 50000
    drain_db(&mut db_rx);

    // Phase 1: rolls the numbers + requests the tickets.
    lottery::finish_begin(&mut world);
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, db::DbCommand::LoadLotteryTickets { round: 1 }))
    );
    // Phase 2: no tickets arrive → whole pot carries.
    lottery::finish_complete(&mut world, 1, vec![]);

    assert_eq!(world.lottery.number, 2); // number++
    assert_eq!(world.lottery.prize, 50000); // no winners → whole pot carries
    assert!(!world.lottery.started);
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(
        c,
        db::DbCommand::FinishLottery {
            idnr: 1,
            newprize: 50000,
            ..
        }
    )));
    assert!(
        world
            .scheduler
            .pending_tasks_for_test()
            .contains(&ScheduledTask::LotteryStart)
    );
}

#[test]
fn encode_decode_round_trips_the_picks() {
    // Numbers spanning both words (1..16 → enchant, 17..20 → type2).
    let (enchant, type2) = lottery::encode(&[1, 5, 16, 17, 20]);
    let mut got = lottery::decode(enchant, type2);
    got.sort();
    assert_eq!(got, [1, 5, 16, 17, 20]);
}

#[test]
fn the_draw_splits_the_pot_by_tier() {
    let (mut world, mut db_rx) = enabled_world();
    lottery::on_loaded(&mut world, None, vec![]); // round 1, pot 50000
    drain_db(&mut db_rx);

    // Rig the draw to numbers [1,2,3,4,5] (enchant = 0b11111 = 31).
    lottery::finish_begin(&mut world);
    world.lottery.draw_enchant = 31;
    world.lottery.draw_type2 = 0;
    drain_db(&mut db_rx);

    // Two offline tickets: one perfect (5 match), one 3-match (numbers 1,2,3,8,9
    // = bits 0,1,2,7,8 = 391 → shares 1,2,3 with the draw).
    let perfect = (1001, 31, 0);
    let three = (1002, 391, 0);
    lottery::finish_complete(&mut world, 1, vec![perfect, three]);

    // prize1 (5-match) = (50000 - prize4) * 0.6 / 1 winner; prize4 = 0 here.
    let cmds = drain_db(&mut db_rx);
    let fin = cmds.iter().find_map(|c| match c {
        db::DbCommand::FinishLottery {
            idnr: 1,
            prize1,
            prize3,
            ..
        } => Some((*prize1, *prize3)),
        _ => None,
    });
    assert_eq!(fin, Some((30000, 10000))); // 50000*0.6, 50000*0.2
    // The result is cached for claims.
    assert!(world.lottery.drawn.contains_key(&1));
}

#[test]
fn buying_a_ticket_charges_adena_grows_the_pot_and_mints_the_ticket() {
    let (mut world, mut db_rx) = enabled_world();
    world.id_pool = 0x6000_0000..0x6000_0100;
    insert_adena_template(&mut world);
    lottery::on_loaded(&mut world, None, vec![]); // round 1, selling open
    drain_db(&mut db_rx);

    add_test_npc(&mut world, 500, 30991, "Folk", 70, 0, 0, 0);
    ingame_player(&mut world, 1, 100, 0, 0, 0);
    inventory::add_inventory_item(&mut world, 100, 57, 10_000);
    // Five picked numbers.
    world
        .objects
        .add_components(&100, model::components::LotoPicks([1, 2, 3, 4, 5]));

    lottery::loto_bypass(&mut world, 1, 100, 500, "Loto 22");

    let inv = world.objects.get_component::<Inventory>(&100).unwrap();
    let ticket = inv
        .items()
        .iter()
        .find(|i| i.item_id == 4442)
        .expect("ticket");
    assert_eq!(ticket.custom_type1, 1); // this round
    assert_eq!(ticket.enchant_level, 31); // 1|2|4|8|16
    assert_eq!(adena(&world, 100), 8_000); // 10000 - 2000
    assert_eq!(world.lottery.prize, 52_000); // pot grew by the ticket price
}

#[test]
fn selling_closed_refuses_the_purchase() {
    let (mut world, mut db_rx) = enabled_world();
    world.id_pool = 0x6000_0000..0x6000_0100;
    insert_adena_template(&mut world);
    lottery::on_loaded(&mut world, None, vec![]);
    world.lottery.selling = false; // sales suspended
    drain_db(&mut db_rx);
    add_test_npc(&mut world, 500, 30991, "Folk", 70, 0, 0, 0);
    ingame_player(&mut world, 1, 100, 0, 0, 0);
    inventory::add_inventory_item(&mut world, 100, 57, 10_000);
    world
        .objects
        .add_components(&100, model::components::LotoPicks([1, 2, 3, 4, 5]));

    lottery::loto_bypass(&mut world, 1, 100, 500, "Loto 22");

    // No ticket, no charge.
    assert!(
        world
            .objects
            .get_component::<Inventory>(&100)
            .unwrap()
            .items()
            .iter()
            .all(|i| i.item_id != 4442)
    );
    assert_eq!(adena(&world, 100), 10_000);
}

#[test]
fn claiming_a_winning_ticket_pays_out_and_consumes_it() {
    let (mut world, mut db_rx) = enabled_world();
    world.id_pool = 0x6000_0000..0x6000_0100;
    insert_adena_template(&mut world);
    // Round 1 drew numbers enchant=31 with a 30000 first prize; round 2 is live.
    lottery::on_loaded(
        &mut world,
        Some(LotteryRow {
            idnr: 1,
            prize: 50_000,
            newprize: 20_000,
            enddate: 0,
            finished: true,
        }),
        vec![(
            1,
            DrawnRound {
                number1: 31,
                number2: 0,
                prize1: 30_000,
                prize2: 0,
                prize3: 0,
            },
        )],
    );
    assert_eq!(world.lottery.number, 2);
    drain_db(&mut db_rx);

    add_test_npc(&mut world, 500, 30991, "Folk", 70, 0, 0, 0);
    ingame_player(&mut world, 1, 100, 0, 0, 0);
    // A round-1 winning ticket (5-match: enchant 31).
    let oid = inventory::add_inventory_item(&mut world, 100, 4442, 1).unwrap()[0];
    world
        .objects
        .get_component_mut::<Inventory>(&100)
        .unwrap()
        .set_lotto_fields(oid, 1, 31, 0);

    lottery::loto_bypass(&mut world, 1, 100, 500, &format!("Loto {oid}"));

    // Ticket consumed, 30000 adena paid.
    assert!(
        world
            .objects
            .get_component::<Inventory>(&100)
            .unwrap()
            .items()
            .iter()
            .all(|i| i.object_id != oid)
    );
    assert_eq!(adena(&world, 100), 30_000);
}
