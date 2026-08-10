//! Castle Chamberlain NPC (of Light / of Darkness) — the full port of
//! `dist/game/data/scripts/ai/others/CastleChamberlain/CastleChamberlain.java`:
//! the castle owner's console. Reports, the vault, door open/close and door /
//! wall / trap upgrades, the five rentable castle functions (recovery /
//! teleport / buffer pages, the confirm + `set_func` flow), banishing
//! foreigners, the siege-clan list, the manor entry, the product shop, and
//! the lord's cloak + crown. The function *effects* live with their systems:
//! regen in [`crate::game_loop::regen`], the revive exp-restore in the
//! restart path, teleport/buffer right here.
//!
//! Fort status (`fort_status` / `chamberlain-28.html`) renders an empty list:
//! Java iterates its castle→fortress table and skips every fortress
//! `FortManager` can't produce — and this port has no fortresses, so the page
//! is what Java shows with none loaded.

use crate::game_loop::castle::{
    add_to_treasury_no_tax, banish_foreigners, castle_function, door_upgrade_ratio, format_adena,
    remove_castle_function, set_door_upgrade, set_trap_upgrade, trap_upgrade_level, treasury,
    update_castle_function,
};
use crate::game_loop::manor::{castle_owner_clan_id, chamberlain_castle_id};
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::Player;
use crate::model::castle::{
    CastleSide, FUNC_RESTORE_EXP, FUNC_RESTORE_HP, FUNC_RESTORE_MP, FUNC_SUPPORT, FUNC_TELEPORT,
};
use crate::model::clan::{
    CS_DISMISS, CS_MANAGE_SIEGE, CS_MANOR_ADMIN, CS_OPEN_DOOR, CS_SET_FUNCTIONS, CS_TAXES,
    CS_USE_FUNCTIONS,
};
use crate::network::server_packets::SmParam;
use crate::network::server_packets::sm_ids;

/// `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;
/// The Lord's Crown and the two side cloaks.
const CROWN: i32 = 6841;
const LORD_CLOAK_OF_LIGHT: i32 = 34925;
const LORD_CLOAK_OF_DARK: i32 = 34926;

/// Java `BUFFS` — the buffer function's 28-entry cast list, indexed by the
/// `cast_buff <index>` bypass off `castlebuff-05/08.html`.
const BUFFS: [(i32, i32); 28] = [
    (4342, 2), // Wind Walk
    (4343, 3), // Decrease Weight
    (4344, 3), // Shield
    (4346, 4), // Mental Shield
    (4345, 3), // Might
    (4347, 2), // Bless the Body
    (4349, 1), // Magic Barrier
    (4350, 1), // Resist Shock
    (4348, 2), // Bless the Soul
    (4351, 2), // Concentration
    (4352, 1), // Berserker Spirit
    (4353, 2), // Bless Shield
    (4358, 1), // Guidance
    (4354, 1), // Vampiric Rage
    (4347, 6), // Bless the Body
    (4349, 2), // Magic Barrier
    (4350, 4), // Resist Shock
    (4348, 6), // Bless the Soul
    (4351, 6), // Concentration
    (4352, 2), // Berserker Spirit
    (4353, 6), // Bless Shield
    (4358, 3), // Guidance
    (4354, 4), // Vampiric Rage
    (4355, 1), // Acumen
    (4356, 1), // Empower
    (4357, 1), // Haste
    (4359, 1), // Focus
    (4360, 1), // Death Whisper
];

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
        // The static navigation pages (Java's pass-through `case`s).
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
            "fort_status" => fort_status(ctx),
            "siege_functions" => siege_functions(ctx),
            "receive_report" => receive_report(ctx),
            "manage_functions" => manage_functions(ctx),
            "banish_foreigner_show" => banish_show(ctx),
            "banish_foreigner" => banish(ctx),
            "doors" => doors_page(ctx),
            "operate_door" => operate_door(ctx, &mut tokens),
            "manage_doors" => manage_doors(ctx, event),
            "upgrade_doors" => upgrade_doors(ctx, event),
            "upgrade_doors_confirm" => upgrade_doors_confirm(ctx, &mut tokens),
            "manage_trap" => manage_trap(ctx, &mut tokens),
            "upgrade_trap" => upgrade_trap(ctx, &mut tokens),
            "upgrade_trap_confirm" => upgrade_trap_confirm(ctx, &mut tokens),
            "additional_functions" => Some(
                if set_func_access(ctx) {
                    "castletdecomanage.html"
                } else {
                    "chamberlain-21.html"
                }
                .to_string(),
            ),
            "recovery" => recovery_page(ctx),
            "other" => other_page(ctx),
            "HP" => func_confirm(ctx, FUNC_RESTORE_HP, amount_token(&mut tokens) as i32),
            "MP" => func_confirm(ctx, FUNC_RESTORE_MP, amount_token(&mut tokens) as i32),
            "XP" => func_confirm(ctx, FUNC_RESTORE_EXP, amount_token(&mut tokens) as i32),
            "TP" => func_confirm(ctx, FUNC_TELEPORT, amount_token(&mut tokens) as i32),
            "BF" => func_confirm(ctx, FUNC_SUPPORT, amount_token(&mut tokens) as i32),
            "set_func" => set_func(ctx, &mut tokens),
            "functions" => functions_page(ctx),
            "teleport" => teleport(ctx),
            "goto" => goto_teleport(ctx, &mut tokens),
            "buffer" => buffer_page(ctx),
            "cast_buff" => cast_buff(ctx, &mut tokens),
            "list_siege_clans" => list_siege_clans(ctx),
            "products" => products(ctx),
            "buy" => buy(ctx, &mut tokens),
            "give_cloak" => give_cloak(ctx),
            "give_crown" => give_crown(ctx),
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
        } else {
            crate::game_loop::helpers::send_sm_bare_to_client(
                ctx.world,
                ctx.client_id,
                sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA,
            );
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
    is_owner(ctx) && ctx.has_clan_privilege(CS_TAXES)
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
        crate::game_loop::helpers::send_sm_to_client(
            ctx.world,
            ctx.client_id,
            sm_ids::S1_TEXT,
            &[SmParam::Text("Manor system is deactivated.".to_string())],
        );
        return None;
    }
    Some(
        if is_owner(ctx) && ctx.has_clan_privilege(CS_MANOR_ADMIN) {
            "manor.html"
        } else {
            "chamberlain-21.html"
        }
        .to_string(),
    )
}

// ---------------------------------------------------------------------------
// Reports, banish, doors
// ---------------------------------------------------------------------------

/// `fort_status` (`chamberlain-28.html`): this port has no fortresses, so the
/// contract list renders exactly as Java does with none loaded — empty.
fn fort_status(ctx: &mut QuestCtx) -> Option<String> {
    if !is_my_lord(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    Some(ctx.get_htm("chamberlain-28.html").replace("%list%", ""))
}

fn siege_functions(ctx: &mut QuestCtx) -> Option<String> {
    Some(
        if !set_func_access(ctx) {
            "chamberlain-21.html"
        } else if siege_in_progress(ctx) {
            "chamberlain-08.html"
        } else {
            "chamberlain-12.html"
        }
        .to_string(),
    )
}

/// `receive_report` (`chamberlain-02.html`): the lord's overview page.
fn receive_report(ctx: &mut QuestCtx) -> Option<String> {
    if !is_my_lord(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    if siege_in_progress(ctx) {
        return Some("chamberlain-07.html".to_string());
    }
    let castle_id = chamberlain_castle_id(ctx.npc_id)?;
    let clan_id = castle_owner_clan_id(ctx.world, castle_id)?;
    let (clan_name, leader_name) = ctx
        .world
        .clans
        .get(&clan_id)
        .map(|c| {
            let leader = c
                .members
                .iter()
                .find(|m| m.char_id == c.leader_id)
                .map(|m| m.name.clone())
                .unwrap_or_default();
            (c.name.clone(), leader)
        })
        .unwrap_or_default();
    Some(
        ctx.get_htm("chamberlain-02.html")
            .replace("%clanleadername%", &leader_name)
            .replace("%clanname%", &clan_name)
            .replace("%castlename%", &(1_001_000 + castle_id).to_string()),
    )
}

fn manage_functions(ctx: &mut QuestCtx) -> Option<String> {
    Some(
        if !is_owner(ctx) {
            "chamberlain-21.html"
        } else if siege_in_progress(ctx) {
            "chamberlain-08.html"
        } else {
            "chamberlain-23.html"
        }
        .to_string(),
    )
}

fn banish_show(ctx: &mut QuestCtx) -> Option<String> {
    Some(
        if !is_owner(ctx) || !ctx.has_clan_privilege(CS_DISMISS) {
            "chamberlain-21.html"
        } else if siege_in_progress(ctx) {
            "chamberlain-08.html"
        } else {
            "chamberlain-10.html"
        }
        .to_string(),
    )
}

fn banish(ctx: &mut QuestCtx) -> Option<String> {
    if !(is_owner(ctx) && ctx.has_clan_privilege(CS_DISMISS)) {
        return Some("chamberlain-21.html".to_string());
    }
    if siege_in_progress(ctx) {
        return Some("chamberlain-08.html".to_string());
    }
    if let Some(castle_id) = chamberlain_castle_id(ctx.npc_id) {
        banish_foreigners(ctx.world, castle_id);
    }
    Some("chamberlain-11.html".to_string())
}

/// `doors` — the castle's named door page (`<Name>-d.html`).
fn doors_page(ctx: &mut QuestCtx) -> Option<String> {
    Some(if !is_owner(ctx) || !ctx.has_clan_privilege(CS_OPEN_DOOR) {
        "chamberlain-21.html".to_string()
    } else if siege_in_progress(ctx) {
        "chamberlain-08.html".to_string()
    } else {
        format!("{}-d.html", castle_name(ctx))
    })
}

/// `operate_door <1|0> <doorId>…` — open/close each named door.
fn operate_door<'a>(
    ctx: &mut QuestCtx,
    tokens: &mut impl Iterator<Item = &'a str>,
) -> Option<String> {
    if !is_owner(ctx) || !ctx.has_clan_privilege(CS_OPEN_DOOR) {
        return Some("chamberlain-21.html".to_string());
    }
    if siege_in_progress(ctx) {
        return Some("chamberlain-08.html".to_string());
    }
    let open = tokens.next().and_then(|t| t.parse::<i32>().ok()) == Some(1);
    for door_id in tokens.filter_map(|t| t.parse::<i32>().ok()) {
        if open {
            crate::game_loop::doors::open_door_by_id(ctx.world, door_id);
        } else {
            close_door_by_id(ctx.world, door_id);
        }
    }
    Some(
        if open {
            "chamberlain-05.html"
        } else {
            "chamberlain-06.html"
        }
        .to_string(),
    )
}

/// `Door.closeMe` by door id (the mirror of `open_door_by_id`).
fn close_door_by_id(world: &mut crate::world::World, door_id: i32) {
    let oid = world.door_regions.values().flatten().copied().find(|oid| {
        world
            .objects
            .get_component::<crate::model::components::InstanceDoorOpen>(oid)
            .is_none()
            && world
                .objects
                .get_component::<crate::model::door::Door>(oid)
                .is_some_and(|d| d.door_id == door_id)
    });
    if let Some(oid) = oid {
        crate::game_loop::doors::close_door(world, oid);
    }
}

// ---------------------------------------------------------------------------
// Door / trap upgrades
// ---------------------------------------------------------------------------

/// `manage_doors [type doors…]` — the pick page, or the castle's `-du` menu.
fn manage_doors(ctx: &mut QuestCtx, event: &str) -> Option<String> {
    if !set_func_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    let rest: Vec<&str> = event.split(' ').skip(1).collect();
    if rest.is_empty() {
        return Some(format!("{}-du.html", castle_name(ctx)));
    }
    Some(
        ctx.get_htm("chamberlain-13.html")
            .replace("%type%", rest[0])
            .replace("%doors%", &format!(" {}", rest[1..].join(" "))),
    )
}

/// `upgrade_doors <type> <level> <doors…>` — the price-confirm page.
fn upgrade_doors(ctx: &mut QuestCtx, event: &str) -> Option<String> {
    if !set_func_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    let rest: Vec<&str> = event.split(' ').skip(1).collect();
    let ty = rest.first()?.parse::<i32>().ok()?;
    let level = rest.get(1)?.parse::<i32>().ok()?;
    Some(
        ctx.get_htm("chamberlain-14.html")
            .replace(
                "%gate_price%",
                &door_upgrade_price(ctx, ty, level).to_string(),
            )
            .replace("%event%", &rest.join(" ")),
    )
}

/// `upgrade_doors_confirm <type> <level> <doorId> [doorId]` — pay and apply.
fn upgrade_doors_confirm<'a>(
    ctx: &mut QuestCtx,
    tokens: &mut impl Iterator<Item = &'a str>,
) -> Option<String> {
    if !set_func_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    if siege_in_progress(ctx) {
        return Some("chamberlain-08.html".to_string());
    }
    let ty = tokens.next()?.parse::<i32>().ok()?;
    let level = tokens.next()?.parse::<i32>().ok()?;
    let price = door_upgrade_price(ctx, ty, level);
    let doors: Vec<i32> = tokens.filter_map(|t| t.parse::<i32>().ok()).collect();
    let &first = doors.first()?;
    let current = door_upgrade_ratio(ctx.world, first);
    if current >= level {
        return Some(
            ctx.get_htm("chamberlain-15.html")
                .replace("%doorlevel%", &current.to_string()),
        );
    }
    if price <= 0 || ctx.quest_items_count(ADENA_ID) < price {
        return Some("chamberlain-09.html".to_string());
    }
    ctx.take_items(ADENA_ID, price);
    for door_id in doors {
        set_door_upgrade(ctx.world, door_id, level);
    }
    Some("chamberlain-16.html".to_string())
}

fn door_upgrade_price(ctx: &QuestCtx, ty: i32, level: i32) -> i64 {
    let slot = match level {
        2 => 0,
        3 => 1,
        5 => 2,
        _ => return 0,
    };
    match ty {
        1..=3 => ctx.world.cfg.feature.door_upgrade_price[(ty - 1) as usize][slot],
        _ => 0,
    }
}

/// `manage_trap [index]` — the pick page (Aden's differs), or the `-tu` menu.
fn manage_trap<'a>(
    ctx: &mut QuestCtx,
    tokens: &mut impl Iterator<Item = &'a str>,
) -> Option<String> {
    if !set_func_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    match tokens.next() {
        Some(index) => {
            let file = if castle_name(ctx).eq_ignore_ascii_case("aden") {
                "chamberlain-17a.html"
            } else {
                "chamberlain-17.html"
            };
            Some(ctx.get_htm(file).replace("%trapIndex%", index))
        }
        None => Some(format!("{}-tu.html", castle_name(ctx))),
    }
}

/// `upgrade_trap <index> <level>` — the price-confirm page.
fn upgrade_trap<'a>(
    ctx: &mut QuestCtx,
    tokens: &mut impl Iterator<Item = &'a str>,
) -> Option<String> {
    if !set_func_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    let index = tokens.next()?;
    let level = tokens.next()?.parse::<i32>().ok()?;
    Some(
        ctx.get_htm("chamberlain-18.html")
            .replace("%trapIndex%", index)
            .replace("%level%", &level.to_string())
            .replace("%dmgzone_price%", &trap_price(ctx, level).to_string()),
    )
}

fn upgrade_trap_confirm<'a>(
    ctx: &mut QuestCtx,
    tokens: &mut impl Iterator<Item = &'a str>,
) -> Option<String> {
    if !set_func_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    if siege_in_progress(ctx) {
        return Some("chamberlain-08.html".to_string());
    }
    let index = tokens.next()?.parse::<i32>().ok()?;
    let level = tokens.next()?.parse::<i32>().ok()?;
    let castle_id = chamberlain_castle_id(ctx.npc_id)?;
    let current = trap_upgrade_level(ctx.world, castle_id, index);
    if current >= level {
        return Some(
            ctx.get_htm("chamberlain-19.html")
                .replace("%dmglevel%", &current.to_string()),
        );
    }
    let price = trap_price(ctx, level);
    if price <= 0 || ctx.quest_items_count(ADENA_ID) < price {
        return Some("chamberlain-09.html".to_string());
    }
    ctx.take_items(ADENA_ID, price);
    set_trap_upgrade(ctx.world, castle_id, index, level);
    Some("chamberlain-20.html".to_string())
}

fn trap_price(ctx: &QuestCtx, level: i32) -> i64 {
    match level {
        1..=4 => ctx.world.cfg.feature.trap_upgrade_price[(level - 1) as usize],
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Castle functions: the recovery/other pages, confirm, set, use
// ---------------------------------------------------------------------------

/// The per-function fee for a level (Java `getFunctionFee`).
fn function_fee(ctx: &QuestCtx, func: i32, level: i32) -> i64 {
    let f = &ctx.world.cfg.feature;
    match func {
        FUNC_RESTORE_EXP => f.cs_expreg_fee[usize::from(level != 45)],
        FUNC_RESTORE_HP => f.cs_hpreg_fee[usize::from(level != 300)],
        FUNC_RESTORE_MP => f.cs_mpreg_fee[usize::from(level != 40)],
        FUNC_SUPPORT => f.cs_support_fee[usize::from(level != 5)],
        FUNC_TELEPORT => f.cs_tele_fee[usize::from(level != 1)],
        _ => 0,
    }
}

/// The per-function rental period (Java `getFunctionRatio`), in ms.
fn function_ratio(ctx: &QuestCtx, func: i32) -> i64 {
    let f = &ctx.world.cfg.feature;
    match func {
        FUNC_RESTORE_EXP => f.cs_expreg_fee_ratio,
        FUNC_RESTORE_HP => f.cs_hpreg_fee_ratio,
        FUNC_RESTORE_MP => f.cs_mpreg_fee_ratio,
        FUNC_SUPPORT => f.cs_support_fee_ratio,
        FUNC_TELEPORT => f.cs_tele_fee_ratio,
        _ => 0,
    }
}

/// Java `funcReplace`: fill one function's Depth/Cost/Expire/Reset slots on
/// the recovery/other pages.
fn func_replace(ctx: &QuestCtx, html: String, func: i32, tag: &str) -> String {
    let castle_id = chamberlain_castle_id(ctx.npc_id).unwrap_or(0);
    match castle_function(ctx.world, castle_id, func) {
        None => html
            .replace(&format!("%{tag}Depth%"), "<fstring>4</fstring>")
            .replace(&format!("%{tag}Cost%"), "")
            .replace(&format!("%{tag}Expire%"), "<fstring>4</fstring>")
            .replace(&format!("%{tag}Reset%"), ""),
        Some(f) => {
            let fstring = if func == FUNC_SUPPORT || func == FUNC_TELEPORT {
                "9"
            } else {
                "10"
            };
            let (day, month, year) = commons::util::date_parts(f.end_time);
            html.replace(
                &format!("%{tag}Depth%"),
                &format!("<fstring p1=\"{}\">{fstring}</fstring>", f.level),
            )
            .replace(
                &format!("%{tag}Cost%"),
                &format!(
                    "<fstring p1=\"{}\" p2=\"{}\">6</fstring>",
                    f.lease,
                    f.rate_ms / 86_400_000
                ),
            )
            .replace(
                &format!("%{tag}Expire%"),
                &format!("<fstring p1=\"{day}\" p2=\"{month}\" p3=\"{year}\">5</fstring>"),
            )
            .replace(
                &format!("%{tag}Reset%"),
                &format!(
                    "[<a action=\"bypass -h Quest CastleChamberlain {tag} 0\">Deactivate</a>]"
                ),
            )
        }
    }
}

fn recovery_page(ctx: &mut QuestCtx) -> Option<String> {
    if !set_func_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    let mut html = ctx.get_htm("castledeco-AR01.html");
    html = func_replace(ctx, html, FUNC_RESTORE_HP, "HP");
    html = func_replace(ctx, html, FUNC_RESTORE_MP, "MP");
    html = func_replace(ctx, html, FUNC_RESTORE_EXP, "XP");
    Some(html)
}

fn other_page(ctx: &mut QuestCtx) -> Option<String> {
    if !set_func_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    let mut html = ctx.get_htm("castledeco-AE01.html");
    html = func_replace(ctx, html, FUNC_TELEPORT, "TP");
    html = func_replace(ctx, html, FUNC_SUPPORT, "BF");
    Some(html)
}

/// Java `funcConfirmHtml`: the reset page for level 0, the already-set page
/// for the current level, else the fee-confirm page.
fn func_confirm(ctx: &mut QuestCtx, func: i32, level: i32) -> Option<String> {
    if !set_func_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    let castle_id = chamberlain_castle_id(ctx.npc_id)?;
    let fstring = if func == FUNC_TELEPORT { "9" } else { "10" };
    if level == 0 {
        return Some(
            ctx.get_htm("castleresetdeco.html")
                .replace("%AgitDecoSubmit%", &func.to_string()),
        );
    }
    if castle_function(ctx.world, castle_id, func).is_some_and(|f| f.level == level) {
        return Some(ctx.get_htm("castledecoalreadyset.html").replace(
            "%AgitDecoEffect%",
            &format!("<fstring p1=\"{level}\">{fstring}</fstring>"),
        ));
    }
    Some(
        ctx.get_htm(&format!("castledeco-0{func}.html"))
            .replace(
                "%AgitDecoCost%",
                &format!(
                    "<fstring p1=\"{}\" p2=\"{}\">6</fstring>",
                    function_fee(ctx, func, level),
                    function_ratio(ctx, func) / 86_400_000
                ),
            )
            .replace(
                "%AgitDecoEffect%",
                &format!("<fstring p1=\"{level}\">{fstring}</fstring>"),
            )
            .replace("%AgitDecoSubmit%", &format!("{func} {level}")),
    )
}

/// `set_func <func> <level>` — deactivate (0) or buy, paying from the buyer's
/// inventory (Java `updateFunctions`).
fn set_func<'a>(ctx: &mut QuestCtx, tokens: &mut impl Iterator<Item = &'a str>) -> Option<String> {
    if !set_func_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    let func = tokens.next()?.parse::<i32>().ok()?;
    let level = tokens.next()?.parse::<i32>().ok()?;
    let castle_id = chamberlain_castle_id(ctx.npc_id)?;
    if level == 0 {
        remove_castle_function(ctx.world, castle_id, func);
        return None;
    }
    let lease = function_fee(ctx, func, level);
    if lease > 0 && !ctx.take_items(ADENA_ID, lease) {
        return Some("chamberlain-09.html".to_string());
    }
    let ratio = function_ratio(ctx, func);
    update_castle_function(ctx.world, castle_id, func, level, lease, ratio);
    None
}

/// `functions` (`castledecofunction.html`) — the CS_USE view of the levels.
fn functions_page(ctx: &mut QuestCtx) -> Option<String> {
    if !use_func_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    let castle_id = chamberlain_castle_id(ctx.npc_id)?;
    let depth = |func: i32| {
        castle_function(ctx.world, castle_id, func)
            .map(|f| f.level.to_string())
            .unwrap_or_else(|| "0".to_string())
    };
    Some(
        ctx.get_htm("castledecofunction.html")
            .replace("%HPDepth%", &depth(FUNC_RESTORE_HP))
            .replace("%MPDepth%", &depth(FUNC_RESTORE_MP))
            .replace("%XPDepth%", &depth(FUNC_RESTORE_EXP)),
    )
}

/// `teleport` — show the rented teleport list (`tel<lvl>`), buttons routed
/// back through `Quest CastleChamberlain goto`.
fn teleport(ctx: &mut QuestCtx) -> Option<String> {
    if !use_func_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    let castle_id = chamberlain_castle_id(ctx.npc_id)?;
    let Some(func) = castle_function(ctx.world, castle_id, FUNC_TELEPORT) else {
        return Some("castlefuncdisabled.html".to_string());
    };
    let (client_id, player, npc) = (ctx.client_id, ctx.player, ctx.npc);
    crate::game_loop::teleporter::show_teleport_list(
        ctx.world,
        client_id,
        player,
        npc,
        &format!("tel{}", func.level),
        "Quest CastleChamberlain goto",
    );
    None
}

/// `goto <listId> <locId>` — teleport, re-verifying the rented level matches
/// the list the button came from (Java's `func.getLvl() == funcLvl`).
fn goto_teleport<'a>(
    ctx: &mut QuestCtx,
    tokens: &mut impl Iterator<Item = &'a str>,
) -> Option<String> {
    if !use_func_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    let castle_id = chamberlain_castle_id(ctx.npc_id)?;
    let Some(func) = castle_function(ctx.world, castle_id, FUNC_TELEPORT) else {
        return Some("castlefuncdisabled.html".to_string());
    };
    let list_id = tokens.next()?;
    let loc = tokens.next().and_then(|t| t.parse::<usize>().ok());
    let func_lvl = list_id.get(3..).and_then(|s| s.parse::<i32>().ok());
    if func_lvl == Some(func.level) {
        let (client_id, player, npc) = (ctx.client_id, ctx.player, ctx.npc);
        crate::game_loop::teleporter::do_teleport(ctx.world, client_id, player, npc, list_id, loc);
    }
    None
}

/// `buffer` (`castlebuff-05/08.html`) — the rented buff menu, with the NPC's
/// remaining MP.
fn buffer_page(ctx: &mut QuestCtx) -> Option<String> {
    if !use_func_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    let castle_id = chamberlain_castle_id(ctx.npc_id)?;
    let Some(func) = castle_function(ctx.world, castle_id, FUNC_SUPPORT) else {
        return Some("castlefuncdisabled.html".to_string());
    };
    Some(
        ctx.get_htm(&format!("castlebuff-0{}.html", func.level))
            .replace("%MPLeft%", &(npc_mp(ctx) as i32).to_string()),
    )
}

/// `cast_buff <index>` — the chamberlain casts off the `BUFFS` table, gated
/// on its own MP (Java `getMpConsume() < npc.getCurrentMp()`).
fn cast_buff<'a>(ctx: &mut QuestCtx, tokens: &mut impl Iterator<Item = &'a str>) -> Option<String> {
    if !use_func_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    let castle_id = chamberlain_castle_id(ctx.npc_id)?;
    if castle_function(ctx.world, castle_id, FUNC_SUPPORT).is_none() {
        return Some("castlefuncdisabled.html".to_string());
    }
    let index = tokens.next()?.parse::<usize>().ok()?;
    let &(skill_id, skill_lvl) = BUFFS.get(index)?;
    let cost = ctx
        .world
        .data
        .skill_data
        .get(skill_id, skill_lvl)
        .map(|sk| f64::from(sk.mp_consume + sk.mp_initial_consume))
        .unwrap_or(0.0);
    let file = if cost < npc_mp(ctx) {
        let (npc, player) = (ctx.npc, ctx.player);
        crate::game_loop::support_magic::cast_from_npc(
            ctx.world,
            npc,
            player,
            (skill_id, skill_lvl),
        );
        if let Some(v) = ctx
            .world
            .objects
            .get_component_mut::<crate::model::components::Vitals>(&npc)
        {
            v.cur_mp = (v.cur_mp - cost).max(0.0);
        }
        "castleafterbuff.html"
    } else {
        "castlenotenoughmp.html"
    };
    Some(
        ctx.get_htm(file)
            .replace("%MPLeft%", &(npc_mp(ctx) as i32).to_string()),
    )
}

fn npc_mp(ctx: &QuestCtx) -> f64 {
    ctx.world
        .objects
        .get_component::<crate::model::components::Vitals>(&ctx.npc)
        .map(|v| v.cur_mp)
        .unwrap_or(0.0)
}

// ---------------------------------------------------------------------------
// Siege list, products, cloak + crown
// ---------------------------------------------------------------------------

fn list_siege_clans(ctx: &mut QuestCtx) -> Option<String> {
    if !(is_owner(ctx) && ctx.has_clan_privilege(CS_MANAGE_SIEGE)) {
        return Some("chamberlain-21.html".to_string());
    }
    let castle_id = chamberlain_castle_id(ctx.npc_id)?;
    let (client_id, player) = (ctx.client_id, ctx.player);
    crate::game_loop::siege::list_register_clan(ctx.world, client_id, player, castle_id);
    None
}

fn products(ctx: &mut QuestCtx) -> Option<String> {
    if !use_func_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    Some(
        ctx.get_htm("chamberlain-22.html")
            .replace("%npcId%", &ctx.npc_id.to_string()),
    )
}

/// `buy <listId>` — the chamberlain's merchant window.
fn buy<'a>(ctx: &mut QuestCtx, tokens: &mut impl Iterator<Item = &'a str>) -> Option<String> {
    if !use_func_access(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    let list_id = tokens.next()?.parse::<i32>().ok()?;
    let (client_id, player, npc) = (ctx.client_id, ctx.player, ctx.npc);
    crate::game_loop::shop::show_buy_window_taxed(ctx.world, client_id, player, npc, list_id, true);
    None
}

/// `give_cloak` — the side-matched lord's cloak, once.
fn give_cloak(ctx: &mut QuestCtx) -> Option<String> {
    if siege_in_progress(ctx) {
        return Some("chamberlain-08.html".to_string());
    }
    if !is_my_lord(ctx) {
        return Some("chamberlain-29.html".to_string());
    }
    let castle_id = chamberlain_castle_id(ctx.npc_id)?;
    let dark = ctx
        .world
        .castles
        .iter()
        .find(|c| c.id == castle_id)
        .is_some_and(|c| c.side == CastleSide::Dark);
    let cloak = if dark {
        LORD_CLOAK_OF_DARK
    } else {
        LORD_CLOAK_OF_LIGHT
    };
    if ctx.quest_items_count(cloak) > 0 {
        return Some("chamberlain-03.html".to_string());
    }
    ctx.give_items(cloak, 1);
    None
}

/// `give_crown` — the Lord's Crown, once, with the presentation page.
fn give_crown(ctx: &mut QuestCtx) -> Option<String> {
    if siege_in_progress(ctx) {
        return Some("chamberlain-08.html".to_string());
    }
    if !is_my_lord(ctx) {
        return Some("chamberlain-21.html".to_string());
    }
    if ctx.quest_items_count(CROWN) > 0 {
        return Some("chamberlain-24.html".to_string());
    }
    let castle_id = chamberlain_castle_id(ctx.npc_id)?;
    let name = ctx
        .world
        .objects
        .get_component::<Player>(&ctx.player)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let html = ctx
        .get_htm("chamberlain-25.html")
        .replace("%owner_name%", &name)
        .replace("%feud_name%", &(1_001_000 + castle_id).to_string());
    ctx.give_items(CROWN, 1);
    Some(html)
}

// ---------------------------------------------------------------------------
// Shared gates
// ---------------------------------------------------------------------------

/// Java `isMyLord`: the clan *leader* whose clan owns this castle.
fn is_my_lord(ctx: &QuestCtx) -> bool {
    let Some(p) = ctx.world.objects.get_component::<Player>(&ctx.player) else {
        return false;
    };
    let Some(castle_id) = chamberlain_castle_id(ctx.npc_id) else {
        return false;
    };
    ctx.world
        .clans
        .get(&p.clan_id)
        .is_some_and(|c| c.leader_id == ctx.player && c.castle_id == castle_id)
}

fn set_func_access(ctx: &QuestCtx) -> bool {
    is_owner(ctx) && ctx.has_clan_privilege(CS_SET_FUNCTIONS)
}

fn use_func_access(ctx: &QuestCtx) -> bool {
    is_owner(ctx) && ctx.has_clan_privilege(CS_USE_FUNCTIONS)
}

fn siege_in_progress(ctx: &QuestCtx) -> bool {
    chamberlain_castle_id(ctx.npc_id)
        .and_then(|id| ctx.world.sieges.get(&id))
        .is_some_and(|s| s.in_progress)
}

/// The castle's display name ("Gludio", …) for the `<Name>-d/du/tu.html` pages.
fn castle_name(ctx: &QuestCtx) -> String {
    chamberlain_castle_id(ctx.npc_id)
        .and_then(|id| ctx.world.castles.iter().find(|c| c.id == id))
        .map(|c| c.name.clone())
        .unwrap_or_default()
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
