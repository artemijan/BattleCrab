//! Punishment runtime (G31) — port of Java `PunishmentManager` +
//! `PunishmentTask` + the punishment handlers. This slice covers the boot load,
//! the JAIL effect (confine a player to the GM prison and release on
//! `//unjail`/expiry), and the login re-apply + JailZone keep-in. Ban /
//! chat-ban / party-ban reuse the same model in later slices.

use crate::game_loop::time::TICKS_PER_SECOND;
use tracing::info;

use crate::db::DbCommand;
use crate::model::Player;
use crate::model::punishment::{
    IllegalActionPunishment, Punishment, PunishmentAffect, PunishmentType,
};
use crate::network::server_packets::{SmParam, sm_ids};
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::World;

use crate::game_loop::helpers::{client_for_player, send_sm_to_player};

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
    crate::game_loop::time::schedule_in_ms(
        world,
        expiration - now,
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
// Generic punishment engine (Java `PunishmentManager.startPunishment` +
// `PunishmentTask.onStart`/`onEnd` handler dispatch)
// ---------------------------------------------------------------------------

/// Java `admin_punishment_add`: register a new punishment, persist it, arm its
/// expiry, and apply its onStart effect to every affected online player.
/// `expiration` is absolute unix millis, or `0` for forever. Returns `false`
/// if that exact `(key, affect, type)` is already punished (Java's "already
/// affected" guard).
pub(crate) fn start_punishment(
    world: &mut World,
    key: String,
    affect: PunishmentAffect,
    ptype: PunishmentType,
    expiration: i64,
    reason: String,
    punished_by: String,
) -> bool {
    if world.punishments.has_punishment(&key, affect, ptype) {
        return false;
    }
    let id = world.punishments.alloc_id();
    let task = Punishment {
        id,
        key: key.clone(),
        affect,
        ptype,
        expiration,
        reason: reason.clone(),
        punished_by: punished_by.clone(),
    };
    let _ = world.db.send(DbCommand::StorePunishment {
        id,
        key,
        affect: affect.as_str().to_string(),
        ptype: ptype.as_str().to_string(),
        expiration,
        reason,
        punished_by,
    });
    world.punishments.add(task.clone());
    arm_expiry(world, id, expiration);
    apply_effect(world, &task);
    true
}

/// Java `admin_punishment_remove`: drop a punishment, delete its row, and run
/// its onEnd effect. Returns `false` if there was no such punishment.
pub(crate) fn stop_punishment(
    world: &mut World,
    key: &str,
    affect: PunishmentAffect,
    ptype: PunishmentType,
) -> bool {
    let Some(task) = world.punishments.remove(key, affect, ptype) else {
        return false;
    };
    let _ = world.db.send(DbCommand::DeletePunishment { id: task.id });
    end_effect(world, &task);
    true
}

/// A punishment's onStart effect on every affected online player (Java handler
/// `onStart`). JAIL confines, BAN disconnects, CHAT_BAN informs + refreshes the
/// chat-block icon; PARTY_BAN has no immediate effect (it is a join-time gate).
fn apply_effect(world: &mut World, task: &Punishment) {
    for oid in players_matching(world, task) {
        match task.ptype {
            PunishmentType::Jail => apply_jail_to_player(world, oid, task.expiration),
            PunishmentType::Ban => disconnect_player(world, oid),
            PunishmentType::ChatBan => apply_chatban_to_player(world, oid, task.expiration),
            PunishmentType::PartyBan => {}
        }
    }
}

/// A punishment's onEnd effect on every affected online player (Java handler
/// `onEnd`). JAIL releases, CHAT_BAN lifts the notice; BAN/PARTY_BAN do nothing.
fn end_effect(world: &mut World, task: &Punishment) {
    for oid in players_matching(world, task) {
        match task.ptype {
            PunishmentType::Jail => remove_jail_from_player(world, oid),
            PunishmentType::ChatBan => remove_chatban_from_player(world, oid),
            PunishmentType::Ban | PunishmentType::PartyBan => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Jail (Java `JailHandler`) — the type-specific wrappers stay for the admin
// layer and the login re-apply.
// ---------------------------------------------------------------------------

/// `//jail <name> [minutes]` for an online character (CHARACTER/JAIL).
/// `minutes == 0` jails forever.
pub(crate) fn jail_character(
    world: &mut World,
    char_id: i32,
    minutes: i64,
    reason: String,
    punished_by: String,
) -> bool {
    start_punishment(
        world,
        char_id.to_string(),
        PunishmentAffect::Character,
        PunishmentType::Jail,
        expiration_from_minutes(minutes),
        reason,
        punished_by,
    )
}

/// `//unjail <name>` — drop a character's jail.
pub(crate) fn unjail_character(world: &mut World, char_id: i32) -> bool {
    stop_punishment(
        world,
        &char_id.to_string(),
        PunishmentAffect::Character,
        PunishmentType::Jail,
    )
}

/// `minutes > 0` → absolute expiry stamp; `0` → forever (Java's `admin_
/// punishment_add`: `exp * 60 * 1000` added to now, `0` left as-is).
pub(crate) fn expiration_from_minutes(minutes: i64) -> i64 {
    if minutes > 0 {
        commons::util::now_millis() + minutes * 60_000
    } else {
        0
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
            .in_game_player_oids()
            .filter(|oid| {
                world
                    .objects
                    .get_component::<Player>(oid)
                    .is_some_and(|p| p.account == task.key)
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
        // HWID (G31 slice 5): match the per-connection hardware fingerprint's
        // MAC against the key.
        PunishmentAffect::Hwid => world
            .clients
            .values()
            .filter_map(|cs| match cs {
                ClientSession::InGame(s)
                    if world
                        .hwids
                        .get(&s.client_id)
                        .is_some_and(|h| h.mac_address == task.key) =>
                {
                    Some(s.player_object_id())
                }
                _ => None,
            })
            .collect(),
    }
}

/// The MAC address the player's client reported (`RequestHardWareInfo`), if any.
fn player_hwid(world: &World, object_id: i32) -> Option<String> {
    client_for_player(world, object_id)
        .and_then(|cid| world.hwids.get(&cid))
        .map(|h| h.mac_address.clone())
}

/// Java `JailHandler.applyToPlayer`: mark the player jailed, teleport them into
/// the prison, and tell them how long for. (Java delays the teleport 2 s and
/// shows `jail_in.htm`; we teleport at once and send the duration line — a
/// documented simplification, the confinement is what matters.)
fn apply_jail_to_player(world: &mut World, player_oid: i32, expiration: i64) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.jailed = true;
    }
    // `JailZone`'s `new TeleportTask(player, JAIL_IN_LOC)` scatters, so a
    // mass jailing does not stack everyone on one tile.
    crate::game_loop::death::teleport_player_scattered(
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
    crate::game_loop::death::teleport_player(
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
// Ban (Java `BanHandler`) + chat-ban (Java `ChatBanHandler`)
// ---------------------------------------------------------------------------

/// Java `BanHandler.applyToPlayer`: `Disconnection.of(player)` — the clean
/// logout teardown (persist, despawn, drop the session). The login gate at
/// character-select ([`is_banned`]) then refuses re-entry.
fn disconnect_player(world: &mut World, target: i32) {
    let Some(tcid) = client_for_player(world, target) else {
        return;
    };
    if let Some(ClientSession::InGame(session)) = world.clients.remove(&tcid) {
        crate::game_loop::net::store_and_remove_player(world, target);
        session.send(crate::network::server_packets::leave_world());
    }
}

/// Java `ChatBanHandler.applyToPlayer`: tell the player, and refresh the
/// chat-block icon (`EtcStatusUpdate`, whose mask now reads `is_chat_banned`).
fn apply_chatban_to_player(world: &mut World, player_oid: i32, expiration: i64) {
    let now = commons::util::now_millis();
    let text = if expiration > 0 {
        let secs = (expiration - now) / 1000;
        if secs > 60 {
            format!("You've been chat banned for {} minutes.", secs / 60)
        } else {
            format!("You've been chat banned for {secs} seconds.")
        }
    } else {
        "You've been chat banned forever.".to_string()
    };
    send_text(world, player_oid, &text);
    refresh_etc_status(world, player_oid);
}

/// Java `ChatBanHandler.removeFromPlayer`.
fn remove_chatban_from_player(world: &mut World, player_oid: i32) {
    send_text(world, player_oid, "Your Chat ban has been lifted");
    refresh_etc_status(world, player_oid);
}

/// Drop a character-affect punishment of `ptype` (the `//un*` commands).
pub(crate) fn stop_character_punishment(
    world: &mut World,
    char_id: i32,
    ptype: PunishmentType,
) -> bool {
    stop_punishment(
        world,
        &char_id.to_string(),
        PunishmentAffect::Character,
        ptype,
    )
}

// ---------------------------------------------------------------------------
// Enforcement predicates (Java `Player.isChatBanned` / `isPartyBanned` + the
// character-select ban gate)
// ---------------------------------------------------------------------------

/// The live IP string of a player's client (`Player.getIPAddress`), or "" when
/// offline / unmatched.
fn player_ip(world: &World, object_id: i32) -> String {
    client_for_player(world, object_id)
        .and_then(|cid| world.clients.get(&cid))
        .map(|cs| cs.addr().ip().to_string())
        .unwrap_or_default()
}

/// Java `Player.isChatBanned` — any of the character's affect keys carries an
/// active CHAT_BAN.
pub(crate) fn is_chat_banned(world: &World, object_id: i32) -> bool {
    let account = world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| p.account.clone())
        .unwrap_or_default();
    world.punishments.player_has(
        PunishmentType::ChatBan,
        object_id,
        &account,
        &player_ip(world, object_id),
        player_hwid(world, object_id).as_deref(),
    )
}

/// Java `Player.isPartyBanned` — **CHARACTER-affect only** (unlike the others).
pub(crate) fn is_party_banned(world: &World, object_id: i32) -> bool {
    world.punishments.has_punishment(
        &object_id.to_string(),
        PunishmentAffect::Character,
        PunishmentType::PartyBan,
    )
}

/// Java `CharacterSelect`'s ban gate — the chosen char id / account / IP / HWID
/// under an active BAN. Checked before a banned character can enter the world.
pub(crate) fn is_banned(
    world: &World,
    char_id: i32,
    account: &str,
    ip: &str,
    hwid: Option<&str>,
) -> bool {
    world
        .punishments
        .player_has(PunishmentType::Ban, char_id, account, ip, hwid)
}

/// The HWID-ban / HWID-jail re-check when a client's fingerprint arrives after
/// enter-world (`RequestHardWareInfo` timing is client-driven). Kicks a
/// HWID-banned session and jails a HWID-jailed one.
pub(crate) fn on_hwid_received(world: &mut World, client_id: u32) {
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    let hwid = world.hwids.get(&client_id).map(|h| h.mac_address.clone());
    let Some(hwid) = hwid else { return };
    if world
        .punishments
        .has_punishment(&hwid, PunishmentAffect::Hwid, PunishmentType::Ban)
    {
        disconnect_player(world, object_id);
        return;
    }
    if !world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(|p| p.jailed)
        && world
            .punishments
            .has_punishment(&hwid, PunishmentAffect::Hwid, PunishmentType::Jail)
    {
        apply_jail_to_player(world, object_id, 0);
    }
}

// ---------------------------------------------------------------------------
// Login re-apply + JailZone keep-in (Java `JailHandler.onPlayerLogin` /
// `JailZone.onExit`)
// ---------------------------------------------------------------------------

/// Whether any of a player's affect keys currently carries a JAIL punishment
/// (Java `Player.isJailed`). Needs the live account + IP, so it takes them
/// rather than reading them back off components.
fn is_jailed(world: &World, char_id: i32, account: &str, ip: &str, hwid: Option<&str>) -> bool {
    world
        .punishments
        .player_has(PunishmentType::Jail, char_id, account, ip, hwid)
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
    let hwid = player_hwid(world, object_id);
    let jailed = is_jailed(world, object_id, &account, &ip, hwid.as_deref());
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

    // Java `EnterWorld`: a chat-banned character logging in gets the chat-block
    // icon lit (its mask reads `is_chat_banned`).
    if is_chat_banned(world, object_id) {
        refresh_etc_status(world, object_id);
    }
}

/// Whether the player is currently standing in a JailZone.
fn in_jail_zone(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::components::space::Position>(&object_id)
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
    crate::game_loop::death::teleport_player(
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

// ---------------------------------------------------------------------------
// Illegal player actions (Java `Util.handleIllegalPlayerAction` +
// `IllegalPlayerActionTask`, G35)
// ---------------------------------------------------------------------------

/// [`handle_illegal_player_action`] at the configured severity
/// (`Config.DefaultPunish`) — what every packet-validation failure wants.
///
/// Reading the config here rather than at each call site removes the
/// `let punish = world.cfg.general.default_punish;` line that used to precede
/// all 35 of them. Reach for [`handle_illegal_player_action`] directly only to
/// pass a severity that isn't the configured default — as
/// `skills::handle_request_acquire_skill` does with
/// [`IllegalActionPunishment::None`].
pub(crate) fn illegal_action(world: &mut World, object_id: i32, message: &str) {
    let punishment = world.cfg.general.default_punish;
    handle_illegal_player_action(world, object_id, message, punishment);
}

/// Java `Util.handleIllegalPlayerAction`: warn the offender at once (and for
/// `KICKBAN`, drop their character + account access levels immediately), then
/// fire the audit/punish task 5 seconds later — Java's
/// `ThreadPool.schedule(new IllegalPlayerActionTask(...), 5000)`. The
/// constructor messages and the access-level drop happen at call time; the
/// audit record and the actual kick/ban/jail happen when the task runs.
pub(crate) fn handle_illegal_player_action(
    world: &mut World,
    object_id: i32,
    message: &str,
    punishment: IllegalActionPunishment,
) {
    let is_gm = world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(|p| p.is_gm(&world.data));
    match punishment {
        IllegalActionPunishment::Kick => {
            send_text(
                world,
                object_id,
                "You will be kicked for illegal action, GM informed.",
            );
        }
        IllegalActionPunishment::KickBan => {
            if !is_gm {
                let account = world
                    .objects
                    .get_component_mut::<Player>(&object_id)
                    .map(|p| {
                        p.access_level = -1;
                        p.account.clone()
                    });
                let _ = world.db.send(DbCommand::SetAccessLevel {
                    char_id: object_id,
                    level: -1,
                });
                if let Some(account) = account {
                    let _ = world.login.link.send(
                        crate::loginlink::LoginLinkCommand::SetAccountAccessLevel {
                            account,
                            level: -1,
                        },
                    );
                }
            }
            send_text(
                world,
                object_id,
                "You are banned for illegal action, GM informed.",
            );
        }
        IllegalActionPunishment::Jail => {
            send_text(world, object_id, "Illegal action performed!");
            send_text(
                world,
                object_id,
                "You will be teleported to GM Consultation Service area and jailed.",
            );
        }
        IllegalActionPunishment::None | IllegalActionPunishment::Broadcast => {}
    }
    world.scheduler.schedule(
        world.tick + 5 * TICKS_PER_SECOND,
        ScheduledTask::IllegalActionPunish {
            object_id,
            message: message.to_string(),
            punishment,
        },
    );
}

/// The 5-second task body (Java `IllegalPlayerActionTask.run`): write the
/// audit record — always, GM or not — then apply the punishment to a non-GM.
/// The offender may have logged out during the delay; the ban/jail punishment
/// rows are keyed by character id and land regardless, the kick just no-ops.
pub(crate) fn on_illegal_action_punish(
    world: &mut World,
    object_id: i32,
    message: &str,
    punishment: IllegalActionPunishment,
) {
    let (char_name, account, is_gm) = world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| (p.name.clone(), p.account.clone(), p.is_gm(&world.data)))
        .unwrap_or_default();
    commons::audit::record(
        commons::audit::Category::Illegal,
        serde_json::json!({
            "char_name": char_name,
            "oid": object_id,
            "account": account,
            "message": message,
            "punishment": punishment.as_str(),
        }),
    );
    if is_gm {
        return;
    }
    let expiration =
        commons::util::now_millis() + world.cfg.general.default_punish_param.max(1) * 1000;
    match punishment {
        IllegalActionPunishment::None => {}
        IllegalActionPunishment::Broadcast => {
            // Java `AdminData.broadcastMessageToGMs` — a plain text line to
            // every online GM.
            let gms: Vec<i32> = world
                .in_game_player_oids()
                .filter(|oid| {
                    world
                        .objects
                        .get_component::<Player>(oid)
                        .is_some_and(|p| p.is_gm(&world.data))
                })
                .collect();
            for gm in gms {
                send_text(world, gm, message);
            }
        }
        IllegalActionPunishment::Kick => disconnect_player(world, object_id),
        IllegalActionPunishment::KickBan => {
            start_punishment(
                world,
                object_id.to_string(),
                PunishmentAffect::Character,
                PunishmentType::Ban,
                expiration,
                message.to_string(),
                "IllegalPlayerActionTask".to_string(),
            );
            disconnect_player(world, object_id);
        }
        IllegalActionPunishment::Jail => {
            start_punishment(
                world,
                object_id.to_string(),
                PunishmentAffect::Character,
                PunishmentType::Jail,
                expiration,
                message.to_string(),
                "IllegalPlayerActionTask".to_string(),
            );
        }
    }
}

/// Send a plain-text system line to a player's client (Java `sendMessage`).
fn send_text(world: &World, player_oid: i32, text: &str) {
    send_sm_to_player(
        world,
        player_oid,
        sm_ids::S1_TEXT,
        &[SmParam::Text(text.to_string())],
    );
}

/// Redraw the chat-block icon (Java `sendPacket(new EtcStatusUpdate(this))`) —
/// its mask bit reads `is_chat_banned`, so a chat-ban change must resend it.
fn refresh_etc_status(world: &World, player_oid: i32) {
    if let Some(cid) = client_for_player(world, player_oid) {
        crate::game_loop::helpers::send_etc_status_update(world, cid, player_oid);
    }
}
