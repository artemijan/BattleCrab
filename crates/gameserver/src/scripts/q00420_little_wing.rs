//! Little Wing (420) — `quests/Q00420_LittleWing`. The level-35 hatchling-pet
//! quest and one of Interlude's most branch-heavy scripts. Cooper points the
//! player at Cronos, who offers a plain **Fairy Stone** or a **Deluxe Fairy
//! Stone** (harder — it can shatter when the bearer strikes certain fae). Maria
//! forges the stone from gathered materials; Byron and Mimyu turn it into
//! Monkshood Juice; one of five drakes (Exarion/Zwov/Kalibran/Suzet/Shamhai)
//! trades a Scale for the juice and sends the player to farm 20 of its eggs;
//! the hatched egg is redeemed with Mimyu for a random **Dragonflute** (Wind /
//! Star / Twilight — the deluxe path rolls twice, plus a rare Hatchling Armor).

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const MARIA: i32 = 30608;
const CRONOS: i32 = 30610;
const BYRON: i32 = 30711;
const MIMYU: i32 = 30747;
const EXARION: i32 = 30748;
const ZWOV: i32 = 30749;
const KALIBRAN: i32 = 30750;
const SUZET: i32 = 30751;
const SHAMHAI: i32 = 30752;
const COOPER: i32 = 30829;
// Gathered materials
const COAL: i32 = 1870;
const CHARCOAL: i32 = 1871;
const SILVER_NUGGET: i32 = 1873;
const STONE_OF_PURITY: i32 = 1875;
const GEMSTONE_D: i32 = 2130;
const GEMSTONE_C: i32 = 2131;
// Quest items
const FAIRY_DUST: i32 = 3499;
const FAIRY_STONE: i32 = 3816;
const DELUXE_FAIRY_STONE: i32 = 3817;
const FAIRY_STONE_LIST: i32 = 3818;
const DELUXE_STONE_LIST: i32 = 3819;
const TOAD_SKIN: i32 = 3820;
const MONKSHOOD_JUICE: i32 = 3821;
const EXARION_SCALE: i32 = 3822;
const EXARION_EGG: i32 = 3823;
const ZWOV_SCALE: i32 = 3824;
const ZWOV_EGG: i32 = 3825;
const KALIBRAN_SCALE: i32 = 3826;
const KALIBRAN_EGG: i32 = 3827;
const SUZET_SCALE: i32 = 3828;
const SUZET_EGG: i32 = 3829;
const SHAMHAI_SCALE: i32 = 3830;
const SHAMHAI_EGG: i32 = 3831;
// Monsters
const LESSER_BASILISK: i32 = 20070;
const TOAD_LORD: i32 = 20231;
const MARSH_SPIDER: i32 = 20233;
const BREKA_PREFECT: i32 = 20270;
const ROAD_SCAVENGER: i32 = 20551;
const LETO_WARRIOR: i32 = 20580;
// Rewards
const DRAGONFLUTE_OF_WIND: i32 = 3500;
const DRAGONFLUTE_OF_STAR: i32 = 3501;
const DRAGONFLUTE_OF_TWILIGHT: i32 = 3502;
const HATCHLING_ARMOR: i32 = 3912;
const HATCHLING_FOOD: i32 = 4038;
// Misc
const MIN_LEVEL: i32 = 35;
/// The order of `EGGS` in Java (`indexOf` scales the reward roll by ×5).
const EGGS: [i32; 5] = [EXARION_EGG, SUZET_EGG, KALIBRAN_EGG, SHAMHAI_EGG, ZWOV_EGG];
/// Fae whose destruction can shatter a Deluxe Fairy Stone (`onAttack`).
const DELUXE_STONE_BREAKERS: [i32; 16] = [
    20589, 20590, 20591, 20592, 20593, 20594, 20595, 20596, 20597, 20598, 20599, 27185, 27186,
    27187, 27188, 27189,
];
/// `NpcStringId.THE_STONE_THE_ELVEN_STONE_BROKE`.
const NS_STONE_BROKE: i32 = 1000308;

fn count(ctx: &QuestCtx, item: i32) -> i64 {
    ctx.quest_items_count(item)
}

fn has(ctx: &QuestCtx, item: i32) -> bool {
    count(ctx, item) > 0
}

/// Maria can forge the plain Fairy Stone: the player took Cronos' plain stone
/// list (`fairy_stone` 1) and gathered every material on it. Asked twice — once
/// to pick her greeting html, once to actually consume the materials — so the
/// two can never drift apart.
fn can_forge_fairy(ctx: &QuestCtx) -> bool {
    ctx.get_int("fairy_stone") == 1
        && count(ctx, COAL) >= 10
        && count(ctx, CHARCOAL) >= 10
        && count(ctx, GEMSTONE_D) >= 1
        && count(ctx, SILVER_NUGGET) >= 3
        && count(ctx, TOAD_SKIN) >= 10
}

/// The Deluxe counterpart of [`can_forge_fairy`]: bigger amounts, a grade-C
/// gemstone instead of grade-D, and a Stone of Purity the plain path never
/// wants.
fn can_forge_deluxe(ctx: &QuestCtx) -> bool {
    ctx.get_int("fairy_stone") == 2
        && count(ctx, COAL) >= 10
        && count(ctx, CHARCOAL) >= 10
        && count(ctx, GEMSTONE_C) >= 1
        && count(ctx, STONE_OF_PURITY) >= 1
        && count(ctx, SILVER_NUGGET) >= 5
        && count(ctx, TOAD_SKIN) >= 20
}

pub struct Q00420LittleWing;

impl Q00420LittleWing {
    /// Take the drake's scale + eggs and hand back a single hatched egg (cond
    /// 6 → 7). Shared by the four drakes handled in `on_talk` (Kalibran does it
    /// via an event).
    fn hatch(ctx: &mut QuestCtx, scale: i32, egg: i32, ok_html: &str, wait_html: &str) -> String {
        if count(ctx, egg) >= 20 {
            ctx.take_items(scale, -1);
            ctx.take_items(egg, -1);
            ctx.give_items(egg, 1);
            ctx.set_cond(7, true);
            ok_html.to_string()
        } else {
            wait_html.to_string()
        }
    }

    /// `giveReward`: a random Dragonflute keyed to which drake's egg is held.
    /// The Deluxe (Fairy Dust) path rolls an extra flute.
    fn give_reward(ctx: &mut QuestCtx) {
        let random = ctx.roll(100);
        for (idx, &egg) in EGGS.iter().enumerate() {
            if !has(ctx, egg) {
                continue;
            }
            let mul = idx as i32 * 5;
            if has(ctx, FAIRY_DUST) {
                let flute = if random < 45 + mul {
                    DRAGONFLUTE_OF_WIND
                } else if random < 75 + mul {
                    DRAGONFLUTE_OF_STAR
                } else {
                    DRAGONFLUTE_OF_TWILIGHT
                };
                ctx.give_items(flute, 1);
            }
            let flute = if random < 50 + mul {
                DRAGONFLUTE_OF_WIND
            } else if random < 85 + mul {
                DRAGONFLUTE_OF_STAR
            } else {
                DRAGONFLUTE_OF_TWILIGHT
            };
            ctx.give_items(flute, 1);
            ctx.take_items(egg, -1);
            break;
        }
    }
}

impl QuestScript for Q00420LittleWing {
    fn id(&self) -> i32 {
        420
    }
    fn name(&self) -> &'static str {
        "Q00420_LittleWing"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00420_LittleWing"
    }
    fn start_npcs(&self) -> &[i32] {
        &[COOPER]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            MARIA, CRONOS, BYRON, MIMYU, EXARION, ZWOV, KALIBRAN, SUZET, SHAMHAI, COOPER,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            TOAD_LORD,
            LESSER_BASILISK,
            MARSH_SPIDER,
            BREKA_PREFECT,
            ROAD_SCAVENGER,
            LETO_WARRIOR,
        ]
    }
    fn attack_npcs(&self) -> &[i32] {
        &DELUXE_STONE_BREAKERS
    }
    fn quest_items(&self) -> &[i32] {
        &[
            FAIRY_DUST,
            FAIRY_STONE,
            DELUXE_FAIRY_STONE,
            FAIRY_STONE_LIST,
            DELUXE_STONE_LIST,
            TOAD_SKIN,
            MONKSHOOD_JUICE,
            EXARION_SCALE,
            EXARION_EGG,
            ZWOV_SCALE,
            ZWOV_EGG,
            KALIBRAN_SCALE,
            KALIBRAN_EGG,
            SUZET_SCALE,
            SUZET_EGG,
            SHAMHAI_SCALE,
            SHAMHAI_EGG,
        ]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            if ctx.npc_id == COOPER {
                return Some(if ctx.player_level() >= MIN_LEVEL {
                    "30829-01.htm".to_string()
                } else {
                    "30829-03.html".to_string()
                });
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            return Some(ctx.already_completed_html());
        }
        let cond = ctx.cond();
        let html = match ctx.npc_id {
            COOPER => "30829-04.html".to_string(),
            CRONOS => cronos_talk(ctx, cond),
            MARIA => maria_talk(ctx, cond),
            BYRON => byron_talk(ctx, cond),
            MIMYU => mimyu_talk(ctx, cond),
            EXARION => drake_talk(ctx, cond, EXARION_EGG, "30748", true),
            ZWOV => drake_talk(ctx, cond, ZWOV_EGG, "30749", true),
            KALIBRAN => drake_talk(ctx, cond, KALIBRAN_EGG, "30750", false),
            SUZET => drake_talk(ctx, cond, SUZET_EGG, "30751", true),
            SHAMHAI => drake_talk(ctx, cond, SHAMHAI_EGG, "30752", true),
            _ => ctx.no_quest_html(),
        };
        Some(html)
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30610-02.html" | "30610-03.html" | "30610-04.html" | "30711-02.html"
            | "30747-05.html" | "30747-06.html" | "30751-02.html" => Some(event.to_string()),
            "30829-02.htm" => {
                if ctx.is_created() {
                    ctx.start_quest();
                    return Some(event.to_string());
                }
                None
            }
            // Cronos: pick a stone kind (fresh at cond 1, or re-pick at cond 5).
            "30610-05.html" => cronos_pick(ctx, 1, 1, FAIRY_STONE_LIST, event),
            "30610-06.html" => cronos_pick(ctx, 1, 2, DELUXE_STONE_LIST, event),
            "30610-12.html" => cronos_repick(ctx, 1, FAIRY_STONE_LIST, event),
            "30610-13.html" => cronos_repick(ctx, 2, DELUXE_STONE_LIST, event),
            // Maria: forge the (deluxe) fairy stone.
            "30608-03.html" => {
                if ctx.is_cond(2) {
                    if can_forge_fairy(ctx) {
                        ctx.take_items(FAIRY_STONE_LIST, -1);
                        ctx.take_items(COAL, 10);
                        ctx.take_items(CHARCOAL, 10);
                        ctx.take_items(GEMSTONE_D, 1);
                        ctx.take_items(SILVER_NUGGET, 3);
                        ctx.take_items(TOAD_SKIN, -1);
                        ctx.give_items(FAIRY_STONE, 1);
                    }
                    ctx.set_cond(3, true);
                    return Some(event.to_string());
                }
                None
            }
            "30608-05.html" => {
                if ctx.is_cond(2) {
                    if can_forge_deluxe(ctx) {
                        ctx.take_items(DELUXE_STONE_LIST, -1);
                        ctx.take_items(COAL, 10);
                        ctx.take_items(CHARCOAL, 10);
                        ctx.take_items(GEMSTONE_C, 1);
                        ctx.take_items(STONE_OF_PURITY, 1);
                        ctx.take_items(SILVER_NUGGET, 5);
                        ctx.take_items(TOAD_SKIN, -1);
                        ctx.give_items(DELUXE_FAIRY_STONE, 1);
                    }
                    ctx.set_cond(3, true);
                    return Some(event.to_string());
                }
                None
            }
            // Byron.
            "30711-03.html" => {
                if ctx.is_cond(3) {
                    ctx.set_cond(4, true);
                    return Some(if ctx.get_int("fairy_stone") == 2 {
                        "30711-04.html".to_string()
                    } else {
                        event.to_string()
                    });
                }
                None
            }
            // Mimyu: accept the stone → Monkshood Juice → final reward.
            "30747-02.html" | "30747-04.html" => {
                if ctx.is_cond(4) && (has(ctx, FAIRY_STONE) || has(ctx, DELUXE_FAIRY_STONE)) {
                    ctx.take_items(FAIRY_STONE, -1);
                    ctx.take_items(DELUXE_FAIRY_STONE, -1);
                    if ctx.get_int("fairy_stone") == 2 {
                        ctx.give_items(FAIRY_DUST, 1);
                    }
                    ctx.set_cond(5, true);
                    return Some(event.to_string());
                }
                None
            }
            "30747-07.html" | "30747-08.html" => {
                if ctx.is_cond(5) && count(ctx, MONKSHOOD_JUICE) == 0 {
                    ctx.give_items(MONKSHOOD_JUICE, 1);
                    return Some(event.to_string());
                }
                None
            }
            "30747-12.html" => {
                if ctx.is_cond(7) {
                    if ctx.get_int("fairy_stone") == 1 || count(ctx, FAIRY_DUST) == 0 {
                        Self::give_reward(ctx);
                        ctx.exit_quest(true, true);
                        return Some("30747-16.html".to_string());
                    }
                    ctx.set_cond(8, false);
                    return Some(event.to_string());
                }
                if ctx.is_cond(8) {
                    return Some(event.to_string());
                }
                None
            }
            "30747-13.html" => {
                if ctx.is_cond(8) {
                    Self::give_reward(ctx);
                    ctx.exit_quest(true, true);
                    return Some(event.to_string());
                }
                None
            }
            "30747-15.html" => {
                if ctx.is_cond(8) && count(ctx, FAIRY_DUST) > 1 {
                    let html = if ctx.roll(100) < 5 {
                        ctx.give_items(HATCHLING_ARMOR, 1);
                        "30747-14.html"
                    } else {
                        ctx.give_items(HATCHLING_FOOD, 20);
                        event
                    };
                    Self::give_reward(ctx);
                    ctx.take_items(FAIRY_DUST, -1);
                    ctx.exit_quest(true, true);
                    return Some(html.to_string());
                }
                None
            }
            // The five drakes: trade Monkshood Juice for a Scale and a hunt.
            "30748-02.html" => drake_accept(ctx, EXARION_SCALE, LETO_WARRIOR, event),
            "30749-02.html" => drake_accept(ctx, ZWOV_SCALE, MARSH_SPIDER, event),
            "30750-02.html" => drake_accept(ctx, KALIBRAN_SCALE, ROAD_SCAVENGER, event),
            "30751-03.html" => drake_accept(ctx, SUZET_SCALE, BREKA_PREFECT, event),
            "30752-02.html" => drake_accept(ctx, SHAMHAI_SCALE, LESSER_BASILISK, event),
            // Kalibran hatches via an event (the others hatch in on_talk).
            "30750-05.html" => {
                if ctx.is_cond(6) && count(ctx, KALIBRAN_EGG) >= 20 {
                    ctx.take_items(KALIBRAN_SCALE, -1);
                    ctx.take_items(KALIBRAN_EGG, -1);
                    ctx.give_items(KALIBRAN_EGG, 1);
                    ctx.set_cond(7, true);
                    return Some(event.to_string());
                }
                None
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() {
            return;
        }
        if ctx.is_cond(2) && ctx.npc_id == TOAD_LORD {
            let limit = if ctx.get_int("fairy_stone") == 1 {
                10
            } else {
                20
            };
            ctx.give_item_randomly(TOAD_SKIN, 1, limit, 0.3, true);
        } else if ctx.is_cond(6)
            && ctx.npc_id == ctx.get_int("drake_hunt")
            && let Some(egg) = egg_drop(ctx.npc_id)
        {
            ctx.give_item_randomly(egg, 1, 20, 0.5, true);
        }
    }

    fn on_attack(&self, ctx: &mut QuestCtx) {
        // A Deluxe Fairy Stone is fragile: striking one of the fae has a 30%
        // chance to shatter it.
        if ctx.has_qs() && has(ctx, DELUXE_FAIRY_STONE) && ctx.roll(100) < 30 {
            ctx.take_items(DELUXE_FAIRY_STONE, -1);
            ctx.play_sound(quest_sounds::MIDDLE);
            ctx.npc_say(NS_STONE_BROKE);
        }
    }
}

/// Monster → the egg it drops (Java's `EGG_DROPS`).
fn egg_drop(npc_id: i32) -> Option<i32> {
    Some(match npc_id {
        LESSER_BASILISK => SHAMHAI_EGG,
        MARSH_SPIDER => ZWOV_EGG,
        BREKA_PREFECT => SUZET_EGG,
        ROAD_SCAVENGER => KALIBRAN_EGG,
        LETO_WARRIOR => EXARION_EGG,
        _ => return None,
    })
}

/// Cronos' first stone choice (cond 1 → 2).
fn cronos_pick(ctx: &mut QuestCtx, from: i32, kind: i32, list: i32, event: &str) -> Option<String> {
    if ctx.is_cond(from) {
        ctx.set_cond(2, true);
        ctx.set_var("old_stone", "0");
        ctx.set_var("fairy_stone", kind.to_string());
        ctx.give_items(list, 1);
        return Some(event.to_string());
    }
    None
}

/// Cronos' re-pick after a failed run (cond 5 → 2), remembering the old kind.
fn cronos_repick(ctx: &mut QuestCtx, kind: i32, list: i32, event: &str) -> Option<String> {
    if ctx.is_cond(5) {
        ctx.set_cond(2, true);
        let old = ctx.get_int("fairy_stone");
        ctx.set_var("old_stone", old.to_string());
        ctx.set_var("fairy_stone", kind.to_string());
        ctx.give_items(list, 1);
        return Some(event.to_string());
    }
    None
}

/// A drake accepting the Monkshood Juice for its Scale (cond 5 → 6).
fn drake_accept(ctx: &mut QuestCtx, scale: i32, hunt: i32, event: &str) -> Option<String> {
    if ctx.is_cond(5) {
        ctx.take_items(MONKSHOOD_JUICE, -1);
        ctx.give_items(scale, 1);
        ctx.set_cond(6, true);
        ctx.set_var("drake_hunt", hunt.to_string());
        return Some(event.to_string());
    }
    None
}

fn cronos_talk(ctx: &QuestCtx, cond: i32) -> String {
    match cond {
        1 => "30610-01.html",
        2 => "30610-07.html",
        3 => {
            if ctx.get_int("old_stone") > 0 {
                "30610-14.html"
            } else {
                "30610-08.html"
            }
        }
        4 => "30610-09.html",
        5 => {
            if !has(ctx, FAIRY_STONE) && !has(ctx, DELUXE_FAIRY_STONE) {
                "30610-10.html"
            } else {
                "30610-11.html"
            }
        }
        _ => return ctx.no_quest_html(),
    }
    .to_string()
}

fn maria_talk(ctx: &QuestCtx, cond: i32) -> String {
    match cond {
        2 => {
            if can_forge_fairy(ctx) {
                "30608-02.html"
            } else if can_forge_deluxe(ctx) {
                "30608-04.html"
            } else {
                "30608-01.html"
            }
        }
        3 => "30608-06.html",
        _ => return ctx.no_quest_html(),
    }
    .to_string()
}

fn byron_talk(ctx: &mut QuestCtx, cond: i32) -> String {
    match cond {
        2 => "30711-10.html".to_string(),
        3 => match ctx.get_int("old_stone") {
            0 => "30711-01.html".to_string(),
            1 => {
                ctx.set_cond(5, true);
                "30711-05.html".to_string()
            }
            _ => {
                ctx.set_cond(4, true);
                "30711-06.html".to_string()
            }
        },
        4 => {
            if !has(ctx, FAIRY_STONE) && !has(ctx, DELUXE_FAIRY_STONE) {
                "30711-09.html".to_string()
            } else if !has(ctx, FAIRY_STONE) {
                "30711-08.html".to_string()
            } else {
                "30711-07.html".to_string()
            }
        }
        _ => ctx.no_quest_html(),
    }
}

fn mimyu_talk(ctx: &QuestCtx, cond: i32) -> String {
    match cond {
        4 => {
            if has(ctx, FAIRY_STONE) {
                "30747-01.html"
            } else if has(ctx, DELUXE_FAIRY_STONE) {
                "30747-03.html"
            } else {
                return ctx.no_quest_html();
            }
        }
        5 => {
            if has(ctx, MONKSHOOD_JUICE) {
                "30747-09.html"
            } else if ctx.get_int("fairy_stone") == 1 {
                "30747-05.html"
            } else {
                "30747-06.html"
            }
        }
        6 => {
            if EGGS.iter().any(|&e| count(ctx, e) >= 20) {
                "30747-10.html"
            } else {
                "30747-09.html"
            }
        }
        7 => "30747-11.html",
        8 => "30747-12.html",
        _ => return ctx.no_quest_html(),
    }
    .to_string()
}

/// The four drakes that hatch in `on_talk` (Kalibran, `has_hatch=false`, uses an
/// event). `egg` is the drake's egg id; `prefix` its html id.
fn drake_talk(ctx: &mut QuestCtx, cond: i32, egg: i32, prefix: &str, has_hatch: bool) -> String {
    let scale = egg - 1; // scale ids sit just below their egg (e.g. 3822/3823)
    match cond {
        5 => {
            if has(ctx, MONKSHOOD_JUICE) {
                format!("{prefix}-01.html")
            } else {
                ctx.no_quest_html()
            }
        }
        6 => {
            if has_hatch {
                Q00420LittleWing::hatch(
                    ctx,
                    scale,
                    egg,
                    &format!("{prefix}-04.html"),
                    &format!("{prefix}-03.html"),
                )
            } else if count(ctx, egg) >= 20 {
                format!("{prefix}-04.html")
            } else {
                format!("{prefix}-03.html")
            }
        }
        7 => {
            if has_hatch {
                format!("{prefix}-05.html")
            } else {
                format!("{prefix}-06.html")
            }
        }
        _ => ctx.no_quest_html(),
    }
}
