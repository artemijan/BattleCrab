//! Port of `handlers/bypasshandlers/NpcViewMod` — the shift-click NPC info
//! window (`AltGameViewNpc`). Java routes a non-GM shift-click on an NPC
//! through `Action` case 1 → `Npc.onActionShift` → `NpcActionShift`, whose
//! `ALT_GAME_VIEWNPC` branch calls `NpcViewMod.sendNpcView`; the info window's
//! "Show Drop" button then bypasses back here (`bypass NpcViewMod dropList
//! DROP <objId> [page]`). Entry points: [`send_npc_view`] (the window) and
//! [`handle_npc_view_bypass`] (the button router).
//!
//! Scope vs. Java:
//! - The **GM** branch (admin `npcinfo.htm`) is not modeled — the live `Player`
//!   carries no access level yet, so every shift-click takes the player path.
//! - `skills` / `aggrolist` sub-views aren't reachable from `Info.htm` (only
//!   from the admin htmls) and rest on NPC data the port doesn't carry (NPC
//!   skill lists), so only `view` + `droplist` are handled.
//! - Both the `DROP` and `SPOIL` scopes are handled: the info window offers a
//!   "Show Drop" and/or "Show Spoil" button per whichever list the NPC carries
//!   (`bypass NpcViewMod dropList <DROP|SPOIL> <objId> [page]`).

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
    let Some(npc) = world.objects.get_component::<crate::model::npc::Npc>(&npc_object_id) else { return };
    let Some(t) = npc.template(world) else { return };
    let (Some(vitals), Some(speeds), Some(stats)) = (
        world.objects.get_component::<Vitals>(&npc_object_id),
        world.objects.get_component::<Speeds>(&npc_object_id),
        world.objects.get_component::<crate::model::components::CombatStats>(&npc_object_id),
    ) else {
        return;
    };

    // Java `NpcHtmlMessage.setFile`: a missing file still sends the packet
    // (empty content, logged) — mirror `interact_with_npc`'s stub fallback so
    // the window always opens even when the datapack html is absent.
    let mut html = crate::data::htm_cache::read_htm(format!("{}data/html/mods/NpcView/Info.htm", world.data.root))
        .unwrap_or_else(|| "<html><body>NPC Info<br>%name%</body></html>".to_string());

    let mut set = |needle: &str, value: &str| html = html.replace(needle, value);
    set("%name%", &t.name);
    set("%hpGauge%", &gauge(250, vitals.cur_hp as i64, vitals.max_hp as i64, GaugeKind::Hp));
    set("%mpGauge%", &gauge(250, vitals.cur_mp as i64, vitals.max_mp as i64, GaugeKind::Mp));
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

    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::npc_html_message(npc_object_id, &html));
    }
}

/// `bypass NpcViewMod <verb> …` router (the info window's buttons). Only the
/// verbs reachable from `Info.htm` are handled; see the module doc.
pub(crate) fn handle_npc_view_bypass(world: &World, client_id: u32, object_id: i32, command: &str) {
    let mut it = command.split_whitespace();
    it.next(); // "NpcViewMod"
    let Some(verb) = it.next() else { return };
    match verb.to_ascii_lowercase().as_str() {
        "view" => {
            let target = it.next().and_then(|s| s.parse::<i32>().ok()).or_else(|| current_target(world, object_id));
            if let Some(npc_object_id) = target.filter(|id| world.objects.has_component::<crate::model::npc::Npc>(id)) {
                send_npc_view(world, client_id, npc_object_id);
            }
        }
        "droplist" => {
            // `dropList <DROP|SPOIL> <objId> [page]`.
            let scope = it.next().unwrap_or("");
            let Some(npc_object_id) = it.next().and_then(|s| s.parse::<i32>().ok()) else { return };
            let page = it.next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
            if world.objects.has_component::<crate::model::npc::Npc>(&npc_object_id) {
                send_npc_drop_list(world, client_id, npc_object_id, scope, page);
            }
        }
        _ => {}
    }
}

fn current_target(world: &World, object_id: i32) -> Option<i32> {
    world.objects.get_component::<crate::model::components::TargetRef>(&object_id).and_then(|t| t.0)
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
fn send_npc_drop_list(world: &World, client_id: u32, npc_object_id: i32, scope: &str, page_value: usize) {
    let is_spoil = scope.eq_ignore_ascii_case("SPOIL");
    if !is_spoil && !scope.eq_ignore_ascii_case("DROP") {
        return; // unknown scope: nothing to show.
    }
    // The scope token echoed back into paging bypasses (upper-case, like Java).
    let scope_token = if is_spoil { "SPOIL" } else { "DROP" };
    let Some(npc) = world.objects.get_component::<crate::model::npc::Npc>(&npc_object_id) else { return };
    let Some(t) = npc.template(world) else { return };

    // The list to show: the spoil list, or the combined death list (ungrouped
    // drops, then each group's drops with the group chance folded into the item
    // chance — Java `chance / 100`).
    let mut drop_list: Vec<DropHolder> = if is_spoil { t.drop_list_spoil.clone() } else { t.drop_list_death.clone() };
    if !is_spoil {
        for group in &t.drop_groups {
            let group_chance = group.chance / 100.0;
            for d in &group.items {
                drop_list.push(DropHolder { item_id: d.item_id, min: d.min, max: d.max, chance: d.chance * group_chance });
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

    let page = if page_value >= pages { pages.saturating_sub(1) } else { page_value };
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
                world.cfg.rates.drop_chance_by_id.get(&d.item_id).copied().unwrap_or(1.0),
                world.cfg.rates.drop_amount_by_id.get(&d.item_id).copied().unwrap_or(1.0),
            )
        };
        let rate_chance = rate_chance_base * chance_by_id;
        let rate_amount = rate_amount_base * amount_by_id;
        let name = world.data.item_data.get(d.item_id).map(|i| i.name.as_str()).unwrap_or("Unknown item");

        let min = (d.min as f64 * rate_amount) as i64;
        let max = (d.max as f64 * rate_amount) as i64;
        let amount = if min == max { format_amount(min) } else { format!("{} - {}", format_amount(min), format_amount(max)) };
        let chance = format!("{:.2}", (d.chance * rate_chance).min(100.0));

        let row = format!(
            "<table width=332 cellpadding=2 cellspacing=0 background=\"L2UI_CT1.Windows.Windows_DF_TooltipBG\">\
             <tr><td width=32 valign=top>\
             <button width=\"32\" height=\"32\" back=\"icon.etc_question_mark_i00\" fore=\"icon.etc_question_mark_i00\" itemtooltip=\"{}\"></td>\
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

    let Some(template_html) =
        crate::data::htm_cache::read_htm(format!("{}data/html/mods/NpcView/DropList.htm", world.data.root))
    else {
        return;
    };
    let body = format!("<table><tr><td>{left_sb}</td><td>{right_sb}</td></tr></table>{limit_reached}");
    let html = template_html
        .replace("%name%", &t.name)
        .replace("%dropListButtons%", &drop_list_buttons(t, npc_object_id))
        .replace("%pages%", &pages_sb)
        .replace("%items%", &body);
    // Java routes this through `Util.sendCBHtml` (a community-board wide html).
    // The window is opened here through the same `NpcHtmlMessage` path the
    // rest of the NPC dialogs use.
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::npc_html_message(npc_object_id, &html));
    }
}

/// `DecimalFormat("#,###")` — thousands-grouped integer.
fn format_amount(value: i64) -> String {
    let s = value.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if value < 0 {
        format!("-{out}")
    } else {
        out
    }
}

/// `Creature.getAttackType`: the equipped weapon's type, else `FIST`,
/// formatted like Java `CommonUtil.capitalizeFirst(name.toLowerCase())`.
fn attack_type_name(world: &World, t: &NpcTemplate) -> String {
    use crate::data::item_data::WeaponType;
    let wt = if t.rhand != 0 { world.data.item_data.weapon_type(t.rhand) } else { WeaponType::None };
    let wt = if wt == WeaponType::None { WeaponType::Fist } else { wt };
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
    const UNITS: [(i64, &str); 4] = [(86400, "Days"), (3600, "Hours"), (60, "Minutes"), (1, "Seconds")];
    let (div, unit) =
        UNITS.into_iter().find(|(u, _)| min_s % u == 0 && max_s % u == 0).unwrap_or((1, "Seconds"));
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
    let image_height = if matches!(kind, GaugeKind::Hp) { 21 } else { 17 };
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
