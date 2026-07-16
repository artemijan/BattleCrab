//! `AdminCastle` — the Game panel's "Castle" button (`//castlemanage`). The
//! castle roster + per-castle status page, the ownership actions (set/take
//! owner, switch side), and the siege *registration + state* actions
//! (add/remove attacker/defender, start/stop siege) over the
//! [`crate::model::siege`] slice. The siege *combat* — control towers, flags,
//! guards, zone PvP, teleport, the timed window and ownership-on-victory, plus
//! the `SiegeInfo` registration window — is a later milestone (TODO(G24)).

use crate::db::DbCommand;
use crate::model::castle::CastleSide;
use crate::model::siege::SiegeClanType;
use crate::model::Player;
use crate::network::server_packets::sm_ids;
use crate::world::World;

use super::{current_target, send_message, send_sm};

/// Resolve the `<castleId[1-9] | castleName>` argument to an index into
/// `world.castles` (Java: digit → `getCastleById`, else `getCastle(name)`).
fn resolve(world: &World, param: &str) -> Option<usize> {
    if param.chars().all(|c| c.is_ascii_digit()) {
        let id: i32 = param.parse().ok()?;
        world.castles.iter().position(|c| c.id == id)
    } else {
        world.castles.iter().position(|c| c.name.eq_ignore_ascii_case(param))
    }
}

/// The clan id that owns `castle_id` (the clan whose `castle_id` matches), or
/// `None` for an unowned castle.
fn owner_clan(world: &World, castle_id: i32) -> Option<i32> {
    world.clans.values().find(|c| c.castle_id == castle_id).map(|c| c.id)
}

/// Java `checkTarget`: the GM's target is a player that belongs to a clan.
fn clanned_target(world: &World, gm_object_id: i32) -> Option<i32> {
    let target = current_target(world, gm_object_id)?;
    world.objects.get_component::<Player>(&target).filter(|p| p.clan_id != 0).map(|_| target)
}

/// `//castlemanage` — no arg opens the roster; `<id|name>` shows one castle's
/// page; `<id|name> <action> …` runs an action.
pub(super) fn admin_castlemanage(world: &mut World, client_id: u32, gm_object_id: i32, args: &[&str]) {
    let Some(param) = args.first() else {
        super::menu::show_admin_html(world, client_id, "castlemanage.htm");
        return;
    };
    let Some(idx) = resolve(world, param) else {
        send_message(world, client_id, "Invalid parameters! Usage: //castlemanage <castleId[1-9] / castleName>");
        return;
    };
    match args.get(1) {
        None => show_castle_menu(world, client_id, idx),
        Some(&action) => castle_action(world, client_id, gm_object_id, idx, action, &args[2..]),
    }
}

/// Java `showCastleMenu`: render `castlemanage_selected.htm` for one castle.
fn show_castle_menu(world: &mut World, client_id: u32, idx: usize) {
    let (castle_id, name, side) = {
        let c = &world.castles[idx];
        (c.id, c.name.clone(), c.side)
    };
    let (owner_name, owner_clan) = match owner_clan(world, castle_id).and_then(|cid| world.clans.get(&cid)) {
        Some(clan) => (clan.leader_name().to_string(), clan.name.clone()),
        None => ("NPC".to_string(), "NPC".to_string()),
    };
    let r: Vec<(&str, String)> = vec![
        ("castleId", castle_id.to_string()),
        ("castleName", name),
        ("ownerName", owner_name),
        ("ownerClan", owner_clan),
        ("castleSide", side.display().to_string()),
    ];
    super::menu::show_admin_html_replace(world, client_id, "castlemanage_selected.htm", &r);
}

fn castle_action(world: &mut World, client_id: u32, gm_object_id: i32, idx: usize, action: &str, rest: &[&str]) {
    match action {
        "switchSide" => switch_side(world, client_id, idx),
        "setOwner" => set_owner(world, client_id, gm_object_id, idx, rest),
        "takeCastle" => take_castle(world, client_id, idx),
        // --- Siege registration + state (AdminCastle's siege actions) ---
        "showRegWindow" => show_reg_window(world, client_id, idx),
        "addAttacker" => siege_register(world, client_id, gm_object_id, idx, true),
        "addDeffender" => siege_register(world, client_id, gm_object_id, idx, false),
        // Java quirk: removeAttacker strips the *GM's own* clan, removeDeffender
        // the target's (both go through `removeSiegeClan`, gated on a clanned
        // target).
        "removeAttacker" => siege_remove(world, client_id, gm_object_id, idx, true),
        "removeDeffender" => siege_remove(world, client_id, gm_object_id, idx, false),
        "startSiege" => start_siege(world, client_id, idx),
        "stopSiege" => stop_siege(world, client_id, idx),
        _ => {}
    }
}

/// Java `switchSide`: DARK↔LIGHT (a neutral castle can't switch).
fn switch_side(world: &mut World, client_id: u32, idx: usize) {
    let next = match world.castles[idx].side {
        CastleSide::Dark => Some(CastleSide::Light),
        CastleSide::Light => Some(CastleSide::Dark),
        CastleSide::Neutral => None,
    };
    match next {
        Some(side) => {
            world.castles[idx].side = side;
            let castle_id = world.castles[idx].id;
            let _ = world.db.send(DbCommand::UpdateCastleSide { castle_id, side: side.as_db().to_string() });
        }
        None => send_message(world, client_id, "You can't switch sides when is castle neutral!"),
    }
    show_castle_menu(world, client_id, idx);
}

/// Java `setOwner`: give the castle to the targeted clanned player's clan (on a
/// chosen side). Residential skills / crests / siege bookkeeping are deferred
/// (TODO(G24)); this sets the ownership + side the panel displays.
fn set_owner(world: &mut World, client_id: u32, gm_object_id: i32, idx: usize, rest: &[&str]) {
    let castle_id = world.castles[idx].id;
    let Some(target) = clanned_target(world, gm_object_id) else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        show_castle_menu(world, client_id, idx);
        return;
    };
    let target_clan_id = world.objects.get_component::<Player>(&target).map(|p| p.clan_id).unwrap_or(0);
    if world.clans.get(&target_clan_id).is_some_and(|c| c.castle_id > 0) {
        send_message(world, client_id, "This clan already have castle!");
        show_castle_menu(world, client_id, idx);
        return;
    }
    if owner_clan(world, castle_id).is_some() {
        send_message(world, client_id, "This castle is already taken by another clan!");
        show_castle_menu(world, client_id, idx);
        return;
    }
    let Some(side) = rest.first().and_then(|s| CastleSide::from_str(s)) else {
        send_message(world, client_id, "Invalid parameters!!");
        show_castle_menu(world, client_id, idx);
        return;
    };
    // castle.setSide(side) + castle.setOwner(clan).
    world.castles[idx].side = side;
    let _ = world.db.send(DbCommand::UpdateCastleSide { castle_id, side: side.as_db().to_string() });
    if let Some(clan) = world.clans.get_mut(&target_clan_id) {
        clan.castle_id = castle_id;
    }
    let _ = world.db.send(DbCommand::UpdateClanCastle { clan_id: target_clan_id, castle_id });
    // TODO(G24): grant residential skills to the clan, set the castle crest,
    // and the siege-side bookkeeping (Castle.setOwner).
    show_castle_menu(world, client_id, idx);
}

/// Java `takeCastle`: strip the current owner (`Castle.removeOwner`).
fn take_castle(world: &mut World, client_id: u32, idx: usize) {
    let castle_id = world.castles[idx].id;
    match owner_clan(world, castle_id) {
        Some(clan_id) => {
            if let Some(clan) = world.clans.get_mut(&clan_id) {
                clan.castle_id = 0;
            }
            let _ = world.db.send(DbCommand::UpdateClanCastle { clan_id, castle_id: 0 });
            // TODO(G24): remove residential skills + crest (Castle.removeOwner).
        }
        None => send_message(world, client_id, "Error during removing castle!"),
    }
    show_castle_menu(world, client_id, idx);
}

/// The clan id of a player (0 = none).
fn clan_of(world: &World, oid: i32) -> i32 {
    world.objects.get_component::<Player>(&oid).map(|p| p.clan_id).unwrap_or(0)
}

/// `showRegWindow` → Java `Siege.listRegisterClan` (the `SiegeInfo` window).
/// TODO(G24): port `SiegeInfo`; a roster summary stands in for the GM.
fn show_reg_window(world: &mut World, client_id: u32, idx: usize) {
    let castle_id = world.castles[idx].id;
    let name = world.castles[idx].name.clone();
    let summary = world.sieges.get(&castle_id).map(|s| s.summary()).unwrap_or_default();
    send_message(world, client_id, &format!("Siege registration for {name}: {summary}"));
}

/// `addAttacker`/`addDeffender` → `registerAttacker`/`registerDefender(target,
/// force=true)`. Both require a clanned target (Java `checkTarget`).
fn siege_register(world: &mut World, client_id: u32, gm_object_id: i32, idx: usize, attacker: bool) {
    let Some(target) = clanned_target(world, gm_object_id) else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    let castle_id = world.castles[idx].id;
    let clan_id = clan_of(world, target);

    // registerDefender: only when the castle has an owner.
    if !attacker && owner_clan(world, castle_id).is_none() {
        let name = world.castles[idx].name.clone();
        send_message(world, client_id, &format!("You cannot register as a defender because {name} is owned by NPC."));
        return;
    }
    // force: already registered → "you have already requested a castle siege".
    if world.sieges.get(&castle_id).is_some_and(|s| s.is_registered(clan_id)) {
        send_sm(world, client_id, sm_ids::YOU_HAVE_ALREADY_REQUESTED_A_CASTLE_SIEGE);
        return;
    }
    // saveSiegeClan: a clan that already owns a castle can't register (silent).
    if world.clans.get(&clan_id).is_some_and(|c| c.castle_id > 0) {
        return;
    }
    let kind = if attacker { SiegeClanType::Attacker } else { SiegeClanType::DefenderPending };
    if let Some(siege) = world.sieges.get_mut(&castle_id) {
        siege.add_clan(clan_id, kind);
    }
    let _ = world.db.send(DbCommand::SaveSiegeClan { castle_id, clan_id, kind: kind.as_db() });
}

/// `removeAttacker`/`removeDeffender` → `removeSiegeClan`. Requires a clanned
/// target; `attacker` removes the GM's *own* clan, else the target's (Java quirk).
fn siege_remove(world: &mut World, client_id: u32, gm_object_id: i32, idx: usize, attacker: bool) {
    let Some(target) = clanned_target(world, gm_object_id) else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    let castle_id = world.castles[idx].id;
    let clan_id = if attacker { clan_of(world, gm_object_id) } else { clan_of(world, target) };
    if clan_id <= 0 {
        return; // Java `removeSiegeClan(clanId <= 0)` → no-op.
    }
    if let Some(siege) = world.sieges.get_mut(&castle_id) {
        siege.remove_clan(clan_id);
    }
    let _ = world.db.send(DbCommand::RemoveSiegeClan { castle_id, clan_id });
}

/// `startSiege` → `Siege.startSiege` (needs a registered attacker); the lifecycle
/// (announce, auto-end schedule) lives in [`crate::game_loop::siege`].
fn start_siege(world: &mut World, client_id: u32, idx: usize) {
    let castle_id = world.castles[idx].id;
    if world.sieges.get(&castle_id).is_some_and(|s| s.has_attackers()) {
        crate::game_loop::siege::start_siege(world, castle_id);
    } else {
        send_message(world, client_id, "There is currently not registered any clan for castle siege!");
    }
}

/// `stopSiege` → `Siege.endSiege` (only while in progress).
fn stop_siege(world: &mut World, client_id: u32, idx: usize) {
    let castle_id = world.castles[idx].id;
    if world.sieges.get(&castle_id).is_some_and(|s| s.in_progress) {
        crate::game_loop::siege::end_siege(world, castle_id);
    } else {
        send_message(world, client_id, "Castle siege is not currently in progress!");
    }
    show_castle_menu(world, client_id, idx);
}
