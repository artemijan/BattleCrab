//! Grand Olympiad (G25) — noble registration into the match queues.
//! Java `model/olympiad/OlympiadManager` (`registerNoble` / `unRegisterNoble`).
//!
//! Slice 1: a qualifying character joins or leaves the class-based / non-class
//! waiting lists, with the eligibility and timing gates. Match-making, the
//! stadiums and hero calculation are later slices.

use crate::model::olympiad::{CompetitionType, NobleStats, OlympiadState, REG_CLOSE_BEFORE_END_MS};
use crate::model::Player;
use crate::network::server_packets::{self as sp, sm_ids, SmParam};
use crate::world::World;

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
