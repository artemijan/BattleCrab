//! Moon Knight (11000) — `quests/Q11000_MoonKnight`.
//!
//! The newbie chain's only race-free entry (levels 25–40): a ten-cond errand
//! around Gludio for a suit of Moon armour, in one of three weights.
//!
//! **This quest cannot be finished, in Java or here.** At cond 7→8 Rolento
//! hands over `ROLENTO_BAG` (49559) and `IRON_SCALE_GUILD_CERTIFICATE`
//! (49560); **neither item exists in this datapack**. Gudz then answers at
//! cond 8 only for a player holding both, so the check can never pass and the
//! quest stalls there permanently. The port reproduces it rather than
//! inventing the items: everything up to cond 8 is real, playable content, and
//! a "fix" would be content this dist does not have.
//!
//! Two Java quirks are kept deliberately:
//!
//! - **Neti's third page is unreachable.** Java writes `qs.getState() > 7`
//!   where every sibling branch writes `getCond() > n` — and `getState()` is
//!   the 1/2/3 CREATED/STARTED/COMPLETED enum, never above 7. `30425-03.html`
//!   is dead in Java, so it is dead here.
//! - **Damion's `else` is a catch-all.** Any cond above 3 gets `30208-04.html`,
//!   including cond 1 handled above it — the ordering is what makes the early
//!   conds reachable at all.
//!
//! SKIP(census): Java also registers a global `ON_PLAYER_LEVEL_CHANGED`
//! listener that pops a tutorial question-mark at anyone who levels into
//! eligibility. The quest framework has no level-changed hook, and building
//! one global listener for a single UI nudge on an unfinishable quest is not
//! the trade to make; the quest is still startable by talking to Jones.

use crate::game_loop::quests::{QuestCtx, QuestScript};

// NPCs
const JONES: i32 = 30939;
const DAMION: i32 = 30208;
const AMORA: i32 = 30940;
const NETI: i32 = 30425;
const ROLENTO: i32 = 30437;
const GUDZ: i32 = 30941;

// Monsters
const OL_MAHUM_THIEF: i32 = 27201;
const TUREK_ORC_COMMANDER: i32 = 27202;
const TUREK_ORC_INVADER: i32 = 27203;

// Quest items
const MOLD: i32 = 49555;
const AMORA_RECEIPT: i32 = 49556;
const ARMOR_TRADE_CONTRACT: i32 = 49557;
const TUREK_ORC_ORDER: i32 = 49558;
const TUREK_ORC_INVADER_HEAD: i32 = 49561;
/// The two items the datapack does not declare — see the module note.
const ROLENTO_BAG: i32 = 49559;
const IRON_SCALE_GUILD_CERTIFICATE: i32 = 49560;

// Rewards
const MOON_HELMET: i32 = 7850;
const MOON_ARMOR: i32 = 7851;
const MOON_GAUNTLETS_HEAVY: i32 = 7852;
const MOON_BOOTS_HEAVY: i32 = 7853;
const MOON_SHELL: i32 = 7854;
const MOON_LEATHER_GLOVES: i32 = 7855;
const MOON_SHOES: i32 = 7856;
const MOON_CAPE: i32 = 7857;
const MOON_SILK_GLOVES: i32 = 7858;
const MOON_SANDALS: i32 = 7859;

const MIN_LEVEL: i32 = 25;
const MAX_LEVEL: i32 = 40;

pub struct Q11000MoonKnight;

impl QuestScript for Q11000MoonKnight {
    fn id(&self) -> i32 {
        11000
    }
    fn name(&self) -> &'static str {
        "Q11000_MoonKnight"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q11000_MoonKnight"
    }
    fn start_npcs(&self) -> &[i32] {
        &[JONES]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[JONES, DAMION, AMORA, NETI, ROLENTO, GUDZ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[OL_MAHUM_THIEF, TUREK_ORC_COMMANDER, TUREK_ORC_INVADER]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            MOLD,
            AMORA_RECEIPT,
            ARMOR_TRADE_CONTRACT,
            TUREK_ORC_ORDER,
            TUREK_ORC_INVADER_HEAD,
        ]
    }

    /// `addCondLevel(25, 40, "no_level.html")` — note the underscore; this
    /// quest's page is not the chain's `no-level.html`. No race condition:
    /// Moon Knight is open to everyone.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (!(MIN_LEVEL..=MAX_LEVEL).contains(&ctx.player_level()))
            .then(|| "no_level.html".to_string())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            // Pages the html buttons only navigate to.
            "30208-01.html" | "30208-02.html" | "30437-02.html" | "30941-02.html"
            | "30941-03.html" => Some(event.to_string()),
            "30939-02.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30939-06.html" => step(ctx, 4, 5, event),
            "30939-09.html" => step(ctx, 5, 6, event),
            "30425-01.html" => step(ctx, 6, 7, event),
            "30437-03.html" => {
                if !ctx.is_cond(7) {
                    return None;
                }
                ctx.set_cond(8, true);
                ctx.take_items(ARMOR_TRADE_CONTRACT, 1);
                // Both give nothing — the items are absent from the datapack,
                // which is what strands the quest at cond 8.
                ctx.give_items(ROLENTO_BAG, 1);
                ctx.give_items(IRON_SCALE_GUILD_CERTIFICATE, 1);
                Some(event.to_string())
            }
            "30941-04.html" => {
                if !ctx.is_cond(8) {
                    return None;
                }
                ctx.set_cond(9, true);
                ctx.take_items(TUREK_ORC_ORDER, 1);
                ctx.take_items(ROLENTO_BAG, 1);
                ctx.take_items(IRON_SCALE_GUILD_CERTIFICATE, 1);
                Some(event.to_string())
            }
            // The three armour weights. Java returns `null` from these, so the
            // client keeps whatever page it was showing.
            "reward1" => reward(
                ctx,
                &[MOON_HELMET, MOON_SHELL, MOON_LEATHER_GLOVES, MOON_SHOES],
            ),
            "reward2" => reward(
                ctx,
                &[
                    MOON_HELMET,
                    MOON_ARMOR,
                    MOON_GAUNTLETS_HEAVY,
                    MOON_BOOTS_HEAVY,
                ],
            ),
            "reward3" => reward(
                ctx,
                &[MOON_HELMET, MOON_CAPE, MOON_SILK_GLOVES, MOON_SANDALS],
            ),
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return (ctx.npc_id == JONES).then(|| "30939-01.htm".to_string());
        }
        if ctx.is_completed() {
            return Some(ctx.already_completed_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        let cond = ctx.cond();
        let html = match ctx.npc_id {
            JONES => match cond {
                1 => "30939-03.html",
                2 | 3 => "30939-04.html",
                4 => "30939-05.html",
                5 => {
                    if ctx.quest_items_count(TUREK_ORC_ORDER) > 0
                        && ctx.quest_items_count(ARMOR_TRADE_CONTRACT) > 0
                    {
                        "30939-08.html"
                    } else {
                        "30939-07.html"
                    }
                }
                6 => "30939-10.html",
                7 => "30939-11.html",
                8 => "30939-12.html",
                9 => "30939-13.html",
                10 => "30939-14.html",
                _ => return Some(ctx.no_quest_html()),
            },
            DAMION => {
                if cond == 1 {
                    ctx.set_cond(2, true);
                    "30208-01.html"
                } else if cond == 2 {
                    "30208-01.html"
                } else if cond == 3 {
                    if ctx.quest_items_count(AMORA_RECEIPT) > 0 {
                        ctx.take_items(AMORA_RECEIPT, 1);
                        ctx.set_cond(4, true);
                        "30208-03.html"
                    } else {
                        "30208-01.html"
                    }
                } else {
                    // Java's bare `else` — every later cond lands here.
                    "30208-04.html"
                }
            }
            AMORA => {
                if cond == 2 {
                    if ctx.quest_items_count(MOLD) < 10 {
                        "30940-01.html"
                    } else {
                        ctx.give_items(AMORA_RECEIPT, 1);
                        ctx.take_items(MOLD, 10);
                        ctx.set_cond(3, true);
                        "30940-02.html"
                    }
                } else if cond == 3 {
                    "30940-03.html"
                } else if cond > 3 {
                    "30940-04.html"
                } else {
                    return Some(ctx.no_quest_html());
                }
            }
            NETI => {
                if cond == 6
                    && ctx.quest_items_count(TUREK_ORC_ORDER) > 0
                    && ctx.quest_items_count(ARMOR_TRADE_CONTRACT) > 0
                {
                    ctx.set_cond(7, true);
                    "30425-01.html"
                } else if cond == 7 {
                    "30425-02.html"
                } else {
                    // Java's third arm is `qs.getState() > 7` — the state enum,
                    // not the cond, so `30425-03.html` is unreachable there and
                    // here. See the module note.
                    return Some(ctx.no_quest_html());
                }
            }
            ROLENTO => {
                if cond == 7
                    && ctx.quest_items_count(TUREK_ORC_ORDER) > 0
                    && ctx.quest_items_count(ARMOR_TRADE_CONTRACT) > 0
                {
                    "30437-01.html"
                } else if cond == 8 {
                    "30437-04.html"
                } else if cond > 8 {
                    "30437-05.html"
                } else {
                    return Some(ctx.no_quest_html());
                }
            }
            GUDZ => {
                if cond == 8 {
                    // Unreachable: two of the three items do not exist.
                    if ctx.quest_items_count(TUREK_ORC_ORDER) > 0
                        && ctx.quest_items_count(ROLENTO_BAG) > 0
                        && ctx.quest_items_count(IRON_SCALE_GUILD_CERTIFICATE) > 0
                    {
                        "30941-01.html"
                    } else {
                        return Some(ctx.no_quest_html());
                    }
                } else if cond == 9 {
                    if ctx.quest_items_count(TUREK_ORC_INVADER_HEAD) < 10 {
                        "30941-05.html"
                    } else {
                        ctx.take_items(TUREK_ORC_INVADER_HEAD, 10);
                        ctx.set_cond(10, true);
                        "30941-06.html"
                    }
                } else if cond == 10 {
                    "30941-07.html"
                } else {
                    return Some(ctx.no_quest_html());
                }
            }
            _ => return Some(ctx.no_quest_html()),
        };
        Some(html.to_string())
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // Java picks a random *party* member within 3 levels
        // (`getRandomPartyMemberState(killer, -1, 3, npc)`); the port credits
        // the killer, which is the same thing solo and the shape every other
        // ported quest uses.
        if !ctx.has_qs() {
            return;
        }
        match ctx.npc_id {
            OL_MAHUM_THIEF if ctx.is_cond(2) => {
                ctx.give_item_randomly(MOLD, 1, 10, 1.0, true);
            }
            TUREK_ORC_COMMANDER if ctx.is_cond(5) => {
                // Two independent quarter-chance rolls; a kill can yield both,
                // one or neither, and each caps at one.
                ctx.give_item_randomly(ARMOR_TRADE_CONTRACT, 1, 1, 0.25, true);
                ctx.give_item_randomly(TUREK_ORC_ORDER, 1, 1, 0.25, true);
            }
            TUREK_ORC_INVADER if ctx.is_cond(9) => {
                ctx.give_item_randomly(TUREK_ORC_INVADER_HEAD, 1, 10, 1.0, true);
            }
            _ => {}
        }
    }
}

/// `if (qs.isCond(from)) { qs.setCond(to, true); htmltext = event; }`.
fn step(ctx: &mut QuestCtx, from: i32, to: i32, event: &str) -> Option<String> {
    if !ctx.is_cond(from) {
        return None;
    }
    ctx.set_cond(to, true);
    Some(event.to_string())
}

/// One armour set. Java sets no `htmltext` in these branches, so the reply is
/// `None` and the client keeps the page it is on.
fn reward(ctx: &mut QuestCtx, items: &[i32]) -> Option<String> {
    if !ctx.is_cond(10) {
        return None;
    }
    for &item in items {
        ctx.give_items(item, 1);
    }
    ctx.exit_quest(false, true);
    None
}
