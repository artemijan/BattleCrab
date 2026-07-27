//! Punishment runtime (G31) — port of Java `PunishmentManager` +
//! `PunishmentTask` + the punishment handlers. This slice covers the boot load,
//! the JAIL effect (confine a player to the GM prison and release on
//! `//unjail`/expiry), and the login re-apply + JailZone keep-in. Ban /
//! chat-ban / party-ban reuse the same model in later slices.

use tracing::info;

use crate::db::DbCommand;
use crate::model::punishment::{Punishment, PunishmentAffect, PunishmentType};
use crate::model::Player;
use crate::network::server_packets::{self as sp, sm_ids, SmParam};
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::World;

use super::helpers::client_for_player;

const TICKS_PER_SECOND: u64 = 10;

/// Java `JailZone.JAIL_IN_LOC` — where a jailed player is teleported.
const JAIL_IN_LOC: (i32, i32, i32) = (-114356, -249645, -2984);
/// Java `JailZone.JAIL_OUT_LOC` — where a released player is teleported.
const JAIL_OUT_LOC: (i32, i32, i32) = (17836, 170178, -3507);

/// Boot restore (Java `PunishmentManager.load`), driven by
/// `DbEvent::PunishmentsLoaded`: seed the id allocator and register every active
/// punishment. Timed punishments re-arm their expiry timer so they lift on
/// schedule even across a restart. No players are online at boot, so the JAIL
/// effect is (re-)applied per-player on enter-world instead ([`on_enter_world`]).
pub(crate) fn on_loaded(world: &mut World, next_id: i32, punishments: Vec<Punishment>) {
    world.punishments.next_id = next_id.max(1);
    let count = punishments.len();
    for task in punishments {
        arm_expiry(world, task.id, task.expiration);
        world.punishments.add(task);
    }
    info!("PunishmentManager: Loaded {count} active punishments.");
}

/// Arm a timed punishment's expiry (Java `PunishmentTask.startPunishment`'s
/// `ThreadPool.schedule(this, expiration - now)`). Permanent punishments
/// (`expiration == 0`) never expire, so no timer is set.
fn arm_expiry(world: &mut World, punishment_id: i32, expiration: i64) {
    if expiration <= 0 {
        return;
    }
    let now = commons::util::now_millis();
    let delay_ticks = ((expiration - now).max(0) / 1000) as u64 * TICKS_PER_SECOND;
    world.scheduler.schedule(
        world.tick + delay_ticks,
        ScheduledTask::PunishmentExpire { punishment_id },
    );
}

/// A timed punishment's expiry fired (Java `PunishmentTask.run` → `onEnd`): drop
/// the punishment, delete its row, and run the release effect. A stale timer
/// (the row already removed by `//unjail`) no-ops.
pub(crate) fn on_expire(world: &mut World, punishment_id: i32) {
    let Some(task) = world.punishments.remove_by_id(punishment_id) else {
        return;
    };
    let _ = world.db.send(DbCommand::DeletePunishment { id: task.id });
    end_effect(world, &task);
}

// ---------------------------------------------------------------------------
// Jail (Java `JailHandler`)
// ---------------------------------------------------------------------------

/// Java `admin_punishment_add` for CHARACTER/JAIL: register a new jail
/// punishment on `char_id`, persist it, arm its expiry, and (if the character
/// is online) apply the effect immediately. `minutes == 0` jails forever.
/// Returns `false` if the character already has an active jail (Java's
/// "already affected" guard).
pub(crate) fn jail_character(
    world: &mut World,
    char_id: i32,
    minutes: i64,
    reason: String,
    punished_by: String,
) -> bool {
    let key = char_id.to_string();
    if world
        .punishments
        .has_punishment(&key, PunishmentAffect::Character, PunishmentType::Jail)
    {
        return false;
    }
    let expiration = if minutes > 0 {
        commons::util::now_millis() + minutes * 60_000
    } else {
        0
    };
    let id = world.punishments.alloc_id();
    let task = Punishment {
        id,
        key,
        affect: PunishmentAffect::Character,
        ptype: PunishmentType::Jail,
        expiration,
        reason: reason.clone(),
        punished_by: punished_by.clone(),
    };
    let _ = world.db.send(DbCommand::StorePunishment {
        id,
        key: char_id.to_string(),
        affect: task.affect.as_str().to_string(),
        ptype: task.ptype.as_str().to_string(),
        expiration,
        reason,
        punished_by,
    });
    world.punishments.add(task);
    arm_expiry(world, id, expiration);

    // Apply to the online character (Java `JailHandler.onStart`'s CHARACTER
    // branch). Offline characters get it on their next login ([`on_enter_world`]).
    if world.objects.has_component::<Player>(&char_id) {
        apply_jail_to_player(world, char_id, expiration);
    }
    true
}

/// Java `admin_punishment_remove` for CHARACTER/JAIL: drop the jail punishment,
/// delete its row, and release the (online) character. Returns `false` if there
/// was no such punishment.
pub(crate) fn unjail_character(world: &mut World, char_id: i32) -> bool {
    let key = char_id.to_string();
    let Some(task) =
        world
            .punishments
            .remove(&key, PunishmentAffect::Character, PunishmentType::Jail)
    else {
        return false;
    };
    let _ = world.db.send(DbCommand::DeletePunishment { id: task.id });
    end_effect(world, &task);
    true
}

/// Run a punishment's release effect on the affected online player (Java
/// handler `onEnd`). Only JAIL has one in this slice.
fn end_effect(world: &mut World, task: &Punishment) {
    if task.ptype != PunishmentType::Jail {
        return;
    }
    // CHARACTER key is the object id; the other affects match live players by
    // account/IP (slice-2+ ban paths), handled the same way.
    let targets = players_matching(world, task);
    for oid in targets {
        remove_jail_from_player(world, oid);
    }
}

/// The online players a punishment currently affects (Java handler's per-affect
/// player lookup). For CHARACTER it is the one object id; for ACCOUNT/IP it is
/// every online player on that account/IP.
fn players_matching(world: &World, task: &Punishment) -> Vec<i32> {
    match task.affect {
        PunishmentAffect::Character => task
            .key
            .parse::<i32>()
            .ok()
            .filter(|oid| world.objects.has_component::<Player>(oid))
            .into_iter()
            .collect(),
        PunishmentAffect::Account => world
            .clients
            .values()
            .filter_map(|cs| match cs {
                ClientSession::InGame(s) => {
                    let oid = s.player_object_id();
                    world
                        .objects
                        .get_component::<Player>(&oid)
                        .filter(|p| p.account == task.key)
                        .map(|_| oid)
                }
                _ => None,
            })
            .collect(),
        PunishmentAffect::Ip => world
            .clients
            .values()
            .filter_map(|cs| match cs {
                ClientSession::InGame(s) if s.addr.ip().to_string() == task.key => {
                    Some(s.player_object_id())
                }
                _ => None,
            })
            .collect(),
        // HWID matching lands in G31 slice 5 (needs client hardware info).
        PunishmentAffect::Hwid => Vec::new(),
    }
}

/// Java `JailHandler.applyToPlayer`: mark the player jailed, teleport them into
/// the prison, and tell them how long for. (Java delays the teleport 2 s and
/// shows `jail_in.htm`; we teleport at once and send the duration line — a
/// documented simplification, the confinement is what matters.)
fn apply_jail_to_player(world: &mut World, player_oid: i32, expiration: i64) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.jailed = true;
    }
    super::death::teleport_player(
        world,
        player_oid,
        JAIL_IN_LOC.0,
        JAIL_IN_LOC.1,
        JAIL_IN_LOC.2,
    );
    let now = commons::util::now_millis();
    let text = if expiration > 0 {
        let secs = (expiration - now) / 1000;
        if secs > 60 {
            format!("You've been jailed for {} minutes.", secs / 60)
        } else {
            format!("You've been jailed for {secs} seconds.")
        }
    } else {
        "You've been jailed forever.".to_string()
    };
    send_text(world, player_oid, &text);
}

/// Java `JailHandler.removeFromPlayer`: clear the flag and teleport the player
/// out of the prison.
fn remove_jail_from_player(world: &mut World, player_oid: i32) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.jailed = false;
    }
    super::death::teleport_player(
        world,
        player_oid,
        JAIL_OUT_LOC.0,
        JAIL_OUT_LOC.1,
        JAIL_OUT_LOC.2,
    );
    send_text(
        world,
        player_oid,
        "You are free for now, respect server rules!",
    );
}

// ---------------------------------------------------------------------------
// Login re-apply + JailZone keep-in (Java `JailHandler.onPlayerLogin` /
// `JailZone.onExit`)
// ---------------------------------------------------------------------------

/// Whether any of a player's affect keys currently carries a JAIL punishment
/// (Java `Player.isJailed`). Needs the live account + IP, so it takes them
/// rather than reading them back off components.
fn is_jailed(world: &World, char_id: i32, account: &str, ip: &str) -> bool {
    world
        .punishments
        .player_has(PunishmentType::Jail, char_id, account, ip, None)
}

/// Java `JailHandler.onPlayerLogin`: a jailed character logging in (or one
/// whose jail lifted while offline) is put in / taken out of the prison.
/// Called from the enter-world flow.
pub(crate) fn on_enter_world(world: &mut World, client_id: u32, object_id: i32) {
    let account = world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| p.account.clone())
        .unwrap_or_default();
    let ip = world
        .clients
        .get(&client_id)
        .map(|cs| cs.addr().ip().to_string())
        .unwrap_or_default();
    let jailed = is_jailed(world, object_id, &account, &ip);
    let in_zone = in_jail_zone(world, object_id);
    let is_gm = world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(|p| p.is_gm(&world.data));

    if jailed {
        if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
            p.jailed = true;
        }
        if !in_zone {
            let expiration = world
                .punishments
                .get(
                    &object_id.to_string(),
                    PunishmentAffect::Character,
                    PunishmentType::Jail,
                )
                .map(|t| t.expiration)
                .unwrap_or(0);
            apply_jail_to_player(world, object_id, expiration);
        }
    } else if in_zone && !is_gm {
        remove_jail_from_player(world, object_id);
    }
}

/// Whether the player is currently standing in a JailZone.
fn in_jail_zone(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::components::Position>(&object_id)
        .is_some_and(|pos| world.data.zone_data.in_jail_zone(pos.x, pos.y, pos.z))
}

/// JailZone keep-in (Java `JailZone.onExit`): a jailed player who has left the
/// prison is teleported straight back. Called from `revalidate_zone` after a
/// position change. GMs are exempt (they can leave to administer).
pub(crate) fn enforce_jail_keep_in(world: &mut World, object_id: i32) {
    let jailed = world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(|p| p.jailed);
    if !jailed {
        return;
    }
    let is_gm = world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(|p| p.is_gm(&world.data));
    if is_gm || in_jail_zone(world, object_id) {
        return;
    }
    super::death::teleport_player(
        world,
        object_id,
        JAIL_IN_LOC.0,
        JAIL_IN_LOC.1,
        JAIL_IN_LOC.2,
    );
    send_text(
        world,
        object_id,
        "You cannot cheat your way out of here. You must wait until your jail time is over.",
    );
}

/// Send a plain-text system line to a player's client (Java `sendMessage`).
fn send_text(world: &World, player_oid: i32, text: &str) {
    if let Some(cs) = client_for_player(world, player_oid).and_then(|cid| world.clients.get(&cid)) {
        cs.send(sp::system_message_with(
            sm_ids::S1_TEXT,
            &[SmParam::Text(text.to_string())],
        ));
    }
}
