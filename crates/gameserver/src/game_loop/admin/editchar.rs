//! `AdminEditChar` breadth — the character info/search, rename, party info,
//! pvp-flag and clan-penalty subcommands whose backing state exists in the
//! port. The info/search commands (`//character_info`, `//character_list`/
//! `//show_characters`, `//find_character`, `//edit_character`) render the same
//! HTML windows as Java (`charinfo`/`charlist`/`charfind`/`charedit.htm`), with
//! stats not yet computed in the port defaulted. `//fullfood` is wired as a
//! pet-blocked stub (see [`admin_fullfood`]); the other pet/summon subcommands
//! (`//summon_info`, `//show_pet_inv`, `//summon_setlvl`, `//unsummon`),
//! `//setparam`/`//unsetparam` (no fixed-stat API) and `//setnoble`/`//rec`
//! (fields not modelled) stay on the not-implemented path. The IP/dualbox tools
//! (`//find_ip`, `//find_dualbox`, `//tracert`) live in [`super::moderation`]
//! since G31.

use crate::game_loop::guard;
use crate::model::Player;
use crate::model::components::{
    CombatStats, PartyRef, PlayerVitals, Position, PvpState, Speeds, Vitals,
};
use crate::world::World;

use super::{find_online_player, send_message, send_sm};
use crate::network::server_packets::sm_ids;

/// Object ids of every in-game player (Java `World.getPlayers()`), name-sorted
/// for stable listing.
fn online_players(world: &World) -> Vec<i32> {
    let mut ids: Vec<i32> = world.in_game_player_oids().collect();
    ids.sort_by_key(|oid| {
        world
            .objects
            .get_component::<Player>(oid)
            .map(|p| p.name.clone())
            .unwrap_or_default()
    });
    ids
}

/// `//current_player` / `//character_info [name]` — dump a player's key fields.
/// `AdminEditChar`'s `//fullfood` — fill the targeted **pet**'s food bar.
///
/// Java's gate is `target.isPet()`, which is narrower than "an owned summon": a
/// skill-summoned servitor has no food bar at all (its `PetInfo` fed slot
/// carries its remaining lifetime instead), so targeting one is `INVALID_TARGET`
/// exactly like targeting a player.
pub(super) fn admin_fullfood(world: &mut World, client_id: u32, gm_object_id: i32) {
    let pet = crate::game_loop::guard::target(world, gm_object_id).filter(|oid| {
        world
            .objects
            .has_component::<crate::model::components::PetOf>(oid)
    });
    let Some(pet_oid) = pet else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };

    // Java `setCurrentFed(getMaxFed())`.
    let owner = {
        let Some(p) = world
            .objects
            .get_component_mut::<crate::model::components::PetOf>(&pet_oid)
        else {
            send_sm(world, client_id, sm_ids::INVALID_TARGET);
            return;
        };
        p.fed = p.max_fed;
        world
            .objects
            .get_component::<crate::model::components::ServitorOf>(&pet_oid)
            .map(|s| s.owner_object_id)
    };

    // Java `broadcastStatusUpdate()`. The food bar rides in `PetInfo`, not in a
    // `StatusUpdate`, and only the owner has a pet window to refresh — so this
    // is the packet that actually moves the bar the GM just filled.
    if let Some(owner_oid) = owner {
        crate::game_loop::servitor::send_pet_info(
            world,
            owner_oid,
            pet_oid,
            crate::game_loop::servitor::PetInfoKind::Default,
        );
    }
}

pub(super) fn admin_character_info(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
    self_only: bool,
) {
    // Java `showCharacterInfo` reaches the player one of three ways, and the two
    // that name him explicitly (`//current_player`, or a name coming from the
    // char list / find-results links) also run `activeChar.setTarget(player)` —
    // the `else` branch of `showCharacterInfo`. Every button on `charinfo.htm`
    // (`Lv/Exp/Sp`, enchant, karma, …) then acts on that target, so the GM's
    // pick from the list has to carry over. The third way — a bare
    // `//character_info` describing the already-selected target — leaves it be.
    let (target, retarget) = if self_only {
        (object_id, true)
    } else if let Some(name) = args.first() {
        match find_online_player(world, name) {
            Some(t) => (t, true),
            None => {
                send_sm(world, client_id, sm_ids::INVALID_TARGET);
                return;
            }
        }
    } else {
        match guard::player_target(world, object_id) {
            Some(t) => (t, false),
            None => {
                send_sm(world, client_id, sm_ids::INVALID_TARGET);
                return;
            }
        }
    };
    if retarget {
        crate::game_loop::target::set_target(world, client_id, object_id, Some(target));
    }
    let Some(p) = world.objects.get_component::<Player>(&target).cloned() else {
        return;
    };
    let pos = world
        .objects
        .get_component::<Position>(&target)
        .copied()
        .unwrap_or(Position {
            x: 0,
            y: 0,
            z: 0,
            heading: 0,
        });
    let vit = world
        .objects
        .get_component::<Vitals>(&target)
        .copied()
        .unwrap_or(Vitals {
            max_hp: 0,
            cur_hp: 0.0,
            max_mp: 0,
            cur_mp: 0.0,
            dead: false,
        });
    let cp = world
        .objects
        .get_component::<PlayerVitals>(&target)
        .copied()
        .unwrap_or(PlayerVitals {
            max_cp: 0,
            cur_cp: 0.0,
        });
    let cs = world
        .objects
        .get_component::<CombatStats>(&target)
        .copied()
        .unwrap_or_default();
    let spd = world
        .objects
        .get_component::<Speeds>(&target)
        .map(|s| s.run_spd)
        .unwrap_or(0.0);
    let pvp_flag = world
        .objects
        .get_component::<PvpState>(&target)
        .map(|s| s.flag)
        .unwrap_or(0);
    let clan = if p.clan_id == 0 {
        "None".to_string()
    } else {
        p.clan_id.to_string()
    };
    // Java `gatherCharacterInfo` reads the IP/HWID off the target's `GameClient`
    // and falls back to "N/A"/"Unknown" when there is none (client null or
    // detached), telling the GM which case it was. An offline trader is the
    // port's detached client — its session is gone but the `Player` stays.
    let (ip, hwid, protocol) = match super::super::helpers::client_for_player(world, target) {
        Some(cid) => (
            world
                .clients
                .get(&cid)
                .map(|cs| cs.addr().ip().to_string())
                .unwrap_or_else(|| "N/A".into()),
            world
                .hwids
                .get(&cid)
                .map(|h| h.mac_address.clone())
                .unwrap_or_else(|| "Unknown".into()),
            // Java `client.getProtocolVersion()`. Reported by the handshake and
            // kept for the connection's lifetime; `0` means the client has not
            // announced one yet, which is also `GameClient`'s initial value.
            world
                .protocol_versions
                .get(&cid)
                .copied()
                .unwrap_or(0)
                .to_string(),
        ),
        None => {
            let detached = super::super::offline_trade::is_offline_trader(world, target);
            send_message(
                world,
                client_id,
                if detached {
                    "Client is detached."
                } else {
                    "Client is null."
                },
            );
            ("N/A".to_string(), "Unknown".to_string(), "0".to_string())
        }
    };
    // Fill `charinfo.htm`. Stats not computed in the port yet (regen/load/
    // ai/instance) default to `0`/`N/A`; class is the numeric id (no client-code
    // table ported).
    let r: Vec<(&str, String)> = vec![
        ("name", p.name.clone()),
        ("account", p.account.clone()),
        ("ip", ip),
        ("hwid", hwid),
        ("protocol", protocol),
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
    let page = args
        .first()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
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
            .map(|i| {
                format!(
                    "<td><a action=\"bypass -h admin_show_characters {i}\">{}</a></td>",
                    i + 1
                )
            })
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
    for p in ids
        .iter()
        .filter_map(|oid| world.objects.get_component::<Player>(oid))
    {
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
        &[
            ("results", rows),
            ("number", found.to_string()),
            ("end", String::new()),
        ],
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
    let Some(account) = world
        .objects
        .get_component::<Player>(&target)
        .map(|p| p.account.clone())
    else {
        return;
    };
    send_message(
        world,
        client_id,
        &format!("=== Account '{account}' (online) ==="),
    );
    for oid in online_players(world) {
        if let Some(p) = world.objects.get_component::<Player>(&oid)
            && p.account == account
        {
            send_message(world, client_id, &format!("  {} (Lv {})", p.name, p.level));
        }
    }
}

/// `//edit_character [name]` — Java opens the char-edit HTML panel; we route to
/// the admin `charedit.htm` (falls back to the retail missing-text placeholder).
/// `//edit_character` — the `charedit.htm` field-editor for the current target
/// (Java `editCharacter`). Falls back to `INVALID_TARGET` with no player target.
pub(super) fn admin_edit_character(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = guard::player_target(world, object_id) else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    let Some(p) = world.objects.get_component::<Player>(&target).cloned() else {
        return;
    };
    let vit = world
        .objects
        .get_component::<Vitals>(&target)
        .copied()
        .unwrap_or(Vitals {
            max_hp: 0,
            cur_hp: 0.0,
            max_mp: 0,
            cur_mp: 0.0,
            dead: false,
        });
    let cp = world
        .objects
        .get_component::<PlayerVitals>(&target)
        .copied()
        .unwrap_or(PlayerVitals {
            max_cp: 0,
            cur_cp: 0.0,
        });
    let percent = if vit.max_hp > 0 {
        (vit.cur_hp / vit.max_hp as f64 * 100.0) as i64
    } else {
        0
    };
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
    let Some(target) = guard::player_target(world, object_id) else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    if find_online_player(world, &new_name).is_some() {
        send_message(
            world,
            client_id,
            &format!("Warning, player {new_name} already exists"),
        );
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
    let Some(target) = guard::player_target(world, object_id) else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    let cur = world
        .objects
        .get_component::<PvpState>(&target)
        .map_or(0, |s| s.flag);
    let next = (cur as i32 - 1).unsigned_abs() as u8;
    crate::game_loop::pvp::update_pvp_flag(world, target, next);
}

/// `//partyinfo [name]` — list the target player's party roster (Java opens an
/// HTML window; text here).
pub(super) fn admin_partyinfo(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let target = match args.first().and_then(|n| find_online_player(world, n)) {
        Some(t) => t,
        None => match guard::player_target(world, object_id) {
            Some(t) => t,
            None => {
                send_sm(world, client_id, sm_ids::INVALID_TARGET);
                return;
            }
        },
    };
    let target_name = world
        .objects
        .get_component::<Player>(&target)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let Some(PartyRef(pid)) = world.objects.get_component::<PartyRef>(&target).copied() else {
        // Java: not-in-party still opens the window (empty party table).
        super::menu::show_admin_html_replace(
            world,
            client_id,
            "partyinfo.htm",
            &[("player", target_name), ("party", String::new())],
        );
        return;
    };
    let members = world
        .parties
        .get(&pid)
        .map(|p| p.members.clone())
        .unwrap_or_default();
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
    super::menu::show_admin_html_replace(
        world,
        client_id,
        "partyinfo.htm",
        &[("player", target_name), ("party", rows)],
    );
}

/// `AdminEditChar`'s `//setparam <stat> <value>` / `//unsetparam <stat>` — set
/// or clear a fixed-value override on one of the target's combat stats (Java
/// `CreatureStat.addFixedValue`/`removeFixedValue`), then recompute + broadcast.
/// `maxHp`/`maxMp` compute on a separate path and aren't overridable here.
pub(super) fn admin_setparam(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
    set: bool,
) {
    let Some(stat) = args.first().and_then(|s| stat_from_name(s)) else {
        send_message(
            world,
            client_id,
            if set {
                "Syntax: //setparam <stat> <value>"
            } else {
                "Syntax: //unsetparam <stat>"
            },
        );
        if !args.is_empty() {
            send_message(world, client_id, "Couldn't find such stat!");
        }
        return;
    };
    let target = guard::target(world, object_id)
        .filter(|oid| {
            world
                .objects
                .has_component::<crate::model::components::StatModifiers>(oid)
        })
        .unwrap_or(object_id);
    if set {
        let Some(value) = args.get(1).and_then(|s| s.parse::<f64>().ok()) else {
            send_message(world, client_id, "Syntax: //setparam <stat> <value>");
            return;
        };
        if let Some(m) = world
            .objects
            .get_component_mut::<crate::model::components::StatModifiers>(&target)
        {
            m.fixed.insert(stat, value);
        }
        send_message(
            world,
            client_id,
            &format!("Fixed stat {} set to {value}.", args[0]),
        );
    } else {
        if let Some(m) = world
            .objects
            .get_component_mut::<crate::model::components::StatModifiers>(&target)
        {
            m.fixed.remove(&stat);
        }
        send_message(
            world,
            client_id,
            &format!("Fixed stat {} has been removed.", args[0]),
        );
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
    if let Some((p, base, mods, inventory, mut speeds, mut combat)) = world.objects.get_many_mut::<(
        &Player,
        &BaseStats,
        &StatModifiers,
        &Inventory,
        &mut Speeds,
        &mut CombatStats,
    )>(&target)
    {
        p.recalculate_stats(data, base, mods, inventory, &mut speeds, &mut combat);
    }
    crate::game_loop::skills::effects::recompute_max_vitals(world, target);
}

/// `//remove_clan_penalty create|join <name>` — clear a clan cooldown. Only the
/// `create` cooldown is modelled (`clan_create_expiry_time`); `join` reports it
/// isn't tracked. Applies to an online target in memory.
pub(super) fn admin_remove_clan_penalty(world: &mut World, client_id: u32, args: &[&str]) {
    let [kind, name] = args else {
        send_message(
            world,
            client_id,
            "Usage: //remove_clan_penalty join|create charname",
        );
        return;
    };
    let is_create = kind.eq_ignore_ascii_case("create");
    if !is_create {
        send_message(
            world,
            client_id,
            "The clan join penalty is not tracked in this build.",
        );
        return;
    }
    let Some(target) = find_online_player(world, name) else {
        send_message(world, client_id, &format!("Player '{name}' is not online."));
        return;
    };
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.clan_create_expiry_time = 0;
    }
    send_message(
        world,
        client_id,
        &format!("Clan penalty successfully removed to character: {name}"),
    );
}

// ---------------------------------------------------------------------------
// Pet / summon subcommands + `//rec` (`AdminEditChar`, category-4 sweep)
// ---------------------------------------------------------------------------

/// `//rec <n>` — set the targeted player's Recommend count (Java
/// `setRecomHave` + `broadcastUserInfo` + both messages).
pub(super) fn admin_rec(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(val) = args.first().and_then(|a| a.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //rec number");
        return;
    };
    let Some(target) = guard::player_target(world, object_id) else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    let name = world
        .objects
        .get_component_mut::<Player>(&target)
        .map(|p| {
            p.rec_have = val;
            p.name.clone()
        })
        .unwrap_or_default();
    super::party::broadcast_user_info(world, target);
    if let Some(cid) = super::helpers::client_for_player(world, target) {
        send_message(
            world,
            cid,
            &format!("A GM changed your Recommend points to {val}"),
        );
    }
    send_message(
        world,
        client_id,
        &format!("{name}'s Recommend changed to {val}"),
    );
}

/// The targeted summon's (npc_oid, owner_oid), if the target is one.
fn targeted_summon(world: &World, object_id: i32) -> Option<(i32, i32)> {
    let target = guard::target(world, object_id)?;
    let owner = world
        .objects
        .get_component::<crate::model::components::ServitorOf>(&target)?
        .owner_object_id;
    Some((target, owner))
}

/// `//unsummon` — dismiss the targeted pet/servitor (Java
/// `Summon.unSummon(owner)`).
pub(super) fn admin_unsummon(world: &mut World, client_id: u32, object_id: i32) {
    let Some((_, owner)) = targeted_summon(world, object_id) else {
        send_message(world, client_id, "Usable only with Pets/Summons");
        return;
    };
    // Capture a pet's state first (Java `unSummon` → `storeMe`); no-op for a
    // servitor.
    crate::game_loop::servitor::sync_pet_row(world, owner);
    crate::game_loop::servitor::unsummon_servitor(world, owner);
}

/// `//summon_info` — the `petinfo.htm` state dump for the targeted summon
/// (Java `gatherSummonInfo`).
pub(super) fn admin_summon_info(world: &mut World, client_id: u32, object_id: i32) {
    let Some((summon_oid, owner)) = targeted_summon(world, object_id) else {
        send_message(world, client_id, "Invalid target.");
        return;
    };
    let npc = world
        .objects
        .get_component::<crate::model::npc::Npc>(&summon_oid);
    let (name, npc_level) = npc
        .and_then(|n| n.template(world))
        .map(|t| (t.name.clone(), t.level))
        .unwrap_or_default();
    let pet = world
        .objects
        .get_component::<crate::model::components::PetOf>(&summon_oid)
        .copied();
    let (cur_hp, max_hp, cur_mp, max_mp) = world
        .objects
        .get_component::<Vitals>(&summon_oid)
        .map_or((0, 0, 0, 0), |v| {
            (v.cur_hp as i32, v.max_hp, v.cur_mp as i32, v.max_mp)
        });
    let owner_name = world
        .objects
        .get_component::<Player>(&owner)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let (level, exp) = pet.map_or((npc_level, 0), |p| (p.level, p.exp));
    let (class, inv, food) = if let Some(p) = pet {
        (
            "Pet",
            format!(" <a action=\"bypass admin_show_pet_inv {owner}\">view</a>"),
            format!("{}/{}", p.fed, p.max_fed),
        )
    } else {
        ("Servitor", "none".to_string(), "N/A".to_string())
    };
    super::menu::show_admin_html_replace(
        world,
        client_id,
        "petinfo.htm",
        &[
            ("name", name),
            ("level", level.to_string()),
            ("exp", exp.to_string()),
            (
                "owner",
                format!(
                    " <a action=\"bypass -h admin_character_info {owner_name}\">{owner_name}</a>"
                ),
            ),
            ("class", class.to_string()),
            ("ai", "N/A".to_string()),
            ("hp", format!("{cur_hp}/{max_hp}")),
            ("mp", format!("{cur_mp}/{max_mp}")),
            ("karma", "0".to_string()),
            ("race", "N/A".to_string()),
            ("inv", inv),
            // Weight isn't tracked on the pet inventory in this port.
            ("food", food),
            ("load", "N/A".to_string()),
        ],
    );
}

/// `//summon_setlvl <n>` — set the targeted *pet*'s level by moving its exp
/// to `exp_for_level(n)` (Java add/removeExp), then re-run the per-level stat
/// row and the collar-enchant sync.
pub(super) fn admin_summon_setlvl(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
) {
    let Some(level) = args.first().and_then(|a| a.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //summon_setlvl level");
        return;
    };
    let Some((pet_oid, owner)) = targeted_summon(world, object_id).filter(|(oid, _)| {
        world
            .objects
            .has_component::<crate::model::components::PetOf>(oid)
    }) else {
        send_message(world, client_id, "Usable only with Pets");
        return;
    };
    let npc_id = world
        .objects
        .get_component::<crate::model::npc::Npc>(&pet_oid)
        .map(|n| n.npc_id)
        .unwrap_or(0);
    let Some((exp, max_fed)) = world.data.pet_data.get(npc_id).and_then(|t| {
        t.level_row(level)
            .map(|_| (t.exp_for_level(level), t.max_meal(level)))
    }) else {
        send_message(world, client_id, "That species has no such level.");
        return;
    };
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::components::PetOf>(&pet_oid)
    {
        p.level = level;
        p.exp = exp;
        p.max_fed = max_fed;
        p.fed = p.fed.min(max_fed);
    }
    crate::game_loop::servitor::recalculate_pet_stats(world, pet_oid);
    crate::game_loop::servitor::sync_collar_enchant_for_admin(world, owner, pet_oid);
    send_message(world, client_id, &format!("Pet level set to {level}."));
}

/// `//show_pet_inv [ownerObjectId]` — `GMViewItemList` of the targeted pet's
/// own inventory (or the pet of the player with the given object id).
pub(super) fn admin_show_pet_inv(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    // Java's argument is the *owner's* object id (`World.getPet(ownerId)`).
    let pet_oid = args
        .first()
        .and_then(|a| a.parse::<i32>().ok())
        .and_then(|owner| crate::game_loop::servitor::servitor_of(world, owner))
        .filter(|oid| {
            world
                .objects
                .has_component::<crate::model::components::PetOf>(oid)
        })
        .or_else(|| {
            targeted_summon(world, object_id)
                .map(|(oid, _)| oid)
                .filter(|oid| {
                    world
                        .objects
                        .has_component::<crate::model::components::PetOf>(oid)
                })
        });
    let Some(pet_oid) = pet_oid else {
        send_message(world, client_id, "Usable only with Pets");
        return;
    };
    let name = world
        .objects
        .get_component::<crate::model::npc::Npc>(&pet_oid)
        .and_then(|n| n.template(world))
        .map(|t| t.name.clone())
        .unwrap_or_default();
    let Some(inv) = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&pet_oid)
    else {
        send_message(world, client_id, "This pet carries no inventory.");
        return;
    };
    let pkt = crate::network::enter_world::gm_view_item_list(&name, inv, &world.data);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(pkt);
    }
}

// ---------------------------------------------------------------------------
// Quest admin (`AdminShowQuests` / `AdminQuest`, category-4 sweep)
// ---------------------------------------------------------------------------

/// Resolve `[name]`-or-target to a player for the quest commands.
fn quest_target(world: &World, object_id: i32, args: &[&str]) -> Option<i32> {
    args.first()
        .and_then(|name| super::find_online_player(world, name))
        .or_else(|| guard::player_target(world, object_id))
        .or(Some(object_id).filter(|oid| world.objects.has_component::<Player>(oid)))
}

/// `//charquestmenu [player]` / `//show_quests` — the target's quest states
/// as a generated panel (Java builds the same table in `AdminShowQuests`).
pub(super) fn admin_charquestmenu(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
) {
    let Some(target) = quest_target(world, object_id, args) else {
        send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    let name = world
        .objects
        .get_component::<Player>(&target)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let mut rows = String::new();
    if let Some(q) = world
        .objects
        .get_component::<crate::model::components::Quests>(&target)
    {
        let mut entries: Vec<_> = q.0.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        for (quest, st) in entries {
            let vars = st
                .vars
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(", ");
            rows.push_str(&format!(
                "<tr><td>{quest}</td><td>{}</td><td>{vars}</td></tr>",
                crate::model::quest::state::name(st.state)
            ));
        }
    }
    if rows.is_empty() {
        rows = "<tr><td>No quest states.</td></tr>".to_string();
    }
    let html = format!(
        "<html><title>Quests of {name}</title><body><center>\
         <table width=270><tr><td>Quest</td><td>State</td><td>Vars</td></tr>{rows}</table>\
         <br>Set: //setcharquest {name} &lt;quest&gt; &lt;var&gt; &lt;value&gt;\
         </center></body></html>"
    );
    super::menu::send_admin_html_content(world, client_id, &html);
}

/// `//setcharquest <player> <quest> <var> <value>` — set a quest variable on
/// the target; var `state` sets the state (`CREATED|STARTED|COMPLETED`), and
/// value `DELETE` with var `state` removes the quest state entirely (Java
/// `AdminShowQuests`' edit branch).
pub(super) fn admin_setcharquest(world: &mut World, client_id: u32, args: &[&str]) {
    let (Some(&name), Some(&quest), Some(&var), Some(&value)) =
        (args.first(), args.get(1), args.get(2), args.get(3))
    else {
        send_message(
            world,
            client_id,
            "Usage: //setcharquest <player> <quest> <var> <value>",
        );
        return;
    };
    let Some(target) = super::find_online_player(world, name) else {
        send_message(world, client_id, &format!("Player '{name}' is not online."));
        return;
    };
    let Some(quests) = world
        .objects
        .get_component_mut::<crate::model::components::Quests>(&target)
    else {
        return;
    };
    if var == "state" && value.eq_ignore_ascii_case("DELETE") {
        quests.0.remove(quest);
        refresh_quest_journal(world, target, quest);
        send_message(world, client_id, &format!("Quest {quest} state removed."));
        return;
    }
    let st = quests.0.entry(quest.to_string()).or_default();
    if var == "state" {
        st.state = match value.to_ascii_uppercase().as_str() {
            "CREATED" => crate::model::quest::state::CREATED,
            "STARTED" => crate::model::quest::state::STARTED,
            "COMPLETED" => crate::model::quest::state::COMPLETED,
            _ => {
                send_message(
                    world,
                    client_id,
                    "State must be CREATED, STARTED, COMPLETED or DELETE.",
                );
                return;
            }
        };
    } else {
        st.vars.insert(var.to_string(), value.to_string());
    }
    refresh_quest_journal(world, target, quest);
    send_message(
        world,
        client_id,
        &format!("Quest {quest}: {var} = {value} set on {name}."),
    );
}

/// Java's `AdminShowQuests.setQuestVar` closes every branch with `QuestList` +
/// `ExShowQuestMark` on the *edited* player, so the journal reflects the edit
/// immediately. The port set the vars in memory and sent nothing, which left
/// the client showing the step it had before — a `//setcharquest … cond 28`
/// looked like it had done nothing until the next relog rebuilt the journal.
fn refresh_quest_journal(world: &mut World, target: i32, quest: &str) {
    let Some(target_cid) = super::helpers::client_for_player(world, target) else {
        return;
    };
    let Some(quests) = world
        .objects
        .get_component::<crate::model::components::Quests>(&target)
    else {
        return;
    };
    let list = crate::network::enter_world::quest_list(quests, &world.quests);
    // Java passes `qs.getCond()`; ExShowQuestMark is skipped for a custom quest
    // (id 0) and while the cond is still 0, as in `QuestState.setCond`.
    let cond = quests.0.get(quest).map(|qs| qs.cond()).unwrap_or(0);
    let mark = world
        .quests
        .quest_id(quest)
        .filter(|&id| id > 0 && cond > 0)
        .map(|id| crate::network::server_packets::ex_show_quest_mark(id, cond));
    if let Some(cs) = world.clients.get(&target_cid) {
        cs.send(list);
        if let Some(mark) = mark {
            cs.send(mark);
        }
    }
}
