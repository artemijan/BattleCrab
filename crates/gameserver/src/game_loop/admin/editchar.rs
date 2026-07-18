//! `AdminEditChar` breadth — the character info/search, rename, party info,
//! pvp-flag and clan-penalty subcommands whose backing state exists in the
//! port. The info/search commands (`//character_info`, `//character_list`/
//! `//show_characters`, `//find_character`, `//edit_character`) render the same
//! HTML windows as Java (`charinfo`/`charlist`/`charfind`/`charedit.htm`), with
//! stats not yet computed in the port defaulted. `//fullfood` is wired as a
//! pet-blocked stub (see [`admin_fullfood`]); the other pet/summon subcommands
//! (`//summon_info`, `//show_pet_inv`, `//summon_setlvl`, `//unsummon`), the
//! IP/dualbox tools (`//find_ip`, `//find_dualbox`, `//tracert` — no per-client
//! IP is tracked), `//setparam`/`//unsetparam` (no fixed-stat API) and
//! `//setnoble`/`//rec` (fields not modelled) stay on the not-implemented path.

use crate::model::components::{CombatStats, PartyRef, PlayerVitals, Position, PvpState, Speeds, Vitals};
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
/// `AdminEditChar`'s `//fullfood` — fill the targeted *pet*'s food bar. Pets are
/// not modelled yet (G29), so no target can resolve as a pet; this always replies
/// `INVALID_TARGET`, matching Java's non-pet-target branch.
pub(super) fn admin_fullfood(world: &mut World, client_id: u32) {
    // TODO(G29): when pets/summons exist, if the current target is a pet, set
    // its CurrentFed to MaxFed and broadcast a StatusUpdate (Java
    // `targetPet.setCurrentFed(targetPet.getMaxFed()); targetPet.broadcastStatusUpdate()`).
    send_sm(world, client_id, sm_ids::INVALID_TARGET);
}

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
    let vit = world.objects.get_component::<Vitals>(&target).copied().unwrap_or(Vitals { max_hp: 0, cur_hp: 0.0, max_mp: 0, cur_mp: 0.0, dead: false });
    let cp = world.objects.get_component::<PlayerVitals>(&target).copied().unwrap_or(PlayerVitals { max_cp: 0, cur_cp: 0.0 });
    let cs = world.objects.get_component::<CombatStats>(&target).copied().unwrap_or_default();
    let spd = world.objects.get_component::<Speeds>(&target).map(|s| s.run_spd).unwrap_or(0.0);
    let pvp_flag = world.objects.get_component::<PvpState>(&target).map(|s| s.flag).unwrap_or(0);
    let clan = if p.clan_id == 0 { "None".to_string() } else { p.clan_id.to_string() };
    // Fill `charinfo.htm`. Stats not computed in the port yet (regen/load/hwid/
    // ai/instance) default to `0`/`N/A`; class is the numeric id (no client-code
    // table ported).
    let r: Vec<(&str, String)> = vec![
        ("name", p.name.clone()),
        ("account", p.account.clone()),
        ("ip", "N/A".into()),
        ("hwid", "N/A".into()),
        ("protocol", "0".into()),
        ("level", p.level.to_string()),
        ("class", p.class_id.to_string()),
        ("baseclass", p.base_class_id.to_string()),
        ("xp", p.exp.to_string()),
        ("sp", p.sp.to_string()),
        ("reputation", p.reputation.to_string()),
        ("pvpflag", pvp_flag.to_string()),
        ("pvpkills", p.pvp_kills.to_string()),
        ("pkkills", p.pk_kills.to_string()),
        ("clan", clan),
        ("noblesse", "false".into()),
        ("currenthp", (vit.cur_hp as i64).to_string()),
        ("maxhp", vit.max_hp.to_string()),
        ("currentmp", (vit.cur_mp as i64).to_string()),
        ("maxmp", vit.max_mp.to_string()),
        ("currentcp", (cp.cur_cp as i64).to_string()),
        ("maxcp", cp.max_cp.to_string()),
        ("patk", (cs.p_atk as i64).to_string()),
        ("pdef", (cs.p_def as i64).to_string()),
        ("matk", (cs.m_atk as i64).to_string()),
        ("mdef", (cs.m_def as i64).to_string()),
        ("patkspd", cs.p_atk_spd.to_string()),
        ("matkspd", cs.m_atk_spd.to_string()),
        ("critical", (cs.crit_hit as i64).to_string()),
        ("accuracy", cs.accuracy.to_string()),
        ("evasion", cs.evasion.to_string()),
        ("runspeed", (spd as i64).to_string()),
        ("hpregen", "0".into()),
        ("mpregen", "0".into()),
        ("cpregen", "0".into()),
        ("currentload", "0".into()),
        ("maxload", "0".into()),
        ("ai", "N/A".into()),
        ("inst", "0".into()),
        ("x", pos.x.to_string()),
        ("y", pos.y.to_string()),
        ("z", pos.z.to_string()),
        ("heading", pos.heading.to_string()),
    ];
    super::menu::show_admin_html_replace(world, client_id, "charinfo.htm", &r);
}

/// One `charlist`/`charfind` table row (Java `listCharacters`/`findCharacter`
/// body): a name link to `admin_character_info`, class id and level.
fn char_row(name: &str, class_id: i32, level: i32) -> String {
    format!(
        "<tr><td width=80><a action=\"bypass -h admin_character_info {name}\">{name}</a></td>\
         <td width=110>{class_id}</td><td width=40>{level}</td></tr>"
    )
}

/// `//character_list` / `//show_characters <page>` — the paginated online roster
/// as `charlist.htm` (Java `listCharacters`, 20 per page).
pub(super) fn admin_character_list(world: &mut World, client_id: u32, args: &[&str]) {
    const PER_PAGE: usize = 20;
    let page = args.first().and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    let ids = online_players(world);
    let pages = ids.len().div_ceil(PER_PAGE).max(1);
    let page = page.min(pages.saturating_sub(1));
    let players: String = ids
        .iter()
        .skip(page * PER_PAGE)
        .take(PER_PAGE)
        .filter_map(|oid| world.objects.get_component::<Player>(oid))
        .map(|p| char_row(&p.name, p.class_id, p.level))
        .collect();
    // Pager (Java `PageBuilder`): a link per page when there's more than one.
    let pager = if pages > 1 {
        (0..pages)
            .map(|i| format!("<td><a action=\"bypass -h admin_show_characters {i}\">{}</a></td>", i + 1))
            .collect::<String>()
    } else {
        String::new()
    };
    let pages_block = if pager.is_empty() {
        String::new()
    } else {
        format!("<table width=280 cellspacing=0><tr>{pager}</tr></table>")
    };
    super::menu::show_admin_html_replace(
        world,
        client_id,
        "charlist.htm",
        &[("players", players), ("pages", pages_block)],
    );
}

/// `//find_character <name>` — case-insensitive substring match rendered as
/// `charfind.htm` (Java `findCharacter`, capped at 20 rows).
pub(super) fn admin_find_character(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(needle) = args.first().map(|s| s.to_lowercase()) else {
        // Java: empty name → usage sysmsg, then the full character list.
        send_message(world, client_id, "Usage: //find_character <character_name>");
        admin_character_list(world, client_id, &[]);
        return;
    };
    let ids = online_players(world);
    let mut rows = String::new();
    let mut found = 0;
    for p in ids.iter().filter_map(|oid| world.objects.get_component::<Player>(oid)) {
        if p.name.to_lowercase().contains(&needle) {
            rows.push_str(&char_row(&p.name, p.class_id, p.level));
            found += 1;
            if found > 20 {
                break;
            }
        }
    }
    super::menu::show_admin_html_replace(
        world,
        client_id,
        "charfind.htm",
        &[("results", rows), ("number", found.to_string()), ("end", String::new())],
    );
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
/// `//edit_character` — the `charedit.htm` field-editor for the current target
/// (Java `editCharacter`). Falls back to `INVALID_TARGET` with no player target.
pub(super) fn admin_edit_character(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid)) else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    let Some(p) = world.objects.get_component::<Player>(&target).cloned() else { return };
    let vit = world.objects.get_component::<Vitals>(&target).copied().unwrap_or(Vitals { max_hp: 0, cur_hp: 0.0, max_mp: 0, cur_mp: 0.0, dead: false });
    let cp = world.objects.get_component::<PlayerVitals>(&target).copied().unwrap_or(PlayerVitals { max_cp: 0, cur_cp: 0.0 });
    let percent = if vit.max_hp > 0 { (vit.cur_hp / vit.max_hp as f64 * 100.0) as i64 } else { 0 };
    let r: Vec<(&str, String)> = vec![
        ("name", p.name.clone()),
        ("access", p.access_level.to_string()),
        ("class", p.class_id.to_string()),
        ("currenthp", (vit.cur_hp as i64).to_string()),
        ("maxhp", vit.max_hp.to_string()),
        ("currentmp", (vit.cur_mp as i64).to_string()),
        ("maxmp", vit.max_mp.to_string()),
        ("currentcp", (cp.cur_cp as i64).to_string()),
        ("maxcp", cp.max_cp.to_string()),
        ("currentload", "0".into()),
        ("maxload", "0".into()),
        ("percent", percent.to_string()),
        ("reputation", p.reputation.to_string()),
        ("pvpkills", p.pvp_kills.to_string()),
        ("pkkills", p.pk_kills.to_string()),
        ("noblesse", "false".into()),
    ];
    super::menu::show_admin_html_replace(world, client_id, "charedit.htm", &r);
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
    let target_name = world.objects.get_component::<Player>(&target).map(|p| p.name.clone()).unwrap_or_default();
    let Some(PartyRef(pid)) = world.objects.get_component::<PartyRef>(&target).copied() else {
        // Java: not-in-party still opens the window (empty party table).
        super::menu::show_admin_html_replace(world, client_id, "partyinfo.htm", &[("player", target_name), ("party", String::new())]);
        return;
    };
    let members = world.parties.get(&pid).map(|p| p.members.clone()).unwrap_or_default();
    let mut rows = String::new();
    for oid in members {
        if let Some(p) = world.objects.get_component::<Player>(&oid) {
            rows.push_str(&format!(
                "<tr><td><table width=270 border=0 cellpadding=2><tr><td width=30 align=right>{}</td>\
                 <td width=130><a action=\"bypass -h admin_character_info {}\">{}</a></td>\
                 <td width=110 align=right>{}</td></tr></table></td></tr>",
                p.level, p.name, p.name, p.class_id
            ));
        }
    }
    super::menu::show_admin_html_replace(world, client_id, "partyinfo.htm", &[("player", target_name), ("party", rows)]);
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
    crate::game_loop::skills::effects::recompute_max_vitals(world, target);
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
