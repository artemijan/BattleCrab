//! Port of `handlers/actionshifthandlers/NpcActionShift`'s **GM branch** — the
//! `data/html/admin/npcinfo.htm` window a GM gets from shift-clicking an NPC.
//!
//! Java routes the shift-click through `Action` case 1: a GM always reaches
//! `Npc.onActionShift` → `NpcActionShift`, whose first branch (`player.isGM()`)
//! targets the NPC and serves the admin html; only a non-GM falls through to
//! the `ALT_GAME_VIEWNPC` player view
//! ([`npc_view`](crate::game_loop::npc_view)).
//!
//! Scope vs. Java, all of it html-cosmetic:
//! - `%mpReward*%` — the MP-reward system is Goddess-era; no Interlude npc
//!   template declares it and the port doesn't parse it, so the line shows
//!   `0 NONE 0 NONE` (Java's own defaults for a template without one).
//! - `%ele_*%` — Interlude Classic has no elemental attributes (same `NONE`/`0`
//!   block the player view sends).
//! - Basic stats (`%str%`…) come off the template rather than a finalized stat
//!   pipeline: NPCs carry no `BaseStats` component here, and no Interlude npc
//!   skill touches the base stats, so the two agree.
//!
//! Every button on the window is live: `Kill`/`Delete`/`Recall`/`Buffs` reach
//! their admin commands, `Quests` is `AdminQuest`'s `//show_quests` (the NPC
//! script listing — *not* `//charquestmenu`), and `NpcView`/`Skills`/
//! `AggroList` bypass into [`npc_view`](crate::game_loop::npc_view).

use crate::data::npc_data::{AiType, NpcTemplate};
use crate::model::components::{CombatStats, Position, Speeds, Vitals};
use crate::model::npc::{Npc, NpcAi, NpcIntention};
use crate::world::World;

/// `NpcActionShift.action`'s GM branch. The caller has already set the
/// player's target (Java does it inside this branch, before building the
/// html).
pub(crate) fn send_npc_info(
    world: &World,
    client_id: u32,
    viewer_object_id: i32,
    npc_object_id: i32,
) {
    let Some(npc) = world.objects.get_component::<Npc>(&npc_object_id) else {
        return;
    };
    let Some(t) = npc.template(world) else { return };
    let (Some(vitals), Some(pos)) = (
        world.objects.get_component::<Vitals>(&npc_object_id),
        world.objects.get_component::<Position>(&npc_object_id),
    ) else {
        return;
    };
    let stats = world
        .objects
        .get_component::<CombatStats>(&npc_object_id)
        .copied()
        .unwrap_or_default();
    let run_spd = world
        .objects
        .get_component::<Speeds>(&npc_object_id)
        .map(|s| s.move_speed())
        .unwrap_or(t.base_run_spd);

    let (d2d, d3d) = match world.objects.get_component::<Position>(&viewer_object_id) {
        Some(v) => {
            let (dx, dy, dz) = (
                (pos.x - v.x) as f64,
                (pos.y - v.y) as f64,
                (pos.z - v.z) as f64,
            );
            (
                (dx * dx + dy * dy).sqrt() as i64,
                (dx * dx + dy * dy + dz * dz).sqrt() as i64,
            )
        }
        None => (0, 0),
    };

    // `ClanHallData.getClanHallByNpcId` — the hall this NPC is an agent of.
    let clan_hall = world
        .data
        .clan_halls
        .values()
        .find(|h| h.npcs.contains(&t.id))
        .map(|h| h.name.clone())
        .unwrap_or_else(|| "none".to_string());

    let (spawn_file, spawn_name, spawn_group, spawn_ai) = spawn_line_labels(world, npc, t);
    let replacements: Vec<(&str, String)> = vec![
        ("objid", npc_object_id.to_string()),
        // Java prints the runtime class simple name; the template `type` *is*
        // that class name (`Monster`, `Folk`, …).
        ("class", t.type_name.clone()),
        ("race", race_name(t).to_string()),
        ("id", t.id.to_string()),
        ("tmplid", t.id.to_string()),
        ("lvl", t.level.to_string()),
        ("name", t.name.clone()),
        (
            "aggro",
            if t.is_attackable_class() {
                t.aggro_range.to_string()
            } else {
                "0".to_string()
            },
        ),
        ("hp", (vitals.cur_hp as i64).to_string()),
        ("hpmax", vitals.max_hp.to_string()),
        ("mp", (vitals.cur_mp as i64).to_string()),
        ("mpmax", vitals.max_mp.to_string()),
        ("exp", (t.exp as i64).to_string()),
        ("sp", (t.sp as i64).to_string()),
        ("patk", (stats.p_atk as i64).to_string()),
        ("matk", (stats.m_atk as i64).to_string()),
        ("pdef", (stats.p_def as i64).to_string()),
        ("mdef", (stats.m_def as i64).to_string()),
        ("accu", stats.accuracy.to_string()),
        ("evas", stats.evasion.to_string()),
        ("crit", (stats.crit_hit as i64).to_string()),
        ("rspd", (run_spd as i64).to_string()),
        ("aspd", stats.p_atk_spd.to_string()),
        ("cspd", stats.m_atk_spd.to_string()),
        (
            "atkType",
            crate::game_loop::npc_view::attack_type_name(world, t),
        ),
        ("atkRng", t.base_atk_range.to_string()),
        ("str", t.base_str.to_string()),
        ("dex", t.base_dex.to_string()),
        ("con", t.base_con.to_string()),
        ("int", t.base_int.to_string()),
        ("wit", t.base_wit.to_string()),
        ("men", t.base_men.to_string()),
        ("loc", format!("{} {} {}", pos.x, pos.y, pos.z)),
        ("heading", pos.heading.to_string()),
        ("collision_radius", t.collision_radius.to_string()),
        ("collision_height", t.collision_height.to_string()),
        ("clanHall", clan_hall),
        // No MP-reward system in Interlude — Java's template defaults.
        ("mpRewardValue", "0".to_string()),
        ("mpRewardTicks", "0".to_string()),
        ("mpRewardType", "DIFF".to_string()),
        ("mpRewardAffectType", "SOLO".to_string()),
        ("loc2d", d2d.to_string()),
        ("loc3d", d3d.to_string()),
        // Interlude Classic has no elemental attributes.
        ("ele_atk", "NONE".to_string()),
        ("ele_atk_value", "0".to_string()),
        ("ele_dfire", "0".to_string()),
        ("ele_dwater", "0".to_string()),
        ("ele_dwind", "0".to_string()),
        ("ele_dearth", "0".to_string()),
        ("ele_dholy", "0".to_string()),
        ("ele_ddark", "0".to_string()),
        ("spawnfile", spawn_file),
        ("spawnname", spawn_name),
        ("spawngroup", spawn_group),
        ("spawnai", spawn_ai),
        (
            "spawn",
            format!(
                "{} {} {}",
                npc.spawn_loc.0, npc.spawn_loc.1, npc.spawn_loc.2
            ),
        ),
        ("resp", respawn_text(npc)),
        // Java reads these three off `Npc.getSpawn()`, which exists for every
        // spawned NPC (admin and quest spawns build one too) — here they live
        // on the `Npc` component itself, so the `spawn == null` fallback branch
        // is unreachable.
        ("chaseRange", npc.chase_range.to_string()),
        ("route", route_row(world, t.id)),
    ];

    let mut replacements = replacements;
    replacements.extend(ai_rows(world, npc_object_id, t));
    super::menu::show_admin_html_replace(world, client_id, "npcinfo.htm", &replacements);
}

/// Java's `<font color=FF0000>--</font>` placeholder for the spawn fields it
/// cannot resolve.
const MISSING: &str = "<font color=FF0000>--</font>";

/// `Spawn.getNpcSpawnTemplate()`'s file/name/group/AI labels. `Npc.spawn_ref`
/// is `(0, 0, 0)` for runtime spawns (minions, quest and `//spawn` NPCs), which
/// would otherwise read an unrelated spawn line — so the reference only counts
/// when the line it points at actually declares this npc id, mirroring Java's
/// `getNpcSpawnTemplate() == null` fallback.
fn spawn_line_labels(
    world: &World,
    npc: &Npc,
    t: &NpcTemplate,
) -> (String, String, String, String) {
    let line = world
        .data
        .spawn_data
        .spawns
        .get(npc.spawn_ref.0)
        .and_then(|s| s.groups.get(npc.spawn_ref.1).map(|g| (s, g)))
        .filter(|(_, g)| {
            g.npcs
                .get(npc.spawn_ref.2)
                .is_some_and(|n| n.npc_id == t.id)
        });
    let Some((template, group)) = line else {
        return (
            MISSING.to_string(),
            MISSING.to_string(),
            MISSING.to_string(),
            MISSING.to_string(),
        );
    };
    // Java uses `String.valueOf` on both — an unnamed template/group prints
    // "null", not an empty cell.
    let name = template.name.clone().unwrap_or_else(|| "null".to_string());
    let group_name = group.name.clone().unwrap_or_else(|| "null".to_string());
    let ai = match &template.ai {
        // Java links the AI to `admin_quest_info` when the name resolves to a
        // loaded script; ours are Rust `NpcScript`s with no such registry
        // lookup, so the name shows as the plain red label Java falls back to.
        Some(ai) => format!("<font color=FF0000>{ai}</font>"),
        None => "<font color=FF0000>null</font>".to_string(),
    };
    (template.file.clone(), name, group_name, ai)
}

/// `NpcActionShift`'s respawn line: `None` when the line never respawns, else
/// `min-max sec` (randomised) or `min sec`. Java prints seconds here — unlike
/// `NpcViewMod`, which picks a coarser unit.
fn respawn_text(npc: &Npc) -> String {
    if npc.respawn_secs == 0 {
        return "None".to_string();
    }
    if npc.respawn_random_secs > 0 {
        let min = (npc.respawn_secs - npc.respawn_random_secs).max(0);
        let max = npc.respawn_secs + npc.respawn_random_secs;
        return format!("{min}-{max} sec");
    }
    format!("{} sec", npc.respawn_secs)
}

/// `WalkingManager.getRouteName(npc)` — the patrol route row, or nothing.
fn route_row(world: &World, npc_id: i32) -> String {
    match world.data.routes.route_for_npc(npc_id) {
        Some((_, route)) => format!(
            "<tr><td><table width=270 border=0><tr><td width=100><font color=LEVEL>Route:</font>\
             </td><td align=right width=170>{}</td></tr></table></td></tr>",
            route.name
        ),
        None => String::new(),
    }
}

/// The five `%ai*%` rows, present only when the NPC has an AI (Java
/// `npc.hasAI()`) — a plain `Folk` shows none of them.
fn ai_rows(world: &World, npc_object_id: i32, t: &NpcTemplate) -> Vec<(&'static str, String)> {
    let Some(ai) = world.objects.get_component::<NpcAi>(&npc_object_id) else {
        return vec![
            ("ai_intention", String::new()),
            ("ai", String::new()),
            ("ai_type", String::new()),
            ("ai_clan", String::new()),
            ("ai_enemy_clan", String::new()),
        ];
    };
    let ignore_clans = t
        .ignore_clan_npc_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    vec![
        (
            "ai_intention",
            row(true, "Intention:", intention_name(ai.intention)),
        ),
        // Every AI-driven NPC here runs the one `AttackableAI` port.
        ("ai", row(false, "AI", "AttackableAI".to_string())),
        (
            "ai_type",
            row(true, "AIType", ai_type_name(t.ai_type).into()),
        ),
        (
            "ai_clan",
            row(
                false,
                "Clan & Range:",
                format!("{} {}", t.clans.join(", "), t.clan_help_range),
            ),
        ),
        (
            "ai_enemy_clan",
            row(
                true,
                "Ignore & Range:",
                format!("{ignore_clans} {}", t.aggro_range),
            ),
        ),
    ]
}

/// One `%ai*%` table row; the shaded rows alternate exactly as in Java.
fn row(shaded: bool, label: &str, value: String) -> String {
    let bg = if shaded { " bgcolor=131210" } else { "" };
    format!(
        "<tr><td><table width=270 border=0{bg}><tr><td width=100><font color=FFAA00>{label}</font>\
         </td><td align=right width=170>{value}</td></tr></table></td></tr>"
    )
}

/// `CtrlIntention.name()` for the intentions the NPC AI port models.
fn intention_name(i: NpcIntention) -> String {
    match i {
        NpcIntention::Active => "AI_INTENTION_ACTIVE",
        NpcIntention::Attack => "AI_INTENTION_ATTACK",
        NpcIntention::MoveTo => "AI_INTENTION_MOVE_TO",
    }
    .to_string()
}

/// `AIType.name()`.
fn ai_type_name(t: AiType) -> &'static str {
    match t {
        AiType::Fighter => "FIGHTER",
        AiType::Archer => "ARCHER",
        AiType::Balanced => "BALANCED",
        AiType::Mage => "MAGE",
        AiType::Healer => "HEALER",
        AiType::Corpse => "CORPSE",
    }
}

/// `NpcTemplate.getRace().toString()` — the `Race` enum constant name. Java
/// prints a bare `null` for the templates that declare no race.
fn race_name(t: &NpcTemplate) -> &'static str {
    let Some(race) = t.race.and_then(crate::enums::Race::from_ordinal) else {
        return "null";
    };
    use crate::enums::Race;
    match race {
        Race::Human => "HUMAN",
        Race::Elf => "ELF",
        Race::DarkElf => "DARK_ELF",
        Race::Orc => "ORC",
        Race::Dwarf => "DWARF",
        Race::Kamael => "KAMAEL",
        Race::Ertheia => "ERTHEIA",
        Race::Animal => "ANIMAL",
        Race::Beast => "BEAST",
        Race::Bug => "BUG",
        Race::CastleGuard => "CASTLE_GUARD",
        Race::Construct => "CONSTRUCT",
        Race::Demonic => "DEMONIC",
        Race::Divine => "DIVINE",
        Race::Dragon => "DRAGON",
        Race::Elemental => "ELEMENTAL",
        Race::Etc => "ETC",
        Race::Fairy => "FAIRY",
        Race::Giant => "GIANT",
        Race::Humanoid => "HUMANOID",
        Race::Mercenary => "MERCENARY",
        Race::None_ => "NONE",
        Race::Plant => "PLANT",
        Race::SiegeWeapon => "SIEGE_WEAPON",
        Race::Undead => "UNDEAD",
        Race::Friend => "FRIEND",
    }
}
