use super::*;
use crate::game_loop::guard::clan_of;
use crate::game_loop::helpers::pos_of;

/// Port of `clientpackets/RequestRestartPoint`: pick the respawn point for the
/// requested restart type — the siege "to castle"/"to siege HQ" cases when the
/// dead player is a participant, else the map-region town respawn — and start
/// the teleport; the revive itself lands on `Appearing`.
pub(crate) fn handle_request_restart_point(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestRestartPoint::read(body) else {
        return;
    };
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    let (px, py, pz, dead) = {
        let Some(pos) = world.objects.get_component::<Position>(&object_id) else {
            return;
        };
        let Some(vitals) = world.objects.get_component::<Vitals>(&object_id) else {
            return;
        };
        (pos.x, pos.y, pos.z, vitals.dead)
    };
    if !dead {
        return;
    }
    let race = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .and_then(|p| crate::enums::Race::from_ordinal(p.race))
        .unwrap_or(crate::enums::Race::Human);
    let pick = if world.cfg.character.random_respawn_in_town {
        world.roll(64) as usize
    } else {
        0
    };
    // Java `RequestRestartPoint.portPlayer` case 1 ("to clanhall"): respawn at
    // the clan's hall, and give back the EXP_RESTORE function's share of the
    // exp the death cost before the teleport.
    let clanhall_spawn = clanhall_restart_location(world, object_id, pkt.point_type);
    if clanhall_spawn.is_some() {
        restore_clanhall_exp(world, object_id);
    }
    // The siege restart cases (Java `RequestRestartPoint.portPlayer`); everything
    // else, and a non-participant, falls through to the map-region town respawn.
    let siege_spawn = siege_restart_location(world, object_id, pkt.point_type, pick);
    // Java case 2 ("to castle"): the castle's EXP-restore function gives back
    // its share of the death penalty, exactly like the clan hall's.
    if pkt.point_type == 2 && siege_spawn.is_some() {
        restore_castle_exp(world, object_id);
    }
    let Some((x, y, z)) = clanhall_spawn
        .or(siege_spawn)
        .or_else(|| world.data.map_region.town_respawn(px, py, pz, race, pick))
    else {
        return;
    };
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&object_id)
    {
        p.pending_revive = true;
    }
    teleport_player(world, object_id, x, y, z);
}

/// Java `RequestRestartPoint.portPlayer` case 1 ("to clanhall"): a player whose
/// clan owns a hall respawns at the hall's owner-restart point
/// (`MapRegionManager.getTeleToLocation(CLANHALL)`, which is the hall's
/// `ownerRestartPoint`). `None` for any other restart type, a clanless player,
/// Java `Die`'s constructor: which restart buttons this corpse is offered.
///
/// **The client only sends a `RequestRestartPoint` for a button it was told
/// exists**, so these flags are what makes `clanhall_restart_location` and
/// `siege_restart_location` reachable at all — both were fully implemented and
/// entirely unreachable, because every flag here was a hard-coded 0.
///
/// `to_village` is `canRevive() && !isPendingRevive()`; a revive already
/// proposed hides the button so the player answers the dialog instead.
pub(crate) fn die_options(world: &World, player_oid: i32) -> server_packets::DieOptions {
    use crate::model::siege::SiegeClanType;
    let mut opts = server_packets::DieOptions {
        to_village: world
            .objects
            .get_component::<crate::model::Player>(&player_oid)
            .is_some_and(|p| p.revive_request.is_none()),
        ..Default::default()
    };
    let Some(clan_id) = clan_of(world, player_oid) else {
        return opts;
    };
    opts.to_clan_hall = world.clan_halls.values().any(|h| h.owner_id == clan_id);
    opts.to_castle = world.clans.get(&clan_id).is_some_and(|c| c.castle_id > 0);

    // The siege half needs the corpse to be standing on a battlefield.
    let Some(castle_id) = world
        .objects
        .get_component::<Position>(&player_oid)
        .and_then(|p| world.data.zone_data.siege_castle_at(p.x, p.y, p.z))
    else {
        return opts;
    };
    let Some(siege) = world.sieges.get(&castle_id).filter(|s| s.in_progress) else {
        return opts;
    };
    let role = siege
        .clans
        .iter()
        .find(|c| c.clan_id == clan_id)
        .map(|c| c.kind);
    let is_castle_defence = role != Some(SiegeClanType::Attacker)
        && (siege.is_defender(clan_id)
            || world
                .clans
                .get(&clan_id)
                .is_some_and(|c| c.castle_id == castle_id));
    // `_toCastle = (clan.getCastleId() > 0) || isInCastleDefense` — a defender
    // gets the button even for a castle they do not own.
    opts.to_castle |= is_castle_defence;
    // `_toOutpost` needs an attacker **with a flag still standing**: Java reads
    // `!siegeClan.getFlag().isEmpty()`, so a razed base camp removes the button
    // rather than offering a respawn that would fail.
    opts.to_outpost = role == Some(SiegeClanType::Attacker)
        && !is_castle_defence
        && siege.flag_of(clan_id).is_some();
    opts
}

/// or a clan that owns no hall.
fn clanhall_restart_location(
    world: &World,
    player_oid: i32,
    point_type: i32,
) -> Option<(i32, i32, i32)> {
    if point_type != 1 {
        return None;
    }
    let clan_id = world
        .objects
        .get_component::<crate::model::Player>(&player_oid)?
        .clan_id;
    if clan_id == 0 {
        return None;
    }
    world
        .clan_halls
        .values()
        .find(|h| h.owner_id == clan_id)
        .map(|h| h.owner_restart)
}

/// Java `Player.restoreExp`: when respawning at the clan hall, the hall's
/// EXP_RESTORE function (if bought) restores that percentage of the exp the
/// death penalty cost. The port pre-computes the lost amount into
/// `lost_exp_on_death` (as the resurrection path does), so this reads it
/// directly rather than `_expBeforeDeath - getExp()`.
fn restore_clanhall_exp(world: &mut World, player_oid: i32) {
    let Some(clan_id) = world
        .objects
        .get_component::<crate::model::Player>(&player_oid)
        .map(|p| p.clan_id)
    else {
        return;
    };
    let Some(hall_id) = world
        .clan_halls
        .values()
        .find(|h| h.owner_id == clan_id)
        .map(|h| h.id)
    else {
        return;
    };
    let Some(percent) =
        crate::game_loop::clan_hall_function::active_function_value(world, hall_id, "EXP_RESTORE")
    else {
        return;
    };
    restore_lost_exp(world, player_oid, percent);
}

/// Give back `percent` of the exp lost on the last death and push the new total
/// to the client — Java `Player.restoreExp(percent)`. A no-op when nothing was
/// lost.
///
/// Shared by the clan-hall `EXP_RESTORE` function and the castle
/// `FUNC_RESTORE_EXP` one, which differ only in where the percent comes from.
fn restore_lost_exp(world: &mut World, player_oid: i32, percent: f64) {
    let restored = {
        let Some(p) = world
            .objects
            .get_component_mut::<crate::model::Player>(&player_oid)
        else {
            return;
        };
        if p.lost_exp_on_death <= 0 {
            return;
        }
        let restored = ((p.lost_exp_on_death as f64 * percent) / 100.0).round() as i64;
        p.exp += restored;
        p.lost_exp_on_death = 0;
        restored
    };
    if restored > 0 {
        // Java's `addExp` pushes the new exp to the client immediately.
        crate::game_loop::party::broadcast_user_info(world, player_oid);
    }
}

/// Java `Player.restoreExp` off the castle's `FUNC_RESTORE_EXP` (the levels
/// are the restore *percent*, 45 or 50 on this dist) — the castle twin of
/// [`restore_clanhall_exp`].
fn restore_castle_exp(world: &mut World, player_oid: i32) {
    let Some(clan_id) = clan_of(world, player_oid) else {
        return;
    };
    let Some(castle_id) = world
        .clans
        .get(&clan_id)
        .map(|c| c.castle_id)
        .filter(|&id| id > 0)
    else {
        return;
    };
    let Some(func) = crate::game_loop::castle::castle_function(
        world,
        castle_id,
        crate::model::castle::FUNC_RESTORE_EXP,
    ) else {
        return;
    };
    let percent = f64::from(func.level);
    restore_lost_exp(world, player_oid, percent);
}

/// The siege restart-point cases of Java `RequestRestartPoint.portPlayer` /
/// `MapRegionManager.getTeleToLocation` we can honor at a castle under an active
/// siege:
/// - **to castle** (type 2): a *defender* (the owner or a registered defender
///   clan) respawns inside the castle at the residence `getSpawnLoc`.
/// - **to siege HQ** (type 4): an *attacker* respawns at their planted HQ flag
///   (`getFlag`), if one still stands.
///
/// `None` (→ the caller's town respawn) for every other type/role. Note the
/// castle respawn is *not* gated on the control-tower count: in Interlude
/// Classic that count has no respawn/resurrection outcome at all (it only picks
/// a rejection message for a normal res skill during a siege — see
/// `Siege.control_tower_count`). The attacker respawn delay
/// (`getAttackerRespawnDelay`) is **0** on this dist
/// (`Siege.ini: AttackerRespawn = 0`), so there is no delay to apply.
fn siege_restart_location(
    world: &World,
    player_oid: i32,
    point_type: i32,
    pick: usize,
) -> Option<(i32, i32, i32)> {
    use crate::model::siege::SiegeClanType;
    let clan_id = world
        .objects
        .get_component::<crate::model::Player>(&player_oid)?
        .clan_id;
    if clan_id == 0 {
        return None;
    }
    let pos = world.objects.get_component::<Position>(&player_oid)?;
    let castle_id = world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z)?;
    let siege = world.sieges.get(&castle_id)?;
    if !siege.in_progress {
        return None;
    }
    let role = siege
        .clans
        .iter()
        .find(|c| c.clan_id == clan_id)
        .map(|c| c.kind);
    // `checkIsDefender` covers the castle owner even if it holds no `siege_clans`
    // row, so fold in castle ownership.
    let is_defender = world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.castle_id == castle_id)
        || matches!(role, Some(SiegeClanType::Owner | SiegeClanType::Defender));
    match point_type {
        2 if is_defender => {
            // `getSpawnLoc()` / `getChaoticSpawnLoc()`: a defender with
            // negative reputation restarts at the castle's chaotic points.
            let pts = world.data.castle_restart_points.get(&castle_id)?;
            let chaotic = world
                .objects
                .get_component::<crate::model::Player>(&player_oid)
                .is_some_and(|p| p.reputation < 0);
            pts.pick(chaotic, pick)
        }
        4 if role == Some(SiegeClanType::Attacker) => {
            let flag_oid = siege.flag_of(clan_id)?;
            pos_of(world, flag_oid)
        }
        _ => None,
    }
}

/// Java's `Creature.teleToLocation(ILocational)` overload — send `subject` to
/// wherever `anchor` currently stands.
///
/// "Teleport to another object" was open-coded at six sites, each re-reading
/// `Position` and unpacking the same `x, y, z` triple by hand. The returned
/// `bool` is what the cursed-weapon `//gocw` branches need: they try the holder,
/// then the dropped item, and fall through to a "not in the World" message only
/// if neither had a position — a plain no-op helper could not express that.
///
/// Note this reads the anchor's position *per call*. Callers that recall a whole
/// group to one spot (party recall, `//recall_all`) deliberately read it once up
/// front instead, so every member lands on the same coordinates even if the
/// anchor were to move mid-loop.
pub(crate) fn teleport_to_object(world: &mut World, subject: i32, anchor: i32) -> bool {
    let Some(pos) = crate::game_loop::guard::position(world, anchor) else {
        return false;
    };
    teleport_player(world, subject, pos.x, pos.y, pos.z);
    true
}

/// `Creature.teleToLocation`: stop moving, vanish from the old neighborhood
/// (`decayMe` → `DeleteObject`), push the new position, and wait for the
/// client's `Appearing` before becoming visible again.
pub(crate) fn teleport_player(world: &mut World, player_oid: i32, x: i32, y: i32, z: i32) {
    if world
        .objects
        .get_component::<crate::model::Player>(&player_oid)
        .is_none()
    {
        return;
    }
    // Java grounds the z on the geodata (`GeoEngine.getHeight`, non-flying)
    // and then lifts it "a bit" (`z += 5`).
    let z = world.geo.get_height(x, y, z) + 5;
    world.objects.remove_component::<Movement>(&player_oid);
    world.objects.remove_component::<Intent>(&player_oid);
    world
        .objects
        .remove_component::<crate::model::components::QueuedAction>(&player_oid);
    // The rest of `teleToLocation`'s prologue, in Java's order: cancel the
    // client's pending action, `abortCast()`, then `setTarget(null)` — all
    // before `decayMe`. The abort is what tells the client to stop drawing
    // the cast animation; a skill that teleports on landing (`/unstuck`'s
    // Escape, Recall) would otherwise leave the FX playing at the destination
    // for the client's own skill duration.
    if let Some(cs) = client_for_player(world, player_oid).and_then(|cid| world.clients.get(&cid)) {
        cs.send(server_packets::action_failed());
    }
    crate::game_loop::skills::cast::abort_cast_when_untargeted(world, player_oid);
    crate::game_loop::target::drop_target_notify(world, player_oid);
    let Some(heading) = world
        .objects
        .get_component::<Position>(&player_oid)
        .map(|p| p.heading)
    else {
        return;
    };
    broadcast_including_self(
        world,
        player_oid,
        &server_packets::teleport_to_location(player_oid, x, y, z, heading),
    );
    // `decayMe`: DeleteObject to everyone who could see the old position
    // (also drops their dangling targets).
    crate::game_loop::visibility::on_leave_world(world, player_oid);
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&player_oid)
    {
        p.teleporting = true;
    }
    // Java `Player.setTeleporting(true)` arms the watchdog here.
    arm_teleport_watchdog(world, player_oid);
    if let Some(pos) = world.objects.get_component_mut::<Position>(&player_oid) {
        pos.x = x;
        pos.y = y;
        pos.z = z;
    }
    // Through `World`, not the component directly, so `player_regions` moves
    // with the cell (an untracked teleport would leave the player receiving
    // broadcasts for the region they left).
    world.set_player_region(player_oid, crate::world::region_of(x, y));
    // "Send teleport finished packet to player" (Java, right after `setXYZ`):
    // the client sits on the black loading screen until this arrives, then
    // loads the destination and answers with `Appearing`.
    if let Some(cs) = client_for_player(world, player_oid).and_then(|cid| world.clients.get(&cid)) {
        cs.send(server_packets::ex_teleport_to_location_activate(
            player_oid, x, y, z, heading,
        ));
    }
    // Java: `if (!isPlayer() || client.isDetached()) onTeleported()` — with no
    // client to answer `Appearing`, the teleport is completed inline. Offline
    // traders are the case that reaches this, and without it they stay
    // `teleporting` for ever: the flag gates position validation and the
    // watchdog cannot clear it either, so a GM-teleported shop was left in a
    // state nothing could resolve short of a relog.
    if client_for_player(world, player_oid).is_none() {
        on_teleported(world, None, player_oid);
    }
}

/// Port of `clientpackets/Appearing`: the client finished loading after a
/// teleport — `onTeleported` (spawnMe → mutual CharInfo/NpcInfo, pending
/// revive resolves, fresh `UserInfo`).
pub(crate) fn handle_appearing(world: &mut World, client_id: u32) {
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    if !world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .is_some_and(|p| p.teleporting)
    {
        return;
    }
    on_teleported(world, Some(client_id), object_id);
}

/// `Creature.onTeleported` + `Player.onTeleported`: leave the teleporting
/// state, become visible again at the destination, and refresh the client.
///
/// Split out of [`handle_appearing`] because the client's packet is not the
/// only way in — [`teleport_watchdog_tick`] calls this too when the client
/// never answers, exactly as Java's `TeleportWatchdogTask` calls the same
/// `onTeleported`. The caller has already checked `teleporting`.
/// `Player.onTeleported`. `client_id` is `None` for a **detached** character —
/// an unattended shop — which Java reaches through the `isDetached()` branch
/// above rather than through `Appearing`.
///
/// The client-facing halves (the visibility exchange and the fresh `UserInfo`)
/// are skipped in that case because there is no session to send to. Onlookers
/// still learn about the move: `set_player_region` has already re-indexed the
/// shop, and every other player's visibility scan reads that index — which is
/// why offline traders are indexed there in the first place.
fn on_teleported(world: &mut World, client_id: Option<u32>, object_id: i32) {
    // Java `setTeleporting(false)` — which also cancels the watchdog.
    world.teleport_watchdog_due.remove(&object_id);
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&object_id)
    {
        p.teleporting = false;
    }
    if world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .is_some_and(|p| p.pending_revive)
    {
        do_revive(world, object_id);
    }
    // `spawnMe`-equivalent visibility exchange at the new position.
    if let Some(cid) = client_id {
        crate::game_loop::visibility::on_enter_world(world, cid, object_id);
    }
    // Java `onTeleported` → `revalidateZone(true)`. Runs for a detached
    // character too: the destination's zone membership is what later decides
    // whether the shop is still allowed to be there.
    crate::game_loop::zones::revalidate_zone(world, object_id, true);
    if let (Some(v), Some(cs)) = (
        crate::model::PlayerView::of_world(world, object_id),
        client_id.and_then(|cid| world.clients.get(&cid)),
    ) {
        cs.send(crate::network::user_info::user_info(
            &v,
            &world.data,
            &world.cfg.character,
            crate::game_loop::party::calculate_relation(world, v.p),
        ));
    }
}

/// How often [`teleport_watchdog_tick`] sweeps for expired teleports — 1 s,
/// well under any sane `TeleportWatchdogTimeout` (the ini recommends ≥ 60 s),
/// so the sweep granularity is noise next to the timeout itself.
pub(crate) const TELEPORT_WATCHDOG_PERIOD: u64 = 10;

/// Java `Player.setTeleporting(true)`'s watchdog arm:
///
/// ```java
/// if ((_teleportWatchdog == null) && (Config.TELEPORT_WATCHDOG_TIMEOUT > 0))
///     _teleportWatchdog = ThreadPool.schedule(new TeleportWatchdogTask(this), TIMEOUT * 1000L);
/// ```
///
/// The `== null` guard is why this is `or_insert` and not a plain insert: a
/// second teleport that starts before the first completed keeps the *original*
/// deadline rather than pushing it out. `0` leaves the feature off, in which
/// case a stuck client stays stuck until it relogs — Java's default too.
fn arm_teleport_watchdog(world: &mut World, player_oid: i32) {
    let timeout = world.cfg.character.teleport_watchdog_timeout_ticks;
    if timeout == 0 {
        return;
    }
    let due = world.tick + timeout;
    world.teleport_watchdog_due.entry(player_oid).or_insert(due);
}

/// Java `TeleportWatchdogTask.run()`, swept for every armed player:
///
/// ```java
/// if ((_player == null) || !_player.isTeleporting()) return;
/// _player.onTeleported();
/// ```
///
/// A teleport only ends when the client answers `ExTeleportToLocationActivate`
/// with `Appearing`. If it never does — hung zone load, dropped packet, a
/// client that crashed on the loading screen — the character stays decayed out
/// of the world: invisible to everyone, its `ValidatePosition` reports ignored
/// (`position.rs`), recoverable only by relogging. This forces the teleport
/// through server-side instead.
///
/// The `isTeleporting()` re-check is Java's and it still matters here: an entry
/// can outlive the teleport it was armed for (removal races a same-tick
/// completion), and a fired watchdog must not disturb a player who has already
/// arrived.
pub(crate) fn teleport_watchdog_tick(world: &mut World) {
    if world.teleport_watchdog_due.is_empty() {
        return;
    }
    let due: Vec<i32> = world
        .teleport_watchdog_due
        .iter()
        .filter(|&(_, &at)| world.tick >= at)
        .map(|(&oid, _)| oid)
        .collect();
    for oid in due {
        world.teleport_watchdog_due.remove(&oid);
        if !world
            .objects
            .get_component::<crate::model::Player>(&oid)
            .is_some_and(|p| p.teleporting)
        {
            continue; // Java's `_player == null || !isTeleporting()` bail.
        }
        // `onTeleported` → `spawnMe` needs a session to send the visibility
        // exchange and `UserInfo` to. A player with no client left (logout
        // raced the sweep) has nothing to be made visible *to*; the logout
        // path despawns them regardless.
        let Some(client_id) = client_for_player(world, oid) else {
            continue;
        };
        tracing::warn!(
            "TeleportWatchdog: forcing teleport completion for player {} (no Appearing within {} s).",
            oid,
            world.cfg.character.teleport_watchdog_timeout_ticks / 10
        );
        on_teleported(world, Some(client_id), oid);
    }
}
