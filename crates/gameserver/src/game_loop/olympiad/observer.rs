//! Observer mode (spectating): the match list, enter/leave observer and the
//! spectator flags.

use super::*;

/// The spectator stand — midway between the two arena spawns. (Java draws a
/// random point from the zone's `spectatorSpawns`; the port has one arena, so a
/// fixed vantage point suffices — matches are instance-scoped anyway.)
const OBSERVE_SPAWN: (i32, i32, i32) = (-88070, -252843, -3320);

/// The ongoing match at arena `arena` (its index), if any.
fn arena_match(world: &World, arena: i32) -> Option<&OlympiadMatch> {
    world
        .olympiad
        .matches
        .iter()
        .find(|m| m.arena as i32 == arena)
}

/// Java `OlyManager` `watchmatch` / `RequestOlympiadMatchList`: send the list of
/// ongoing matches a spectator can jump between.
pub(crate) fn send_match_list(world: &World, client_id: u32) {
    let rows: Vec<sp::OlympiadMatchRow> = world
        .olympiad
        .matches
        .iter()
        .map(|m| sp::OlympiadMatchRow {
            arena: m.arena as i32,
            // A match in the live list is under way (post-countdown).
            running: true,
            player_a: player_name_or_empty(world, m.player_a),
            player_b: player_name_or_empty(world, m.player_b),
        })
        .collect();
    send_to_client(world, client_id, sp::ex_olympiad_match_list(&rows));
}

/// Java `OlyManager.arenachange` → `Player.enterOlympiadObserverMode`: teleport
/// the viewer into the chosen arena's instance as a hidden spectator. Refused
/// outside the competition period, or while registered / competing.
pub(crate) fn enter_observer(world: &mut World, client_id: u32, player_oid: i32, arena: i32) {
    if !world.olympiad.in_comp_period
        || world.olympiad.is_registered(player_oid)
        || world.olympiad.is_in_competition(player_oid)
    {
        return;
    }
    let Some(instance_id) = arena_match(world, arena).map(|m| m.instance_id) else {
        return; // no match at that arena
    };

    // On first entry, remember where to return to (Java `setLastLocation`).
    let already = world.objects.has_component::<OlympiadObserver>(&player_oid);
    if !already {
        let return_pos = pos_of(world, player_oid).unwrap_or(OBSERVE_SPAWN);
        world
            .objects
            .add_components(&player_oid, OlympiadObserver { return_pos, arena });
    } else if let Some(o) = world
        .objects
        .get_component_mut::<OlympiadObserver>(&player_oid)
    {
        o.arena = arena; // switching arenas
    }
    // Scope the viewer to the match's instance so they see only that fight.
    world.objects.add_components(
        &player_oid,
        crate::model::components::InstanceId(instance_id),
    );
    crate::game_loop::death::teleport_player(
        world,
        player_oid,
        OBSERVE_SPAWN.0,
        OBSERVE_SPAWN.1,
        OBSERVE_SPAWN.2,
    );
    send_to_client(world, client_id, sp::ex_olympiad_mode(3));
    // Java `enterOlympiadObserverMode` also makes the spectator invulnerable +
    // invisible so a stray AoE can't touch them and they don't clutter the
    // arena. Set the two flags (adding the component if absent), leaving any
    // other admin flag untouched.
    set_observer_flags(world, player_oid, true);
}

/// Toggle the spectator's invulnerable + invisible flags (Java the observer
/// mode's `setInvul`/`setInvisible`), adding the `AdminFlags` component on first
/// use and preserving any other flags already set (e.g. a GM's).
fn set_observer_flags(world: &mut World, player_oid: i32, on: bool) {
    crate::game_loop::helpers::update_admin_flags(world, player_oid, |f| {
        f.invul = on;
        f.hidden = on;
    });
}

/// Java `RequestOlympiadObserverEnd` → `Player.leaveOlympiadObserverMode`:
/// teleport the spectator back and drop the observer state.
pub(crate) fn leave_observer(world: &mut World, client_id: u32, player_oid: i32) {
    let Some(observer) = world
        .objects
        .get_component::<OlympiadObserver>(&player_oid)
        .copied()
    else {
        return;
    };
    world
        .objects
        .remove_component::<OlympiadObserver>(&player_oid);
    world
        .objects
        .remove_component::<crate::model::components::InstanceId>(&player_oid);
    // Clear the spectator's invul + invisible (Java restores the normal state).
    set_observer_flags(world, player_oid, false);
    send_to_client(world, client_id, sp::ex_olympiad_mode(0));
    let (x, y, z) = observer.return_pos;
    crate::game_loop::death::teleport_player(world, player_oid, x, y, z);
}

/// Whether the player is currently spectating a match.
pub(crate) fn is_observing(world: &World, player_oid: i32) -> bool {
    world.objects.has_component::<OlympiadObserver>(&player_oid)
}
