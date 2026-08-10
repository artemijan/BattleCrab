//! Port of `handlers/bypasshandlers/NpcViewMod` — the shift-click NPC info
//! window (`AltGameViewNpc`). Java routes a non-GM shift-click on an NPC
//! through `Action` case 1 → `Npc.onActionShift` → `NpcActionShift`, whose
//! `ALT_GAME_VIEWNPC` branch calls `NpcViewMod.sendNpcView`; the info window's
//! "Show Drop" button then bypasses back here (`bypass NpcViewMod dropList
//! DROP <objId> [page]`). Entry points: [`send_npc_view`] (the window) and
//! [`handle_npc_view_bypass`] (the button router).
//!
//! The **GM** branch of `NpcActionShift` (the admin `npcinfo.htm` window) is
//! the sibling [`admin::npc_info`](super::admin::npc_info); `handle_action`
//! picks between the two exactly like Java's `isGM()` test.
//!
//! All four verbs are handled: `view` (Info.htm), `skills` (Skills.htm),
//! `aggrolist` (AggroList.htm) and `droplist` (DropList.htm) — the last three
//! reached from the admin `npcinfo.htm` buttons and the info window's own
//! "Show Drop"/"Show Spoil" pair. `dropList` covers both the `DROP` and
//! `SPOIL` scopes.
//!
//! Two channel details that are easy to get wrong and were, until 2026-08-01:
//! - Info/Skills/AggroList go out as `new NpcHtmlMessage()` — the **no-arg**
//!   ctor, i.e. npcObjId `0`, not the NPC's object id.
//! - The drop list is **not** an NPC dialog at all: Java sends it through
//!   `Util.sendCBHtml`, the chunked community-board channel that `DropList.htm`
//!   is laid out for and whose ceiling the 16000-char row budget assumes.

use crate::game_loop::guard;
use crate::game_loop::helpers::{format_amount, send_to_client};
use crate::network::server_packets;
use crate::world::World;

use crate::data::npc_data::{DropHolder, NpcTemplate};

const DROP_LIST_ITEMS_PER_PAGE: usize = 10;

/// `NpcViewMod.sendNpcView`: the `data/html/mods/NpcView/Info.htm` window —
/// name, HP/MP gauges, respawn, the combat-stat block, and (Interlude Classic
/// has no elemental system) an all-`NONE`/`0` attribute block. The caller has
/// already set the player's target, matching `NpcActionShift`.
pub(crate) fn send_npc_view(world: &World, client_id: u32, npc_object_id: i32) {
    use crate::model::components::{Speeds, Vitals};
    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_object_id)
    else {
        return;
    };
    let Some(t) = npc.template(world) else { return };
    let (Some(vitals), Some(speeds), Some(stats)) = (
        world.objects.get_component::<Vitals>(&npc_object_id),
        world.objects.get_component::<Speeds>(&npc_object_id),
        world
            .objects
            .get_component::<crate::model::components::CombatStats>(&npc_object_id),
    ) else {
        return;
    };

    // Java `NpcHtmlMessage.setFile`: a missing file still sends the packet
    // (empty content, logged) — mirror `interact_with_npc`'s stub fallback so
    // the window always opens even when the datapack html is absent.
    let mut html = crate::data::htm_cache::read_htm(format!(
        "{}data/html/mods/NpcView/Info.htm",
        world.data.root
    ))
    .unwrap_or_else(|| "<html><body>NPC Info<br>%name%</body></html>".to_string());

    let mut set = |needle: &str, value: &str| html = html.replace(needle, value);
    set("%name%", &t.name);
    set(
        "%hpGauge%",
        &gauge(
            250,
            vitals.cur_hp as i64,
            vitals.max_hp as i64,
            GaugeKind::Hp,
        ),
    );
    set(
        "%mpGauge%",
        &gauge(
            250,
            vitals.cur_mp as i64,
            vitals.max_mp as i64,
            GaugeKind::Mp,
        ),
    );
    set("%respawn%", &respawn_text(npc));
    set("%atktype%", &attack_type_name(world, t));
    set("%atkrange%", &stats.atk_range.to_string());
    set("%patk%", &(stats.p_atk as i64).to_string());
    set("%pdef%", &(stats.p_def as i64).to_string());
    set("%matk%", &(stats.m_atk as i64).to_string());
    set("%mdef%", &(stats.m_def as i64).to_string());
    set("%atkspd%", &stats.p_atk_spd.to_string());
    set("%castspd%", &stats.m_atk_spd.to_string());
    set("%critrate%", &(stats.crit_hit as i64).to_string());
    set("%evasion%", &stats.evasion.to_string());
    set("%accuracy%", &stats.accuracy.to_string());
    set("%speed%", &(speeds.move_speed() as i64).to_string());
    // Interlude Classic has no elemental attributes (`AttributeType.NONE`, 0).
    set("%attributeatktype%", "NONE");
    set("%attributeatkvalue%", "0");
    set("%attributefire%", "0");
    set("%attributewater%", "0");
    set("%attributewind%", "0");
    set("%attributeearth%", "0");
    set("%attributedark%", "0");
    set("%attributeholy%", "0");
    set("%dropListButtons%", &drop_list_buttons(t, npc_object_id));

    // Java `new NpcHtmlMessage()` — the **no-arg** ctor, so the window carries
    // npcObjId 0 and its bypasses are not bound to the NPC (Java's
    // `HtmlActionCache` origin). All three NpcViewMod pages do this.
    send_to_client(world, client_id, server_packets::npc_html_message(0, &html));
}

/// `bypass NpcViewMod <verb> …` router — the `Info.htm` buttons *and* the
/// admin `npcinfo.htm` ones (`view`/`skills`/`aggrolist`). Java's `view`,
/// `skills` and `aggrolist` all take an optional object id and fall back to
/// the player's current target; `dropList` requires its two arguments.
pub(crate) fn handle_npc_view_bypass(world: &World, client_id: u32, object_id: i32, command: &str) {
    let mut it = command.split_whitespace();
    it.next(); // "NpcViewMod"
    let Some(verb) = it.next() else { return };
    let verb = verb.to_ascii_lowercase();
    if verb == "droplist" {
        // `dropList <DROP|SPOIL> <objId> [page]` — Java bails when fewer than
        // two tokens follow.
        let scope = it.next().unwrap_or("");
        let Some(npc_object_id) = it.next().and_then(|s| s.parse::<i32>().ok()) else {
            return;
        };
        let page = it.next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
        if world
            .objects
            .has_component::<crate::model::npc::Npc>(&npc_object_id)
        {
            send_npc_drop_list(world, client_id, npc_object_id, scope, page);
        }
        return;
    }
    // The shared `target = <objId argument> else player.getTarget()` prologue.
    // A non-NPC target (or a stale id) is Java's `npc == null` → drop.
    let Some(npc_object_id) = it
        .next()
        .and_then(|s| s.parse::<i32>().ok())
        .or_else(|| guard::target(world, object_id))
        .filter(|id| world.objects.has_component::<crate::model::npc::Npc>(id))
    else {
        return;
    };
    match verb.as_str() {
        "view" => send_npc_view(world, client_id, npc_object_id),
        "skills" => send_npc_skill_view(world, client_id, npc_object_id),
        "aggrolist" => send_aggro_list_view(world, client_id, npc_object_id),
        _ => {}
    }
}

/// `NpcViewMod.sendNpcSkillView` — `data/html/mods/NpcView/Skills.htm`, one
/// icon/name/id/level row per skill the NPC carries. Java reads
/// `npc.getSkills()`, which the `Creature` ctor filled from the template's
/// `<skillList>`, so the template list is the same set.
fn send_npc_skill_view(world: &World, client_id: u32, npc_object_id: i32) {
    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_object_id)
    else {
        return;
    };
    let Some(t) = npc.template(world) else { return };

    let mut rows = String::new();
    for &(skill_id, level) in &t.skill_list {
        // An id/level the skill parser didn't produce is simply absent from
        // Java's map too (`addSkill` of a null skill is a no-op).
        let Some(skill) = world.data.skill_data.get(skill_id, level) else {
            continue;
        };
        rows.push_str(&format!(
            "<table width=277 height=32 cellspacing=0 background=\"L2UI_CT1.Windows.Windows_DF_TooltipBG\">\
             <tr><td width=32><img src=\"{}\" width=32 height=32></td>\
             <td width=110>{}</td>\
             <td width=45 align=center>{}</td>\
             <td width=35 align=center>{}</td></tr></table>",
            skill.icon, skill.name, skill.id, skill.level
        ));
    }

    let html = read_view_htm(world, "Skills.htm")
        .replace("%skills%", &rows)
        .replace("%npc_name%", &t.name)
        .replace("%npcId%", &t.id.to_string());
    send_to_client(world, client_id, server_packets::npc_html_message(0, &html));
}

/// `NpcViewMod.sendAggroListView` — `data/html/mods/NpcView/AggroList.htm`,
/// one name/hate/damage row per aggro entry. Java guards on `isAttackable()`,
/// so a non-Attackable NPC shows an empty list rather than no page.
fn send_aggro_list_view(world: &World, client_id: u32, npc_object_id: i32) {
    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_object_id)
    else {
        return;
    };
    let Some(t) = npc.template(world) else { return };

    let mut rows = String::new();
    if t.is_attackable_class()
        && let Some(aggro) = world
            .objects
            .get_component::<crate::model::npc::AggroList>(&npc_object_id)
    {
        for (attacker_oid, info) in &aggro.0 {
            // Java prints "NULL" for an attacker reference that has gone away
            // (`a.getAttacker() != null ? … : "NULL"`); ours is an object id,
            // so a despawned/logged-out attacker is the same case.
            let name = attacker_name(world, *attacker_oid).unwrap_or_else(|| "NULL".to_string());
            rows.push_str(&format!(
                "<table width=277 height=32 cellspacing=0 background=\"L2UI_CT1.Windows.Windows_DF_TooltipBG\">\
                 <tr><td width=110>{name}</td>\
                 <td width=60 align=center>{}</td>\
                 <td width=60 align=center>{}</td></tr></table>",
                info.hate as i64, info.damage as i64
            ));
        }
    }

    let html = read_view_htm(world, "AggroList.htm")
        .replace("%aggrolist%", &rows)
        .replace("%npc_name%", &t.name)
        .replace("%npcId%", &t.id.to_string())
        // The Refresh button re-issues the bypass, so this one *is* the
        // object id, not the template id.
        .replace("%objid%", &npc_object_id.to_string());
    send_to_client(world, client_id, server_packets::npc_html_message(0, &html));
}

/// A player's name, else an NPC's template name — whatever holds the aggro.
fn attacker_name(world: &World, object_id: i32) -> Option<String> {
    if let Some(p) = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
    {
        return Some(p.name.clone());
    }
    let npc = world
        .objects
        .get_component::<crate::model::npc::Npc>(&object_id)?;
    Some(npc.template(world)?.name.clone())
}

/// One of the three `data/html/mods/NpcView/*` files. Java's `setFile` sends
/// the packet even when the file is missing (logged, empty body); mirror the
/// stub fallback the info window already used so a window always opens.
fn read_view_htm(world: &World, file: &str) -> String {
    crate::data::htm_cache::read_htm(format!("{}data/html/mods/NpcView/{file}", world.data.root))
        .unwrap_or_else(|| format!("<html><body>{file}<br>%npc_name%</body></html>"))
}

/// `NpcViewMod.getDropListButtons`: a "Show Drop" button when the NPC has any
/// death drops (grouped or ungrouped) and a "Show Spoil" button when it carries
/// a `<spoil>` list — side by side, each present only if its list is non-empty
/// (Java builds the same two-cell row).
fn drop_list_buttons(t: &NpcTemplate, npc_object_id: i32) -> String {
    let has_drops = !t.drop_list_death.is_empty() || !t.drop_groups.is_empty();
    let has_spoil = !t.drop_list_spoil.is_empty();
    if !has_drops && !has_spoil {
        return String::new();
    }
    let mut cells = String::new();
    if has_drops {
        cells.push_str(&format!(
            "<td align=center><button value=\"Show Drop\" width=100 height=25 \
             action=\"bypass NpcViewMod dropList DROP {npc_object_id}\" \
             back=\"L2UI_CT1.Button_DF_Calculator_Down\" fore=\"L2UI_CT1.Button_DF_Calculator\"></td>"
        ));
    }
    if has_spoil {
        cells.push_str(&format!(
            "<td align=center><button value=\"Show Spoil\" width=100 height=25 \
             action=\"bypass NpcViewMod dropList SPOIL {npc_object_id}\" \
             back=\"L2UI_CT1.Button_DF_Calculator_Down\" fore=\"L2UI_CT1.Button_DF_Calculator\"></td>"
        ));
    }
    format!("<table width=275 cellpadding=0 cellspacing=0><tr>{cells}</tr></table>")
}

/// `NpcViewMod.sendNpcDropList` for the `DROP` and `SPOIL` scopes: the death
/// drop list (ungrouped drops plus every group's drops scaled by the group
/// chance) or the spoil list, sorted by item id, paginated 10-per-page, each
/// row showing the server-rate amount and chance.
///
/// Rate math mirrors Java for the default/stock config. `DROP` applies the
/// per-item overrides then the raid or death multiplier; `SPOIL` uses the
/// spoil multipliers (no per-item overrides — Java's `SPOIL` branch doesn't
/// read them). The premium system, the herb special case
/// (`hasExImmediateEffect`), and the player's `BONUS_DROP_*` effects are not
/// ported, so those factors stay at ×1 — exact for the stock rates.
fn send_npc_drop_list(
    world: &World,
    client_id: u32,
    npc_object_id: i32,
    scope: &str,
    page_value: usize,
) {
    let is_spoil = scope.eq_ignore_ascii_case("SPOIL");
    if !is_spoil && !scope.eq_ignore_ascii_case("DROP") {
        return; // unknown scope: nothing to show.
    }
    // The scope token echoed back into paging bypasses (upper-case, like Java).
    let scope_token = if is_spoil { "SPOIL" } else { "DROP" };
    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_object_id)
    else {
        return;
    };
    let Some(t) = npc.template(world) else { return };

    // The list to show: the spoil list, or the combined death list (ungrouped
    // drops, then each group's drops with the group chance folded into the item
    // chance — Java `chance / 100`).
    let mut drop_list: Vec<DropHolder> = if is_spoil {
        t.drop_list_spoil.clone()
    } else {
        t.drop_list_death.clone()
    };
    if !is_spoil {
        for group in &t.drop_groups {
            let group_chance = group.chance / 100.0;
            for d in &group.items {
                drop_list.push(DropHolder {
                    item_id: d.item_id,
                    min: d.min,
                    max: d.max,
                    chance: d.chance * group_chance,
                });
            }
        }
    }
    drop_list.sort_by_key(|d| d.item_id);

    let mut pages = drop_list.len() / DROP_LIST_ITEMS_PER_PAGE;
    if DROP_LIST_ITEMS_PER_PAGE * pages < drop_list.len() {
        pages += 1;
    }

    let mut pages_sb = String::new();
    if pages > 1 {
        pages_sb.push_str("<table><tr>");
        for i in 0..pages {
            pages_sb.push_str(&format!(
                "<td align=center><button value=\"{}\" width=20 height=20 \
                 action=\"bypass NpcViewMod dropList {} {} {}\" \
                 back=\"L2UI_CT1.Button_DF_Calculator_Down\" fore=\"L2UI_CT1.Button_DF_Calculator\"></td>",
                i + 1,
                scope_token,
                npc_object_id,
                i
            ));
        }
        pages_sb.push_str("</tr></table>");
    }

    let page = if page_value >= pages {
        pages.saturating_sub(1)
    } else {
        page_value
    };
    let start = page * DROP_LIST_ITEMS_PER_PAGE;
    let end = (start + DROP_LIST_ITEMS_PER_PAGE).min(drop_list.len());

    let rate_chance_base = if is_spoil {
        world.cfg.rates.spoil_drop_chance_multiplier
    } else if t.is_raid() {
        world.cfg.rates.raid_drop_chance_multiplier
    } else {
        world.cfg.rates.death_drop_chance_multiplier
    };
    let rate_amount_base = if is_spoil {
        world.cfg.rates.spoil_drop_amount_multiplier
    } else if t.is_raid() {
        world.cfg.rates.raid_drop_amount_multiplier
    } else {
        world.cfg.rates.death_drop_amount_multiplier
    };

    // Two balanced columns, like Java (`leftHeight`/`rightHeight`).
    let mut left_sb = String::new();
    let mut right_sb = String::new();
    let (mut left_h, mut right_h) = (0i32, 0i32);
    let mut limit_reached = "";
    for d in &drop_list[start..end] {
        // Per-item overrides apply to death drops only (Java's `SPOIL` branch
        // seeds the rates from the spoil multipliers and never reads them).
        let (chance_by_id, amount_by_id) = if is_spoil {
            (1.0, 1.0)
        } else {
            (
                world
                    .cfg
                    .rates
                    .drop_chance_by_id
                    .get(&d.item_id)
                    .copied()
                    .unwrap_or(1.0),
                world
                    .cfg
                    .rates
                    .drop_amount_by_id
                    .get(&d.item_id)
                    .copied()
                    .unwrap_or(1.0),
            )
        };
        let rate_chance = rate_chance_base * chance_by_id;
        let rate_amount = rate_amount_base * amount_by_id;
        let name = world
            .data
            .item_data
            .get(d.item_id)
            .map(|i| i.name.as_str())
            .unwrap_or("Unknown item");

        let min = (d.min as f64 * rate_amount) as i64;
        let max = (d.max as f64 * rate_amount) as i64;
        let amount = if min == max {
            format_amount(min)
        } else {
            format!("{} - {}", format_amount(min), format_amount(max))
        };
        let chance = format_chance((d.chance * rate_chance).min(100.0));
        // Java `item.getIcon()`, with `icon.etc_question_mark_i00` only when the
        // template declares none (`ItemData::icon` applies that same fallback).
        let icon = world.data.item_data.icon(d.item_id);

        let row = format!(
            "<table width=332 cellpadding=2 cellspacing=0 background=\"L2UI_CT1.Windows.Windows_DF_TooltipBG\">\
             <tr><td width=32 valign=top>\
             <button width=\"32\" height=\"32\" back=\"{icon}\" fore=\"{icon}\" itemtooltip=\"{}\"></td>\
             <td fixwidth=300 align=center><font name=\"hs9\" color=\"CD9000\">{}</font></td></tr>\
             <tr><td width=32></td><td width=300><table width=295 cellpadding=0 cellspacing=0>\
             <tr><td width=48 align=right valign=top><font color=\"LEVEL\">Amount:</font></td>\
             <td width=247 align=center>{}</td></tr>\
             <tr><td width=48 align=right valign=top><font color=\"LEVEL\">Chance:</font></td>\
             <td width=247 align=center>{}%</td></tr></table></td></tr>\
             <tr><td width=32></td><td width=300>&nbsp;</td></tr></table>",
            d.item_id, name, amount, chance
        );

        if row.len() + left_sb.len() + right_sb.len() < 16000 {
            if left_h >= right_h + 64 {
                right_sb.push_str(&row);
                right_h += 64;
            } else {
                left_sb.push_str(&row);
                left_h += 64;
            }
        } else {
            limit_reached = "<br><center>Too many drops! Could not display them all!</center>";
        }
    }

    let Some(template_html) = crate::data::htm_cache::read_htm(format!(
        "{}data/html/mods/NpcView/DropList.htm",
        world.data.root
    )) else {
        return;
    };
    let body =
        format!("<table><tr><td>{left_sb}</td><td>{right_sb}</td></tr></table>{limit_reached}");
    let html = template_html
        .replace("%name%", &t.name)
        .replace("%dropListButtons%", &drop_list_buttons(t, npc_object_id))
        .replace("%pages%", &pages_sb)
        .replace("%items%", &body);
    // Java `Util.sendCBHtml` — the drop list is a **community-board** page, not
    // an NPC dialog: DropList.htm is laid out for the wide board window, and
    // the 16000-char budget above is sized for the chunked `ShowBoard` channel
    // rather than `NpcHtmlMessage`'s much smaller ceiling.
    super::community_board::send_cb_html(world, client_id, &html);
}

/// Java `DecimalFormat("0.00##")` — at least 2 decimals, at most 4, with the
/// 3rd/4th dropped when they are zero (`1.5` → `1.50`, `0.012345` → `0.0123`).
fn format_chance(value: f64) -> String {
    let mut s = format!("{value:.4}");
    let dot = s.find('.').expect("{:.4} always writes a decimal point");
    // Trim trailing zeros, but never below two decimals ("1.5000" → "1.50").
    while s.ends_with('0') && (s.len() - dot - 1) > 2 {
        s.pop();
    }
    s
}

/// `Creature.getAttackType`: the equipped weapon's type, else `FIST`,
/// formatted like Java `CommonUtil.capitalizeFirst(name.toLowerCase())`.
pub(super) fn attack_type_name(world: &World, t: &NpcTemplate) -> String {
    use crate::data::item_data::WeaponType;
    let wt = if t.rhand != 0 {
        world.data.item_data.weapon_type(t.rhand)
    } else {
        WeaponType::None
    };
    let wt = if wt == WeaponType::None {
        WeaponType::Fist
    } else {
        wt
    };
    capitalize_first(&format!("{wt:?}").to_ascii_lowercase())
}

fn capitalize_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_ascii_uppercase().to_string() + c.as_str(),
        None => String::new(),
    }
}

/// `NpcViewMod.sendNpcView`'s respawn line: "None" when the spawn never
/// respawns, else the delay in the coarsest whole time unit (Java's
/// `TimeUnit` sweep), with a `min-max` range when randomised.
fn respawn_text(npc: &crate::model::npc::Npc) -> String {
    if npc.respawn_secs == 0 {
        return "None".to_string();
    }
    // Match `death.rs`'s in-repo convention for the randomised window.
    let min_s = (npc.respawn_secs - npc.respawn_random_secs).max(0) as i64;
    let max_s = (npc.respawn_secs + npc.respawn_random_secs) as i64;
    const UNITS: [(i64, &str); 4] = [
        (86400, "Days"),
        (3600, "Hours"),
        (60, "Minutes"),
        (1, "Seconds"),
    ];
    let (div, unit) = UNITS
        .into_iter()
        .find(|(u, _)| min_s % u == 0 && max_s % u == 0)
        .unwrap_or((1, "Seconds"));
    if npc.respawn_random_secs > 0 {
        format!("{}-{} {unit}", min_s / div, max_s / div)
    } else {
        format!("{} {unit}", min_s / div)
    }
}

enum GaugeKind {
    Hp,
    Mp,
}

/// `HtmlUtil.getGauge` with `displayAsPercentage = false` — a filled bar over a
/// `current / max` label, as `getHpGauge`/`getMpGauge` call it in `sendNpcView`.
fn gauge(width: i64, current_value: i64, max: i64, kind: GaugeKind) -> String {
    let (bg, fg) = match kind {
        GaugeKind::Hp => (
            "L2UI_CT1.Gauges.Gauge_DF_Large_HP_bg_Center",
            "L2UI_CT1.Gauges.Gauge_DF_Large_HP_Center",
        ),
        GaugeKind::Mp => (
            "L2UI_CT1.Gauges.Gauge_DF_Large_MP_bg_Center",
            "L2UI_CT1.Gauges.Gauge_DF_Large_MP_Center",
        ),
    };
    let image_height = if matches!(kind, GaugeKind::Hp) {
        21
    } else {
        17
    };
    let max = max.max(1); // guard div-by-zero (Java NPCs always have max hp/mp).
    let current = current_value.min(max);
    let fill = ((current as f64 / max as f64) * width as f64) as i64;
    let td_width = (width - 10) / 2;
    format!(
        "<table width={width} cellpadding=0 cellspacing=0><tr>\
         <td background=\"{bg}\"><img src=\"{fg}\" width={fill} height={image_height}></td></tr>\
         <tr><td align=center><table cellpadding=0 cellspacing=-13><tr><td>\
         <table cellpadding=0 cellspacing=0><tr>\
         <td width={td_width} align=right>{current}</td>\
         <td width=10 align=center>/</td>\
         <td width={td_width}>{max}</td>\
         </tr></table></td></tr></table></td></tr></table>"
    )
}
