//! Grand Olympiad (G25) — noble registration into the match queues.
//! Java `model/olympiad/OlympiadManager` (`registerNoble` / `unRegisterNoble`).
//!
//! Slice 1: a qualifying character joins or leaves the class-based / non-class
//! waiting lists, with the eligibility and timing gates. Match-making, the
//! stadiums and hero calculation are later slices.

use crate::db::{DbCommand, OlympiadNobleRow};
use crate::model::olympiad::{CompetitionType, NobleStats, OlympiadState, REG_CLOSE_BEFORE_END_MS};
use crate::model::Player;
use crate::network::server_packets::{self as sp, sm_ids, SmParam};
use crate::world::World;

/// Apply the boot-loaded `olympiad_data` + `olympiad_nobles` (Java
/// `Olympiad.load` / `loadNoblesRank`) into the live state.
pub(crate) fn apply_loaded(
    world: &mut World,
    current_cycle: i32,
    period: i32,
    olympiad_end: i64,
    validation_end: i64,
    next_weekly_change: i64,
    nobles: Vec<OlympiadNobleRow>,
) {
    let oly = &mut world.olympiad;
    oly.current_cycle = current_cycle;
    oly.period = period;
    oly.olympiad_end = olympiad_end;
    oly.validation_end = validation_end;
    oly.next_weekly_change = next_weekly_change;
    oly.nobles = nobles
        .into_iter()
        .map(|n| {
            (
                n.char_id,
                NobleStats {
                    class_id: n.class_id,
                    // The saved name isn't in `olympiad_nobles`; it is filled in
                    // when the noble next registers (Java reads it via a join).
                    name: String::new(),
                    points: n.points,
                    comp_done: n.comp_done,
                    comp_won: n.comp_won,
                    comp_lost: n.comp_lost,
                    comp_drawn: n.comp_drawn,
                    comp_done_week: n.comp_done_week,
                },
            )
        })
        .collect();
    tracing::info!(
        "GameLoop: loaded Olympiad (cycle {current_cycle}, period {period}, {} nobles).",
        world.olympiad.nobles.len()
    );
}

/// `Olympiad.saveOlympiadStatus` + `saveNobleData` — persist the period row and
/// every noble record. Called on shutdown (and can be called on demand).
pub(crate) fn save_all(world: &World) {
    let oly = &world.olympiad;
    let nobles = oly
        .nobles
        .iter()
        .map(|(&char_id, n)| OlympiadNobleRow {
            char_id,
            class_id: n.class_id,
            points: n.points,
            comp_done: n.comp_done,
            comp_won: n.comp_won,
            comp_lost: n.comp_lost,
            comp_drawn: n.comp_drawn,
            comp_done_week: n.comp_done_week,
        })
        .collect();
    let _ = world.db.send(DbCommand::SaveOlympiad {
        current_cycle: oly.current_cycle,
        period: oly.period,
        olympiad_end: oly.olympiad_end,
        validation_end: oly.validation_end,
        next_weekly_change: oly.next_weekly_change,
        nobles,
    });
}

/// The player fields the registration gates and the noble record need.
struct NobleInfo {
    name: String,
    /// The active class (for the eligibility category + level check).
    class_id: i32,
    /// The main class the noble competes on (Java `getBaseClass`).
    base_class_id: i32,
    level: i32,
}

fn noble_info(world: &World, object_id: i32) -> Option<NobleInfo> {
    let p = world.objects.get_component::<Player>(&object_id)?;
    Some(NobleInfo {
        name: p.name.clone(),
        class_id: p.class_id,
        base_class_id: p.base_class_id,
        level: p.level,
    })
}

/// Java `OlympiadManager`'s "Classic noble equivalent" gate: the character must
/// be in the 3rd- or 4th-class group **and** at least level 55.
fn is_eligible(world: &World, info: &NobleInfo) -> bool {
    let cats = &world.data.categories;
    let class_ok = cats.contains("THIRD_CLASS_GROUP", info.class_id)
        || cats.contains("FOURTH_CLASS_GROUP", info.class_id);
    class_ok && info.level >= 55
}

/// `OlympiadManager.registerNoble` — join a match waiting list. Returns whether
/// the character is now registered, sending the appropriate system message
/// either way (Java's behaviour).
pub(crate) fn register(world: &mut World, object_id: i32, kind: CompetitionType) -> bool {
    let Some(info) = noble_info(world, object_id) else {
        return false;
    };

    // Only during the competition period.
    if !world.olympiad.in_comp_period {
        send_sm(
            world,
            object_id,
            sm_ids::THE_OLYMPIAD_GAMES_ARE_NOT_CURRENTLY_IN_PROGRESS,
        );
        return false;
    }

    // Eligibility (3rd/4th class + level 55).
    if !is_eligible(world, &info) {
        send_sm_c1(
            world,
            object_id,
            sm_ids::CHARACTER_C1_DOES_NOT_MEET_THE_OLYMPIAD_CONDITIONS,
            &info.name,
        );
        return false;
    }

    // Registration closes 20 minutes before the window ends.
    let ms_to_end = world.olympiad.comp_end_tick.saturating_sub(world.tick) * 100;
    if ms_to_end < REG_CLOSE_BEFORE_END_MS {
        send_sm(
            world,
            object_id,
            sm_ids::PARTICIPATION_REQUESTS_ARE_NO_LONGER_BEING_ACCEPTED,
        );
        return false;
    }

    // Weekly match cap.
    if world.olympiad.remaining_weekly_matches(object_id) < 1 {
        send_sm(
            world,
            object_id,
            sm_ids::THE_MAXIMUM_MATCHES_YOU_CAN_PARTICIPATE_IN_1_WEEK_IS_30,
        );
        return false;
    }

    // Already waiting (Java reports which list).
    if world.olympiad.is_registered(object_id) {
        let sm = if world.olympiad.non_class_registers.contains(&object_id) {
            sm_ids::C1_IS_ALREADY_REGISTERED_ON_THE_WAITING_LIST_FOR_THE_ALL_CLASS_BATTLE
        } else {
            sm_ids::C1_IS_ALREADY_REGISTERED_ON_THE_CLASS_MATCH_WAITING_LIST
        };
        send_sm_c1(world, object_id, sm, &info.name);
        return false;
    }

    // First-ever registration creates the noble's record with the starting points.
    world
        .olympiad
        .nobles
        .entry(object_id)
        .or_insert_with(|| NobleStats::fresh(info.base_class_id, info.name.clone()));

    match kind {
        CompetitionType::Classed => {
            world
                .olympiad
                .class_registers
                .entry(OlympiadState::class_group(info.base_class_id))
                .or_default()
                .insert(object_id);
            send_sm(
                world,
                object_id,
                sm_ids::YOU_HAVE_BEEN_REGISTERED_FOR_THE_OLYMPIAD_WAITING_LIST_FOR_A_CLASS_BATTLE,
            );
        }
        CompetitionType::NonClassed => {
            world.olympiad.non_class_registers.insert(object_id);
            send_sm(
                world,
                object_id,
                sm_ids::YOU_ARE_CURRENTLY_REGISTERED_FOR_A_1V1_CLASS_IRRELEVANT_MATCH,
            );
        }
    }
    true
}

/// `OlympiadManager.unRegisterNoble` — leave the waiting list.
pub(crate) fn unregister(world: &mut World, object_id: i32) -> bool {
    let Some(info) = noble_info(world, object_id) else {
        return false;
    };

    if !world.olympiad.in_comp_period {
        send_sm(
            world,
            object_id,
            sm_ids::THE_OLYMPIAD_GAMES_ARE_NOT_CURRENTLY_IN_PROGRESS,
        );
        return false;
    }

    if !is_eligible(world, &info) {
        send_sm_c1(
            world,
            object_id,
            sm_ids::CHARACTER_C1_DOES_NOT_MEET_THE_OLYMPIAD_CONDITIONS,
            &info.name,
        );
        return false;
    }

    if !world.olympiad.is_registered(object_id) {
        send_sm(
            world,
            object_id,
            sm_ids::YOU_ARE_NOT_CURRENTLY_REGISTERED_FOR_THE_OLYMPIAD,
        );
        return false;
    }

    // TODO(G25): Java also refuses if the noble is already in a running match
    // (`isInCompetition`); no matches exist yet, so there is nothing to check.

    if world.olympiad.remove_registration(object_id).is_some() {
        send_sm(
            world,
            object_id,
            sm_ids::YOU_HAVE_BEEN_REMOVED_FROM_THE_OLYMPIAD_WAITING_LIST,
        );
        return true;
    }
    false
}

fn send_sm(world: &World, object_id: i32, sm_id: i16) {
    if let Some(cid) = crate::game_loop::helpers::client_for_player(world, object_id) {
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(sp::system_message_with(sm_id, &[]));
        }
    }
}

fn send_sm_c1(world: &World, object_id: i32, sm_id: i16, name: &str) {
    if let Some(cid) = crate::game_loop::helpers::client_for_player(world, object_id) {
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(sp::system_message_with(
                sm_id,
                &[SmParam::PlayerName(name.to_string())],
            ));
        }
    }
}
