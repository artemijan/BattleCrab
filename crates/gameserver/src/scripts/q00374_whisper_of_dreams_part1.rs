//! Whisper of Dreams Part 1 (374) — `quests/Q00374_WhisperOfDreamsPart1`.
//! Vanutu (30938, level 56–66) sends the player to collect 360 Cave Beast Teeth
//! and 360 Death Wave Lights in the Cave of Trials; each turn-in (`cond` 2 or 4)
//! grants a chosen B-grade enchant/scroll + 9000 adena. Between them, `cond` 3
//! adds a 20% Sealed Mysterious Stone drop, which Galman (31044) exchanges for
//! the Mysterious Stone that opens Part 2. `addCondLevel(56, 66)`.
use crate::game_loop::quests::{QuestCtx, QuestScript};

const VANUTU: i32 = 30938;
const GALMAN: i32 = 31044;
const CAVE_BEAST: i32 = 20620;
const DEATH_WAVE: i32 = 20621;
const CAVE_BEAST_TOOTH: i32 = 5884;
const DEATH_WAVE_LIGHT: i32 = 5885;
const SEALED_MYSTERIOUS_STONE: i32 = 5886;
const MYSTERIOUS_STONE: i32 = 5887;
const REQUIRED: i64 = 360;
const MIN_LEVEL: i32 = 56;
const MAX_LEVEL: i32 = 66;
// Reward choices (`reward1`..`reward4`).
const SCROLL_PART_EA: i32 = 49475;
const REFINED_SCROLL_PART_EA: i32 = 49478;
const ENCHANT_ARMOR_B: i32 = 948;
const IMPROVED_ENCHANT_ARMOR_B: i32 = 29743;

/// `hasAllItems(player, true, DEATH_WAVE_LIGHT, CAVE_BEAST_TOOTH)` — 360 of each.
fn has_both(ctx: &QuestCtx) -> bool {
    ctx.quest_items_count(DEATH_WAVE_LIGHT) >= REQUIRED
        && ctx.quest_items_count(CAVE_BEAST_TOOTH) >= REQUIRED
}

pub struct Q00374WhisperOfDreamsPart1;

impl Q00374WhisperOfDreamsPart1 {
    /// Shared body of `reward1`..`reward4`: hand over both stacks for `reward` +
    /// 9000 adena, choosing the page by cond (2 = keep going, 4 = done).
    fn claim_reward(&self, ctx: &mut QuestCtx, reward: i32) -> Option<String> {
        if !has_both(ctx) {
            return None;
        }
        let html = if ctx.is_cond(2) {
            "30938-05.html"
        } else if ctx.is_cond(4) {
            "30938-08.html"
        } else {
            return None;
        };
        ctx.give_items(reward, 1);
        ctx.take_items(DEATH_WAVE_LIGHT, -1);
        ctx.take_items(CAVE_BEAST_TOOTH, -1);
        ctx.give_adena(9000, true);
        Some(html.to_string())
    }
}

impl QuestScript for Q00374WhisperOfDreamsPart1 {
    fn id(&self) -> i32 {
        374
    }
    fn name(&self) -> &'static str {
        "Q00374_WhisperOfDreamsPart1"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00374_WhisperOfDreamsPart1"
    }
    fn start_npcs(&self) -> &[i32] {
        &[VANUTU]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[VANUTU, GALMAN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[CAVE_BEAST, DEATH_WAVE]
    }
    fn quest_items(&self) -> &[i32] {
        &[SEALED_MYSTERIOUS_STONE]
    }

    /// `addCondLevel(56, 66, "30938-02.html")` — a two-sided level gate.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.cond_level(MIN_LEVEL, MAX_LEVEL, "30938-02.html")
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        match ctx.npc_id {
            VANUTU => {
                if ctx.is_completed() {
                    return Some(ctx.already_completed_html());
                }
                if ctx.is_created() {
                    return Some("30938.htm".to_string());
                }
                if ctx.is_started() {
                    return Some(
                        match ctx.cond() {
                            1 => "30938-03.html",
                            2 => "30938-04.html",
                            3 => "30938-07.html",
                            4 => "30938-08.html",
                            _ => return Some(ctx.no_quest_html()),
                        }
                        .to_string(),
                    );
                }
                Some(ctx.no_quest_html())
            }
            GALMAN => {
                if ctx.is_cond(4) {
                    return Some("31044.html".to_string());
                }
                Some(ctx.no_quest_html())
            }
            _ => Some(ctx.no_quest_html()),
        }
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30938-01.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30938-06.html" => {
                if ctx.is_cond(2) {
                    ctx.set_cond(3, true);
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "reward1" => self.claim_reward(ctx, SCROLL_PART_EA),
            "reward2" => self.claim_reward(ctx, REFINED_SCROLL_PART_EA),
            "reward3" => self.claim_reward(ctx, ENCHANT_ARMOR_B),
            "reward4" => self.claim_reward(ctx, IMPROVED_ENCHANT_ARMOR_B),
            "31044-01.html" => {
                if ctx.is_cond(4) {
                    ctx.give_items(MYSTERIOUS_STONE, 1);
                    ctx.take_items(SEALED_MYSTERIOUS_STONE, -1);
                    ctx.exit_quest(true, true);
                    Some(event.to_string())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `getRandomPartyMemberState(killer, -1, 3, npc)`. Port is killer-only
        // (G11 party deviation).
        if !ctx.has_qs() {
            return;
        }
        if ctx.cond() < 4 {
            let ingredient = if ctx.npc_id == CAVE_BEAST {
                CAVE_BEAST_TOOTH
            } else {
                DEATH_WAVE_LIGHT
            };
            ctx.give_item_randomly(ingredient, 1, REQUIRED, 0.9, true);
            if ctx.is_cond(3) {
                ctx.give_item_randomly(SEALED_MYSTERIOUS_STONE, 1, 1, 0.2, true);
            }
        }
        if ctx.is_cond(1) && has_both(ctx) {
            ctx.set_cond(2, true);
        }
        if ctx.is_cond(3) && ctx.quest_items_count(SEALED_MYSTERIOUS_STONE) >= 1 {
            ctx.set_cond(4, true);
        }
    }
}
