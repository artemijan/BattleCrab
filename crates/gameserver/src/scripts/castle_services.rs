//! The castle-staff NPCs from `ai/others` — everyone inside a castle who
//! answers only to its owning clan:
//!
//! - `CastleBlacksmith` — the owner's blacksmith console.
//! - `CastleWarehouse` — the warehouse keeper + the Blood Alliance exchange.
//! - `CastleMercenaryManager` — mercenary-ticket buy lists and the limit pages.
//! - `CastleDoorManager` — open/close the castle gates, jump between posts.
//! - `CastleSiegeManager` — the siege registration window.
//! - `CastleTeleporter` — the defenders' battlefield gatekeepers, plus the
//!   mass gatekeeper's `MASS_TELEPORT` (in [`crate::game_loop::area_npcs`],
//!   since it fires with nobody in particular attached).
//!
//! All six resolve their castle the way Java does — `npc.getCastle()` =
//! `CastleManager.findNearestCastle` = [`super::super::data::zone_data::ZoneData
//! ::nearest_castle_at`] — rather than through a hand-written id table, and
//! share the rights helpers below (`isMyLord` / owning clan / clan privilege /
//! the GM `CASTLE_CONDITIONS` override).
//!
//! Door ids and teleport destinations come from the NPC *template*
//! `<parameters>` (`DoorId1`, `pos_x01`, …), which is where this dist puts them
//! — the spawn entries carry none.
//!
//! **`CastleSideEffect` is deliberately not ported**: it pushes `ExCastleState`
//! (the Grand Crusade castle-side banner) on town-zone entry, a packet the
//! Interlude client has no opcode for.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::Player;
use crate::model::clan::{CS_MANOR_ADMIN, CS_MERCENARIES};
use crate::model::siege::SiegeClanType;

/// `ClanPrivilege.CS_OPEN_DOOR` (ordinal 16) — open/close the castle doors and
/// use the doormen's teleports.
const CS_OPEN_DOOR: i32 = 1 << 16;

// ---------------------------------------------------------------------------
// Shared rights helpers (Java's per-script `hasRights`/`isOwningClan`/`isOwner`)
// ---------------------------------------------------------------------------

/// `npc.getCastle().getResidenceId()`.
fn castle_id(ctx: &QuestCtx) -> Option<i32> {
    let pos = ctx
        .world
        .objects
        .get_component::<crate::model::components::Position>(&ctx.npc)?;
    let (x, y, z) = (pos.x, pos.y, pos.z);
    ctx.world.data.zone_data.nearest_castle_at(x, y, z)
}

/// `player.canOverrideCond(PlayerCondOverride.CASTLE_CONDITIONS)` — a GM here,
/// as everywhere else in this port.
fn can_override(ctx: &QuestCtx) -> bool {
    ctx.world
        .objects
        .get_component::<Player>(&ctx.player)
        .is_some_and(|p| p.is_gm(&ctx.world.data))
}

fn player_clan_id(ctx: &QuestCtx) -> i32 {
    ctx.world
        .objects
        .get_component::<Player>(&ctx.player)
        .map_or(0, |p| p.clan_id)
}

/// `npc.getCastle().getOwnerId() == player.getClanId()` (with the clanless
/// player excluded, as every Java caller does).
fn is_owning_clan(ctx: &QuestCtx) -> bool {
    let clan_id = player_clan_id(ctx);
    if clan_id == 0 {
        return false;
    }
    castle_id(ctx).and_then(|id| crate::game_loop::siege::owner_clan_id_opt(ctx.world, id))
        == Some(clan_id)
}

/// Java's `isMyLord`: the clan **leader** of the clan that owns this castle.
fn is_my_lord(ctx: &QuestCtx) -> bool {
    ctx.is_clan_leader()
        && ctx
            .world
            .objects
            .get_component::<Player>(&ctx.player)
            .and_then(|p| ctx.world.clans.get(&p.clan_id))
            .map(|c| c.castle_id)
            == castle_id(ctx)
}

/// `player.hasClanPrivilege(...)` — the leader holds every privilege.
fn has_priv(ctx: &QuestCtx, privilege: i32) -> bool {
    let Some(p) = ctx.world.objects.get_component::<Player>(&ctx.player) else {
        return false;
    };
    ctx.world
        .clans
        .get(&p.clan_id)
        .is_some_and(|c| c.has_privilege(ctx.player, p.clan_privs, privilege))
}

/// `npc.getCastle().getSiege().isInProgress()`.
fn siege_in_progress(ctx: &QuestCtx) -> bool {
    castle_id(ctx).is_some_and(|id| ctx.world.sieges.get(&id).is_some_and(|s| s.in_progress))
}

/// `player.getSiegeState() == 2` — a defender: the flag Java stamps on the
/// owner clan and the approved defender clans while their siege runs.
fn is_defender(ctx: &QuestCtx) -> bool {
    let clan_id = player_clan_id(ctx);
    if clan_id == 0 {
        return false;
    }
    let Some(id) = castle_id(ctx) else {
        return false;
    };
    ctx.world.sieges.get(&id).is_some_and(|s| {
        s.in_progress
            && s.clans.iter().any(|c| {
                c.clan_id == clan_id
                    && matches!(c.kind, SiegeClanType::Owner | SiegeClanType::Defender)
            })
    })
}

/// An NPC-template `<parameters><param>` int (Java `npc.getParameters()
/// .getInt(name, 0)`).
fn npc_param(ctx: &QuestCtx, name: &str) -> i32 {
    ctx.world
        .data
        .npc_data
        .get(ctx.npc_id)
        .map_or(0, |t| t.ai_param_i32(name, 0))
}

// ---------------------------------------------------------------------------
// CastleBlacksmith
// ---------------------------------------------------------------------------

pub struct CastleBlacksmith;

const BLACKSMITHS: &[i32] = &[
    35098, // Gludio
    35140, // Dion
    35182, // Giran
    35224, // Oren
    35272, // Aden
    35314, // Innadril
    35361, // Goddard
    35507, // Rune
    35553, // Schuttgart
];

impl QuestScript for CastleBlacksmith {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "CastleBlacksmith"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/CastleBlacksmith"
    }
    fn start_npcs(&self) -> &[i32] {
        BLACKSMITHS
    }
    fn talk_npcs(&self) -> &[i32] {
        BLACKSMITHS
    }
    fn first_talk_npcs(&self) -> &[i32] {
        BLACKSMITHS
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        Some(if blacksmith_rights(ctx) {
            format!("{}-01.html", ctx.npc_id)
        } else {
            "no.html".to_string()
        })
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    /// Java: the single `<npcId>-02.html` page, and only with rights.
    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        let page = format!("{}-02.html", ctx.npc_id);
        (event.eq_ignore_ascii_case(&page) && blacksmith_rights(ctx)).then_some(page)
    }
}

/// Java `CastleBlacksmith.hasRights`: cond-override, or the castle's lord, or a
/// clan member holding `CS_MANOR_ADMIN`.
fn blacksmith_rights(ctx: &QuestCtx) -> bool {
    can_override(ctx) || is_my_lord(ctx) || (is_owning_clan(ctx) && has_priv(ctx, CS_MANOR_ADMIN))
}

// ---------------------------------------------------------------------------
// CastleWarehouse
// ---------------------------------------------------------------------------

pub struct CastleWarehouse;

const WAREHOUSE_KEEPERS: &[i32] = &[
    35099, 35141, 35183, 35225, 35273, 35315, 35362, 35508, 35554,
];

/// The siege reward currencies the keeper hands out (Java `BLOOD_OATH` /
/// `BLOOD_ALLIANCE`) — both ship in this dist's item data.
const BLOOD_OATH: i32 = 9910;
const BLOOD_ALLIANCE: i32 = 9911;

/// Java's pass-through pages.
const WAREHOUSE_PAGES: &[&str] = &[
    "warehouse-01.html",
    "warehouse-02.html",
    "warehouse-03.html",
];

impl QuestScript for CastleWarehouse {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "CastleWarehouse"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/CastleWarehouse"
    }
    fn start_npcs(&self) -> &[i32] {
        WAREHOUSE_KEEPERS
    }
    fn talk_npcs(&self) -> &[i32] {
        WAREHOUSE_KEEPERS
    }
    fn first_talk_npcs(&self) -> &[i32] {
        WAREHOUSE_KEEPERS
    }

    fn on_first_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        Some("warehouse-01.html".to_string())
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if WAREHOUSE_PAGES.contains(&event) {
            return Some(event.to_string());
        }
        let my_lord = is_my_lord(ctx);
        match event {
            // The Blood Alliance page shows the clan's unclaimed count.
            "warehouse-04.html" => Some(if my_lord {
                ctx.get_htm("warehouse-04.html")
                    .replace("%blood%", &blood_alliance_count(ctx).to_string())
            } else {
                "warehouse-no.html".to_string()
            }),
            // Claim the Blood Alliances the clan earned defending its castle.
            "Receive" => Some(if !my_lord {
                "warehouse-no.html".to_string()
            } else {
                let count = blood_alliance_count(ctx);
                if count == 0 {
                    "warehouse-05.html".to_string()
                } else {
                    ctx.give_items(BLOOD_ALLIANCE, count as i64);
                    set_blood_alliance_count(ctx, 0);
                    "warehouse-06.html".to_string()
                }
            }),
            // One Blood Alliance → 30 Blood Oaths.
            "Exchange" => Some(if !my_lord {
                "warehouse-no.html".to_string()
            } else if ctx.quest_items_count(BLOOD_ALLIANCE) == 0 {
                "warehouse-08.html".to_string()
            } else {
                ctx.take_items(BLOOD_ALLIANCE, 1);
                ctx.give_items(BLOOD_OATH, 30);
                "warehouse-07.html".to_string()
            }),
            _ => None,
        }
    }
}

/// `player.getClan().getBloodAllianceCount()`.
fn blood_alliance_count(ctx: &QuestCtx) -> i32 {
    let clan_id = player_clan_id(ctx);
    ctx.world
        .clans
        .get(&clan_id)
        .map_or(0, |c| c.blood_alliance_count)
}

/// `Clan.resetBloodAllianceCount()` — mirrors `siege::increase_blood_alliance`
/// on the way down: update the live clan and write the column through.
fn set_blood_alliance_count(ctx: &mut QuestCtx, value: i32) {
    let clan_id = player_clan_id(ctx);
    let Some(clan) = ctx.world.clans.get_mut(&clan_id) else {
        return;
    };
    clan.blood_alliance_count = value;
    let _ = ctx
        .world
        .db
        .send(crate::db::DbCommand::UpdateClanBloodAlliance {
            clan_id,
            count: value,
        });
}

// ---------------------------------------------------------------------------
// CastleMercenaryManager
// ---------------------------------------------------------------------------

pub struct CastleMercenaryManager;

const MERCENARY_MANAGERS: &[i32] = &[
    35102, 35144, 35186, 35228, 35276, 35318, 35365, 35511, 35557,
];

impl QuestScript for CastleMercenaryManager {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "CastleMercenaryManager"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/CastleMercenaryManager"
    }
    fn start_npcs(&self) -> &[i32] {
        MERCENARY_MANAGERS
    }
    fn talk_npcs(&self) -> &[i32] {
        MERCENARY_MANAGERS
    }
    fn first_talk_npcs(&self) -> &[i32] {
        MERCENARY_MANAGERS
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        Some(mercenary_main(ctx))
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        let mut tokens = event.split(' ');
        match tokens.next().unwrap_or("") {
            // The ticket-limit page: Aden and Rune have their own, and every
            // page names the castle through the client string `1001000 + id`.
            "limit" => {
                let id = castle_id(ctx).unwrap_or(0);
                let file = match id {
                    5 => "mercmanager-aden-limit.html",
                    8 => "mercmanager-rune-limit.html",
                    _ => "mercmanager-limit.html",
                };
                Some(
                    ctx.get_htm(file)
                        .replace("%feud_name%", &(1_001_000 + id).to_string()),
                )
            }
            // `buy <n>` → the merchant buy list `<npcId><n>` (Java notes these
            // lists are not castle-taxed; the port applies no tax anyway).
            "buy" => {
                if let Some(list_id) = tokens
                    .next()
                    .and_then(|n| format!("{}{}", ctx.npc_id, n.trim()).parse::<i32>().ok())
                {
                    crate::game_loop::shop::show_buy_window(
                        ctx.world,
                        ctx.client_id,
                        ctx.player,
                        ctx.npc,
                        list_id,
                    );
                }
                None
            }
            "main" => Some(mercenary_main(ctx)),
            "mercmanager-01.html" => Some("mercmanager-01.html".to_string()),
            _ => None,
        }
    }
}

/// Java `onFirstTalk`: the console for an authorized owner (a siege-time
/// variant while the castle is under attack), otherwise the refusal page.
fn mercenary_main(ctx: &mut QuestCtx) -> String {
    if can_override(ctx) || (is_owning_clan(ctx) && has_priv(ctx, CS_MERCENARIES)) {
        if siege_in_progress(ctx) {
            "mercmanager-siege.html"
        } else {
            "mercmanager.html"
        }
    } else {
        "mercmanager-no.html"
    }
    .to_string()
}

// ---------------------------------------------------------------------------
// CastleDoorManager
// ---------------------------------------------------------------------------

pub struct CastleDoorManager;

/// Java's `DOORMEN_OUTTER` — the outer-gate doormen.
const DOORMEN_OUTTER: &[i32] = &[
    35096, 35138, 35180, 35222, 35267, 35312, 35356, 35503, 35548,
];
/// Java's `DOORMEN_INNER`.
const DOORMEN_INNER: &[i32] = &[
    35097, 35139, 35181, 35223, 35268, 35269, 35270, 35271, 35313, 35357, 35358, 35359, 35360,
    35504, 35505, 35549, 35550, 35551, 35552,
];

const DOORMEN_ALL: &[i32] = &[
    35096, 35138, 35180, 35222, 35267, 35312, 35356, 35503, 35548, 35097, 35139, 35181, 35223,
    35268, 35269, 35270, 35271, 35313, 35357, 35358, 35359, 35360, 35504, 35505, 35549, 35550,
    35551, 35552,
];

impl QuestScript for CastleDoorManager {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "CastleDoorManager"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/CastleDoorManager"
    }
    fn start_npcs(&self) -> &[i32] {
        DOORMEN_ALL
    }
    fn talk_npcs(&self) -> &[i32] {
        DOORMEN_ALL
    }
    fn first_talk_npcs(&self) -> &[i32] {
        DOORMEN_ALL
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let base = doorman_html(ctx.npc_id);
        Some(if doorman_rights(ctx) && has_priv(ctx, CS_OPEN_DOOR) {
            format!("{base}.html")
        } else {
            format!("{base}-no.html")
        })
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        let mut tokens = event.split(' ');
        let action = tokens.next().unwrap_or("");
        let base = doorman_html(ctx.npc_id);
        match action {
            "manageDoors" => {
                let Some(arg) = tokens.next() else {
                    return None;
                };
                if !doorman_rights(ctx) {
                    return Some(format!("{base}-no.html"));
                }
                // The gates are frozen while the castle is under siege.
                if siege_in_progress(ctx) {
                    return Some("CastleDoorManager-siege.html".to_string());
                }
                let open = arg == "1";
                for param in ["DoorId1", "DoorId2"] {
                    let door_id = npc_param(ctx, param);
                    if door_id != 0 {
                        crate::game_loop::doors::set_door_by_id(ctx.world, door_id, open);
                    }
                }
                None
            }
            // The doorman's two posts (`pos_*01` / `pos_*02`).
            "teleport" => {
                let Some(arg) = tokens.next() else {
                    return None;
                };
                if !doorman_rights(ctx) {
                    return Some(format!("{base}-no.html"));
                }
                let suffix = if arg == "1" { "01" } else { "02" };
                let (x, y, z) = (
                    npc_param(ctx, &format!("pos_x{suffix}")),
                    npc_param(ctx, &format!("pos_y{suffix}")),
                    npc_param(ctx, &format!("pos_z{suffix}")),
                );
                ctx.teleport_to(x, y, z);
                None
            }
            _ => None,
        }
    }
}

/// Java `getHtmlName`: the inner and outer doormen share two page sets.
fn doorman_html(npc_id: i32) -> &'static str {
    if DOORMEN_INNER.contains(&npc_id) {
        "CastleDoorManager-Inner"
    } else {
        debug_assert!(DOORMEN_OUTTER.contains(&npc_id), "unknown doorman {npc_id}");
        "CastleDoorManager-Outter"
    }
}

/// Java `CastleDoorManager.isOwningClan` — note it does **not** ask for
/// `CS_OPEN_DOOR` (only `onFirstTalk` does), so a clan member who reached the
/// page can still work the gates.
fn doorman_rights(ctx: &QuestCtx) -> bool {
    can_override(ctx) || is_owning_clan(ctx)
}

// ---------------------------------------------------------------------------
// CastleSiegeManager
// ---------------------------------------------------------------------------

pub struct CastleSiegeManager;

const SIEGE_MANAGERS: &[i32] = &[
    35104, 35146, 35188, 35232, 35278, 35320, 35367, 35513, 35559,
];

impl QuestScript for CastleSiegeManager {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "CastleSiegeManager"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/CastleSiegeManager"
    }
    fn start_npcs(&self) -> &[i32] {
        SIEGE_MANAGERS
    }
    fn talk_npcs(&self) -> &[i32] {
        SIEGE_MANAGERS
    }
    fn first_talk_npcs(&self) -> &[i32] {
        SIEGE_MANAGERS
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let in_siege = siege_in_progress(ctx);
        // The owner's leader gets the console; everyone else gets the siege
        // notice, or — outside a siege — the registration window itself.
        if ctx.is_clan_leader() && is_owning_clan(ctx) {
            return Some(
                if in_siege {
                    "CastleSiegeManager.html"
                } else {
                    "CastleSiegeManager-01.html"
                }
                .to_string(),
            );
        }
        if in_siege {
            return Some("CastleSiegeManager-02.html".to_string());
        }
        if let Some(id) = castle_id(ctx) {
            crate::game_loop::siege::list_register_clan(ctx.world, ctx.client_id, ctx.player, id);
        }
        None
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }
}

// ---------------------------------------------------------------------------
// CastleTeleporter
// ---------------------------------------------------------------------------

pub struct CastleTeleporter;

/// Java's `MASS_TELEPORTERS` — the gatekeeper that pulls the whole castle back
/// inside when the walls fall.
pub(crate) const MASS_TELEPORTERS: &[i32] = &[
    35095, 35137, 35179, 35221, 35266, 35311, 35355, 35502, 35547,
];

/// Java's `SIEGE_TELEPORTERS` — the per-post battlefield gatekeepers.
const SIEGE_TELEPORTERS: &[i32] = &[
    35092, 35093, 35094, // Gludio
    35134, 35135, 35136, // Dion
    35176, 35177, 35178, // Giran
    35218, 35219, 35220, // Oren
    35261, 35262, 35263, 35264, 35265, // Aden
    35308, 35309, 35310, // Innadril
    35352, 35353, 35354, // Goddard
    35497, 35498, 35499, 35500, 35501, // Rune
    35544, 35545, 35546, // Schuttgart
];

const TELEPORTERS_ALL: &[i32] = &[
    35095, 35137, 35179, 35221, 35266, 35311, 35355, 35502, 35547, 35092, 35093, 35094, 35134,
    35135, 35136, 35176, 35177, 35178, 35218, 35219, 35220, 35261, 35262, 35263, 35264, 35265,
    35308, 35309, 35310, 35352, 35353, 35354, 35497, 35498, 35499, 35500, 35501, 35544, 35545,
    35546,
];

impl QuestScript for CastleTeleporter {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "CastleTeleporter"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/CastleTeleporter"
    }
    fn start_npcs(&self) -> &[i32] {
        TELEPORTERS_ALL
    }
    fn talk_npcs(&self) -> &[i32] {
        TELEPORTERS_ALL
    }
    fn first_talk_npcs(&self) -> &[i32] {
        TELEPORTERS_ALL
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        if MASS_TELEPORTERS.contains(&ctx.npc_id) {
            // Already counting down → the "on its way" page; otherwise the
            // urgent page while the towers are down mid-siege.
            if ctx.npc_script_value() != 0 {
                return Some("CastleTeleporter-06.html".to_string());
            }
            let towers_down = castle_id(ctx).is_some_and(|id| {
                ctx.world
                    .sieges
                    .get(&id)
                    .is_some_and(|s| s.in_progress && s.control_tower_count == 0)
            });
            return Some(
                if towers_down {
                    "CastleTeleporter-05.html"
                } else {
                    "CastleTeleporter-04.html"
                }
                .to_string(),
            );
        }
        debug_assert!(
            SIEGE_TELEPORTERS.contains(&ctx.npc_id),
            "unknown castle teleporter {}",
            ctx.npc_id
        );
        let base = teleporter_html(ctx.npc_id);
        Some(if is_owning_clan(ctx) && is_defender(ctx) {
            format!("{base}.html")
        } else {
            format!("{base}-no.html")
        })
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        let mut tokens = event.split(' ');
        match tokens.next().unwrap_or("") {
            // Arming the mass teleport: 8 minutes normally, 30 seconds once the
            // control towers are gone (Java's `MASS_TELEPORT` timer).
            "CastleTeleporter-06.html" => {
                if ctx.npc_script_value() == 0 {
                    let towers_down = castle_id(ctx).is_some_and(|id| {
                        ctx.world
                            .sieges
                            .get(&id)
                            .is_some_and(|s| s.in_progress && s.control_tower_count == 0)
                    });
                    let delay_ms = if towers_down { 480_000 } else { 30_000 };
                    crate::game_loop::area_npcs::arm_castle_mass_teleport(
                        ctx.world, ctx.npc, delay_ms,
                    );
                    ctx.set_npc_script_value(1);
                }
                Some("CastleTeleporter-06.html".to_string())
            }
            // `teleportMe <n>`: post `n`'s three candidate spots, one picked at
            // random; post 5 is the lord's own and refuses everyone else.
            "teleportMe" => {
                let Some(n) = tokens.next().and_then(|s| s.trim().parse::<i32>().ok()) else {
                    return None;
                };
                if n == 5 {
                    if !is_teleporter_owner(ctx) {
                        return Some("CastleTeleporter-noAuthority.html".to_string());
                    }
                    let (x, y, z) = (
                        npc_param(ctx, "pos_x51"),
                        npc_param(ctx, "pos_y51"),
                        npc_param(ctx, "pos_z51"),
                    );
                    ctx.teleport_to(x, y, z);
                    return None;
                }
                if !(0..=4).contains(&n) {
                    return None;
                }
                // Java's `getTeleportLocation`: <33 → the first spot, else
                // <66 on a *second* roll → the second, else the third.
                let suffix = if ctx.roll(100) < 33 {
                    format!("{n}1")
                } else if ctx.roll(100) < 66 {
                    format!("{n}2")
                } else {
                    format!("{n}3")
                };
                let (x, y, z) = (
                    npc_param(ctx, &format!("pos_x{suffix}")),
                    npc_param(ctx, &format!("pos_y{suffix}")),
                    npc_param(ctx, &format!("pos_z{suffix}")),
                );
                ctx.teleport_to(x, y, z);
                None
            }
            _ => None,
        }
    }
}

/// Java `CastleTeleporter.isOwner`: the castle owner's **clan leader** (or a
/// cond-override GM).
fn is_teleporter_owner(ctx: &QuestCtx) -> bool {
    can_override(ctx) || (is_owning_clan(ctx) && ctx.is_clan_leader())
}

/// Java `getHtmlName`: the first three posts of every castle share pages 01–03;
/// Aden's and Rune's extra posts use per-npc pages.
fn teleporter_html(npc_id: i32) -> String {
    match npc_id {
        35092 | 35134 | 35176 | 35218 | 35308 | 35352 | 35544 => "CastleTeleporter-01".to_string(),
        35093 | 35135 | 35177 | 35219 | 35309 | 35353 | 35545 => "CastleTeleporter-02".to_string(),
        35094 | 35136 | 35178 | 35220 | 35310 | 35354 | 35546 => "CastleTeleporter-03".to_string(),
        other => other.to_string(),
    }
}

/// Guard against the id lists drifting apart (`SIEGE_TELEPORTERS` +
/// `MASS_TELEPORTERS` must equal `TELEPORTERS_ALL`, same for the doormen).
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npc_id_lists_agree() {
        let mut joined: Vec<i32> = MASS_TELEPORTERS
            .iter()
            .chain(SIEGE_TELEPORTERS)
            .copied()
            .collect();
        joined.sort_unstable();
        let mut all = TELEPORTERS_ALL.to_vec();
        all.sort_unstable();
        assert_eq!(joined, all, "teleporter id lists agree");

        let mut doormen: Vec<i32> = DOORMEN_OUTTER
            .iter()
            .chain(DOORMEN_INNER)
            .copied()
            .collect();
        doormen.sort_unstable();
        let mut all_doormen = DOORMEN_ALL.to_vec();
        all_doormen.sort_unstable();
        assert_eq!(doormen, all_doormen, "doorman id lists agree");
    }
}
