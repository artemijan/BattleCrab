//! Monument of Heroes NPC (31690) — where a hero claims their rewards: an
//! Infinity weapon, the Wings of Destiny Circlet, and (for the Olympiad's #1)
//! the Hero Cloak. Port of `dist/game/data/scripts/ai/others/MonumentOfHeroes/
//! MonumentOfHeroes.java`.
//!
//! The reward claims are wired here. The hero *list* (`heroList` → `ExHeroList`)
//! and the `heroCertification`/`heroConfirm` claim flow are deferred: this port
//! auto-crowns heroes at the Olympiad period end (there is no unclaimed-hero
//! step to certify), and `ExHeroList` isn't ported yet — both `TODO(G25)`.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::inventory::Inventory;
use crate::model::Player;
use crate::network::server_packets::sm_ids;

pub struct MonumentOfHeroes;

const MONUMENT: i32 = 31690;
const HERO_CLOAK: i32 = 30372;
const WINGS_OF_DESTINY_CIRCLET: i32 = 6842;
/// `MonumentOfHeroes.WEAPONS` — the Infinity hero weapons (one per weapon type).
const HERO_WEAPONS: &[i32] = &[
    6611, // Infinity Blade
    6612, // Infinity Cleaver
    6613, // Infinity Axe
    6614, // Infinity Rod
    6616, // Infinity Scepter
    6617, // Infinity Stinger
    6618, // Infinity Fang
    6619, // Infinity Bow
    6620, // Infinity Wing
    6621, // Infinity Spear
];

impl QuestScript for MonumentOfHeroes {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "MonumentOfHeroes"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/MonumentOfHeroes"
    }
    fn start_npcs(&self) -> &[i32] {
        &[MONUMENT]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[MONUMENT]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[MONUMENT]
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        Some(first_talk_page(ctx).to_string())
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if event.ends_with(".html") || event.ends_with(".htm") {
            return Some(event.to_string());
        }
        // `give_<weaponId>`: hand over the chosen Infinity weapon (only the
        // listed ids — Java's fixed `give_...` case labels).
        if let Some(id) = event.strip_prefix("give_").and_then(|s| s.parse().ok()) {
            if HERO_WEAPONS.contains(&id) {
                ctx.give_items(id, 1);
            }
            return None;
        }
        match event {
            "index" => Some(first_talk_page(ctx).to_string()),
            "heroWeapon" => hero_weapon(ctx),
            "heroCirclet" => hero_circlet(ctx),
            "receiveCloak" => receive_cloak(ctx),
            "heroCertification" => Some(hero_certification(ctx).to_string()),
            // `heroList` → `ExHeroList` and `heroConfirm` → `claimHero` are
            // deferred (TODO(G25), see the module doc).
            _ => None,
        }
    }
}

fn first_talk_page(ctx: &QuestCtx) -> &'static str {
    if eligible(ctx) {
        "MonumentOfHeroes-noblesse.html"
    } else {
        "MonumentOfHeroes-noNoblesse.html"
    }
}

/// Java's `onFirstTalk` gate: 3rd/4th class group and level ≥ 55.
fn eligible(ctx: &QuestCtx) -> bool {
    (ctx.is_in_category("THIRD_CLASS_GROUP") || ctx.is_in_category("FOURTH_CLASS_GROUP"))
        && ctx.player_level() >= 55
}

fn is_hero(ctx: &QuestCtx) -> bool {
    ctx.world.olympiad.is_hero(ctx.player)
}

fn has_item(ctx: &QuestCtx, item_id: i32) -> bool {
    ctx.world
        .objects
        .get_component::<Inventory>(&ctx.player)
        .is_some_and(|inv| inv.count_of(item_id) > 0)
}

/// `Player.isInventoryUnder80(false)`: the non-quest slot count is within 80 %
/// of the inventory limit.
fn inventory_under_80(ctx: &QuestCtx) -> bool {
    let used = ctx
        .world
        .objects
        .get_component::<Inventory>(&ctx.player)
        .map_or(0, |inv| inv.non_quest_size(&ctx.world.data.item_data));
    let race = ctx
        .world
        .objects
        .get_component::<Player>(&ctx.player)
        .map_or(0, |p| p.race);
    let limit = ctx.world.cfg.character.inventory_limit(race);
    used as f64 <= limit as f64 * 0.8
}

fn send_inventory_full(ctx: &QuestCtx) {
    if let Some(cs) = ctx.world.clients.get(&ctx.client_id) {
        cs.send(crate::network::server_packets::system_message_with(
            sm_ids::UNABLE_TO_PROCESS_UNTIL_INVENTORY_UNDER_80_PERCENT,
            &[],
        ));
    }
}

/// `heroWeapon`: a hero opens the Infinity-weapon list (or the "already have
/// one" page), gated on the inventory-80 % check.
fn hero_weapon(ctx: &QuestCtx) -> Option<String> {
    if !is_hero(ctx) {
        return Some("MonumentOfHeroes-weaponNo.html".to_string());
    }
    if !inventory_under_80(ctx) {
        send_inventory_full(ctx);
        return None;
    }
    Some(
        if HERO_WEAPONS.iter().any(|&w| has_item(ctx, w)) {
            "MonumentOfHeroes-weaponHave.html"
        } else {
            "MonumentOfHeroes-weaponList.html"
        }
        .to_string(),
    )
}

/// `heroCirclet`: a hero receives the Wings of Destiny Circlet once.
fn hero_circlet(ctx: &mut QuestCtx) -> Option<String> {
    if !is_hero(ctx) {
        return Some("MonumentOfHeroes-circletNo.html".to_string());
    }
    if has_item(ctx, WINGS_OF_DESTINY_CIRCLET) {
        return Some("MonumentOfHeroes-circletHave.html".to_string());
    }
    if !inventory_under_80(ctx) {
        send_inventory_full(ctx);
        return None;
    }
    ctx.give_items(WINGS_OF_DESTINY_CIRCLET, 1);
    None
}

/// `receiveCloak`: the Olympiad's #1 (a hero, in this port) receives the Hero
/// Cloak once. Java gates on `getOlympiadRank == 1`; a crowned hero is exactly
/// the rank-1 noble of their class, so `is_hero` stands in for it.
fn receive_cloak(ctx: &mut QuestCtx) -> Option<String> {
    if !is_hero(ctx) {
        return Some("MonumentOfHeroes-cloakNo.html".to_string());
    }
    if has_item(ctx, HERO_CLOAK) {
        return Some("MonumentOfHeroes-cloakHave.html".to_string());
    }
    if !inventory_under_80(ctx) {
        send_inventory_full(ctx);
        return None;
    }
    ctx.give_items(HERO_CLOAK, 1);
    None
}

/// `heroCertification`: this port auto-crowns heroes at the Olympiad end, so a
/// hero always sees the "already certified" page and everyone else the "not a
/// hero" page (there is no unclaimed-hero certification step to run).
fn hero_certification(ctx: &QuestCtx) -> &'static str {
    if is_hero(ctx) {
        "MonumentOfHeroes-heroCertificationAlready.html"
    } else {
        "MonumentOfHeroes-heroCertificationNo.html"
    }
}
