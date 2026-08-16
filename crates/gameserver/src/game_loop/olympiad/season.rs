//! Season transitions: noble ranks, trade points, the olympiad-end crowning,
//! the end-of-month snapshot, leaderboards and the validation-day end.

use super::*;

/// `Olympiad.loadNoblesRank`: rank the classified nobles (≥ 10 matches) by points
/// into percentile tiers — 1 (top 1 %), 2 (10 %), 3 (25 %), 4 (50 %), 5 (rest).
fn compute_noble_ranks(world: &World) -> std::collections::HashMap<i32, u8> {
    let mut classified: Vec<(i32, i32)> = world
        .olympiad
        .nobles
        .iter()
        .filter(|(_, n)| n.comp_done >= world.cfg.olympiad.min_matches_for_points)
        .map(|(&id, n)| (id, n.points))
        .collect();
    // Highest points first (Java orders the query by points DESC).
    classified.sort_by(|a, b| b.1.cmp(&a.1));

    let total = classified.len() as f64;
    let mut r1 = (total * 0.01).round() as usize;
    let mut r2 = (total * 0.10).round() as usize;
    let mut r3 = (total * 0.25).round() as usize;
    let mut r4 = (total * 0.50).round() as usize;
    if r1 == 0 {
        r1 = 1;
        r2 += 1;
        r3 += 1;
        r4 += 1;
    }

    let mut ranks = std::collections::HashMap::new();
    for (i, (id, _)) in classified.iter().enumerate() {
        let place = i + 1; // 1-based, like Java's `place++`
        let rank = if place <= r1 {
            1
        } else if place <= r2 {
            2
        } else if place <= r3 {
            3
        } else if place <= r4 {
            4
        } else {
            5
        };
        ranks.insert(*id, rank);
    }
    ranks
}

/// `Olympiad.getOlympiadTradePoint`: the points a noble may exchange for marks —
/// a hero bonus plus a rank bonus. Zero for the unranked or point-less.
fn olympiad_trade_point(
    world: &World,
    ranks: &std::collections::HashMap<i32, u8>,
    object_id: i32,
) -> i32 {
    let Some(&rank) = ranks.get(&object_id) else {
        return 0;
    };
    if world
        .olympiad
        .nobles
        .get(&object_id)
        .map_or(0, |n| n.points)
        == 0
    {
        return 0;
    }
    // Java `isHero(objectId) || isUnclaimedHero(objectId)` — the trade bonus
    // rides the crown, not the claim, so a hero who has not been to the monument
    // yet still exchanges at the hero rate.
    let hero = if world.olympiad.is_crowned(object_id) {
        world.cfg.olympiad.hero_points
    } else {
        0
    };
    hero + world.cfg.olympiad.rank_points[(rank as usize) - 1]
}

/// After a round ends, bank each noble's exchangeable points on their
/// `UNCLAIMED_OLYMPIAD_POINTS` variable (Java `loadNoblesRank`'s reward loop) —
/// on the live component for online nobles, straight to `character_variables`
/// for offline ones.
fn store_trade_points(world: &mut World) {
    let ranks = compute_noble_ranks(world);
    let ids: Vec<i32> = world.olympiad.nobles.keys().copied().collect();
    for oid in ids {
        let points = olympiad_trade_point(world, &ranks, oid);
        if points <= 0 {
            continue;
        }
        if let Some(v) = world
            .objects
            .get_component_mut::<crate::model::components::PlayerVariables>(&oid)
        {
            v.set_int(UNCLAIMED_POINTS_VAR, points);
        } else {
            let _ = world.db.send(DbCommand::StoreCharVar {
                char_id: oid,
                var: UNCLAIMED_POINTS_VAR.to_string(),
                value: points.to_string(),
            });
        }
    }
}

/// `OlympiadEndTask`: the monthly round ends — enter the validation period,
/// crown the new heroes, and schedule the return to a fresh cycle.
pub(crate) fn handle_olympiad_end(world: &mut World) {
    if world.olympiad.period != 0 {
        return;
    }
    world.olympiad.period = 1;

    // Uncrown the previous cycle's (online) heroes, then crown the new ones.
    let old: Vec<i32> = world.olympiad.heroes.iter().map(|(id, _)| *id).collect();
    for id in old {
        if is_online(world, id) {
            crate::game_loop::admin::hero::set_hero(world, id, false);
        }
    }
    // The new crown grants **no** status yet: Java's `computeNewHeroes` writes
    // `claimed = false` and stops, so each hero must collect the title at the
    // Monument of Heroes (`claim_hero`) before `setHero` runs.
    let heroes = compute_heroes(world);
    world.olympiad.heroes = heroes;
    // Record each new hero's display data (name from the noble, clan from the
    // online player) for the `ExHeroList` window.
    world.olympiad.hero_info.clear();
    let crowned: Vec<i32> = world.olympiad.heroes.iter().map(|(id, _)| *id).collect();
    for id in crowned {
        let name = world
            .olympiad
            .nobles
            .get(&id)
            .map(|n| n.name.clone())
            .unwrap_or_default();
        let clan_id = clan_of_or_zero(world, id);
        world.olympiad.hero_info.insert(
            id,
            crate::model::olympiad::HeroInfo {
                name,
                clan_id,
                // A freshly-crowned hero has not written any words yet.
                message: String::new(),
            },
        );
    }
    tracing::info!(
        "Olympiad: round {} ended; {} heroes crowned.",
        world.olympiad.current_cycle,
        world.olympiad.heroes.len()
    );
    // Persist the crown (Java `Hero.computeNewHeroes`): bump each hero's count
    // and replace the `heroes` table so it survives relogs and applies to
    // offline heroes on their next login.
    let hero_rows: Vec<HeroRow> = world
        .olympiad
        .heroes
        .iter()
        .map(|&(char_id, class_id)| {
            let count = world
                .olympiad
                .hero_counts
                .get(&char_id)
                .copied()
                .unwrap_or(0)
                + 1;
            world.olympiad.hero_counts.insert(char_id, count);
            let info = world.olympiad.hero_info.get(&char_id);
            HeroRow {
                char_id,
                class_id,
                count,
                // Not persisted (no columns), but carried for consistency.
                name: info.map(|i| i.name.clone()).unwrap_or_default(),
                clan_id: info.map(|i| i.clan_id).unwrap_or(0),
                message: info.map(|i| i.message.clone()).unwrap_or_default(),
                // Java `computeNewHeroes` writes `claimed = false` for both the
                // re-crowned and the newly crowned: a fresh cycle must be
                // collected again at the monument.
                claimed: false,
            }
        })
        .collect();
    world.olympiad.claimed_heroes.clear();
    let _ = world.db.send(DbCommand::SaveHeroes { heroes: hero_rows });

    // Bank each noble's exchangeable points for the mark exchange at the manager.
    store_trade_points(world);
    // Announce the round's end to everyone online.
    let announce = sp::system_message_with(
        sm_ids::ROUND_S1_OF_THE_OLYMPIAD_GAMES_HAS_NOW_ENDED,
        &[SmParam::Int(world.olympiad.current_cycle)],
    );
    world.broadcast_to_all_online(&announce);

    let now = commons::util::now_millis();
    let validation_period = world.cfg.olympiad.validation_period_ms;
    world.olympiad.validation_end = now + validation_period;
    save_all(world);
    // `updateMonthlyData`, which Java runs right after `saveOlympiadStatus`:
    // freeze this cycle's nobles as the leaderboard the Olympiad Manager shows
    // until the next round ends. The DB half rides the same channel behind
    // `save_all`'s `SaveOlympiad`, so it copies rows that are already written.
    snapshot_eom(world);
    world.scheduler.schedule(
        fire_at(world, validation_period),
        ScheduledTask::OlympiadValidationEnd,
    );
}

/// Java `Olympiad.updateMonthlyData` — replace the end-of-cycle snapshot with a
/// copy of the live nobles, in memory and in `olympiad_nobles_eom`.
fn snapshot_eom(world: &mut World) {
    world.olympiad.eom_nobles = world
        .olympiad
        .nobles
        .values()
        .map(|n| OlympiadEomRow {
            class_id: n.class_id,
            name: n.name.clone(),
            points: n.points,
            comp_done: n.comp_done,
            comp_won: n.comp_won,
        })
        .collect();
    let _ = world.db.send(DbCommand::SnapshotOlympiadEom);
}

/// Java `Olympiad.getClassLeaderBoard(classId)` — the top ten of the last
/// completed cycle for one class: at least `AltOlyMinMatchesForPoints` matches,
/// ordered by points, then matches played, then wins, all descending. The
/// Soulhound branch (class 132/133) is Kamael-only and unreachable here.
pub(crate) fn class_leader_board(world: &World, class_id: i32) -> Vec<String> {
    let mut rows: Vec<&OlympiadEomRow> = world
        .olympiad
        .eom_nobles
        .iter()
        .filter(|n| {
            n.class_id == class_id && n.comp_done >= world.cfg.olympiad.min_matches_for_points
        })
        .collect();
    rows.sort_by(|a, b| {
        b.points
            .cmp(&a.points)
            .then(b.comp_done.cmp(&a.comp_done))
            .then(b.comp_won.cmp(&a.comp_won))
    });
    rows.into_iter()
        .take(LEADER_BOARD_LIMIT)
        .map(|n| n.name.clone())
        .collect()
}

/// `ValidationEndTask`: the validation period ends — start a new cycle's
/// competition period with a clean noble table.
pub(crate) fn handle_validation_end(world: &mut World) {
    world.olympiad.period = 0;
    world.olympiad.current_cycle += 1;
    world.olympiad.nobles.clear(); // `deleteNobles` (TRUNCATE olympiad_nobles)
    let now = commons::util::now_millis();
    world.olympiad.olympiad_end = next_olympiad_end(&world.cfg.olympiad, now);
    save_all(world);
    tracing::info!(
        "Olympiad: validation ended; cycle {} begins.",
        world.olympiad.current_cycle
    );
    // Re-arm the competition window + the next period end.
    arm_comp_schedule(world, now);
    world.scheduler.schedule(
        fire_at(world, world.olympiad.olympiad_end - now),
        ScheduledTask::OlympiadEnd,
    );
}
