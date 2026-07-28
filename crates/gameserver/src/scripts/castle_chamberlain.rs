//! Castle Chamberlain NPC (of Light / of Darkness) — port of
//! `dist/game/data/scripts/ai/others/CastleChamberlain/CastleChamberlain.java`,
//! **narrowed to the manor entry** (G26). The chamberlain is the castle
//! owner's console; this slice wires only the "Manage manor" branch and its
//! help pages. The rest of the console (reports, vault, functions, siege info,
//! products) routes to nothing yet.
//!
//! Flow: click → [`on_first_talk`] serves the owner main menu
//! (`chamberlain-01.html`) or the non-owner page (`chamberlain-04.html`). The
//! "Manage manor" button (`Quest CastleChamberlain manor`) opens `manor.html`
//! for an authorized owner, whose buttons then send `manor_menu_select` to the
//! manor display packets (see [`crate::game_loop::manor`]).

use crate::game_loop::manor::{castle_owner_clan_id, chamberlain_castle_id};
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::Player;
use crate::model::clan::CS_MANOR_ADMIN;
use crate::network::server_packets::sm_ids;
use crate::network::server_packets::{SmParam, system_message_with};

pub struct CastleChamberlain;

/// The 18 chamberlain NPC ids (9 castles × Light/Dark), from
/// `CastleChamberlain.NPC`.
const CHAMBERLAIN_IDS: &[i32] = &[
    35100, 36653, // Gludio
    35142, 36654, // Dion
    35184, 36655, // Giran
    35226, 36656, // Oren
    35274, 36657, // Aden
    35316, 36658, // Innadril
    35363, 36659, // Goddard
    35509, 36660, // Rune
    35555, 36661, // Schuttgart
];

/// The static pages the manor branch navigates between (Java lists these as
/// pass-through `event`s in `onEvent`). Everything else is either a wired verb
/// or an unported console branch.
const MANOR_PAGES: &[&str] = &[
    "chamberlain-01.html",
    "manor-help-01.html",
    "manor-help-02.html",
    "manor-help-03.html",
    "manor-help-04.html",
];

impl QuestScript for CastleChamberlain {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "CastleChamberlain"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/CastleChamberlain"
    }
    fn start_npcs(&self) -> &[i32] {
        CHAMBERLAIN_IDS
    }
    fn talk_npcs(&self) -> &[i32] {
        CHAMBERLAIN_IDS
    }
    fn first_talk_npcs(&self) -> &[i32] {
        CHAMBERLAIN_IDS
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        // Java `onFirstTalk`: the owner console, or the "not your castle" page.
        Some(
            if is_owner(ctx) {
                "chamberlain-01.html"
            } else {
                "chamberlain-04.html"
            }
            .to_string(),
        )
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        // The manor navigation pages (Java's pass-through `case`s).
        if MANOR_PAGES.contains(&event) {
            return Some(event.to_string());
        }
        match event {
            "manor" => manor(ctx),
            // TODO(G24/G26): the rest of the chamberlain console
            // (receive_report, manage_vault, manage_functions, functions,
            // list_siege_clans, products, …) — unported console branches.
            _ => None,
        }
    }
}

/// Java `case "manor"`: gated on `Config.ALLOW_MANOR`; an authorized owner sees
/// `manor.html`, anyone else `chamberlain-21.html`; when the manor is disabled
/// the player just gets the "deactivated" chat line.
fn manor(ctx: &mut QuestCtx) -> Option<String> {
    if !ctx.world.cfg.general.allow_manor {
        if let Some(cs) = ctx.world.clients.get(&ctx.client_id) {
            cs.send(system_message_with(
                sm_ids::S1_TEXT,
                &[SmParam::Text("Manor system is deactivated.".to_string())],
            ));
        }
        return None;
    }
    Some(
        if is_owner(ctx) && has_priv(ctx, CS_MANOR_ADMIN) {
            "manor.html"
        } else {
            "chamberlain-21.html"
        }
        .to_string(),
    )
}

/// Java `isOwner`: `canOverrideCond(CASTLE_CONDITIONS)` (a GM here) or the
/// player's clan owns this chamberlain's castle.
fn is_owner(ctx: &QuestCtx) -> bool {
    let Some(player) = ctx.world.objects.get_component::<Player>(&ctx.player) else {
        return false;
    };
    if player.is_gm(&ctx.world.data) {
        return true;
    }
    let Some(castle_id) = chamberlain_castle_id(ctx.npc_id) else {
        return false;
    };
    player.clan_id != 0 && castle_owner_clan_id(ctx.world, castle_id) == Some(player.clan_id)
}

/// Java `player.hasClanPrivilege(...)`: the leader has all, otherwise the
/// member's rank privilege mask must carry the bit.
fn has_priv(ctx: &QuestCtx, privilege: i32) -> bool {
    let Some(p) = ctx.world.objects.get_component::<Player>(&ctx.player) else {
        return false;
    };
    ctx.world
        .clans
        .get(&p.clan_id)
        .is_some_and(|c| c.has_privilege(ctx.player, p.clan_privs, privilege))
}
