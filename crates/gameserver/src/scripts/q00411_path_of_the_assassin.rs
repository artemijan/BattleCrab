//! Path Of The Assassin (411) — port of
//! `dist/game/data/scripts/quests/Q00411_PathOfTheAssassin/`.
//!
//! Awards the **Iron Heart** (1252), the second of `DarkElfChange1`'s four
//! proofs.
//!
//! ## The whole quest is one token passed along a chain
//!
//! Java writes every talk branch as "hold *this* item and **none** of the
//! others" — `!hasAtLeastOneQuestItem(a, b, c, d, e) && hasQuestItems(f)` —
//! repeated seventeen times across three NPCs. That is verbose but it encodes
//! something simple: **exactly one token is ever in the bag**, because each
//! hand-over takes the old one before giving the new:
//!
//! ```text
//! Shilen's Call → Arkenia's Letter → Leikan's Note → (10 molars)
//!   → Shilen's Tears → Arkenia's Recommendation → Iron Heart
//! ```
//!
//! So the port asks *which* token is held ([`Token`]) and matches on it. That
//! is the same predicate Java writes, expressed once instead of seventeen
//! times, and the invariant it relies on is the quest's own design rather than
//! an assumption — every transition below is take-then-give.
//!
//! The molars are the exception: they coexist with Leikan's note, which is why
//! Leikan's branches test them separately.
//!
//! Neither drop rolls a chance — 10 molars is 10 kills, and Calpico always
//! yields the tears.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const TRISKEL: i32 = 30416;
const GUARD_LEIKAN: i32 = 30382;
const ARKENIA: i32 = 30419;

const SHILENS_CALL: i32 = 1245;
const ARKENIAS_LETTER: i32 = 1246;
const LEIKANS_NOTE: i32 = 1247;
const MOONSTONE_BEASTS_MOLAR: i32 = 1248;
const SHILENS_TEARS: i32 = 1250;
const ARKENIAS_RECOMMENDATION: i32 = 1251;
const IRON_HEART: i32 = 1252;

const MOONSTONE_BEAST: i32 = 20369;
/// A quest monster, not a normal spawn.
const CALPICO: i32 = 27036;

const DARK_FIGHTER: i32 = 31;
const ASSASSIN: i32 = 35;
const MIN_LEVEL: i32 = 19;
const MOLARS_NEEDED: i64 = 10;

/// The chain, in order. Exactly one of these is held at a time.
const TOKENS: [i32; 6] = [
    SHILENS_CALL,
    ARKENIAS_LETTER,
    LEIKANS_NOTE,
    SHILENS_TEARS,
    ARKENIAS_RECOMMENDATION,
    IRON_HEART,
];

const QUEST_ITEMS: [i32; 6] = [
    SHILENS_CALL,
    ARKENIAS_LETTER,
    LEIKANS_NOTE,
    MOONSTONE_BEASTS_MOLAR,
    SHILENS_TEARS,
    ARKENIAS_RECOMMENDATION,
];

pub struct Q00411PathOfTheAssassin;

impl Q00411PathOfTheAssassin {
    fn has(&self, ctx: &QuestCtx, item: i32) -> bool {
        ctx.quest_items_count(item) > 0
    }

    /// The single chain token currently held, or `None` between hand-overs.
    /// This is Java's `!hasAtLeastOneQuestItem(others) && hasQuestItems(x)`,
    /// asked once.
    fn token(&self, ctx: &QuestCtx) -> Option<i32> {
        let held: Vec<i32> = TOKENS
            .iter()
            .copied()
            .filter(|id| ctx.quest_items_count(*id) > 0)
            .collect();
        match held.as_slice() {
            [one] => Some(*one),
            _ => None,
        }
    }
}

impl QuestScript for Q00411PathOfTheAssassin {
    fn id(&self) -> i32 {
        411
    }
    fn name(&self) -> &'static str {
        "Q00411_PathOfTheAssassin"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00411_PathOfTheAssassin"
    }
    fn start_npcs(&self) -> &[i32] {
        &[TRISKEL]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[TRISKEL, GUARD_LEIKAN, ARKENIA]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[MOONSTONE_BEAST, CALPICO]
    }
    fn quest_items(&self) -> &[i32] {
        &QUEST_ITEMS
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "ACCEPT" => Some(match ctx.player_class_id() {
                DARK_FIGHTER if ctx.player_level() < MIN_LEVEL => "30416-03.htm".to_string(),
                DARK_FIGHTER if self.has(ctx, IRON_HEART) => "30416-04.htm".to_string(),
                DARK_FIGHTER => {
                    ctx.start_quest();
                    ctx.give_items(SHILENS_CALL, 1);
                    "30416-05.htm".to_string()
                }
                ASSASSIN => "30416-02a.htm".to_string(),
                _ => "30416-02.htm".to_string(),
            }),
            "30382-02.html" | "30382-04.html" => Some(event.to_string()),
            "30419-02.html" | "30419-03.html" | "30419-04.html" | "30419-06.html" => {
                Some(event.to_string())
            }
            // Arkenia: the call becomes her letter.
            "30419-05.html" => {
                if self.has(ctx, SHILENS_CALL) {
                    ctx.take_items(SHILENS_CALL, 1);
                    ctx.give_items(ARKENIAS_LETTER, 1);
                    ctx.set_cond(2, true);
                    return Some(event.to_string());
                }
                None
            }
            // Leikan: her letter becomes his note.
            "30382-03.html" => {
                if self.has(ctx, ARKENIAS_LETTER) {
                    ctx.take_items(ARKENIAS_LETTER, 1);
                    ctx.give_items(LEIKANS_NOTE, 1);
                    ctx.set_cond(3, true);
                    return Some(event.to_string());
                }
                None
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        match ctx.npc_id {
            MOONSTONE_BEAST => {
                if !self.has(ctx, LEIKANS_NOTE)
                    || ctx.quest_items_count(MOONSTONE_BEASTS_MOLAR) >= MOLARS_NEEDED
                {
                    return;
                }
                ctx.collect_toward(MOONSTONE_BEASTS_MOLAR, MOLARS_NEEDED, 4);
            }
            // Note: no note/token gate here at all — Java only checks that the
            // tears aren't already held.
            CALPICO if !self.has(ctx, SHILENS_TEARS) => {
                ctx.give_items(SHILENS_TEARS, 1);
                ctx.set_cond(6, true);
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc = ctx.npc_id;
        if ctx.is_created() {
            if npc == TRISKEL {
                return Some(
                    if self.has(ctx, IRON_HEART) {
                        "30416-04.htm"
                    } else {
                        "30416-01.htm"
                    }
                    .to_string(),
                );
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        match npc {
            TRISKEL => self.talk_triskel(ctx),
            GUARD_LEIKAN => self.talk_leikan(ctx),
            ARKENIA => self.talk_arkenia(ctx),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

impl Q00411PathOfTheAssassin {
    fn talk_triskel(&self, ctx: &mut QuestCtx) -> Option<String> {
        match self.token(ctx) {
            Some(ARKENIAS_RECOMMENDATION) => {
                ctx.give_items(IRON_HEART, 1);
                // Java's three-way level branch awards identical exp/sp.
                ctx.add_exp_and_sp(80314, 5087);
                ctx.exit_quest(false, true);
                ctx.social_action(3);
                Some("30416-06.html".to_string())
            }
            Some(ARKENIAS_LETTER) => Some("30416-07.html".to_string()),
            Some(LEIKANS_NOTE) => Some("30416-08.html".to_string()),
            Some(SHILENS_TEARS) => Some("30416-10.html".to_string()),
            Some(SHILENS_CALL) => Some("30416-11.html".to_string()),
            // Between hand-overs.
            None => Some("30416-09.html".to_string()),
            _ => Some(ctx.no_quest_html()),
        }
    }

    fn talk_leikan(&self, ctx: &mut QuestCtx) -> Option<String> {
        let molars = ctx.quest_items_count(MOONSTONE_BEASTS_MOLAR);
        match self.token(ctx) {
            Some(ARKENIAS_LETTER) if molars == 0 => Some("30382-01.html".to_string()),
            Some(LEIKANS_NOTE) if molars == 0 => Some("30382-05.html".to_string()),
            Some(LEIKANS_NOTE) if molars < MOLARS_NEEDED => Some("30382-06.html".to_string()),
            Some(LEIKANS_NOTE) => {
                ctx.take_items(LEIKANS_NOTE, 1);
                ctx.take_items(MOONSTONE_BEASTS_MOLAR, -1);
                ctx.set_cond(5, true);
                Some("30382-07.html".to_string())
            }
            Some(SHILENS_TEARS) => Some("30382-08.html".to_string()),
            None if molars == 0 => Some("30382-09.html".to_string()),
            _ => Some(ctx.no_quest_html()),
        }
    }

    fn talk_arkenia(&self, ctx: &mut QuestCtx) -> Option<String> {
        match self.token(ctx) {
            Some(SHILENS_CALL) => Some("30419-01.html".to_string()),
            Some(ARKENIAS_LETTER) => Some("30419-07.html".to_string()),
            Some(SHILENS_TEARS) => {
                ctx.take_items(SHILENS_TEARS, 1);
                ctx.give_items(ARKENIAS_RECOMMENDATION, 1);
                ctx.set_cond(7, true);
                Some("30419-08.html".to_string())
            }
            Some(ARKENIAS_RECOMMENDATION) => Some("30419-09.html".to_string()),
            Some(LEIKANS_NOTE) => Some("30419-10.html".to_string()),
            None => Some("30419-11.html".to_string()),
            _ => Some(ctx.no_quest_html()),
        }
    }
}
