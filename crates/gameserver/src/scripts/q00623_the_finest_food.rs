//! The Finest Food (623) — `quests/Q00623_TheFinestFood`. Jeremy (31521, level
//! 71+) wants 100 each of Leaf of Flava / Buffalo Meat / Horn of Antelope from
//! the thermal beasts on the Plains of the Lizardmen. Turn-in (`31521-06`) rolls
//! a weighted 1000-slot reward table (accessories or adena + XP/SP) and exits;
//! the quest is repeatable.
use crate::game_loop::quests::{QuestCtx, QuestScript};

const JEREMY: i32 = 31521;
const THERMAL_BUFFALO: i32 = 21315;
const THERMAL_FLAVA: i32 = 21316;
const THERMAL_ANTELOPE: i32 = 21318;
const LEAF_OF_FLAVA: i32 = 7199;
const BUFFALO_MEAT: i32 = 7200;
const HORN_OF_ANTELOPE: i32 = 7201;
const REQUIRED_EACH: i64 = 100;
// Rewards.
const RING_OF_AURAKYRA: i32 = 6849;
const SEALED_SANDDRAGONS_EARING: i32 = 6847;
const DRAGON_NECKLACE: i32 = 6851;
const MIN_LEVEL: i32 = 71;

/// `MONSTER_DROPS`: which beast drops which ingredient.
fn drop_for(npc_id: i32) -> Option<i32> {
    match npc_id {
        THERMAL_BUFFALO => Some(BUFFALO_MEAT),
        THERMAL_FLAVA => Some(LEAF_OF_FLAVA),
        THERMAL_ANTELOPE => Some(HORN_OF_ANTELOPE),
        _ => None,
    }
}

/// `hasAllItems(player, true, LEAF_OF_FLAVA, BUFFALO_MEAT, HORN_OF_ANTELOPE)` —
/// each ingredient at its required count (100).
fn has_all(ctx: &QuestCtx) -> bool {
    ctx.quest_items_count(LEAF_OF_FLAVA) >= REQUIRED_EACH
        && ctx.quest_items_count(BUFFALO_MEAT) >= REQUIRED_EACH
        && ctx.quest_items_count(HORN_OF_ANTELOPE) >= REQUIRED_EACH
}

pub struct Q00623TheFinestFood;

impl QuestScript for Q00623TheFinestFood {
    fn id(&self) -> i32 {
        623
    }
    fn name(&self) -> &'static str {
        "Q00623_TheFinestFood"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00623_TheFinestFood"
    }
    fn start_npcs(&self) -> &[i32] {
        &[JEREMY]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[JEREMY]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[THERMAL_BUFFALO, THERMAL_FLAVA, THERMAL_ANTELOPE]
    }
    fn quest_items(&self) -> &[i32] {
        &[LEAF_OF_FLAVA, BUFFALO_MEAT, HORN_OF_ANTELOPE]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "31521-03.htm" => {
                if ctx.is_created() {
                    ctx.start_quest();
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "31521-06.html" => {
                if !ctx.is_cond(2) {
                    return None;
                }
                if has_all(ctx) {
                    let random = ctx.roll(1000);
                    if random < 120 {
                        ctx.give_adena(25000, true);
                        ctx.reward_items(RING_OF_AURAKYRA, 1);
                    } else if random < 240 {
                        ctx.give_adena(65000, true);
                        ctx.reward_items(SEALED_SANDDRAGONS_EARING, 1);
                    } else if random < 340 {
                        ctx.give_adena(25000, true);
                        ctx.reward_items(DRAGON_NECKLACE, 1);
                    } else if random < 940 {
                        ctx.give_adena(73000, true);
                        ctx.add_exp_and_sp(230000, 18200);
                    }
                    // The 940..1000 slice (6%) grants nothing but still exits.
                    ctx.exit_quest(true, true);
                    Some(event.to_string())
                } else {
                    Some("31521-07.html".to_string())
                }
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `getRandomPartyMemberState(killer, 1, 3, npc)`: a cond-1 started member.
        // Port is killer-only (G11 party deviation).
        if !ctx.has_qs() || !ctx.is_cond(1) {
            return;
        }
        let Some(item) = drop_for(ctx.npc_id) else {
            return;
        };
        if ctx.give_item_randomly(item, 1, REQUIRED_EACH, 1.0, true) && has_all(ctx) {
            ctx.set_cond(2, false);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL { "31521-01.htm" } else { "31521-02.htm" }
                    .to_string(),
            );
        }
        if ctx.is_started() {
            return Some(
                match ctx.cond() {
                    1 => "31521-04.html",
                    2 => "31521-05.html",
                    _ => return Some(ctx.no_quest_html()),
                }
                .to_string(),
            );
        }
        if ctx.is_completed() {
            return Some(ctx.already_completed_html());
        }
        Some(ctx.no_quest_html())
    }
}
