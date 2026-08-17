//! Trial of the Seeker (213) — `quests/Q00213_TrialOfTheSeeker`. The scout
//! trial (Rogue / Elven Scout / Assassin, level 35+). Master Dufner sends the
//! seeker to Master Terry, who has them gather spirit ores from named beasts,
//! ferry letters between Viktor, Marina and Brunon to analyse Shilen's ore, and
//! finally hunt the four Abyss spirit ores for the Mark of the Seeker.
//!
//! Item-gated (no `memoState`). Two legs are order-independent set collections:
//! four class spirit ores (each from its own mob, all held → cond 5) and four
//! Abyss spirit ores (all held → cond 16), mirroring quest 224's bow materials.

use crate::game_loop::quests::{QuestCtx, QuestScript};

// NPCs
const MASTER_TERRY: i32 = 30064;
const MASTER_DUFNER: i32 = 30106;
const BLACKSMITH_BRUNON: i32 = 30526;
const TRADER_VIKTOR: i32 = 30684;
const MAGISTER_MARINA: i32 = 30715;
// Items
const DUFNERS_LETTER: i32 = 2647;
const TERRYS_1ST_ORDER: i32 = 2648;
const TERRYS_2ND_ORDER: i32 = 2649;
const TERRYS_LETTER: i32 = 2650;
const VIKTORS_LETTER: i32 = 2651;
const HAWKEYES_LETTER: i32 = 2652;
const MYSTERIOUS_SPIRIT_ORE: i32 = 2653;
const OL_MAHUM_SPIRIT_ORE: i32 = 2654;
const TUREK_SPIRIT_ORE: i32 = 2655;
const ANT_SPIRIT_ORE: i32 = 2656;
const TURAK_BUGBEAR_SPIRIT_ORE: i32 = 2657;
const TERRY_BOX: i32 = 2658;
const VIKTORS_REQUEST: i32 = 2659;
const MEDUSA_SCALES: i32 = 2660;
const SHILENS_SPIRIT_ORE: i32 = 2661;
const ANALYSIS_REQUEST: i32 = 2662;
const MARINAS_LETTER: i32 = 2663;
const EXPERIMENT_TOOLS: i32 = 2664;
const ANALYSIS_RESULT: i32 = 2665;
const TERRYS_3RD_ORDER: i32 = 2666;
const LIST_OF_HOST: i32 = 2667;
const ABYSS_SPIRIT_ORE1: i32 = 2668;
const ABYSS_SPIRIT_ORE2: i32 = 2669;
const ABYSS_SPIRIT_ORE3: i32 = 2670;
const ABYSS_SPIRIT_ORE4: i32 = 2671;
const TERRYS_REPORT: i32 = 2672;
/// Every quest item above, Dufner's Letter (2647) … Terry's Report (2672),
/// which the client assigns as one contiguous block.
const QUEST_ITEMS: [i32; (TERRYS_REPORT - DUFNERS_LETTER + 1) as usize] = {
    let mut ids = [0; (TERRYS_REPORT - DUFNERS_LETTER + 1) as usize];
    let mut i = 0;
    while i < ids.len() {
        ids[i] = DUFNERS_LETTER + i as i32;
        i += 1;
    }
    ids
};
// Reward
const MARK_OF_SEEKER: i32 = 2673;
// Monsters
const ANT_CAPTAIN: i32 = 20080;
const ANT_WARRIOR_CAPTAIN: i32 = 20088;
const MEDUSA: i32 = 20158;
const NEER_GHOUL_BERSERKER: i32 = 20198;
const OL_MAHUM_CAPTAIN: i32 = 20211;
const MARSH_STAKATO_DRONE: i32 = 20234;
const TURAK_BUGBEAR_WARRIOR: i32 = 20249;
const BREKA_ORC_OVERLORD: i32 = 20270;
const TUREK_ORC_WARLORD: i32 = 20495;
const LETO_LIZARDMAN_WARRIOR: i32 = 20580;
// Misc
const MIN_LEVEL: i32 = 35;
const LEVEL: i32 = 36;
const ROGUE: i32 = 7;
const ELVEN_SCOUT: i32 = 22;
const ASSASSIN: i32 = 35;

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

/// A set-collection kill: drop `own` once (gated on `gate`), advancing to
/// `cond` when the other three set members are already held.
fn ore_kill(ctx: &mut QuestCtx, gate: i32, own: i32, a: i32, b: i32, c: i32, cond: i32) {
    if has(ctx, gate) && ctx.award_once(own) && has(ctx, a) && has(ctx, b) && has(ctx, c) {
        ctx.set_cond(cond, false);
    }
}

pub struct Q00213TrialOfTheSeeker;

impl QuestScript for Q00213TrialOfTheSeeker {
    fn id(&self) -> i32 {
        213
    }
    fn name(&self) -> &'static str {
        "Q00213_TrialOfTheSeeker"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00213_TrialOfTheSeeker"
    }
    fn start_npcs(&self) -> &[i32] {
        &[MASTER_DUFNER]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            MASTER_DUFNER,
            MASTER_TERRY,
            BLACKSMITH_BRUNON,
            TRADER_VIKTOR,
            MAGISTER_MARINA,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            ANT_CAPTAIN,
            ANT_WARRIOR_CAPTAIN,
            MEDUSA,
            NEER_GHOUL_BERSERKER,
            OL_MAHUM_CAPTAIN,
            MARSH_STAKATO_DRONE,
            TURAK_BUGBEAR_WARRIOR,
            BREKA_ORC_OVERLORD,
            TUREK_ORC_WARLORD,
            LETO_LIZARDMAN_WARRIOR,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &QUEST_ITEMS
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "ACCEPT" => {
                ctx.accept_with_item(DUFNERS_LETTER);
                None
            }
            "30106-04.htm" | "30064-02.html" | "30064-07.html" | "30064-16.html"
            | "30064-17.html" | "30064-19.html" | "30684-02.html" | "30684-03.html"
            | "30684-04.html" | "30684-06.html" | "30684-07.html" | "30684-08.html"
            | "30684-09.html" | "30684-10.html" => Some(event.to_string()),
            "30064-03.html" => ctx
                .swap_quest_item(DUFNERS_LETTER, TERRYS_1ST_ORDER, 2)
                .then(|| event.to_string()),
            "30064-06.html" => {
                if has(ctx, TERRYS_1ST_ORDER) {
                    ctx.take_items(TERRYS_1ST_ORDER, 1);
                    ctx.give_items(TERRYS_2ND_ORDER, 1);
                    ctx.take_items(MYSTERIOUS_SPIRIT_ORE, 1);
                    ctx.set_cond(4, true);
                    return Some(event.to_string());
                }
                None
            }
            "30064-10.html" => {
                ctx.give_items(TERRYS_LETTER, 1);
                ctx.take_items(OL_MAHUM_SPIRIT_ORE, 1);
                ctx.take_items(TUREK_SPIRIT_ORE, 1);
                ctx.take_items(ANT_SPIRIT_ORE, 1);
                ctx.take_items(TURAK_BUGBEAR_SPIRIT_ORE, 1);
                ctx.take_items(TERRYS_2ND_ORDER, 1);
                ctx.give_items(TERRY_BOX, 1);
                ctx.set_cond(6, true);
                Some(event.to_string())
            }
            "30064-18.html" => ctx
                .swap_quest_item(ANALYSIS_RESULT, LIST_OF_HOST, 15)
                .then(|| event.to_string()),
            "30684-05.html" => ctx
                .swap_quest_item(TERRYS_LETTER, VIKTORS_LETTER, 7)
                .then(|| event.to_string()),
            "30684-11.html" => {
                ctx.take_items(TERRYS_LETTER, 1);
                ctx.take_items(TERRY_BOX, 1);
                ctx.take_items(HAWKEYES_LETTER, 1);
                ctx.take_items(VIKTORS_LETTER, 1);
                ctx.give_items(VIKTORS_REQUEST, 1);
                ctx.set_cond(9, true);
                Some(event.to_string())
            }
            "30684-15.html" => {
                ctx.take_items(VIKTORS_REQUEST, 1);
                ctx.take_items(MEDUSA_SCALES, -1);
                ctx.give_items(SHILENS_SPIRIT_ORE, 1);
                ctx.give_items(ANALYSIS_REQUEST, 1);
                ctx.set_cond(11, true);
                Some(event.to_string())
            }
            "30715-02.html" => {
                ctx.take_items(SHILENS_SPIRIT_ORE, 1);
                ctx.take_items(ANALYSIS_REQUEST, 1);
                ctx.give_items(MARINAS_LETTER, 1);
                ctx.set_cond(12, true);
                Some(event.to_string())
            }
            "30715-05.html" => {
                ctx.take_items(EXPERIMENT_TOOLS, 1);
                ctx.give_items(ANALYSIS_RESULT, 1);
                ctx.set_cond(14, true);
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        match ctx.npc_id {
            ANT_CAPTAIN => ore_kill(
                ctx,
                TERRYS_2ND_ORDER,
                ANT_SPIRIT_ORE,
                OL_MAHUM_SPIRIT_ORE,
                TUREK_SPIRIT_ORE,
                TURAK_BUGBEAR_SPIRIT_ORE,
                5,
            ),
            OL_MAHUM_CAPTAIN => ore_kill(
                ctx,
                TERRYS_2ND_ORDER,
                OL_MAHUM_SPIRIT_ORE,
                TUREK_SPIRIT_ORE,
                ANT_SPIRIT_ORE,
                TURAK_BUGBEAR_SPIRIT_ORE,
                5,
            ),
            TUREK_ORC_WARLORD => ore_kill(
                ctx,
                TERRYS_2ND_ORDER,
                TUREK_SPIRIT_ORE,
                OL_MAHUM_SPIRIT_ORE,
                ANT_SPIRIT_ORE,
                TURAK_BUGBEAR_SPIRIT_ORE,
                5,
            ),
            TURAK_BUGBEAR_WARRIOR => ore_kill(
                ctx,
                TERRYS_2ND_ORDER,
                TURAK_BUGBEAR_SPIRIT_ORE,
                OL_MAHUM_SPIRIT_ORE,
                TUREK_SPIRIT_ORE,
                ANT_SPIRIT_ORE,
                5,
            ),
            MARSH_STAKATO_DRONE => ore_kill(
                ctx,
                LIST_OF_HOST,
                ABYSS_SPIRIT_ORE1,
                ABYSS_SPIRIT_ORE2,
                ABYSS_SPIRIT_ORE3,
                ABYSS_SPIRIT_ORE4,
                16,
            ),
            BREKA_ORC_OVERLORD => ore_kill(
                ctx,
                LIST_OF_HOST,
                ABYSS_SPIRIT_ORE2,
                ABYSS_SPIRIT_ORE1,
                ABYSS_SPIRIT_ORE3,
                ABYSS_SPIRIT_ORE4,
                16,
            ),
            ANT_WARRIOR_CAPTAIN => ore_kill(
                ctx,
                LIST_OF_HOST,
                ABYSS_SPIRIT_ORE3,
                ABYSS_SPIRIT_ORE1,
                ABYSS_SPIRIT_ORE2,
                ABYSS_SPIRIT_ORE4,
                16,
            ),
            LETO_LIZARDMAN_WARRIOR => ore_kill(
                ctx,
                LIST_OF_HOST,
                ABYSS_SPIRIT_ORE4,
                ABYSS_SPIRIT_ORE1,
                ABYSS_SPIRIT_ORE2,
                ABYSS_SPIRIT_ORE3,
                16,
            ),
            MEDUSA => {
                if has(ctx, VIKTORS_REQUEST) && ctx.quest_items_count(MEDUSA_SCALES) < 10 {
                    ctx.collect_toward(MEDUSA_SCALES, 10, 10);
                }
            }
            NEER_GHOUL_BERSERKER
                if has(ctx, TERRYS_1ST_ORDER) && !has(ctx, MYSTERIOUS_SPIRIT_ORE) =>
            {
                if ctx.roll(2) == 0 {
                    ctx.give_items(MYSTERIOUS_SPIRIT_ORE, 1);
                    ctx.set_cond(3, true);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            if ctx.npc_id == MASTER_DUFNER {
                let class = ctx.player_class_id();
                if class == ROGUE || class == ELVEN_SCOUT || class == ASSASSIN {
                    return Some(if ctx.player_level() < MIN_LEVEL {
                        "30106-02.html".to_string()
                    } else {
                        "30106-03.htm".to_string()
                    });
                }
                return Some("30106-01.html".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            if ctx.npc_id == MASTER_DUFNER {
                return Some(ctx.already_completed_html());
            }
            return Some(ctx.no_quest_html());
        }
        match ctx.npc_id {
            MASTER_DUFNER => Some(dufner_talk(ctx)),
            MASTER_TERRY => Some(terry_talk(ctx)),
            BLACKSMITH_BRUNON => Some(brunon_talk(ctx)),
            TRADER_VIKTOR => Some(viktor_talk(ctx)),
            MAGISTER_MARINA => Some(marina_talk(ctx)),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

fn dufner_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, DUFNERS_LETTER) && !has(ctx, TERRYS_REPORT) {
        "30106-06.html".to_string()
    } else if !has(ctx, DUFNERS_LETTER) && !has(ctx, TERRYS_REPORT) {
        "30106-07.html".to_string()
    } else if has(ctx, TERRYS_REPORT) && !has(ctx, DUFNERS_LETTER) {
        ctx.give_adena(187606, true);
        ctx.give_items(MARK_OF_SEEKER, 1);
        ctx.add_exp_and_sp(1029478, 66768);
        ctx.exit_quest(false, true);
        ctx.social_action(3);
        "30106-08.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn terry_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, DUFNERS_LETTER) {
        "30064-01.html".to_string()
    } else if has(ctx, TERRYS_1ST_ORDER) {
        if !has(ctx, MYSTERIOUS_SPIRIT_ORE) {
            "30064-04.html".to_string()
        } else {
            "30064-05.html".to_string()
        }
    } else if has(ctx, TERRYS_2ND_ORDER) {
        let ores = ctx.quest_items_count(OL_MAHUM_SPIRIT_ORE)
            + ctx.quest_items_count(TUREK_SPIRIT_ORE)
            + ctx.quest_items_count(ANT_SPIRIT_ORE)
            + ctx.quest_items_count(TURAK_BUGBEAR_SPIRIT_ORE);
        if ores < 4 {
            "30064-08.html".to_string()
        } else {
            "30064-09.html".to_string()
        }
    } else if has(ctx, TERRYS_LETTER) {
        "30064-11.html".to_string()
    } else if has(ctx, VIKTORS_LETTER) {
        ctx.take_items(VIKTORS_LETTER, 1);
        ctx.give_items(HAWKEYES_LETTER, 1);
        ctx.set_cond(8, true);
        "30064-12.html".to_string()
    } else if has(ctx, HAWKEYES_LETTER) {
        "30064-13.html".to_string()
    } else if has(ctx, VIKTORS_REQUEST)
        || has(ctx, ANALYSIS_REQUEST)
        || has(ctx, MARINAS_LETTER)
        || has(ctx, EXPERIMENT_TOOLS)
    {
        "30064-14.html".to_string()
    } else if has(ctx, ANALYSIS_RESULT) {
        "30064-15.html".to_string()
    } else if has(ctx, TERRYS_3RD_ORDER) {
        if ctx.player_level() < LEVEL {
            "30064-20.html".to_string()
        } else {
            ctx.take_items(TERRYS_3RD_ORDER, 1);
            ctx.give_items(LIST_OF_HOST, 1);
            ctx.set_cond(15, true);
            "30064-21.html".to_string()
        }
    } else if has(ctx, LIST_OF_HOST) {
        let ores = ctx.quest_items_count(ABYSS_SPIRIT_ORE1)
            + ctx.quest_items_count(ABYSS_SPIRIT_ORE2)
            + ctx.quest_items_count(ABYSS_SPIRIT_ORE3)
            + ctx.quest_items_count(ABYSS_SPIRIT_ORE4);
        if ores < 4 {
            "30064-22.html".to_string()
        } else {
            ctx.take_items(LIST_OF_HOST, 1);
            ctx.take_items(ABYSS_SPIRIT_ORE1, 1);
            ctx.take_items(ABYSS_SPIRIT_ORE2, 1);
            ctx.take_items(ABYSS_SPIRIT_ORE3, 1);
            ctx.take_items(ABYSS_SPIRIT_ORE4, 1);
            ctx.give_items(TERRYS_REPORT, 1);
            ctx.set_cond(17, true);
            "30064-23.html".to_string()
        }
    } else if has(ctx, TERRYS_REPORT) {
        "30064-24.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn brunon_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, MARINAS_LETTER) {
        ctx.take_items(MARINAS_LETTER, 1);
        ctx.give_items(EXPERIMENT_TOOLS, 1);
        ctx.set_cond(13, true);
        "30526-01.html".to_string()
    } else if has(ctx, EXPERIMENT_TOOLS) {
        "30526-02.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn viktor_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, TERRYS_LETTER) {
        "30684-01.html".to_string()
    } else if has(ctx, HAWKEYES_LETTER) {
        "30684-12.html".to_string()
    } else if has(ctx, VIKTORS_REQUEST) {
        if ctx.quest_items_count(MEDUSA_SCALES) < 10 {
            "30684-13.html".to_string()
        } else {
            "30684-14.html".to_string()
        }
    } else if has(ctx, SHILENS_SPIRIT_ORE) && has(ctx, ANALYSIS_REQUEST) {
        "30684-16.html".to_string()
    } else if has(ctx, MARINAS_LETTER)
        && has(ctx, EXPERIMENT_TOOLS)
        && has(ctx, ANALYSIS_REQUEST)
        && has(ctx, TERRYS_REPORT)
    {
        "30684-17.html".to_string()
    } else if has(ctx, VIKTORS_LETTER) {
        "30684-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn marina_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, SHILENS_SPIRIT_ORE) && has(ctx, ANALYSIS_REQUEST) {
        "30715-01.html".to_string()
    } else if has(ctx, MARINAS_LETTER) {
        "30715-03.html".to_string()
    } else if has(ctx, EXPERIMENT_TOOLS) {
        "30715-04.html".to_string()
    } else if has(ctx, ANALYSIS_RESULT) {
        "30715-06.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}
