//! Whisper of Dreams Part 2 (375) — `quests/Q00375_WhisperOfDreamsPart2`.
//! Vanutu (30938, level 60–74) — reached only by carrying the Mysterious Stone
//! from Part 1 ([`Q00374`](super::q00374_whisper_of_dreams_part1)), which is
//! consumed on the first talk. Collect 325 Karik Horns and 325 Limal Karinness
//! Bloods; each turn-in grants a chosen B-grade weapon scroll/enchant + 9000
//! adena. `addCondLevel(60, 74)`.
use crate::game_loop::quests::{QuestCtx, QuestScript};

const VANUTU: i32 = 30938;
const KARIK: i32 = 20629;
const LIMAL_KARINNESS: i32 = 20628;
const KARIK_HORN: i32 = 5888;
const LIMAL_KARINESS_BLOOD: i32 = 5889;
const MYSTERIOUS_STONE: i32 = 5887;
const REQUIRED: i64 = 325;
const MIN_LEVEL: i32 = 60;
const MAX_LEVEL: i32 = 74;
// Reward choices (`reward1`..`reward4`).
const SCROLL_PART_EW: i32 = 49474;
const REFINED_SCROLL_PART_EW: i32 = 49476;
const ENCHANT_WEAPON_B: i32 = 947;
const IMPROVED_ENCHANT_WEAPON_B: i32 = 33808;

pub struct Q00375WhisperOfDreamsPart2;

impl Q00375WhisperOfDreamsPart2 {
    /// Shared body of `reward1`..`reward4`: at cond 2 with both stacks full, hand
    /// them over for `reward` + 9000 adena.
    fn claim_reward(&self, ctx: &mut QuestCtx, reward: i32) -> Option<String> {
        if ctx.is_cond(2)
            && ctx.quest_items_count(KARIK_HORN) >= REQUIRED
            && ctx.quest_items_count(LIMAL_KARINESS_BLOOD) >= REQUIRED
        {
            ctx.give_items(reward, 1);
            ctx.take_items(KARIK_HORN, REQUIRED);
            ctx.take_items(LIMAL_KARINESS_BLOOD, REQUIRED);
            ctx.give_adena(9000, true);
            Some("30938-06.html".to_string())
        } else {
            None
        }
    }
}

impl QuestScript for Q00375WhisperOfDreamsPart2 {
    fn id(&self) -> i32 {
        375
    }
    fn name(&self) -> &'static str {
        "Q00375_WhisperOfDreamsPart2"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00375_WhisperOfDreamsPart2"
    }
    fn start_npcs(&self) -> &[i32] {
        &[VANUTU]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[VANUTU]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[LIMAL_KARINNESS, KARIK]
    }

    /// `addCondLevel(60, 74, "30938-02.html")` — a two-sided level gate.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.cond_level(MIN_LEVEL, MAX_LEVEL, "30938-02.html")
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            // The Mysterious Stone from Part 1 is the ticket in — consumed here.
            if ctx.quest_items_count(MYSTERIOUS_STONE) >= 1 {
                ctx.take_items(MYSTERIOUS_STONE, 1);
                return Some("30938-01.htm".to_string());
            }
            return Some("30938-05.html".to_string());
        }
        if ctx.is_started() {
            return Some(match ctx.cond() {
                1 => "30938-04.html".to_string(),
                2 => {
                    if ctx.quest_items_count(KARIK_HORN) >= REQUIRED
                        && ctx.quest_items_count(LIMAL_KARINESS_BLOOD) >= REQUIRED
                    {
                        "30938-05.html".to_string()
                    } else {
                        "30938-06.html".to_string()
                    }
                }
                _ => ctx.no_quest_html(),
            });
        }
        Some(ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30938-03.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30938-07.html" => {
                ctx.set_cond(1, false);
                Some(event.to_string())
            }
            "30938-08.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "reward1" => self.claim_reward(ctx, SCROLL_PART_EW),
            "reward2" => self.claim_reward(ctx, REFINED_SCROLL_PART_EW),
            "reward3" => self.claim_reward(ctx, ENCHANT_WEAPON_B),
            "reward4" => self.claim_reward(ctx, IMPROVED_ENCHANT_WEAPON_B),
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `getRandomPartyMemberState(killer, -1, 3, npc)`. Port is killer-only
        // (G11 party deviation).
        if !ctx.has_qs() {
            return;
        }
        if ctx.is_cond(1) {
            match ctx.npc_id {
                KARIK => {
                    ctx.give_item_randomly(KARIK_HORN, 1, REQUIRED, 0.95, true);
                }
                LIMAL_KARINNESS => {
                    ctx.give_item_randomly(LIMAL_KARINESS_BLOOD, 1, REQUIRED, 0.95, true);
                }
                _ => {}
            }
        }
        if ctx.is_cond(1)
            && ctx.quest_items_count(LIMAL_KARINESS_BLOOD) >= REQUIRED
            && ctx.quest_items_count(KARIK_HORN) >= REQUIRED
        {
            ctx.set_cond(2, true);
        }
    }
}
