//! Castle Chamberlain NPC (of Light / of Darkness) — port of
//! `dist/game/data/scripts/ai/others/CastleChamberlain/CastleChamberlain.java`,
//! narrowed to the **manor entry** (G26) and the **castle vault**. The
//! chamberlain is the castle owner's console; the rest of it (reports,
//! functions, siege info, products) routes to nothing yet.
//!
//! Flow: click → [`on_first_talk`] serves the owner main menu
//! (`chamberlain-01.html`) or the non-owner page (`chamberlain-04.html`). The
//! "Manage manor" button (`Quest CastleChamberlain manor`) opens `manor.html`
//! for an authorized owner, whose buttons then send `manor_menu_select` to the
//! manor display packets (see [`crate::game_loop::manor`]). The "Manage vault"
//! branch (`manage_vault*`, `deposit <n>`, `withdraw <n>`) moves adena between
//! the player and the castle treasury — see [`crate::game_loop::castle`].

use crate::game_loop::castle::{add_to_treasury_no_tax, format_adena, treasury};
use crate::game_loop::manor::{castle_owner_clan_id, chamberlain_castle_id};
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::Player;
use crate::model::clan::{CS_MANOR_ADMIN, CS_TAXES};
use crate::network::server_packets::sm_ids;
use crate::network::server_packets::{SmParam, system_message_with};

/// `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;

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
        let mut tokens = event.split(' ');
        match tokens.next().unwrap_or("") {
            "manor" => manor(ctx),
            // The three vault pages differ only in file: all show the balance.
            "manage_vault" => vault_page(ctx, "castlemanagevault.html"),
            "manage_vault_deposit" => vault_page(ctx, "castlemanagevault_deposit.html"),
            "manage_vault_withdraw" => vault_page(ctx, "castlemanagevault_withdraw.html"),
            "deposit" => deposit(ctx, amount_token(&mut tokens)),
            "withdraw" => withdraw(ctx, amount_token(&mut tokens)),
            // TODO(G24/G26): the rest of the chamberlain console
            // (receive_report, manage_functions, functions, list_siege_clans,
            // products, …) — unported console branches.
            _ => None,
        }
    }
}

/// Java `Long.parseLong(st.nextToken())` with its `hasMoreTokens() ? … : 0`
/// guard. A non-numeric token throws in Java (the event is then dropped); the
/// port folds that to 0, which every caller treats as "do nothing".
fn amount_token<'a>(tokens: &mut impl Iterator<Item = &'a str>) -> i64 {
    tokens
        .next()
        .map_or(0, |t| t.trim().parse::<i64>().unwrap_or(0))
}

/// The three `manage_vault*` pages: `CS_TAXES` owners see the balance,
/// everyone else the refusal page.
fn vault_page(ctx: &mut QuestCtx, file: &str) -> Option<String> {
    if !vault_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    let balance = format_adena(castle_treasury(ctx));
    Some(ctx.get_htm(file).replace("%tax_income%", &balance))
}

/// Java `case "deposit"`: `0 < amount < MAX_ADENA`, the player must hold it,
/// and it lands in the treasury untaxed. Java always returns the main page —
/// including when the amount was out of range or unaffordable (the "not enough
/// adena" system message is the only feedback).
fn deposit(ctx: &mut QuestCtx, amount: i64) -> Option<String> {
    if !vault_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    if amount > 0 && amount < ctx.world.cfg.character.max_adena {
        if ctx.quest_items_count(ADENA_ID) >= amount {
            ctx.take_items(ADENA_ID, amount);
            if let Some(castle_id) = chamberlain_castle_id(ctx.npc_id) {
                add_to_treasury_no_tax(ctx.world, castle_id, amount);
            }
        } else if let Some(cs) = ctx.world.clients.get(&ctx.client_id) {
            cs.send(system_message_with(
                sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA,
                &[],
            ));
        }
    }
    Some("chamberlain-01.html".to_string())
}

/// Java `case "withdraw"`: any amount up to the balance is paid out; asking for
/// more opens `castlenotenoughbalance.html`. Note Java's gate is
/// `amount <= treasury` with **no lower bound**, so a 0 (or malformed) amount
/// takes the success branch and pays nothing — kept verbatim.
fn withdraw(ctx: &mut QuestCtx, amount: i64) -> Option<String> {
    if !vault_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    let balance = castle_treasury(ctx);
    if amount > balance {
        let page = ctx
            .get_htm("castlenotenoughbalance.html")
            .replace("%tax_income%", &format_adena(balance))
            .replace("%withdraw_amount%", &format_adena(amount));
        return Some(page);
    }
    if let Some(castle_id) = chamberlain_castle_id(ctx.npc_id) {
        add_to_treasury_no_tax(ctx.world, castle_id, -amount);
    }
    ctx.give_adena(amount, false);
    Some("chamberlain-01.html".to_string())
}

/// Java's shared vault gate: `isOwner(player, npc) && hasClanPrivilege(CS_TAXES)`.
fn vault_access(ctx: &QuestCtx) -> bool {
    is_owner(ctx) && has_priv(ctx, CS_TAXES)
}

/// This chamberlain's castle vault balance.
fn castle_treasury(ctx: &QuestCtx) -> i64 {
    chamberlain_castle_id(ctx.npc_id).map_or(0, |id| treasury(ctx.world, id))
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
