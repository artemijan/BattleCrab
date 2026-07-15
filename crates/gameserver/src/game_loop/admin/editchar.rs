//! `AdminEditChar` breadth — the character info/search, rename, party info,
//! pvp-flag and clan-penalty subcommands whose backing state exists in the
//! port. Java renders most of these as HTML windows; here the info/search
//! commands answer as text lines (the documented G13 simplification also used
//! by `//serverinfo`/`//getbuffs`). The pet/summon subcommands (`//fullfood`,
//! `//summon_info`, `//show_pet_inv`, `//summon_setlvl`, `//unsummon`), the
//! IP/dualbox tools (`//find_ip`, `//find_dualbox`, `//tracert` — no per-client
//! IP is tracked), `//setparam`/`//unsetparam` (no fixed-stat API) and
//! `//setnoble`/`//rec` (fields not modelled) stay on the not-implemented path.

use crate::model::components::{PartyRef, Position, PvpState, Vitals};
use crate::model::Player;
use crate::session::ClientSession;
use crate::world::World;

use super::{current_target, find_online_player, send_message, send_sm};
use crate::network::server_packets::sm_ids;

/// Object ids of every in-game player (Java `World.getPlayers()`), name-sorted
/// for stable listing.
fn online_players(world: &World) -> Vec<i32> {
    let mut ids: Vec<i32> = world
        .clients
        .values()
        .filter_map(|cs| match cs {
            ClientSession::InGame(s) => Some(s.player_object_id()),
            _ => None,
        })
        .collect();
    ids.sort_by_key(|oid| world.objects.get_component::<Player>(oid).map(|p| p.name.clone()).unwrap_or_default());
    ids
}

/// `//current_player` / `//character_info [name]` — dump a player's key fields.
pub(super) fn admin_character_info(world: &mut World, client_id: u32, object_id: i32, args: &[&str], self_only: bool) {
    let target = if self_only {
        object_id
    } else if let Some(name) = args.first() {
        match find_online_player(world, name) {
            Some(t) => t,
            None => {
                send_sm(world, client_id, sm_ids::INVALID_TARGET);
                return;
            }
        }
    } else {
        match current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid)) {
            Some(t) => t,
            None => {
                send_sm(world, client_id, sm_ids::INVALID_TARGET);
                return;
            }
        }
    };
    let Some(p) = world.objects.get_component::<Player>(&target).cloned() else { return };
    let pos = world.objects.get_component::<Position>(&target).copied().unwrap_or(Position { x: 0, y: 0, z: 0, heading: 0 });
    send_message(world, client_id, &format!("=== {} ===", p.name));
    send_message(world, client_id, &format!("Account: {}  Level: {}  Class: {}", p.account, p.level, p.class_id));
    send_message(world, client_id, &format!("XP: {}  SP: {}", p.exp, p.sp));
    send_message(world, client_id, &format!("Reputation: {}  Fame: {}  PvP: {}  PK: {}", p.reputation, p.fame, p.pvp_kills, p.pk_kills));
    send_message(world, client_id, &format!("Clan: {}  Loc: {},{},{}", p.clan_id, pos.x, pos.y, pos.z));
}

/// `//character_list` / `//show_characters <page>` — paginated online roster
/// (20 per page, matching Java's page size).
pub(super) fn admin_character_list(world: &mut World, client_id: u32, args: &[&str]) {
    const PER_PAGE: usize = 20;
    let page = args.first().and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    let ids = online_players(world);
    let pages = ids.len().div_ceil(PER_PAGE).max(1);
    let page = page.min(pages.saturating_sub(1));
    send_message(world, client_id, &format!("=== Online players ({}) — page {}/{} ===", ids.len(), page + 1, pages));
    for oid in ids.iter().skip(page * PER_PAGE).take(PER_PAGE) {
        if let Some(p) = world.objects.get_component::<Player>(oid) {
            send_message(world, client_id, &format!("  {} (Lv {}, class {})", p.name, p.level, p.class_id));
        }
    }
}

/// `//find_character <name>` — case-insensitive substring match over the online
/// roster.
pub(super) fn admin_find_character(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(needle) = args.first().map(|s| s.to_lowercase()) else {
        send_message(world, client_id, "Usage: //find_character <character_name>");
        return;
    };
    let ids = online_players(world);
    let matches: Vec<String> = ids
        .iter()
        .filter_map(|oid| world.objects.get_component::<Player>(oid))
        .filter(|p| p.name.to_lowercase().contains(&needle))
        .map(|p| p.name.clone())
        .collect();
    send_message(world, client_id, &format!("Characters found: {}", matches.len()));
    for name in matches {
        send_message(world, client_id, &format!("  {name}"));
    }
}

/// `//find_account <name>` — list every online character on the named player's
/// account (Java also lists offline chars from the DB; online-only here).
pub(super) fn admin_find_account(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(name) = args.first() else {
        send_message(world, client_id, "Usage: //find_account <player_name>");
        return;
    };
    let Some(target) = find_online_player(world, name) else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    let Some(account) = world.objects.get_component::<Player>(&target).map(|p| p.account.clone()) else { return };
    send_message(world, client_id, &format!("=== Account '{account}' (online) ==="));
    for oid in online_players(world) {
        if let Some(p) = world.objects.get_component::<Player>(&oid) {
            if p.account == account {
                send_message(world, client_id, &format!("  {} (Lv {})", p.name, p.level));
            }
        }
    }
}

/// `//edit_character [name]` — Java opens the char-edit HTML panel; we route to
/// the admin `charedit.htm` (falls back to the retail missing-text placeholder).
pub(super) fn admin_edit_character(world: &mut World, client_id: u32) {
    super::menu::show_admin_html(world, client_id, "charedit.htm");
}

/// `//changename <newname>` — rename the targeted player. Rejected if the name
/// collides with an *online* player (Java also checks the offline name table).
/// The rename lives in memory (persisted on the next flush, per the memory-first
/// model) and rebroadcasts UserInfo/CharInfo.
pub(super) fn admin_changename(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(new_name) = args.first().map(|s| s.to_string()) else {
        send_message(world, client_id, "Usage: //changename <new_name>");
        return;
    };
    let Some(target) = current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid)) else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    if find_online_player(world, &new_name).is_some() {
        send_message(world, client_id, &format!("Warning, player {new_name} already exists"));
        return;
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.name = new_name.clone();
    }
    super::party::broadcast_user_info(world, target);
    // CharInfo to nearby so the new name shows on other clients too.
    super::visibility::update_region(world, target);
    send_message(world, client_id, &format!("Changed name to {new_name}"));
}

/// `//set_pvp_flag` — toggle the target playable's PvP flag (Java
/// `updatePvPFlag(abs(flag - 1))`).
pub(super) fn admin_set_pvp_flag(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid)) else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    let cur = world.objects.get_component::<PvpState>(&target).map_or(0, |s| s.flag);
    let next = (cur as i32 - 1).unsigned_abs() as u8;
    crate::game_loop::pvp::update_pvp_flag(world, target, next);
}

/// `//partyinfo [name]` — list the target player's party roster (Java opens an
/// HTML window; text here).
pub(super) fn admin_partyinfo(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let target = match args.first().and_then(|n| find_online_player(world, n)) {
        Some(t) => t,
        None => match current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid)) {
            Some(t) => t,
            None => {
                send_sm(world, client_id, sm_ids::INVALID_TARGET);
                return;
            }
        },
    };
    let Some(PartyRef(pid)) = world.objects.get_component::<PartyRef>(&target).copied() else {
        send_message(world, client_id, "Not in party.");
        return;
    };
    let members = world.parties.get(&pid).map(|p| p.members.clone()).unwrap_or_default();
    send_message(world, client_id, &format!("=== Party ({} members) ===", members.len()));
    for oid in members {
        if let Some(p) = world.objects.get_component::<Player>(&oid) {
            let hp = world.objects.get_component::<Vitals>(&oid).map_or(0, |v| v.cur_hp as i32);
            send_message(world, client_id, &format!("  {} (Lv {}, HP {})", p.name, p.level, hp));
        }
    }
}

/// `AdminEditChar`'s `//setparam <stat> <value>` / `//unsetparam <stat>` — set
/// or clear a fixed-value override on one of the target's combat stats (Java
/// `CreatureStat.addFixedValue`/`removeFixedValue`), then recompute + broadcast.
/// `maxHp`/`maxMp` compute on a separate path and aren't overridable here.
pub(super) fn admin_setparam(world: &mut World, client_id: u32, object_id: i32, args: &[&str], set: bool) {
    let Some(stat) = args.first().and_then(|s| stat_from_name(s)) else {
        send_message(world, client_id, if set { "Syntax: //setparam <stat> <value>" } else { "Syntax: //unsetparam <stat>" });
        if args.first().is_some() {
            send_message(world, client_id, "Couldn't find such stat!");
        }
        return;
    };
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<crate::model::components::StatModifiers>(oid))
        .unwrap_or(object_id);
    if set {
        let Some(value) = args.get(1).and_then(|s| s.parse::<f64>().ok()) else {
            send_message(world, client_id, "Syntax: //setparam <stat> <value>");
            return;
        };
        if let Some(m) = world.objects.get_component_mut::<crate::model::components::StatModifiers>(&target) {
            m.fixed.insert(stat, value);
        }
        send_message(world, client_id, &format!("Fixed stat {} set to {value}.", args[0]));
    } else {
        if let Some(m) = world.objects.get_component_mut::<crate::model::components::StatModifiers>(&target) {
            m.fixed.remove(&stat);
        }
        send_message(world, client_id, &format!("Fixed stat {} has been removed.", args[0]));
    }
    recompute_combat_stats(world, target);
    super::party::broadcast_user_info(world, target);
}

/// Map a `//setparam` stat token (the XML `getValue()` name) to the engine
/// [`Stat`]. Only the combat stats the finalizers compute are settable.
fn stat_from_name(name: &str) -> Option<crate::model::stats::Stat> {
    use crate::model::stats::Stat;
    Some(match name {
        "pAtk" => Stat::PhysicalAttack,
        "pDef" => Stat::PhysicalDefence,
        "mAtk" => Stat::MagicalAttack,
        "mDef" => Stat::MagicalDefence,
        "pAtkSpd" => Stat::PhysicalAttackSpeed,
        "mAtkSpd" => Stat::MagicAttackSpeed,
        "rCrit" => Stat::CriticalRate,
        "mCritRate" => Stat::MagicCriticalRate,
        "accCombat" => Stat::AccuracyCombat,
        "accMagic" => Stat::AccuracyMagic,
        "rEvas" => Stat::EvasionRate,
        "mEvas" => Stat::MagicEvasionRate,
        "runSpd" => Stat::RunSpeed,
        "walkSpd" => Stat::WalkSpeed,
        _ => return None,
    })
}

/// Re-run `recalculate_stats` (which folds `StatModifiers.fixed`) and push the
/// new `CombatStats`/`Speeds` into the entity.
fn recompute_combat_stats(world: &mut World, target: i32) {
    use crate::model::components::{BaseStats, CombatStats, Speeds, StatModifiers};
    use crate::model::inventory::Inventory;
    let data = &world.data;
    if let Some((p, base, mods, inventory, mut speeds, mut combat)) =
        world.objects.get_many_mut::<(&Player, &BaseStats, &StatModifiers, &Inventory, &mut Speeds, &mut CombatStats)>(&target)
    {
        p.recalculate_stats(data, &base, &mods, &inventory, &mut speeds, &mut combat);
    }
}

/// `//remove_clan_penalty create|join <name>` — clear a clan cooldown. Only the
/// `create` cooldown is modelled (`clan_create_expiry_time`); `join` reports it
/// isn't tracked. Applies to an online target in memory.
pub(super) fn admin_remove_clan_penalty(world: &mut World, client_id: u32, args: &[&str]) {
    let [kind, name] = args else {
        send_message(world, client_id, "Usage: //remove_clan_penalty join|create charname");
        return;
    };
    let is_create = kind.eq_ignore_ascii_case("create");
    if !is_create {
        send_message(world, client_id, "The clan join penalty is not tracked in this build.");
        return;
    }
    let Some(target) = find_online_player(world, name) else {
        send_message(world, client_id, &format!("Player '{name}' is not online."));
        return;
    };
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.clan_create_expiry_time = 0;
    }
    send_message(world, client_id, &format!("Clan penalty successfully removed to character: {name}"));
}
